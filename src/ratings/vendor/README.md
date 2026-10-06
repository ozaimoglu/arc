# CriticPeek integration

`core.js` and `service.js` originate from the maintainer's CriticPeek 1.5.4 extension. Arc reuses its title matching, Steam identity checks, Metacritic HTML parsers, review definition, deduplicated queues and seven-day score cache. No Chrome installation or runtime is required. The adapted files are included under Arc's MIT license.

Arc changes: explicit PC/PlayStation 4 parser selection, separate PS4 cache keys, no Steam lookup for console games; canonical platform URLs; Complete/GOTY aliases on either side of matching with exact names preferred. Network and durable storage are supplied by Arc's Rust commands; the frontend cannot fetch arbitrary URLs or read Chrome storage. Retail edition cleanup preserves remake/remaster identity. No downloaded page scripts execute. Arc's adapter adds verified installed App IDs, existing linked catalogue titles and explicit Steam corrections without altering upstream extension files.

No provider logos are copied. Steam and Metacritic remain the respective data sources, independent of Arc. Tests use reduced HTML fixtures from CriticPeek, plus generated platform variants. See the root [third-party notices](../../../THIRD_PARTY_NOTICES.md).
