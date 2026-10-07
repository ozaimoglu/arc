# Third-party notices

Arc's application code and original Eclipse brand assets are licensed under MIT; dependencies retain their own licenses.

- **CriticPeek 1.5.4:** title matching, provider parsers, queues and cache logic in `src/ratings/vendor/` are adapted from the maintainer's CriticPeek extension. See the [integration notes](src/ratings/vendor/README.md). Chrome and the extension are not required at runtime.
- **Tauri, Rust crates, React, Vite and Lucide:** versions are recorded in Cargo/npm manifests and lockfiles. Consult their package distributions for license notices. Lucide supplies interface icons; the Arc logo is an original vector asset.
- **Steam, Metacritic and SteamGridDB:** independently operated data/artwork services. Their names, marks, game artwork and review content belong to their respective owners. Arc is not affiliated with or endorsed by these services.
- **README brand poster:** original Eclipse vector artwork, included under Arc's MIT license. It presents Arc's identity and is not an application screenshot.
- **Application screenshot:** `docs/assets/arc-library.png` shows the maintainer's installed library. The game artwork, titles and logos visible in it belong to their respective owners and are not covered by Arc's MIT license.
- **SCSKiller:** optional external shader-preparation tool by the SCSKiller authors, licensed under GPL-3.0-or-later with its upstream additional permission. Arc invokes the CLI as a separate process and does not bundle or link its binaries in the Arc installer. The separately licensed compatibility patch in [tools/scskiller-compat](tools/scskiller-compat/README.md) includes upstream source excerpts and is GPL-3.0-or-later with that permission; Arc's application code remains MIT. See [integration details](docs/SCSKILLER.md) and the [upstream repository](https://github.com/BlueHeisenberg/SCSKiller).

Arc does not bundle games, ROMs, PS4 firmware, game data or the shadPS4 emulator. Users supply their own installations.
