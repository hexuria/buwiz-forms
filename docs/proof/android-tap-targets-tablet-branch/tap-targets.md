# Tap targets on `feat/ios-store-tablet` @ 14dc0edf — Android, phone, API 35

Measured from uiautomator dumps, density 420 (`wm density`), dp = px×160/420. Measure only — no code changed. Old numbers are job 7's rows (tap-targets.md on `try/android-nav-bar-scrim-1`), copied so the two compare.

## Header — Back and avatar

| control | job 7 (before) | this branch | verdict now |
|---|---|---|---|
| nb-back | 44.2×44.2 dp UNDER px[21,63][137,179] | 48.0×48.0 dp px[21,136][147,262] | **PASS** |
| nb-avatar (Account) | 43.8×44.2 dp UNDER px[944,63][1059,179] | 48.0×48.0 dp px[933,136][1059,262] | **PASS** |

(The avatar glyph inside is still drawn 28.2 dp, but the clickable control — what job 7 measured — is the 48×48 `Account` button. The header on this branch also carries `New return` and `Search` buttons, each 48.0×48.0 PASS.)

## "+ New return" and "Copy TIN"

| control | job 7 | this branch | verdict now |
|---|---|---|---|
| tp-copy-tin | 106.3×43.8 dp UNDER | 106.3×48.0 dp px[42,495][321,621] | **PASS** |
| tp-new-return | 117.0×43.8 dp UNDER | 117.0×48.0 dp px[342,495][649,621] | **PASS** |

## Year chip and filter chips

| control | job 7 | this branch | verdict now |
|---|---|---|---|
| tp-years-2026 | 51.0×24.0 dp UNDER | 51.0×48.0 dp px[47,871][181,997] | **PASS** |
| tp-forms-freq-All | 35.0×24.0 dp UNDER | 48.0×48.0 dp px[47,1343][173,1469] | **PASS** |
| tp-forms-freq-Monthly | 67.4×24.0 dp UNDER | 67.0×48.0 dp px[179,1343][355,1469] | **PASS** |
| tp-forms-freq-Quarterly | 74.3×24.0 dp UNDER | 74.7×48.0 dp px[360,1343][556,1469] | **PASS** |
| tp-forms-freq-Annual | 60.2×24.0 dp UNDER | 60.6×48.0 dp px[561,1343][720,1469] | **PASS** |
| tp-forms-freq-As needed | 83.0×24.0 dp UNDER | 82.7×48.0 dp px[725,1343][942,1469] | **PASS** |

(Also on the picker on this branch: Save forms 96.0×48.0, the setup chips Non-VAT professional 161.5×48.0 / VAT business 109.0×48.0 / Employer 84.6×48.0 — all PASS. Cancel 69.0×48.0 PASS, Add year 104.0×48.0 PASS, Edit forms 111.6×48.0 PASS. The segment tabs Profile/Forms/Templates are 61.0/84.2/109.7 × 48.0 — all PASS where job 7 had them at 43.8 UNDER.)

## Home calendar day strip

Job 7: 30 days squeezed into the width, each **8.8–9.1×80.0 dp UNDER**, no scrolling.
This branch: each day cell is exactly **48.0×80.0 dp PASS** — 7 whole days visible (Nov 25 → Dec 1) plus the 8th clipped at the edge — and the strip sits in a HorizontalScrollView. It **does scroll sideways**: after one 600 ms horizontal drag the window shifted ~4 days (Nov 29 partial at the left edge through Dec 6 partial at the right), every fully visible day still exactly 48.0×80.0 dp. The only UNDER readings are the half-clipped edge cells mid-scroll, which is geometric, not a target-size defect.

| day | job 7 width | this branch width |
|---|---|---|
| each of 30 days | 8.8–9.1 dp UNDER | 48.0 dp PASS (scrolls) |

## Bottom tabs — unchanged, still passing

| control | job 7 | this branch | verdict now |
|---|---|---|---|
| tab-home | 102.9×56.0 dp PASS | 102.9×56.0 dp px[0,2169][270,2298] | PASS |
| tab-library (Forms) | 102.9×56.0 dp PASS | 102.9×56.0 dp px[270,2169][540,2298] | PASS |
| tab-filings | 102.9×56.0 dp PASS | 102.9×56.0 dp px[540,2169][810,2298] | PASS |
| tab-calendar | 102.9×56.0 dp PASS | 102.9×56.0 dp px[810,2169][1080,2298] | PASS |

## Summary

Every control job 7 measured as UNDER is now PASS on this branch: header Back and avatar 44→48 dp, Copy TIN / New return 43.8→48 dp tall, year chip and all five filter chips 24→48 dp tall, and the Home day strip rebuilt from thirty 9 dp slivers into scrolling 48 dp cells. Nothing that passed before regressed.
