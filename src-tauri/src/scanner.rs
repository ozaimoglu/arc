use std::{collections::HashMap, path::{Path, PathBuf}};
use walkdir::WalkDir;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use crate::{metadata::{self, ExeMetadata}, models::{path_string, Candidate}, ps4};

const BLACKLIST: &[&str] = &["unins", "uninstall", "setup", "install", "vc_redist", "vcredist", "dxsetup", "unitycrashhandler", "crashreport", "crashhandler", "crashpad", "easyanticheat", "eac_launcher", "beservice", "belauncher", "ue4prereq", "updater", "update", "redistributable", "dotnet", "msiexec", "helper", "cefsubprocess", "launcher", "reporter", "service", "benchmark", "config", "diagnostic", "gamesessionmonitor", "blizzardbrowser", "crs-handler", "crs-uploader", "crs-video", "x360ce", "vconsole", "leagueclientux", "riotclient", "battle.net", "winmtr", "ucsvc", "iigw_server", "tslgame_be", "tslgame_zk", "csgo_legacy_app"];
const GENERIC: &[&str] = &["bin", "bin64", "binaries", "win64", "win32", "x64", "x86", "x64 dx12", "game", "games", "content", "engine", "shipping", "ship", "release", "debug", "app", "apps", "live", "installed games", "steam", "steamapps", "common", "steam library", "steam libraries", "riot games", "epic games", "epicgames", "gog games", "goggames", "ubisoft games", "battle net", "bethesda net launcher"];
const SKIP_FOLDERS: &[&str] = &["$recycle.bin", "system volume information", ".git", "node_modules", "_commonredist", "redist", "redistributables", "windows", "crack", "cracks", "backup", "backups", "__backup", "thirdparty", "battlEye", "riot client", "steamworks shared", "jre", "jre32", "jre64"];

pub fn normalize(value: &str) -> String {
    let chars: Vec<_> = value.nfkd().filter(|char| !is_combining_mark(*char)).collect();
    let mut out = String::new();
    for (index, char) in chars.iter().copied().enumerate() {
        let previous = index.checked_sub(1).and_then(|index| chars.get(index));
        let next = chars.get(index + 1);
        if char.is_uppercase() && (previous.is_some_and(|char| char.is_lowercase()) || (previous.is_some_and(|char| char.is_uppercase()) && next.is_some_and(|char| char.is_lowercase()))) { out.push(' '); }
        if char.is_alphanumeric() { if char == 'ø' || char == 'Ø' { out.push('o'); } else { out.extend(char.to_lowercase()); } }
        else { out.push(' '); }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn useful(value: &str) -> bool { !value.trim().is_empty() && !GENERIC.contains(&normalize(value).as_str()) && !["unreal engine", "unity", "unity player", "unityplayer", "bootstrap packaged game"].contains(&normalize(value).as_str()) }

pub fn clean_title(value: &str) -> String {
    let mut text = String::new(); let mut bracket = 0;
    for char in value.split('\0').next().unwrap_or_default().chars() {
        if char == '[' { bracket += 1; continue; }
        if char == ']' && bracket > 0 { bracket -= 1; continue; }
        if bracket == 0 && !char.is_control() && !['™', '®', '\u{fffd}'].contains(&char) { text.push(char); }
    }
    text = text.replace("(TM)", "").replace("(tm)", "").replace(['_', '.'], " ");
    loop {
        text = text.trim().to_owned();
        let Some(start) = text.rfind('(') else { break; };
        let suffix = &text[start..];
        if suffix.ends_with(')') && suffix[1..suffix.len()-1].chars().all(|char| char.is_ascii_digit() || ['-', ' ', '–'].contains(&char)) { text.truncate(start); } else { break; }
    }
    if text.ends_with(" Client") { text.truncate(text.len() - 7); }
    let chars: Vec<_> = text.chars().collect(); let mut spaced = String::new();
    for (index, char) in chars.iter().copied().enumerate() {
        if char.is_uppercase() && index > 0 && (chars[index-1].is_lowercase() || (chars[index-1].is_uppercase() && chars.get(index+1).is_some_and(|char| char.is_lowercase()))) { spaced.push(' '); }
        spaced.push(char);
    }
    let mut words: Vec<_> = spaced.split_whitespace().collect();
    // Distribution build suffixes are not part of a game's name or sequel number.
    if let Some(index) = words.iter().enumerate().skip(1).find_map(|(index, word)| {
        let version = word.strip_prefix('v').or_else(|| word.strip_prefix('V'));
        (version.is_some_and(|value| !value.is_empty() && value.chars().all(|char| char.is_ascii_digit()))
            && words[index + 1..].iter().all(|word| word.chars().all(|char| char.is_ascii_digit()) || ["fixed", "fix", "portable", "repack", "x64", "x86"].contains(&word.to_lowercase().as_str()))).then_some(index)
    }) { words.truncate(index); }
    words.join(" ")
}

pub fn excluded_path(path: &Path) -> bool {
    path.components().any(|part| skip_directory(&part.as_os_str().to_string_lossy()))
}

fn skip_directory(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.starts_with("battle.net.") || SKIP_FOLDERS.iter().any(|name| lower.eq_ignore_ascii_case(name))
}
pub fn blocked(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("shadps4") || ["editor.exe", "unrealeditor.exe", "fmeditor.exe", "option.exe", "kscviewer.exe"].contains(&lower.as_str()) || BLACKLIST.iter().any(|word| lower.contains(*word))
}
fn title_and_folder(path: &Path, root: &Path, metadata: &ExeMetadata, game_assets: bool) -> (String, PathBuf) {
    let parent = path.parent().unwrap_or(root);
    // Prefer the first meaningful game directory beneath the configured library root.
    let mut folder = root.to_path_buf();
    let relative = parent.strip_prefix(root).unwrap_or(parent);
    let mut running = root.to_path_buf();
    for part in relative.components() {
        running.push(part);
        if useful(&part.as_os_str().to_string_lossy()) { folder = running; break; }
    }
    let folder_name = clean_title(&folder.file_name().unwrap_or_default().to_string_lossy());
    let meta = metadata.product_name.as_deref().map(clean_title).filter(|s| useful(s))
        .or_else(|| metadata.description.as_deref().map(clean_title).filter(|s| useful(s)));
    let folder_title = (useful(&folder_name) && (folder != root || parent != root || game_assets)).then_some(folder_name);
    let title = match (meta, folder_title) {
        (Some(meta), Some(folder)) => {
            let short = normalize(&meta); let full = normalize(&folder);
            let internal = short.chars().filter(|char| char.is_alphanumeric()).count() <= 4
                || (short.split_whitespace().count() == 1 && full.split_whitespace().count() > 1 && strsim::jaro_winkler(&short, &full) < 0.8)
                || (short.ends_with(" game") && !full.contains(&short));
            let shortened = full.split_whitespace().count() > short.split_whitespace().count()
                && short.split_whitespace().all(|word| full.split_whitespace().any(|part| part == word))
                && !full.split_whitespace().any(|word| ["edition", "deluxe", "complete", "goty", "portable", "repack", "en", "eu", "us", "kr"].contains(&word));
            if internal || shortened || (full.contains("remake") && !short.contains("remake")) { folder } else { meta }
        },
        (Some(meta), None) => meta,
        (None, Some(folder)) => folder,
        (None, None) => clean_title(&path.file_stem().unwrap_or_default().to_string_lossy().replace('-', " ")),
    };
    (title, folder)
}

fn manifest_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let parts: Vec<_> = text.split('"').collect();
    parts.windows(3).find(|parts| parts[0] == key && parts[1].trim().is_empty()).map(|parts| parts[2])
}

fn steam_title(folder: &Path) -> Option<String> {
    let common = folder.parent()?;
    if !common.file_name()?.to_string_lossy().eq_ignore_ascii_case("common") { return None; }
    let steamapps = common.parent()?;
    if !steamapps.file_name()?.to_string_lossy().eq_ignore_ascii_case("steamapps") { return None; }
    let directory = folder.file_name()?.to_string_lossy();
    for entry in std::fs::read_dir(steamapps).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.starts_with("appmanifest_") || !name.ends_with(".acf") || entry.metadata().ok().is_none_or(|meta| meta.len() > 128 * 1024) { continue; }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue; };
        if manifest_field(&text, "installdir").is_some_and(|value| value.eq_ignore_ascii_case(&directory)) {
            return manifest_field(&text, "name").map(clean_title).filter(|name| useful(name));
        }
    }
    None
}
pub fn score(name: &str, size: u64, title: &str, folder_named: bool, metadata: &ExeMetadata) -> i32 {
    let support = [metadata.product_name.as_deref(), metadata.description.as_deref()].into_iter().flatten().any(|value| {
        let value = normalize(value);
        ["battle net", "blizzard browser", "game session monitor", "riot client", "crash reporter", "crash handler", "wellbia com security", "xigncode"].iter().any(|prefix| value.starts_with(prefix))
    });
    if blocked(name) || support { return -100; }
    let mut score = 10;
    if size >= 20 * 1024 * 1024 { score += 40; } else if size >= 1024 * 1024 { score += 10; } else { score -= 30; }
    if folder_named { score += 30; }
    if metadata.product_name.as_deref().is_some_and(useful) { score += 25; }
    if metadata.description.as_deref().is_some_and(|desc| strsim::jaro_winkler(&normalize(desc), &normalize(title)) >= 0.8) { score += 15; }
    let stem = Path::new(name).file_stem().unwrap_or_default().to_string_lossy();
    if strsim::jaro_winkler(&normalize(&stem), &normalize(title)) >= 0.75 { score += 20; }
    if name.to_lowercase().contains("shipping") { score += 10; }
    score
}

fn substantial_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() >= 1024 * 1024)
}

fn has_game_assets(path: &Path) -> bool {
    let Some(parent) = path.parent() else { return false; };
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    // A matching Unity data directory plus the engine distinguishes tiny game
    // bootstraps from unrelated executables that merely share a folder name.
    if parent.join(format!("{stem}_Data")).is_dir() && parent.join("UnityPlayer.dll").is_file() { return true; }
    let steam = parent.join("steam_api.dll").is_file() || parent.join("steam_api64.dll").is_file();
    if steam {
        // Native games can keep their real payload in packed resource archives.
        if std::fs::read_dir(parent.join("resources/packed")).is_ok_and(|entries| entries.flatten().any(|entry| {
            entry.path().extension().is_some_and(|extension| ["a", "pak", "pck", "dat"].iter().any(|kind| extension.eq_ignore_ascii_case(kind)))
                && substantial_file(&entry.path())
        })) { return true; }
        // Java launchers carry an EXE-specific configuration. Require its entry
        // point, local game JAR and bundled runtime, not just the presence of Java.
        let config_path = path.with_extension("json");
        if std::fs::metadata(&config_path).is_ok_and(|meta| meta.len() <= 128 * 1024) {
            if let Some(config) = std::fs::read(&config_path).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()) {
                let entry = config.get("mainClass").and_then(|value| value.as_str()).is_some_and(|value| !value.is_empty());
                let jar = config.get("classpath").and_then(|value| value.as_array()).is_some_and(|items| items.iter().filter_map(|item| item.as_str()).any(|value| {
                    let relative = Path::new(value);
                    // Only examine local payloads within this installation.
                    !relative.is_absolute() && relative.components().all(|part| !matches!(part, std::path::Component::ParentDir | std::path::Component::Prefix(_)))
                        && relative.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("jar")) && substantial_file(&parent.join(relative))
                }));
                let runtime = ["jre/bin/java.exe", "jre32/bin/java.exe", "jre64/bin/java.exe"].iter().any(|relative| parent.join(relative).is_file());
                if entry && jar && runtime { return true; }
            }
        }
    }
    // id Tech games store assets outside their small native executable. Keep
    // the engine's player entry points distinct from utilities in the same folder.
    match stem.to_lowercase().as_str() {
        "quake3" | "ioquake3" => substantial_file(&parent.join("baseq3/pak0.pk3")),
        "quake2" => substantial_file(&parent.join("baseq2/pak0.pak")),
        "quake" | "glquake" | "winquake" => substantial_file(&parent.join("id1/pak0.pak")),
        _ => false,
    }
}

pub fn scan(folders: &[String]) -> (Vec<Candidate>, usize, Vec<String>) {
    let mut candidates: HashMap<String, Candidate> = HashMap::new(); let mut ignored = 0; let mut warnings = vec![];
    let mut steam_names: HashMap<PathBuf, Option<String>> = HashMap::new();
    for folder in folders {
        let root = match std::fs::canonicalize(folder) { Ok(root) if root.is_dir() => root, _ => { warnings.push(format!("Folder unavailable: {folder}.")); continue; } };
        let mut failed = 0;
        for entry in WalkDir::new(&root).follow_links(false).max_depth(18).into_iter().filter_entry(|entry| {
            !entry.file_type().is_dir() || !skip_directory(&entry.file_name().to_string_lossy())
        }) {
            let entry = match entry { Ok(entry) => entry, Err(_) => { failed += 1; continue; } };
            let path = entry.path();
            if entry.file_type().is_file() && entry.file_name().to_string_lossy().eq_ignore_ascii_case("eboot.bin") {
                if let Some(candidate) = ps4::candidate(path) {
                    let serial = match candidate.console_launch.as_ref().unwrap() {
                        crate::models::ConsoleLaunch::ShadPs4 { title_id, .. } | crate::models::ConsoleLaunch::BloodborneLauncher { title_id, .. } => title_id,
                    };
                    let key = format!("ps4:{serial}");
                    if candidates.get(&key).is_none_or(|old| candidate.exe_path < old.exe_path) { candidates.insert(key, candidate); }
                    else { ignored += 1; }
                } else { ignored += 1; }
                continue;
            }
            if !entry.file_type().is_file() || !path.extension().is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("exe")) { continue; }
            let name = entry.file_name().to_string_lossy();
            if blocked(&name) { ignored += 1; continue; }
            let size = match entry.metadata() { Ok(data) => data.len(), Err(_) => { failed += 1; continue; } };
            let metadata = metadata::read(path);
            let game_assets = has_game_assets(path);
            let (mut title, folder) = title_and_folder(path, &root, &metadata, game_assets);
            let registered = steam_names.entry(folder.clone()).or_insert_with(|| steam_title(&folder));
            if let Some(name) = registered { title = name.clone(); }
            let named_installation = folder != root || (game_assets && useful(&folder.file_name().unwrap_or_default().to_string_lossy()));
            let mut points = score(&name, size, &title, named_installation, &metadata);
            if registered.is_some() { points += 50; }
            if game_assets { points += 70; }
            if points < 60 { ignored += 1; continue; }
            let candidate = Candidate { title, exe_path: path_string(path), folder: path_string(&folder), score: points, console_launch: None };
            // Unreal titles can have both bootstrap and Shipping binaries. Pick one per game folder.
            let key = if folder == root && !named_installation { candidate.exe_path.to_lowercase() } else { candidate.folder.to_lowercase() };
            if let Some(existing) = candidates.get(&key) {
                if existing.score > candidate.score || (existing.score == candidate.score && existing.exe_path <= candidate.exe_path) { ignored += 1; continue; }
                ignored += 1;
            }
            candidates.insert(key, candidate);
        }
        if failed > 0 { warnings.push(format!("{failed} paths could not be read in {folder}.")); }
    }
    let mut result: Vec<_> = candidates.into_values().collect(); result.sort_by(|a, b| a.exe_path.cmp(&b.exe_path));
    (result, ignored, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn support_executables_never_pass_even_with_large_size() {
        for name in ["UnityCrashHandler64.exe", "unins000.exe", "setup.exe", "vc_redist.exe", "launcher.exe", "EasyAntiCheat.exe", "CrashReportClient.exe", "Option.exe", "KscViewer.exe"] {
            assert_eq!(score(name, 50_000_000, "Elden Ring", true, &ExeMetadata::default()), -100);
        }
        let metadata = ExeMetadata { product_name: Some("Battle.net Overlay Runtime".into()), description: None };
        assert_eq!(score("unknown.exe", 50_000_000, "Battle net Overlay Runtime", true, &metadata), -100);
        let metadata = ExeMetadata { product_name: Some("Wellbia.com Security Loader".into()), description: None };
        assert_eq!(score("xldr_KnightOnline_NA_loader_win32.exe", 50_000_000, "Knight Online", true, &metadata), -100);
    }
    #[test] fn large_game_and_small_bootstrap_with_metadata_pass() {
        assert!(score("eldenring.exe", 50_000_000, "Elden Ring", true, &ExeMetadata::default()) >= 60);
        let data = ExeMetadata { product_name: Some("Hollow Knight".into()), description: Some("Hollow Knight".into()) };
        assert!(score("hollow_knight.exe", 400_000, "Hollow Knight", true, &data) >= 60);
    }
    #[test] fn tiny_unknown_executable_is_rejected() { assert!(score("random.exe", 1000, "Unknown Game", true, &ExeMetadata::default()) < 60); }
    fn assets_fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("arc-assets-test-{name}-{}-{}", std::process::id(), crate::models::now()));
        std::fs::create_dir_all(&root).unwrap(); root
    }
    fn payload(path: &Path, size: u64) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::File::create(path).unwrap().set_len(size).unwrap();
    }
    #[test] fn unity_payload_identifies_small_games_from_library_or_exact_root() {
        let root = assets_fixture("unity"); let folder = root.join("Escape from Duckov (2025)");
        payload(&folder.join("Duckov.exe"), 666_624);
        payload(&folder.join("UnityPlayer.dll"), 1);
        std::fs::create_dir_all(folder.join("Duckov_Data")).unwrap();
        payload(&folder.join("UnityCrashHandler64.exe"), 32_000_000);
        std::fs::create_dir_all(folder.join("UnityCrashHandler64_Data")).unwrap();
        let (library, _, warnings) = scan(&[path_string(&root)]);
        let (direct, _, _) = scan(&[path_string(&folder)]);
        assert!(warnings.is_empty()); assert_eq!(library.len(), 1); assert_eq!(direct.len(), 1);
        assert_eq!(library[0].title, "Escape from Duckov"); assert_eq!(direct[0].title, library[0].title);
        assert_eq!(direct[0].exe_path, library[0].exe_path);
        // A data folder on its own is insufficient evidence.
        std::fs::remove_file(folder.join("UnityPlayer.dll")).unwrap();
        assert!(scan(&[path_string(&root)]).0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn java_game_requires_local_config_payload_runtime_and_game_sdk() {
        let root = assets_fixture("java"); let folder = root.join("Project Zomboid");
        let exe = folder.join("ProjectZomboid64.exe"); payload(&exe, 990_720);
        payload(&folder.join("steam_api64.dll"), 1);
        payload(&folder.join("projectzomboid.jar"), 2_000_000);
        payload(&folder.join("jre64/bin/java.exe"), 32_000_000);
        std::fs::write(exe.with_extension("json"), r#"{"mainClass":"zombie/gameStates/MainScreenState","classpath":[".","projectzomboid.jar"]}"#).unwrap();
        let (games, _, _) = scan(&[path_string(&root)]);
        assert_eq!(games.len(), 1); assert_eq!(games[0].title, "Project Zomboid");
        assert!(games[0].exe_path.ends_with("ProjectZomboid64.exe"));
        // Malformed / external classpaths must not validate unrelated programs.
        std::fs::write(exe.with_extension("json"), r#"{"mainClass":"Other","classpath":["../projectzomboid.jar"]}"#).unwrap();
        assert!(!has_game_assets(&exe));
        std::fs::write(exe.with_extension("json"), "not JSON").unwrap();
        assert!(!has_game_assets(&exe));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn native_packed_game_requires_both_archives_and_sdk() {
        let root = assets_fixture("packed"); let folder = root.join("The Binding of Isaac Rebirth Complete Edition");
        let exe = folder.join("isaac-ng.exe"); payload(&exe, 8_691_480);
        payload(&folder.join("steam_api.dll"), 1);
        payload(&folder.join("resources/packed/graphics.a"), 2_000_000);
        payload(&folder.join("setup.exe"), 32_000_000);
        let (games, _, _) = scan(&[path_string(&root)]);
        assert_eq!(games.len(), 1); assert!(games[0].exe_path.ends_with("isaac-ng.exe"));
        std::fs::remove_file(folder.join("resources/packed/graphics.a")).unwrap();
        assert!(scan(&[path_string(&root)]).0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn id_tech_archives_identify_game_under_bethesda_container() {
        let root = assets_fixture("id-tech"); let folder = root.join("Bethesda.net Launcher/games/Quake III Arena");
        payload(&folder.join("quake3.exe"), 905_216);
        payload(&folder.join("baseq3/pak0.pk3"), 2_000_000);
        payload(&folder.join("unknown.exe"), 905_216);
        let (games, _, warnings) = scan(&[path_string(&root)]);
        assert!(warnings.is_empty()); assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Quake III Arena"); assert!(games[0].folder.ends_with("Quake III Arena"));
        std::fs::remove_file(folder.join("baseq3/pak0.pk3")).unwrap();
        assert!(scan(&[path_string(&root)]).0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn build_suffixes_and_region_folders_do_not_change_game_identity() {
        assert_eq!(clean_title("Farm.Together.2.v52.FIXED"), "Farm Together 2");
        assert_eq!(clean_title("Game.v1.2.3.Fixed"), "Game");
        assert_eq!(clean_title("Game V2 Chapter 3"), "Game V2 Chapter 3");
        assert_eq!(clean_title("V2"), "V2");
        let meta = ExeMetadata { product_name: Some("Knight Online Client".into()), description: None };
        let (title, _) = title_and_folder(Path::new("/library/KnightOnlineEn/KnightOnLine.exe"), Path::new("/library"), &meta, false);
        assert_eq!(title, "Knight Online");
    }
    #[test] fn nested_binary_uses_game_folder() {
        let root = Path::new("/library"); let path = Path::new("/library/The Witcher 3/bin/x64/witcher3.exe");
        let (title, folder) = title_and_folder(path, root, &ExeMetadata::default(), false);
        assert_eq!(title, "The Witcher 3"); assert_eq!(folder, Path::new("/library/The Witcher 3"));
    }
    #[test] fn normalization_preserves_game_identity() { assert_eq!(normalize("The Witcher 3: Wild Hunt"), "the witcher 3 wild hunt"); }
    #[test] fn internal_project_names_use_readable_installation_names() {
        let root = Path::new("/library");
        for (directory, project, expected) in [
            ("Black Myth Wukong [v1.0.21] [Portable]", "b1", "Black Myth Wukong"),
            ("Hades II (2025)", "F10", "Hades II"),
            ("The First Berserker Khazan (2025)", "KZ", "The First Berserker Khazan"),
            ("The Blood of Dawnwalker (2026)", "Dawnwalker", "The Blood of Dawnwalker"),
            ("Clair Obscur. Expedition 33 (2025)", "Expedition 33", "Clair Obscur Expedition 33"),
            ("God of War Ragnarok (2022-2024)", "GoWR", "God of War Ragnarok"),
            ("Resident Evil 4 Remake (2023)", "Resident Evil 4", "Resident Evil 4 Remake"),
            ("PUBG", "TslGame", "PUBG"),
        ] {
            let path = root.join(directory).join("Binaries/Win64/game.exe");
            let meta = ExeMetadata { product_name: Some(project.into()), description: Some(project.into()) };
            assert_eq!(title_and_folder(&path, root, &meta, false).0, expected);
        }
    }
    #[test] fn library_containers_do_not_merge_different_installations() {
        let root = Path::new("/library");
        for relative in ["SteamLibrary/steamapps/common/Apex Legends/bin/apex.exe", "Riot Games/League of Legends/Game/game.exe"] {
            let (_, folder) = title_and_folder(&root.join(relative), root, &ExeMetadata::default(), false);
            assert!(folder.ends_with("Apex Legends") || folder.ends_with("League of Legends"));
        }
    }
    #[test] fn title_cleanup_handles_trademarks_dotted_folders_and_accents() {
        assert_eq!(clean_title("DARK SOULS™ III"), "DARK SOULS III");
        assert_eq!(clean_title("League of Legends (TM) Client"), "League of Legends");
        assert_eq!(clean_title("CONTROLResonant"), "CONTROL Resonant");
        assert_eq!(normalize("God of War Ragnarök"), "god of war ragnarok");
        assert!(blocked("GameSessionMonitor.exe"));
        assert!(blocked("editor.exe"));
        assert!(excluded_path(Path::new("/games/Game/Backup/game.exe")));
        assert!(excluded_path(Path::new("/games/Battle.net/Battle.net.12345/BlizzardError.exe")));
        let (title, _) = title_and_folder(Path::new("/library/Stellar.Blade.Complete.Edition-Copy/game.exe"), Path::new("/library"), &ExeMetadata { product_name: Some("Stellar Blade".into()), description: None }, false);
        assert_eq!(title, "Stellar Blade");
    }
    #[test] fn steam_manifests_identify_small_games_and_keep_them_separate() {
        let root = std::env::temp_dir().join(format!("arc-steam-test-{}-{}", std::process::id(), crate::models::now()));
        let apps = root.join("SteamLibrary/steamapps");
        for (id, directory, title, exe) in [(1, "Counter-Strike Global Offensive", "Counter-Strike 2", "cs2.exe"), (2, "MARVEL SNAP", "MARVEL SNAP", "SNAP.exe")] {
            let folder = apps.join("common").join(directory); std::fs::create_dir_all(&folder).unwrap();
            std::fs::File::create(folder.join(exe)).unwrap().set_len(600_000).unwrap();
            std::fs::write(apps.join(format!("appmanifest_{id}.acf")), format!("\"AppState\"\n{{\n\"name\" \"{title}\"\n\"installdir\" \"{directory}\"\n}}" )).unwrap();
        }
        let (games, _, warnings) = scan(&[path_string(&root)]); std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(games.len(), 2); assert!(warnings.is_empty());
        assert!(games.iter().any(|game| game.title == "Counter-Strike 2")); assert!(games.iter().any(|game| game.title == "MARVEL SNAP"));
    }
    #[test] fn scan_deduplicates_installations_but_retains_loose_games() {
        let root = std::env::temp_dir().join(format!("arc-scanner-test-{}-{}", std::process::id(), crate::models::now()));
        let install = root.join("Game One"); let nested = install.join("bin/x64");
        std::fs::create_dir_all(&nested).unwrap();
        let backup = install.join("Backup"); std::fs::create_dir_all(&backup).unwrap();
        for path in [install.join("GameOne.exe"), nested.join("GameOne-Win64-Shipping.exe"), install.join("setup.exe"), backup.join("GameOne-Win64-Shipping.exe"), root.join("Alpha.exe"), root.join("Beta.exe")] {
            std::fs::File::create(path).unwrap().set_len(32 * 1024 * 1024).unwrap();
        }
        let (games, ignored, warnings) = scan(&[path_string(&root)]);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(games.len(), 3); assert!(ignored >= 2); assert!(warnings.is_empty());
        assert!(games.iter().any(|game| game.title == "Alpha")); assert!(games.iter().any(|game| game.title == "Beta"));
    }
    #[test] fn ps4_base_games_are_imported_once_and_emulators_are_not_native_games() {
        let root = assets_fixture("ps4");
        payload(&root.join("shadPS4.exe"), 32_000_000);
        for (directory, category) in [("Games/CUSA03173", "gd"), ("Games/CUSA03173-UPDATE", "gp"), ("Games/savedata/CUSA03173", "sd"), ("Games/DLC/CUSA03173", "ac"), ("Games/Duplicate", "gd")] {
            let folder = root.join(directory); payload(&folder.join("eboot.bin"), 10);
            std::fs::create_dir_all(folder.join("sce_sys")).unwrap();
            std::fs::write(folder.join("sce_sys/param.sfo"), ps4::tests::sfo("Bloodborne", "CUSA03173", category)).unwrap();
        }
        let (games, _, warnings) = scan(&[path_string(&root), path_string(&root.join("Games"))]);
        assert!(warnings.is_empty()); assert_eq!(games.len(), 1); assert_eq!(games[0].title, "Bloodborne");
        assert!(games[0].console_launch.is_some()); assert!(games[0].exe_path.ends_with("eboot.bin"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
