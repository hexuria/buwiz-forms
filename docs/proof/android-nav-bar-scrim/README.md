# android-nav-bar-scrim — before/after hexuria/nsh#38 (`0c7a8433`)

## Builds (one build folder per checkout; nothing shared)

| side | app commit | sibling NSH | tool | runtime classes.dex sha256 | CARGO_TARGET_DIR |
|---|---|---|---|---|---|
| BEFORE | b51fabe4 (`~/j7-before/app`) | 8c78cfd6 (`~/j7-before/nsh`) | `nsh 0.6.1 (8c78cfd)` | `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` | `~/BuwizNsh/target-8c78cfd6` |
| AFTER | b51fabe4 (`~/j7-after/app`) | 0c7a8433 (`~/j7-after/nsh`) | `nsh 0.6.1 (0c7a843)` | `83ec025691e516102c1b57993929a80e3e46042d613ce4f0d07a73a07c835118` | `~/BuwizNsh/target-0c7a8433` |

Each side had its own app checkout AND its own sibling NSH checkout (`<side>/../../nsh` resolves
per side); the tool was installed from the same commit as the sibling (pr38 CLI built in
`target-0c7a8433`); runtimes were compiled in each side's own target folder (java forced
after a hardlink-seeded cache almost smuggled the wrong dex in — caught by sha256).
AFTER diff proof vs pin: `nsh-diff-proof.txt` — one commit, 18 insertions, `DevActivity.java` only.
Dex shas are of the runtime folders AND were re-verified inside both packaged APKs.
APK targetSdk 35, minSdk 26.

## Verdict on the strip: no visible difference on Android 15 (API 35), in any mode

Screenshots: `before|after-<gesture|threebutton>-<light|dark>-<home|forms|signin>.png`, all with the
correct nav mode — nav modes were switched with `cmd overlay enable-exclusive
com.android.internal.systemui.navbar.threebutton|gestural`, because `settings put secure
navigation_mode` does NOT switch this emulator (both overlays stay enabled and gestural wins).
A pixel sweep of the bottom strip (every 5 px rows / 20 px cols across the bottom 160 px)
finds **zero differing pixels in 8 of 12 combos**; the remaining four (the sign-in page) differ
only by the keyboard's presence in some shots, not the strip. Strip hexes sampled at bottom y-35:

| combo | before | after |
|---|---|---|
| threebutton-light home/forms/signin | #fefefe / #f1f0f7 / #ffffff | identical |
| threebutton-dark home/forms/signin | #0e0e18 / #14151a / #303134 | identical |
| gesture-light | strip matches app bg (#f1f0f7/#f4f4f6) | identical |
| gesture-dark | #0e0e10..#0e0e19 | identical |

Why this is the expected result on this API level, not a void test: the app targets API 35.
On Android 15 the platform enforces edge-to-edge for target-35 apps itself — transparent nav
bar, no contrast scrim, layout into the cutout — which is exactly what #38 switches on manually
(the commit message says the same for the cutout part). On API 35 the BEFORE build already
gets the AFTER behavior from the system. **The fix's effect is only observable on API 29–34
devices**, where the scrim exists and the opt-out matters. An API-34 emulator run was added (`api34-before|after-threebutton-firstrun.png`,
buwiz34 AVD, google_apis x86_64, first-run page, three-button mode enforced by overlay):
**pixel-identical there too — strip #fefefe in both builds.** In both, the strip is a
system-drawn opaque band slightly whiter than the app's own background (#f1f0f7), so on these
emulator images the app is not painting behind the navigation bar in EITHER build, and the
scrim difference #38 removes does not present. Honest reading: the fix's visible effect could
not be reproduced on the API-34/35 google_apis x86_64 emulator images at all; whether that is
emulator rendering or a real gap in how the window lays out behind the bar needs a physical
device or an image with a different nav implementation. Reported as a finding, not hidden.

Landscape: `before|after-landscape-threebutton-*.png` — the side bar in both builds is the
system's three-button rail; no black cutout band visible on this emulator (its cutout is a
small centered punch hole; short-edge layout already in effect on 15).

## Tap targets (tap-targets.md) — systematic 44 dp controls, under the 48 dp rule

uiautomator bounds, dp = px × 160/420. Highlights:
- **Header Back (`nb-back`) and avatar (`nb-avatar`): 44.2 × 44.2 and 43.8 × 44.2 dp — UNDER.**
- The four bottom tabs pass: 102.9 × 56 dp.
- "+ New return" (`tp-new-return`) 117 × 43.8 dp — height UNDER. "Copy TIN" the same.
- Year chips: `tp-years-2026` 51 × **24 dp** — far under. "Add year" trigger 96 × 44.2 — under.
- Filter chips (`tp-forms-freq-*`) heights **24 dp** — far under; widths 35–83 dp, "All" 35 dp wide — under both ways.
- Form-picker search field 238 × 42.3 — under. "Save forms"/"Cancel" 44.2/43.8 — under.
- Home tiles pass (~185 × 105); the calendar strips in Home (`home-strip-*`) are **~9 dp wide** — far under.
- Settings/Filings search field 276 × 41.9 — under.
The pattern: nearly every NSH control is 44 dp tall where the Android guideline wants 48 dp.
Note the audit is the app's hit-target sizes, identical in both builds (a runtime change does
not move them); it was measured on the BEFORE install. sign-in email page and the form page
could not be dumped inside the time (the scratch database wipes on every process start, so each
screen costs a full taxpayer creation) — marked as gaps, not measured.

## Method notes that cost time, so they are written down

- `EBIR_TEST_ENV=1` gives a fresh scratch database per engine: a force-stop or process restart
  wipes the signed-in state. Everything within one process lifetime.
- In three-button mode the tap at (160,2341) is the system BACK triangle, not the keyboard
  chevron; it abandons the New taxpayer form.
- ZIP/RDO sheets: swipe-down inside them dismisses them; type the query immediately after
  opening (the search autofocuses), tap the row at ~y489.

## Amendment — API 34, three-button, correct overlay switching (job 7 follow-up)

Method per the follow-up order: nav mode with `cmd overlay enable-exclusive com.android.internal.systemui.navbar.threebutton` only, verified by the visible three buttons in the shots; themes with `cmd uimode night yes/no`. Emulator buwiz34 (emulator-5564, API 34 google_apis x86_64). BEFORE = target-8c78cfd6 build, AFTER = target-0c7a8433 build, same install measured both ways (`install -r`, dex re-verified: before a5365288…, after 83ec0256…). Screen: Home with the tab bar, taxpayer created fresh in each install.

Pixel colours at screen centre x=540 (1080x2400), identical in BEFORE and AFTER in both themes:

| y | light (both) | dark (both) | what |
|---|---|---|---|
| 2360 | #fefefe | #121518 | three-button band |
| 2300 | #fefefe | #121518 | three-button band |
| 2270 | #f4f4f6 | #0e0e10 | tab bar |
| 2240–2150 | #f4f4f6 | #0e0e10 | tab bar |

Off-center samples (x=200/900) identical too. So on API 34 the band keeps its own surface colour — slightly whiter than the tab bar in light (#fefefe vs #f4f4f6), slightly lighter in dark (#121518 vs #0e0e10) — in BOTH builds: the contrast scrim is present in the BEFORE build as expected, but it is equally present in the AFTER build. nsh#38's two flags (`setNavigationBarContrastEnforced(false)`, cutout short-edges) produce NO measurable change on API 34: they only take effect once the app actually lays out edge-to-edge behind the system bars, which nothing in the chain does for API 29–34. Together with the API-35 result (identical because the platform forces edge-to-edge for targetSdk-35 apps), the honest summary: the strips never differ — on API 35 because both are edge-to-edge already, on API 34 because neither is. Deciding #38 on rendering evidence: the PR as written does not change what the phone draws; the edge-to-edge opt-in itself is what's missing.

Method honesty: `install -r` wipes the scratch database (new engine), so the taxpayer was recreated per install; the scripted creation sequence is in ~/j34setup.py on the machine. Screenshots: before/after-threebutton-light/dark-home.png in this folder.
