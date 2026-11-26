# SearchPick iOS repro — buwiz-forms 44ccfd80

Date: 2026-11-26. App `hexuria/buwiz-forms` exactly `44ccfd8080f3028528077cd9a7fa64733846f309`; sibling `nsh` exactly `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (also the workflow `NSH_PINNED_REF`); scratch CLI `nsh 0.6.1 (8c78cfd)`. Fresh scratch databases only. Simulator input was real mouse input on the on-screen keyboard; Connect Hardware Keyboard was OFF. Safety environment used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` in `SIMCTL_CHILD_*` form; no Queue, real service, or `BIR_SFTP_LIVE`.

## Fixture caveat

New-taxpayer evidence used the production `App.tsx` entry on a fresh scratch database. Form evidence used the repository's `ShellDemo.tsx` fixture in the same exact build to seed Juana Santos/current-year forms, because a complete taxpayer/forms fixture was needed; the `SearchPick` control and form code under test are the same, but the startup route was not production first-run. This is fixture setup only, not synthesized input.

## Short answer

The new-taxpayer RDO/ZIP pickers can be made to work, but their keyboard avoidance is wrong on both devices. With the software keyboard up, the sheet is compressed/moved so the search field is above the visible sheet area. Typing still reaches the focused search field, but the user cannot see what is being typed. Dismissing the keyboard drops/expands the sheet downward and reveals the search field and results. The list scroll view scrolls rows independently; scrolling the list does not bring the missing search field back. The sheet/page underneath does not scroll as the cause, and the sheet top/title remains visible rather than the whole sheet simply leaving the screen.

The form RDO picker is worse: in Form 1701Q item 6, the picker opens only as a crushed/clipped bottom panel behind the persistent bottom UI. No software keyboard appears, the search cannot be reached, typed, or scrolled, and no row can be picked. This failed on both iPhone and iPad.

## iPhone 17 Pro

Video: `artifacts/iphone/new-taxpayer-pickers.mp4` (edited-time timestamps below).

- 5.93 s — RDO opened. Letters keyboard, not a number pad. Search field already out of view; title remained visible and rows began around 013 (`rdo-open-keyboard.png`).
- 44.13 s — keyboard dismissed. Sheet dropped/expanded downward; search field became visible (`rdo-keyboard-dismissed.png`).
- 111.63 s — exact query `05 makati` visible after dismissal; result `050 South Makati` (`rdo-correct-query.png`).
- 116.47 s — row picked; trigger showed `050 - South Makati` (`rdo-picked.png`).
- 148.10 s — ZIP query `12` was scrolled to the end. Footer: `Showing 60 of 141. Type more words to narrow it.` (`zip-end.png`). Row `2812 Pilar` was picked.
- 161.80 s — reopened RDO, searched `050`; current value `050 South Makati` had a check mark (`rdo-current-check.png`).

Form video: `artifacts/iphone/form-rdo-picker.mp4`.

- Form 1701Q item 6 RDO Code was reached through the real UI. At about 43.8–45.5 s the picker appeared only as a bottom fragment/sliver (`form-rdo-open-panel.png`, `form-rdo-after-tap.png`).
- Simulator I/O → Keyboard showed Connect Hardware Keyboard OFF, yet Toggle Software Keyboard was unavailable/no keyboard appeared (`form-rdo-hardware-off-menu.png`).
- At 67.07/68.37 s annotated result: the form RDO sheet was crushed under the bottom bars; only a `Choose…` sliver was visible; RDO search could not be typed or scrolled. A contrasting ordinary radio sheet did open normally (`form-radio-sheet-contrast.png`).

## iPad Pro 13-inch (M5)

Video: `artifacts/ipad/new-taxpayer-pickers.mp4`.

- 6.80 s — first open looked like a centered sheet/card with search visible and the letters keyboard up (`rdo-open-keyboard.png`). This was transient/initial presentation, not the reliable steady state.
- 47.10 s — after close/reopen the search stayed pushed out. Pulling/scrolling the list moved rows but did not reveal the search field; an over-drag could dismiss the sheet (`rdo-stuck-search-hidden.png`).
- The process was restarted as a fixture reset after that stuck state. This does not change the conclusion because the same displacement was then reproduced.
- 92.30 s — exact real-keyboard query `05 makati` became visible only after keyboard dismissal; row `050 South Makati` appeared (`rdo-correct-query.png`).
- 97.10 s — row picked; trigger showed `050 - South Makati` (`rdo-picked.png`).
- 114.80 s — ZIP query `12` reached the end; footer `Showing 60 of 141. Type more words to narrow it.` (`zip-end.png`). Row `2812 Pilar` was picked.
- 126.40 s — reopened RDO, searched `050`; current value was check-marked (`rdo-current-check.png`).

Form video: `artifacts/ipad/form-rdo-picker.mp4`.

- Form 1701Q item 6 RDO Code opened only as a clipped white bottom panel (`form-rdo-clipped-panel.png`).
- No keyboard appeared even though Connect Hardware Keyboard was off (`form-rdo-no-keyboard.png`).
- Annotated failed at 26.3–28.9 s: search field unreachable; typing/scrolling/picking impossible.

## Exact scroll/push mechanism

Evidence favors this mechanism:

1. The sheet itself remains in place at its top/title; the page underneath does not scroll away.
2. Keyboard avoidance compresses/displaces the sheet content. The search field is above the visible/clipped content region while the keyboard is up.
3. The list scroll view scrolls only the rows. It does not pull the search field back into view: on iPad, rows moved from 014 back toward 001 while the search field remained absent.
4. Dismissing the keyboard lets the sheet drop/expand downward, revealing the search field and current query/results.
5. In the form picker, the sheet/panel is additionally crushed/clipped by the persistent form pager/tab bar area and never becomes focusable/usable.

## Keyboard/overlap/current-value answers

- Keyboard type: letters keyboard on both devices for the new-taxpayer RDO/ZIP search fields. On iPad it is the large letters keyboard with a digit row; on iPhone it is the letters keyboard with mode switching.
- Sheet vs keyboard: the sheet is constrained above the keyboard, but usable content is not preserved. Rows/blank area are clipped; the search field is pushed out of visible content. In form RDO, the picker is clipped behind/under the bottom UI and no keyboard opens.
- Current value marking: yes. After reopening and searching `050`, the current `050 South Makati` row showed a check mark on both devices.

## gpui-agent tree/frame y dumps

Not available in this evidence run. The normal dev host was built without the optional `agent` cargo feature (`agent` is off by default in `nsh/Cargo.toml`), so there was no gpui-agent listener/tree dump to query. No agent input synthesis was used. The video frames and timestamps above are the positional evidence.

## Artifact index

- iPhone: `artifacts/iphone/new-taxpayer-pickers.mp4`, `artifacts/iphone/form-rdo-picker.mp4`, matching `*-annotations.json`, and key frames `rdo-*.png`, `zip-end.png`, `form-*.png`.
- iPad: `artifacts/ipad/new-taxpayer-pickers.mp4`, `artifacts/ipad/form-rdo-picker.mp4`, matching `*-annotations.json`, and key frames `rdo-*.png`, `zip-end.png`, `form-*.png`.
