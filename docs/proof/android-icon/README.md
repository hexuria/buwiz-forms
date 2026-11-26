# Android launcher icon proof — BEFORE vs AFTER

Measure only; no code changes.

## Header

| | BEFORE | AFTER |
|---|---|---|
| app commit | `ee3e649d` (main) | `f9c604e3` (feat/ios-store-icon) |
| icon source | `assets/icon.png` | `assets/android/icon.png` (1024px white square, whole logo) |
| build folder | `~/BuwizNsh/target-ee3e649d` | `~/BuwizNsh/target-f9c604e3` |
| build command | `nsh package --target android` (dev.env safeties: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1) | same |

- Sibling nsh, tool and both runtimes: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the pin named in each commit's `.github/workflows/nsh.yml`), real copies in `~/BuwizNsh/tools` with `nsh-ref`.
- classes.dex sha256, both APKs: `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17` (identical — the Java side does not change between the builds).
- Device: emulator AVD `buwiz`, google_apis Android API 35 x86_64, 1080x2400, `wm density` 420, gesture navigation.
- Launcher: `com.google.android.apps.nexuslauncher` versionName 15.
- `dev.goldcoders.bir` was uninstalled before each install, so the launcher's icon cache was fresh.

## APK static facts

`aapt2 dump badging` (`badging-before.txt` / `badging-after.txt`), identical lines in both:
`application-icon-160` and `application-icon-640` both `res/mipmap-xxxhdpi-v4/ic_launcher.png`; `application: label='eBIRForms'`. One density bucket ships inside both APKs.

`res/mipmap-xxxhdpi-v4/ic_launcher.png` extracted (files in this folder):
- BEFORE: 192×192 RGBA, sha256 `06273bb2b1b75419d3e04bab5765b72ff56f1d49c39da4dcab3d156fa678ded8` — blue disc with the white lowercase "e".
- AFTER: 192×192 RGB (no alpha), sha256 `e2ae2a60715a661c36997b0e04e2c572290cdffd2b47e3b7baea97aa3a60c489` — white square, whole logo centered with white padding.

## Masks

`cmd overlay list` on this image shows no `com.android.theme.icon.*` entries of its own — the stock API-35 emulator image ships no icon-shape overlays. Squircle, roundedrect and teardrop overlays were built to the AOSP spec (`config_icon_mask` strings) and installed into `/product/overlay` (`-writable-system`); they are non-static, switchable at runtime with `cmd overlay enable-exclusive --category`. All three exist here plus the default shape — four rows in the contact sheet. Mask paths used (viewport 100×100):

- squircle: `M 50,0 C 10,0 0,10 0,50 C 0,90 10,100 50,100 C 90,100 100,90 100,50 C 100,10 90,0 50,0 Z`
- roundedrect: `M 50,0 L 92,0 C 96.5,0 100,3.5 100,8 L 100,92 C 100,96.5 96.5,100 92,100 L 8,100 C 3.5,100 0,96.5 0,92 L 0,8 C 0,3.5 3.5,0 8,0 Z`
- teardrop: `M 50,0 L 100,0 L 100,50 C 100,77.6 77.6,100 50,100 C 22.4,100 0,77.6 0,50 C 0,22.4 22.4,0 50,0 Z`

The mask visibly changes the drawer icons (pairwise image diffs are large); the launcher was force-stopped after each switch.

## What was captured

Per build (before/after) × light/dark (`cmd uimode night no/yes`):

1. App drawer under each mask — full screenshot `*-drawer-<mask>-<ui>.png` plus crop `*-crop.png` (icon with label, upscaled). Drawer open verified via uiautomator `apps_list_view` before capture.
2. Settings > Apps > App info (opened with `am start -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:dev.goldcoders.bir`) — full + crop.
3. Recents (app launched, `KEYCODE_APP_SWITCH`) — full + crop.

`contact-sheet.png`: rows = drawer per mask, Settings, Recents; columns = BEFORE light, BEFORE dark, AFTER light, AFTER dark. (Settings and Recents shots are not per-mask; they were taken with the last-enabled mask active.)

The red bar at the top of full screenshots and the thin trace lines are the emulator's tap/swipe indicators (taps shown).

## Judgement — one line per picture

Drawer, AFTER (all four masks, light and dark): the whole logo — eagle, sun and both words "Buwiz" and "Forms" — is visible inside every mask with nothing clipped, on white. But the launcher treats it as a legacy icon: the white plate is scaled down inside the mask shape, so the mask is not filled to its edge and the logo reads small beside the other apps. WORKS (nothing clipped) — with the legacy-shrink caveat the adaptive-icon job measures next.

Drawer, BEFORE: the old icon is a full-bleed blue disc, so it fills every mask shape edge-to-edge (circle, squircle, rounded square, teardrop); the white lowercase "e" stays whole in all of them. Light and dark differ only in the drawer background. WORKS as a legacy icon because the art is full-bleed.

Settings App info: both icons shown unmasked — BEFORE the blue disc, AFTER the full white square with the whole logo legible at larger size. WORKS both.

Recents: the card header shows the masked icon at small size — same treatment as the drawer (BEFORE fills the shape; AFTER is a scaled-down white plate). WORKS both, same caveat on AFTER.

## Items

- aapt2 badging lines recorded — WORKS
- xxxhdpi ic_launcher.png extracted + sha256, both builds — WORKS
- masks: 3 exist + default, switching verified — WORKS
- drawer per mask × light/dark, both builds, full + crop — WORKS
- App info × light/dark, both builds — WORKS
- Recents × light/dark, both builds — WORKS
- contact sheet — WORKS
