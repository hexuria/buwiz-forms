# iPhone 17 Pro AFTER notes — PR406 34c29679

App `34c29679db70d5e408719b28bb24573dc445bbd5`; nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target `/Users/devin/BuwizNsh/target-34c29679`; fresh scratch DBs; Connect Hardware Keyboard OFF (`hardware-keyboard-off.png`). Real Simulator-window input only.

## 1. New tax profile RDO — PASS (`1-rdo.mp4`, edited timestamps)

- 7.3 PASS open: search focused/pinned at top; letters keyboard up.
- 21.4 PASS query `05` at list end: search stayed pinned; row 105 reachable above keyboard.
- 34.2 PASS exact query `05 makati` filtered to `050 South Makati` above keyboard.
- 37.2 PASS tapping row while keyboard stayed up picked and closed; trigger `050 — South Makati`.
- 42.5 PASS X closed sheet without changing choice.
- 50.5 PASS `zzz` showed `Nothing matches. Try a code or other words.`
- 55.8 PASS reopen showed current `050 South Makati` in view at top with check mark.

## 2. New tax profile ZIP — PASS (`2-zip.mp4`, edited timestamps)

- 5.8 PASS open: search pinned; iOS digits-first keyboard up (`2-zip-digits-open.png`).
- 59.9 PASS query `12` at end: search pinned; `9712 Taraka` reachable above keyboard (`2-zip-end-12.png`).
- 62.9 PASS tap while digits keyboard was up picked `9712 Taraka`.
- 67.5 PASS ABC switched to letters (`2-zip-abc-letters.png`).
- 78.3 PASS `makati` showed ZIP rows while keyboard stayed up (`2-zip-makati.png`).
- 81.4 PASS picked `1224 Makati Commercial Ctr` (`2-zip-picked.png`).

## 6. Owner exact RDO end case — PASS (`owner-case-rdo-scrolled-to-end.mp4`)

- 5.7 PASS precondition: blank search pinned; current 050 check-marked.
- 40.9 PASS very end: search still pinned; last row `93B Zamboanga Sibugay` visible above keyboard.
- 43.8 PASS last-row tap picked `93B` and closed while keyboard was up.

## 3. Form 2551Q item 7 RDO — PASS (`3-form-rdo.mp4`, fixture TaxpayerDemo)

- Open PASS: search focused/pinned; letters keyboard up; current `039` check-marked.
- Query `05` scrolled to end PASS: search pinned; `105` reachable above keyboard.
- Exact query `05 makati` PASS: filtered to `050`; tap while keyboard up picked and closed.
- X PASS: closed without picking. Observation: form behind returned scrolled to top/collapsed.
- Reopen PASS: current `050` in view at top and check-marked.
- `zzz` PASS: `Nothing matches` while search stayed pinned.

## 4. Individual Birth date and Business start date — PASS (`4-dates.mp4`, fixture TaxpayerDemo)

- Birth date sheet opened with typed field and calendar.
- Calendar pick filled `11/20/2026`; digits keyboard opened for typing.
- Typed `05171990` became `05/17/1990` and calendar jumped to May 1990.
- Impossible `02/30/1990` refused with red format guidance.
- Next-year `01/01/2027` refused: `A birth date can't be in the future.`
- Valid typed date accepted; Save persisted `May 17, 1990`.
- Clear then Save persisted Birth date as `Not set`.
- Business start impossible `02/30/2020` refused with same guidance.
- Business start next-year `01/01/2027` refused with future sentence.
- Valid `01/01/2020` accepted and persisted after Save.

## 7. 2551Q missing-business-start prerequisite — PASS (`7-prereq-business-date.mp4`, fresh fixture TaxpayerDemo)

- Fresh fixture Juana had no business start date; sample was manually saved as 2551Q 2026 Q3 (fixture/setup navigation disclosed).
- Saved form showed `Set business start date` notice.
- Tapping link opened Registered activity editor; date sheet auto-opened on top, with no sheet fight or destructive ordering observed.
- Typed `01/01/2026`, Done, Save.
- Back on form, business-start notice was gone.

## 5. Throughout observations on completed iPhone steps

- No header/grabber clipping observed in completed steps.
- No empty-sheet flash while closing observed in completed steps.
- No wrong check on a recycled row observed; current value check was correct in steps 1/3/6.
- No tap failure while keyboard up observed; taps picked in steps 1/2/3/6.
- One behavior to review, not obviously a defect: after X close in the form RDO picker, the form behind returned scrolled to top/collapsed.

## Run interrupted here

Lead instructed stopping `34c29679` manual evidence before iPad and macOS AFTER work because final AFTER head changed to `a9c9fd7c0a50a3639435dcb3e83c71fdb6f6bdea`. Completed 34c artifacts are supplemental only.
