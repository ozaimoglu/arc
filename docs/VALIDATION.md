# Release validation

## v0.1.19 — Windows short-path process tracking

The Windows CI process-image regression exposed a real path comparison issue:
`QueryFullProcessImageNameW` can retain an executable's 8.3 launch spelling while
the installation root uses its long path. The failure was reproduced locally
with a short-path temporary directory and an owned copy of the system ping helper.
Arc now resolves both the observed image and installation root before comparing
them. The regression explicitly launches through a short alias when available and
allows a bounded startup interval before checking presence and disappearance.
Only the owned helper is stopped; game and store-client files are unchanged.

86 Rust tests passed (two external-tool tests remain opt-in), including the native
short-path regression, along with strict Clippy across all targets. TypeScript/Vite
production bundling was rerun with the v0.1.19 package metadata. The frontend logic
retains v0.1.18's 77-test validation. The separate SCSKiller compatibility workflow
passed all 706 synthetic tests using the reachable upstream source pin described
below.

The Windows x64 NSIS release is built from frozen source/configuration inputs.
This release does not claim additional gameplay acceptance beyond the Sekiro
confirmation documented for v0.1.18.

## v0.1.18 — Windows launch path compatibility (local build)

Sekiro's two previous Arc attempts matched Windows Application Error 1000
events with exception `0xc0000005`; the selected executable and working directory
were correct. A native child-process probe reproduced the Windows verbatim prefix
in the previous command's `argv[0]`. `Plan::command` now supplies ordinary Windows
program and working-directory paths only when they resolve to the same validated
canonical identity. Store selection, process tracking and argument boundaries remain
unchanged. The footer reads the package version instead of a stale literal.

86 Rust tests passed (two external-tool tests remain opt-in), along with 77 frontend
tests, strict Clippy across all targets, TypeScript/Vite bundling and the SQLite
migration check. A Windows child-process regression observes the real command line
and working directory with a spaced Unicode path. The read-only plan audit checked
ordinary command paths for all 47 visible available records, including Sekiro.

The Windows x64 NSIS package (3.48 MiB) was installed over v0.1.17 after a SQLite
backup and a check that no Arc-tracked session was active. Installed executable
identity, SQLite integrity, foreign keys and complete library/history/preference/
credential preservation passed; existing rating identities were retained. Arc
restarted as v0.1.18 and responded.

After the user's manually started Sekiro session had ended, a single native
diagnostic used the exact validated `Plan::command` source to start its unchanged
EXE. It remained running beyond the previous startup failure interval (20 seconds),
with no new matching crash event; the user confirmed that it worked. The diagnostic
left the game running. Arc's UI Play interaction and further gameplay were not
automated, and no game binaries, saves, shader recorders or caches were changed
by the fix.

## SCSKiller 1.2.5 — Local tool upgrade

The optional compatibility CLI was rebased onto upstream commit
`bf2571af3db4d1d3c49aeb05cda170571eae0704` and installed as **1.2.5-arc.1**.
706 synthetic upstream/Arc regressions passed. Tests with external game/GPU
prerequisites were excluded; the synthetic CSV fixtures use invariant culture on
non-English Windows, without changing the published CLI's culture. Upstream
compiler/analyzer warnings remain. The official 1.2.5 portable SHA-256 was
verified, and installed CLI/native/license assets match the validated build.

Upstream subsequently replaced v1.2.5's release history. The build recipe now pins
the reachable `b10c1e38be5901330c7af39c026d03d660db6883` commit; both commits have
the identical complete Git tree `e27cb1acfc09c922a4e9f647be8ab74092bed3bc`.
The portable asset's SHA-256 is unchanged. This fixes fresh CI checkouts without
changing the tested or installed tool's source files.

Arc's opt-in read-only native cache adapter test passed against the new CLI on
the RTX 4090. It reported up to 53 GB on disk and retained the 100 GB driver limit.
No GPU warm, cache cleanup, driver-setting mutation or game launch was performed
for this upgrade. Arc stayed open during the existing AION 2 session; its tool
preference now points to the new installation.

The previously enabled 007 First Light, CONTROL Resonant, Resident Evil 4 and
Witcher 3 recorder DLLs were updated through upstream's installer and matched to
the verified official 1.2.5 native recorder. Upstream refused Requiem's recorder
because DLSS on NVIDIA loads DirectX 12 before it can, and removed the incompatible
proxy. That guard was retained; gameplay recording was not captured.

Backups and SQLite integrity/schema comparisons verified preservation of the
library, rating identities, preferences and encrypted credentials. Active-session
history remained live. SCSKiller's existing game identities, preparation history,
manual metadata and global settings were preserved.

Upstream imported Witcher 3's existing raw recorder database into its compact
shared-store format (1,845,590 to 671,918 bytes). Reading both files through the
new CLI's `PsoDb.Read` produced the same 1,591 records in the same order, with
identical framed-content SHA-256 and no missing records. The original game-side
inbox was emptied only after this verified import; its pre-upgrade backup remains.

## v0.1.17 — Other launcher routing (local build)

77 frontend tests and 85 Rust tests passed (two opt-in external-tool tests remain
ignored). TypeScript/Vite production bundling, strict Clippy across all targets
and the independent SQLite migration check passed. New coverage includes Riot
product/install associations with stale metadata, Epic local catalogue records,
compressed/uncompressed binary manifest integrity and bounds, EA offer identities,
Ubisoft/GOG numeric IDs, Rockstar argument quoting, Battle.net Agent-to-client IDs
and bounded protobuf parsing. Process tests reject shared clients, bootstrap wrappers,
helpers and neighboring installations, and cover delayed starts and handoff gaps.

A Windows runtime test copied the system ping executable into an owned temporary
installation, started it hidden and confirmed that process-image queries observe
its presence and disappearance independently of a shared client. Only that test
helper was terminated; no installed game or store client was launched or stopped.

The read-only native plan audit verified all 49 visible available library records:
12 Steam, three Riot, two Epic, one NTTGame and 31 standalone/console routes. Existing
LoL, Runeterra and VALORANT entries resolve to their associated Riot Client; GTA V
Enhanced resolves to its full installed Epic catalogue identity; Knight Online
resolves to its own launcher. Fortnite's missing global Epic registration falls
back to its integrity-checked local binary manifest, verified bootstrapper and
known catalogue alias. That route does not restore Epic's own registration.

The frozen Windows x64 NSIS package (3.48 MiB) was installed over v0.1.16 and
Arc restarted as v0.1.17 with a responding main window. It includes the automatic
rating-refresh changes from v0.1.16. The installed executable matches the release
artifact. A pre-install SQLite backup and complete table/schema comparison verified
that every library, rating, history, preference and encrypted credential record was
preserved; integrity and foreign-key checks passed. Game/store files and their
installation records were unchanged. Native screenshot QA was not performed.

Actual gameplay/client acceptance was not observed, including Fortnite's Epic
launch. EA, Ubisoft, Battle.net, Rockstar standalone and GOG arguments were checked
against primary implementations and synthetic identities; the current visible
library has no live installation using those routes. Unknown/new launcher record
formats and inaccessible elevated game processes can still prevent tracking.

## v0.1.16 — Automatic rating refresh (local build)

77 frontend tests and 70 Rust tests passed (two opt-in external-tool tests remain
ignored), along with TypeScript/Vite production bundling, Clippy with warnings
denied and SQLite schema verification. Rating regressions cover the exact
72-hour boundary, repeated refreshes in an open session, resume after a clock
jump, manual-refresh scheduling, hidden-game exclusion, request ordering,
offline backoff and timer/listener cleanup. Existing provider-cache entries with
a seven-day stored expiry respect the new 72-hour score lifetime.

The isolated Windows x64 NSIS build retains the previously installed Steam
launch behavior; concurrent work on additional store routes was excluded. It
was installed and restarted as v0.1.16. Frozen inputs and the installed executable
were verified. A SQLite backup, integrity/foreign-key checks and table comparison
confirmed preservation of the library, history, preferences, encrypted credentials
and schema. Existing rating snapshots were retained; overdue score/cache updates
are expected after startup. Native screenshot QA was not performed.

## v0.1.15 — Steam launch routing (local build)

71 frontend tests and 70 Rust tests passed (two opt-in external-tool tests remain
ignored). TypeScript/Vite bundling, Clippy with warnings denied and schema verification passed. Regression
coverage includes validated installed App IDs, nested/exact Steam scan roots,
existing library records, escaped/nested manifest fields, local SDK IDs retaining
the native route, delayed starts, cancellation timeouts and launcher handoffs.

The read-only native launch diagnostic verified all 12 visible installed Steam
records route through Steam without a rescan. The existing AION 2 entry on F:
resolves to the registered Steam client on C: with `-applaunch 3393110`.
Gameplay and the game's own client startup have not been observed in this check.

The frozen Windows x64 NSIS build was installed and Arc restarted as v0.1.15 with
a live main window. Its installed executable matches the packaged release build.
A pre-install SQLite backup and complete table comparison verified that the
library, ratings, sessions, settings, credentials and schema remain identical;
SQLite integrity and foreign-key checks passed.

## v0.1.14 — Custom desktop title bar (local build)

71 frontend tests and 64 Rust tests passed, together with strict TypeScript/Vite
bundling, Clippy and schema verification. Window-control coverage exercises native
command routing, action failures, out-of-order maximize-state queries, subscription
cleanup after unmount and browser-preview gating. The modal regression verifies that
window controls stay inside the dialog top layer without dismissing its panel.

The frozen Windows x64 NSIS package was installed and restarted as v0.1.14 with a
responding Arc window. Its executable matches the release artifact; the frozen
inputs include the changed capability file. SQLite integrity/schema checks and an
exact pre-install comparison verified preservation of all library, rating, session,
preference and credential records, including the selected SCSKiller path. The native
cache query still reports a 100 GB limit. Pixel-level native UI and pointer-interaction
QA were not performed.

## v0.1.13 — SCSKiller 1.2.4 update (local build)

66 frontend tests, 64 Rust tests, strict TypeScript/Vite bundling, Clippy and schema
verification passed. A clean checkout of SCSKiller 1.2.4 accepted the rebased
compatibility patch and passed 34 reader/manual-folder/recorder/portable-store
regressions. The native assets came from the checksum-verified official portable
archive. Upstream now handles malformed Unreal INI data, so the older Arc workaround
was removed.

Tests cover the migrated portable store, partial/unreached compilation results,
no-stutter verdicts and offline-recording UI gating. Explicit compatibility scans use
upstream's user-requested I/O mode and report the current game. DXIL detection avoids
unnecessary import-table reads; unsupported carved formats retain their support
reason without opening extra DLLs to infer an API label.

The frozen Windows x64 NSIS build was installed and restarted as v0.1.13. Its binary
matches the built artifact. SQLite integrity/schema and all library, rating, session
and credential records were preserved; only the selected shader-tool preference
changed. The native Rust cache adapter queried SCSKiller 1.2.4 on the RTX 4090 and
verified the unchanged 100 GB limit with a 52.5 GB upper-bound disk reading. No cache
deletion or new GPU warming was performed. Native screenshot QA and gameplay stutter
measurements were not performed.

Optional whole-library scans were stopped after extended archive parsing; a complete
1.2.4 re-audit of all installed games was not established. The five existing gameplay
recorder installations and their recordings were verified unchanged, rather than
upgraded in game folders. The CLI's recorder-install command also performs a global
scan before installing. These limits do not affect the verified tool connection or
native cache query.

## v0.1.12 — Shader compatibility and recording preparation (local build)

61 frontend tests, 62 Rust tests, strict TypeScript/Vite bundling, Clippy and schema verification passed. A clean checkout of the pinned SCSKiller source accepted the maintained patch and passed 30 synthetic reader/manual-folder/recorder regression tests before publishing the self-contained compatibility CLI. Its native assets came from the checksum-verified official portable archive.

All 47 visible native PC installations were re-analyzed. The 19 compilable installations passed full shader indexing; three require gameplay recordings, 23 retain unsupported reader/packed/encrypted-format reasons, and two use a store executable different from Arc's selected executable. The shadPS4 entry is handled by the emulator. Full indexes are compatibility evidence, not proof that every game's shaders have been warmed.

007 First Light now detects its raw pipeline-cache containers without a title-specific override: its index contains 8,429 shader programs, and a real GPU warm completed 5,748 planned pipelines with zero failures, skips or crashes. MECCHA CHAMELEON's malformed Unreal localization mapping no longer prevents shader reading: its 23,560-program index produced a successful 20,102-pipeline warm, also with zero failures, skips or crashes. Streaming diagnostic JSON also resolved five previously failing large index exports. These fixes do not add AES keys or readers for encrypted/unknown formats.

Recorder installation was verified for CONTROL Resonant, Resident Evil 4, Resident Evil Requiem and the ray-tracing portions of 007/Witcher 3. The opt-in native Rust tests verified 007's repaired installation identity and CONTROL Resonant's recording setup through Arc's command adapter. Actual gameplay recordings were not captured: those three recording-required games and the two ray-tracing portions still await gameplay. No game was launched for this validation; driver cache sizing and cache deletion were not changed.

The final Windows x64 NSIS installer was built from frozen inputs, installed and restarted as v0.1.12 with a responding Arc window. The installed executable matches the built artifact. SQLite integrity/schema and every existing library, rating, session and credential record were preserved; only the selected shader-tool preference changed to the stable compatibility installation. The global GPU cache limit remained 100 GB. Native screenshot QA and gameplay stutter measurements were not performed.

## v0.1.11 — Shader cache controls (local build)

57 frontend tests and 59 Rust tests passed, along with strict TypeScript/Vite production bundling, Clippy and the SQLite schema check. Coverage includes explicit cleanup confirmation/cancellation, optional generated precache, exact game identity, unsupported GPU controls, global operation locks, navigation recovery, localized cache sizes, administrator-result validation and failure feedback.

The opt-in read-only Rust query exercised the new adapter against official SCSKiller 1.2.3 on an RTX 4090. It reported the actual 100 GB driver limit and an upper-bound disk reading of 51.5 GB. The official per-game cleanup prompt was also exercised with a negative answer: its three attributed MARVEL SNAP cache files and preparation timestamp remained intact. Driver-setting mutation and actual deletion were not performed during validation; those actions remain explicit user controls. Native screenshot validation was not performed for this update.

The optimized Windows x64 NSIS package was built and installed. Arc restarted as v0.1.11 with a responding window; the installed executable matches the built artifact and contains all three cache IPC commands. SQLite integrity/schema checks passed and every existing library, rating, session, credential and preference record was preserved. Build inputs remained unchanged during packaging.

## v0.1.10 — Shader folder correction (local build)

44 frontend tests and 54 Rust tests passed. Regression coverage now includes nested REDengine/Unreal roots, repair of old unconfirmed metadata, preservation of confirmed and narrower folders, and the folder-confirmation UI state. Clippy and production bundling passed.

A local compatibility audit analyzed 47 native installations without launching games or compiling their shaders. SCSKiller reported 17 ready and one already warmed; three required folder confirmation/recording, three had encrypted shaders, and the remaining 23 reported unsupported formats/readers. Two of those unsupported store entries use a different executable from Arc, so they remain unmatched in Arc. The shadPS4 entry was excluded. Ready is an analysis result, not evidence of successful compilation.

The Witcher 3 previously received an outer delivery folder and fell back to the generic shader reader. After resolving its actual `bin/x64_dx12` installation root, official SCSKiller analysis recognized REDengine 3 and reported ready without recording.

The opt-in native regression test reproduced the old unconfirmed root and verified that Arc repaired it through the official CLI. The final v0.1.10 NSIS package was installed and started with a responding Arc window; its executable matches the built artifact. SQLite integrity, all existing library/settings records and the SCSKiller connection were preserved.

## v0.1.9 — SCSKiller integration (local build)

The optional CLI integration passed 43 frontend tests and 52 Rust tests, strict TypeScript/Vite bundling, SQLite migration verification and Clippy. Its additional official-CLI smoke test is opt-in rather than a CI game dependency.

The official SCSKiller 1.2.3 portable archive matched upstream's SHA-256. Native analysis/import, successful MARVEL SNAP shader preparation and graceful queue cancellation were exercised through Arc's Rust integration without launching a game. SCSKiller recorded 3,013 shaders, zero failures and zero skips for the successful warm. Arc refreshes the CLI snapshot after warming before displaying the result.

The v0.1.9 Windows x64 NSIS package was built, installed and restarted successfully. The installed executable matches the release build and exposes the shader IPC commands. SQLite integrity and foreign-key checks passed; every existing game, rating, cache, exclusion, session and encrypted credential record was preserved. Only the SCSKiller tool path was added to preferences. Build inputs remained unchanged during packaging.

Upstream limitations were observed: MECCHA CHAMELEON's shader reader fails with an array-bounds error despite its ready status, while the selected Witcher 3 executable needs a confirmed folder/recording. Arc reports these reasons rather than claiming successful preparation. Stutter reduction and new native UI screenshot validation have not been measured.

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

Startup and memory goals have not been benchmarked. MSI is available as a source build target, but the downloadable release is NSIS. Real-game gameplay/long-session stability and pixel-level native UI QA are not established by unit tests or launch-plan checks. Steam sessions follow the client's per-app flag; supported store routes observe installation-scoped game images. Standalone/console sessions track the directly launched process. Closing Arc still interrupts tracking, and unusual clients/installation records may require additional support.

Artwork and ratings rely on independent services and can be unavailable or rate-limited. Cached data remains usable. Detection is heuristic and unusual installations may require user correction.

See [contributing](../CONTRIBUTING.md) for reproducible checks and [architecture](ARCHITECTURE.md) for boundaries and current behavior.
