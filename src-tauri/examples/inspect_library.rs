//! Read-only scanner diagnostics: cargo run --example inspect_library -- D:\Games
#[allow(dead_code)]
#[path = "../src/models.rs"] mod models;
#[allow(dead_code)]
#[path = "../src/metadata.rs"] mod metadata;
#[allow(dead_code)]
#[path = "../src/scanner.rs"] mod scanner;
#[allow(dead_code)]
#[path = "../src/ps4.rs"] mod ps4;
#[allow(dead_code)]
#[path = "../src/launch.rs"] mod launch;
#[allow(dead_code)]
#[path = "../src/secrets.rs"] mod secrets;
#[allow(dead_code)]
#[path = "../src/db.rs"] mod db;
#[allow(dead_code)]
#[path = "../src/artwork.rs"] mod artwork;

fn main() {
    let mut roots: Vec<String> = std::env::args().skip(1).collect();
    if roots.first().is_some_and(|arg| arg == "--plan") && roots.len() == 3 {
        let connection = rusqlite::Connection::open_with_flags(&roots[1], rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let game = db::game(&connection, roots[2].parse().expect("Pass a library game ID")).unwrap();
        match launch::plan(&game) {
            Ok(plan) => println!("{}", serde_json::json!({"title": game.title, "program": models::path_string(&plan.program), "args": plan.args, "directory": models::path_string(&plan.directory)})),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); },
        }
        return;
    }
    if roots.first().is_some_and(|arg| arg == "--search") && roots.len() >= 3 {
        let connection = rusqlite::Connection::open_with_flags(&roots[1], rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let client = artwork::ArtClient::new(db::settings(&connection).unwrap().api_key).unwrap();
        tauri::async_runtime::block_on(async {
            for query in &roots[2..] {
                match client.search(query).await {
                    Ok(matches) => for found in matches {
                        let grids = client.artworks(found.id, "grid").await;
                        let heroes = client.artworks(found.id, "hero").await;
                        println!("{}", serde_json::json!({"query": query, "game": found, "grids": grids.ok(), "heroes": heroes.ok()}));
                    },
                    Err(error) => println!("{}", serde_json::json!({"query": query, "error": error})),
                }
            }
        });
        return;
    }
    let artwork_database = if roots.first().is_some_and(|arg| arg == "--artwork") && roots.len() > 1 { roots.remove(0); Some(roots.remove(0)) } else { None };
    if roots.is_empty() { eprintln!("Pass one or more game-library folders to inspect."); std::process::exit(1); }
    let (games, ignored, warnings) = scanner::scan(&roots);
    if let Some(database) = artwork_database {
        // No database mutations or downloads. The encrypted API key stays in memory.
        let connection = rusqlite::Connection::open_with_flags(database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let installed = db::games(&connection).unwrap();
        let client = artwork::ArtClient::new(db::settings(&connection).unwrap().api_key).unwrap();
        tauri::async_runtime::block_on(async {
            for game in &games {
                if installed.iter().any(|old| old.folder.eq_ignore_ascii_case(&game.folder) && old.cover.as_ref().is_some_and(|path| std::path::Path::new(path).is_file())) { continue; }
                match client.resolve(&game.title, &game.folder).await {
                    Ok(Some(found)) => {
                        let grids = client.artworks(found.id, "grid").await;
                        println!("{}", serde_json::json!({ "title": game.title, "match": found.name, "matchId": found.id, "covers": grids.as_ref().map(|items| items.len()).ok(), "error": grids.err() }));
                    },
                    Ok(None) => println!("{}", serde_json::json!({ "title": game.title, "match": null })),
                    Err(error) => println!("{}", serde_json::json!({ "title": game.title, "error": error })),
                }
            }
        });
    } else {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "games": games, "ignored": ignored, "warnings": warnings })).unwrap());
    }
}
