# Arc's optional SCSKiller compatibility CLI

This is a maintained patch for SCSKiller **1.2.3**, based on commit
`7d8d6d362852c3427007ea81b06b6901978bef1b`. It builds a separate
**1.2.3-arc.1** CLI. It is not an official SCSKiller release or part of Arc's installer.

The patch:

- Recognizes `pipeline` paths alongside shader/PSO paths, so raw shader containers
  inside pipeline-cache files receive the larger detection sample and full indexing.
- Continues shader archive reading after CUE4Parse's specific malformed localization
  mapping error. Other bounds, archive and decryption errors still fail.
- Streams diagnostic index JSON to disk instead of constructing a potentially
  multi-gigabyte JSON string in memory.
- Adds a versioned `arc-info` capability and `arc-record prepare` command. Recording
  preparation accepts an exact game identity, validates a manual game's own folder
  through upstream's public API, and uses upstream's recorder installer. Anti-cheat,
  running-process, existing-DLL and mod checks remain in force.

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

It installs under `%LOCALAPPDATA%\Programs\ArcTools\SCSKiller-1.2.3-arc.1\current`.
Choose its `cli\scskiller.exe` in **Arc Settings → SCSKiller** and save. Existing
official installations remain separate. Arc's **Analyze again** explicitly requests
fresh engine detection, so an older unsupported result does not persist indefinitely.

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
