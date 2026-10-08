//! Optional integration with the external SCSKiller CLI; no upstream code is linked.
use std::{fs, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, process::{ChildStdin, Command, Stdio}, sync::{Mutex, MutexGuard}, time::{Duration, Instant}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;
use crate::{db, launch, models::{self, Game}, SharedState};

type Result<T> = std::result::Result<T, String>;
const MAX_JSON: u64 = 16 * 1024 * 1024;

#[derive(Default)]
pub struct Manager { inner: Mutex<Runtime> }
#[derive(Default)]
pub struct Runtime { pub job: Option<Job>, pub cache_busy: bool, stdin: Option<ChildStdin>, stopping: bool }
impl Runtime {
    pub fn busy(&self) -> bool { self.cache_busy || self.job.as_ref().is_some_and(|job| job.running) }
    pub(crate) fn begin(&mut self, job: Job) { self.stopping = false; self.stdin = None; self.job = Some(job); }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub game_id: i64, pub title: String, pub action: Action, pub running: bool,
    pub phase: String, pub lines: Vec<String>, pub error: Option<String>, pub stopped: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action { Analyze, Compile, ClearCache, PrepareRecording }
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    pub id: String, pub status: String, pub reason: String, pub engine: Option<String>,
    pub graphics_api: Option<String>, pub anti_cheat: String, pub shader_count: Option<u64>,
    pub warmed_at: Option<String>, pub driver: Option<String>, pub can_compile: bool,
    pub can_record: bool, pub recorder_installed: bool, pub recording_bytes: u64,
    pub recorded_enough: bool, pub recorder_note: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot { installed: bool, recording_supported: bool, game: Option<GameStatus>, job: Option<Job>, busy: bool, warning: Option<String> }

impl Manager {
    pub fn lock(&self) -> Result<MutexGuard<'_, Runtime>> { self.inner.lock().map_err(|_| "Shader preparation state is unavailable.".into()) }
    pub(crate) fn line(&self, line: &str) {
        if let Ok(mut state) = self.lock() {
            if let Some(job) = &mut state.job {
                job.lines.push(line.chars().take(1500).collect());
                if job.lines.len() > 100 { job.lines.remove(0); }
            }
        }
    }
    pub fn stop(&self) -> Result<()> {
        self.stop_for(None)
    }
    pub(crate) fn stop_for(&self, id: Option<i64>) -> Result<()> {
        let mut state = self.lock()?;
        if id.is_some_and(|id| !state.job.as_ref().is_some_and(|job| job.running && job.game_id == id)) { return Err("There is no running shader operation for this game.".into()); }
        if !state.job.as_ref().is_some_and(|job| job.running) { return Ok(()); }
        if state.job.as_ref().is_some_and(|job| matches!(job.action, Action::ClearCache | Action::PrepareRecording)) {
            return if id.is_some() { Err("This operation must finish before starting another operation.".into()) } else { Ok(()) };
        }
        if state.stopping { return Ok(()); }
        state.stopping = true;
        if let Some(job) = &mut state.job { job.phase = "Stopping".into(); }
        if let Some(input) = &mut state.stdin { input.write_all(b"stop\nquit\n").map_err(|e| e.to_string())?; }
        Ok(())
    }
}

fn local_data_dir() -> Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("SCSKiller")).ok_or_else(|| "SCSKiller integration is supported on Windows only.".into())
}
fn resolved_data_dir(cli: &Path, local: PathBuf) -> PathBuf {
    // SCSKiller 1.2.4 migrates a marked portable package only after an atomic copy completes.
    // The Arc compatibility package has no .portable marker and continues using the shared store.
    for root in cli.parent().into_iter().flat_map(|dir| dir.ancestors().skip(1).take(2)) {
        if root.join(".portable").is_file() {
            let data = root.join("data");
            return if data.join("migrated").is_file() { data } else { local };
        }
    }
    local
}
fn data_dir(cli: &Path) -> Result<PathBuf> { Ok(resolved_data_dir(cli, local_data_dir()?)) }

pub fn tool_path(selected: &str) -> Result<PathBuf> {
    let path = PathBuf::from(selected);
    let cli = if path.is_dir() { path.join("cli/scskiller.exe") }
        else if path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("SCSKiller.exe")) && path.parent().is_some_and(|parent| parent.join("cli/scskiller.exe").is_file()) {
            path.parent().unwrap().join("cli/scskiller.exe")
        } else { path };
    let cli = fs::canonicalize(cli).map_err(|_| "Choose SCSKiller.exe or cli/scskiller.exe from an official SCSKiller installation.".to_string())?;
    if !cli.is_file() || !cli.file_name().is_some_and(|name| name.eq_ignore_ascii_case("scskiller.exe")) || !cli.parent().is_some_and(|p| p.join("SCSKiller.Core.dll").is_file()) {
        return Err("This folder does not contain the SCSKiller command-line tool.".into());
    }
    Ok(cli)
}

pub(crate) fn configured_tool(selected: &str) -> Result<PathBuf> {
    if !selected.is_empty() { return tool_path(selected); }
    let local = local_data_dir()?.parent().unwrap().to_path_buf();
    for folder in [local.join("Programs/SCSKiller/current"), local.join("SCSKiller/current"), local.join("Programs/SCSKiller"), local.join("SCSKiller")] {
        if let Ok(path) = tool_path(&models::path_string(&folder)) { return Ok(path); }
    }
    Err("Connect SCSKiller in Settings to prepare shaders.".into())
}

fn read_json(path: &Path) -> Result<Option<Value>> {
    let file = match fs::File::open(path) { Ok(file) => file, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None), Err(e) => return Err(e.to_string()) };
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_JSON { return Err("The SCSKiller data file is too large.".into()); }
    let mut bytes = Vec::new(); file.take(MAX_JSON + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_JSON { return Err("The SCSKiller data file is too large.".into()); }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    serde_json::from_slice(bytes).map(Some).map_err(|_| "SCSKiller data could not be read. Analyze again or open SCSKiller to repair it.".into())
}

fn invariant_upper(value: &str) -> String {
    // Windows/.NET invariant simple casing does not expand ß into SS or a ligature into several characters.
    value.chars().map(|character| {
        let mut upper = character.to_uppercase();
        let first = upper.next().unwrap_or(character);
        if upper.next().is_none() { first } else { character }
    }).collect()
}
fn path_key(path: &str) -> String {
    let full = fs::canonicalize(path).map(|p| models::path_string(&p)).unwrap_or_else(|_| path.into());
    invariant_upper(full.replace('/', "\\").trim_end_matches('\\'))
}
pub(crate) fn valid_id(id: &str) -> bool {
    id.len() <= 256 && id.split_once(':').is_some_and(|(prefix, name)| !prefix.is_empty() && !name.is_empty() && prefix.bytes().all(|b| b.is_ascii_alphanumeric()))
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
}
fn status_from_list(list: &Value, exe: &str) -> Result<Option<GameStatus>> {
    let entries = list.get("Games").and_then(Value::as_array).ok_or("SCSKiller's game list format is unsupported. Use the official 1.2.3+ CLI.")?;
    let key = path_key(exe);
    let matches: Vec<_> = entries.iter().filter(|entry| entry.pointer("/Game/ExePath").and_then(Value::as_str).is_some_and(|path| path_key(path) == key)).collect();
    if matches.len() > 1 {
        // Store discovery can merge manual imports; duplicate records are accepted only when all identities agree.
        if matches.iter().skip(1).any(|entry| entry.pointer("/Game/Id") != matches[0].pointer("/Game/Id")) { return Err("Several SCSKiller entries point to this executable. Resolve the duplicate in SCSKiller before compiling.".into()); }
    }
    let Some(entry) = matches.first() else { return Ok(None); };
    let id = entry.pointer("/Game/Id").and_then(Value::as_str).filter(|id| valid_id(id)).ok_or("The SCSKiller game identity is invalid.")?.to_owned();
    let raw_status = entry.get("Status").and_then(Value::as_str).unwrap_or("Unknown");
    let no_stutter = entry.get("NoStutter").and_then(Value::as_str);
    let can_compile = matches!(raw_status, "Ready" | "Stale" | "Warmed") && no_stutter.is_none()
        && !entry.get("NotPlanned").and_then(Value::as_bool).unwrap_or(false)
        && !entry.get("ShaderModBlocks").and_then(Value::as_bool).unwrap_or(false);
    Ok(Some(GameStatus { id, can_compile, status: display_status(entry, raw_status).into(),
        reason: no_stutter.or_else(|| entry.get("StatusReason").and_then(Value::as_str)).unwrap_or("Analyze this game again.").into(),
        engine: entry.pointer("/Engine/Family").and_then(Value::as_str).map(str::to_owned),
        graphics_api: entry.pointer("/Engine/GraphicsApi").and_then(Value::as_str).map(str::to_owned),
        anti_cheat: entry.get("AntiCheat").and_then(Value::as_str).unwrap_or("Unknown").into(),
        shader_count: entry.get("ShaderCount").and_then(Value::as_u64),
        warmed_at: entry.get("WarmedAt").and_then(Value::as_str).map(str::to_owned),
        driver: entry.get("WarmedDriverVersion").and_then(Value::as_str).map(str::to_owned),
        can_record: recording_eligible(entry),
        recorder_installed: entry.get("RecorderInstalled").and_then(Value::as_bool).unwrap_or(false),
        recording_bytes: entry.get("RecordingBytes").and_then(Value::as_u64).unwrap_or(0),
        recorded_enough: entry.get("RecordedEnough").and_then(Value::as_bool).unwrap_or(false),
        recorder_note: entry.get("RecorderNote").and_then(Value::as_str).or_else(|| entry.get("RecorderSkip").and_then(Value::as_str)).map(str::to_owned),
    }))
}
fn display_status<'a>(entry: &Value, raw: &'a str) -> &'a str {
    if entry.get("NoStutter").is_some_and(Value::is_string) { return "NoShaderStutter"; }
    if entry.get("CompileUnreached").and_then(Value::as_bool) == Some(true) { return "CompileUnreached"; }
    if raw == "Warmed" {
        // Mirrors upstream's >10% rejected/crashed threshold; unknown historical counts make no claim.
        let items: u64 = ["Recorded", "Generated", "D3D11Shaders", "MiddlewareItems", "Variants"].iter()
            .filter_map(|field| entry.get("Plan")?.get(field)?.as_u64()).fold(0, u64::saturating_add);
        if entry.get("Plan").is_some_and(Value::is_object) && entry.get("LastWarmFailed").and_then(Value::as_u64).is_some_and(|failed|
            failed.saturating_add(entry.get("LastWarmCrashed").and_then(Value::as_u64).unwrap_or(0)) as f64 > 0.1 * items as f64) {
            return "PartlyCompiled";
        }
        if entry.pointer("/Careful/LaunchCompiled").and_then(Value::as_f64).is_some_and(|share| share > 0.2) { return "PartlyWarmed"; }
    }
    if raw == "NeedsRecording" && entry.get("StatusReason").and_then(Value::as_str)
        == Some("EasyAntiCheat blocks the recorder, but you can record at your own risk with an offline session.") { return "NeedsOfflineSession"; }
    raw
}
fn recording_eligible(entry: &Value) -> bool {
    entry.get("AntiCheat").and_then(Value::as_str) == Some("None")
        && entry.get("Engine").is_some_and(Value::is_object)
        && entry.pointer("/Engine/Encrypted").and_then(Value::as_bool) == Some(false)
        && entry.pointer("/Engine/Unsupported").is_some_and(Value::is_null)
        && entry.pointer("/Engine/GraphicsApi").and_then(Value::as_str).is_some_and(|api| api.contains("D3D12"))
        && !entry.get("ShaderModBlocks").and_then(Value::as_bool).unwrap_or(false)
        && !entry.get("NotPlanned").and_then(Value::as_bool).unwrap_or(false)
        && !entry.get("NoStutter").is_some_and(Value::is_string)
}
fn recording_supported(cli: &Path) -> bool {
    cli.parent().and_then(|dir| read_json(&dir.join("arc-compat.json")).ok().flatten())
        .is_some_and(|manifest| manifest.get("Protocol").and_then(Value::as_u64) == Some(1))
}
pub(crate) fn game_status(cli: &Path, game: &Game) -> Result<Option<GameStatus>> {
    if game.console_launch.is_some() { return Ok(None); }
    read_json(&data_dir(cli)?.join("games.json"))?.map(|list| status_from_list(&list, &game.exe_path)).transpose().map(Option::flatten)
}

fn install_root(game: &Game) -> Result<PathBuf> {
    let folder = fs::canonicalize(&game.folder).map_err(|e| e.to_string())?;
    let exe = fs::canonicalize(&game.exe_path).map_err(|e| e.to_string())?;
    let directory = exe.parent().ok_or("The game executable has no directory.")?;
    let mut candidate = folder.clone();
    if let Some(parent) = directory.parent() {
        if parent.file_name().is_some_and(|name| name.eq_ignore_ascii_case("Binaries")) {
            if let Some(project) = parent.parent() {
                candidate = project.parent().filter(|above| above.join("Engine").is_dir()).unwrap_or(project).to_path_buf();
            }
        } else if parent.file_name().is_some_and(|name| name.eq_ignore_ascii_case("bin")) {
            if let Some(root) = parent.parent() { candidate = root.to_path_buf(); }
        } else if directory.file_name().is_some_and(|name| name.to_string_lossy().to_ascii_lowercase().starts_with("bin")
            || (name.eq_ignore_ascii_case("Retail") && parent.join("Runtime").is_dir())) {
            candidate = parent.to_path_buf();
        }
    }
    // An outer library/delivery folder is not necessarily the engine's install root.
    // Narrow known layouts, but never expand the scan beyond Arc's existing game folder.
    let root_key = path_key(&models::path_string(&candidate));
    let folder_key = path_key(&models::path_string(&folder));
    if candidate.is_dir() && (root_key == folder_key || root_key.starts_with(&format!("{folder_key}\\"))) { Ok(candidate) } else { Ok(folder) }
}
fn confirmed_recording_root(list: &Value, game: &Game) -> Result<Option<PathBuf>> {
    let entries = list.as_array().ok_or("The manual game list is invalid; recording setup was left unchanged.")?;
    let Some(entry) = entries.iter().find(|entry| entry.get("Exe").and_then(Value::as_str).is_some_and(|exe| path_key(exe) == path_key(&game.exe_path))
        && entry.get("Confirmed").and_then(Value::as_bool) == Some(true)) else { return Ok(None); };
    let root = entry.get("InstallDir").and_then(Value::as_str).ok_or("The confirmed game folder is missing.")?;
    let root = fs::canonicalize(root).map_err(|_| "The confirmed game folder is unavailable.")?;
    let root_key = path_key(&models::path_string(&root)); let folder_key = path_key(&game.folder);
    if (root_key == folder_key || root_key.starts_with(&format!("{folder_key}\\"))) && path_key(&game.exe_path).starts_with(&format!("{root_key}\\")) { Ok(Some(root)) }
    else { Err("The previously confirmed folder does not match this Arc installation. Review the game folder in SCSKiller.".into()) }
}
fn recording_root(cli: &Path, game: &Game) -> Result<PathBuf> {
    match read_json(&data_dir(cli)?.join("manual-games.json"))? {
        Some(list) => confirmed_recording_root(&list, game)?.map(Ok).unwrap_or_else(|| install_root(game)),
        None => install_root(game),
    }
}

fn merge_manual(list: &mut Value, game: &Game) -> Result<bool> {
    let entries = list.as_array_mut().ok_or("The SCSKiller manual game list is invalid; it was left unchanged.")?;
    let root = models::path_string(&install_root(game)?);
    if let Some(entry) = entries.iter_mut().find(|entry| entry.get("Exe").and_then(Value::as_str).is_some_and(|exe| path_key(exe) == path_key(&game.exe_path))) {
        if entry.get("Confirmed").and_then(Value::as_bool) == Some(true) || entry.get("InstallDir").and_then(Value::as_str).is_some_and(|folder| path_key(folder) == path_key(&root)) { return Ok(false); }
        let Some(previous) = entry.get("InstallDir").and_then(Value::as_str) else { return Ok(false); };
        if !path_key(&root).starts_with(&format!("{}\\", path_key(previous))) { return Ok(false); }
        entry.as_object_mut().ok_or("The manual game entry is invalid.")?.insert("InstallDir".into(), json!(root));
        return Ok(true);
    }
    // Unconfirmed roots deliberately keep SCSKiller's recorder disabled for newly imported games.
    entries.push(json!({"Exe": models::path_string(&fs::canonicalize(&game.exe_path).map_err(|e| e.to_string())?),
        "InstallDir": root, "Name": game.title, "Confirmed": false}));
    Ok(true)
}

fn ensure_manual(cli: &Path, game: &Game) -> Result<bool> {
    if game_status(cli, game)?.is_some_and(|status| !status.id.starts_with("manual:")) { return Ok(false); }
    let dir = data_dir(cli)?; fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let _lock = ManualLock::acquire(&dir)?;
    let path = dir.join("manual-games.json");
    let mut list = read_json(&path)?.unwrap_or_else(|| json!([]));
    if !merge_manual(&mut list, game)? { return Ok(false); }
    if path.is_file() { fs::copy(&path, dir.join("manual-games.arc-backup.json")).map_err(|e| e.to_string())?; }
    let temporary = dir.join(format!("manual-games.arc-{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
    let mut file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?; drop(file);
    fs::rename(&temporary, path).map_err(|e| e.to_string())?;
    Ok(true)
}

// SCSKiller's documented manual-game JSON is updated under its cross-process Windows mutex.
#[cfg(windows)]
struct ManualLock(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ManualLock {
    fn acquire(dir: &Path) -> Result<Self> {
        use windows_sys::Win32::{Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0}, System::Threading::{CreateMutexW, WaitForSingleObject}};
        let digest = Sha256::digest(invariant_upper(&models::path_string(dir)).as_bytes());
        let suffix: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
        let name: Vec<u16> = format!("Local\\SCSKiller.manual-games.{suffix}").encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() { return Err("Could not lock SCSKiller's game list.".into()); }
        if !matches!(unsafe { WaitForSingleObject(handle, 5000) }, WAIT_OBJECT_0 | WAIT_ABANDONED) {
            unsafe { CloseHandle(handle); } return Err("SCSKiller is updating its game list. Try again in a moment.".into());
        }
        Ok(Self(handle))
    }
}
#[cfg(windows)]
impl Drop for ManualLock {
    fn drop(&mut self) { unsafe { windows_sys::Win32::System::Threading::ReleaseMutex(self.0); windows_sys::Win32::Foundation::CloseHandle(self.0); } }
}
#[cfg(not(windows))]
struct ManualLock;
#[cfg(not(windows))]
impl ManualLock { fn acquire(_: &Path) -> Result<Self> { Err("Shader integration requires Windows.".into()) } }

fn queue_stage(line: &str, id: &str) -> Option<&'static str> {
    let words: Vec<_> = line.split_whitespace().take(3).collect();
    if words.len() != 3 || words[1] != id || words[0].len() != 8 || !words[0].contains(':') { return None; }
    match words[2] { "Waiting" => Some("Waiting"), "Indexing" => Some("Indexing"), "Planning" => Some("Planning"), "Materializing" => Some("Materializing"), "Warming" => Some("Warming"), "Paused" => Some("Paused"), "Done" => Some("Done"), "Failed" => Some("Failed"), "Stopped" => Some("Stopped"), _ => None }
}

pub(crate) fn execute(manager: &Manager, cli: &Path, game: &Game, action: Action, imported: bool) -> Result<()> {
    launch::plan(game)?;
    let recording_target = if action == Action::PrepareRecording {
        if !recording_supported(cli) { return Err("Recording setup requires the Arc compatibility CLI. See tools/scskiller-compat in the Arc repository.".into()); }
        execute(manager, cli, game, Action::Analyze, false)?;
        let status = game_status(cli, game)?.ok_or("Analyze this exact game executable before preparing a recording.")?;
        if !status.can_record { return Err("Recording is unavailable for this build, graphics API or anti-cheat configuration.".into()); }
        if let Some(job) = &mut manager.lock()?.job { job.phase = "PreparingRecording".into(); }
        Some(status.id)
    } else { None };
    let target = if action == Action::Compile {
        let status = game_status(cli, game)?.ok_or("Analyze this game before compiling.")?;
        if !status.can_compile { return Err(status.reason); } Some(status.id)
    } else { None };
    if manager.lock()?.stopping { return Ok(()); }
    let mut command = Command::new(cli);
    if let Some(id) = &recording_target {
        command.args(["arc-record", "prepare", id]).arg(models::path_string(&recording_root(cli, game)?));
    } else if action == Action::Analyze {
        command.arg("scan");
        if !imported { command.arg("--rescan"); }
    } else { command.arg("queue"); }
    command.current_dir(cli.parent().ok_or("SCSKiller has no installation directory.")?)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| format!("Could not start SCSKiller: {e}"))?;
    let input = child.stdin.take().ok_or("SCSKiller's input pipe is unavailable.")?;
    let stdout = child.stdout.take().ok_or("SCSKiller's output pipe is unavailable.")?;
    let stderr = child.stderr.take().ok_or("SCSKiller's error pipe is unavailable.")?;
    {
        let mut state = manager.lock()?; state.stdin = Some(input);
        let stop = state.stopping;
        if let Some(input) = &mut state.stdin {
            if let Some(id) = &target {
                let commands = if stop { "quit\n".into() } else { format!("add {id}\nstart\n") };
                input.write_all(commands.as_bytes()).map_err(|e| e.to_string())?;
            }
        }
    }
    // Both streams are drained concurrently; a verbose scan must never fill an unread pipe.
    let started = Instant::now();
    let mut terminal = None;
    let outcome = std::thread::scope(|scope| {
        let output = scope.spawn(|| {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break }; manager.line(&line);
                if let Some(stage) = target.as_ref().and_then(|id| queue_stage(&line, id)) {
                    if let Ok(mut state) = manager.lock() {
                        if !state.stopping { if let Some(job) = &mut state.job { job.phase = stage.into(); } }
                        if matches!(stage, "Done" | "Failed" | "Stopped") {
                            terminal = Some(stage);
                            if let Some(input) = &mut state.stdin { let _ = input.write_all(b"quit\n"); }
                        }
                    }
                }
                // QueueShell reports admission failures on stdout and otherwise waits for another command.
                if line.starts_with("error:") { if let Ok(mut state) = manager.lock() { if let Some(input) = &mut state.stdin { let _ = input.write_all(b"quit\n"); } } }
            }
        });
        let errors = scope.spawn(|| { for line in BufReader::new(stderr).lines().map_while(std::result::Result::ok) { manager.line(&line); } });
        let result = loop {
            match child.try_wait() {
                Ok(Some(exit)) => break Ok(exit),
                Err(e) => break Err(e.to_string()),
                Ok(None) => {}
            }
            let stopping = manager.lock().map(|state| state.stopping).unwrap_or(true);
            if action == Action::Analyze && (stopping || started.elapsed() > Duration::from_secs(600)) {
                let _ = child.kill(); let _ = child.wait();
                break if stopping { Ok(std::process::ExitStatus::default()) } else { Err("SCSKiller analysis timed out. Try again or inspect its logs.".into()) };
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let _ = output.join(); let _ = errors.join(); result
    });
    manager.lock()?.stdin = None;
    let exit = outcome?;
    if manager.lock()?.stopping { return Ok(()); }
    if !exit.success() || (action == Action::Compile && terminal != Some("Done")) {
        let runtime = manager.lock()?;
        let reason = runtime.job.as_ref().and_then(|job| job.lines.iter().rev().find(|line| line.starts_with("error:") || target.as_ref().is_some_and(|id| queue_stage(line, id) == Some("Failed")))).cloned();
        return Err(reason.unwrap_or_else(|| "SCSKiller could not finish this operation. See the output below.".into()));
    }
    if action == Action::PrepareRecording {
        execute(manager, cli, game, Action::Analyze, true)?;
        if !game_status(cli, game)?.is_some_and(|status| status.recorder_installed) {
            return Err("Recorder setup did not produce a verified installed recorder. Inspect the operation output.".into());
        }
    }
    if action == Action::Compile {
        if let Some(job) = &mut manager.lock()?.job { job.phase = "Refreshing".into(); }
        // CLI queue updates state.json, but games.json is only refreshed by a scan.
        // Re-evaluate the real driver/cache state before reporting completion to Arc.
        execute(manager, cli, game, Action::Analyze, true)
            .map_err(|error| format!("The compile queue finished, but its status could not refresh: {error}"))?;
        if !manager.lock()?.stopping && game_status(cli, game)?.is_none_or(|status| status.warmed_at.is_none()) {
            return Err("The queue finished without a verified preparation timestamp. Analyze again and inspect SCSKiller's output.".into());
        }
    }
    if action == Action::Analyze {
        let status = game_status(cli, game)?;
        // Discover store entries first; then repair a suggested manual root or add the missing executable.
        if !imported && (status.is_none() || status.as_ref().is_some_and(|status| status.id.starts_with("manual:"))) && ensure_manual(cli, game)? {
            return execute(manager, cli, game, action, true);
        }
        if status.is_none() { return Err("SCSKiller did not recognize this executable. Only native x64 DirectX games are supported.".into()); }
    }
    Ok(())
}

#[tauri::command]
pub async fn shader_state(state: State<'_, SharedState>, id: i64) -> Result<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = db::open(&state.db_path)?; let game = db::game(&db, id)?;
        let runtime = state.shader.lock()?;
        let job = runtime.job.clone();
        let busy = runtime.busy(); drop(runtime);
        let tool = configured_tool(&db::settings(&db)?.shader_tool);
        let status = match &tool { Ok(cli) => game_status(cli, &game), Err(_) => Ok(None) };
        let (game, warning) = match status { Ok(game) => (game, None), Err(error) => (None, Some(error)) };
        Ok(Snapshot { installed: tool.is_ok(), recording_supported: tool.is_ok_and(|cli| recording_supported(&cli)), game, job, busy, warning })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn start_shader_job(state: State<'_, SharedState>, id: i64, action: Action) -> Result<()> {
    if action == Action::ClearCache { return Err("Use the cache cleanup action to clear this game's cache.".into()); }
    let state = state.inner().clone();
    let game = db::game(&db::open(&state.db_path)?, id)?;
    if game.console_launch.is_some() { return Err("SCSKiller supports native PC DirectX games, not shadPS4 games.".into()); }
    let cli = configured_tool(&db::settings(&db::open(&state.db_path)?)?.shader_tool)?;
    launch::plan(&game)?;
    {
        let mut runtime = state.shader.lock()?;
        if runtime.busy() { return Err("Another shader operation is running. Finish or stop it first.".into()); }
        if state.launching.lock().map_err(|_| "Running game state is unavailable.")?.contains(&id) { return Err("Close this game before preparing its shaders.".into()); }
        runtime.begin(Job { game_id: id, title: game.title.clone(), action, running: true, phase: "Analyzing".into(), lines: vec![], error: None, stopped: false });
    }
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = execute(&state.shader, &cli, &game, action, false);
        if let Ok(mut runtime) = state.shader.lock() {
            let stopped = runtime.stopping; runtime.stdin = None;
            if let Some(job) = &mut runtime.job {
                job.running = false; job.stopped = stopped;
                job.phase = if stopped { "Stopped" } else if outcome.is_ok() { "Done" } else { "Failed" }.into();
                job.error = if stopped { None } else { outcome.err() };
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn stop_shader_job(state: State<'_, SharedState>, id: i64) -> Result<()> {
    state.shader.stop_for(Some(id))
}
#[tauri::command]
pub async fn pick_shader_tool() -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new().set_title("Choose SCSKiller.exe or cli/scskiller.exe").add_filter("SCSKiller", &["exe"]).pick_file()
            .map(|path| tool_path(&models::path_string(&path)).map(|path| models::path_string(&path))).transpose()
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn open_shader_site() -> Result<()> {
    #[cfg(windows)] { Command::new("explorer.exe").arg("https://github.com/BlueHeisenberg/SCSKiller/releases").spawn().map_err(|e| e.to_string())?; Ok(()) }
    #[cfg(not(windows))] { Err("Shader integration requires Windows.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Game) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("arc-shaders-{}-{}-{serial}", std::process::id(), models::now()));
        fs::create_dir_all(&dir).unwrap(); fs::write(dir.join("Game.exe"), b"fixture").unwrap();
        let mut database = rusqlite::Connection::open_in_memory().unwrap(); db::migrate(&mut database).unwrap();
        let (id, _) = db::upsert(&database, &models::Candidate { title: "Fixture Game".into(), exe_path: models::path_string(&dir.join("Game.exe")), folder: models::path_string(&dir), score: 80, console_launch: None }).unwrap().unwrap();
        (dir, db::game(&database, id).unwrap())
    }
    fn entry(id: &str, exe: &str, status: &str) -> Value {
        json!({"Game": {"Id": id, "ExePath": exe, "Name": "Same title"}, "Status": status,
            "StatusReason": "Verified provider reason", "Engine": {"Family": "Unreal", "GraphicsApi": "D3D12"}, "AntiCheat": "None", "WarmedAt": null})
    }
    #[test] fn identity_uses_executable_not_title_or_steam_rating_id() {
        let list = json!({"Games": [entry("steam:1", "D:\\Old\\Game.exe", "Ready"), entry("manual:abcd", "G:\\New\\Game.exe", "NeedsRecording")]});
        let status = status_from_list(&list, "g:/new/game.exe").unwrap().unwrap();
        assert_eq!(status.id, "manual:abcd"); assert!(!status.can_compile);
        assert!(status_from_list(&list, "C:\\Another\\Game.exe").unwrap().is_none());
    }
    #[test] fn unicode_case_expansion_does_not_merge_different_installations() {
        assert_ne!(path_key("D:\\Straße\\Game.exe"), path_key("D:\\Strasse\\Game.exe"));
        assert_eq!(path_key("D:\\Straße\\Game.exe"), path_key("d:/straße/game.exe"));
    }
    #[test] fn ambiguity_and_unsafe_queue_id_are_rejected() {
        let list = json!({"Games": [entry("steam:1", "D:\\Game.exe", "Ready"), entry("manual:abcd", "D:\\Game.exe", "Ready")]});
        assert!(status_from_list(&list, "D:\\Game.exe").is_err());
        for id in ["--all-ready", "manual:a\nstart", "steam:1 stop", "", "manual:"] { assert!(!valid_id(id)); }
    }
    #[test] fn unknown_and_recording_states_do_not_enable_compile() {
        for (value, expected) in [("Ready", true), ("Stale", true), ("Warmed", true), ("NeedsRecording", false), ("Unsupported", false), ("NewStatus", false)] {
            let list = json!({"Games": [entry("steam:1", "D:\\Game.exe", value)]});
            assert_eq!(status_from_list(&list, "D:\\Game.exe").unwrap().unwrap().can_compile, expected);
        }
        assert!(status_from_list(&json!([]), "D:\\Game.exe").is_err());
    }
    #[test] fn new_verdicts_and_partial_compiles_do_not_claim_full_preparation() {
        let mut candidate = entry("steam:1", "D:\\Game.exe", "Warmed");
        candidate["Plan"] = json!({"Recorded": 100, "Generated": 50, "D3D11Shaders": 25, "MiddlewareItems": 25, "Variants": 100});
        candidate["LastWarmFailed"] = json!(30); candidate["LastWarmCrashed"] = json!(0);
        assert_eq!(display_status(&candidate, "Warmed"), "Warmed"); // exactly 10%, not partly
        candidate["LastWarmCrashed"] = json!(1);
        assert_eq!(display_status(&candidate, "Warmed"), "PartlyCompiled");
        candidate["LastWarmFailed"] = Value::Null; candidate["LastWarmSkipped"] = json!(200);
        assert_eq!(display_status(&candidate, "Warmed"), "Warmed"); // missing/recreated shaders aren't driver rejections
        candidate["Careful"] = json!({"LaunchCompiled": 0.21});
        assert_eq!(display_status(&candidate, "Warmed"), "PartlyWarmed");
        candidate["CompileUnreached"] = json!(true);
        assert_eq!(display_status(&candidate, "Warmed"), "CompileUnreached");
        candidate["NoStutter"] = json!("The game precompiles its shaders.");
        let result = status_from_list(&json!({"Games": [candidate.clone()]}), "D:\\Game.exe").unwrap().unwrap();
        assert_eq!(result.status, "NoShaderStutter"); assert!(!result.can_compile); assert!(!result.can_record);
        assert_eq!(result.reason, "The game precompiles its shaders.");
        candidate["NoStutter"] = Value::Null; candidate["CompileUnreached"] = json!(false); candidate["NotPlanned"] = json!(true);
        assert!(!status_from_list(&json!({"Games": [candidate]}), "D:\\Game.exe").unwrap().unwrap().can_compile);
        let mut offline = entry("steam:2", "D:\\Game.exe", "NeedsRecording");
        offline["AntiCheat"] = json!("EasyAntiCheat");
        offline["StatusReason"] = json!("EasyAntiCheat blocks the recorder, but you can record at your own risk with an offline session.");
        assert_eq!(display_status(&offline, "NeedsRecording"), "NeedsOfflineSession"); assert!(!recording_eligible(&offline));
    }
    #[test] fn portable_store_switches_only_after_upstreams_atomic_migration_marker() {
        let (dir, _) = fixture(); let cli = dir.join("current/cli/scskiller.exe");
        let local = dir.join("shared"); fs::create_dir_all(cli.parent().unwrap()).unwrap();
        fs::create_dir(dir.join("data")).unwrap(); fs::write(dir.join(".portable"), "").unwrap();
        assert_eq!(resolved_data_dir(&cli, local.clone()), local);
        fs::write(dir.join("data/migrated"), "").unwrap();
        assert_eq!(resolved_data_dir(&cli, local.clone()), dir.join("data"));
        fs::remove_file(dir.join(".portable")).unwrap();
        assert_eq!(resolved_data_dir(&cli, local.clone()), local);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn recorder_requires_explicit_engine_compatibility_and_no_anti_cheat() {
        let mut candidate = entry("manual:abc", "D:\\Game.exe", "Unsupported");
        assert!(!recording_eligible(&candidate));
        candidate["Engine"]["Encrypted"] = json!(false); candidate["Engine"]["Unsupported"] = Value::Null;
        assert!(recording_eligible(&candidate));
        for (field, value) in [("Encrypted", json!(true)), ("Unsupported", json!("packed shader format")), ("GraphicsApi", json!("Vulkan"))] {
            let mut blocked = candidate.clone(); blocked["Engine"][field] = value; assert!(!recording_eligible(&blocked));
        }
        candidate["AntiCheat"] = json!("Other"); assert!(!recording_eligible(&candidate));
        candidate["AntiCheat"] = json!("None"); candidate["ShaderModBlocks"] = json!(true); assert!(!recording_eligible(&candidate));
    }
    #[test] fn recorder_setup_cannot_be_interrupted_by_stop() {
        let manager = Manager::default();
        manager.lock().unwrap().begin(Job { game_id: 4, title: "Fixture".into(), action: Action::PrepareRecording, running: true, phase: "PreparingRecording".into(), lines: vec![], error: None, stopped: false });
        assert!(manager.stop_for(Some(4)).is_err()); assert!(manager.stop().is_ok()); assert!(!manager.lock().unwrap().stopping);
    }
    #[test] fn recording_keeps_a_confirmed_narrower_folder_and_rejects_outside_roots() {
        let (dir, mut game) = fixture(); let narrow = dir.join("ActualGame"); fs::create_dir(&narrow).unwrap();
        fs::write(narrow.join("Game.exe"), b"fixture").unwrap(); game.exe_path = models::path_string(&narrow.join("Game.exe"));
        let mut list = json!([{"Exe": game.exe_path, "InstallDir": models::path_string(&narrow), "Confirmed": true}]);
        assert_eq!(confirmed_recording_root(&list, &game).unwrap().unwrap(), fs::canonicalize(&narrow).unwrap());
        list[0]["InstallDir"] = json!(models::path_string(dir.parent().unwrap())); assert!(confirmed_recording_root(&list, &game).is_err());
        list[0]["Confirmed"] = json!(false); assert!(confirmed_recording_root(&list, &game).unwrap().is_none());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn manual_import_preserves_existing_unknown_fields_and_confirmation() {
        let (dir, game) = fixture();
        let mut list = json!([{"Exe": game.exe_path, "InstallDir": game.folder, "Name": "Personal title", "Confirmed": true, "FutureField": 42}]);
        let before = list.clone(); assert!(!merge_manual(&mut list, &game).unwrap()); assert_eq!(list, before);
        let mut list = json!([{"UnknownEntry": true}]); assert!(merge_manual(&mut list, &game).unwrap());
        assert_eq!(list[0], json!({"UnknownEntry": true})); assert_eq!(list[1]["Confirmed"], false); assert_eq!(list[1]["Name"], game.title);
        assert!(merge_manual(&mut json!({}), &game).is_err()); fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn nested_engine_layouts_use_the_game_root_inside_the_library_folder() {
        let (dir, mut game) = fixture();
        let root = dir.join("Delivered/Game");
        let exe = root.join("bin/x64_dx12/Game.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap(); fs::write(&exe, b"fixture").unwrap();
        game.exe_path = models::path_string(&exe);
        assert_eq!(install_root(&game).unwrap(), fs::canonicalize(&root).unwrap());
        let mut list = json!([]); merge_manual(&mut list, &game).unwrap();
        assert_eq!(path_key(list[0]["InstallDir"].as_str().unwrap()), path_key(&models::path_string(&root)));
        let unreal = root.join("Project/Binaries/Win64/Game.exe");
        fs::create_dir_all(unreal.parent().unwrap()).unwrap(); fs::create_dir(root.join("Engine")).unwrap(); fs::write(&unreal, b"fixture").unwrap();
        game.exe_path = models::path_string(&unreal);
        assert_eq!(install_root(&game).unwrap(), fs::canonicalize(&root).unwrap());
        game.folder = models::path_string(&root.join("Project"));
        assert_eq!(install_root(&game).unwrap(), fs::canonicalize(root.join("Project")).unwrap());
        let retail = root.join("Retail/Game.exe"); fs::create_dir_all(retail.parent().unwrap()).unwrap(); fs::create_dir(root.join("Runtime")).unwrap(); fs::write(&retail, b"fixture").unwrap();
        game.exe_path = models::path_string(&retail); game.folder = models::path_string(&dir);
        assert_eq!(install_root(&game).unwrap(), fs::canonicalize(&root).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn repairing_an_unconfirmed_root_preserves_the_users_metadata() {
        let (dir, mut game) = fixture(); let root = dir.join("Game"); let exe = root.join("bin/x64/Game.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap(); fs::write(&exe, b"fixture").unwrap(); game.exe_path = models::path_string(&exe);
        let mut list = json!([{"Exe": game.exe_path, "InstallDir": game.folder, "Name": "Personal title", "Confirmed": false, "FutureField": 42}]);
        assert!(merge_manual(&mut list, &game).unwrap());
        assert_eq!(list[0]["Name"], "Personal title"); assert_eq!(list[0]["Confirmed"], false); assert_eq!(list[0]["FutureField"], 42);
        assert_eq!(path_key(list[0]["InstallDir"].as_str().unwrap()), path_key(&models::path_string(&root)));
        assert!(!merge_manual(&mut list, &game).unwrap());
        list[0]["InstallDir"] = json!(models::path_string(&root.join("bin"))); let narrower = list.clone();
        assert!(!merge_manual(&mut list, &game).unwrap()); assert_eq!(list, narrower);
        list[0]["InstallDir"] = json!(game.folder); list[0]["Confirmed"] = json!(true); let before = list.clone();
        assert!(!merge_manual(&mut list, &game).unwrap()); assert_eq!(list, before);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn bounded_json_accepts_bom_and_refuses_corruption() {
        let (dir, _) = fixture(); let path = dir.join("cache.json");
        assert!(read_json(&path).unwrap().is_none());
        fs::write(&path, b"\xef\xbb\xbf{\"Games\":[]}").unwrap(); assert!(read_json(&path).unwrap().is_some());
        fs::write(&path, b"bad data").unwrap(); assert!(read_json(&path).is_err());
        let file = fs::File::create(&path).unwrap(); file.set_len(MAX_JSON + 1).unwrap(); assert!(read_json(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn progress_is_scoped_to_the_requested_game_and_stage() {
        assert_eq!(queue_stage("12:32:01 steam:1 Warming 512 / 1024", "steam:1"), Some("Warming"));
        assert_eq!(queue_stage("12:32:01 steam:2 Done", "steam:1"), None);
        assert_eq!(queue_stage("error: steam:1 Done", "steam:1"), None);
        assert_eq!(queue_stage("12:32:01 steam:1 NewStage", "steam:1"), None);
    }
    #[test] fn stop_is_idempotent_and_logs_are_bounded() {
        let manager = Manager::default();
        manager.lock().unwrap().job = Some(Job { game_id: 1, title: "Game".into(), action: Action::Compile, running: true, phase: "Warming".into(), lines: vec![], error: None, stopped: false });
        for i in 0..120 { manager.line(&format!("{i}: {}", "x".repeat(1600))); }
        manager.stop().unwrap(); manager.stop().unwrap();
        let runtime = manager.lock().unwrap(); let job = runtime.job.as_ref().unwrap();
        assert_eq!(job.lines.len(), 100); assert!(job.lines[0].starts_with("20:")); assert!(job.lines.iter().all(|line| line.len() <= 1500));
        assert!(runtime.stopping); assert_eq!(job.phase, "Stopping");
    }
    #[test] fn old_preferences_get_an_empty_optional_tool() {
        let settings: models::Settings = serde_json::from_value(json!({"folders": [], "displayName": "Player", "autoWatch": true})).unwrap();
        assert_eq!(settings.shader_tool, "");
    }
    #[test] fn tool_selection_requires_the_cli_and_its_core() {
        let (dir, _) = fixture();
        assert!(tool_path(&models::path_string(&dir.join("Game.exe"))).is_err());
        fs::create_dir(dir.join("cli")).unwrap(); fs::write(dir.join("cli/scskiller.exe"), b"fixture").unwrap();
        assert!(tool_path(&models::path_string(&dir)).is_err());
        fs::write(dir.join("cli/SCSKiller.Core.dll"), b"fixture").unwrap();
        assert!(tool_path(&models::path_string(&dir)).is_ok()); fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "Opt-in: requires an official SCSKiller installation and an existing native game; never launches the game"]
    fn official_cli_smoke() {
        let cli = tool_path(&std::env::var("ARC_SHADER_SMOKE_TOOL").expect("Set ARC_SHADER_SMOKE_TOOL")).unwrap();
        let database = db::open(Path::new(&std::env::var("ARC_SHADER_SMOKE_DB").expect("Set ARC_SHADER_SMOKE_DB"))).unwrap();
        let id = std::env::var("ARC_SHADER_SMOKE_GAME_ID").expect("Set ARC_SHADER_SMOKE_GAME_ID").parse().unwrap();
        let game = db::game(&database, id).unwrap();
        assert!(game.console_launch.is_none());
        let manager = Manager::default();
        manager.lock().unwrap().job = Some(Job { game_id: id, title: game.title.clone(), action: Action::Analyze, running: true, phase: "Analyzing".into(), lines: vec![], error: None, stopped: false });
        execute(&manager, &cli, &game, Action::Analyze, false).unwrap();
        let status = game_status(&cli, &game).unwrap().expect("The executable should be mapped by its actual path");
        println!("Official CLI analysis: {} · {}", game.title, status.status);
        if std::env::var("ARC_SHADER_SMOKE_RECORDING").is_ok_and(|value| value == "1") {
            assert!(status.can_record, "Recording must be compatible with the exact installation");
            manager.lock().unwrap().begin(Job { game_id: id, title: game.title.clone(), action: Action::PrepareRecording, running: true, phase: "Analyzing".into(), lines: vec![], error: None, stopped: false });
            execute(&manager, &cli, &game, Action::PrepareRecording, false).unwrap();
            assert!(game_status(&cli, &game).unwrap().unwrap().recorder_installed);
            println!("Arc recording setup verified: {}", game.title);
        }
        if std::env::var("ARC_SHADER_SMOKE_COMPILE").is_ok_and(|value| value == "1") {
            assert!(status.can_compile, "{}", status.reason);
            let stop = std::env::var("ARC_SHADER_SMOKE_STOP").is_ok_and(|value| value == "1");
            let finished = std::sync::atomic::AtomicBool::new(false);
            let outcome = std::thread::scope(|scope| {
                scope.spawn(|| {
                    let mut previous = String::new();
                    while !finished.load(std::sync::atomic::Ordering::Relaxed) {
                        let phase = manager.lock().unwrap().job.as_ref().unwrap().phase.clone();
                        if phase != previous { println!("Queue stage: {phase}"); previous = phase.clone(); }
                        if stop && matches!(phase.as_str(), "Indexing" | "Warming") { manager.stop().unwrap(); }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                });
                let result = execute(&manager, &cli, &game, Action::Compile, false);
                finished.store(true, std::sync::atomic::Ordering::Relaxed); result
            });
            outcome.unwrap();
            if stop { assert!(manager.lock().unwrap().stopping); println!("Official queue stopped gracefully"); return; }
            let status = game_status(&cli, &game).unwrap().unwrap();
            assert!(status.warmed_at.is_some());
            println!("Official CLI compile: {} · {}", game.title, status.status);
        }
    }
}
