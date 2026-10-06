CREATE TABLE ratings_cache (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE game_ratings (
    game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    platform TEXT NOT NULL CHECK(platform IN ('pc', 'playstation-4')),
    value TEXT NOT NULL
);
PRAGMA user_version = 3;
