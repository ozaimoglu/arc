PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    exe_path TEXT NOT NULL COLLATE NOCASE UNIQUE,
    folder TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'Local game',
    genre TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    cover TEXT,
    hero TEXT,
    logo TEXT,
    sgdb_id INTEGER,
    favorite INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0,
    available INTEGER NOT NULL DEFAULT 1,
    last_played INTEGER,
    playtime INTEGER NOT NULL DEFAULT 0,
    score INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL,
    edited INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS games_visible ON games(hidden, last_played DESC);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS ignored_executables (exe_path TEXT PRIMARY KEY COLLATE NOCASE);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY,
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at INTEGER
);
PRAGMA user_version = 1;
