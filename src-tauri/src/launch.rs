use std::path::PathBuf;
use crate::{models::{path_string, ConsoleLaunch, Game}, ps4};

#[derive(Debug)]
pub struct Plan { pub program: PathBuf, pub args: Vec<String>, pub directory: PathBuf }

fn executable(path: &str, expected_name: Option<&str>) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|_| "The game or emulator executable is missing. Scan your library to refresh it.".to_string())?;
    if !path.is_file() || !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || expected_name.is_some_and(|expected| !path.file_name().is_some_and(|name| name.eq_ignore_ascii_case(expected))) {
        return Err("The configured executable is not supported.".into());
    }
    Ok(path)
}

pub fn plan(game: &Game) -> Result<Plan, String> {
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
    Ok(Plan { program, args, directory })
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
}
