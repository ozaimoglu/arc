use std::path::PathBuf;
use crate::{models::{path_string, ConsoleLaunch, Game}, ps4, steam, stores};

#[derive(Debug)]
pub struct Plan { pub program: PathBuf, pub args: Vec<String>, pub directory: PathBuf, pub steam_app_id: Option<u32>, pub store: Option<stores::Store>, pub tracked_directory: Option<PathBuf> }

// Canonical paths remain the validation/tracking identity. The Windows verbatim
// prefix must not leak into argv[0]: some game runtimes cannot parse it. Only use
// the ordinary spelling when it resolves to the exact same validated path.
fn application_path(path: &std::path::Path) -> PathBuf {
    #[cfg(windows)] {
        let compatible = PathBuf::from(path_string(path));
        if compatible == path || std::fs::canonicalize(&compatible).is_ok_and(|resolved| resolved == path) { return compatible; }
    }
    path.to_path_buf()
}

impl Plan {
    pub fn command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(application_path(&self.program));
        command.args(&self.args).current_dir(application_path(&self.directory));
        command
    }
}

fn executable(path: &str, expected_name: Option<&str>) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|_| "The game or emulator executable is missing. Scan your library to refresh it.".to_string())?;
    if !path.is_file() || !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || expected_name.is_some_and(|expected| !path.file_name().is_some_and(|name| name.eq_ignore_ascii_case(expected))) {
        return Err("The configured executable is not supported.".into());
    }
    Ok(path)
}

pub fn plan(game: &Game) -> Result<Plan, String> {
    if game.console_launch.is_none() {
        let game_exe = executable(&game.exe_path, None)?;
        if let Some(installation) = steam::installation(&game_exe) {
            let program = steam::client(&installation)?;
            let directory = program.parent().ok_or("Steam has no parent folder.")?.to_path_buf();
            return Ok(Plan { program, args: vec!["-applaunch".into(), installation.app_id.to_string()], directory, steam_app_id: Some(installation.app_id), store: None, tracked_directory: None });
        }
        if let Some(route) = stores::route(&game_exe)? {
            let directory = route.program.parent().ok_or("The launcher has no parent folder.")?.to_path_buf();
            return Ok(Plan { program: route.program, args: route.args, directory, steam_app_id: None, store: Some(route.store), tracked_directory: Some(route.install) });
        }
    }
    let (program, args) = if let Some(launch) = &game.console_launch {
        if game.source != ps4::SOURCE { return Err("The PS4 launch record is invalid. Scan your library again.".into()); }
        let eboot = std::fs::canonicalize(&game.exe_path).map_err(|_| "The PS4 game file is missing.".to_string())?;
        let metadata = ps4::metadata(&eboot).ok_or("The PS4 base game metadata is missing or invalid.")?;
        let (emulator, title_id) = match launch {
            ConsoleLaunch::ShadPs4 { emulator, title_id } | ConsoleLaunch::BloodborneLauncher { emulator, title_id, .. } => (emulator, title_id),
        };
        if &metadata.title_id != title_id { return Err("The PS4 game serial changed. Scan your library again.".into()); }
        let emulator = executable(emulator, Some("shadPS4.exe"))?;
        match launch {
            ConsoleLaunch::ShadPs4 { .. } => (emulator, vec!["-g".into(), path_string(&eboot)]),
            ConsoleLaunch::BloodborneLauncher { launcher, .. } => {
                let launcher = executable(launcher, Some("BB_Launcher.exe"))?;
                let (install, configured_emulator) = ps4::bloodborne_configuration(&launcher).ok_or("BBLauncher configuration is missing or invalid.")?;
                if Some(install.as_path()) != eboot.parent() || configured_emulator != emulator || !metadata.title.eq_ignore_ascii_case("Bloodborne") {
                    return Err("BBLauncher points to a different game or emulator. Scan your library again.".into());
                }
                (launcher, vec!["-n".into()])
            },
        }
    } else { (executable(&game.exe_path, None)?, vec![]) };
    let directory = program.parent().ok_or("The executable has no parent folder.")?.to_path_buf();
    Ok(Plan { program, args, directory, steam_app_id: None, store: None, tracked_directory: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, models::{Candidate, GamePatch}};
    fn fixture(label: &str) -> (PathBuf, rusqlite::Connection, i64) {
        let root = std::env::temp_dir().join(format!("arc-ps4-{label}-{}-{}", std::process::id(), crate::models::now()));
        let folder = root.join("Games/GAME/CUSA03173"); std::fs::create_dir_all(folder.join("sce_sys")).unwrap();
        std::fs::write(folder.join("eboot.bin"), b"PS4 payload").unwrap();
        std::fs::write(folder.join("sce_sys/param.sfo"), ps4::tests::sfo("Bloodborne", "CUSA03173", "gd")).unwrap();
        std::fs::write(root.join("shadPS4.exe"), b"fixture executable").unwrap();
        let mut db = rusqlite::Connection::open_in_memory().unwrap(); db::migrate(&mut db).unwrap();
        let candidate = ps4::candidate(&folder.join("eboot.bin")).unwrap();
        let (id, _) = db::upsert(&db, &candidate).unwrap().unwrap(); (root, db, id)
    }
    #[test] fn shad_arguments_keep_a_spaced_path_as_one_argument() {
        let (root, db, id) = fixture("game with spaces");
        let game = db::game(&db, id).unwrap(); let plan = plan(&game).unwrap();
        assert_eq!(plan.args, vec!["-g".to_owned(), path_string(&std::fs::canonicalize(&game.exe_path).unwrap())]);
        assert_eq!(plan.program, std::fs::canonicalize(root.join("shadPS4.exe")).unwrap());
        assert_eq!(plan.directory, plan.program.parent().unwrap());
        std::fs::remove_file(root.join("shadPS4.exe")).unwrap(); assert!(super::plan(&game).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn configured_bloodborne_launcher_preserves_the_dlss_route() {
        let (root, db, id) = fixture("bloodborne");
        let launcher = root.join("Bloodborne/Launcher/BB_Launcher.exe");
        let emulator = root.join("Bloodborne/DLSS/shadPS4.exe");
        std::fs::create_dir_all(launcher.parent().unwrap().join("BBLauncher")).unwrap();
        std::fs::create_dir_all(emulator.parent().unwrap()).unwrap();
        std::fs::write(&launcher, b"fixture executable").unwrap(); std::fs::write(&emulator, b"fixture emulator").unwrap();
        let install = root.join("Games/GAME/CUSA03173");
        let config = launcher.parent().unwrap().join("BBLauncher/LauncherSettings.toml");
        std::fs::write(&config, format!("[Launcher]\ninstallPath = '{}'\nshadPath-New = '{}'\n", install.display(), emulator.display())).unwrap();
        assert!(ps4::bloodborne_configuration(&launcher).is_some(), "{}", std::fs::read_to_string(&config).unwrap());
        let candidate = ps4::candidate(&install.join("eboot.bin")).unwrap();
        assert!(matches!(candidate.console_launch, Some(ConsoleLaunch::BloodborneLauncher { .. })));
        db::patch(&db, id, GamePatch { favorite: Some(true), ..Default::default() }).unwrap();
        db::upsert(&db, &candidate).unwrap();
        let game = db::game(&db, id).unwrap(); assert!(game.favorite);
        let plan = plan(&game).unwrap(); assert_eq!(plan.args, ["-n"]);
        assert_eq!(plan.program, std::fs::canonicalize(&launcher).unwrap());
        // A changed BBLauncher installation must never start another game.
        std::fs::write(config, format!("[Launcher]\ninstallPath = '{}'\nshadPath-New = '{}'\n", root.display(), emulator.display())).unwrap();
        assert!(super::plan(&game).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn ordinary_binary_files_cannot_launch_as_native_executables() {
        let (root, db, id) = fixture("validation");
        let mut game = db::game(&db, id).unwrap();
        game.console_launch = None; assert!(plan(&game).is_err());
        game.console_launch = Some(ConsoleLaunch::ShadPs4 { emulator: path_string(&root.join("shadPS4.exe")), title_id: "CUSA99999".into() });
        assert!(plan(&game).is_err());
        std::fs::write(root.join("native.exe"), b"fixture").unwrap();
        let candidate = Candidate { title: "Native".into(), exe_path: path_string(&root.join("native.exe")), folder: path_string(&root), score: 100, console_launch: None };
        let (id, _) = db::upsert(&db, &candidate).unwrap().unwrap();
        assert!(plan(&db::game(&db, id).unwrap()).unwrap().args.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test] fn windows_child_receives_an_ordinary_executable_path_and_the_game_directory() {
        use std::os::windows::process::CommandExt;
        let root = std::env::temp_dir().join(format!("arc-launch spaced-İ-{}-{}", std::process::id(), crate::models::now()));
        std::fs::create_dir_all(&root).unwrap();
        let program = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let plan = Plan {
            program: std::fs::canonicalize(program).unwrap(),
            args: ["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); [pscustomobject]@{ CommandLine = [Environment]::CommandLine; Directory = [Environment]::CurrentDirectory } | ConvertTo-Json -Compress"].map(str::to_owned).to_vec(),
            directory: std::fs::canonicalize(&root).unwrap(),
            steam_app_id: None, store: None, tracked_directory: None,
        };
        let mut command = plan.command();
        assert_eq!(std::fs::canonicalize(command.get_program()).unwrap(), plan.program);
        assert_eq!(std::fs::canonicalize(command.get_current_dir().unwrap()).unwrap(), plan.directory);
        let output = command.creation_flags(0x08000000).output().unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        // Use explicit UTF-8 so the Unicode-path check is independent of the
        // Windows console code page.
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!value["CommandLine"].as_str().unwrap().contains("\\\\?\\"));
        assert_eq!(value["Directory"].as_str().unwrap(), path_string(&plan.directory));
    }
    #[test] fn steam_launches_use_the_installed_id_even_for_existing_nested_library_records() {
        let (root, db, id) = fixture("steam-route");
        let install = root.join("Steam/steamapps/common/Game with spaces");
        let exe = install.join("Binaries/Win64/game.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"fixture").unwrap();
        std::fs::write(root.join("Steam/steam.exe"), b"client fixture").unwrap();
        let manifest = root.join("Steam/steamapps/appmanifest_3393110.acf");
        std::fs::write(&manifest, "\"AppState\" { \"appid\" \"3393110\" \"name\" \"Installed game\" \"installdir\" \"Game with spaces\" }").unwrap();
        let mut game = db::game(&db, id).unwrap();
        game.console_launch = None; game.exe_path = path_string(&exe);
        // Older records, edited titles and catalogue IDs do not define launch identity.
        game.folder = path_string(&root); game.title = "Custom title".into(); game.sgdb_id = Some(999);
        let steam = plan(&game).unwrap();
        assert_eq!(steam.steam_app_id, Some(3393110));
        assert_eq!(steam.args, ["-applaunch", "3393110"]);
        assert!(steam.program.file_name().unwrap().eq_ignore_ascii_case("steam.exe"));
        assert_ne!(steam.program, std::fs::canonicalize(&exe).unwrap());
        // A local SDK ID is not proof that Steam owns this installation.
        std::fs::remove_file(manifest).unwrap();
        std::fs::write(exe.parent().unwrap().join("steam_appid.txt"), "3393110").unwrap();
        let native = plan(&game).unwrap();
        assert!(native.steam_app_id.is_none()); assert!(native.args.is_empty());
        assert_eq!(native.program, std::fs::canonicalize(&exe).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
