# PR406 AFTER notes — iphone17

App `a9c9fd7c0a50a3639435dcb3e83c71fdb6f6bdea`; nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target `/Users/devin/BuwizNsh/target-a9c9fd7c`. Scratch DBs; safety env; real input only. Connect Hardware Keyboard OFF for iOS evidence.

Device: iPhone 17 Pro, portrait. Production `App.tsx`; fresh scratch `newtp.db`; Connect Hardware Keyboard verified OFF.

## New tax profile RDO/ZIP — PASS with one opening-transient observation (`pickers.mp4`, edited timestamps)

- 6.4 OBSERVATION/FAIL: initial RDO opening frame had sheet top/grabber/close clipped under the status area while search and letters keyboard were visible.
- 10.9 PASS after typing `0`, sheet settled with grabber/header/search visible and pinned.
- 55.8 PASS owner case end: query `0`, search pinned, last match `120 — RDO 120` reachable above keyboard (`rdo-owner-end.png`).
- 58.8 PASS last-row tap picked `120` and closed while keyboard was up.
- 75.2 PASS `05 makati` filtered to `050 South Makati` (`rdo-05-makati.png`); tap picked it.
- 82.3 PASS X closed without changing value.
- 90.2 PASS `zzz` showed `Nothing matches` while pinned (`rdo-zzz.png`).
- 95.8 PASS reopen showed current `050` checked/in view at top (`rdo-current-check.png`).
- 117.8 PASS ZIP opened focused with digits-first keyboard (`zip-digits-open.png`).
- 169.7 PASS query `12` end: search pinned; `9712 Taraka` reachable above keyboard (`zip-12-end.png`); picked.
- 176.3 PASS ABC switched to letters (`zip-letters-abc.png`).
- 186.6 PASS `makati` showed ZIP rows; `1224 Makati Commercial Ctr` picked (`zip-makati.png`).

## 1701Q item 6 RDO Code — PASS (`form1701-rdo.mp4`, fixture database seeded with 1701Q)

- Open PASS: search focused/pinned; letters keyboard up; current `039` checked.
- Query `0` end PASS: search pinned; last match `120 — RDO 120` reachable above keyboard.
- Last-row tap PASS: picked `120` and closed while keyboard was up.

## Added exact 1701Q script + dropdown 5–12 choices — UNTESTED / BLOCKED

The exact newly required `050` / clear / `makati` / leave-return script and the real-click 5–12-choice dropdown check were not completed in this continuation. The attempt to run parallel macOS+iPhone evidence hit the development-session/cargo-lock constraint and then critical disk exhaustion (114 MiB free). Existing earlier `form1701-rdo.mp4` evidence above remains valid for the older pinned/end/pick script but does not claim the exact new script.

## exact 1701Q + dropdown 5–12 — UNTESTED / BLOCKED

The exact new 1701Q script and real-click dropdown check could not launch because compiling the describe runner exhausted disk (`ld: write() failed, errno=28`). Earlier form1701 evidence remains only partial for the old script.

## exact 1701Q RDO — PASS (`1701q-rdo.mp4`)

- 42.9 PASS typed `050`, exact South Makati row filtered, search pinned/focused with letters keyboard.
- 69.2 PASS cleared then corrected to exact `makati`; all four rows/end visible with keyboard up (`1701q-rdo-makati.png`). An initial mis-typed `majati` was visibly corrected during the recording.
- 72.2 PASS picked `050 — South Makati`; form box showed code+name.
- 77.2 PASS returned after leaving edit mode; current value checked/in view (`1701q-rdo-current.png`).

## dropdown 5–12 choices — UNTESTED

Mac and exact-1701Q evidence consumed the remaining run; a qualifying form item was not reached in time.

## dropdown 5–12 choices — PASS (`dropdown-5to12.mp4`)

Device: iPhone 17 Pro. Fixture `TaxpayerDemo.tsx` / seeded scratch 1701Q DB, disclosed. Connect Hardware Keyboard OFF; real Simulator-window click only.

- 1701Q Part I item 8 “Alphanumeric Tax Code (ATC)” is a six-choice kit dropdown (`frm1701q:optATC` has six choices).
- PASS dropdown opened as a bottom chooser showing exactly six ATC choices (`dropdown-5to12-open.png`).
- PASS a real click on `Business Income - Graduated IT Rates` landed; chooser closed and field retained the picked row (`dropdown-5to12-picked.png`).
