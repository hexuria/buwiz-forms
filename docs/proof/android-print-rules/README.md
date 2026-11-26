# Android print rules — form 0605 long values (#348 + #333 + #355)

Measure only; no code changes.

## Header

- App: hexuria/buwiz-forms `try/print-rules-android` at **`fe17a23f`** (main ee3e649d + #348 accaf8f1 + #333 62490278 + #355 a067f980, merged). Folder `~/BuwizNsh/target-fe17a23f`.
- Sibling nsh, tool and runtime: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the pin in that commit's `.github/workflows/nsh.yml`), real copies in `~/BuwizNsh/tools`.
- classes.dex sha256 (dev runtime, from the dev host APK): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`.
- The four safety variables; demo taxpayer; never Queue. Android API 35 emulator, density 420.
- Seeding: `Check0605.tsx` (copy here) — same host-API fixture pattern as job 9 (`bir.screen_form_open`/`fill`/`save`, `import "./screens/print/host"`, `screen_form_print` after 8 s), printed through the real system print path; adb drives "Save as PDF". `rejected 0` both runs.
- Values (exactly): case 1 address = 137 chars `UNIT 1234 TOWER B, THE VERY LONG BUILDING NAME RESIDENCES, 5678 BARANGAY SAN ISIDRO LABRADOR EXTENSION ROAD, QUEZON CITY, METRO MANILA PH` (job 9's exact string does not survive verbatim anywhere; its README said 137 characters and this is its text as printed in job 9's own proof sheet html, completed to exactly 137). Case 2: registered name, address and line of business each exactly 200 chars (in the fixture file).

## Case 1 — 137-character address: prints NOTHING on Android — DOES NOT WORK

- `case1-0605.pdf` saved through Save as PDF first try, one page. TIN, RDO, zip, the profile name "TEST SURNAME", all captions print fine.
- **The Registered Address box is empty on paper** — not clipped, not truncated: zero glyphs. The value does not occur anywhere in the PDF's text. 600 dpi crop: `case1-android-address-600dpi.png`.
- Yet the rules had been applied correctly in the print input: `case1-sheet.html` carries the address in the field's span with a second-line break, `font-size:8.55pt;line-height:1.04` — two lines at 8.55 pt sheet units (7.51 pt on paper after the 0.8781 zoom), no truncation, exactly #355's first two rules.
- Control: the **same html** printed through desktop headless Chromium (`case1-desktop-chromium.pdf`) renders the address on **two lines at 7.51 pt, full text** (crop `case1-desktop-address-600dpi.png`). The html is good; Android's WebView print pipeline drops the two-line span entirely.

## Case 2 — 200 characters each: the PDF cannot be produced at all — DOES NOT WORK

- The print preview **renders** — two pages, page 2 visibly the additional sheet.
- Tapping save crashes the WebView renderer every time: `aw_browser_terminator.cc(154): Renderer process crash detected` in logcat, the preview flips to "Sorry, that didn't work. Try again.", Retry does not recover, and the print spooler's `print_job_*.pdf` stays at 0 bytes (pulled, empty — job 9's fallback capture is impossible). **4/4 attempts failed identically**, one of them after a full emulator reboot. Same failure signature as job 9's BEFORE build on the heavy two-page 2550M.
- What the document would have been, measured on `case2-sheet.html` and the desktop-chromium render of it (`case2-desktop-chromium.pdf`, 2 pages):

| field | lines on the form | size | ends with "..." | full value on the additional sheet |
|---|---|---|---|---|
| registered name | 2 | 8.55pt sheet / **7.51pt printed** | yes — line 2 ends `...SSSSSS TT...` | yes, but **twice**: once in the sheet's own header block (`Taxpayer's Name: <full 200 chars>`) and once in its entry row |
| registered address | 2 | 7.51pt printed | yes — `...METRO MANILA, NATIONAL...` | yes, exactly once |
| line of business | 2 | 7.51pt printed | yes — `LONG-NAME IMPORTED SPECIALTY...` | yes, exactly once |

- So the "…" uses three ASCII dots (`...`), per the sheet header: `Entries shortened with "..." on the form are printed in full below.` The additional sheet carries TIN, the taxpayer name, the per-item rows with page references, and every full value.
- Note: 8.55 pt is the size the rules converged on for all three fields in the html (second line first, then smaller — never reaching the 7.5 pt floor here: 8.55 × 0.8781 zoom = 7.51 on paper, and 8.55 is above the floor in sheet units).

## Unexpected extras observed (facts, not judgement)

- The printed form body carries **RULER04–RULER10 rows** (`RULER04 HXHXHXHX abcdefghij 0123456789` …) in the Manner-of-Payment area, overlapping the "Income Tax Second Installment" rows — visible in `case1-0605.pdf` page 1. Neither `case1-sheet.html` nor `case2-sheet.html` contains them, so they enter through the print-overrides layer compiled into this build. They look like the ruler instrumentation of job 9's ruler branches.
- The additional sheet's TIN line reads `000-123-453-00000` while the form page prints the test profile's TIN `12345678900000` — the two pages disagree about the taxpayer.

## Desktop comparison

The Linux desktop app at fe17a23f was **not** built: the machine's disk could not hold a second full rust workspace build next to the Android pipeline (target-fe17a23f alone is 11 GB; the job already hit disk-full twice), so a real desktop-app comparison is absent for that reason. Instead the exact print html the Android app produced was rendered through headless Chromium 156 on this machine — same engine family, desktop print path — and that comparison is what the controls above are.

## Harness events (every attempt)

1. First dev-build install died with the emulator going offline mid-install; emulator hung, cold-booted, re-ran — green.
2. Disk-full killed the first compile at the zip step; purged finished-job artifacts and re-ran — green.
3. Case 2 save: attempt 1 crash, Retry ×2 same, post-reboot retry same — 4/4, reported above.
4. Case 1 save: first try, no crash.

## Items

- case 1 Android print — DOES NOT WORK (address box empty)
- case 1 rules in the print input — WORKS (2 lines, 8.55pt, no truncation)
- case 1 chromium control — WORKS (2 lines, 7.51pt, full text)
- case 2 Android print — DOES NOT WORK (renderer crash, 4/4 attempts, no PDF at all)
- case 2 rules in the print input — WORKS (2 lines then "..." at 8.55pt + additional sheet; name's full value appears twice there)
- 600 dpi crops — WORKS
- desktop-app comparison — NOT DONE (disk; reason above), chromium-of-the-same-html control instead
