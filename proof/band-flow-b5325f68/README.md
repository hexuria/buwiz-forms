# Independent band-flow check — b5325f68

Independent validation checkout: `/home/ubuntu/forms-validation/b5325f68`, exact commit `b5325f680f4aa18ad82c73866b8a707d7fe3f7b4`. Baseline: `4321b79be9b177ff55fb2f76a5d9c45734423d86`. Separate target directories were used (`target-b5325f68`, `target-4321b79b`, `target-nsh-b5325f68`), the pinned toolchain's own cargo, empty `RUSTC_WRAPPER`, and pinned NSH `8c78cfd6`.

Safety: the app-door print run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, demo taxpayer Juana Santos and invented values only. Nothing was queued or sent.

## Method and instrumentation

The six print inputs were serialized from the same typed draft constructors and values used by the invariant (`bandflow_proof_drafts`, local uncommitted instrumentation). They were printed through the NSH app door host `screen_form_print_sample`, which uses the app's print-map/static-print-preview pipeline. The NSH print hook normally reads and deletes the transient preview file. To inspect the actual output, the validation checkout had one local uncommitted proof-only copy line gated by `BANDFLOW_PROOF_COPY_DIR`; it is not product code and is disclosed here. This was a host-function print-path check, not a click-through UI test.

## Invariant test

- At `b5325f68`: PASS, 15/15 (`logs/invariant-b5325f68-rerun.log`).
- At `4321b79b`: FAIL, 14/15; only `invariant_1801_schedule1` passed (`logs/invariant-4321b79b-rerun.log`). The baseline does not contain the test, so the target test file was copied verbatim there as local uncommitted instrumentation.

The test does count row markers on the text a reader sees after stripping tags and joining input values/comb text, not by counting raw HTML row elements. It checks exact-once occurrence, order, totals, nonblank page fragments, continuation identity/page markers, and rejects placeholder/fixed-height spill text. Important limitation: it does not prove continuous printed row numbering for every form; only 1702Q has explicit continuation-number assertions. It also reasons about HTML text, not final PDF clipping; visual PDF inspection below is therefore necessary.

## App-print visual results at capacity+3

Programmatic corroboration of the actual app output (`summary.txt`, `analysis.json`):

```
1801: pages=3 rows_once=True order=True total=True no_blank=True cont_identity=True
1707A: pages=3 rows_once=True order=True total=True no_blank=True cont_identity=True
2200P: pages=3 rows_once=True order=True total=True no_blank=True cont_identity=True
1601C: pages=2 rows_once=True order=True total=True no_blank=True cont_identity=True
2551Q: pages=2 rows_once=True order=True total=True no_blank=True cont_identity=True
1702MX: pages=5 rows_once=True order=True total=True no_blank=True cont_identity=True
```

Visual observations from the PDFs/contact sheets/crops:

- **1707A**: page 3 is an additional sheet with TIN `000-123-453-00000`, `SANTOS, JUANA`, return period, `ADDITIONAL SHEET — PAGE 3`, repeated Schedule 1 heads, rows 4–7 once and in order. The official page-2 total row carries capital gains `28,000.00` and tax paid `2,800.00` in columns E/F; it does not total selling price/cost there. No blank page or clipping seen.
- **2200P**: page 3 has the same kind of identity strip/page heading and repeated 15-column heads. Rows 28–30 print once, all columns fit across the width, Applicable Rate is not summed, Subtotal is `8,700.00`, and TOTAL TAX DUE is `46,500.00`. However the continuation `#` cells are numbered `1–3` rather than continuously as `28–30` (the heading says “Rows 1–3 (continuing after 27 printed rows)”). The table is not clipped, but the 6 pt font finding below stands.
- **1702MX**: page 5 is an additional sheet with identity/page/repeated heads. It prints MORE rows once and in order as printed positions 5–8 after paper positions 1–4; subtotal row is present. No blank page or clipping seen.
- **1601C**: capacity+3 exactly fills the paper's own continuation rows `3a`, `3b`, `3c` on page 2; no separate additional sheet is expected at this count. Those rows visibly carry Previous Month/s and Date Paid combs. Rows are once/in order and total adjustment `3,000.00` prints in the official total row. No blank page or clipping seen.
- **2551Q**: capacity+3 exactly fills the paper's own continuation rows `6a`, `6b`, `6c` on page 2; no separate additional sheet is expected at this count. Continued rates visibly print `3%`, `7%`, `3%` in the same style as paper rows. Rows are once/in order; total tax due `3,284.19` prints in the official total row. No blank page or clipping seen.
- **1801**: all five real-property rows print once on page 2 and the total `750,000.00` is correct. The output also has a page 3 containing continued body Schedule 5 zero rows, not a property-row continuation table. This means capacity+3 did not demonstrate property rows moving to a row continuation sheet in the app print; page 3 is nevertheless nonblank and carries identity. Treat the broad “rows flow onto continuation pages” claim as not demonstrated for 1801 at capacity+3, even though exact-once/order/total pass.

## 2200P font-size check

The claim does **not** hold as worded. Source evidence: `crates/bir-print/src/continuation.rs` chooses `font-size: 6pt` for wide continuation tables (`>=10` columns); 2200P has 15 columns. The frozen official sheet's page-2 Schedule 1 row inputs use `fh*` classes defined at `10.56pt` in `html-frozen/2200p-2020/form.css` (for example `.fh16`). Therefore 6 pt is smaller than the paper's own row-entry text size, not the paper's own size, and it is below the 7.5 pt floor stated in the print rules. Visually the table is legible only when zoomed and fits without clipping.

## Battery at b5325f68

- Workspace `cargo test --locked --workspace`: PASS; bir-core lib **1594 passed / 11 ignored**, sample payloads **142 passed / 1 ignored**, bir-print lib **177 passed / 1 ignored** plus band-flow integration suites blank-pages 1, fixes3 4, invariant 15, rank1 6, sweep3 7; bir-app-api normal lib **818 passed / 7 ignored**.
- 43-form audit: PASS, `audit_all_screen_forms` 1/1.
- Agent-focused feature test `cargo test --locked -p bir-app-api --features headless --lib agent::`: PASS, **65 passed** (760 filtered out). The author's “agent 806” label does not match this command/count; see the full headless log if present.
- Freeze verify: PASS, `OK verify 43 bundles at html-frozen` using `hexuria/buwiz-forms-html` `12171d975b65a6faab141c82c6540bc8d8b38e2a` as CI does.
- Stamps self-test: PASS, `OK self-test`.
- Stamps check-all: PASS.
- Formatting: PASS, `cargo fmt --all -- --check` after formatting only local proof instrumentation.
- Clippy: PASS, `cargo clippy --locked --workspace -- -D warnings`.

## Verdicts on the author's specific claims

- Invariant covers 15 forms at capacity/capacity+3/capacity+40 and passes at target: **holds**, with the limitations above.
- It fails in 14 of 15 at `4321b79b`: **holds exactly** under the stated copied-test method.
- 1702Q duplicate rows fixed: **supported** by target invariant pass and baseline failure; no separate app visual was requested/produced for 1702Q.
- Page 2+ identity strip duplicating body schedules fixed: **supported** by invariant and the clean generated additional sheets inspected.
- 1707A official total is E/F only: **holds visually**.
- 2200A/2200P no longer sum rate column: **supported**; 2200P visual subtotal leaves Applicable Rate unsummed.
- 2200P 15-column table no longer clipped: **holds visually**; it fits width.
- “it now prints at 6 pt” / paper-size implication: **does not hold**; 6 pt is below both the paper row-entry size (10.56 pt) and the 7.5 pt floor.
- 1601C continued rows carry Date Paid: **holds visually**.
- 2551Q continued rates print “3” like paper rows: **holds visually** for rows 6b/6c; row 6a correctly shows 7% because the constructed seventh row used PT103.
- Battery green: **mostly holds**; all independently completed suites passed, but the “agent 806” count was not reproduced as stated (agent-filtered run was 65 tests; normal lib was 818).
- Broad requirement that rows be numbered continuously: **does not hold universally**; 2200P continuation numbering restarts at 1–3.
- Broad requirement that rows flow onto continuation pages: **not demonstrated universally at capacity+3**; 1601C/2551Q use the paper's own continuation rows, and 1801 kept the five property rows on page 2 while page 3 carried unrelated Schedule 5 continuation content.

## Evidence map

- `pdfs/`, `images/*contact.png`: every rendered page for all six forms.
- `images/*header.png`, `images/*totals.png`, `images/*schedule-zoom.png`: continuation/header/total crops.
- `html/`: exact app print-preview HTML copies inspected.
- `analysis.json`, `summary.txt`, `render-summary.json`: programmatic corroboration.
- `logs/`: exact test, battery, draft-generation, app-print, analysis and rendering logs.
- `evidence.html`, `video/band-flow-evidence.mp4`: a recorded desktop walkthrough of the visual evidence. There were no interactive taps because the validated path was the app print host function.
