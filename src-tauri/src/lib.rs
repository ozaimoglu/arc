mod artwork;
mod db;
mod metadata;
mod models;
mod ps4;
mod launch;
mod scanner;
mod secrets;
mod watcher;
mod ratings;
mod shaders;

use std::{collections::HashSet, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use models::{Artwork, GameMatch, GamePatch, ScanReport, Settings, Snapshot};
use tauri::{Emitter, Manager, State};

pub struct AppState {
    data_dir: PathBuf,
    db_path: PathBuf,
    scan_lock: tokio::sync::Mutex<()>,
    launching: Mutex<HashSet<i64>>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    shader: Arc<shaders::Manager>,
}
type SharedState = Arc<AppState>;
type Result<T> = std::result::Result<T, String>;
pub(crate) fn changed(app: &tauri::AppHandle) { let _ = app.emit("library-changed", ()); }

#[tauri::command]
async fn ratings_request(url: String) -> Result<ratings::Response> { ratings::request(&url).await }
#[tauri::command]
async fn get_rating_app_id(state: State<'_, SharedState>, id: i64) -> Result<Option<i64>> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || Ok(ratings::local_app_id(&db::game(&db::open(&path)?, id)?))).await.map_err(|error| error.to_string())?
}
#[tauri::command]
async fn rating_catalogue_title(state: State<'_, SharedState>, id: i64) -> Result<String> {
    let game = db::game(&db::open(&state.db_path)?, id)?;
    let catalogue_id = game.sgdb_id.ok_or("This game has no linked catalogue entry.")?;
    Ok(client(&state)?.game(catalogue_id).await?.name)
}
#[tauri::command]
async fn ratings_cache_read(state: State<'_, SharedState>, key: String) -> Result<std::collections::BTreeMap<String, serde_json::Value>> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || ratings::cache_read(&path, &key)).await.map_err(|error| error.to_string())?
}
#[tauri::command]
async fn ratings_cache_write(state: State<'_, SharedState>, values: std::collections::BTreeMap<String, serde_json::Value>) -> Result<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || ratings::cache_write(&path, values)).await.map_err(|error| error.to_string())?
}
#[tauri::command]
async fn save_game_ratings(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64, title: String, platform: String, value: models::GameRatings) -> Result<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || ratings::save(&db::open(&path)?, id, &title, &platform, &value)).await.map_err(|error| error.to_string())??;
    changed(&app); Ok(())
}
#[tauri::command]
async fn open_rating_source(url: String) -> Result<()> {
    let validated = ratings::link(&url)?;
    #[cfg(windows)] { std::process::Command::new("explorer.exe").arg(validated.as_str()).spawn().map_err(|error| error.to_string())?; Ok(()) }
    #[cfg(not(windows))] { let _ = validated; Err("Opening rating sources is supported on Windows only.".into()) }
}

#[tauri::command]
async fn get_library(state: State<'_, SharedState>) -> Result<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let db = db::open(&state.db_path)?;
        let games = db::games(&db)?;
        let settings = db::settings(&db)?;
        let running_game_ids = state.launching.lock().map_err(|_| "Could not access running games.".to_string())?.iter().copied().collect();
        Ok(Snapshot { games, settings, running_game_ids })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(|| Ok(rfd::FileDialog::new().set_title("Choose a game library folder").pick_folder().map(|path| path.to_string_lossy().into_owned()))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn save_settings(app: tauri::AppHandle, state: State<'_, SharedState>, mut settings: Settings) -> Result<()> {
    if settings.display_name.chars().count() > 40 { return Err("Your name must be 40 characters or fewer.".into()); }
    if settings.api_key.len() > 256 || settings.api_key.contains(['\r', '\n']) { return Err("The API key is not valid.".into()); }
    if settings.folders.len() > 32 { return Err("Add up to 32 game library folders.".into()); }
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut folders = vec![];
        for folder in settings.folders {
            let folder = std::fs::canonicalize(&folder).map_err(|_| format!("Folder not found or inaccessible: {folder}"))?;
            if !folder.is_dir() { return Err("Choose a folder, rather than a file.".into()); }
            let value = models::path_string(&folder);
            if !folders.iter().any(|existing: &String| existing.eq_ignore_ascii_case(&value)) { folders.push(value); }
        }
        settings.folders = folders;
        if !settings.shader_tool.is_empty() { settings.shader_tool = models::path_string(&shaders::tool_path(&settings.shader_tool)?); }
        let mut db = db::open(&path)?; db::save_settings(&mut db, &settings)
    }).await.map_err(|e| e.to_string())??;
    changed(&app);
    watcher::restart(&app, state.inner().clone()).map_err(|error| format!("Settings were saved, but folder watching could not start: {error}"))?;
    Ok(())
}

#[tauri::command]
async fn update_game(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64, patch: GamePatch) -> Result<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || db::patch(&db::open(&path)?, id, patch)).await.map_err(|e| e.to_string())??;
    changed(&app); Ok(())
}

#[tauri::command]
async fn remove_game(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64) -> Result<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || db::remove(&mut db::open(&path)?, id)).await.map_err(|e| e.to_string())??;
    changed(&app); Ok(())
}

pub(crate) async fn scan(app: tauri::AppHandle, state: SharedState, with_artwork: bool) -> Result<ScanReport> {
    let _guard = state.scan_lock.try_lock().map_err(|_| "A library scan is already running.".to_string())?;
    let path = state.db_path.clone();
    let (mut report, metadata_games, key) = tauri::async_runtime::spawn_blocking(move || {
        let mut db = db::open(&path)?; let settings = db::settings(&db)?;
        if settings.folders.is_empty() { return Err("Add at least one game folder in Settings.".into()); }
        let (candidates, ignored, warnings) = scanner::scan(&settings.folders);
        let mut report = ScanReport { ignored, warnings, ..Default::default() }; let mut metadata_games = vec![];
        let tx = db.transaction().map_err(|e| e.to_string())?;
        for candidate in candidates {
            if let Some((_id, added)) = db::upsert(&tx, &candidate)? {
                if added { report.added += 1; } else { report.updated += 1; }
            } else { report.ignored += 1; }
        }
        db::hide_support_programs(&tx)?;
        for game in db::games(&tx)? {
            let exists = launch::plan(&game).is_ok();
            tx.execute("UPDATE games SET available=?2 WHERE id=?1", rusqlite::params![game.id, exists]).map_err(|e| e.to_string())?;
            if exists && !game.hidden && (game.cover.as_ref().is_none_or(|cover| !Path::new(cover).is_file()) || game.hero.as_ref().is_none_or(|hero| !Path::new(hero).is_file())) { metadata_games.push(game); }
        }
        db::mark_detection_upgrade(&tx)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok::<_, String>((report, metadata_games, settings.api_key))
    }).await.map_err(|e| e.to_string())??;
    drop(_guard);
    // Publish local games before any network operation so the UI need not wait for artwork.
    changed(&app);
    if with_artwork && !key.is_empty() {
        let client = artwork::ArtClient::new(key)?;
        for game in metadata_games {
            let outcome: Result<()> = async {
                let match_id = if let Some(id) = game.sgdb_id { Some(id) } else { client.resolve(&game.title, &game.folder).await?.map(|found| found.id) };
                let Some(match_id) = match_id else { return Ok(()); };
                db::link(&db::open(&state.db_path)?, game.id, match_id)?;
                for kind in ["grid", "hero"] {
                    let current = if kind == "grid" { &game.cover } else { &game.hero };
                    if current.as_ref().is_some_and(|path| Path::new(path).is_file()) { continue; }
                    if let Some(art) = client.artworks(match_id, kind).await?.first() {
                        let path = client.download(&art.url, &state.data_dir, game.id, kind).await?;
                        db::set_artwork(&db::open(&state.db_path)?, game.id, kind, &path)?;
                    }
                }
                changed(&app); Ok(())
            }.await;
            if let Err(error) = outcome {
                let stop = error.contains("API key") || error.contains("Could not reach SteamGridDB") || error.contains("HTTP 429") || error.contains("SteamGridDB is busy");
                report.warnings.push(format!("{}: {error}", game.title));
                if stop { break; }
            }
        }
    }
    Ok(report)
}
#[tauri::command]
async fn scan_library(app: tauri::AppHandle, state: State<'_, SharedState>) -> Result<ScanReport> { scan(app, state.inner().clone(), true).await }

fn client(state: &AppState) -> Result<artwork::ArtClient> { artwork::ArtClient::new(db::settings(&db::open(&state.db_path)?)?.api_key) }
#[tauri::command]
async fn search_metadata(state: State<'_, SharedState>, query: String) -> Result<Vec<GameMatch>> {
    if query.trim().is_empty() || query.len() > 800 { return Err("Enter a game name to search.".into()); }
    client(&state)?.search(&query).await
}
#[tauri::command]
async fn link_metadata(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64, sgdb_id: i64) -> Result<()> {
    if sgdb_id <= 0 { return Err("Choose a valid SteamGridDB game.".into()); }
    db::link(&db::open(&state.db_path)?, id, sgdb_id)?; changed(&app); Ok(())
}
#[tauri::command]
async fn get_artworks(state: State<'_, SharedState>, id: i64, kind: String) -> Result<Vec<Artwork>> {
    let game = db::game(&db::open(&state.db_path)?, id)?;
    client(&state)?.artworks(game.sgdb_id.ok_or("Find and select a SteamGridDB game first.")?, &kind).await
}
#[tauri::command]
async fn set_artwork(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64, kind: String, artwork_id: i64) -> Result<()> {
    let game = db::game(&db::open(&state.db_path)?, id)?; let client = client(&state)?;
    let items = client.artworks(game.sgdb_id.ok_or("Find a SteamGridDB game first.")?, &kind).await?;
    let selected = items.iter().find(|art| art.id == artwork_id).ok_or("That artwork is no longer available. Refresh and try again.")?;
    let path = client.download(&selected.url, &state.data_dir, id, &kind).await?;
    db::set_artwork(&db::open(&state.db_path)?, id, &kind, &path)?; changed(&app); Ok(())
}
#[tauri::command]
async fn import_artwork(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64, kind: String) -> Result<bool> {
    db::game(&db::open(&state.db_path)?, id)?;
    let directory = state.data_dir.clone(); let db_path = state.db_path.clone();
    let changed_art = tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new().set_title("Choose game artwork").add_filter("Artwork", &["png", "jpg", "jpeg", "webp"]).pick_file() else { return Ok(false); };
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 20 * 1024 * 1024 { return Err("Artwork must be smaller than 20 MB.".into()); }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let cached = artwork::cache_bytes(&directory, id, &kind, &bytes)?;
        db::set_artwork(&db::open(&db_path)?, id, &kind, &cached)?;
        Ok::<_, String>(true)
    }).await.map_err(|e| e.to_string())??;
    if changed_art { changed(&app); } Ok(changed_art)
}

#[tauri::command]
async fn open_artwork_site() -> Result<()> {
    #[cfg(windows)] {
        std::process::Command::new("explorer.exe").arg("https://www.steamgriddb.com/profile/preferences/api").spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))] { Err("This action is supported on Windows only.".into()) }
}

#[tauri::command]
async fn open_game_folder(state: State<'_, SharedState>, id: i64) -> Result<()> {
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let game = db::game(&db::open(&path)?, id)?;
        let folder = std::fs::canonicalize(game.folder).map_err(|_| "The game folder is no longer available.".to_string())?;
        #[cfg(windows)] { std::process::Command::new("explorer.exe").arg(models::path_string(&folder)).spawn().map_err(|e| e.to_string())?; }
        #[cfg(not(windows))] { let _ = folder; return Err("Opening game folders is currently supported on Windows only.".into()); }
        #[allow(unreachable_code)] Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn launch_game(app: tauri::AppHandle, state: State<'_, SharedState>, id: i64) -> Result<()> {
    let state = state.inner().clone();
    {
        let shader = state.shader.lock()?;
        if shader.job.as_ref().is_some_and(|job| job.running && job.game_id == id) { return Err("Finish or stop shader preparation before launching this game.".into()); }
        let mut running = state.launching.lock().map_err(|_| "Could not access running games.".to_string())?;
        if !running.insert(id) { return Err("This game is already running.".into()); }
    }
    changed(&app);
    let shared = state.clone();
    let launched = tauri::async_runtime::spawn_blocking(move || {
        let mut db = db::open(&shared.db_path)?; let game = db::game(&db, id)?;
        let plan = launch::plan(&game)?;
        // Command::spawn uses CreateProcessW on Windows with proper argument quoting. No shell string is evaluated.
        let mut child = std::process::Command::new(&plan.program).args(&plan.args).current_dir(&plan.directory).spawn().map_err(|error| format!("Windows could not start this game: {error}."))?;
        let start = models::now();
        let session: Result<i64> = (|| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO sessions(game_id,started_at) VALUES (?1,?2)", rusqlite::params![id, start]).map_err(|e| e.to_string())?;
            let session_id = tx.last_insert_rowid();
            tx.execute("UPDATE games SET last_played=?2 WHERE id=?1", rusqlite::params![id, start]).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(session_id)
        })();
        if session.is_err() { let _ = child.kill(); let _ = child.wait(); }
        Ok::<_, String>((child, session?, start))
    }).await.map_err(|e| e.to_string()).and_then(|result| result);
    match launched {
        Err(error) => { if let Ok(mut running) = state.launching.lock() { running.remove(&id); } changed(&app); Err(error) }
        Ok((mut child, session_id, start)) => {
            changed(&app);
            tauri::async_runtime::spawn_blocking(move || {
                let ended = child.wait();
                let end = models::now();
                if let Ok(mut db) = db::open(&state.db_path) {
                    if let Ok(tx) = db.transaction() {
                        let _ = tx.execute("UPDATE sessions SET ended_at=?2 WHERE id=?1", rusqlite::params![session_id, end]);
                        if ended.is_ok() { let _ = tx.execute("UPDATE games SET playtime=playtime+?2 WHERE id=?1", rusqlite::params![id, (end - start).max(0) / 60_000]); }
                        let _ = tx.commit();
                    }
                }
                if let Ok(mut running) = state.launching.lock() { running.remove(&id); }
                changed(&app);
            });
            Ok(())
        }
    }
}

pub fn run() {
    tauri::Builder::default().setup(|app| {
        let data_dir = app.path().app_local_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("db.sqlite");
        db::initialize(&db_path).map_err(std::io::Error::other)?;
        let state = Arc::new(AppState { data_dir, db_path, scan_lock: tokio::sync::Mutex::new(()), launching: Mutex::new(HashSet::new()), watcher: Mutex::new(None), shader: Arc::new(shaders::Manager::default()) });
        app.manage(state.clone());
        // Re-evaluate configured folders once after a detection upgrade, away from the UI thread.
        let upgrade = db::needs_detection_upgrade(&db::open(&state.db_path).map_err(std::io::Error::other)?).map_err(std::io::Error::other)?;
        let preferences = db::settings(&db::open(&state.db_path).map_err(std::io::Error::other)?).map_err(std::io::Error::other)?;
        if upgrade && !preferences.folders.is_empty() {
            let handle = app.handle().clone(); let shared = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = scan(handle, shared, true).await { eprintln!("Library update: {error}"); }
            });
        }
        // Register watching in a background thread; normal startup loads SQLite only.
        let handle = app.handle().clone();
        std::thread::spawn(move || { if let Err(error) = watcher::restart(&handle, state) { eprintln!("Folder watcher: {error}"); } });
        Ok(())
    }).on_window_event(|window, event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
            if let Some(state) = window.try_state::<SharedState>() { let _ = state.shader.stop(); }
        }
    }).invoke_handler(tauri::generate_handler![get_library, pick_folder, save_settings, update_game, remove_game, scan_library, search_metadata, link_metadata, get_artworks, set_artwork, import_artwork, open_artwork_site, open_game_folder, launch_game, ratings_request, ratings_cache_read, ratings_cache_write, save_game_ratings, open_rating_source, rating_catalogue_title, get_rating_app_id, shaders::shader_state, shaders::start_shader_job, shaders::stop_shader_job, shaders::pick_shader_tool, shaders::open_shader_site])
    .run(tauri::generate_context!()).expect("Arc could not start");
}
