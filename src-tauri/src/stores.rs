//! Store routes are derived from local installation identity, not game titles.
use std::{collections::BTreeSet, path::{Path, PathBuf}};
use serde_json::Value;
use crate::{launcher_metadata, models::path_string, registry::{self, Hive}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store { Epic, Riot, Ea, Ubisoft, BattleNet, Rockstar, Gog, Ntt }
impl Store {
    pub fn name(self) -> &'static str { match self { Self::Epic => "Epic Games", Self::Riot => "Riot Client", Self::Ea => "EA App", Self::Ubisoft => "Ubisoft Connect", Self::BattleNet => "Battle.net", Self::Rockstar => "Rockstar Games", Self::Gog => "GOG Galaxy", Self::Ntt => "NTTGame" } }
}
#[derive(Debug, Clone)]
pub struct Route { pub store: Store, pub program: PathBuf, pub args: Vec<String>, pub install: PathBuf }
#[derive(Debug, Clone)]
struct Registration { store: Store, install: PathBuf, args: Vec<String>, client: Option<PathBuf> }

fn json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&launcher_metadata::read(path, 1024 * 1024)?).ok()
}
fn token(value: &str) -> bool { !value.is_empty() && value.len() <= 200 && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)) }
fn numeric(value: &str) -> bool { !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()) && value.parse::<u64>().is_ok_and(|id| id > 0) }
fn child(path: &Path, install: &Path) -> Option<PathBuf> {
    let root = std::fs::canonicalize(install).ok()?;
    path.starts_with(&root).then_some(root)
}
fn executable(path: &Path, expected: &[&str]) -> Option<PathBuf> {
    let path = std::fs::canonicalize(path).ok()?;
    (path.is_file() && path.file_name().is_some_and(|name| expected.iter().any(|expected| name.eq_ignore_ascii_case(expected)))).then_some(path)
}
fn protocol_client(scheme: &str, expected: &[&str]) -> Option<PathBuf> {
    let command = registry::string(Hive::Classes, &format!("{scheme}\\shell\\open\\command"), "")?;
    executable(&registry::executable_from_command(&command)?, expected)
}
fn client(store: Store) -> Option<PathBuf> {
    match store {
        Store::Epic => protocol_client("com.epicgames.launcher", &["EpicGamesLauncher.exe"]),
        Store::Ea => protocol_client("origin2", &["EALauncher.exe", "EADesktop.exe", "Origin.exe"]),
        Store::Ubisoft => protocol_client("uplay", &["UbisoftConnect.exe", "upc.exe", "Uplay.exe"]),
        Store::BattleNet => protocol_client("battlenet", &["Battle.net.exe"]),
        Store::Rockstar => protocol_client("rockstar", &["Launcher.exe"]),
        Store::Gog => ["SOFTWARE\\WOW6432Node\\GOG.com\\GalaxyClient\\paths", "SOFTWARE\\GOG.com\\GalaxyClient\\paths"].into_iter().find_map(|key| {
            let directory = registry::string(Hive::Machine, key, "client")?;
            executable(&PathBuf::from(directory).join("GalaxyClient.exe"), &["GalaxyClient.exe"])
        }),
        Store::Riot | Store::Ntt => None,
    }
}
fn epic(value: &Value) -> Option<Registration> {
    if value.get("bIsIncompleteInstall").and_then(Value::as_bool) == Some(true) { return None; }
    let install = PathBuf::from(value.get("InstallLocation")?.as_str()?);
    let app = value.get("AppName")?.as_str()?;
    if !token(app) { return None; }
    let namespace = value.get("CatalogNamespace").or_else(|| value.get("NamespaceId")).and_then(Value::as_str);
    let item = value.get("CatalogItemId").or_else(|| value.get("ItemId")).and_then(Value::as_str);
    let identity = match (namespace, item) {
        (Some(namespace), Some(item)) if token(namespace) && token(item) => format!("{namespace}%3A{item}%3A{app}"),
        (None, None) => app.into(),
        _ => return None,
    };
    Some(Registration { store: Store::Epic, install, args: vec![format!("com.epicgames.launcher://apps/{identity}?action=launch&silent=true")], client: None })
}
fn epic_local(game: &Path) -> Result<Option<Registration>, String> {
    for install in game.ancestors().skip(1).take(12) {
        let Ok(entries) = std::fs::read_dir(install.join(".egstore")) else { continue; };
        let mut records = vec![];
        for entry in entries.flatten().take(256) {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mancpn")) {
                if let Some(mut value) = json(&path).filter(Value::is_object) {
                    value["InstallLocation"] = path_string(install).into();
                    if let Some(record) = epic(&value) { records.push(record); }
                }
            } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("manifest"))
                && game.file_name().is_some_and(|name| name.eq_ignore_ascii_case("FortniteClient-Win64-Shipping.exe")) {
                if let Some((app, launch)) = launcher_metadata::read(&path, 16 * 1024 * 1024 + 4096).and_then(|bytes| launcher_metadata::epic_manifest(&bytes)) {
                    // Fortnite's download name differs from its catalogue launch alias.
                    // Do not infer aliases for other products from binary manifests.
                    let launch_path = Path::new(&launch);
                    if app == "FortniteReleaseBuilds" && launch_path.components().all(|part| matches!(part, std::path::Component::Normal(_)))
                        && executable(&install.join(launch_path), &["FortniteBootstrapper.exe"]).is_some_and(|exe| child(&exe, install).is_some()) {
                        if let Some(record) = epic(&serde_json::json!({"InstallLocation": path_string(install), "AppName": "Fortnite"})) { records.push(record); }
                    }
                }
            }
        }
        if let Some(record) = select(records, game)? { return Ok(Some(record)); }
    }
    Ok(None)
}
fn yaml_install(text: &str) -> Option<PathBuf> {
    let value = text.lines().find_map(|line| line.strip_prefix("product_install_full_path:"))?.trim();
    let value = if value.starts_with('"') { serde_json::from_str::<String>(value).ok()? }
        else if value.starts_with('\'') { value.strip_prefix('\'')?.strip_suffix('\'')?.replace("''", "'") }
        else { value.to_owned() };
    Some(PathBuf::from(value))
}
fn riot(program_data: &Path, game: &Path) -> Result<Option<Registration>, String> {
    let directory = program_data.join("Riot Games");
    let Some(clients) = json(&directory.join("RiotClientInstalls.json")) else { return Ok(None); };
    let metadata = directory.join("Metadata");
    let Ok(entries) = std::fs::read_dir(metadata) else { return Ok(None); };
    let mut records = vec![];
    for entry in entries.flatten().take(256) {
        let name = entry.file_name().to_string_lossy().into_owned(); let Some((product, patchline)) = name.split_once('.') else { continue; };
        if !["league_of_legends", "valorant", "bacon", "lion"].contains(&product) || !["live", "pbe"].contains(&patchline) { continue; }
        let settings = entry.path().join(format!("{name}.product_settings.yaml"));
        let Some(install) = launcher_metadata::read(&settings, 65536).and_then(|bytes| String::from_utf8(bytes).ok()).and_then(|text| yaml_install(&text)) else { continue; };
        let Some(install) = child(game, &install) else { continue; };
        let associated = clients.get("associated_client").and_then(Value::as_object).and_then(|values| values.iter().filter_map(|(root, client)| {
            child(game, Path::new(root)).and_then(|root| client.as_str().map(|client| (root.components().count(), PathBuf::from(client))))
        }).max_by_key(|(depth, _)| *depth).map(|(_, client)| client));
        let Some(client) = associated.or_else(|| clients.get("rc_live").and_then(Value::as_str).map(PathBuf::from)) else { continue; };
        records.push(Registration { store: Store::Riot, install, args: vec![format!("--launch-product={product}"), format!("--launch-patchline={patchline}")], client: Some(client) });
    }
    select(records, game)
}
fn registry_installs(store: Store, key: &str, field: &str) -> Vec<Registration> {
    registry::subkeys(Hive::Machine, key).into_iter().filter_map(|id| {
        if !numeric(&id) { return None; }
        let install = PathBuf::from(registry::string(Hive::Machine, &format!("{key}\\{id}"), field)?);
        let args = numeric_args(store, &id, &install)?;
        // GOG DLC registry entries have no executable; they must not capture the base game.
        if store == Store::Gog && registry::string(Hive::Machine, &format!("{key}\\{id}"), "exe").is_none() { return None; }
        Some(Registration { store, install, args, client: None })
    }).collect()
}
fn numeric_args(store: Store, id: &str, install: &Path) -> Option<Vec<String>> {
    if !numeric(id) { return None; }
    match store {
        Store::Ubisoft => Some(vec![format!("uplay://launch/{id}")]),
        Store::Gog => Some(vec!["/launchViaAutostart".into(), format!("/gameId={id}"), "/command=runGame".into(), format!("/path={}", path_string(install))]),
        _ => None,
    }
}
fn battle_id(internal: &str) -> Option<&'static str> {
    // Client launch codes differ from Agent/uninstaller internal identifiers.
    [ ("wow", "WoW"), ("diablo3", "D3"), ("s2", "S2"), ("s1", "S1"), ("hs_beta", "WTCG"),
      ("heroes", "Hero"), ("prometheus", "Pro"), ("viper", "VIPR"), ("odin", "ODIN"), ("w3", "W3"),
      ("lazarus", "LAZR"), ("zeus", "ZEUS"), ("wlby", "WLBY"), ("osi", "OSI"), ("rtro", "RTRO"),
      ("fore", "FORE"), ("anbs", "ANBS"), ("auks", "AUKS"), ("Fen", "Fen"), ("D1", "D1"),
      ("w1r", "W1R"), ("w2r", "W2R"), ("w1", "W1"), ("w2", "W2"), ("gryphon", "GRY"),
      ("aris", "ARIS"), ("scorpio", "SCOR"), ("arkansas", "ARK"), ("libra", "LBRA"), ("pinta", "PNTA"), ("aqua", "AQUA") ]
        .into_iter().find_map(|(name, id)| name.eq_ignore_ascii_case(internal).then_some(id))
}
fn battle_registration(internal: &str, install: PathBuf) -> Option<Registration> {
    Some(Registration { store: Store::BattleNet, install, args: vec![format!("--exec=launch {}", battle_id(internal)?)], client: None })
}
fn battle_database(program_data: &Path, game: &Path) -> Result<Option<Registration>, String> {
    let records = launcher_metadata::read(&program_data.join("Battle.net/Agent/product.db"), 4 * 1024 * 1024)
        .and_then(|bytes| launcher_metadata::battle_net(&bytes)).unwrap_or_default().into_iter()
        .filter_map(|(internal, path)| battle_registration(&internal, path.into())).collect();
    select(records, game)
}
fn rockstar_registration(install: PathBuf) -> Registration {
    Registration { store: Store::Rockstar, args: vec!["-launchTitleInFolder".into(), path_string(&install)], install, client: None }
}
fn rockstar(game: &Path) -> Result<Option<Registration>, String> {
    let mut records = vec![];
    for hive in [Hive::Machine, Hive::User] {
        for root in ["SOFTWARE\\Rockstar Games", "SOFTWARE\\WOW6432Node\\Rockstar Games"] {
            for name in registry::subkeys(hive, root) {
                // Steam/Epic-specific fields deliberately keep their owning store's route.
                if let Some(path) = registry::string(hive, &format!("{root}\\{name}"), "InstallFolder") {
                    records.push(rockstar_registration(path.into()));
                }
            }
        }
    }
    select(records, game)
}
fn ea_ids(text: &str) -> Option<String> {
    let mut ids = BTreeSet::new();
    for part in text.split("<contentID>").skip(1) {
        let id = part.split_once("</contentID>")?.0.trim();
        if id.is_empty() || id.len() > 200 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-:".contains(&byte)) { return None; }
        ids.insert(id.replace(':', "%3A"));
    }
    (!ids.is_empty()).then(|| ids.into_iter().collect::<Vec<_>>().join(","))
}
fn ea(game: &Path) -> Option<Registration> {
    for key in ["SOFTWARE\\WOW6432Node\\EA Games", "SOFTWARE\\EA Games", "SOFTWARE\\WOW6432Node\\Origin Games", "SOFTWARE\\Origin Games"] {
        for id in registry::subkeys(Hive::Machine, key) {
            let key = format!("{key}\\{id}");
            for field in ["Install Dir", "InstallDir", "InstallLocation", "InstallPath"] {
                let Some(path) = registry::string(Hive::Machine, &key, field) else { continue; };
                let Some(install) = child(game, Path::new(&path)) else { continue; };
                if let Some(record) = ea_registration(install) { return Some(record); }
            }
        }
    }
    None
}
fn ea_registration(install: PathBuf) -> Option<Registration> {
    let file = install.join("__Installer/installerdata.xml");
    let ids = launcher_metadata::read(&file, 1024 * 1024).and_then(|bytes| String::from_utf8(bytes).ok()).and_then(|text| ea_ids(&text))?;
    Some(Registration { store: Store::Ea, install, args: vec![format!("origin2://game/launch/?offerIds={ids}")], client: None })
}
fn uninstall(game: &Path) -> Option<Registration> {
    for hive in [Hive::Machine, Hive::User] {
        for root in ["SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall", "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall"] {
            for name in registry::subkeys(hive, root) {
                let key = format!("{root}\\{name}");
                let Some(install) = registry::string(hive, &key, "InstallLocation").and_then(|path| child(game, Path::new(&path))) else { continue; };
                let command = registry::string(hive, &key, "UninstallString").unwrap_or_default();
                let publisher = registry::string(hive, &key, "Publisher").unwrap_or_default().to_ascii_lowercase();
                if ["electronic arts", "ea games", "ea sports"].iter().any(|name| publisher.contains(name)) {
                    if let Some(record) = ea_registration(install.clone()) { return Some(record); }
                }
                if publisher.contains("blizzard") {
                    let id = command.split("--uid=").nth(1).and_then(|value| value.split_whitespace().next()).unwrap_or_default().trim_matches('"');
                    if let Some(record) = battle_registration(id, install.clone()) { return Some(record); }
                }
                if publisher.contains("rockstar") && command.to_ascii_lowercase().contains("-uninstall=") {
                    return Some(rockstar_registration(install));
                }
            }
        }
    }
    None
}
fn finish(registration: Registration, game: &Path) -> Result<Route, String> {
    let install = child(game, &registration.install).ok_or("The launcher installation changed. Scan your library again.")?;
    let program = if let Some(path) = registration.client {
        executable(&path, if registration.store == Store::Riot { &["RiotClientServices.exe"] } else { &["Launcher.exe"] })
    } else { client(registration.store) }.ok_or_else(|| format!("{} is required to start this game. Open or repair that launcher, then try again.", registration.store.name()))?;
    Ok(Route { store: registration.store, program, args: registration.args, install })
}
fn select(registrations: Vec<Registration>, game: &Path) -> Result<Option<Registration>, String> {
    let mut found: Option<Registration> = None;
    for mut registration in registrations {
        let Some(root) = child(game, &registration.install) else { continue; }; registration.install = root;
        if let Some(old) = &found {
            if old.install == registration.install && old.args != registration.args { return Err("This installation has conflicting launcher records. Repair the launcher installation, then try again.".into()); }
            if old.install.components().count() >= registration.install.components().count() { continue; }
        }
        found = Some(registration);
    }
    Ok(found)
}
pub fn route(game: &Path) -> Result<Option<Route>, String> {
    let program_data = std::env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:/ProgramData"));
    let mut epic_records = vec![];
    if let Ok(entries) = std::fs::read_dir(program_data.join("Epic/EpicGamesLauncher/Data/Manifests")) {
        for entry in entries.flatten().take(4096) {
            if entry.path().extension().is_some_and(|extension| extension.eq_ignore_ascii_case("item")) {
                if let Some(value) = json(&entry.path()).and_then(|value| epic(&value)) { epic_records.push(value); }
            }
        }
    }
    if let Some(value) = json(&program_data.join("Epic/UnrealEngineLauncher/LauncherInstalled.dat")) {
        if let Some(values) = value.get("InstallationList").and_then(Value::as_array) { epic_records.extend(values.iter().filter_map(epic)); }
    }
    if let Some(found) = select(epic_records, game)? { return finish(found, game).map(Some); }
    if let Some(found) = epic_local(game)? { return finish(found, game).map(Some); }
    if let Some(found) = riot(&program_data, game)? { return finish(found, game).map(Some); }
    for (store, keys, field) in [
        (Store::Ubisoft, ["SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs", "SOFTWARE\\Ubisoft\\Launcher\\Installs"], "InstallDir"),
        (Store::Gog, ["SOFTWARE\\WOW6432Node\\GOG.com\\Games", "SOFTWARE\\GOG.com\\Games"], "path"),
    ] {
        let records = keys.into_iter().flat_map(|key| registry_installs(store, key, field)).collect();
        if let Some(found) = select(records, game)? {
            if store == Store::Gog && client(store).is_none() { continue; } // DRM-free offline installations remain usable.
            return finish(found, game).map(Some);
        }
    }
    if let Some(found) = battle_database(&program_data, game)? { return finish(found, game).map(Some); }
    if let Some(found) = ea(game).or_else(|| uninstall(game)) { return finish(found, game).map(Some); }
    if let Some(found) = rockstar(game)? { return finish(found, game).map(Some); }
    // NTTGame's installed client is named explicitly; arbitrary Launcher.exe files
    // in unrelated installations must never become launch targets.
    if game.file_name().is_some_and(|name| name.eq_ignore_ascii_case("KnightOnLine.exe")) {
        let install = game.parent().ok_or("The game executable has no parent folder.")?;
        let launcher = install.join("Launcher.exe");
        if launcher.is_file() { return finish(Registration { store: Store::Ntt, install: install.into(), args: vec![], client: Some(launcher) }, game).map(Some); }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn epic_identity_is_encoded_from_the_installed_catalogue() {
        let value = serde_json::json!({"InstallLocation":"/games/Game", "AppName":"app-id", "CatalogNamespace":"namespace", "CatalogItemId":"item"});
        assert_eq!(epic(&value).unwrap().args, ["com.epicgames.launcher://apps/namespace%3Aitem%3Aapp-id?action=launch&silent=true"]);
        for app in ["", "app?action=install", "app\n--bad", "../other", "app%3Aother"] {
            let mut bad = value.clone(); bad["AppName"] = app.into(); assert!(epic(&bad).is_none());
        }
        let mut incomplete = value; incomplete["bIsIncompleteInstall"] = true.into(); assert!(epic(&incomplete).is_none());
    }
    #[test] fn riot_install_settings_accept_escaped_paths_but_not_nested_fields() {
        assert_eq!(yaml_install("product_install_full_path: \"D:\\\\Games\\\\Riot\\\\League\"").unwrap(), Path::new("D:\\Games\\Riot\\League"));
        assert_eq!(yaml_install("product_install_full_path: 'D:/Games/League'").unwrap(), Path::new("D:/Games/League"));
        assert!(yaml_install("settings:\n    product_install_full_path: /wrong").is_none());
    }
    #[test] fn ea_launch_uses_all_installer_content_ids_without_xml_execution() {
        assert_eq!(ea_ids("<contentIDs><contentID>123</contentID><contentID>game_eu</contentID><contentID>123</contentID></contentIDs>").as_deref(), Some("123,game_eu"));
        assert!(ea_ids("<contentID>123&amp;bad</contentID>").is_none());
        assert!(ea_ids("<contentID>123&action=install</contentID>").is_none());
        assert_eq!(ea_ids("<contentID>OFB-EAST:109552154</contentID>").as_deref(), Some("OFB-EAST%3A109552154"));
        assert!(ea_ids("<contentID>%3Aother</contentID>").is_none());
    }
    #[test] fn client_arguments_keep_spaced_paths_intact_and_do_not_launch_agent_ids() {
        let install = Path::new("D:/Games/Game with spaces");
        assert_eq!(numeric_args(Store::Ubisoft, "1803", install).unwrap(), ["uplay://launch/1803"]);
        assert_eq!(numeric_args(Store::Gog, "1207658680", install).unwrap(), ["/launchViaAutostart", "/gameId=1207658680", "/command=runGame", "/path=D:/Games/Game with spaces"]);
        assert_eq!(rockstar_registration(install.into()).args, ["-launchTitleInFolder", "D:/Games/Game with spaces"]);
        assert_eq!(battle_registration("prometheus", install.into()).unwrap().args, ["--exec=launch Pro"]);
        assert_eq!(battle_registration("diablo3", install.into()).unwrap().args, ["--exec=launch D3"]);
        for id in ["", "0", "1803/other", "1803?action=install", "18446744073709551616"] { assert!(numeric_args(Store::Ubisoft, id, install).is_none()); }
        // Different branches/classic installations cannot be guessed as the retail game.
        for id in ["wow_classic", "battle.net", "Diablo II", "unknown", "prometheus --bad"] { assert!(battle_registration(id, install.into()).is_none()); }
    }
    #[test] fn ea_requires_the_registered_installs_own_content_identity() {
        let root = fixture("ea"); let install = root.join("Game with spaces");
        let metadata = install.join("__Installer"); std::fs::create_dir_all(&metadata).unwrap();
        assert!(ea_registration(install.clone()).is_none());
        std::fs::write(metadata.join("installerdata.xml"), "<contentIDs><contentID>OFB-EAST:109552154</contentID><contentID>123</contentID></contentIDs>").unwrap();
        let record = ea_registration(install.clone()).unwrap();
        assert_eq!(record.install, install);
        assert_eq!(record.args, ["origin2://game/launch/?offerIds=123,OFB-EAST%3A109552154"]);
        assert!(ea_registration(root.join("Another Game")).is_none());
        std::fs::write(metadata.join("installerdata.xml"), "<contentID>&external;</contentID>").unwrap();
        assert!(ea_registration(install).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    fn fixture(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("arc-store-{label}-{}-{}", std::process::id(), crate::models::now()));
        std::fs::create_dir_all(&root).unwrap(); root
    }
    #[test] fn riot_resolves_each_product_from_its_own_install_not_an_adjacent_game() {
        let root = fixture("riot"); let data = root.join("ProgramData/Riot Games");
        let client = root.join("Client with spaces/RiotClientServices.exe");
        std::fs::create_dir_all(client.parent().unwrap()).unwrap(); std::fs::write(&client, b"fixture").unwrap();
        let mut associated = serde_json::Map::new();
        for (product, binary) in [("league_of_legends", "Game/League of Legends.exe"), ("valorant", "ShooterGame/Binaries/Win64/VALORANT-Win64-Shipping.exe"), ("bacon", "LoR.exe")] {
            let install = root.join("Games").join(product); let game = install.join(binary);
            std::fs::create_dir_all(game.parent().unwrap()).unwrap(); std::fs::write(&game, b"fixture").unwrap();
            associated.insert(path_string(&install), path_string(&client).into());
            let metadata = data.join(format!("Metadata/{product}.live")); std::fs::create_dir_all(&metadata).unwrap();
            std::fs::write(metadata.join(format!("{product}.live.product_settings.yaml")), format!("product_install_full_path: {}\n", serde_json::to_string(&path_string(&install)).unwrap())).unwrap();
        }
        std::fs::write(data.join("RiotClientInstalls.json"), serde_json::to_vec(&serde_json::json!({"associated_client": associated})).unwrap()).unwrap();
        // Stale/malformed records must not prevent a later valid installation from matching.
        let stale = data.join("Metadata/lion.live"); std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("lion.live.product_settings.yaml"), "invalid").unwrap();
        for (product, binary) in [("league_of_legends", "Game/League of Legends.exe"), ("valorant", "ShooterGame/Binaries/Win64/VALORANT-Win64-Shipping.exe"), ("bacon", "LoR.exe")] {
            let game = std::fs::canonicalize(root.join("Games").join(product).join(binary)).unwrap();
            let registration = riot(&root.join("ProgramData"), &game).unwrap().unwrap();
            assert_eq!(registration.args, [format!("--launch-product={product}"), "--launch-patchline=live".into()]);
            let route = finish(registration, &game).unwrap();
            assert_eq!(route.program, std::fs::canonicalize(&client).unwrap());
            assert_eq!(route.install, std::fs::canonicalize(root.join("Games").join(product)).unwrap());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn epic_local_identity_is_only_used_for_the_containing_game() {
        let root = fixture("epic-local"); let install = root.join("Game with spaces");
        let game = install.join("bin/Game.exe"); std::fs::create_dir_all(game.parent().unwrap()).unwrap(); std::fs::write(&game, b"fixture").unwrap();
        let metadata = install.join(".egstore"); std::fs::create_dir_all(&metadata).unwrap();
        std::fs::write(metadata.join("broken.mancpn"), "[]").unwrap();
        std::fs::write(metadata.join("identity.mancpn"), r#"{"AppName":"installed-app","CatalogNamespace":"ns","CatalogItemId":"item"}"#).unwrap();
        let game = std::fs::canonicalize(&game).unwrap();
        assert_eq!(epic_local(&game).unwrap().unwrap().args, ["com.epicgames.launcher://apps/ns%3Aitem%3Ainstalled-app?action=launch&silent=true"]);
        let neighbor = root.join("Game with spaces neighbor/bin/Game.exe"); std::fs::create_dir_all(neighbor.parent().unwrap()).unwrap(); std::fs::write(&neighbor, b"fixture").unwrap();
        assert!(epic_local(&std::fs::canonicalize(neighbor).unwrap()).unwrap().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn matching_requires_a_real_containing_installation_and_rejects_conflicts() {
        let root = std::env::temp_dir().join(format!("arc-store-test-{}-{}", std::process::id(), crate::models::now()));
        let game = root.join("Game/bin/game.exe"); std::fs::create_dir_all(game.parent().unwrap()).unwrap(); std::fs::write(&game, b"game").unwrap();
        let game = std::fs::canonicalize(game).unwrap();
        let registration = Registration { store: Store::Epic, install: root.join("Game"), args: vec!["route-one".into()], client: None };
        assert!(select(vec![registration.clone()], &game).unwrap().is_some());
        let mut other = registration.clone(); other.install = root.join("Game-neighbor");
        assert!(select(vec![other], &game).unwrap().is_none());
        let mut conflict = registration.clone(); conflict.args = vec!["route-two".into()];
        assert!(select(vec![registration, conflict], &game).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
