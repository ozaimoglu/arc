use std::{path::Path, time::Duration};
use rusqlite::{params, Connection, OptionalExtension, Row};
use crate::models::{now, Candidate, Game, GamePatch, Settings};
use crate::secrets;

pub type Result<T> = std::result::Result<T, String>;
fn err(error: rusqlite::Error) -> String { error.to_string() }
pub fn open(path: &Path) -> Result<Connection> {
    let db = Connection::open(path).map_err(err)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(err)?;
    db.execute_batch("PRAGMA foreign_keys = ON;").map_err(err)?;
    Ok(db)
}
pub fn initialize(path: &Path) -> Result<()> {
    let mut db = open(path)?;
    migrate(&mut db)?;
    // A closed process cannot reliably be resumed after a launcher crash.
    db.execute("UPDATE sessions SET ended_at = started_at WHERE ended_at IS NULL", []).map_err(err)?;
    Ok(())
}
pub fn migrate(db: &mut Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(err)?;
    if version > 3 { return Err("This library was created by a newer Arc version.".into()); }
    if version == 0 { db.execute_batch(include_str!("../migrations/001_initial.sql")).map_err(err)?; }
    if version < 2 {
        let tx = db.transaction().map_err(err)?;
        tx.execute_batch(include_str!("../migrations/002_console_launch.sql")).map_err(err)?;
        tx.commit().map_err(err)?;
    }
    if version < 3 {
        let tx = db.transaction().map_err(err)?;
        tx.execute_batch(include_str!("../migrations/003_ratings.sql")).map_err(err)?;
        tx.commit().map_err(err)?;
    }
    Ok(())
}
fn game_from_row(row: &Row<'_>) -> rusqlite::Result<Game> {
    let console_launch: Option<String> = row.get("console_launch")?;
    Ok(Game {
        id: row.get("id")?, title: row.get("title")?, exe_path: row.get("exe_path")?, folder: row.get("folder")?,
        source: row.get("source")?, genre: row.get("genre")?, description: row.get("description")?,
        cover: row.get("cover")?, hero: row.get("hero")?, logo: row.get("logo")?, sgdb_id: row.get("sgdb_id")?,
        favorite: row.get("favorite")?, hidden: row.get("hidden")?, available: row.get("available")?,
        last_played: row.get("last_played")?, playtime: row.get("playtime")?, score: row.get("score")?, added_at: row.get("added_at")?,
        console_launch: console_launch.map(|json| serde_json::from_str(&json)).transpose().map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error)))?,
        ratings: row.get::<_, Option<String>>("ratings").ok().flatten().and_then(|json| serde_json::from_str(&json).ok()),
    })
}
pub fn games(db: &Connection) -> Result<Vec<Game>> {
    let mut statement = db.prepare("SELECT games.*, game_ratings.value AS ratings FROM games LEFT JOIN game_ratings ON game_ratings.game_id=games.id AND game_ratings.title=games.title AND game_ratings.platform=CASE WHEN games.console_launch IS NULL THEN 'pc' ELSE 'playstation-4' END ORDER BY games.last_played DESC, games.title COLLATE NOCASE").map_err(err)?;
    let rows = statement.query_map([], game_from_row).map_err(err)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)
}
pub fn game(db: &Connection, id: i64) -> Result<Game> {
    db.query_row("SELECT * FROM games WHERE id = ?1", [id], game_from_row).optional().map_err(err)?.ok_or_else(|| "This game is no longer in your library.".into())
}
pub fn settings(db: &Connection) -> Result<Settings> {
    let value: Option<String> = db.query_row("SELECT value FROM settings WHERE key = 'preferences'", [], |row| row.get(0)).optional().map_err(err)?;
    let mut settings: Settings = value.map(|value| serde_json::from_str(&value)).transpose().map_err(|e| e.to_string())?.unwrap_or_default();
    let secret: Option<Vec<u8>> = db.query_row("SELECT value FROM settings WHERE key = 'steamgriddb-key'", [], |row| row.get(0)).optional().map_err(err)?;
    // A moved/corrupt credential must not prevent the local library from opening.
    settings.api_key = secret.and_then(|bytes| secrets::decrypt(&bytes).ok()).unwrap_or_default();
    Ok(settings)
}
pub fn save_settings(db: &mut Connection, settings: &Settings) -> Result<()> {
    let mut public = settings.clone(); public.api_key.clear();
    let json = serde_json::to_string(&public).map_err(|e| e.to_string())?;
    let encrypted = secrets::encrypt(settings.api_key.as_bytes())?;
    let tx = db.transaction().map_err(err)?;
    tx.execute("INSERT INTO settings(key,value) VALUES ('preferences',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json]).map_err(err)?;
    tx.execute("INSERT INTO settings(key,value) VALUES ('steamgriddb-key',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [encrypted]).map_err(err)?;
    tx.commit().map_err(err)
}
pub fn upsert(db: &Connection, candidate: &Candidate) -> Result<Option<(i64, bool)>> {
    let launch = candidate.console_launch.as_ref().map(serde_json::to_string).transpose().map_err(|error| error.to_string())?;
    let source = if launch.is_some() { crate::ps4::SOURCE } else { "Local game" };
    let ignored: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ignored_executables WHERE exe_path = ?1)", [&candidate.exe_path], |row| row.get(0)).map_err(err)?;
    if ignored { return Ok(None); }
    let existing: Option<i64> = db.query_row("SELECT id FROM games WHERE exe_path = ?1", [&candidate.exe_path], |row| row.get(0)).optional().map_err(err)?;
    // When an older scan chose a support/backup copy, move the existing library entry
    // to the real executable in that same installation, preserving its ID and history.
    let existing = if existing.is_none() {
        let in_folder: Option<(i64, String, String)> = db.query_row("SELECT id,exe_path,title FROM games WHERE folder=?1", [&candidate.folder], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional().map_err(err)?;
        in_folder.filter(|(_, exe, title)| crate::scanner::excluded_path(Path::new(exe)) || (Path::new(exe).parent() != Some(Path::new(&candidate.folder)) && crate::scanner::normalize(title).len() <= 4 && crate::scanner::normalize(&candidate.title).len() > 4)).map(|(id, _, _)| id)
    } else { existing };
    if let Some(id) = existing {
        db.execute("UPDATE games SET available=1, folder=?2, score=?3, sgdb_id=CASE WHEN edited=0 AND title!=?4 AND cover IS NULL THEN NULL ELSE sgdb_id END, title=CASE WHEN edited=0 THEN ?4 ELSE title END, exe_path=?5, console_launch=?6, source=CASE WHEN ?6 IS NOT NULL THEN ?7 ELSE source END WHERE id=?1", params![id, candidate.folder, candidate.score, candidate.title, candidate.exe_path, launch, source]).map_err(err)?;
        Ok(Some((id, false)))
    } else {
        db.execute("INSERT INTO games(title,exe_path,folder,score,added_at,console_launch,source) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![candidate.title, candidate.exe_path, candidate.folder, candidate.score, now(), launch, source]).map_err(err)?;
        Ok(Some((db.last_insert_rowid(), true)))
    }
}
pub fn patch(db: &Connection, id: i64, patch: GamePatch) -> Result<()> {
    let mut game = game(db, id)?;
    let edited = patch.title.is_some();
    if let Some(title) = patch.title { if title.trim().is_empty() || title.len() > 800 { return Err("Game name must be between 1 and 200 characters.".into()); } game.title = title.trim().into(); }
    if let Some(value) = patch.genre { if value.len() > 240 { return Err("Genre is too long.".into()); } game.genre = value; }
    if let Some(value) = patch.description { if value.len() > 8000 { return Err("Description is too long.".into()); } game.description = value; }
    if let Some(value) = patch.favorite { game.favorite = value; }
    if let Some(value) = patch.hidden { game.hidden = value; }
    db.execute("UPDATE games SET title=?2, genre=?3, description=?4, favorite=?5, hidden=?6, edited=MAX(edited,?7) WHERE id=?1", params![id, game.title, game.genre, game.description, game.favorite, game.hidden, edited]).map_err(err)?;
    Ok(())
}
pub fn remove(db: &mut Connection, id: i64) -> Result<()> {
    let game = game(db, id)?;
    let tx = db.transaction().map_err(err)?;
    tx.execute("INSERT OR IGNORE INTO ignored_executables(exe_path) VALUES (?1)", [game.exe_path]).map_err(err)?;
    tx.execute("DELETE FROM games WHERE id=?1", [id]).map_err(err)?;
    tx.commit().map_err(err)
}
pub fn set_artwork(db: &Connection, id: i64, kind: &str, path: &str) -> Result<()> {
    game(db, id)?;
    let column = match kind { "grid" => "cover", "hero" => "hero", "logo" => "logo", _ => return Err("Unknown artwork type.".into()) };
    db.execute(&format!("UPDATE games SET {column}=?2 WHERE id=?1"), params![id, path]).map_err(err)?;
    Ok(())
}
pub fn link(db: &Connection, id: i64, sgdb_id: i64) -> Result<()> {
    game(db, id)?;
    db.execute("UPDATE games SET sgdb_id=?2 WHERE id=?1", params![id, sgdb_id]).map_err(err)?;
    Ok(())
}

pub fn hide_support_programs(db: &Connection) -> Result<()> {
    for game in games(db)? {
        let name = Path::new(&game.exe_path).file_name().unwrap_or_default().to_string_lossy();
        if crate::scanner::blocked(&name) || crate::scanner::excluded_path(Path::new(&game.exe_path)) {
            db.execute("UPDATE games SET hidden=1 WHERE id=?1 AND edited=0", [game.id]).map_err(err)?;
        }
    }
    Ok(())
}

pub fn needs_detection_upgrade(db: &Connection) -> Result<bool> {
    let version: Option<String> = db.query_row("SELECT value FROM settings WHERE key='detection-version'", [], |row| row.get(0)).optional().map_err(err)?;
    Ok(version.as_deref() != Some("4"))
}

pub fn mark_detection_upgrade(db: &Connection) -> Result<()> {
    db.execute("INSERT INTO settings(key,value) VALUES ('detection-version','4') ON CONFLICT(key) DO UPDATE SET value=excluded.value", []).map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Connection, Candidate) {
        let mut db = Connection::open_in_memory().unwrap(); migrate(&mut db).unwrap();
        (db, Candidate { title: "Elden Ring".into(), exe_path: "D:\\Games\\Elden Ring\\Game\\eldenring.exe".into(), folder: "D:\\Games\\Elden Ring".into(), score: 105, console_launch: None })
    }
    #[test]
    fn rescanning_preserves_user_edits_and_hidden_state() {
        let (db, mut candidate) = fixture();
        let (id, fresh) = upsert(&db, &candidate).unwrap().unwrap(); assert!(fresh);
        patch(&db, id, GamePatch { title: Some("My Elden Ring".into()), hidden: Some(true), favorite: Some(true), ..Default::default() }).unwrap();
        candidate.title = "ELDEN RING".into(); upsert(&db, &candidate).unwrap();
        let game = game(&db, id).unwrap(); assert_eq!(game.title, "My Elden Ring"); assert!(game.hidden && game.favorite);
    }
    #[test]
    fn schema_upgrade_is_repeatable_and_preserves_legacy_data() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../migrations/001_initial.sql")).unwrap();
        db.execute("INSERT INTO games(title,exe_path,folder,favorite,hidden,cover,added_at) VALUES ('Saved game','game.exe','games',1,1,'saved.jpg',123)", []).unwrap();
        db.execute("INSERT INTO settings(key,value) VALUES ('credential',?1)", [vec![1_u8, 2, 3]]).unwrap();
        migrate(&mut db).unwrap(); migrate(&mut db).unwrap();
        let game = game(&db, 1).unwrap();
        assert!(game.favorite && game.hidden); assert_eq!(game.cover.as_deref(), Some("saved.jpg"));
        assert_eq!(game.added_at, 123); assert!(game.console_launch.is_none());
        assert_eq!(db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)).unwrap(), 3);
        assert_eq!(db.query_row("SELECT value FROM settings WHERE key='credential'", [], |row| row.get::<_, Vec<u8>>(0)).unwrap(), [1, 2, 3]);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM pragma_table_info('games') WHERE name='console_launch'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
    }
    #[test]
    fn ps4_rescans_preserve_art_edits_history_and_removal_exclusions() {
        let (mut db, mut candidate) = fixture();
        candidate.title = "Bloodborne".into(); candidate.exe_path = "D:\\PS4\\CUSA03173\\eboot.bin".into();
        candidate.console_launch = Some(crate::models::ConsoleLaunch::ShadPs4 { emulator: "D:\\shadPS4.exe".into(), title_id: "CUSA03173".into() });
        let (id, _) = upsert(&db, &candidate).unwrap().unwrap();
        patch(&db, id, GamePatch { favorite: Some(true), title: Some("My Bloodborne".into()), hidden: Some(true), ..Default::default() }).unwrap();
        set_artwork(&db, id, "grid", "bloodborne.jpg").unwrap();
        candidate.console_launch = Some(crate::models::ConsoleLaunch::ShadPs4 { emulator: "E:\\shadPS4.exe".into(), title_id: "CUSA03173".into() });
        assert_eq!(upsert(&db, &candidate).unwrap().unwrap(), (id, false));
        let game = game(&db, id).unwrap(); assert!(game.favorite && game.hidden);
        assert_eq!(game.title, "My Bloodborne"); assert_eq!(game.source, crate::ps4::SOURCE);
        assert_eq!(game.cover.as_deref(), Some("bloodborne.jpg")); assert_eq!(game.console_launch, candidate.console_launch);
        remove(&mut db, id).unwrap(); assert!(upsert(&db, &candidate).unwrap().is_none());
    }
    #[test]
    fn removed_games_are_not_readded_by_scans() {
        let (mut db, candidate) = fixture(); let (id, _) = upsert(&db, &candidate).unwrap().unwrap();
        remove(&mut db, id).unwrap(); assert!(upsert(&db, &candidate).unwrap().is_none()); assert!(games(&db).unwrap().is_empty());
    }
    #[test]
    fn exe_identity_is_case_insensitive() {
        let (db, mut candidate) = fixture(); upsert(&db, &candidate).unwrap(); candidate.exe_path = candidate.exe_path.to_uppercase();
        assert!(!upsert(&db, &candidate).unwrap().unwrap().1); assert_eq!(games(&db).unwrap().len(), 1);
    }
    #[test]
    fn correcting_a_project_name_discards_an_unusable_artwork_match() {
        let (db, mut candidate) = fixture(); candidate.title = "b1".into();
        let (id, _) = upsert(&db, &candidate).unwrap().unwrap(); link(&db, id, 123).unwrap();
        candidate.title = "Black Myth Wukong".into(); upsert(&db, &candidate).unwrap();
        assert_eq!(game(&db, id).unwrap().sgdb_id, None);
    }
    #[test]
    fn moving_from_backup_to_actual_binary_preserves_identity_and_art() {
        let (db, mut candidate) = fixture(); candidate.exe_path = "D:\\Games\\Elden Ring\\Backup\\eldenring.exe".into();
        let (id, _) = upsert(&db, &candidate).unwrap().unwrap();
        patch(&db, id, GamePatch { favorite: Some(true), ..Default::default() }).unwrap();
        link(&db, id, 123).unwrap(); set_artwork(&db, id, "grid", "saved-cover.png").unwrap();
        candidate.exe_path = "D:\\Games\\Elden Ring\\Game\\eldenring.exe".into();
        assert_eq!(upsert(&db, &candidate).unwrap().unwrap(), (id, false));
        let record = game(&db, id).unwrap(); assert!(record.favorite); assert_eq!(record.cover.as_deref(), Some("saved-cover.png"));
        assert_eq!(games(&db).unwrap().len(), 1);
    }
    #[test]
    fn replacement_of_internal_bootstrap_preserves_id_without_merging_loose_games() {
        let (db, mut candidate) = fixture(); candidate.title = "F10".into();
        let (id, _) = upsert(&db, &candidate).unwrap().unwrap();
        candidate.exe_path = "D:\\Games\\Elden Ring\\Game\\Hades2.exe".into(); candidate.title = "Hades II".into();
        assert_eq!(upsert(&db, &candidate).unwrap().unwrap(), (id, false));
        candidate.folder = "D:\\Games".into(); candidate.exe_path = "D:\\Games\\Beta.exe".into(); candidate.title = "Beta".into(); upsert(&db, &candidate).unwrap();
        candidate.exe_path = "D:\\Games\\Gamma.exe".into(); candidate.title = "Gamma".into();
        assert!(upsert(&db, &candidate).unwrap().unwrap().1);
        assert_eq!(games(&db).unwrap().len(), 3);
    }
}
