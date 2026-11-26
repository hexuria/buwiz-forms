# Android sheet panel with the keyboard up — where it actually sits

Measure only. The probe edit was local and is reverted; the checkout is clean (`git checkout nsh/ui/choice.tsx` done, tree verified clean). The probe's diff is in `probe.diff`.

## Header

- App `ee3e649d` (main; per the revised job, not 0043bc5b — no rebuild of job 8's AFTER). Main already passes `popupStyle={{ maxHeight: sheetMaxHeight() }}` in `nsh/ui/choice.tsx` `sheetRoom()` (lines ~287, 357, 436), so main is the AFTER case.
- Sibling nsh, tool and runtime: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the pin named in that commit's `.github/workflows/nsh.yml`), real copies in `~/BuwizNsh/tools`.
- classes.dex sha256 (dev runtime): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`.
- Folder `~/BuwizNsh/target-ee3e649d` — reused from job 26's BEFORE build, as instructed.
- Device: emulator API 35, 1080×2400, density 420 (dp = px ÷ 2.625), gesture navigation.
- **Step 4 (the BEFORE repeat at 6bb90ad1) skipped as instructed** — job 8's BEFORE folder no longer exists and rebuilding was waived.
- `console.log` does not reach logcat in this dev-run setup (same as job 22); the probe lines were captured from the `nsh run` dev-server console, in order, in `probe-lines.txt`.

## The probe

`nsh/ui/choice.tsx`, inside `sheetRoom()` — so it logs what the sheet sees on every render:

```
console.log("sheet-inner", windowSize().height, safeArea().top, safeArea().bottom, keyboardFrame().height, sheetMaxHeight());
```

Fields: window height / safe top / safe bottom / keyboard / sheetMaxHeight, all dp.

## Probe lines, in order (`probe-lines.txt`)

Opening 1 (field auto-focuses, keyboard animates up):
```
sheet-inner 914.2857 51.809525 24 0         838.476175   ×4  (keyboard still down)
sheet-inner 914.2857 51.809525 24 336.38095 526.095225   ×4  (keyboard fully up)
```
Keyboard then hidden with Back, sheet still open:
```
sheet-inner 914.2857 51.809525 24 0         838.476175   ×8
```
Opening 2, keyboard up:
```
sheet-inner 914.2857 51.809525 24 336.38095 526.095225   ×4
```

## Measurements, dp (px ÷ 2.625)

| | keyboard down | keyboard up |
|---|---|---|
| window height the sheet sees | 914.29 | 914.29 |
| safeArea top / bottom | 51.81 / 24 | 51.81 / 24 |
| keyboardFrame height | 0 | 336.38 |
| sheetMaxHeight() | 838.48 | **526.10** |
| sheet panel top edge | 91.4 (below the status bar) | **negative — clipped above the screen** (title and search field cut off) |
| sheet panel bottom edge | 914.3 (screen bottom) | **≈265.5** (sheet ends mid-screen, dimmed page visible below it) |
| sheet panel height | 822.9 | 526.10 maxed, top off-screen |
| app window frame (dumpsys) | [0,0][1080,2400] full screen | [0,0][1080,2400] full screen |
| app content root (uiautomator) | [0,0][1080,2400] | **[0,0][1080,1517] = resized to 577.9 dp** (adjustResize) |
| keyboard top edge | — | 1517 px = **577.9 dp** |
| IME window frame | [0,136][1080,2400] | [0,136][1080,2400], content inset top 1381 px |

There is no separate dialog window: the sheet renders inside the single `AppActivity` window.

## What this decides

The orchestrator's first reading is confirmed: **the keyboard is subtracted twice.**

- The panel bottom with the keyboard up sits at ≈697 px — near 2400 − 2×883 px (the keyboard height in px, 336.38 dp), i.e. the panel is lifted by the keyboard **twice**. A single lift would put its bottom at the keyboard's top edge, 1517 px.
- At the same time the panel's height is capped at `sheetMaxHeight()` = 526.10 dp — a height that already subtracts the keyboard once. Lifted twice and shortened once, its top edge lands at ≈ −260 dp, far above the status bar: the title bar and the search field are clipped off the top of the screen, exactly what the screenshots show.
- With the keyboard down the geometry is consistent: panel height 822.9 dp ≤ maxHeight 838.48, top at 91.4 dp below the status bar — correct.

## Files

- `kb-down.png` / `kb-up.png` — the two states (taps shown).
- `ui-kb-down.xml` / `ui-kb-up.xml` — uiautomator dumps; the kb-up dump shows the list rows starting at y=0 with no search-field node visible (clipped).
- `dumpsys-kb-down.txt` / `dumpsys-kb-up.txt` — full `dumpsys window windows` captures.
- `j22-zip-keyboard-down.png` / `j22-zip-keyboard-up.png` — job 22's two pictures (app 0043bc5b, same kit behavior), included because the Devin attachment couldn't be opened from the orchestrator side.
- `probe.diff` — the probe as applied; reverted afterwards.

## Items

- probe lines keyboard down + up, in order — WORKS
- uiautomator ancestor bounds — WORKS
- dumpsys window frames — WORKS
- screenshots both states — WORKS
- job 22 pictures included — WORKS
- probe reverted, tree clean — WORKS
- BEFORE repeat — SKIPPED per instruction
