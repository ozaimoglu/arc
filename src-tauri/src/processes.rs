//! Observe game installations across client/bootstrap handoffs, never the shared store client.
use std::{path::{Path, PathBuf}, time::{Duration, Instant}};

fn game_process(path: &Path, install: &Path) -> bool {
    let mut parts = path.components();
    if install.components().any(|root| parts.next().is_none_or(|part| !part.as_os_str().eq_ignore_ascii_case(root.as_os_str()))) { return false; }
    if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("exe")) { return false; }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    !crate::scanner::blocked(&name) && !crate::scanner::excluded_path(path)
        && !name.to_ascii_lowercase().contains("bootstrap")
        && !["steam.exe", "EpicGamesLauncher.exe", "RiotClientServices.exe", "Battle.net.exe", "GalaxyClient.exe", "UbisoftConnect.exe", "upc.exe", "EADesktop.exe", "EALauncher.exe", "Origin.exe", "SocialClubHelper.exe", "RockstarService.exe", "PlayGTAV.exe", "VALORANT.exe", "start_protected_game.exe"].iter().any(|client| name.eq_ignore_ascii_case(client))
}

pub fn running(install: &Path) -> Result<bool, String> {
    #[cfg(windows)] {
        use std::mem::size_of;
        use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::{Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS}, Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION}}};
        let install = std::fs::canonicalize(install).unwrap_or_else(|_| install.to_path_buf());
        let root = PathBuf::from(crate::models::path_string(&install));
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE { return Err("Windows could not read the running game processes.".into()); }
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() }; entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut next = unsafe { Process32FirstW(snapshot, &mut entry) };
        while next != 0 {
            let end = entry.szExeFile.iter().position(|char| *char == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if !crate::scanner::blocked(&name) {
                let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID) };
                if !process.is_null() {
                    let mut buffer = [0_u16; 32768]; let mut size = buffer.len() as u32;
                    if unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) } != 0 {
                        let image = PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize]));
                        // Process image names retain the spelling used at launch, including
                        // 8.3 aliases. Resolve both sides to the same filesystem identity.
                        let image = std::fs::canonicalize(&image).unwrap_or(image);
                        let path = PathBuf::from(crate::models::path_string(&image));
                        found = game_process(&path, &root);
                    }
                    unsafe { CloseHandle(process); }
                    if found { break; }
                }
            }
            next = unsafe { Process32NextW(snapshot, &mut entry) };
        }
        unsafe { CloseHandle(snapshot); }
        Ok(found)
    }
    #[cfg(not(windows))] { let _ = install; Err("Launcher process tracking is supported on Windows only.".into()) }
}

#[derive(Debug, PartialEq, Eq)]
enum Status { Waiting, Started, Running, Ended, TimedOut }
#[derive(Default)]
struct Monitor { started: bool, absent_since: Option<Duration> }
impl Monitor {
    fn update(&mut self, present: bool, elapsed: Duration) -> Status {
        if present {
            self.absent_since = None;
            if !self.started { self.started = true; return Status::Started; }
            return Status::Running;
        }
        if self.started {
            let absent = *self.absent_since.get_or_insert(elapsed);
            if elapsed.saturating_sub(absent) >= Duration::from_secs(10) { Status::Ended } else { Status::Running }
        } else if elapsed >= Duration::from_secs(180) { Status::TimedOut } else { Status::Waiting }
    }
}

pub struct Session { install: PathBuf, clock: Instant, monitor: Monitor }
impl Session {
    pub fn wait(mut self) -> Result<(), String> {
        loop {
            if self.monitor.update(running(&self.install)?, self.clock.elapsed()) == Status::Ended { return Ok(()); }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
pub fn wait_for_start(install: PathBuf) -> Result<Session, String> {
    let mut session = Session { install, clock: Instant::now(), monitor: Monitor::default() };
    loop {
        match session.monitor.update(running(&session.install)?, session.clock.elapsed()) {
            Status::Started => return Ok(session),
            Status::TimedOut => return Err("The launcher did not start a game process. Check it for a login, update, Play button or error, then try again.".into()),
            _ => {},
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn shared_clients_helpers_and_neighboring_installations_do_not_count_as_gameplay() {
        let root = Path::new("/games/Game");
        assert!(game_process(Path::new("/games/Game/Binaries/Win64/ActualGame.exe"), root));
        assert!(game_process(Path::new("/games/Game/LeagueClient.exe"), root));
        assert!(game_process(Path::new("/GAMES/GAME/bin/ActualGame.exe"), root));
        for path in ["/games/GameNeighbor/ActualGame.exe", "/launchers/EpicGamesLauncher.exe", "/games/Game/RiotClientServices.exe", "/games/Game/CrashReportClient.exe", "/games/Game/Launcher.exe", "/games/Game/Backup/ActualGame.exe", "/games/Game/FortniteBootstrapper.exe", "/games/Game/PlayGTAV.exe", "/games/Game/VALORANT.exe", "/games/Game/start_protected_game.exe"] {
            assert!(!game_process(Path::new(path), root), "{path}");
        }
    }
    #[cfg(windows)]
    #[test] fn windows_observes_the_game_image_after_a_launcher_command_exits() {
        use std::{os::windows::{ffi::OsStrExt, process::CommandExt}, process::{Command, Stdio}};
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
        let root = std::env::temp_dir().join(format!("arc-process-test-{}-{}", std::process::id(), crate::models::now()));
        std::fs::create_dir_all(&root).unwrap();
        let image = root.join("ActualGame.exe");
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/ping.exe");
        std::fs::copy(system, &image).unwrap();
        let input: Vec<u16> = image.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut output = [0_u16; 32768];
        let length = unsafe { GetShortPathNameW(input.as_ptr(), output.as_mut_ptr(), output.len() as u32) } as usize;
        let image = if length > 0 && length < output.len() { PathBuf::from(String::from_utf16_lossy(&output[..length])) } else { image };
        let mut child = Command::new(&image).args(["-n", "20", "127.0.0.1"]).creation_flags(0x08000000).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let install = std::fs::canonicalize(&root).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut result = running(&install);
        while result.as_ref().is_ok_and(|present| !present) && Instant::now() < deadline && child.try_wait().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(20));
            result = running(&install);
        }
        let status = child.try_wait().unwrap();
        let _ = child.kill(); let _ = child.wait();
        let stopped = running(&install);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(result.unwrap(), "image={}, install={}, status={status:?}", image.display(), install.display()); assert!(!stopped.unwrap());
    }
    #[test] fn sessions_wait_for_real_game_start_and_survive_bootstrap_gaps() {
        let seconds = Duration::from_secs; let mut monitor = Monitor::default();
        assert_eq!(monitor.update(false, seconds(0)), Status::Waiting);
        assert_eq!(monitor.update(false, seconds(120)), Status::Waiting);
        assert_eq!(monitor.update(true, seconds(130)), Status::Started);
        assert_eq!(monitor.update(false, seconds(140)), Status::Running);
        assert_eq!(monitor.update(true, seconds(149)), Status::Running);
        assert_eq!(monitor.update(false, seconds(200)), Status::Running);
        assert_eq!(monitor.update(false, seconds(210)), Status::Ended);
        assert_eq!(Monitor::default().update(false, seconds(180)), Status::TimedOut);
    }
}
