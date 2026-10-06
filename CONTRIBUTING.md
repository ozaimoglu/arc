# Contributing to Arc

Arc is a small Windows launcher with a Rust core and a React interface. Small, well-tested changes are welcome. You can improve detection, refine keyboard navigation, add regression fixtures, or build the next library feature.

## Get running

Install Node.js 22+, stable Rust with the MSVC toolchain, Visual Studio C++ Build Tools and WebView2. See the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
git clone https://github.com/ozaimoglu/arc.git
cd arc
npm ci
npm run desktop
```

For frontend work, `npm run dev` runs the browser preview with eight sample games. Native scanning and launching require the desktop build. A fresh desktop installation starts with an empty library.

## A useful first contribution

- Add a scanner regression fixture for a legitimate game layout that Arc misses.
- Reproduce an ambiguous rating match with a reduced, synthetic fixture.
- Improve focus order, keyboard navigation or resized-window behavior.
- Help design collections or store imports using the existing Rust/SQLite boundary.

Discuss a larger feature through a [feature request](https://github.com/ozaimoglu/arc/issues/new/choose) before implementing it. See the [roadmap](README.md#roadmap) and [architecture](docs/ARCHITECTURE.md).

## Before a pull request

```powershell
npm test
npm run build
python scripts/check-schema.py
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
```

Describe the problem, resulting behavior and checks you ran. UI changes should include a screenshot from a browser preview or native installation and respect the [design system](docs/DESIGN_SYSTEM.md). Detection changes should preserve user titles, artwork, favorites, hidden games and removal exclusions. Use temporary fixtures for tests; never launch an installed game from an automated test.

Keep API keys, databases, personal installation paths, downloaded artwork and build outputs out of commits. Reduce provider HTML to the smallest fixture that proves a matching or parsing bug. Tests should work without live provider requests.

## Build an installer

```powershell
npm run tauri -- build --bundles nsis
```

The x64 setup file is written to `src-tauri/target/release/bundle/nsis/`. `npm run package:win` additionally builds MSI; the published v0.1.8 download is NSIS.

## Brand assets

With Python Pillow installed, run `python scripts/generate-icons.py` to regenerate logo/icon exports. Run `python scripts/generate-readme-assets.py` for the original README illustrations. Both use editable SVG geometry and the installed Tauri renderer. See [branding](docs/BRANDING.md).

Contributions are provided under the repository's [MIT license](LICENSE). Please follow the [code of conduct](CODE_OF_CONDUCT.md).
