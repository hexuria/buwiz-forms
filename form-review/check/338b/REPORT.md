# Independent check of pull request #338 at afe7bc34

Commit tested: `afe7bc340c95651029617ed52588484f9c560ba2` (branch head verified before testing). Base for comparisons: `origin/main` = `3aceafe8`. You did not write this pull request; neither did I.

Method: the live agent control plane (`agent::session::Session`, the same copy the NSH app hosts — docs/agent.md) through the headless `bir serve` daemon, driving real `form.fill` invokes over the gpui-agent protocol; each probe on a fresh scratch database (a draft's saved values survive `form.dismiss` + reopen, so probes never share a database). Demo taxpayer Juana Santos only, invented values, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` on every run. GUI proof in the running NSH app on Xvfb, driven the same way while the page was visible.

## 1. The claims, live

### 2552 — exact replies, fresh return each time

| probe | call | result at afe7bc34 |
|---|---|---|
| fresh return, fill Item 12 total shares + Part IV row 2 amount | both keys in one call | both refused: `Needs item 11 = Initial Public Offering (Primary or Secondary)` / `Needs Part IV row 1` |
| Item 11 = LSE + Item 12 in one call | `LSE` filled; Item 12 refused with the same reason |
| Item 11 = Primary + Item 12 + rows 1–2 in one call, choice first | all six filled |
| same, values first (reverse order) | all six filled |
| values-only call, then choice in a later call | first call: Item 12 refused (rows 1–2 filled — row 1 opens row 2 within the call); after the choice call, the same values are accepted |
| Primary + Item 12 filled, then switch to LSE | Item 12's stored value is cleared by the write; a later Item 12 fill is refused again |
| row 1 amount `0` + row 2 in one call | row 1 stored `0.00`, row 2 refused `Needs Part IV row 1` (a zero row does not open the next row) |
| rows filled, then row 1 amount cleared with `""` | clear REFUSED: `` must be an amount such as 1234.50 (got ``) `` — an amount box cannot be blanked through form.fill (same on main: the amount parser, not the gate; a person can backspace the box on the page — pre-existing fill gap, not caused by this PR) |

Official rule: `rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json` — `txtPg1Pt1I12TotShare` `disabled: true` (source line 759), `txtPg2P4I2Amount` `disabled: true` (source line 1327), row 1 enabled. The corpus records no `enabled_when` for 2552; what enables the rows is hash-only in the official scripts, so the IPO/row-order gates are the port's documented inference. The page states match: see the screenshots — note the page renders shut boxes as **absent rows** (Item 12 does not exist on the page before IPO; Part IV shows `No rows yet` with the visible gate text lines `Needs Part IV row 1` …), not as greyed inputs. 2552's Part IV gate text on the page is the same string the refusal carries; Item 12 has no visible reason on the page because the row is not rendered at all.

### 1702EX — exact replies, fresh return

All five boxes refused, each naming its attached schedule:

| box | refusal |
|---|---|
| txtPg3Pt6I17OthersIDesc / IAmt | `` filled from the Schedule 1 Item 17i (more...) attached schedule; BIR's program keeps this row shut `` |
| txtPg3Pt6S2I4Description / LegalBasis / Amount | `` filled from the Schedule 2 Item 4 (more...) attached schedule; BIR's program keeps this row shut `` |

Official rule: `rules/forms/1702ex-v2018c/fixtures/runtime-control-inventory-v796.json` — all five `disabled: true` (source lines 2057, 2064, 2190, 2199, 2203), popup/attached-schedule-owned. Page state matches: greyed rows with lock icons and the visible note `Filled from the attached schedule.` — same meaning, not the identical string the refusal carries. F3 holds, including LegalBasis.

### 1701 — the claim's keys are not boxes; the real gates hold

The three keys the claim and the F2 test use — `frm1701:txtPg2Sched3AI8A`, `frm1701:txtPg2Sched4I3A`, `frm1701:txtPg3Sched6I6A` — **do not exist on form 1701** (not in the agent form definition, not in the official inventory; the strings appear nowhere in the repository except the pull request's own test). Live reply for each: `` `<key>` is not a box on Form 1701; use the keys bir_form_fields lists ``. The F2 test passes vacuously: it asserts only that the refusal string is non-empty. On main the same keys get the same not-a-box refusal, so nothing changed here — but the claim as written is unproven by its own test, and the named boxes are imaginary.

The real 1701 gates, tested live with real keys: on a fresh return the spouse Part IV boxes are refused with the page's reason (`Part IV is for a joint return: choose Married at item 16, Yes at item 17 and Joint Filing at item 18 first.`); one call containing Civil Status = Married + Item 17 = Yes + Item 18 = J + the dependent boxes, in values-first order, chains through the multi-pass loop and fills everything (deep three-level chaining verified). On the page, the spouse part renders as `0 items` when shut — again absent, not greyed.

## 2. What a general change can break

### 2a/2d — ordering and one-call open/shut

Covered in the 2552 table above: one call opening a gate and filling behind it works in either key order; choice in a later call refuses the earlier values (what a person typing gets — the box is shut on the page); a call whose choice shuts the gate (`LSE` + Item 12) refuses the gated value; switching the choice in a later call clears the shut boxes' stored values, leaving nothing in the draft the page would not show.

### 2b — ten-plus forms, same fill at both commits

One `form.fill` per form per commit with identical input (values harvested from the official sample payload of `crates/bir-core/tests/sample-payloads/`), then the final field state compared. Forms: 2551Q, 1701Q, 1701A, 1601C, 0619E, 1601EQ, 0605 (the most-filed), plus 2550M, 2550Q, 2200S, 1701, and the joint samples 1701-joint / 1701A-joint. Per-form transcripts: `fills/<form>-fill-comparison.txt`.

| form | filled 338/main | refused 338/main | validate errors 338/main | final values |
|---|---|---|---|---|
| 2551Q | 16/16 | 3/3 | 5/5 | identical |
| 1701A | 67/67 | 8/8 | 4/4 | identical |
| 1601C | 33/33 | 5/5 | 6/6 | identical |
| 0619E | 5/5 | 3/3 | 4/4 | identical |
| 1601EQ | 8/8 | 4/4 | 6/6 | identical |
| 2550M | 28/28 | 4/4 | 5/5 | identical |
| 2550Q | 32/32 | 14/14 | 4/4 | identical |
| 2200S | 42/42 | 3/3 | 6/6 | identical |
| 1701-joint | 223/223 | 6/6, same keys | — | identical |
| 1701A-joint | 71/71 | 8/8, same keys | — | identical |
| 1701Q | 42/59 | 20/3 | 4/4 | one diff, correct |
| 1701 (single sample) | 208/221 | 19/6 | 4/4 | identical |
| 0605 | 15/16 | 1/0 | 7/7 | identical |

The joint rows are the decisive evidence: where the return genuinely opens its gates, filled and refused sets are **identical** at both commits — the gate check does not wrongly refuse anything an assistant could fill before.

The new refusals in the 1701Q / 1701-single / 0605 rows were each traced: they are boxes whose opener the fill never sets (the sample return itself is single, or non-amended, or non-installment, or my harvesting missed the opening radio) — states in which a person cannot type the box either. Example verified in detail: on 1701Q `txt48Desc` (Schedule II's description) main accepted and stored `INTEREST` while the return's Item 16 is Graduated — the box belongs to the 8% schedule the return does not have. At afe7bc34 it is refused with `Item 48 is Schedule II's: choose the 8% rate at item 16 (or item 25 for the spouse) first.` — correct per the official enabled_when, and the stored value is correctly absent. Main accepting it was the bug this pull request exists to fix. Same shape for the column-B spouse boxes on the single-filer samples, `txt59A` (Needs Amended Return = Yes) on a non-amended sample, and 0605's `txtNumOfInstallment` (Needs item 18 = Installment) when item 18 is not Installment.

No box that a person can type on the page in the fill's state was found refused. For a consistent return the final stored state is byte-identical between commits on 11 of 13 probes; the two differing states (1701Q schedule descriptions) differ because main wrongly kept values behind shut gates. Since the serializers are untouched and the stored state is identical for consistent returns, the uploads are identical.

### 2c — the repository's own tests and recipes

- `cargo test --locked -p bir-app-api --features headless --lib agent::`: 68 passed, 0 failed at afe7bc34 (3 new tests are the pull request's own); 65 passed, 0 failed at origin/main. Nothing that passes on main fails here.
- `agent-recipes/generated_recipe_runs.py` (the 1701Q generated-recipe run over the CSV data set, which fills exactly the gated income boxes through `form.check` inside recipes): identical outcome at both commits — rows 3, 5, 6 pass; row 4 fails with the same `not in recipe: ['prior_credits']` gap on main and at afe7bc34 (pre-existing, unrelated). Logs: `fills/recipe-338.log`, `fills/recipe-main.log`.
- nsh agent tests (`nsh/tests/agent.rs`, `agent_send.rs`): not rerun for this check — they exercise the send guard and window plane, not `form.fill`'s gate logic, which lives in bir-app-api (covered above). Scope note, not a pass.

### 2d leftovers in the draft

Covered in the 2552 table: nothing is left in the draft that the page would not allow — the write that closes a gate clears the dependent boxes itself (Item 12 blank after switching to LSE, verified through form.fields).

## 3. Nothing else moved

- `cargo test --locked -p bir-core --test sample_payloads_test`: **144 passed, 1 ignored** at afe7bc34; **144 passed, 1 ignored** at origin/main. This pull request adds no samples.
- `git diff -w 75dcab06..afe7bc34`: 6 files, all within the claim — `agent/session.rs` (multi-pass gate check + the tests), `screen_form_module.rs` (`page_gate_refusal`), `rows_annual_corporate.rs` (the LegalBasis wait note), its test, `agent-carry.json`, `draft_locks.rs` (the EX attached-schedule lock table). No serializer, probe, payload or other form code touched. The shared-file risk is the multi-pass loop itself, which is what 2b tests.
- `crates/bir-core/data/agent-carry.json`: regenerated by its script, not by hand — `agent_carry_table_matches_the_page_schema` passes at afe7bc34, and the two new `page_writes` entries (`frm1702EX:txtPg3Pt6I17OthersIDesc`, `frm1702EX:txtPg3Pt6S2I4Description`) are exactly what the generator derives once the page writer owns those boxes.

## Verdict for #338: the fixes hold and nothing else is refused wrongly

Faults found, ranked by harm:

| fault | harm | caused by this PR? |
|---|---|---|
| The F2 1701 test uses three keys that do not exist on form 1701; it passes vacuously and the claim's 1701 example names imaginary boxes. The real 1701 gates do hold (verified live), but the test proves nothing about them. | a wrong or missing reason — test quality, not runtime | yes (test added by this PR) |
| An amount box cannot be blanked through form.fill (`""` fails the amount parse), so an assistant cannot clear a filled amount the way a person backspaces it | a return an assistant cannot fully edit in one direction | pre-existing on main (amount parser, unchanged) |
| On 2552/1701 the page renders shut boxes as absent rows, so the page shows no reason for Item 12 at all in the shut state (Part IV does show its gate lines; 1702EX shows `Filled from the attached schedule.`) | cosmetic/informational — a person gets less explanation than the agent's refusal carries | pre-existing page design, not changed by this PR |
| `form.dismiss` with `discard: true` does not reset a draft's previously filled values when the same period is reopened | surprising statefulness for an assistant, same for a person | pre-existing on main |

Within the claim's scope the mechanism does what it says: the same reviewed reason a person would see, one shared mechanism, correct in both key orders, correct when the choice comes later, correct when the same call closes the gate, and — the important part — byte-identical results to main wherever the gates are genuinely open.
