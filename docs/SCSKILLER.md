# Shader preparation with SCSKiller

Arc development builds after v0.1.8 can analyze and compile shaders through the **official SCSKiller CLI**. The integration is tested against SCSKiller 1.2.3. SCSKiller is an optional external installation; its binaries and source are not bundled in Arc.

## Connect

1. Download an official Windows installer or portable archive from [SCSKiller releases](https://github.com/BlueHeisenberg/SCSKiller/releases).
2. In Arc's **Settings → SCSKiller**, choose `SCSKiller.exe` or `cli/scskiller.exe` from that installation, then save.
3. Open a native game's details and choose **Analyze game**.
4. If the game is ready, choose **Compile shaders**. Keep the game closed while it runs.

Arc shows the actual engine, graphics API, support reason, preparation time/driver when known, and live operation stages. **Stop** asks SCSKiller's queue to stop gracefully, allowing in-flight compiles to finish. Operation state belongs to Rust and survives frontend navigation. The current game cannot be launched through Arc during its preparation.

SCSKiller targets NVIDIA/AMD DirectX 12 games and NVIDIA DirectX 11 games. A game that needs a recording remains disabled for compilation until the recording is provided through SCSKiller. shadPS4 games use the emulator's cache and are excluded from this integration. Preparing shaders does not guarantee that traversal, streaming or other non-shader stutters disappear.

## Process and data boundary

Rust runs only the fixed `scan` and `queue` commands with separate process arguments. The frontend supplies an Arc database game ID and an `analyze`/`compile` enum. Compile queue identities come from SCSKiller's local game list and are validated before being sent to stdin; titles and Steam rating IDs never select an installation.

Arc reads the bounded `%LOCALAPPDATA%\SCSKiller\games.json` snapshot and matches canonical executable paths. Store discovery runs first. If the selected executable is absent, Arc adds only that game to `manual-games.json`, preserving existing entries and unknown fields. This update uses SCSKiller's cross-process named mutex, a backup and an atomic replacement. Newly imported roots are **unconfirmed**, so SCSKiller's recorder is not enabled by the import.

For nested `bin/x64*` and Unreal `Binaries/Win64` layouts, Arc passes the engine's installation root instead of an outer delivery folder. Re-analysis also repairs an older unconfirmed root when the detected root is inside it. Confirmed folders, personal names and unknown metadata remain unchanged. A game awaiting folder confirmation displays **Folder confirmation needed**, with instructions for recording in SCSKiller.

Arc does not invoke recorder installation, offline anti-cheat launches, cache deletion, driver-setting changes or scheduled-task commands. Existing SCSKiller preferences and accounts remain owned by SCSKiller. Its CLI performs its own shader/driver compatibility checks and cache warming. Arc drains stdout/stderr concurrently and retains only the latest 100 output lines.

One operation runs at a time. Analysis has a ten-minute timeout per scan pass. Compilation uses the CLI queue's graceful `stop`/`quit` protocol instead of forcibly killing a warmer. Closing Arc requests the same stop. Driver or game updates are re-evaluated by the next SCSKiller scan/compile rather than inferred from an old successful exit code.

## Why compilation may be unavailable

- **Not analyzed:** choose Analyze game first; this is not a failed compile.
- **Folder confirmation needed / Recording needed:** confirm the game's own folder and create a recording in SCSKiller, then analyze again. Arc does not change recording preferences automatically.
- **Encrypted or packed shaders / unsupported engine:** the installed SCSKiller reader cannot prepare this build. Recording alone is not a universal fix.
- **Different store executable:** store discovery can replace a manual entry with the store's executable. If Arc uses another program in the same installation, exact matching remains disabled; review Game properties and the SCSKiller entry instead of compiling a different executable silently.
- **Ready, then operation fails:** detection is a compatibility check, not a full shader-index test. An upstream reader or GPU compiler can still fail; the operation output contains the actual error.

## Validation

Automated tests cover exact installation matching, duplicate/unsafe identities, unknown and recording-required states, backward-compatible preferences, bounded/corrupt JSON, metadata preservation, progress parsing, global-job recovery after navigation and UI gating.

An opt-in Rust smoke test exercises the same integration against an official installation. It analyzes the chosen native game without launching it. Set `ARC_SHADER_SMOKE_TOOL`, `ARC_SHADER_SMOKE_DB` and `ARC_SHADER_SMOKE_GAME_ID`, then run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked shaders::tests::official_cli_smoke -- --ignored --nocapture
```

Set `ARC_SHADER_SMOKE_COMPILE=1` only when you also want the test to populate the GPU driver's cache. This test is excluded from normal CI.

## Upstream

[SCSKiller](https://github.com/BlueHeisenberg/SCSKiller) is independently maintained and licensed under GPL-3.0-or-later with its published additional permission. Arc calls the installed program as a separate process and contains no copied SCSKiller implementation. Consult upstream's [architecture](https://github.com/BlueHeisenberg/SCSKiller/blob/main/ARCHITECTURE.md), [CLI source](https://github.com/BlueHeisenberg/SCSKiller/blob/main/src/SCSKiller.Cli/Program.cs) and [license](https://github.com/BlueHeisenberg/SCSKiller/blob/main/LICENSE).
