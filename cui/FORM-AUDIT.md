# All-43-Forms Correctness Audit — CUI vs NSH reference

**Scope:** field-level and validation-level comparison of every form in the CUI app
(`cui/`, branch `feat/cui-forms-port`) against the NSH reference implementation
(`nsh/`, PR #221, hexuria/buwiz-forms). Read-only audit — **no code was changed**.

**Reference provenance (important limitation):**
`https://github.com/betterebirforms/ebirforms` could **not** be cloned — GitHub
rejects both `git clone` ("Password authentication is not supported") and the API
(HTTP 401, invalid `GH_TOKEN`). The comparison is therefore against the NSH
implementation in this repo (`nsh/`), which is itself the reference port of that
project, plus the host-side schema/engine (`app/forms/forms-schema.json`,
`screen_form_validate`, conditions engine) that **both** apps share. All field
metadata below is taken verbatim from `forms-schema.json` (43 forms, 2,186 items,
8,820 fields), so "missing fields" means missing *rendering/handling*, not missing
schema data — the schemas are identical for both apps.

**Method:** static trace of `nsh/screens/form/*` vs `cui/src/screens/form.rs` +
`cui/src/form/{session,model,schema}.rs`, cross-checked against
`rules/forms/*/validations.json` (note: those files are an *audit/evidence*
corpus, not runtime rules — runtime validation is host-side and shared), and the
existing live audit (`audit-forms.sh`, 43/43 PASS on `:1`) plus the session test
`all_form_sessions_open_edit_and_check` (all 43 open/edit/validate/save as sample
and real return).

---

## CONFIRMED INCONSISTENCIES

### I-1. "Additional details" fold missing — 233 items / 2,133 fields
NSH (`rows.tsx`): items whose label is exactly `"Additional details"` render as a
collapsed toggle — `"N more fields not yet placed on the paper layout"` — behind
`s.moreOpen`. These are meta fields (e.g. `ebirOnlineSecret`, `txtFinalFlag`,
`ebirOnlineUsername`) deliberately hidden from the paper flow.

CUI: `draw_item` has **no fold**. `session.rs` still contains
`more_open`/`toggle_more`/`more_open()` (dead code, never called from
`screens/form.rs`), so all 2,133 fields render expanded inline. Users see every
meta field stacked at full width instead of one collapsed row.

Per-form field counts under this fold:
0605:6, 0619E:8, 1600PT:21, 1600VT:26, 1600WP:76, 1601FQ:4, 1602Q:59, 1603Q:4,
1604C:7, 1604E:14, 1604F:17, 1606:26, 1700:115, 1701:**373**, 1701A:55,
1701MS:92, 1702EX:22, 1702MX:91, 1702RT:27, 1706:46, 1707:72, 1707A:51,
1800:117, 1801:81, 2000:104, 2000OT:101, 2200A:26, 2200AN:20, 2200C:34,
2200M:32, 2200P:42, 2200S:20, 2200T:17, 2550Q:105, 2551Q:53, 2552:**169**.
(2550M, 2550Q-v2007, 2553, 1601EQ, 1601C, 1601FQ-partial, 1702Q, 2551Q-others have
none.)

### I-2. TIN items not segmented — 68 items
NSH (`rows.tsx` + `controls/model.ts` `tinKind`, `schema.ts` `isTinItem`): items
that are a taxpayer TIN render as segmented 3-3-3-5 (or branch-code) boxes —
58/76-unit boxes joined by "-" separators, or a single `TinInput` for a 14-digit
"full" TIN. `is_tin_item`/`tin_kind` **are** ported in CUI
(`schema.rs:220`, `model.rs:87`, used by `session.rs:4721,5315` for focus/validation)
but **`draw_item`/`draw_field` never branch on them** — every segment renders as a
plain full-width `draw_text` with only a placeholder.

Affected forms (TIN item count): 0605, 0619E, 0619F, 1600PT, 1600VT(2), 1600WP,
1601C(2), 1601EQ, 1601FQ, 1602Q, 1603Q, 1604C, 1604E, 1604F(2), 1606(2), 1700,
1701(2), 1701A(2), 1701MS, 1701Q(2), 1702EX(3), 1702MX(4), 1702Q(2), 1702RT(5),
1706(2), 1707A, 1800, 1801(3), 2000, 2000OT(2), 2200AN, 2200C(2), 2200M(2),
2200P(2), 2200S, 2200T(3), 2550M, 2550Q(2), 2551Q, 2552, 2553 — **37 forms**.

### I-3. Multi-field row layout + sub-labels
NSH (`rows.tsx`): an item's boxes are packed into horizontal flex rows
(`FIELD_ROW_WRAP`); each box carries its own small label above it (`RowLabel`),
so 0605 item 4 (Due Date month/day/year) renders as one row of three labeled
boxes.

CUI: `draw_item` stacks **every field vertically full-width**, with the field
label only as the input *placeholder* — which disappears once the box has text.
No per-box sub-label, no horizontal packing. This affects essentially every item
with ≥2 non-radio fields (dates split into M/D/Y, names, addresses, TIN segments,
schedule-adjacent fields). Functionally editable but visually/structurally
different from NSH.

### I-4. Per-field invalid state not rendered
NSH: a field whose value fails validation gets `invalid` on the control → red
border on the box itself (`FieldControl.tsx` `box.invalid()`), in addition to
the item-level message.

CUI: errors bind to the *item* via `bind_error` → `row_info.message` → one
danger-colored message line under the item. **No per-box error styling.** The
user cannot see *which* of several boxes in an item is wrong without trial.
Validation *content* is identical (same host `screen_form_validate`), only the
visual mapping differs.

### I-5. `maxLength` (`f.m`) not enforced on text-like inputs
NSH (`FieldControl.tsx`): `maxLength={field.m}` on every typed box — text,
email, phone, money text entry, TIN parts.

CUI: masks exist only for `money`/`pct`/`int` (`mask::mask_for`); `text`,
`email`, `phone`, `sel`-typed text, and TIN segments accept unbounded input.
Host validation may still reject overlong values, but the *input constraint*
NSH enforces client-side is absent. Affects all 3,245 text + 26 email + 12
phone fields (and every `m`-bearing money/text field during typing).

### I-6. Searchable select missing for long lists
NSH (`controls/Pick.tsx`, `SEARCH_FROM = 12`): a `sel` with ≥12 options opens a
`SearchPick` with a filter box. CUI: `draw_select` always renders the plain
dropdown — no search. Affects 7 select fields: 0605(1), 0619E(1), 0619F(1),
1601C(1), 1701MS(1), 2550M(1), 2553(1). (Small count, but real UX divergence.)

### I-7. Date asks render as plain text inputs
NSH (`session.ts:1745`, `FormPage.tsx`): an ask with `control === "date"` gets a
date placeholder and 140pt width. CUI `draw_asks` only special-cases
`control == "choice"`; everything else is a generic `text_input` — no
date-shaped placeholder/hint. Session *value handling* is correct
(`session.rs:2921,5543` handles `control == "date"`).

### I-8. Item number chip missing on cards
NSH row shows the printed item number ("9", "13"…) as a leading chip next to the
label. CUI `draw_item` renders only `it.l` via `section_title`; `it.no` appears
only inside the pill's step label (`"Item X · label"`, `form.rs:1371`) and
navigator. The paper item number is not visible on the field card itself.

### I-9. Election choices lack hint text
NSH (`controls/choice`, `FormPage` election): each income-tax-election option
carries a descriptive hint line. CUI `draw_election` shows label + state chip
only. Cosmetic but user-facing.

### I-10. Read-only box presentation simplified
NSH `ReadBox` (`rows.tsx`): lock icon, computed values semibold, `₱`/`%` unit
suffixes, a "growing" hint for auto-calculated fields, and the *reason* as a
tooltip. CUI renders `"{label}: {value}"` meta text — values **are** correctly
formatted (`shown_text`: money grouped + whole-peso aware, `shown_date`, sel
option label — verified `session.rs:2084-2117`), but no lock icon, no unit
suffix, reason shown inline rather than as hint. Functional parity, visual gap.

### I-11. `chk`/`radio` "Y" value not recognized (confirmed at draw layer)
NSH `isOn` (`controls/model.ts`): ticked = `"true" | "1" | "Y"`. The host
(`app/shell/form_page.js:2887`, `todo.js:43`) and our session model
(`model.rs:75` `is_on`) all treat `"Y"` as on — **but the draw layer does not**:
`draw_item` chk (`form.rs:2046`), `draw_radio` (`form.rs:2181`) and schedule
checkbox cells (`form.rs:2476`) check only `== "true" || == "1"` inline.
A field holding `"Y"` renders unticked in CUI while NSH shows it ticked.
Whether any field actually stores `"Y"` is unverified — flagged for the
validating agent to check against host data.

### I-12. Locked radio choices look clickable
NSH: `choiceLocked` dims individual radio options (e.g. period-locked options)
with an off-reason. CUI `pick_radio` correctly **rejects** shut keys at session
level (`session.rs:1470`), but the checkbox still renders enabled — clicking
does nothing silently. Functional behavior correct; affordance missing.

---

## VERIFIED CONSISTENT (no discrepancy found)

- **Field inventory:** all 8,820 schema fields exist in `forms-schema.json`
  (shared by both apps). No schema field is dropped by the CUI renderer except
  intentional skips: `t == "hidden"` and `t == "radio"` (radios re-emerge as
  grouped controls). `withoutHidden`-equivalent filtering is host-side.
- **Schedule grids:** `shown_rows` = filled rows + first empty row — exact port
  of NSH `shownRows` (`form.rs:2326`). "Add row" opens first empty row; Delete
  clears cells and closes; all-computed tables render totals cards; row drawer
  = NSH Sheet with Delete/Done. Verified live on 2550Q.
- **one-date / split-date:** month/year/day composite fields, calendar popover
  with `dateRange` (min/max from locked companion fields) — matches NSH
  `dateRange`. Verified live (1701 DOB, 1601C month/year).
- **Read-only formatting:** `shown_text` formats money (whole-peso flag
  `whole_keys` honored), dates (`shown_date`), selects (option label) — same
  output as NSH `readText`.
- **Radio groups:** `grp` grouping, `rg >= 3` uncheck-to-clear semantics in
  `pick_radio`, per-item options from `o`/`a` — ported.
- **Conditions:** `conditions.json` (27 forms) evaluated host-side — shared
  engine, same visibility/enabled results for both apps.
- **Validation rules:** runtime validation is host-side `screen_form_validate`
  (same call for NSH and CUI). CUI surfaces errors via `bind_error` →
  item-level messages, merges `fact_errors`, suppresses quiet fields, and the
  session test reaches `save=Saved` on all 43. `rules/forms/*/validations.json`
  are evidence/audit packets, not runtime rules — nothing to port. **Gap is
  only I-4 (per-box styling), not rule content.**
- **Kiosk/asks/elections:** ask choice rendering + `save_ask`; income-tax
  election (`2551Q graduated/flat`) flows through session and was exercised in
  the all-43 real-return test.
- **Autosave/save/check/print/queue:** session states and host calls shared;
  print preview honestly reports unavailable off-macOS (same as NSH).
- **`email`/`phone` kinds:** NSH renders them through the same `TextField`
  (`keyboardType`/`autoCapitalize` are mobile-only props — no desktop effect).
  CUI's generic `draw_text` is **equivalent on desktop**; only relevant combined
  with I-5 (no maxLength).

---

## PER-FORM STATUS

All 43 forms share the single `draw_page`/`draw_item`/`draw_field` renderer, so
every inconsistency above applies wherever the construct appears. No form is
missing whole sections/items/fields at the schema level (the schema is the same
file NSH consumes). Forms with **zero** Additional-details fields and no other
special constructs are visually closest to NSH; the heaviest divergences are
1701 (373 folded fields), 2552 (169), 1800 (117), 1700 (115), 2550Q (105),
2000OT (101), 2000 (104), 1701MS (92), 1702MX (91), 1801 (81), 1600WP (76),
1707 (72).

## Recommended fix order (for the follow-up agent)

1. **I-1** Additional-details fold (biggest visual gap; session API already exists).
2. **I-2** TIN segmented rendering (37 forms).
3. **I-3** multi-field rows + sub-labels (affects nearly every form).
4. **I-4** per-field invalid styling; **I-5** maxLength enforcement.
5. **I-6…I-12** smaller items.

*Audit performed on `feat/cui-forms-port` @ `aaa5a31`; 211 tests green.*
No application code was modified during this audit.


---

# SECTION B — CROSS-REPO FIELD DIFF vs betterebirforms/ebirforms + official-xml

**Reference obtained:** `https://github.com/betterebirforms/ebirforms` cloned via
codeload tarball (git auth is broken in this env; the repo is public). It is a
rebuild of BIR's *original* Offline eBIRForms — `src/form/fields/<CODE>.json`
(BIR field ids + kinds, 70 files) and `src/form/template/<CODE>.json` (BIR
savefile templates, 84 files) plus per-form `validate()` functions.

**Second ground truth found in our own repo:**
`crates/bir-core/data/official-xml/<form-id>.json` — generated by running BIR's
*official HTA* saveXML loop in jsdom (sha256-pinned per form). Used as the
primary key inventory; betterebirforms fields/*.json used for kind/required
cross-checks.

## B-1. Missing UI fields (schema-level, confirmed)

Fields present in BIR's official save contract but with **no field in
`forms-schema.json`** (so nothing renders). Classified by engine behavior:

### B-1a. Page-2 filer block (`txtPg2TIN1-3`, `txtPg2BranchCode`,
`txtPg2TaxpayerName`) — engine fills from taxpayer profile, no UI
- **1701Q**: 5 keys missing from schema; `form_1701q*.rs` writes them from
  profile/spouse data. Note our schema DOES have the separate spouse block
  (`txtSpouseTIN*`, `txtSpouseName`, …) — the missing piece is the Pg2
  *taxpayer* block, a different section.
- **2551Q**: same 5 keys; `form_2551q_xml.rs:245-251` writes them. Our 2551Q
  schema has **no spouse/Pg2 section at all** (items run 1-24 then
  "Additional details"). betterebirforms `fields/2551Qv2018.json` lists all 5.
  conditions.json has no 2551Q entry → not conditionally hidden.
- (Same pattern likely explains other `Pg2*` absences — see B-1c.)

### B-1b. RDO code fields — profile-sourced everywhere (design, but inconsistent UX)
Every form engine fills RDO from `profile.rdo_code` (`form_*.rs` `put(...)`),
but only **some** forms also expose a UI field. Official has an RDO control on
all of these; ours shows it on ~half:
- **No RDO UI field:** 1604C (`rdoCode`), 1606 (`txtRDOCode16A`), 1706
  (`txtRDOCode14A`), 1707 (`rdoPg1Pt1I5RDO`), 1707A (`rdoI6RDO` + `hdnI5TIN4/P2`,
  `hdnRDO` hidden), 1702EX (`rdoPg1Pt1I7RDO` + hdn), 1702MX
  (`drpPg1Pt1I7RDO`), 1701 (`txtPg1I5RDOCode` + `txtPg2I2SpouseRDOCode`),
  1701A (`txtRDOCode` + `txtSpouseRDOCode`), 2000 (`txtRDOCode`), 1700
  (`txtRDOCode` + `txtSpouseRDOCode`).
- **Has RDO UI field:** 1601C/EQ/FQ, 1603Q, 1604E, 1701Q, 1702Q, 1800, 2000OT,
  2550M, 2551Q, 2552, 2553, 0605, 0619E/F, 1600PT/VT/VT/WP…
- Verdict: functional (profile is authoritative) but **inconsistent** — a filer
  cannot see/override RDO on half the forms where BIR prints it.

### B-1c. Second address / name line — engine writes "" (data cannot be entered)
- **1601EQ, 1601FQ, 1603Q, 1604E, 2000, 2000OT, 1701Q**: official
  `txtAddress2` (and `txtOtherName2` on 2000/2000OT) — engines do
  `put("txtAddress2", String::new())` or omit it. Our single address field
  cannot supply line 2 → **second address line is unenterable** (1701Q is excused:
  BIR's own HTA merges the lines — `form_1701q_xml.rs:43`).
- **1800**: `txtRegAddress2`/`txtResAddress2` — engine *does* split
  (`form_1800.rs:370-376`), covered if source field is long enough.
- **1701/1701A**: have explicit `txtPg1I9Address2`/`txtAddress2` UI fields — fine.

### B-1d. 1700 — wholesale key-namespace difference (UNRESOLVED, needs label-level check)
Our 1700 uses an entirely different key scheme (`AmendedRtn_1`,
`txtPg2Pt4I3EMPTIN1{}`, `modalTable1:*`…) vs official `frm1700:rdoPg1I2*Yes/No`,
`txtPg1I*`, `txtPg2I*` — 181 official keys unmatched, 330 ours unmatched.
Engine maps on save (`form_1700.rs:656` writes `rdoPg1I2AmendedYes/No`), and our
schema has *more* keys (362 vs 188), so coverage is plausible — but **field-level
parity for 1700 is NOT proven by key diff**. Flagged for the validating agent:
compare 1700 item-by-item on paper labels.

### B-1e. Single-field gaps (confirmed absent, engine writes fixed/computed)
- **1707**: `txtPg2P4Sch2NoOfShares1` — ours has NoOfShares2/3/4 + StockCertNo1-4
  + NameOfCorpStock1-4 but **row 1's shares field is missing**.
- **1702Q**: `txtLOB` (line of business) — engine fills from
  `profile.line_of_business` (`form_1702q.rs:1027`); no UI. (1707 *does* show a
  `txtLOB` field → inconsistent.)
- **1702MX**: `drpPg3Sc1I11CB` — special-rate dropdown BIR rebuilds from item 4 +
  ATC; our engine writes a default, no UI.
- **1702RT**: `txtPg4Sc5I1NetIncome` — engine computes/writes it
  (`form_1702rt_xml.rs:676`); no UI (likely computed — verify).
- **2550M**: `txtEmail` — no email field anywhere in our 2550M schema.
- **2550M ATC**: official `AtcCode1..36` are bool flags; ours are 36 `sel`
  dropdowns `txtAtcCde1..36` — engine converts (`form_2550m.rs:1162`). Covered,
  different control kind.
- **2000/1800**: `numOfDays`, `numOfMonths(131/132)`, `modLabel` — computed
  penalty/attachment internals the engine writes (`form_2000.rs:1029,1525`);
  no UI needed. NOT missing.
- **1702EX**: `Pg3Pt6S*S*PopLength` — hidden popup-length plumbing. NOT missing.
- **1707A**: `hdnI5TIN4/P2`, `hdnRDO` — hidden fields. NOT missing.

## B-2. Required-flag diffs (betterebirforms `required: true` overrides vs our `r`)
Extraction covered ~30 forms (per-form override maps; several forms —
0605, 1600PT, 1700, 1701, 1701MS, 1702EX/MX/RT, 2200S — had no extractable
required map → not comparable). Results:
- **1707A**: `txtI5TIN1`, `txtI5TIN2`, `txtI5TIN3`, `txtI5BranchMask` are
  ref-required but **optional in ours** (fields exist).
- **2552**: `txtPg1TIN1..4` ref-required, ours optional.
- **2200AN**: `txtMonth`, `txtYear` ref-required, ours optional.
- All other comparable forms: **0 diffs**.
- Note: our `r` flags feed UI required markers; actual blocking validation is
  host-side `screen_form_validate` — these diffs mean the *marker* may be
  missing, not necessarily the check.

## B-3. Kind diffs (betterebirforms FieldKind vs ours)
Their taxonomy is coarse (text/money/select/radio/checkbox); `money`/`date`/
`int`/`pct`/`email`/`phone` all map to their `text` — not mismatches.
Control-kind mismatches: only **3**, all RDO fields ours=`sel` vs theirs=`text`
(1600PT, 1600VT, 1600WP `txtRDOCode`/`txtPg1I6RDO`) — ours is arguably better.

## B-4. Validation comparison
- betterebirforms: per-form `validate()` in entry/*.ts + `validateRequired`
  (required flags above) + custom checks (e.g. 1701v2018 TIN check-digit,
  year>=2018).
- Ours: host-side `screen_form_validate` via bir-core rules; the
  `rules/forms/*/validations.json` files are evidence/audit packets.
- All 43 reach `save=Saved` in the session audit. **Full rule-by-rule
  validation diff was NOT completed** — it requires comparing bir-core
  validators against their TS validate functions; flagged as remaining work
  for the validating agent. Spot-verified: TIN check-digit and period
  validation exist host-side.

## B-5. Per-form missing-field summary (schema-level)
| Form | Missing UI fields (excluding profile-filled/computed) |
|---|---|
| 1701Q | Pg2 block ×5 (profile-filled), txtAddress2 (BIR merges — OK) |
| 2551Q | Pg2 block ×5 (profile-filled) — no spouse/Pg2 section at all |
| 1700 | unverifiable by key diff (namespace rename) — 181 keys to label-check |
| 1707 | NoOfShares1 (1 cell) |
| 1702Q | txtLOB (profile-filled) |
| 1702MX | drpPg3Sc1I11CB (engine default) |
| 1702RT | txtPg4Sc5I1NetIncome (engine-computed?) |
| 2550M | txtEmail |
| 2000/2000OT | txtAddress2, txtOtherName2 (engine writes "") |
| 1601EQ/1601FQ/1603Q/1604E | txtAddress2 (engine writes "") |
| ~15 forms | RDO UI field absent (profile-filled; inconsistent vs forms that show it) |
| 1707A, 2552, 2200AN | required-flag gaps on existing fields |

*No application code was modified. All findings reproducible from
`app/forms/forms-schema.json`, `crates/bir-core/data/official-xml/`,
`/tmp/ebirforms-main/src/form/{fields,template}/`, and `crates/bir-core/src/forms/`.*

# ROUND 2 — INDEPENDENT VERIFICATION (effective schemas + subagent cross-checks)

Round 2 re-did the field diff against the **effective** schema — the payload
`screen_form_schema` actually returns after `schema-overrides.json` `add:`/`move:`/
`table:`/`hide:` processing — dumped live via a temporary test to
`/tmp/effective-schemas/<CODE>.json` (43 files). Round 1 compared raw
`forms-schema.json` and **over-counted missing fields**: the extractor missed
controls that overrides inject. Four independent subagents re-verified claims
without sight of each other's conclusions.

## R-1. REFUTED round-1 findings (corrected)

| Round-1 claim | Verdict | Evidence |
|---|---|---|
| B-1b: ~15 forms have no RDO UI field | **REFUTED** — all 43 have RDO controls | `add:` overrides inject `t:"sel"` RDO fields bound `/rdo_code` for 1604C, 1701, 1701A, 1702EX/MX/RT, 1706, 1606, 1707, 1707A, 2000; 1700 via `screen-1700-v2018.json` (`txtRDOCode`, `txtSpouseRDOCode`). Residual nuance: bindings carry `lock:"profile"` — shown but profile-sourced. |
| B-1e: 1707 `NoOfShares1` missing | **REFUTED** — `add:3:0` injects `frm1707:txtPg2P4Sch2NoOfShares1` (`t:"int"`); layout grid has all 4 rows; engine writes all 4 (`form_1707.rs:687-702`). |
| B-1e: 1702RT `txtPg4Sc5I1NetIncome` missing | **REFUTED** — `add:11:0` injects it (`t:"money"`); real editable draft field, serialized by `form_1702rt_xml.rs`. |
| B-1d: 1700 key-namespace mismatch (181 unmatched) | **RESOLVED** — `screen_form_module.rs` replaces the stale extracted 1700 entry with `screen-1700-v2018.json`, which uses the official `frm1700:txtPg*` namespace; `form_1700.rs::to_bir_field_map` prefixes and writes official keys. No live mismatch. (See R-2 for the real 1700 gap found instead.) |
| B-1e/B-2: 2551Q `txtEmail` / 1702Q `txtTelNum` missing | **REFUTED** — present as `#occurrence-N` keys (2551Q `txtEmail` text+hidden; 1702Q `frm1702q:txtTelNum#occurrence-1` "Contact number"). |
| B-1e: 1702EX `rdoPg1Pt1I7RDO` missing | **Covered** — engine writes the official radio key (`form_1702ex.rs:879`); UI uses the `hdnPg1Pt1I7RDO` alias bound `/rdo_code` (`add:` override). |
| B-1e: 1707A `hdnI5TIN4/P2`, `hdnRDO` missing | **Covered** — engine-written hidden keys (`form_1707a.rs:598-599,648`). |
| B-1e: 1702EX `Pg3Pt6S*PopLength` ×5 missing | **Covered** — engine-computed hidden popup counters (`form_1702ex.rs:1136`; agent-forms `required:"hidden"`). |
| B-1e: 2550M `AtcCode1..36` missing | **Covered differently** — official bool flags ↔ our 36 `sel` dropdowns `txtAtcCde*`; engine converts (`form_2550m.rs:1160-1163`). |
| B-1e: 2550M `txtEmail` missing | **Intentional** — `agent-forms/2550m-v2007.json` marks it `hidden/workflow-metadata`; engine still writes it. |
| B-1e: 1702Q `txtLOB` missing | **Intentional** — `hidden/workflow-metadata`, engine fills `profile.line_of_business`. |
| B-1e: 1702MX `drpPg3Sc1I11CB` missing | **Intentional** — `final-copy-only-or-runtime-generated-control`; engine computes `special_rate_option()`. |

## R-2. NEW confirmed finding — 1700 "Part IV Details of Payment" absent

The effective-schema vs official-XML diff (normalized keys, all 43 forms) found
**1700 is the only form with a real user-facing field-family gap**:

- Missing from UI schema (and from the draft model — "the model does not keep
  it", `screen-1700-v2018.json` `_evidence`):
  `frm1700:txtPg1I38{Agency,Number,Date,Amount}`,
  `txtPg1I39{Agency,Number,Date,Amount}`,
  `txtPg1I40{Number,Date,Amount}`,
  `txtPg1I41{Agency,Number,Date,Amount}`,
  `txtPg1I37Particular` — **17 controls**, the payment-agency/check-no/date/
  amount rows of printed items 38-41.
- Official XML carries all of them (`official-xml/1700-v2013.json:697-877`);
  the engine emits them **empty**.
- **betterebirforms HAS all 17** as editable fields
  (`/tmp/ebirforms-main/src/form/fields/1700v2018.json:264-330`).
- Also absent: `frm1700:txtTelno` (emitted by engine; likely a legacy duplicate
  of `txtPg1I14TelNum` — low risk).
- Impact: a 1700 filer cannot record *how* tax was paid (agency, check/receipt
  number, date, amount per payment) — data BIR's form collects and our savefile
  silently blanks. This is the audit's most concrete missing-fields defect.

## R-3. Confirmed missing fields — final classification (effective schema)

Normalized key diff, all 43 forms, official-XML as contract. "Missing" = no
control in the effective schema. Everything else in the diff was classified
non-user-facing (workflow/pagination keys `txtCurrentPage`, `txtMaxPage`,
`txtEnroll`, `txtFinalFlag`, `ebirOnline*`, `driveSelectTPExport`,
`hPartIITableSize`, `modLabel`, `numOfDays/Months`, `attachment*`; schedule
popup-row expansions and `#occurrence-N` duplicates accounted for).

| Form | Missing controls | Classification |
|---|---|---|
| **1700** | 17 payment-detail cells + `txtPg1I37Particular` (+`txtTelno`) | **REAL GAP** — unenterable, engine emits empty (R-2) |
| 1601EQ, 1601FQ, 1603Q, 1604E, 2000, 2000OT | `txtAddress2` (2000/2000OT also `txtOtherName2`) | **REAL GAP** — engine `put("txtAddress2", "")`; line 2 unenterable. 1604E derives a print-only line 2 that never reaches XML. |
| 1701Q | `txtAddress2` | By design — BIR HTA merges address lines; `_2` never populated anywhere. |
| 1800 | `txtRegAddress2`, `txtResAddress2` | Partial — engine auto-splits profile address at 100 chars; user can't control split. |
| 1701Q, 2551Q | `txtPg2TIN1-3`, `txtPg2BranchCode`, `txtPg2TaxpayerName` | Page-2 header *copies* of page-1 filer data — engine/profile-filled duplicates, same class other forms mark `kind:"hidden"`. Low impact; 2551Q/1701Q are the only forms where the block was never extracted at all. |

All other forms: **zero unexplained missing keys**.

## R-4. Required flags — refined verdict

Round-1 B-2 flagged `r`-marker diffs on 1707A (TIN×3+branch), 2552 (TIN×4),
2200AN (month/year). Round-2 checked the actual validators:

- **Blocking parity holds.** Blank TIN fails `tin_is_well_formed`/TIN-shape
  checks (1707A `form_1707a.rs:1073-1077`, 2552 `form_2552.rs:848-861`);
  blank month/year fails `date()`→error (2200AN `form_2200an.rs:664-668`).
  1707A `txtI1YearEndYear`/`txtI5BranchMask` (ref-required) are covered by
  typed draft fields with defaults / the 14-digit TIN.
- **Residual inconsistency (cosmetic):** the schema `r`/required *marker* is
  absent on those fields, so the UI won't show `*` even though validation
  blocks. Affects at least 1707A TIN fields, 2552 TIN fields, 2200AN
  month/year — worth a sweep of all `r` flags vs `validateRequired` maps.

## R-5. Validation parity — quantified, still not proven

Rule corpus: `rules/forms/<id>/validations.json` documents **2,007 rules**
across 43 forms (1,678 with `exact_message` from the official HTA). Of the
**1,245 validate/save rules**, **829 (66.6%)** have their exact or fuzzy
message text present in our Rust validators. Per-form coverage and the full
miss list: `cui/audit-validation-misses.json` (regenerate with the method in this
section's history).

Miss concentration (needs semantic check, not assume missing):
- **1700 (71 misses)** — expected: our v2018 screen renumbers items vs the
  v2013 rules packet (TIN "Item 4" vs official "Item 5", etc.); our
  `form_1700.rs` validator has ~30 err() branches covering the same rule
  families with different wording. Likely reworded equivalents — verify.
- **1702mx (60), 1702rt (27), 2550q (27), 1601c (22), 1600pt/vt (13+13)** —
  schedule-row/basis rules; our validators port `validateForm` explicitly
  (e.g. `form_1604f.rs:368-521` `remittance_error`), so misses are probably
  reworded — but **1604f's per-quarter schedule-row rules and 1702mx's
  Schedule-1 basis rules are the top candidates for genuine gaps**.
- Message-text parity is inherently approximate: the reference
  (betterebirforms) uses paraphrased messages too, so "exact official
  message" ≠ required for correctness — the *rule* (field + condition +
  blocking) is what matters.

**Verdict: validation parity is PARTIALLY VERIFIED.** Architecture confirmed
(host `screen_form_validate` → per-form `FormValidator`), shared checks
(TIN check-digit, RDO official-option, min-year, ZIP/email/phone) confirmed,
required-blank blocking confirmed on sampled forms. A rule-by-rule semantic
diff of all 2,007 rules was NOT completed — it is the largest remaining work
item for the validating agent.

## R-6. NEW renderer/session findings (subagent-verified this round)

Beyond I-1..I-12, independent re-check confirmed or added:

- **I-13. "From tax profile" fold is cosmetic-only.** `part_info()` computes
  `fold` (tucked items + open flag) and the UI draws the
  "From tax profile · N filled boxes (show/hide)" button
  (`form.rs:938-945`), but `draw_part_items` (`form.rs:1771`) iterates **all**
  items unconditionally — tucked items render whether the fold is open or
  not. `profile_open`/`toggle_profile_fold` have no effect on visibility.
- **I-14. Empty selects display the first option, not "Choose…".**
  `sel_shown` (`session.rs:2126-2164`) discards the `CHOOSE` head option that
  `model::select_model` builds and returns bare `listed`; `draw_select`
  (`form.rs:2159-2166`) then `unwrap_or(0)` → an unset select *shows* e.g.
  "01 - January". No data corruption (write is guarded by `v != shown`), but
  the display lies about emptiness. NSH shows "Choose…".
- **I-15. Schedule `radio` cells cannot be picked — CONFIRMED BUG.**
  Schedule columns of `t:"radio"` exist on **1700 and 1701** (employer
  Taxpayer/Spouse columns, 4 items total). Cells render via `draw_radio` →
  `s.pick_radio(k)` → `field_of.get(k)` returns `None` for schedule keys
  (only `kind_of`/`key_index` are populated, `session.rs:5090-5104`) → early
  return. The checkbox looks clickable but never writes. Schedule `chk` cells
  work (they use `s.choose` directly).
- **I-16. Schedule totals show `sel` codes, not labels.** NSH `readText`
  maps select values to option labels in total rows; CUI `cell_read_text`
  (`form.rs:2345-2353`) shows the raw code.
- **I-17. Choice/election Save buttons never disabled.** NSH disables Save
  until a pick (`rows.tsx:299,340`); CUI always enables (`form.rs:1965-1991`).
  Session no-ops safely — affordance divergence only.
- **I-18. Session-layer divergences flagged for review:** `notes_always` not
  updated in `apply_fill`; pre-fill values not captured before `st.values`
  overwrite (undo/"kept" accuracy); schedule radio cells also bypass `is_shut`
  locked-field handling (`form.rs:2473-2486` vs `draw_field` path).
- **I-19. Item chrome/order differences:** validation message renders *above*
  controls (NSH reserves the line *below*); item-number chip still missing
  (I-8); radio option `choiceLabel` "Group: "-prefix stripping not ported.

## R-7. Labels & options — round-2 verdict

- Our labels are **verbatim BIR printed captions** (via `agent-forms`/
  `print-cells.json`/`writer-cells.json`); the reference uses sparse
  hand-paraphrases ("Taxpayer / registered name" reused across ~20 forms).
  No sampled case where ours contradicts the printed caption — ours is
  strictly more faithful. Item numbers render as a separate chip vs embedded
  "N —" strings: presentation difference, not data loss.
- Options: base schema `o` lists are plain strings; all `{value,label}`
  objects come via overrides (41 entries, well-formed). No empty/malformed
  option lists found.
- **RDO list scope:** ours offers the full RDO catalog
  (`reference::get_all_rdos`); reference filters per-form via `row-option.json`.
  Superset — functional, but a behavioral difference worth noting.
- **Least-verified areas (hand to next agent):** schedule `cols[].l` vs
  reference row-cell labels on the big schedules (1701, 2552, 1800, 2000OT);
  ATC option *value-sets* per form; treaty-code dropdown contents; 1700
  item-caption fidelity on the v2018 screen vs paper is spot-checked only.

## R-8. What round 2 could NOT verify (explicit hand-off list)

1. **Semantic rule-by-rule validation diff** — 2,007 rules; only message-text
   coverage measured (66.6%). Needs per-rule field+condition+blocking
   comparison, prioritizing 1702mx, 1702rt, 2550q, 1604f, 1601c, 1700.
2. **Schedule column labels** cell-by-cell on large schedules.
3. **ATC/treaty option value-sets** per form vs reference catalogs.
4. **Runtime audit** `audit_all_screen_forms` could not run here (git deps
   `gpui-component`/`gpui-agent` pinned revs unfetchable) — rerun where
   network allows.
5. **1700 payment-detail fix** is the one confirmed *data-loss-class* defect
   (R-2) — needs model + schema + serializer + print work, not just UI.

---

*Round-1 sections above are preserved for provenance; where Round 2 refutes
them, R-1 is authoritative.*

*No application code was modified. All findings reproducible from
`app/forms/forms-schema.json`, `app/forms/schema-overrides.json`,
`app/forms/screen-1700-v2018.json`, `crates/bir-core/data/official-xml/`,
`/tmp/effective-schemas/` (dumped via `door::tests::dump_effective_schemas`),
`/tmp/ebirforms-main/src/form/{fields,template}/`,
`rules/forms/*/validations.json`, and `crates/bir-core/src/forms/`.*
