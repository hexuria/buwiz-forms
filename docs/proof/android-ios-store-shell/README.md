# Android check: ios-store-shell (#347) — job 8

Phone, portrait, API 35 emulator, gesture AND three-button navigation modes. Taps shown.

## Builds

- BEFORE: app `6bb90ad1` (base of #347) · sibling NSH `ca39c0e9103a389a31ea280799ef34796d7aac70`
  (clean worktree) · nsh tool built from `ca39c0e9103a389a31ea280799ef34796d7aac70` · dev runtime
  classes.dex sha256 `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17` ·
  CARGO_TARGET_DIR `~/BuwizNsh/target-8-before`.
- AFTER: app `0043bc5b` (head of feat/ios-store-shell) · sibling NSH `ca39c0e9103a389a31ea280799ef34796d7aac70`
  (clean worktree) · nsh tool built from `ca39c0e9103a389a31ea280799ef34796d7aac70` · dev runtime
  classes.dex sha256 `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17` ·
  CARGO_TARGET_DIR `~/BuwizNsh/target-8-after`.
- ca39c0e9's dev and release classes.dex are byte-identical to 8c78cfd6's — the pins differ
  only in CLI commits — but each build used its own tools folder (`~/BuwizNsh/tools-ca39c0e`,
  nsh-ref present) and its own target dir, one per checkout.

## 1. Settings pages — Back-control count

Opened from the avatar (bottom sheet → Settings). Sections present: General, Account,
Import/Export, Jobs, Device.

| Page | BEFORE controls | AFTER controls |
|---|---|---|
| General | 2 — header arrow + "< Settings" link in page | 1 — header arrow only |
| Account | 2 — same | 1 — same |
| Import/Export | 2 — same | 1 — same |
| Jobs | 2 — same | 1 — same |
| Device | 2 — same | 1 — same |

- BEFORE: every section page shows TWO back controls (`before-set-*.png`). The header title
  stays "Settings" on the section pages.
- AFTER: every section page shows exactly ONE (`after-set-*.png`); the header title is the
  section's own name (e.g. "General").
- The one Back control in AFTER returns to the Settings list — **WORKS**, verified for all
  five sections by tapping the header arrow (`after-general-backresult.png` shows the list
  after the tap from General; the other four were exercised the same way in sequence).
- In BEFORE, the in-page "< Settings" link also returns to the list
  (`before-general-backresult.png`).

## 2. Sheets (ZIP and RDO choosers) with the keyboard up — top vs status bar

On New taxpayer, chooser opened, keyboard up (the sheet's search takes focus automatically):

| Shot | Sheet top |
|---|---|
| BEFORE gesture ZIP (`before-gesture-zip-kb.png`) | UNDER the status bar — list rows overlap the clock |
| BEFORE gesture RDO (`before-gesture-rdo-kb.png`) | UNDER — same |
| BEFORE three-button ZIP (`before-3btn-zip-kb.png`) | UNDER — same |
| BEFORE three-button RDO (`before-3btn-rdo-kb.png`) | UNDER — same |
| AFTER gesture ZIP (`after-gesture-zip-kb.png`) | UNDER — same |
| AFTER gesture RDO (`after-gesture-rdo-kb.png`) | UNDER — same |
| AFTER three-button ZIP (`after-3btn-zip-kb.png`) | UNDER — same |
| AFTER three-button RDO (`after-3btn-rdo-kb.png`) | UNDER — same |

- With the keyboard up the sheet expands to full height and its scrolling content runs to
  y=0, behind the status bar, in BOTH builds and BOTH navigation modes. **No visible
  difference between BEFORE and AFTER on Android** — saying so plainly, that is the result.
- With the keyboard DOWN the sheet is a normal bottom sheet whose title, close button and
  search field sit fully below the status bar in both builds
  (`sheet-keyboard-closed-reference.png`).

## Recordings

- `settings-rerun-after.mp4` (169s, taps shown): the AFTER settings pass — avatar → Settings →
  sections → back. (Re-run of the already-screenshotted flow for the recording requirement.)
- `before-settings-demo.mp4`: the BEFORE settings pass — two back controls visible per page,
  navigation exercised.
- No recording of the sheet passes survives: the segment covering them was killed mid-write
  (moov atom missing, corrupt) — documented here as a lost artifact, not app behavior. The
  eight sheet screenshots are the evidence.

## Item status

- Settings Back-control count — WORKS as the PR intends on Android: two controls BEFORE,
  exactly one AFTER, and the one returns to the Settings list.
- Sheets below the status bar — DOES NOT change on Android: the sheet top is under the
  status bar with the keyboard up in both builds, both navigation modes. If #347 meant to
  fix this on Android too, it does not; if the fix was iOS-only, Android is unchanged either way.
