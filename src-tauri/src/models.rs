use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub exe_path: String,
    pub folder: String,
    pub source: String,
    pub genre: String,
    pub description: String,
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub sgdb_id: Option<i64>,
    pub favorite: bool,
    pub hidden: bool,
    pub available: bool,
    pub last_played: Option<i64>,
    pub playtime: i64,
    pub score: i32,
    pub added_at: i64,
    pub console_launch: Option<ConsoleLaunch>,
    pub ratings: Option<GameRatings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRatings {
    pub title: String,
    pub name: Option<String>,
    pub app_id: Option<i64>,
    pub status: String,
    pub metacritic: Option<MetacriticRating>,
    pub steam: Option<SteamRating>,
    pub errors: Vec<String>,
    pub checked_at: i64,
    pub meta_checked_at: Option<i64>,
    pub meta_user_checked_at: Option<i64>,
    pub meta_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetacriticRating {
    pub score: Option<f64>, pub user_score: Option<f64>,
    pub url: Option<String>, pub user_url: Option<String>,
    pub source: String, pub platform: String, pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamRating { pub positive: i64, pub negative: i64, pub total: i64, pub percent: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub folders: Vec<String>,
    pub api_key: String,
    pub display_name: String,
    pub auto_watch: bool,
    pub shader_tool: String,
}
impl Default for Settings {
    fn default() -> Self { Self { folders: vec![], api_key: String::new(), display_name: "Player".into(), auto_watch: true, shader_tool: String::new() } }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot { pub games: Vec<Game>, pub settings: Settings, pub running_game_ids: Vec<i64> }

#[derive(Debug, Default, Serialize)]
pub struct ScanReport { pub added: usize, pub updated: usize, pub ignored: usize, pub warnings: Vec<String> }

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GamePatch {
    pub title: Option<String>, pub genre: Option<String>, pub description: Option<String>,
    pub favorite: Option<bool>, pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMatch {
    pub id: i64,
    pub name: String,
    #[serde(default, alias = "release_date")]
    pub release_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Artwork { pub id: i64, pub url: String, pub thumb: String, pub author: String }

#[derive(Debug, Clone, Serialize)]
pub struct Candidate { pub title: String, pub exe_path: String, pub folder: String, pub score: i32, pub console_launch: Option<ConsoleLaunch> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ConsoleLaunch {
    ShadPs4 { emulator: String, title_id: String },
    BloodborneLauncher { launcher: String, emulator: String, title_id: String },
}

pub fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64 }

pub fn path_string(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") { format!("\\\\{unc}") }
    else { text.strip_prefix("\\\\?\\").unwrap_or(&text).to_owned() }
}
