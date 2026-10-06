"""Execute the real migration and verify persistence invariants without a Rust toolchain."""
import sqlite3
from pathlib import Path

migrations = Path(__file__).resolve().parents[1] / 'src-tauri/migrations'
schema = (migrations / '001_initial.sql').read_text()
db = sqlite3.connect(':memory:')
db.executescript(schema)
db.executescript(schema)
db.execute('INSERT INTO games(title,exe_path,folder,added_at) VALUES (?,?,?,?)', ('Game', r'D:\Games\Game\game.exe', r'D:\Games\Game', 1))
try:
    db.execute('INSERT INTO games(title,exe_path,folder,added_at) VALUES (?,?,?,?)', ('Duplicate', r'd:\games\game\GAME.EXE', r'D:\Games\Game', 2))
except sqlite3.IntegrityError:
    pass
else:
    raise AssertionError('Executable identity must be case insensitive')
db.execute('INSERT INTO sessions(game_id,started_at) VALUES (1,10)')
db.commit()
# Upgrade the existing v1 library as the production initializer does, then
# leave v2 untouched on subsequent startup rather than repeating ALTER TABLE.
db.executescript((migrations / '002_console_launch.sql').read_text())
assert db.execute('SELECT title,console_launch FROM games WHERE id=1').fetchone() == ('Game', None)
assert db.execute('PRAGMA user_version').fetchone()[0] == 2
db.executescript((migrations / '003_ratings.sql').read_text())
db.execute("INSERT INTO game_ratings(game_id,title,platform,value) VALUES (1,'Game','pc','{}')")
db.execute('DELETE FROM games WHERE id=1')
assert db.execute('SELECT count(*) FROM sessions').fetchone()[0] == 0
assert db.execute('SELECT count(*) FROM game_ratings').fetchone()[0] == 0
assert db.execute('PRAGMA user_version').fetchone()[0] == 3
print('PASS: repeatable migration, case-insensitive EXE identity, session cascade, schema version.')
