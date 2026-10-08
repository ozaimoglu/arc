# Shader preparation with SCSKiller

Arc development builds after v0.1.8 can analyze and compile shaders through the **SCSKiller CLI**. Arc 0.1.13 supports official SCSKiller **1.2.4**, older 1.2.3 installations and the optional **1.2.4-arc.2 compatibility CLI**. SCSKiller remains an external installation; its binaries are not bundled in Arc's installer. The separately licensed patch and reproducible build instructions live in [tools/scskiller-compat](../tools/scskiller-compat/README.md).

## Connect

1. Download an official Windows installer or portable archive from [SCSKiller releases](https://github.com/BlueHeisenberg/SCSKiller/releases).
2. In Arc's **Settings → SCSKiller**, choose `SCSKiller.exe` or `cli/scskiller.exe` from that installation, then save.
3. Open a native game's details and choose **Analyze game**.
4. If the game is ready, choose **Compile shaders**. Keep the game closed while it runs.

Arc shows the actual engine, graphics API, support reason, preparation time/driver when known, and live operation stages. **Stop** asks SCSKiller's queue to stop gracefully, allowing in-flight compiles to finish. Operation state belongs to Rust and survives frontend navigation. The current game cannot be launched through Arc during its preparation.

SCSKiller targets NVIDIA/AMD DirectX 12 games and NVIDIA DirectX 11 games. A game needing a recording stays disabled for compilation until SCSKiller has usable pipeline state. shadPS4 games use the emulator's cache and are excluded from this integration. Preparing shaders does not guarantee that traversal, streaming or other non-shader stutters disappear.

## Recording preparation

With the compatibility CLI selected, eligible DirectX 12 games offer **Prepare recording**. Arc explains that this confirms the game's own folder and adds a recorder DLL beside its executable. After confirmation, the external CLI validates the exact identity and folder through SCSKiller's public API, then installs its recorder. Anti-cheat, existing DLLs, mod conflicts and running-process checks remain enforced. Arc shows success only after a scan verifies that the recorder is installed.

Play for at least five minutes in the game world, close the game completely, and choose **Analyze again**. The panel shows recording size and upstream's capture status. Menus alone may not capture enough real pipelines. The recorder can be removed through SCSKiller; installing it does not itself compile shaders or guarantee complete coverage. Setup has no Stop control, and Arc waits for setup to finish before closing.

## Process and data boundary

Rust runs fixed `scan`, `queue`, `arc-record prepare`, `cache get`, `cache set` and `cache clear` commands. Arguments are passed separately; the elevated driver-setting action uses the Windows shell's `runas` verb with validated size tokens and a generated result-file path. The frontend supplies an Arc database game ID and typed actions. Operation identities come from SCSKiller's local game list and are validated; titles and Steam rating IDs never select an installation. Explicit analysis uses `scan --rescan`; compile/setup completion uses a normal scan to refresh actual state.

Arc reads the bounded `games.json` snapshot and matches canonical executable paths. Installed and Arc compatibility CLIs use `%LOCALAPPDATA%\SCSKiller`. Official 1.2.4 portable packages use their own `data` folder after upstream completes its atomic migration; Arc follows the selected package's `.portable` and `data\migrated` markers, falling back to the shared store until then. Status reads, manual imports and recording folder checks use the same resolved store. Store discovery runs first. If the selected executable is absent, Arc adds only that game to `manual-games.json`, preserving existing entries and unknown fields. This update uses SCSKiller's cross-process named mutex, a backup and an atomic replacement. Newly imported roots are **unconfirmed**, so SCSKiller's recorder is not enabled by the import.

For nested `bin/x64*`, Unreal `Binaries/Win64` and sibling `Retail`/`Runtime` layouts, Arc passes the engine's installation root instead of an outer delivery folder. Re-analysis also repairs an older unconfirmed root when the detected root is inside it. Confirmed folders, personal names and unknown metadata remain unchanged during analysis. Recording preparation explicitly confirms an eligible manual game's own folder through SCSKiller.

Arc does not invoke offline anti-cheat launches or scheduled-task commands. Recording setup changes only the selected game's folder confirmation and recorder override. Existing SCSKiller global preferences and accounts remain owned by SCSKiller. Its CLI performs shader/driver compatibility checks, cache attribution and cache warming. Arc drains stdout/stderr concurrently and retains only the latest 100 output lines.

One operation runs at a time. Analysis has a ten-minute timeout per scan pass. Compilation uses the CLI queue's graceful `stop`/`quit` protocol instead of forcibly killing a warmer. Closing Arc requests the same stop. Driver or game updates are re-evaluated by the next SCSKiller scan/compile rather than inferred from an old successful exit code.

The compatibility CLI treats explicit `scan --rescan` as a user-requested operation,
matching upstream's manual refresh mode. It reports the game being analyzed and
avoids background I/O starvation. Normal status-refresh scans retain upstream's
background I/O policy.

## Cache controls

**Settings → Shader cache** reads the actual GPU driver limit and reported disk usage. NVIDIA cache limits can be set to **Driver default, 1, 5, 10, 20, 50, 100 GB or Unlimited**. Changes apply immediately to the global driver setting for all games, independently of Save changes. Windows requests administrator approval. Arc checks the CLI's result and reads the driver setting again before reporting success. AMD and other GPUs show the measured values without enabling unsupported size changes. SCSKiller's NVIDIA disk reading is an upper bound, which Arc labels **Up to**.

In a native game's details, **Clear cache** asks for confirmation before invoking per-game cleanup. By default it removes only driver and Windows shader-cache files that SCSKiller attributes to the matched executable. The optional checkbox also includes detected generated Unreal user pipeline and shader precache files; shipped shader libraries and saved games are preserved. SCSKiller can refuse cleanup if cache is shared with another game, the game is running or files are in use. Unattributed files may remain.

Cleanup remains available for analyzed games even when their shader format cannot be compiled. It has no Stop control: deletion is allowed to finish, followed by a scan to refresh the preparation state. Arc blocks launching that game during cleanup. Cache-size changes are blocked while a shader operation or an Arc-launched game is running; new launches are blocked while the driver setting is being changed. shadPS4 cache management stays in the emulator.

## Why compilation may be unavailable

- **Not analyzed:** choose Analyze game first; this is not a failed compile.
- **Folder confirmation needed / Recording needed:** use Prepare recording with the compatibility CLI, or set it up in official SCSKiller. Capture actual gameplay, then close the game and analyze again.
- **Encrypted or packed shaders / unsupported engine:** the installed SCSKiller reader cannot prepare this build. Recording alone is not a universal fix.
- **Different store executable:** store discovery can replace a manual entry with the store's executable. If Arc uses another program in the same installation, exact matching remains disabled; review Game properties and the SCSKiller entry instead of compiling a different executable silently.
- **Ready, then operation fails:** detection is a compatibility check, not a full shader-index test. An upstream reader or GPU compiler can still fail; the operation output contains the actual error.
- **Partly compiled / Partly warmed:** upstream reports that the driver rejected over 10% of the plan, or the first launch still compiled over 20% of its pipeline creates. Arc keeps these distinct from fully prepared shaders; unknown historical counts do not imply a partial failure.
- **Compile did not help:** the recorded launch did not use the prepared cache. Review the game's loader requirements in SCSKiller.
- **No shader stutter:** upstream's verdict says preparation is unnecessary. Arc shows that reason and does not offer compilation or recorder setup.
- **Offline session needed:** handle the offline workflow in SCSKiller. Arc does not install a recorder while anti-cheat is present or start an offline launch.

## Validation

Automated tests cover exact installation matching, duplicate/unsafe identities, unknown and recording-required states, backward-compatible preferences, bounded/corrupt JSON, metadata preservation, progress parsing, global-job recovery after navigation and UI gating.

An opt-in Rust smoke test exercises the same integration against an official installation. It analyzes the chosen native game without launching it. Set `ARC_SHADER_SMOKE_TOOL`, `ARC_SHADER_SMOKE_DB` and `ARC_SHADER_SMOKE_GAME_ID`, then run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked shaders::tests::official_cli_smoke -- --ignored --nocapture
```

Set `ARC_SHADER_SMOKE_COMPILE=1` only when you also want the test to populate the GPU driver's cache. This test is excluded from normal CI.

Set `ARC_SHADER_SMOKE_RECORDING=1` only for an eligible game when you also want the test to confirm its folder and install/verify the recorder through Arc's adapter. This writes to that game's folder; it does not launch the game or capture gameplay. The compatibility patch has a separate Windows CI workflow that builds the pinned source and runs synthetic regressions without local games.

## Upstream

[SCSKiller](https://github.com/BlueHeisenberg/SCSKiller) is independently maintained and licensed under GPL-3.0-or-later with its published additional permission. Arc calls the installed program as a separate process. The optional compatibility patch is separately licensed under those upstream terms; its source is not linked into Arc. Consult upstream's [architecture](https://github.com/BlueHeisenberg/SCSKiller/blob/main/ARCHITECTURE.md), [CLI source](https://github.com/BlueHeisenberg/SCSKiller/blob/main/src/SCSKiller.Cli/Program.cs) and [license](https://github.com/BlueHeisenberg/SCSKiller/blob/main/LICENSE).
