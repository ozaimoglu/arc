# Arc v0.1 architecture

Arc is a Windows game library built with Tauri 2.12.1, Rust, React, TypeScript, Vite and SQLite. It uses the system WebView2 runtime. The MSI/NSIS installers use the Evergreen bootstrapper when WebView2 is missing; Chromium is not bundled.

## Boundaries

- React owns navigation, filtering, artwork selection and presentation.
- Rust owns directory traversal, EXE metadata, persistence, credentials, artwork downloads and process launch.
- The frontend passes a database game ID to launch/open-folder commands. It cannot submit a shell command or an arbitrary executable path.
- SQLite is the source of truth. Normal startup reads the database. A detector version upgrade triggers one background rescan of configured folders, then records the version to avoid repeated startup traversal.
- The browser build is an explicitly labeled preview with eight sample games and localStorage persistence. Desktop starts with an empty SQLite library. Browser scan/launch/artwork operations never pretend to execute native actions.

## Modules

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/scanner.rs` | `walkdir`, depth cap, no symlink following, auxiliary EXE blacklist, confidence score, best candidate per game directory |
| `metadata.rs` | Read Windows ProductName/FileDescription without loading the executable |
| `ps4.rs` | Bounded PS4 SFO reader, base-game/serial detection and existing shadPS4/BBLauncher discovery |
| `launch.rs` | Validate native or console launch records and build program/argument/working-directory plans |
| `db.rs` and `migrations/001_initial.sql` | Library, preferences, ignored executables and process sessions |
| `artwork.rs` | SteamGridDB search, conservative match ranking, grid/hero/logo selection and bounded image cache |
| `ratings.rs` | Provider URL/redirect allowlists, bounded HTTP responses, typed ratings validation, local Steam identity and isolated SQLite rating storage |
| `shaders.rs` | Optional official SCSKiller CLI, exact executable matching, manual metadata handoff, bounded job output and graceful queue control |
| `shader_cache.rs` | Actual driver usage/limit queries, elevated NVIDIA size changes and per-game cache cleanup through the official CLI |
| `src/ratings/` | Bundled CriticPeek 1.5.4 matching/parsers/queues adapted to Rust transport and PC/PS4 platforms |
| `secrets.rs` | Windows DPAPI credential encryption for the current Windows user |
| `watcher.rs` | `notify` Windows filesystem watcher, coalesce change bursts and refresh persisted library |
| `lib.rs` | Constrained IPC commands and direct process launch |
| `src/bridge.ts` | Desktop IPC / browser preview boundary |

## Data model

`games` stores canonical game-file identity (case insensitive), title, install folder, artwork paths, SteamGridDB ID, favorite/hidden/availability flags, confidence and timestamps. The legacy `exe_path` field identifies a native EXE or PS4 `eboot.bin`. Schema version 2 adds nullable `console_launch` JSON with a constrained shadPS4/BBLauncher mode, emulator/launcher paths and CUSA serial. Native records retain `NULL`. Migration runs once transactionally and preserves the existing v1 library. A title edit sets `edited`, preventing a rescan from overwriting it. All scans preserve favorites, hidden state, descriptions and artwork.

Removing a library entry adds the executable to `ignored_executables` in the same transaction. Later scans cannot re-add it. Hiding preserves the game and makes it available in Hidden games. No game files are deleted.

`sessions` stores the start/end of the directly launched process. On clean process exit, elapsed whole minutes are added to playtime. Session and last-played writes are transactional. Pending sessions from a previous launcher crash are closed without estimating playtime. Playtime is basic process tracking: launcher handoffs, descendants and closing Arc while a game runs are not tracked reliably in v1.

The library snapshot includes `runningGameIds` from the in-memory launch guard, covering both pending starts and directly launched processes. Library-change events publish reservations, launch failures and process exits. Play stays disabled across the hero, details, cards, list and context menu until the tracked process exits or its start fails; a failed library refresh after a successful start retains the disabled state. Running status is transient and is not stored as an installation flag or restored from stale sessions.

Preferences are serialized to SQLite. The personal API key is excluded from preferences JSON and stored as a DPAPI encrypted BLOB. An unreadable credential falls back to an empty key so the offline library still opens. API key entry is disabled in the browser preview.

## Detection

Blacklist matches reject setup/uninstaller/redist/crash/anti-cheat/helper/launcher executables before reading metadata. The remaining score starts at 10, adds 40 for files at least 20 MiB (10 for 1–20 MiB), 30 for a game folder, 25 for a meaningful ProductName, 15 for a matching description and 20 for a filename/title match. Sub-MiB executables lose 30; Shipping binaries gain 10. Threshold: 60.

Local Steam manifests add 50. Verified game payloads add 70, allowing small real game EXEs without lowering the general threshold: Unity requires both the EXE-specific `_Data` directory and `UnityPlayer.dll`; a Java launcher requires its EXE-specific JSON entry point, a substantial local classpath JAR, bundled Java runtime and Steam API DLL; packed native resources require substantial game archives plus a Steam API DLL; id Tech player executables require the corresponding `pak0` asset archive. Java runtime directories, options utilities and security-loader metadata are excluded. Build suffixes such as `v52 FIXED` are removed while preserving sequel numbers.

The first meaningful directory under a configured library root identifies an installation; generic `bin/x64/Game/Binaries` and Steam/Riot/Bethesda library-container components are skipped. The best candidate wins per installation; loose EXEs at the configured root retain separate identities. A verified payload also recognizes an exact game folder selected as a scan root. Metadata matching cannot by itself turn a blacklisted helper into a game. Artwork service availability never determines whether a local game is imported.

This is a heuristic, not a universal detector. Deep publisher collections, games with generic Unity metadata and unusual launcher-only titles may need adjustment. General emulators, ROM libraries and store imports remain deferred; the specific shadPS4 integration below is supported.

## PS4 / shadPS4

Use the ordinary game-folder settings to add extracted PS4 installations. During the same traversal, an `eboot.bin` paired with `sce_sys/param.sfo` is recognized by its base-application `gd` category, nonempty UTF-8 title and CUSA serial. Metadata reads are limited to 128 KiB and offsets/lengths are bounds-checked. Patch (`gp`), DLC (`ac`) and save (`sd`) records are not imported; duplicate base copies and overlapping configured roots are consolidated by serial. Emulator EXEs are excluded from native detection.

An existing `shadPS4.exe` must be discoverable within eight ancestors of the game folder. Ordinary console launch uses its directory as the working directory and separate `-g` / canonical game-file arguments, following the [shadPS4 0.19 CLI](https://github.com/shadps4-emu/shadPS4/blob/v.0.19.0/src/main.cpp). This supports games beneath a shadPS4 installation; arbitrary external emulator paths and packaged `.pkg` installation are not configured by Arc.

For Bloodborne, a matching `Bloodborne/Launcher/BB_Launcher.exe` and `BBLauncher/LauncherSettings.toml` take priority when the configured installation equals the detected base folder and its emulator exists. This preserves the existing DLSS/mod/save-backup launch route using the [documented `-n` argument](https://github.com/rainmakerv3/BB_Launcher/blob/Release16.11/README.md). Arc reads that configuration and does not edit emulator settings, patches, saves or game files. The launch plan validates the current game serial, emulator and matching BBLauncher configuration before starting a process. Missing or changed components produce an unavailable state or a scan-again error. Session timing still tracks the directly started process, including BBLauncher, rather than following arbitrary child processes.

PS4 records use the existing artwork, favorites, hide/restore, removal exclusions and detail workflows. The details/properties screens show their platform/serial and their `eboot.bin` game file. Folder watching recognizes base-game and SFO changes. Installed-game and emulator availability are checked during scanning; normal startup loads the stored library.

## Game ratings

Arc 0.1.5 reuses the maintainer's CriticPeek extension's `core.js` / `service.js` in the bundled frontend. Chrome is not needed. JavaScript performs title normalization, verified source parsing, queueing and cache expiry; Rust owns network access and SQLite. Downloaded HTML is parsed as strings, never inserted into the page or executed. CSP still permits no direct frontend network requests.

Three readings remain distinct: Metacritic critics /100, Metacritic players /10, and Steam positive review percentage. The Steam definition matches CriticPeek: all languages, all purchase types and off-topic activity included, following the [official review API](https://partner.steamgames.com/doc/store/getreviews). Missing readings are null, never fabricated zeros. PC entries use PC-specific cards/overviews; shadPS4 entries use PlayStation 4 cards/overviews and never inherit PC Steam ratings or caches.

Title matching rejects mismatched sequels, remakes, DLC and ambiguous exact matches. Complete/GOTY retail edition aliases are supported, with exact names preferred. For installed native games, an unambiguous numeric Steam App ID can be read from a small allowlist of existing SDK/config identity files beside the executable or in the game root, including `steam_appid.txt`. Reads stay within the canonical game folder and are capped at 64 KiB. Those files are not modified or uploaded; only the ID is sent to Steam, whose embedded app identity is verified. A linked artwork catalogue can supply a canonical name when title lookup fails; this preserves release identity without blindly dropping “remake/remastered”. Users can correct a match with a Steam URL/App ID in the detail view. The verified ID persists with its ratings and is reused on refresh.

Schema 3 adds `ratings_cache` (CriticPeek's source/title aliases) and `game_ratings` (one typed payload per game, bound to current title and PC/PS4 platform). Rename/platform changes stop displaying old matches; game removal cascades its rating record. The original game fields, detector confidence, preferences, encrypted artwork key, exclusions and play history are untouched. Rating records are read with the startup snapshot. Due/missing ratings load sequentially in the background, with detail games prioritized between jobs. Hidden games are excluded. Scores have a seven-day TTL, empty matches one hour, failures without readings one minute. A failed refresh keeps verified prior readings with their original age; manual refresh remains available.

Rust accepts only HTTPS Steam search/details/review endpoints and Metacritic game/review paths. Credentials, custom ports, unrelated hosts/paths and unsafe redirects are rejected. Provider replies are limited to 8 MiB with a 20-second timeout. 403/429 responses are not bypassed; the reused service honors cooldowns and reports partial availability. Rating storage commands only access dedicated `score:v2:` keys/tables, not preferences or secrets. The only data providers are Steam and Metacritic; an optional canonical-title fallback uses the already configured SteamGridDB credential in Rust. No extension profile, developer backend, analytics or account is required.

## Shader preparation

The optional SCSKiller integration runs its official CLI as a separate process. Rust owns the single active job, fixed commands, typed actions and validated installation identity. The frontend presents support reasons and polls the authoritative state, retaining active jobs across navigation. SCSKiller's local snapshot supplies engine/driver/cache status; Arc does not infer successful warming from an installer or process launch. Manual metadata imports preserve the external list under its named mutex and leave new roots unconfirmed for recording. See [process, data and validation boundaries](SCSKILLER.md).

## Artwork files

SteamGridDB requests run in Rust with a personal API key. Automatic matching requires similarity at least 0.92 and rejects close alternatives. An exact catalogue title takes priority, with the cleaned search title as a secondary tie-breaker for distribution suffixes such as Complete Edition; sequel-number and remake-year checks still apply. The artwork dialog supports manual game search and selection. Grid requests prefer static 600×900 covers, with other standard portrait dimensions as a fallback. Hero and logo are separately selectable. Local PNG/JPEG/WebP import works without a key.

Files live in Tauri's app local data directory, normally `%LOCALAPPDATA%\com.arc.launcher\`:

```text
db.sqlite
covers/game-<id>-<revision>.webp
heroes/game-<id>-<revision>.jpg
logos/game-<id>-<revision>.png
```

Each download is capped at 20 MiB; the URL must use HTTPS on SteamGridDB's CDN. Redirects are disabled and credentials are never forwarded to the CDN. Image formats are checked by signature. Write to a temporary file, then rename before updating the database. New revision paths avoid WebView stale-cache issues. Old revisions remain on disk in v1; cache pruning is deferred.

The asset protocol is scoped to the three cache directories. Artwork HTTP failures produce a visible warning while the local library stays usable.

## Watching and concurrency

Only explicit scans or filesystem create/remove/rename events trigger traversal. `notify` uses Windows's native watcher backend. Events are debounced for 1.5 seconds, with a 10-second burst ceiling. Paired rename events update executable/folder identities and removal exclusions transactionally, preserving game IDs, artwork and preferences. Unpaired rename notifications fall back to detection during the rescan. v1 refreshes configured roots as a batch, rather than implementing an incremental file index. Watcher refreshes do not request network artwork; a manual scan fills missing covers. A scan mutex serializes filesystem/SQLite updates; local results publish before network requests.

## Performance targets and validation

The requested <1s cold startup and 50–150 MiB idle RAM are targets, not measured claims. Measure the packaged app on representative hardware and real libraries. No performance benchmark has been run in this workspace.

Rust unit tests cover EXE rejection, metadata scoring, nested install folders, case-insensitive identities, removal persistence and preservation of edits. Frontend tests cover visibility/search/filter/sort combinations. `scripts/check-schema.py` independently executes the production migration. Windows CI runs Rust tests, Clippy, frontend tests and installer packaging.

## Reference documentation

- [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri Windows installers](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Rust commands](https://v2.tauri.app/develop/calling-rust/)
- [Tauri 2.12.1](https://docs.rs/tauri/2.12.1/tauri/)
- [SteamGridDB API v2](https://www.steamgriddb.com/api/v2)
- [Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
