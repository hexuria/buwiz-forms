# android-tap-targets-tablet-branch — WORKS

Measure-only proof for `feat/ios-store-tablet` @ 14dc0edf on Android. See `tap-targets.md` for the old-vs-new table.

## Build header

- App commit: `14dc0edf` (feat/ios-store-tablet)
- NSH pin (`NSH_PINNED_REF` in that commit's `.github/workflows/nsh.yml`): `ca39c0e9103a389a31ea280799ef34796d7aac70`
- Sibling NSH: clean checkout at ca39c0e9 beside the app
- Tool + both runtimes: `~/BuwizNsh/tools-ca39c0e` (real copies, nsh-ref inside), `nsh 0.6.1`
- CARGO_TARGET_DIR: `~/BuwizNsh/target-14dc0edf`
- classes.dex sha256 (runtime): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`
- Emulator-5554, API 35, gesture navigation, `wm density` = 420. dp = px×160/420.
- Safety env: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1`. No live anything. No code changed.

## Items

- header Back + avatar — WORKS (48.0×48.0 both, were 44.2/43.8×44.2 UNDER)
- Copy TIN + New return — WORKS (48.0 tall, were 43.8 UNDER)
- year chip + 5 filter chips — WORKS (48.0 tall, were 24.0 UNDER)
- Home day strip — WORKS (each day exactly 48.0×80.0, strip scrolls sideways; was 30 non-scrolling 8.8–9.1 dp slivers)
- four bottom tabs — WORKS (102.9×56.0 PASS, unchanged)

## Files

- `forms-page.png` — taxpayer forms page (header, Copy TIN/New return, year chip, tabs)
- `chooseforms.png` — picker with the filter chips
- `home-strip.png` — Home day strip at rest
- `home-strip-scrolled.png` — the same strip after one horizontal drag (shifted ~4 days)
