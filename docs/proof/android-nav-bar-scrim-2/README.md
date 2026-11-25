# Job 20 — was #38 in effect on API 34? dexdump, NshBars runtime logging, strip pixels

**Report header**

| piece | value |
|---|---|
| app commit | `b51fabe4` (same as job 7's AFTER) |
| sibling NSH | `b0840032` (`~/j20/nsh`, real worktree — 0c7a8433 + `NshBars` logging, never to merge) |
| `nsh` tool | `nsh 0.6.1 (b084003)` (`~/BuwizNsh/tools-b0840032/bin/nsh`, built at the commit) |
| runtime classes.dex sha256 | `a9dff714…` (b0840032 release build — full hash below, re-hashed from the packaged APK) |
| runtimes | `tools-b0840032/runtime/android/{release,dev}` — real copies with `nsh-ref` (`b0840032832bf62d7737afb66dfebef3506f832a`) beside classes.dex |
| CARGO_TARGET_DIR | `~/BuwizNsh/target-b0840032` (sibling, CLI, both runtimes, app) |

Device: buwiz34 emulator (API 34, 1080×2400), three-button mode set with `cmd overlay enable-exclusive com.android.internal.systemui.navbar.threebutton` and verified in the shots, themes with `cmd uimode night yes/no`.

## (a) Was #38 in the job-7 AFTER APK? — YES, and a correction to job 7's write-up

`dexdump -d` of the AFTER APK's classes.dex (sha256 `83ec025691e516102c1b57993929a80e3e46042d613ce4f0d07a73a07c835118`), method `com.longbridge.nsh.DevActivity.edgeToEdge` — full excerpt in `dexdump-after-edgeToEdge.txt`:

- `setNavigationBarContrastEnforced(false)` — present, gated SDK ≥ 29
- `layoutInDisplayCutoutMode = 1` (short edges) — present, gated SDK ≥ 30
- called from two `onCreate` sites at the very top (before anything else)

**Correction to the job-7 API-34 report**: I wrote "nothing in the chain lays out edge-to-edge on API 29–34". Wrong: even the pin (BEFORE) runtime already calls `setDecorFitsSystemWindows(false)` and `setNavigationBarColor(0)` on SDK ≥ 30 (see `dexdump-before-edgeToEdge.txt`); #38 adds exactly the contrast-enforcement flag and the cutout mode. Both builds were edge-to-edge on the API-34 emulator — so the identical #fefefe band in AFTER means the scrim persisted **despite** `setNavigationBarContrastEnforced(false)` being in the code path. This file's runtime evidence below says why (or confirms it).

## (b) NshBars lines and Home strip pixels

`adb logcat -s NshBars` from launch through sign-in, first-run, taxpayer creation and Home (full file `nshbars.log`):

```
11-25 19:01:26.654  9506  9506 I NshBars : edgeToEdge contrastEnforced=true navColor=#0 decorFits=set-false-by-nsh cutoutMode=1 flags=#80810100
11-25 19:01:27.182  9506  9506 I NshBars : focus=true contrastEnforced=true navColor=#0 decorFits=set-false-by-nsh cutoutMode=1 flags=#81810100
(every later focus line identical: contrastEnforced=true — six lines total, never false)
```

**The log reads `true` — and it reads true at the very first line, the one printed immediately after edgeToEdge.** There is no line anywhere with `contrastEnforced=false`. The bytecode call (proved in (a)) either never runs or never sticks; nothing in the log shows even a transient false. So the scrim is on because the enforcement flag is on the whole time, on a build whose dex provably contains the disabling call.

Home strip pixels (x=540 center / x=200 off-center identical unless noted), three-button mode verified in shots:

| zone | light (`home-light.png`) | dark (`home-dark.png`) |
|---|---|---|
| three-button band y2300–2390 | `#fefefe` | `#121518` |
| tab bar y2180–2270 | `#f4f4f6` | `#0e0e10` |

Pixel-identical to job 7's BEFORE and AFTER values on this emulator — the scrim behaviour did not change with the b0840032 logging build, as expected (the patch only logs).

## (c) Modal sheet control — avatar sheet (`sheet-light.png`, `sheet-dark.png`)

| zone | light | dark |
|---|---|---|
| sheet body y1900–2270 | `#ffffff` | `#161619` |
| strip under three buttons y2290–2390 | `#ffffff` | `#16191c` |

- **Light: yes** — the strip is the sheet's exact colour `#ffffff` end to end, while Home's strip is `#fefefe`. (Note: in light this test cannot by itself distinguish "no scrim" from "white scrim over white"; dark is the discriminator.)
- **Dark: close but not exact** — the strip reads `#16191c` against the sheet body `#161619` (+0/+3/+3 per channel), the shift starting exactly at the nav-zone boundary (same y≈2280 edge where Home's tab bar ends). It is far smaller than Home's scrim shift (`#0e0e10`→`#121518`) but it is measurably not the sheet's own colour. Consistent with either a residual enforcement layer on the sheet window or the sheet window not extending into the nav zone at all (system drawing its own background there) — the numbers alone don't separate those two; stated plainly, not resolved.

## Verdict, plainly

`setNavigationBarContrastEnforced(false)` is in `DevActivity.edgeToEdge`'s bytecode and edgeToEdge runs, yet `isNavigationBarContrastEnforced` reads **true from the first log line** — the disable never takes effect on this API-34 image for this app, so the system scrim stays on in both themes. #38's second flag (`layoutInDisplayCutoutMode=1`) does stick (`cutoutMode=1` in every line), and the nav bar IS transparent (`navColor=#0`), so this is not a case of edgeToEdge being skipped wholesale: the contrast-enforcement disable specifically is the one that never lands.

## Method notes

- Build: app `b51fabe4` (`~/j20/app`), sibling NSH `b0840032` (`~/j20/nsh`, real worktree), tool `nsh 0.6.1 (b084003)`, CARGO_TARGET_DIR `~/BuwizNsh/target-b0840032`; packaged APK classes.dex sha256 `a9dff714cd2f756ee33b8e4cde3a65ed70f023fed82342d7d07d00ab96f35d09` re-hashed from the APK, `NshBars` present in its strings.
- Sign-in at this commit is the email-code flow; mock `mock_account --port 3008 --auto-approve` printed `sign-in code for maria@example.com: 246810`, used once.
- Nav mode set with `cmd overlay enable-exclusive com.android.internal.systemui.navbar.threebutton` and verified by the visible three buttons in every shot; themes with `cmd uimode night yes/no`.
- y2320 readings are the white/dark button glyphs themselves and were ignored; off-center x=200 matches x=540 everywhere.
- One UI-driving note: the sign-in Continue button sits under the keyboard when the email field keeps focus; a Back to drop the keyboard was needed before the tap registered. Measurement-relevant only in that it cost time.

