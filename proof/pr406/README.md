# PR406 Apple proof — a9c9fd7c

App commit `a9c9fd7c0a50a3639435dcb3e83c71fdb6f6bdea` (`34c29679` with main `642bfc78` merged). NSH pin verified as `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; cargo `1.99.0 (5f94df478 2026-08-27)`; target folder `~/BuwizNsh/target-a9c9fd7c`.

Safety: scratch databases only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`SIMCTL_CHILD_*` on Simulators); `BIR_SFTP_LIVE` never set; Queue never pressed; no real account or service. Input evidence used real Simulator-window input or real Mac mouse/keyboard, not gpui-agent typing/paste synthesis. Connect Hardware Keyboard was OFF for iPhone/iPad evidence.

## Merge-condition result matrix

| device/orientation | owner RDO | owner ZIP digits + letters/`makati` | result |
|---|---|---|---|
| iPhone 17 Pro | PASS: search pinned, scrolled to end above keyboard, row picked | PASS: digits-first keyboard, ABC letters, `makati` rows, row picked | PASS |
| iPhone SE (3rd generation) | PASS: search pinned, end reachable, row picked | PASS: digits-first keyboard, ABC letters, `makati` rows, row picked | PASS |
| iPad Pro 13 portrait | PASS after settling: search pinned, end reachable, row picked | **FAIL**: digits/`12` pick passed, but correct lowercase `makati` showed `Nothing matches` | **FAIL** |
| iPad Pro 13 landscape | PASS: search pinned, end reachable, row picked | PASS: digits-first keyboard, ABC letters, `makati` rows, row picked | PASS |

Opening-transient observations: iPhone 17 Pro and iPad portrait each showed one initial clipped-header/grabber frame on first RDO opening, then the sheet settled after typing. See `iphone/after/rdo-open-transient-clipped.png` and `ipad/after/portrait-rdo-open-clipped.png`.

## 1701Q and 5–12-choice dropdown at a9c9fd7c

| check | result |
|---|---|
| iPhone 17 Pro, 1701Q item “RDO Code” | PASS — full search picker; `050`, clear, corrected exact `makati`, end visible, picked `050 — South Makati`, persisted after leave/return |
| iPad Pro 13, 1701Q item “RDO Code” | PASS — full search picker; `050`, clear, `makati`, end visible, picked `050 — South Makati`, persisted after leave/return |
| iPhone 17 Pro, six-choice ATC dropdown, real Simulator click | PASS — real click picked “Business Income - Graduated IT Rates” and the form retained it |

The iPhone exact 1701Q recording visibly includes one intermediate mis-typed `majati`, corrected to exact `makati` during the same real-input run.

## macOS desktop at a9c9fd7c

PASS in a 1000×800 window (>600 pt), production `App.tsx`, scratch DB:

- RDO typed `05`; Down/Up moved highlight; Enter picked `005 — Alaminos City`.
- Chevron reopened full list with current value checked/in view; Escape closed it.
- `xyz` then Tab reverted with the sentence under the box.
- ZIP typed `12`, real scrolling reached final row `9712 — Taraka`, and picking populated the box.

See `mac/after/comboboxes.mp4`, screenshots, and `mac/after/notes.md`. Mac BEFORE evidence at main `44ccfd80` is in `mac/before/`.

## macOS suites at a9c9fd7c

| suite/step | result |
|---|---|
| `just nsh-check -v` | PASS |
| `just nsh-build` | PASS |
| app-crate Rust tests | PASS |
| agent feature build | PASS |
| agent + agent_send tests | PASS |
| agent cargo-tree gate | PASS |
| iOS-simulator agent compile check | PASS |
| iPhone compile check | PASS |
| headless TSX screen suite | **1898 passed, 1 failed** across 202 files: 201 files had passing summaries; one file failed |

Failing test, reproducible on isolated rerun:

- `screens/form/FormPageNoRdo.test.tsx › desktop: 1601C with no profile RDO lists the real RDOs and not 000, and takes a pick`
- failure: condition never held in 10000 ms; visible text included `RDO Code *` and `From tax profile · 5 filled boxes` rather than the expected full catalog state.

Named files in the passing screen-suite shards:

- `screens/form/controls/pick.test.tsx`: 12/12 passed
- `screens/form/controls/controls.test.tsx`: 31/31 passed
- `screens/form/controls/model.test.ts`: 15/15 passed
- `screens/taxpayer/profileDates.test.tsx`: 2/2 passed
- `screens/taxpayer/profileDates.store.test.ts`: 3/3 passed
- `screens/taxpayer/profile.test.tsx`: 8/8 passed in the named desktop file
- `screens/taxpayer/logic.test.ts`: 9/9 passed

CI status output and the isolated failing-test rerun are in `ci/`.

## BEFORE evidence

Key BEFORE stills are under `iphone/before/` and `ipad/before/`; `before-README.md` and `before-README-REPORT.md` are copied from the pushed BEFORE proof. Full BEFORE videos remain in branch `try/ios-searchpick-repro-44ccfd80` under `proof/searchpick-repro/` rather than duplicating all of them here.

## Supplemental 34c29679 evidence

`supplemental-34c/iphone/after/` preserves the completed pre-merge iPhone evidence for RDO, ZIP, owner end-scroll, form RDO, profile/business dates, and the 2551Q missing-business-start-date flow. It is supplemental because final merge proof was rerun at `a9c9fd7c` as ordered; see `supplemental-34c/iphone/after/notes.md` for exact results.
