//! Launch identity comes from the installed library, never artwork or rating IDs.
use std::{collections::BTreeMap, path::{Path, PathBuf}, time::{Duration, Instant}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation { pub app_id: u32, pub name: String, pub directory: PathBuf }

pub fn installation_root(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|folder| {
        folder.parent().is_some_and(|common| common.file_name().is_some_and(|name| name.eq_ignore_ascii_case("common"))
            && common.parent().is_some_and(|apps| apps.file_name().is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))))
    }).map(Path::to_path_buf)
}

// KeyValues tokens retain quoted strings and skip comments. Only direct AppState
// fields count; similarly named fields inside depot/user objects cannot override them.
fn fields(text: &str) -> Option<BTreeMap<String, String>> {
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    let mut tokens = Vec::new();
    while let Some(char) = chars.next() {
        match char {
            char if char.is_whitespace() => {},
            '/' if chars.peek() == Some(&'/') => { for char in chars.by_ref() { if char == '\n' { break; } } },
            '{' | '}' => tokens.push(char.to_string()),
            '"' => {
                let mut value = String::new(); let mut closed = false;
                while let Some(char) = chars.next() {
                    match char {
                        '"' => { closed = true; break; },
                        '\\' => { let next = chars.next()?; if !['\\', '"'].contains(&next) { value.push('\\'); } value.push(next); },
                        _ => value.push(char),
                    }
                }
                if !closed { return None; } tokens.push(value);
            },
            _ => return None,
        }
    }
    if tokens.first()? != "AppState" || tokens.get(1)? != "{" { return None; }
    let mut result = BTreeMap::new(); let mut index = 2;
    while tokens.get(index)? != "}" {
        let key = tokens.get(index)?.to_ascii_lowercase(); index += 1;
        let value = tokens.get(index)?; index += 1;
        if value == "{" {
            let mut depth = 1;
            while depth > 0 {
                match tokens.get(index)?.as_str() { "{" => depth += 1, "}" => depth -= 1, _ => {} }
                index += 1;
            }
        } else if value == "}" || result.insert(key, value.clone()).is_some() { return None; }
    }
    (index + 1 == tokens.len()).then_some(result)
}

fn manifest(text: &str, filename: &str, directory: &Path) -> Option<Installation> {
    let expected: u32 = filename.to_ascii_lowercase().strip_prefix("appmanifest_")?.strip_suffix(".acf")?.parse().ok()?;
    let fields = fields(text)?;
    let id = fields.get("appid")?;
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
    let app_id: u32 = id.parse().ok()?;
    if app_id == 0 || app_id != expected { return None; }
    let install = fields.get("installdir")?;
    if install.is_empty() || install.contains(['/', '\\', ':']) || [".", ".."].contains(&install.as_str())
        || !directory.file_name()?.to_string_lossy().eq_ignore_ascii_case(install) { return None; }
    Some(Installation { app_id, name: fields.get("name").cloned().unwrap_or_else(|| install.clone()), directory: directory.to_path_buf() })
}

pub fn installation(path: &Path) -> Option<Installation> {
    let directory = installation_root(path)?;
    let apps = directory.parent()?.parent()?;
    let mut matched = None;
    for entry in std::fs::read_dir(apps).ok()?.flatten() {
        let filename = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !filename.starts_with("appmanifest_") || !filename.ends_with(".acf") { continue; }
        let Ok(metadata) = entry.metadata() else { continue; };
        if !metadata.is_file() || metadata.len() > 128 * 1024 { continue; }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue; };
        if let Some(found) = manifest(&text, &filename, &directory) {
            if matched.is_some() { return None; } // Ambiguous installs cannot select another app.
            matched = Some(found);
        }
    }
    matched
}

#[cfg(windows)]
fn registry_string(name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let key: Vec<u16> = "Software\\Valve\\Steam".encode_utf16().chain(Some(0)).collect();
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut buffer = vec![0_u16; 32768]; let mut size = (buffer.len() * 2) as u32;
    let status = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
    if status != 0 || size < 2 { return None; }
    let value = &buffer[..(size as usize / 2).min(buffer.len())];
    Some(String::from_utf16_lossy(&value[..value.iter().position(|char| *char == 0).unwrap_or(value.len())]))
}

fn validated_client(path: PathBuf) -> Option<PathBuf> {
    let path = std::fs::canonicalize(path).ok()?;
    (path.is_file() && path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("steam.exe"))).then_some(path)
}

pub fn client(installation: &Installation) -> Result<PathBuf, String> {
    #[cfg(windows)]
    for path in [registry_string("SteamExe").map(PathBuf::from), registry_string("SteamPath").map(|path| PathBuf::from(path).join("steam.exe"))].into_iter().flatten() {
        if let Some(path) = validated_client(path) { return Ok(path); }
    }
    // The primary library lives alongside steam.exe; other drives use the registry.
    if let Some(path) = installation.directory.parent().and_then(Path::parent).and_then(Path::parent).and_then(|root| validated_client(root.join("steam.exe"))) { return Ok(path); }
    Err("Steam is required to launch this game. Open Steam or repair its installation, then try again.".into())
}

pub fn running(app_id: u32) -> bool {
    #[cfg(windows)] {
        use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let key: Vec<u16> = format!("Software\\Valve\\Steam\\Apps\\{app_id}").encode_utf16().chain(Some(0)).collect();
        let name: Vec<u16> = "Running".encode_utf16().chain(Some(0)).collect();
        let mut value = 0_u32; let mut size = 4_u32;
        let status = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut value as *mut u32).cast(), &mut size) };
        status == 0 && value != 0
    }
    #[cfg(not(windows))] { let _ = app_id; false }
}

#[derive(Debug, PartialEq, Eq)]
enum Status { Waiting, Started, Running, Ended, TimedOut }
#[derive(Default)]
struct Monitor { started: bool, stopped_at: Option<Duration> }
impl Monitor {
    fn update(&mut self, running: bool, elapsed: Duration) -> Status {
        if running {
            self.stopped_at = None;
            if !self.started { self.started = true; return Status::Started; }
            return Status::Running;
        }
        if self.started {
            let stopped = *self.stopped_at.get_or_insert(elapsed);
            if elapsed.saturating_sub(stopped) >= Duration::from_secs(3) { Status::Ended } else { Status::Running }
        } else if elapsed >= Duration::from_secs(180) { Status::TimedOut } else { Status::Waiting }
    }
}

// The Steam command process may exit immediately or stay alive as the client.
// Neither lifetime is the game's session, so observe Steam's per-app state instead.
pub struct Session { app_id: u32, clock: Instant, monitor: Monitor }
impl Session {
    pub fn wait(mut self) -> Result<(), String> {
        loop {
            if self.monitor.update(running(self.app_id), self.clock.elapsed()) == Status::Ended { return Ok(()); }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
pub fn wait_for_start(app_id: u32) -> Result<Session, String> {
    let mut session = Session { app_id, clock: Instant::now(), monitor: Monitor::default() };
    loop {
        match session.monitor.update(running(app_id), session.clock.elapsed()) {
            Status::Started => return Ok(session),
            Status::TimedOut => return Err("Steam did not report the game as running. Check Steam for a launch dialog or error and try again.".into()),
            _ => {},
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(id: &str, directory: &str) -> String { format!("\"AppState\" {{ \"appid\" \"{id}\" \"name\" \"Installed game\" \"installdir\" \"{directory}\" }}") }
    #[test] fn manifest_identity_requires_the_matching_install_and_filename() {
        let path = Path::new("/library/steamapps/common/Game with spaces");
        assert_eq!(manifest(&text("1325200", "Game with spaces"), "appmanifest_1325200.acf", path).unwrap().app_id, 1325200);
        for input in [text("0", "Game with spaces"), text("42", "Game with spaces"), text("1325200 --bad", "Game with spaces"), text("1325200", "Other game"), text("1325200", "../Game with spaces")] {
            assert!(manifest(&input, "appmanifest_1325200.acf", path).is_none());
        }
    }
    #[test] fn parser_ignores_nested_fields_and_handles_comments_and_escaped_names() {
        let input = "// installed game\n\"AppState\" { \"appid\" \"12\" \"name\" \"Game \\\"Deluxe\\\"\" \"installdir\" \"Game\" \"UserConfig\" { \"appid\" \"99\" \"installdir\" \"Wrong\" } }";
        let found = manifest(input, "appmanifest_12.acf", Path::new("/library/steamapps/common/Game")).unwrap();
        assert_eq!(found.app_id, 12); assert_eq!(found.name, "Game \"Deluxe\"");
        assert!(fields("\"AppState\" { \"appid\" \"1\" \"appid\" \"2\" }").is_none());
        assert!(fields("\"AppState\" { \"name\" \"unfinished").is_none());
    }
    #[test] fn nested_game_paths_resolve_the_install_not_a_similarly_named_folder() {
        let path = Path::new("/secondary/steamapps/common/Game/Binaries/Win64/game.exe");
        assert_eq!(installation_root(path).unwrap(), Path::new("/secondary/steamapps/common/Game"));
        assert!(installation_root(Path::new("/games/Game/steam_api64.dll")).is_none());
        assert!(installation_root(Path::new("/games/common/Game/game.exe")).is_none());
    }
    #[test] fn tracking_waits_for_the_game_and_survives_a_launcher_handoff() {
        let mut monitor = Monitor::default();
        let seconds = Duration::from_secs;
        assert_eq!(monitor.update(false, seconds(0)), Status::Waiting);
        assert_eq!(monitor.update(false, seconds(10)), Status::Waiting);
        assert_eq!(monitor.update(true, seconds(11)), Status::Started);
        assert_eq!(monitor.update(false, seconds(15)), Status::Running);
        assert_eq!(monitor.update(true, seconds(16)), Status::Running);
        assert_eq!(monitor.update(false, seconds(20)), Status::Running);
        assert_eq!(monitor.update(false, seconds(23)), Status::Ended);
        assert_eq!(Monitor::default().update(false, seconds(180)), Status::TimedOut);
    }
}
