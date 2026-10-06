//! JSON-lines diagnostics using the same provider, validation and SQLite code as Arc.
//! Pass a database path explicitly. No game is launched and no credentials are read.
#[allow(dead_code)] #[path = "../src/models.rs"] mod models;
#[allow(dead_code)] #[path = "../src/metadata.rs"] mod metadata;
#[allow(dead_code)] #[path = "../src/scanner.rs"] mod scanner;
#[allow(dead_code)] #[path = "../src/ps4.rs"] mod ps4;
#[allow(dead_code)] #[path = "../src/secrets.rs"] mod secrets;
#[allow(dead_code)] #[path = "../src/db.rs"] mod db;
#[allow(dead_code)] #[path = "../src/ratings.rs"] mod ratings;
#[allow(dead_code)] #[path = "../src/artwork.rs"] mod artwork;
use std::{io::{BufRead, Write}, path::PathBuf};
use serde_json::{json, Value};

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("Pass a database path"));
    let mut db = db::open(&path).expect("Open database"); db::migrate(&mut db).expect("Migrate database");
    for line in std::io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line.expect("Read JSON line")).expect("Parse JSON line");
        let result: Result<Value, String> = (|| {
            let string = |key| input.get(key).and_then(Value::as_str).ok_or_else(|| format!("Missing {key}"));
            match string("op")? {
                "request" => tauri::async_runtime::block_on(async { serde_json::to_value(ratings::request(string("url")?).await?).map_err(|error| error.to_string()) }),
                "cacheRead" => serde_json::to_value(ratings::cache_read(&path, string("key")?)?).map_err(|error| error.to_string()),
                "cacheWrite" => { ratings::cache_write(&path, serde_json::from_value(input["values"].clone()).map_err(|error| error.to_string())?)?; Ok(Value::Null) },
                "snapshot" => serde_json::to_value(db::games(&db)?).map_err(|error| error.to_string()),
                "localAppId" => Ok(json!(ratings::local_app_id(&db::game(&db, input["gameId"].as_i64().ok_or("Missing game ID")?)?))),
                "catalogueTitle" => {
                    let game = db::game(&db, input["gameId"].as_i64().ok_or("Missing game ID")?)?;
                    let client = artwork::ArtClient::new(db::settings(&db)?.api_key)?;
                    tauri::async_runtime::block_on(async { Ok(Value::String(client.game(game.sgdb_id.ok_or("Missing catalogue ID")?).await?.name)) })
                },
                "save" => { ratings::save(&db, input["gameId"].as_i64().ok_or("Missing game ID")?, string("title")?, string("platform")?, &serde_json::from_value(input["value"].clone()).map_err(|error| error.to_string())?)?; Ok(Value::Null) },
                _ => Err("Unsupported operation".into()),
            }
        })();
        let output = match result { Ok(value) => json!({"id": input["id"], "value": value}), Err(error) => json!({"id": input["id"], "error": error}) };
        println!("{output}"); std::io::stdout().flush().expect("Flush response");
    }
}
