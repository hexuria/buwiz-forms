# Job 24 — the decor-first contrast fix (nsh 60b1f1d9) on API 34

**App commit:** `b51fabe4` (same as job 20). **Sibling NSH:** `60b1f1d99edc194d861df634ab5cfb4989700756` (`~/j24/nsh`, real worktree — b0840032 + "create the window's decor before setting its bars"). **Tool:** `nsh 0.6.1` built at 60b1f1d9 (`tools-60b1f1d9/bin/nsh`). **Runtime classes.dex sha256 re-hashed from the packaged APK:** `f7d2578845754ea114b09e5670c8563beabb9bc550c61fb1799d8800959de23f` (NshBars present; both runtimes real copies in `tools-60b1f1d9/runtime/android/{release,dev}` with `nsh-ref`). **CARGO_TARGET_DIR:** `~/BuwizNsh/target-60b1f1d9`. Device: API-34 emulator, three-button mode set with `cmd overlay enable-exclusive com.android.internal.systemui.navbar.threebutton` (verified in the shots), themes via `cmd uimode`.

## Verdict: the fix works exactly as predicted

**NshBars reads `contrastEnforced=false` from the very first line** — the one printed immediately after edgeToEdge — and in every focus-change line after it (full file `nshbars.log`):

```
11-26 00:47:19.917  I NshBars : edgeToEdge contrastEnforced=false navColor=#0 decorFits=set-false-by-nsh cutoutMode=1 flags=#80810100
11-26 00:47:20.262  I NshBars : focus=true contrastEnforced=false navColor=#0 decorFits=set-false-by-nsh cutoutMode=1 flags=#81810100
```

A theme change recreates the activity, and the recreation's first line reads false too (00:50:08 in the log) — it is not a first-launch-only effect.

**Home strip pixels** (same coordinates as job 20: x=540 centre / x=200 off-centre):

| zone | light (`home-light.png`) | dark (`home-dark.png`) |
|---|---|---|
| three-button band y2300–2390 | **`#f4f4f6`** | **`#0e0e10`** |
| tab bar y2180–2270 | `#f4f4f6` | `#0e0e10` |

The band **equals the tab bar's colour exactly** in both themes — gone is job 20's `#fefefe` (light) / `#121518` (dark) scrim. The strip is now continuous with the app's own surface, which is what the disabled enforcement was meant to produce. (x=540 at y2360 in dark reads `#0c0c0d`, the anti-aliased edge of the white square button glyph — x=200 confirms the band itself.)

**Gesture-mode control** (`home-gesture-light.png`): one Home shot in gesture mode after switching back, nothing else changed — no band in either mode, the pill zone sits on the app's own colour as before.

Method notes: install wiped the scratch database, so sign-in (email-code flow, mock `--auto-approve`, code 246811) and the scratch taxpayer (MariaClara) were recreated before measuring; nav-mode switches used the overlay commands only, verified in the shots. Mock account on localhost:3008 via `adb reverse`.
