# Arc design system — 0.1.8

The user selected a cinematic, cover-first desktop interface and asked to remove the generic AI-generated appearance. The visual hierarchy is game artwork → game title → Play → library, with restrained controls around it. Native Windows chrome remains available.

## Research and decisions

Reddit was researched on 5 October 2026. These are qualitative observations from current discussions, not a representative survey of desktop design:

- [Playnite PS5 theme, 19 April 2026](https://www.reddit.com/r/playnite/comments/1spe60n/i_am_building_new_ps5_theme_with_multiuser_support/) explores console navigation with top-level tabs and actual working content. Arc uses compact top navigation and real local-library data.
- [Playnite desktop customization, 21 July 2026](https://www.reddit.com/r/playnite/comments/1v2jtc2/i_finally_took_the_time_to_set_up_playnite/) shows interest in a library-focused launcher and carefully customized presentation. Arc opens directly into the user's games.
- [Fullscreen themes, 14 April 2026](https://www.reddit.com/r/playnite/comments/1slcdf3/fullscreen_theme_recommendation_for_showing/) includes preferences for vertical covers and complaints about cropped artwork, intrusive descriptions and slow animation. Arc keeps 2:3 portrait covers, limits description length in the hero, and has no auto-advancing carousel or video background.
- [What makes an app AI slop?, 19 August 2026](https://www.reddit.com/r/ProductivityApps/comments/1vsb3t0/what_makes_a_productivity_app_considered_ai_slop/) criticizes generic gradients, meaningless elements and untested interactions. Arc removes greeting slogans, repeated installation badges, the decorative local-library card and scanner scores from ordinary UI.
- [Desktop-first design, September 2026](https://www.reddit.com/r/UIUX/comments/1w9m6m9/is_desktopfirst_design_a_thing/) emphasizes flexible layouts for resized desktop windows. Arc keeps search and game management reachable at the packaged 760px minimum width.

The `ui-ux-pro-max` design-system search recommended minimal typography and grid structure. Its blue/orange SaaS palette, online fonts and landing-page conversion pattern do not fit this app; they are not applied. React guidance informed semantic buttons, modal focus management and accessible controls.

## Tokens

| Role | Value |
| --- | --- |
| Background | `#101012` |
| Surface / hover | `#1b1b1f` / `#28282d` |
| Primary text | `#f1f0ee` |
| Secondary text | `#a5a5ae` |
| Play / focus accent | `#ece7de` |
| Accent text | `#181716` |
| Border | `#303036` |
| Danger | `#ffaaa4` |
| Cover radius | 7px |
| Dialog radius | 14px |
| Body type | Segoe UI Variable Text, Segoe UI |
| Display type | Bahnschrift, Segoe UI Variable Display, Segoe UI |
| Interaction / view transition | 160–260ms / 240ms |

All typography is local. Blur is limited to dialogs, menus and small controls over artwork; the main library remains sharp. Game artwork supplies the main color. The compact brand mark uses platinum facets and a gold arc; its editable vector geometry, wordmark and native icon share one source pipeline. See [branding](BRANDING.md) for assets and regeneration.

## Composition

A compact top bar contains Library, Recently played, Favorites, global search, scan, hidden games and settings. Narrow desktop windows use two header rows rather than squeezing navigation into icon-only controls.

The library starts with an edge-to-edge hero. Actual game logos are used when available, otherwise a large title is shown. Play is the primary action. Six portrait thumbnails explicitly select the featured game; arrows and an index provide additional navigation. Selection is stable by game ID and never advances automatically. Featured games exclude hidden and unavailable entries and prioritize play history, favorites, hero artwork and recent additions.

The portrait grid uses responsive columns, neutral title rows and a permanently available action menu. Hover or keyboard focus reveals quick Play. Repeated “Local game” / “Ready to play” badges are omitted. Only missing-file warnings, favorites and restore actions add contextual detail. List view adds actual genre/session information.

The detail view uses a large hero, title/logo, Play, favorite and game menu. A separate portrait preserves the clicked cover's image and proportions during the shared-element transition. Installation path, artwork and properties remain accessible below. Missing descriptions and genres are omitted rather than filled with marketing copy.

## Interaction and accessibility

Arc 0.1.7 shows a single line beneath covers: `MC 84/7.4 · Steam 87`. The first Metacritic number is critics /100; the second is users /10; Steam is the positive-review percentage. Tooltips and accessible labels retain source, platform and scale explanations. If only one Metacritic reading exists, the missing counterpart is a neutral em dash; if neither exists, the whole MC group is omitted. Separators appear only between available sources. The line uses the full cover width beneath the title menu and does not wrap in either grid or list view. The detail page labels the user field `Users` and keeps three numeric columns with platform labels, source links, review count and check date. Console entries omit Steam.

The user requested colored ratings. Values now reuse CriticPeek's soft green `#8ee5ac`, yellow `#ffd478` and red `#ff9792`; source labels and scale suffixes remain neutral. CriticPeek's common bands are applied to the reading's scale: critics 75+/50+/below 50, users 7.5+/5.0+/below 5.0, Steam 75%+/50%+/below 50% using its displayed rounded percentage. These are Arc/CriticPeek presentation bands, not a replacement for the providers' own review categories. Numbers and labels remain readable without color. Metacritic's [color explanation](https://metacritichelp.zendesk.com/hc/en-us/articles/15456077802647-What-s-with-these-green-yellow-and-red-colors) informed the favorable/mixed/unfavorable direction. Score color alone does not change artwork, backgrounds or controls.

Refresh and collapsed match correction retain labelled native controls and existing focus/motion tokens. Card scores use a single compact line with source and scale explanations. The detail rating row spans the existing grid, with 32px vertical separation from game information.

- Semantic controls, labelled icon buttons, `aria-current`, `aria-pressed`, visible focus and a skip link.
- Search: Ctrl/Cmd K. Global shortcuts do not interfere with an open dialog. Escape closes a game menu, returns from detail or clears search.
- Native dialogs contain focus and restore it on close. Menus support up/down, Escape and outside dismissal.
- Detail navigation focuses Back, then restores the library's scroll and original cover focus on return.
- Cover/detail transitions keep the portrait separate from the hero. Unsupported WebViews use ordinary state changes.
- Reduced motion disables animation and hover transforms; touch input keeps quick actions visible.
- Covers load lazily beyond the first row; hero/featured artwork loads eagerly. Startup reads the existing SQLite library.
- Empty, filtered, hidden, unavailable, loading and error states use direct operational language.

See [validation](VALIDATION.md) for the actual checks and visual-review limitations.
