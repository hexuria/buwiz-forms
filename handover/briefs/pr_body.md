All 43 eBIRForms 7.9.6.2.1 forms now send exactly what BIR's own app uploads and queue for submission, including 2550Q (application freeze lifted).

## The upload is `saveEncryptedProfile`, not `saveXMLsubmit`

Every official HTA uploads the output of `saveEncryptedProfile(true)`. That function writes every `frmMain` control in DOM order, mostly raw. #78/#79 targeted `saveXMLsubmit`, which is the eBIRForms Online path. I checked this against two real uploads (1601C, 2551Q), and they match once IE dropping the newline after `<xmp id=xmlFormat>` is modelled.

- **Tooling.** `tools/official-xml` now runs `saveEncryptedProfile(true)` in jsdom and captures the `IAF_RDO_Copy` file. All layouts and samples are regenerated from it.
- **Samples.** Each of the 64 samples under `crates/bir-core/tests/sample-payloads` is byte-identical to the official page's upload. Each pins the SHA-256 of its `.iaf.xml`, checked against an Encrypt.exe emulation (`ours_identical`).

## Generic queue

This branch contains #80 (generic submission queue + 2553). On top of it, every form is moved onto `QueueableForm`/`QueueableKind`, with desktop pages wired through `form_view_specs!`.

## Print preview

- **Print maps.** 43 frozen sheets have a `writer-cells.json` map. 41 of them are new in this PR.
- **New join type.** `bir-print` gains `peso_joins`, for peso combs that have no centavo comb ("Do not enter centavos"). The amount is rounded to whole pesos, with 50 centavos rounding up. This fills the amount boxes on 1701, 1701Q, 1702RT and 1702MX.
- **New test.** `every_print_map_is_registered_and_fills_its_sheet` loads every map and fills its sheet.
- **Desktop.** The 2200A and 2200P pages now open the filled sheet. 1601C and 2550Q pages are now responsive (Phone <700px, Tablet <1100px, Desktop). 1601C, 2550Q and 2551Q were rendered under noVNC at 390/900/1280 px; the fixes that check found are in (status stepper wraps on every form page, 1601C subtitle/toggles, 2551Q Part I title).
- **Revision mismatch: 2200T, 1601EQ, 1601FQ, 1602Q.** Each files an older return than the only frozen sheet we have (2200T: 2022 sheet for the January 2020 return; 1601EQ/1602Q: 2019; 1601FQ: 2020). Their maps exist, but the registry doesn't claim print support for them.
- **Negatives.** Money and whole-peso combs print a minus in the slot left of the pesos, so overpayments no longer print blank.
- **Left blank.** Cells whose caption doesn't identify them clearly stay blank and are listed with a reason in each map's `left_out`. The common cases are split MM/DD/YYYY combs, unlabelled schedule rows, and shared TIN combs.

## Forms Set picker
`FORM_REGISTRY` (the source for the Forms & Elections picker and the Tax Form Library) folded 1604C and 1604F into the pre-2018 combined 1604CF, aliased 1707 to 1707A and had no 1606, so those four pages could not be added or opened. Each now has its own `FormDefinition` (1604-C/1604-F due Jan 31; 1606/1707 per transaction). 1604CF stays readable for old Forms Sets but is hidden from the picker. A test checks that every `QueueableKind::ALL` code resolves and is in the picker.

## Registry and freeze gate

`support_level.rs` sets editor and print flags only where a page or print map exists. The freeze rows in `status.rs` and the generated manifest follow the gate.

### 2550Q: application freeze lifted
- `QueueableForm` for `Form2550QDraft`; the upload appends `<dateFiled>` like the official page. Sample `2550Q-122025Q1` is byte-identical, and its hash is confirmed against the Encrypt.exe emulation.
- Lifecycle moves to `SubmissionLifecycle` (stored field names kept). `recompute()` gains the official 44b/45b/46b and Schedule 2 exempt-sales formulas.
- Gate: capability row 1007 → 1023 (`release_ready` stays false); the closed-action pins become generic-queue shape pins. The authority-token inventory, evaluator surface and cfg(test) guards are unchanged.
- The queue does not apply the import-only "required raw value missing" rule, because the page never captures radio raw values. Raw values that are present are still checked.

## Verification (on `forms/integration`)

| Check | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo test -p bir-core` | lib 1088 passed (11 ignored); `sample_payloads_test` 64 passed; integration suites ok |
| `cargo test -p bir-print` | 75 lib tests, all ok |
| `cargo clippy -p bir-core -- -D warnings` | clean |
| `cargo clippy -p bir-desktop --all-targets --features dev-tools,agent -- -D warnings` | clean |
| `cargo clippy -p bir-desktop -- -D warnings` | clean |
| `bir-rules-codegen status --boundaries-only` | 0 unchecked |
| `bir-rules-codegen check` | passed |

## Known leftovers

- **No PROD SFTP folder.** BIR has none for 2200M, 2200A, 2200P or 2200AN.
- **Per-category layouts.** 1700 and 1600 keep theirs.
- **Wide amounts.** An amount with more digits than its comb still prints blank.
- **Clippy on bir-core test targets.** `cargo clippy -p bir-core --tests -- -D warnings` still fails on lints that already exist on main (e.g. `form_rules/shadow.rs`).

## Status and remaining work (2026-10-11)
- **Render check (390/900/1280 px, isolated Xvfb display):** 36 of 43 forms checked and fixed. Fixes: phone-width label wrap/stack sweep across every page and the shared kit; reading order and item numbering matched to the printed sheets; Part II after Part I; rate items as read-only rows; 1701Q/1702Q show Q1–Q3 only; 2000OT and 2200C wording from the wrong sibling form; 2200C catalog title.
- **Still to render-check:** 2200M, 2200P, 2200S (in progress), 2200T, 2550M, 2552, 2553.
- **bir-desktop tests:** 208 pass. Three stale tests were fixed (1700/1701A `%` parsing, the 2550Q bespoke-page spec). The verify now runs them too.
- **Field-UI audit:** 295 findings across 43 forms, one visual report per form (artifact). Not yet acted on: comboboxes for month/ATC, locked library period, inline validation, growable schedules, Linux print preview via the browser.
- **Open questions for the owner:**
  - 1706 numbering: the rule package and the printed sheet disagree.
  - 1702Q centavos: the printed sheet says whole pesos.
  - Corporations need Tax Classification for 2550Q.
  - Part III / declaration blocks are not rendered on several forms. 1700 has no Part IV fields; 2200AN and 2200C have no "sheets attached" field.
- **Related PRs against `main`:** #84 (dev asset lookup), #85 (Linux SQLCipher key persistence; data loss on every relaunch).

🤖 Generated with [Claude Code](https://claude.com/claude-code)