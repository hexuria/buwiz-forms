# Search picker reproduction at 44ccfd80

App `44ccfd8080f3028528077cd9a7fa64733846f309`; nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; scratch CLI `nsh 0.6.1 (8c78cfd)`; cargo `1.99.0`; build folder `~/BuwizNsh/target-44ccfd80`.

Real Simulator input, Connect Hardware Keyboard OFF, scratch databases only. See `README-REPORT.md` for exact timestamps, scroll mechanism, keyboard/overlap/current-value answers, fixture caveat, and artifact index.

Short result: new-taxpayer RDO/ZIP can be completed but the search field is pushed out of the visible sheet content while the software keyboard is up; list scrolling moves rows only and does not recover it. Form 1701Q RDO is worse: a crushed/clipped bottom panel with no keyboard and unusable search on both devices.
