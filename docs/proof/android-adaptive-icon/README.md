# Android adaptive icon proof — hexuria/nsh#41

Measure only; no code changes.

## Header

- App: hexuria/buwiz-forms `try/android-adaptive-icon-app` at **`a3335121`** (#369's `f9c604e3` plus `android.adaptiveIcon` in `nsh.json`: foreground `../assets/android/adaptive-foreground.png`, background `#ffffff`). Build folder `~/BuwizNsh/target-a3335121`.
- Sibling nsh: clean checkout at **`dc6b6581890da787e6477ed40c637dd0b5eae7de`** (hexuria/nsh#41 head, branch feat/android-store-adaptive-icon). Tool and both Android runtimes built from that commit into `~/BuwizNsh/tools-dc6b6581` — real copies, `nsh-ref` written (`dc6b65818`). Tool reports `nsh 0.6.1 (dc6b658)`.
- **The app's own `.github/workflows/nsh.yml` still names pin 8c78cfd6; this run uses dc6b6581 on purpose** — the adaptive-icon key only exists in #41's tool.
- classes.dex sha256, taken from the APK: `3f102613d7aa3b3e917d91abf6fd175cad865eb02dccc9598cbff0c484d52f8c`.
- Build: `nsh package --target android` with the four safety variables (BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1). **The build log carries no `unknown key android.adaptiveIcon` warning** — checked.
- Device: emulator API 35, 1080x2400, density 420, gesture navigation; launcher `com.google.android.apps.nexuslauncher` versionName 15. `dev.goldcoders.bir` uninstalled before install. Masks: squircle/roundedrect/teardrop overlays built to the AOSP spec and installed in `/product/overlay` for job 26 (the stock image ships none), plus the default shape — same setup, same mask paths as `docs/proof/android-icon/README.md`.

## APK static checks

1. `aapt2 dump badging` (`badging.txt`): every `application-icon-*` bucket (160/240/320/480/640/65534) is **`res/mipmap-anydpi-v26/ic_launcher.xml`** — the adaptive drawable. WORKS.
2. `aapt2 dump xmltree --file AndroidManifest.xml` (`xmltree-icons.txt`): `<application>` carries both **`android:icon`=@0x7f020000** (mipmap/ic_launcher) and **`android:roundIcon`=@0x7f020002** (mipmap/ic_launcher_round). WORKS.
3. `aapt2 dump resources` (`resources.txt`):
   - `mipmap/ic_launcher`: anydpi-v26 XML adaptive **plus** one PNG fallback at xxxhdpi;
   - `mipmap/ic_launcher_round`: anydpi-v26 XML adaptive;
   - `mipmap/ic_launcher_foreground`: PNG at **mdpi, hdpi, xhdpi, xxhdpi, xxxhdpi**;
   - `color/ic_launcher_background` = **`#ffffffff`**. WORKS.
4. xxxhdpi `ic_launcher_foreground.png` (file `ic_launcher_foreground-xxxhdpi.png` here): **432×432 RGBA**, sha256 `488d171635a82257833d3c1de9e328e33ce02343ad6a7528bbe02abf4366e4a7` — exactly the expected adaptive foreground size. WORKS.

## Device shots

Per light/dark (`cmd uimode night no/yes`), `dev.goldcoders.bir` uninstalled then installed fresh:

- App drawer under each mask (default, squircle, roundedrect, teardrop) — full screenshots plus upscaled crops, drawer-open verified via uiautomator before capture.
- Settings > Apps > App info.
- Recents.

## Judgement — one line per picture

- **Drawer, every mask, light and dark: the whole logo — eagle, sun and both words "Buwiz" and "Forms" — sits inside every mask with nothing clipped, and the white fills the mask edge to edge** (no inset plate). WORKS.
- **Logo size: clearly larger than job 26's wrapped plain icon** — compare columns 1–2 (job 26 AFTER, small centered plate inside the mask) with columns 3–4 (this build, mask filled) in `contact-sheet.png`. WORKS.
- Mask shapes are visibly honored: circle-ish default, squircle, rounded square, teardrop each clip the white background layer to their own outline. WORKS.
- Settings App info: full square white icon with the whole logo, larger and legible. WORKS.
- Recents: the card header icon is masked; the logo reads bigger than job 26's at the same size. WORKS.
- Themed icons: the image has no themed-icons toggle of its own; forcing `settings put secure themed_icon_enabled 1` + launcher restart gives `adaptive-drawer-themed.png` — the icon stays **full colour**, as expected, since no monochrome layer ships. (The dark backdrop in that shot is the themed drawer's own.)

## Contact sheet

`contact-sheet.png`: columns = job 26 AFTER (`f9c604e3`, plain icon the launcher wraps) light/dark beside job 29 (adaptive) light/dark; rows = drawer per mask, Settings, Recents. Job 26's crops are re-used from `docs/proof/android-icon/`.

## Items

- no `unknown key android.adaptiveIcon` warning in the build log — WORKS
- badging anydpi-v26 — WORKS
- xmltree android:icon + android:roundIcon — WORKS
- resources: adaptive both, foreground mdpi→xxxhdpi, background #ffffffff — WORKS
- xxxhdpi foreground 432×432 + sha256 — WORKS
- drawer per mask × light/dark — WORKS
- App info, Recents × light/dark — WORKS
- themed-icons shot — WORKS (full colour, no monochrome layer shipped)
- contact sheet vs job 26 AFTER — WORKS
