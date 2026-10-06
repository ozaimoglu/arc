use std::{path::Path, time::Duration};
use reqwest::{Client, Url};
use serde::Deserialize;
use crate::{models::{Artwork, GameMatch}, scanner::{clean_title, normalize}};

const BASE: &str = "https://www.steamgriddb.com/api/v2/";
const MAX_IMAGE: usize = 20 * 1024 * 1024;
#[derive(Deserialize)] struct Envelope<T> { success: bool, data: T }
#[derive(Deserialize)] struct Author { name: String }
#[derive(Deserialize)] struct RemoteArtwork { id: i64, url: String, thumb: String, author: Author }

pub struct ArtClient { client: Client, key: String }
impl ArtClient {
    pub fn new(key: String) -> Result<Self, String> {
        if key.is_empty() { return Err("Add your SteamGridDB API key in Settings to find artwork.".into()); }
        let client = Client::builder().timeout(Duration::from_secs(15)).connect_timeout(Duration::from_secs(5)).redirect(reqwest::redirect::Policy::none()).user_agent(concat!("Arc/", env!("CARGO_PKG_VERSION"))).build().map_err(|e| e.to_string())?;
        Ok(Self { client, key })
    }
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = self.client.get(format!("{BASE}{path}")).bearer_auth(&self.key).send().await.map_err(|_| "Could not reach SteamGridDB. Your library still works offline.".to_string())?;
        match response.status().as_u16() {
            401 | 403 => return Err("SteamGridDB rejected the API key. Check your key in Settings.".into()),
            429 => return Err("SteamGridDB is busy. Please try again in a little while.".into()),
            200 => {},
            code => return Err(format!("SteamGridDB returned HTTP {code}.")),
        }
        let envelope = response.json::<Envelope<T>>().await.map_err(|_| "SteamGridDB returned an unexpected response.".to_string())?;
        if !envelope.success { return Err("SteamGridDB could not complete that request.".into()); }
        Ok(envelope.data)
    }
    pub async fn search(&self, query: &str) -> Result<Vec<GameMatch>, String> {
        let mut url = Url::parse(BASE).map_err(|e| e.to_string())?;
        url.path_segments_mut().map_err(|_| "Invalid API URL".to_string())?.pop_if_empty().extend(["search", "autocomplete", query]);
        let path = url.as_str().strip_prefix(BASE).ok_or("Invalid search URL")?;
        self.get(path).await
    }
    pub async fn game(&self, id: i64) -> Result<GameMatch, String> {
        if id <= 0 { return Err("Choose a valid SteamGridDB game.".into()); }
        self.get(&format!("games/id/{id}")).await
    }
    pub async fn resolve(&self, title: &str, folder: &str) -> Result<Option<GameMatch>, String> {
        let query = search_name(title);
        if query.len() < 4 { return Ok(None); }
        let matches = self.search(&query).await?;
        Ok(select_match(title, folder, matches))
    }
    pub async fn artworks(&self, id: i64, kind: &str) -> Result<Vec<Artwork>, String> {
        let endpoint = endpoint(kind)?;
        let query = if kind == "grid" { "?dimensions=600x900&types=static&nsfw=false&humor=false" } else { "?types=static&nsfw=false&humor=false" };
        let mut result: Vec<RemoteArtwork> = self.get(&format!("{endpoint}/game/{id}{query}")).await?;
        if kind == "grid" && result.is_empty() {
            // Older/smaller libraries sometimes only have other portrait grid sizes.
            result = self.get(&format!("{endpoint}/game/{id}?dimensions=342x482,660x930&types=static&nsfw=false&humor=false")).await?;
        }
        Ok(result.into_iter().take(40).map(|art| Artwork { id: art.id, url: art.url, thumb: art.thumb, author: art.author.name }).collect())
    }
    pub async fn download(&self, url: &str, directory: &Path, id: i64, kind: &str) -> Result<String, String> {
        let url = Url::parse(url).map_err(|_| "Invalid artwork URL".to_string())?;
        if url.scheme() != "https" || !matches!(url.host_str(), Some("cdn2.steamgriddb.com" | "cdn.steamgriddb.com")) { return Err("The artwork URL is not a supported SteamGridDB CDN.".into()); }
        // Never attach the API token to an artwork/CDN request.
        let mut response = self.client.get(url).send().await.map_err(|_| "Could not download artwork. Try again when online.".to_string())?.error_for_status().map_err(|_| "Artwork download failed.".to_string())?;
        if response.content_length().is_some_and(|length| length > MAX_IMAGE as u64) { return Err("Artwork must be smaller than 20 MB.".into()); }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(|_| "Artwork download was interrupted.".to_string())? {
            if bytes.len() + chunk.len() > MAX_IMAGE { return Err("Artwork must be smaller than 20 MB.".into()); }
            bytes.extend_from_slice(&chunk);
        }
        cache_bytes(directory, id, kind, &bytes)
    }
}

fn search_name(title: &str) -> String {
    let value = normalize(&clean_title(title));
    let mut words: Vec<_> = value.split_whitespace().collect();
    for suffix in ["game of the year edition", "goty edition", "complete edition", "deluxe edition", "remake", "lite", "demo", "showcase"] {
        let tail: Vec<_> = suffix.split_whitespace().collect();
        if words.ends_with(&tail) { words.truncate(words.len() - tail.len()); }
    }
    words.join(" ")
}

fn year_start(year: i64) -> i64 {
    let previous = year - 1;
    let leap_days = previous / 4 - 1969 / 4 - (previous / 100 - 1969 / 100) + (previous / 400 - 1969 / 400);
    (365 * (year - 1970) + leap_days) * 86400
}

fn numbers(value: &str) -> Vec<String> {
    value.split_whitespace().filter(|word| word.chars().all(|char| char.is_ascii_digit()) || ["ii", "iii", "iv", "v", "vi", "vii", "viii", "ix"].contains(word)).map(str::to_owned).collect()
}

fn select_match(title: &str, folder: &str, matches: Vec<GameMatch>) -> Option<GameMatch> {
    let query = search_name(title);
    if query.len() < 4 { return None; }
    let years: Vec<_> = folder.split(|char: char| !char.is_ascii_digit()).filter_map(|part| part.parse::<i64>().ok()).filter(|year| (1980..=2100).contains(year)).collect();
    let mut ranked: Vec<_> = matches.into_iter().filter_map(|game| {
        let name = search_name(&game.name);
        // Similar series titles must not silently change a sequel number.
        if numbers(&query) != numbers(&name) { return None; }
        let similarity = strsim::jaro_winkler(&query, &name);
        if similarity < 0.92 { return None; }
        let year_match = game.release_date.is_some_and(|date| years.iter().any(|year| date >= year_start(*year) && date < year_start(*year + 1)));
        let catalogue_name = normalize(&clean_title(&game.name));
        let exact = if normalize(&clean_title(title)) == catalogue_name { 2 } else if query == catalogue_name { 1 } else { 0 };
        Some((similarity, exact, year_match, game))
    }).collect();
    ranked.sort_by(|a,b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
    let (score, exact, year_match, game) = ranked.first()?;
    let unambiguous = ranked.get(1).is_none_or(|next| score - next.0 >= 0.04 || (*exact > next.1) || (*year_match && !next.2));
    unambiguous.then(|| game.clone())
}

fn endpoint(kind: &str) -> Result<&'static str, String> { match kind { "grid" => Ok("grids"), "hero" => Ok("heroes"), "logo" => Ok("logos"), _ => Err("Unknown artwork type.".into()) } }
pub fn cache_bytes(directory: &Path, id: i64, kind: &str, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_IMAGE { return Err("Artwork must be smaller than 20 MB.".into()); }
    let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { "png" }
        else if bytes.starts_with(b"\xff\xd8\xff") { "jpg" }
        else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" { "webp" }
        else { return Err("Choose a PNG, JPEG, or WebP image.".into()); };
    let folder = directory.join(match kind { "grid" => "covers", "hero" => "heroes", "logo" => "logos", _ => return Err("Unknown artwork type.".into()) });
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    // Revision filenames avoid WebView image-cache staleness and permit atomic writes.
    let path = folder.join(format!("game-{id}-{}.{extension}", crate::models::now()));
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game(id: i64, name: &str, year: i64) -> GameMatch { GameMatch { id, name: name.into(), release_date: Some(year_start(year) + 86400) } }
    #[test]
    fn internal_code_and_wrong_sequels_do_not_auto_match() {
        assert!(select_match("b1", "Black Myth Wukong", vec![game(1, "B1", 2024)]).is_none());
        assert!(select_match("Hades II", "Hades II", vec![game(1, "Hades", 2020)]).is_none());
        assert!(select_match("Resident Evil 4", "Resident Evil 4", vec![game(1, "Resident Evil 5", 2009)]).is_none());
    }
    #[test]
    fn identical_remake_names_need_release_year_context() {
        let matches = vec![game(1, "Resident Evil 4", 2005), game(2, "Resident Evil 4", 2023)];
        assert!(select_match("Resident Evil 4", "Resident Evil 4", matches.clone()).is_none());
        assert_eq!(select_match("Resident Evil 4 Remake", "Resident Evil 4 Remake (2023)", matches).unwrap().id, 2);
    }
    #[test]
    fn distribution_suffix_prefers_the_base_catalogue_title() {
        let matches = vec![game(1, "The Binding of Isaac: Rebirth", 2014), game(2, "The Binding of Isaac: Rebirth - Afterbirth", 2015)];
        assert_eq!(select_match("The Binding of Isaac Rebirth Complete Edition", "The Binding of Isaac Rebirth Complete Edition", matches).unwrap().id, 1);
        let matches = vec![game(1, "The Witcher 3: Wild Hunt", 2015), game(2, "The Witcher 3: Wild Hunt - Complete Edition", 2016)];
        assert_eq!(select_match("The Witcher 3 Wild Hunt Complete Edition", "The Witcher 3", matches).unwrap().id, 2);
    }
    #[test]
    fn accents_and_store_punctuation_do_not_prevent_matching() {
        assert_eq!(select_match("God of War Ragnarok", "God of War Ragnarok (2022-2024)", vec![game(1, "God of War Ragnarök", 2022)]).unwrap().id, 1);
        assert_eq!(select_match("Clair Obscur Expedition 33", "Clair Obscur Expedition 33", vec![game(2, "Clair Obscur: Expedition 33", 2025)]).unwrap().id, 2);
        assert_eq!(select_match("The Witcher 3 Wild Hunt", "The Witcher 3 (2015-2022)", vec![game(3, "The Witcher 3: Wild Hunt", 2015), game(4, "The Witcher 3: Wild Hunt - Game of the Year Edition", 2016)]).unwrap().id, 3);
        assert_eq!(select_match("EA SPORTS FC 27 Lite", "EA SPORTS FC 27 Demo", vec![game(5, "EA SPORTS FC 27", 2026), game(6, "EA SPORTS FC 26", 2025)]).unwrap().id, 5);
    }
}
