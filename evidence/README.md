# Taffy 0.15 → 0.16: iOS Simulator before/after visual comparison

driver: gpui-agent-cli 0.5.0

Does any screen look different after the kit's layout engine moves from Taffy 0.15 to 0.16
(hexuria/nsh#62)? A tour of the app's screens driven only through the project's test driver
(`just e2e ios tour`, gpui-agent recipes; no fixed-coordinate taps, no OS keystroke injection),
run on one iPhone and one iPad Simulator, before and after, every pair of screenshots compared
pixel by pixel with the status bar masked.

## The two states

- BEFORE: buwiz-forms `main` at b983b2a4eff06b0c6203fd5829d62f5128db8639, kit at the pin it names
  (8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf).
- AFTER: buwiz-forms branch `try/kit-taffy-016` at 8f825621dc697f13bea69e4f83a46711d59a42c5,
  kit pin moved to bc417406b63479bf137b8327374265a62a2c0f6a (the branch's own `NSH_PINNED_REF`).

Both run heads were read from GitHub before building; each run's `app.log` first line names the
commits and the tool it built with.

## Devices

- iPhone 17 Pro, iOS 26.5 (one booted at a time, per `scripts/e2e/machines/ios.sh`)
- iPad Pro 13-inch (M5), iOS 26.5 — the phone layout, as the runner expects on mobile

## The tour

`agent-recipes/e2e/tour.json` (13 recipes, `just e2e check tour`: 0 problems) plus the
`tour-*.json` recipes beside it: first-run set-up cards, sign-up, Home, the navigation (every tab,
the Library among them), New return, a 2551Q form page at the top / in the middle (a part title
pinned) / scrolled to the very end, Schedule 1 with three rows, the ATC pick sheet with the
software keyboard up scrolled to its last row, the period calendar sheet. Screenshots are the
recipe steps marked `"screenshot": true`; the stock sign-up and tax-year recipes contribute
theirs. Every run: 12 pass of 13 — sheet-pick's screenshots are taken before its last step
(a cleanup click on a virtualized list option that has scrolled out of the tree) fails; the
failure does not touch any picture.

## Result

**No screen looks different.** iPhone: all 21 pairs pixel-identical. iPad: 14 identical, 7 differ —
every one of the seven is run-to-run noise, shown in the side-by-sides in `compare-ipad/`:
a toast notification mid-animation, the search box's caret blink, and mid-flow timing
(one run catching "Opening 2551Q…" or the sign-up card, the other the settled screen).
Not one differing pixel is in a laid-out element: the scroll-area ends, the pinned part titles,
the keyboard-up pick sheet at its last row and the calendar grid — the reviewer's expected risk
spots — are all in identical pairs on both devices. See `compare.md` for the table.

## Layout of this evidence

- `before-iphone/`, `after-iphone/`, `before-ipad/`, `after-ipad/`: one folder per run as the
  runner wrote it (report.md/report.json/run.log/app.log, per-recipe screenshots and receipts).
- `compare.md`: the comparison table (screen / device / identical-or-differs with box and count).
- `compare-iphone/`, `compare-ipad/`: the machine-readable table (`table.json`) and, for every
  differing pair, a side-by-side PNG (before | after).
- `smoke-after/`, `smoke-before/`: the `just e2e ios smoke` cross-check (see the session report).

Runs used the dry-run test environment with a scratch database; nothing was sent.
