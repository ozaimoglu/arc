# Arc's optional SCSKiller compatibility CLI

This is a maintained patch for SCSKiller **1.2.4**, based on commit
`7087ff653b0c799d4d9fe26a76e2e26767c0db3b`. It builds a separate
**1.2.4-arc.2** CLI. It is not an official SCSKiller release or part of Arc's installer.

The patch:

- Recognizes `pipeline` paths alongside shader/PSO paths, so raw shader containers
  inside pipeline-cache files receive the larger detection sample and full indexing.
- When DXIL shaders already identify DirectX 12, avoids opening unrelated executable
  and DLL import tables. The graphics API result is unchanged; older DXBC-only games
  with a usable shader library still use import-based detection. When the carved
  reader cannot use a game's shaders, it leaves an unverified API as `Unknown`
  instead of inspecting extra DLLs just for that label. The unsupported reason and
  compilation/recording restrictions remain enforced.
- Streams diagnostic index JSON to disk instead of constructing a potentially
  multi-gigabyte JSON string in memory.
- Uses upstream's user-requested scan mode for explicit `scan --rescan` operations.
  This avoids Windows background I/O starvation during a manual analysis. Per-game
  progress appears in the operation output; normal maintenance scans remain in
  background mode.
- Adds a versioned `arc-info` capability and `arc-record prepare` command. Recording
  preparation accepts an exact game identity, validates a manual game's own folder
  through upstream's public API, and uses upstream's recorder installer. Anti-cheat,
  running-process, existing-DLL and mod checks remain in force.

The earlier Arc localization workaround was removed: 1.2.4 handles invalid Unreal
INI data upstream. The build runs upstream's regression for that case alongside
the retained Arc pipeline-cache regression and manual-folder/recorder tests.

There are no game-name overrides, fake readiness flags, bundled game shaders or keys.
Recording remains necessary when the game does not ship enough pipeline state.
Encrypted formats and engines without a suitable reader can remain unsupported.

## Build and install on Windows

Requires Git and a .NET 10 SDK; the generated CLI is self-contained.

```powershell
./tools/scskiller-compat/build.ps1 -Install
```

The script checks out the pinned source, applies `compat.patch`, runs the synthetic
regression tests, publishes the CLI, and verifies the official portable archive's
SHA-256 before copying its native warmer/recorder assets. To reuse a downloaded
archive, pass `-PortableArchive`. To choose an SDK, pass `-DotNet`.

It installs under `%LOCALAPPDATA%\Programs\ArcTools\SCSKiller-1.2.4-arc.2\current`.
Choose its `cli\scskiller.exe` in **Arc Settings → SCSKiller** and save. Existing
official installations remain separate. Arc's **Analyze again** explicitly requests
fresh engine detection, so an older unsupported result does not persist indefinitely.

This CLI package deliberately has no Velopack `.portable` marker. It keeps the
shared `%LOCALAPPDATA%\SCSKiller` store used by previous compatibility versions,
including settings, recordings and preparation history. The official 1.2.4 portable
package instead migrates to its own `data` folder; Arc 0.1.13 follows that folder
after upstream's `migrated` marker is present.

Build output and source are retained under the repository's ignored `.tools` folder.
The local installation retains the pinned upstream source archive, patch and build
instructions. The script does not install recorders, launch games, register tasks or
change GPU cache settings; those are separate actions.

If the CLI needs a codec absent from the portable distribution, upstream's existing
codec loading/download behavior applies. Proprietary codec redistribution is not
added by this patch.

## License and distribution

`compat.patch` and the patched SCSKiller program are **GPL-3.0-or-later with the
upstream additional permission**, reproduced in [LICENSE](LICENSE) and
[LICENSE-EXCEPTION.txt](LICENSE-EXCEPTION.txt). Arc's build script is MIT. Arc's
Rust/React application continues to call the tool as an external process.

If distributing a built compatibility tool, retain its license and third-party
notices and provide its complete corresponding source, including the pinned upstream
source, this patch and build instructions. Do not publish local game indexes,
recordings, keys or cache files. Native assets are unchanged from the pinned official
release; their source is included in upstream's `proxy` directory.
