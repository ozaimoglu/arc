//! CriticPeek's bundled parsers run in the UI; only these bounded Rust commands
//! can access the two rating providers and the dedicated SQLite cache.
use std::{collections::BTreeMap, path::Path, sync::OnceLock, time::Duration};
use reqwest::{Client, Url};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use crate::{db, models::{GameRatings, now}};

type Result<T> = std::result::Result<T, String>;
const MAX_BODY: usize = 8 * 1024 * 1024;

fn source_url(input: &str) -> Result<Url> {
    let url = Url::parse(input).map_err(|_| "Invalid ratings source.".to_string())?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some() || url.fragment().is_some() || input.len() > 1500 {
        return Err("Invalid ratings source.".into());
    }
    Ok(url)
}
fn steam_id(value: &str) -> bool {
    !value.starts_with('0') && !value.is_empty() && value.len() <= 10 && value.bytes().all(|byte| byte.is_ascii_digit())
}
fn parse_local_id(text: &str, plain: bool) -> Option<i64> {
    if plain {
        let value = text.trim().trim_start_matches('\u{feff}');
        return steam_id(value).then(|| value.parse().ok()).flatten();
    }
    let ids: std::collections::BTreeSet<i64> = text.lines().filter_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        if !["appid", "app_id"].contains(&key.trim().to_ascii_lowercase().as_str()) { return None; }
        let value = value.trim();
        steam_id(value).then(|| value.parse().ok()).flatten()
    }).collect();
    (ids.len() == 1).then(|| *ids.first().unwrap())
}
pub fn local_app_id(game: &crate::models::Game) -> Option<i64> {
    if game.console_launch.is_some() { return None; }
    let root = std::fs::canonicalize(&game.folder).ok()?;
    let executable = std::fs::canonicalize(&game.exe_path).ok()?;
    if !executable.starts_with(&root) { return None; }
    let mut ids = std::collections::BTreeSet::new();
    for directory in [Some(root.as_path()), executable.parent()].into_iter().flatten() {
        for name in ["steam_appid.txt", "steam_settings/steam_appid.txt", "steam_settings/configs.app.ini", "steam_emu.ini", "steam_api.ini", "tenoke.ini"] {
            let path = directory.join(name);
            let Ok(path) = std::fs::canonicalize(path) else { continue; };
            if !path.starts_with(&root) { continue; }
            let Ok(metadata) = std::fs::metadata(&path) else { continue; };
            if !metadata.is_file() || metadata.len() > 65_536 { continue; }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Some(id) = parse_local_id(&text, name.ends_with(".txt")) { ids.insert(id); }
            }
        }
    }
    // Conflicting install metadata must be corrected explicitly in the UI.
    (ids.len() == 1).then(|| *ids.first().unwrap())
}
fn metacritic_path(path: &str) -> bool {
    let parts: Vec<_> = path.trim_matches('/').split('/').collect();
    parts.len() >= 2 && parts.len() <= 3 && parts[0] == "game" && !parts[1].is_empty()
        && parts[1].bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && (parts.len() == 2 || ["user-reviews", "critic-reviews"].contains(&parts[2]))
}
fn endpoint(input: &str) -> Result<Url> {
    let url = source_url(input)?;
    let allowed = match url.host_str() {
        Some("store.steampowered.com") => ["/api/storesearch/", "/api/appdetails"].contains(&url.path())
            || url.path().strip_prefix("/appreviews/").is_some_and(steam_id),
        Some("www.metacritic.com") => metacritic_path(url.path()),
        _ => false,
    };
    if !allowed { return Err("Unsupported ratings endpoint.".into()); }
    Ok(url)
}
pub fn link(input: &str) -> Result<Url> {
    let url = source_url(input)?;
    let allowed = match url.host_str() {
        Some("store.steampowered.com") => url.path().strip_prefix("/app/").is_some_and(|path| steam_id(path.trim_end_matches('/'))),
        Some("www.metacritic.com") => metacritic_path(url.path()),
        _ => false,
    };
    if !allowed { return Err("Unsupported ratings link.".into()); }
    Ok(url)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response { body: String, status: u16, url: String, retry_after: Option<String> }
pub async fn request(input: &str) -> Result<Response> {
    let url = endpoint(input)?;
    static CLIENT: OnceLock<Client> = OnceLock::new();
    let client = CLIENT.get_or_init(|| Client::builder().timeout(Duration::from_secs(20))
        .user_agent(concat!("Arc/", env!("CARGO_PKG_VERSION"), " (game ratings)"))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 4 || attempt.url().host_str() != Some("www.metacritic.com") || endpoint(attempt.url().as_str()).is_err() {
                attempt.error("Unsupported ratings redirect")
            } else { attempt.follow() }
        })).build().expect("Valid ratings HTTP client"));
    let mut response = client.get(url).send().await.map_err(|_| "Could not reach the ratings provider.".to_string())?;
    let status = response.status().as_u16();
    let url = endpoint(response.url().as_str())?.to_string();
    let retry_after = response.headers().get("retry-after").and_then(|value| value.to_str().ok()).map(str::to_owned);
    // Never download challenge pages or error bodies. The service honors 429 and
    // does not retry forbidden responses or attempt to bypass provider access.
    let mut bytes = Vec::new();
    if (200..300).contains(&status) {
        if response.content_length().is_some_and(|size| size > MAX_BODY as u64) { return Err("Ratings response is too large.".into()); }
        while let Some(chunk) = response.chunk().await.map_err(|_| "Could not read ratings response.".to_string())? {
            if bytes.len() + chunk.len() > MAX_BODY { return Err("Ratings response is too large.".into()); }
            bytes.extend_from_slice(&chunk);
        }
    }
    Ok(Response { body: String::from_utf8(bytes).map_err(|_| "Invalid ratings response text.".to_string())?, status, url, retry_after })
}
fn cache_key(key: &str) -> Result<()> {
    if !key.starts_with("score:v2:") || key.len() > 700 || key.chars().any(char::is_control) { return Err("Invalid ratings cache key.".into()); }
    Ok(())
}
pub fn cache_read(path: &Path, key: &str) -> Result<BTreeMap<String, Value>> {
    cache_key(key)?;
    let db = db::open(path)?;
    let value: Option<String> = db.query_row("SELECT value FROM ratings_cache WHERE key=?1", [key], |row| row.get(0)).optional().map_err(|error| error.to_string())?;
    let mut result = BTreeMap::new();
    if let Some(value) = value.and_then(|json| serde_json::from_str(&json).ok()) { result.insert(key.to_owned(), value); }
    Ok(result)
}
pub fn cache_write(path: &Path, values: BTreeMap<String, Value>) -> Result<()> {
    if values.len() > 20 { return Err("Too many ratings cache entries.".into()); }
    let mut db = db::open(path)?;
    let tx = db.transaction().map_err(|error| error.to_string())?;
    for (key, value) in values {
        cache_key(&key)?;
        let json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        if json.len() > 32 * 1024 { return Err("Ratings cache entry is too large.".into()); }
        tx.execute("INSERT INTO ratings_cache(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, json]).map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())
}
fn validate(value: &GameRatings, platform: &str) -> Result<()> {
    if value.status != "ok" && (value.steam.is_some() || value.metacritic.is_some()) { return Err("Ratings need a verified game identity.".into()); }
    if !["pc", "playstation-4"].contains(&platform) || !["ok", "not-found", "unverified"].contains(&value.status.as_str())
        || value.title.len() > 1200 || value.checked_at < 0 || value.checked_at > now() + 60_000
        || value.app_id.is_some_and(|id| id <= 0 || id > 9_999_999_999) { return Err("Invalid ratings record.".into()); }
    if platform != "pc" && (value.steam.is_some() || value.app_id.is_some()) { return Err("PC ratings cannot be used for a PS4 game.".into()); }
    if let Some(meta) = &value.metacritic {
        if meta.platform != if platform == "pc" { "PC" } else { "PS4" }
            || meta.score.is_some_and(|score| !score.is_finite() || score <= 0.0 || score > 100.0)
            || meta.user_score.is_some_and(|score| !score.is_finite() || !(0.0..=10.0).contains(&score))
            || !["steam", "metacritic"].contains(&meta.source.as_str()) { return Err("Invalid Metacritic ratings.".into()); }
        for url in [&meta.url, &meta.user_url].into_iter().flatten() {
            if link(url)?.host_str() != Some("www.metacritic.com") { return Err("Invalid Metacritic link.".into()); }
        }
    }
    if let Some(steam) = &value.steam {
        let total = steam.positive.checked_add(steam.negative).ok_or("Invalid review count.")?;
        if steam.positive < 0 || steam.negative < 0 || total <= 0 || steam.total != total || !steam.percent.is_finite()
            || (steam.percent - steam.positive as f64 / total as f64 * 100.0).abs() > 0.000001 || value.app_id.is_none() {
            return Err("Invalid Steam ratings.".into());
        }
    }
    if serde_json::to_vec(value).map_err(|error| error.to_string())?.len() > 32 * 1024 { return Err("Ratings record is too large.".into()); }
    Ok(())
}
pub fn save(db: &Connection, id: i64, title: &str, platform: &str, value: &GameRatings) -> Result<()> {
    validate(value, platform)?;
    let game = db::game(db, id)?;
    let current_platform = if game.console_launch.is_some() { "playstation-4" } else { "pc" };
    if game.title != title || current_platform != platform { return Err("The game changed while ratings were loading. Try again.".into()); }
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    db.execute("INSERT INTO game_ratings(game_id,title,platform,value) VALUES (?1,?2,?3,?4) ON CONFLICT(game_id) DO UPDATE SET title=excluded.title,platform=excluded.platform,value=excluded.value", params![id, title, platform, json]).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_steam_identity_reads_only_exact_numeric_fields() {
        assert_eq!(parse_local_id("\u{feff}2050650\n", true), Some(2050650));
        assert_eq!(parse_local_id("[Game]\nAppId=2531310\nUserName=Private", false), Some(2531310));
        assert_eq!(parse_local_id("appid=292030\nAPP_ID=292030", false), Some(292030));
        assert_eq!(parse_local_id("appid=1\nappid=2", false), None);
        assert_eq!(parse_local_id("appid=1; run command", false), None);
        assert_eq!(parse_local_id("../../123", true), None);
    }
    #[test]
    fn source_urls_reject_unrelated_hosts_credentials_and_paths() {
        for input in ["http://store.steampowered.com/api/appdetails", "https://store.steampowered.com.evil.test/api/appdetails", "https://localhost/api/appdetails", "https://user@www.metacritic.com/game/elden-ring/", "https://www.metacritic.com/news/", "https://store.steampowered.com/appreviews/0", "https://www.metacritic.com/game/../news/"] { assert!(endpoint(input).is_err(), "{input}"); }
        assert!(endpoint("https://store.steampowered.com/appreviews/1245620?json=1").is_ok());
        assert!(endpoint("https://www.metacritic.com/game/bloodborne/user-reviews/?platform=playstation-4").is_ok());
        assert!(link("https://store.steampowered.com/api/appdetails").is_err());
    }
    fn value() -> GameRatings {
        serde_json::from_value(serde_json::json!({"title":"Game", "name":"Game", "appId":123, "status":"ok", "metacritic":{"score":85,"userScore":8.4,"url":"https://www.metacritic.com/game/game/?platform=pc","source":"steam","platform":"PC"},"steam":{"positive":90,"negative":10,"total":100,"percent":90},"errors":[],"checkedAt":123})).unwrap()
    }
    #[test]
    fn scores_validate_scales_platforms_and_review_identity() {
        let mut rating = value(); assert!(validate(&rating, "pc").is_ok());
        rating.status = "unverified".into(); assert!(validate(&rating, "pc").is_err()); rating.status = "ok".into();
        assert!(validate(&rating, "playstation-4").is_err());
        rating.metacritic.as_mut().unwrap().user_score = Some(11.0); assert!(validate(&rating, "pc").is_err());
        rating = value(); rating.steam.as_mut().unwrap().percent = 99.0; assert!(validate(&rating, "pc").is_err());
        rating = value(); rating.app_id = None; assert!(validate(&rating, "pc").is_err());
        rating = value(); rating.metacritic.as_mut().unwrap().url = Some("https://evil.test/".into()); assert!(validate(&rating, "pc").is_err());
    }
    #[test]
    fn ratings_survive_restart_but_do_not_follow_renamed_or_removed_games() {
        let mut db = Connection::open_in_memory().unwrap(); db::migrate(&mut db).unwrap();
        db.execute("INSERT INTO games(title,exe_path,folder,added_at) VALUES ('Game','game.exe','games',1)", []).unwrap();
        save(&db, 1, "Game", "pc", &value()).unwrap();
        assert_eq!(db::games(&db).unwrap()[0].ratings.as_ref().unwrap().steam.as_ref().unwrap().percent, 90.0);
        db::patch(&db, 1, crate::models::GamePatch { title: Some("Another game".into()), ..Default::default() }).unwrap();
        assert!(db::games(&db).unwrap()[0].ratings.is_none());
        assert!(save(&db, 1, "Game", "pc", &value()).is_err());
        db::remove(&mut db, 1).unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM game_ratings", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }
}
