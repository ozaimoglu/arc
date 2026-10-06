# Release validation

## v0.1.8 — Eclipse preview

The Windows x64 NSIS release is checked with the following source-level and packaging gates:

| Check | Coverage |
| --- | --- |
| Frontend tests | 29 Vitest cases across library interactions, rating presentation, matching and the native bridge |
| Rust tests | 42 cases covering scanner heuristics, metadata, PS4 parsing/launch plans, provider boundaries and SQLite behavior |
| TypeScript / Vite | Strict compilation and production bundling |
| SQLite migration check | Executes production migrations independently through Python SQLite |
| Clippy | Locked dependencies, all targets, warnings treated as errors |
| Windows packaging | Optimized Tauri release + NSIS x64 installer |

Existing regressions cover missing versus zero scores, correct per-scale colors, PC/PS4 separation, invalid/ambiguous Steam identities, seven-day offline caching, manual matching during active requests, removal exclusions and persistence of user edits.

## Native and asset checks

- The current-user application has been installed and started on Windows with a responding Arc window and system WebView2.
- Existing library, settings, sessions, exclusions and rating/cache records were preserved through the branding upgrade. SQLite integrity and foreign-key checks passed with schema version 3.
- All seven icon sizes (16–256px) were decoded and visually reviewed. Their image bytes are embedded in the Windows executable. SVG/PNG branding uses shared geometry and transparent backgrounds/corners.
- A maintainer-supplied [screenshot of the installed Windows library](assets/arc-library.png) was visually reviewed and included unchanged in the README. It shows the Bloodborne hero and the cover grid; it does not establish interactive UI coverage.
- The README brand poster uses original Eclipse artwork and is separate from the application screenshot.

## Limits

Startup and memory goals have not been benchmarked. MSI is available as a source build target, but the downloadable release is NSIS. Real-game gameplay/long-session stability and pixel-level native UI QA are not established by unit tests or launch-plan checks. Playtime tracks the directly launched process; launcher handoffs and descendants remain a roadmap item.

Artwork and ratings rely on independent services and can be unavailable or rate-limited. Cached data remains usable. Detection is heuristic and unusual installations may require user correction.

See [contributing](../CONTRIBUTING.md) for reproducible checks and [architecture](ARCHITECTURE.md) for boundaries and current behavior.
