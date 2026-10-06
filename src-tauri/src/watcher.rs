use std::{sync::mpsc, time::Duration};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use crate::{SharedState, db, scan, models::path_string};

pub fn restart(app: &tauri::AppHandle, state: SharedState) -> Result<(), String> {
    let settings = db::settings(&db::open(&state.db_path)?)?;
    let mut slot = state.watcher.lock().map_err(|_| "Could not access folder watcher".to_string())?;
    // Dropping a watcher disconnects the old receiver, allowing the worker to exit.
    *slot = None;
    if !settings.auto_watch || settings.folders.is_empty() { return Ok(()); }
    let (sender, receiver) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(move |event| { let _ = sender.send(event); }).map_err(|e| e.to_string())?;
    for folder in &settings.folders {
        if std::path::Path::new(folder).is_dir() {
            watcher.watch(std::path::Path::new(folder), RecursiveMode::Recursive).map_err(|e| format!("Could not watch {folder}: {e}"))?;
        }
    }
    *slot = Some(watcher);
    drop(slot);
    let app = app.clone(); let weak = std::sync::Arc::downgrade(&state);
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if !relevant(&event) { continue; }
            if let Some(state) = weak.upgrade() { if let Err(error) = reconcile_rename(&state.db_path, &event) { eprintln!("Folder rename: {error}"); } }
            // Coalesce installer bursts, with a 10-second ceiling to avoid starvation.
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_secs(10) {
                match receiver.recv_timeout(Duration::from_millis(1500)) {
                    Ok(event) => {
                        if let Some(state) = weak.upgrade() { if let Err(error) = reconcile_rename(&state.db_path, &event) { eprintln!("Folder rename: {error}"); } }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            let Some(state) = weak.upgrade() else { return; };
            let app = app.clone();
            tauri::async_runtime::spawn(async move { if let Err(error) = scan(app, state, false).await { eprintln!("Folder update: {error}"); } });
        }
    });
    Ok(())
}
fn relevant(event: &notify::Result<Event>) -> bool {
    match event {
        Ok(event) => matches!(event.kind, EventKind::Create(notify::event::CreateKind::Folder) | EventKind::Remove(notify::event::RemoveKind::Folder) | EventKind::Modify(notify::event::ModifyKind::Name(_)))
            || (matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Data(_))) && event.paths.iter().any(|path| path.extension().is_none() || path.extension().is_some_and(|extension| ["exe", "sfo"].iter().any(|kind| extension.eq_ignore_ascii_case(kind))) || path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("eboot.bin")))),
        Err(_) => true,
    }
}

fn renamed_path(path: &str, old: &str, new: &str) -> Option<String> {
    if path.eq_ignore_ascii_case(old) { return Some(new.to_owned()); }
    let prefix = format!("{}{}", old.trim_end_matches(['\\', '/']), std::path::MAIN_SEPARATOR);
    if path.get(..prefix.len()).is_some_and(|value| value.eq_ignore_ascii_case(&prefix)) { Some(format!("{}{}{}", new.trim_end_matches(['\\', '/']), std::path::MAIN_SEPARATOR, &path[prefix.len()..])) }
    else { None }
}

fn reconcile_rename(path: &std::path::Path, event: &notify::Result<Event>) -> Result<(), String> {
    let Ok(event) = event else { return Ok(()); };
    if !matches!(event.kind, EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::Both))) || event.paths.len() != 2 { return Ok(()); }
    let old = path_string(&event.paths[0]); let new = path_string(&event.paths[1]);
    let mut db = db::open(path)?; let tx = db.transaction().map_err(|e| e.to_string())?;
    for game in db::games(&tx)? {
        if let Some(exe_path) = renamed_path(&game.exe_path, &old, &new) {
            let folder = renamed_path(&game.folder, &old, &new).unwrap_or(game.folder);
            tx.execute("UPDATE games SET exe_path=?2, folder=?3 WHERE id=?1", rusqlite::params![game.id, exe_path, folder]).map_err(|e| e.to_string())?;
        }
    }
    let paths = {
        let mut statement = tx.prepare("SELECT exe_path FROM ignored_executables").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    for old_path in paths {
        if let Some(new_path) = renamed_path(&old_path, &old, &new) {
            tx.execute("UPDATE ignored_executables SET exe_path=?2 WHERE exe_path=?1", rusqlite::params![old_path, new_path]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rename_does_not_match_a_neighboring_folder() {
        let sep = std::path::MAIN_SEPARATOR;
        let old = format!("{sep}games{sep}Game"); let new = format!("{sep}games{sep}Renamed");
        assert_eq!(renamed_path(&format!("{old}{sep}bin{sep}game.exe"), &old, &new), Some(format!("{new}{sep}bin{sep}game.exe")));
        assert!(renamed_path(&format!("{old}2{sep}game.exe"), &old, &new).is_none());
    }
    #[test] fn ps4_game_changes_trigger_a_scan_without_watching_all_binary_assets() {
        for name in ["eboot.bin", "param.sfo"] {
            let event = Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path(name.into());
            assert!(relevant(&Ok(event)));
        }
        let unrelated = Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path("texture.bin".into());
        assert!(!relevant(&Ok(unrelated)));
    }
}
