//! Cache controls delegate attribution, deletion and driver settings to the official CLI.
use std::{collections::VecDeque, fs, io::{BufRead, BufReader, Write}, path::Path, process::{Command, Stdio}, sync::Mutex, time::{Duration, Instant}};
use serde::{Deserialize, Serialize};
use tauri::State;
use crate::{db, launch, shaders::{self, Action, Job, Manager}, SharedState};

type Result<T> = std::result::Result<T, String>;
const LIMITS: &[&str] = &["default", "1", "5", "10", "20", "50", "100", "unlimited"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheState {
    installed: bool, gpu: String, usage: String, limit: String,
    selected_limit: Option<String>, configurable: bool, busy: bool,
}

fn validate_limit(limit: &str) -> Result<()> {
    if LIMITS.contains(&limit) { Ok(()) } else { Err("Choose a supported shader cache size.".into()) }
}

fn parse_state(lines: &[String], busy: bool) -> Result<CacheState> {
    let gpu = lines.iter().find_map(|line| line.strip_prefix("GPU: ")).ok_or("SCSKiller did not report a GPU. Update the tool and try again.")?;
    let limit = lines.iter().find_map(|line| line.strip_prefix("limit: ")).ok_or("SCSKiller did not report a cache limit.")?;
    let usage = lines.iter().find_map(|line| line.strip_prefix("cache: ")).map(|line| {
        if let Some((_, size)) = line.split_once(" <= ") { format!("Up to {}", size.trim_end_matches(" on disk")) }
        else if line.ends_with(" on disk") {
            line.trim_end_matches(" on disk").split_whitespace().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ")
        } else { line.to_string() }
    }).unwrap_or_else(|| "Unavailable".into());
    let selected = if limit.ends_with(" (driver default)") { Some("default") }
        else if limit == "unlimited" { Some("unlimited") }
        else { limit.strip_suffix(" GB").filter(|size| LIMITS.contains(size)) };
    Ok(CacheState { installed: true, gpu: gpu.into(), usage, limit: limit.into(), selected_limit: selected.map(str::to_string),
        configurable: gpu.contains("(Nvidia),") && limit != "not readable", busy })
}

#[derive(Default)]
struct Output { lines: VecDeque<String>, error: Option<String>, cleared: bool }
impl Output {
    fn push(&mut self, line: &str) {
        let line: String = line.chars().take(2000).collect();
        if line.starts_with("error:") { self.error = Some(line.clone()); }
        self.cleared |= line.starts_with("cleared ");
        self.lines.push_back(line);
        if self.lines.len() > 100 { self.lines.pop_front(); }
    }
}

fn run_cli(cli: &Path, args: &[String], answer: Option<&[u8]>, manager: Option<&Manager>) -> Result<Output> {
    let mut command = Command::new(cli);
    command.args(args).current_dir(cli.parent().ok_or("SCSKiller's installation is unavailable.")?)
        .stdin(if answer.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| format!("Could not start SCSKiller: {e}"))?;
    if let Some(answer) = answer {
        if let Err(error) = child.stdin.take().ok_or("SCSKiller's input is unavailable.")?.write_all(answer) {
            let _ = child.wait(); return Err(error.to_string());
        }
    }
    let stdout = child.stdout.take().ok_or("SCSKiller's output is unavailable.")?;
    let stderr = child.stderr.take().ok_or("SCSKiller's error output is unavailable.")?;
    let output = Mutex::new(Output::default());
    let started = Instant::now();
    let exit = std::thread::scope(|scope| {
        for stream in [Box::new(stdout) as Box<dyn std::io::Read + Send>, Box::new(stderr)] {
            let output = &output;
            scope.spawn(move || {
                for line in BufReader::new(stream).lines().map_while(std::result::Result::ok) {
                    if let Some(manager) = manager { manager.line(&line); }
                    if let Ok(mut output) = output.lock() { output.push(&line); }
                }
            });
        }
        loop {
            match child.try_wait() {
                Ok(Some(exit)) => break Ok(exit),
                Err(error) => break Err(error.to_string()),
                Ok(None) => {}
            }
            // Never terminate a cache deletion halfway through. Only read-only cache queries time out.
            if answer.is_none() && started.elapsed() > Duration::from_secs(30) {
                let _ = child.kill(); let _ = child.wait(); break Err("Reading the shader cache timed out. Try Refresh.".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })?;
    let output = output.into_inner().map_err(|_| "SCSKiller output is unavailable.")?;
    if !exit.success() || output.error.is_some() {
        return Err(output.error.unwrap_or_else(|| format!("SCSKiller failed (exit code {:?}).", exit.code())));
    }
    Ok(output)
}

fn read_state(cli: &Path, busy: bool) -> Result<CacheState> {
    let output = run_cli(cli, &["cache".into(), "get".into()], None, None)?;
    parse_state(&output.lines.into_iter().collect::<Vec<_>>(), busy)
}

#[tauri::command]
pub async fn shader_cache_state(state: State<'_, SharedState>) -> Result<CacheState> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let busy = state.shader.lock()?.busy();
        let settings = db::settings(&db::open(&state.db_path)?)?;
        let Ok(cli) = shaders::configured_tool(&settings.shader_tool) else {
            return Ok(CacheState { installed: false, gpu: String::new(), usage: String::new(), limit: String::new(), selected_limit: None, configurable: false, busy });
        };
        let mut snapshot = read_state(&cli, busy)?;
        snapshot.busy = state.shader.lock()?.busy();
        Ok(snapshot)
    }).await.map_err(|e| e.to_string())?
}

struct LimitLease(SharedState);
impl Drop for LimitLease {
    fn drop(&mut self) { if let Ok(mut runtime) = self.0.shader.lock() { runtime.cache_busy = false; } }
}

#[tauri::command]
pub async fn set_shader_cache_limit(state: State<'_, SharedState>, limit: String) -> Result<String> {
    validate_limit(&limit)?;
    let state = state.inner().clone();
    let cli = shaders::configured_tool(&db::settings(&db::open(&state.db_path)?)?.shader_tool)?;
    {
        let mut runtime = state.shader.lock()?;
        if runtime.busy() { return Err("Finish the current shader operation before changing the cache limit.".into()); }
        if !state.launching.lock().map_err(|_| "Running game state is unavailable.")?.is_empty() { return Err("Close running games before changing the cache limit.".into()); }
        runtime.cache_busy = true;
    }
    // The lease also releases the reservation on errors or a cancelled UAC prompt.
    let lease = LimitLease(state);
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        if !read_state(&cli, true)?.configurable { return Err("This GPU's shader cache size is not configurable through SCSKiller.".into()); }
        // A fresh thread gives the Windows shell its own STA, even if a pooled worker has used COM before.
        let message = std::thread::scope(|scope| scope.spawn(|| elevated_set(&cli, &limit)).join())
            .map_err(|_| "The Windows driver-setting request could not finish.")??;
        let actual = read_state(&cli, true).map_err(|e| format!("The driver setting was applied, but verification failed: {e}"))?;
        if actual.selected_limit.as_deref() != Some(limit.as_str()) { return Err(format!("The driver reported {} after applying the limit. Refresh and review the setting.", actual.limit)); }
        Ok(message)
    }).await.map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ElevatedResult { ok: bool, message: String }
fn result_message(json: &str, exit: u32) -> Result<String> {
    let result: ElevatedResult = serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|_| "SCSKiller did not return a valid result. Refresh the cache setting and try again.")?;
    if !result.ok || exit != 0 { return Err(if result.message.is_empty() { format!("SCSKiller could not change the driver setting (exit code {exit}).") } else { result.message }); }
    if result.message.is_empty() { return Err("SCSKiller did not confirm the cache size change. Refresh and try again.".into()); }
    Ok(result.message)
}

#[cfg(windows)]
fn elevated_set(cli: &Path, limit: &str) -> Result<String> {
    use windows_sys::Win32::{Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0}, System::{Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE}, Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE}}, UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_NOASYNC, SEE_MASK_FLAG_NO_UI}};
    use std::os::windows::ffi::OsStrExt;
    validate_limit(limit)?;
    let initialized = unsafe { CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32) };
    if initialized < 0 { return Err(format!("Windows could not initialize the driver-setting request (error {initialized:#x}).")); }
    struct Apartment;
    impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
    let _apartment = Apartment;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let dir = std::env::temp_dir().join(format!("arc-cache-{}-{tick}-{serial}", std::process::id()));
    fs::create_dir(&dir).map_err(|e| format!("Could not prepare the driver setting request: {e}"))?;
    let result_path = dir.join("result.json");
    let outcome = (|| {
        let wide = |text: &std::ffi::OsStr| text.encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let verb = wide(std::ffi::OsStr::new("runas"));
        let file = wide(cli.as_os_str());
        let directory = wide(cli.parent().ok_or("SCSKiller's directory is unavailable.")?.as_os_str());
        // Fixed tokens and a Windows file path; no shell, scripts or user-provided command text.
        let parameters = format!("cache set {limit} --yes --result \"{}\"", result_path.display());
        let parameters = wide(std::ffi::OsStr::new(&parameters));
        let mut info = SHELLEXECUTEINFOW { cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI, lpVerb: verb.as_ptr(), lpFile: file.as_ptr(),
            lpParameters: parameters.as_ptr(), lpDirectory: directory.as_ptr(), nShow: 0, ..Default::default() };
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            let code = unsafe { GetLastError() };
            return Err(if code == 1223 { "Windows administrator approval was cancelled. The cache limit was not changed.".into() }
                else { format!("Could not request Windows administrator approval (error {code}).") });
        }
        if info.hProcess.is_null() { return Err("Windows did not return the driver-setting process.".into()); }
        let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
        let mut exit = 1;
        let got_exit = unsafe { GetExitCodeProcess(info.hProcess, &mut exit) };
        unsafe { CloseHandle(info.hProcess); }
        if wait != WAIT_OBJECT_0 || got_exit == 0 { return Err("Could not verify completion of the driver-setting process. Refresh the cache setting.".into()); }
        let metadata = fs::metadata(&result_path).map_err(|_| "SCSKiller did not return the driver-setting result. Refresh and try again.")?;
        if metadata.len() > 65536 { return Err("SCSKiller returned an oversized driver-setting result.".into()); }
        use std::io::Read;
        let mut json = String::new();
        fs::File::open(&result_path).map_err(|e| e.to_string())?.take(65537).read_to_string(&mut json).map_err(|e| e.to_string())?;
        if json.len() > 65536 { return Err("SCSKiller returned an oversized driver-setting result.".into()); }
        result_message(&json, exit)
    })();
    let _ = fs::remove_file(&result_path); let _ = fs::remove_dir(&dir);
    outcome
}
#[cfg(not(windows))]
fn elevated_set(_: &Path, _: &str) -> Result<String> { Err("Shader cache settings require Windows.".into()) }

fn clear_args(id: &str, game_precache: bool) -> Result<Vec<String>> {
    if !shaders::valid_id(id) { return Err("SCSKiller's game identity is invalid. Analyze again.".into()); }
    let mut args = vec!["cache".into(), "clear".into(), id.into()];
    if game_precache { args.push("--game-precache".into()); }
    Ok(args)
}

#[tauri::command]
pub async fn clear_shader_cache(state: State<'_, SharedState>, id: i64, game_precache: bool) -> Result<()> {
    let state = state.inner().clone();
    let database = db::open(&state.db_path)?;
    let game = db::game(&database, id)?;
    if game.console_launch.is_some() { return Err("shadPS4 manages its own shader cache.".into()); }
    launch::plan(&game)?;
    let cli = shaders::configured_tool(&db::settings(&database)?.shader_tool)?;
    let status = shaders::game_status(&cli, &game)?.ok_or("Analyze this executable before clearing its cache.")?;
    let args = clear_args(&status.id, game_precache)?;
    {
        let mut runtime = state.shader.lock()?;
        if runtime.busy() { return Err("Finish the current shader operation before clearing a cache.".into()); }
        if state.launching.lock().map_err(|_| "Running game state is unavailable.")?.contains(&id) { return Err("Close this game before clearing its cache.".into()); }
        runtime.begin(Job { game_id: id, title: game.title.clone(), action: Action::ClearCache, running: true, phase: "Clearing".into(), lines: vec![], error: None, stopped: false });
    }
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = (|| {
            let output = run_cli(&cli, &args, Some(b"y\n"), Some(&state.shader))?;
            if !output.cleared { return Err("SCSKiller did not confirm cache cleanup. Review its output and analyze again.".into()); }
            if let Some(job) = &mut state.shader.lock()?.job { job.phase = "Refreshing".into(); }
            shaders::execute(&state.shader, &cli, &game, Action::Analyze, true)
                .map_err(|e| format!("Cache cleanup finished, but the game status could not refresh: {e}"))
        })();
        if let Ok(mut runtime) = state.shader.lock() {
            if let Some(job) = &mut runtime.job {
                job.running = false; job.phase = if outcome.is_ok() { "CacheCleared" } else { "Failed" }.into(); job.error = outcome.err();
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lines(text: &str) -> Vec<String> { text.lines().map(str::to_string).collect() }
    #[test] fn cache_output_preserves_localized_sizes_and_upper_bounds() {
        let state = parse_state(&lines("GPU: RTX 4090 (Nvidia), driver 1, profile nvidia-1\ncache: C:\\A Folder\\DXCache <= 51,5 GB on disk\nlimit: 100 GB"), false).unwrap();
        assert_eq!(state.usage, "Up to 51,5 GB"); assert_eq!(state.selected_limit.as_deref(), Some("100")); assert!(state.configurable);
        let state = parse_state(&lines("GPU: AMD (Amd), driver 1\ncache: 25 MB D3D12 + 35 MB D3D11\nlimit: 4 GB"), true).unwrap();
        assert!(!state.configurable); assert!(state.busy); assert_eq!(state.usage, "25 MB D3D12 + 35 MB D3D11"); assert_eq!(state.selected_limit, None);
    }
    #[test] fn missing_unknown_and_default_driver_settings_are_distinct() {
        assert!(parse_state(&lines("An update is running"), false).is_err());
        for (limit, selected) in [("16 GB (driver default)", Some("default")), ("unlimited", Some("unlimited")), ("not readable", None), ("16 GB", None)] {
            let state = parse_state(&lines(&format!("GPU: RTX (Nvidia), driver 1\ncache: C:\\DXCache 256 MB on disk\nlimit: {limit}")), false).unwrap();
            assert_eq!(state.selected_limit.as_deref(), selected); assert_eq!(state.usage, "256 MB");
            if limit == "not readable" { assert!(!state.configurable); }
        }
    }
    #[test] fn elevation_requires_a_real_success_result_and_exit_code() {
        assert_eq!(result_message(r#"{"Ok":true,"Message":"Applied"}"#, 0).unwrap(), "Applied");
        for (json, exit) in [("", 0), ("{}", 0), (r#"{"Ok":true,"Message":""}"#, 0), (r#"{"Ok":true,"Message":"Applied"}"#, 1), (r#"{"Ok":false,"Message":"Permission denied"}"#, 0)] { assert!(result_message(json, exit).is_err()); }
    }
    #[test] fn cache_commands_reject_injection_and_never_select_every_game() {
        for limit in ["0", "-1", "200", "5 --yes", "unlimited\n", "default; del"] { assert!(validate_limit(limit).is_err()); }
        for id in ["--all-ready", "Game title", "steam:1\ny", "manual:"] { assert!(clear_args(id, true).is_err()); }
        assert_eq!(clear_args("steam:123", false).unwrap(), ["cache", "clear", "steam:123"]);
        assert_eq!(clear_args("manual:abc", true).unwrap(), ["cache", "clear", "manual:abc", "--game-precache"]);
    }
    #[test] fn cleanup_is_not_cancelled_and_global_limit_reservations_block_jobs() {
        let manager = Manager::default();
        manager.lock().unwrap().cache_busy = true; assert!(manager.lock().unwrap().busy());
        manager.lock().unwrap().cache_busy = false;
        manager.lock().unwrap().job = Some(Job { game_id: 1, title: "Game".into(), action: Action::ClearCache, running: true, phase: "Clearing".into(), lines: vec![], error: None, stopped: false });
        assert!(manager.stop().is_ok()); assert!(manager.stop_for(Some(1)).is_err()); assert!(manager.lock().unwrap().busy());
        assert_eq!(manager.lock().unwrap().job.as_ref().unwrap().phase, "Clearing");
    }
    #[test]
    #[ignore = "Opt-in read-only query against an official CLI; never changes driver settings or deletes caches"]
    fn official_cache_query() {
        let cli = shaders::tool_path(&std::env::var("ARC_SHADER_SMOKE_TOOL").expect("Set ARC_SHADER_SMOKE_TOOL")).unwrap();
        let state = read_state(&cli, false).unwrap();
        assert!(!state.gpu.is_empty()); assert!(!state.limit.is_empty());
        println!("Official cache query: {} · {} · limit {}", state.gpu, state.usage, state.limit);
    }
}
