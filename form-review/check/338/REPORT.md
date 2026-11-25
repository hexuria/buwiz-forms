# Independent Check — PR #338 (`feat/store-launch-agent-fill-gaps`)

- **PR / Branch:** `#338` — `feat/store-launch-agent-fill-gaps`
- **Tested Commit:** `75dcab06` (exact; remote branch head confirmed at `75dcab06`, not moved)
- **Base Comparison:** `origin/main` (`93820d0f`)
- **Role:** Independent checker (report only; no fixes, no pushes to PR branch, no PR comments)
- **Live Environment:** NSH desktop app (`nsh-check-338`, Xvfb `:99`, `1440×900`), fresh scratch DB `/home/ubuntu/BuwizNsh/scratch/check-338.db`, demo taxpayer (`00012345300000`, RDO `039`), `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`BIR_SFTP_LIVE` never set; Queue never pressed)

---

## Verdicts

| Form | Verdict | Summary |
| --- | --- | --- |
| **1702-MX** | **`broken`** | **Tier 1 (Wrong number filed):** All 24 newly fillable Schedule 7.1 & 8.1 NOLCO columns A–D write `WholePesoInput.amount` while leaving `raw: ""`, so `form_1702mx_xml.rs::insert_money` serializes them empty into the BIR XML payload and prints all 24 boxes **completely blank** on paper (`preview/1702mx-p4.png`, `pdf/1702mx-p4.png`, `saved-1702mx.pdf`). |
| **2552** | **`broken`** | Fresh-state and LSE-state `form.fill` accept `txtPg1Pt1I12TotShare` (marked `not_shown`, not refused) and accept `txtPg2P4I2Amount` before row 1 is filled — not enforcing the official conditions claimed. On paper, all 5 Part IV amounts print left-aligned (`pdf/2552-p2.png`), Item 12A/12B share counts print left-aligned in their combs (`pdf/2552-p1.png`), and long address/email cross comb ticks. |
| **1702-EX** | **`broken`** | Popup-owned `frm1702EX:txtPg3Pt6S2I4LegalBasis` (Schedule 2 Item 4 Legal Basis) is **accepted** by `form.fill` instead of refused; `17i` and Schedule 2 row 4 `Description`/`Amount` are refused as `"computed by BIR's own formulas"` rather than naming the attached schedule (only rows `17e–17h` and Schedule 2 rows `2–3` carry the schedule-name refusal). On paper, long Registered Address truncates at `BRGY SAN LORENZ` (`pdf/1702ex-p1.png`). |

---

## Ranked Faults

### Form 1702-MX (`broken`)
1. **[Wrong number filed / Paper — Rule 2]** All 24 newly fillable NOLCO boxes in Schedule 7.1 and Schedule 8.1 (rows 4–6, columns A–D) serialize as empty (`""`) in the BIR XML payload and print **completely blank** on paper (`preview/1702mx-p4.png`, `pdf/1702mx-p4.png`, `saved-1702mx.pdf`), even though they appear filled on the form page (`form-1702mx-sch7.png`, `form-1702mx-sch8.png`) and feed the computed Column E (`95000000`, `96000000`, `97000000`, `88000000`, `82000000`, `75500000`) and Row 8 totals (`75000000`, `48000000`). **Root cause:** `crates/bir-core/data/agent-forms/1702mx-v2018c.json` maps these 24 fields to `.../{amount,applied_previous_years,expired,applied_current_year}/amount` (`WholePesoInput.amount`), leaving `WholePesoInput.raw` empty (`""`) in `form_drafts.data_json`; `crates/bir-core/src/forms/form_1702mx_xml.rs:1021,1430` (`insert_money`) emits `value.raw.clone()`.
2. **[Paper — Rule 2 & Rule 5]** Schedule 7.1 and Schedule 8.1 Column E values and Row 8 Total NOLCO (`95000000`, `75000000`, `48000000`, etc.) print **left-aligned and unformatted** (no thousands commas) instead of right-aligned (`preview/1702mx-p4.png`, `pdf/1702mx-p4.png`).
3. **[Paper — Rule 1]** Long Registered Address (`Part I Item 9`) crosses comb ticks on `pdf/1702mx-p1.png`; Registered Name in the top header crosses comb ticks on `pdf/1702mx-p2.png`, `pdf/1702mx-p3.png`, and `pdf/1702mx-p4.png`. Schedule 1 Items 2 and 3 print stray `0` in all three columns on an unfilled schedule (`pdf/1702mx-p2.png`).

### Form 2552 (`broken`)
1. **[Agent gate mismatch — Claimed condition not enforced]** Live `form.fill` on a fresh 2552 return (no Item 11 transaction chosen) and after choosing `LSE` **accepted** `frm2552:txtPg1Pt1I12TotShare` (returning `"applied": ["frm2552:txtPg1Pt1I12TotShare"]` with `"not_shown"` rather than refusing it), and **accepted** `frm2552:txtPg2P4I2Amount` before Part IV Row 1 was filled (`probe_338.json`, `live_338_results.json`, `recording-2552.mp4`). Official eBIRForms keeps Item 12 disabled unless IPO (`Primary`/`Secondary`) is selected and keeps Part IV rows 2–5 disabled until the preceding row is filled.
2. **[Paper — Rule 5]** All 5 Part IV `Amount Involved` values (`101,234,567.89` .. `505,678,901.56`) print **left-aligned** instead of right-aligned (`preview/2552-p2.png`, `pdf/2552-p2.png`, `saved-2552.pdf`). Schedule 1–3 and Part VI `0.00` amounts are also left-aligned.
3. **[Paper — Rule 5 & Rule 1]** Item 12A (`2500000`) and PR #338's Item 12B (`10000000`) print **left-aligned** in their comb boxes instead of right-aligned; long Registered Address (`Item 7`) and Email (`Item 9`) cross comb ticks; blank Date of Transaction prints `00/00/2027` (`preview/2552-p1.png`, `pdf/2552-p1.png`).

### Form 1702-EX (`broken`)
1. **[Agent refusal gap — Popup-owned box accepted]** `frm1702EX:txtPg3Pt6S2I4LegalBasis` (Schedule 2 Item 4 Legal Basis) is **accepted** by live `form.fill` (`"applied": ["frm1702EX:txtPg3Pt6S2I4LegalBasis"]`) instead of refused (`live_338_results.json`, `recording-1702ex.mp4`), even though official eBIRForms disables Schedule 2 row 4 as popup-owned (`more...`).
2. **[Agent refusal wording gap]** Schedule 1 Item `17i` (`txtPg3Pt6I17OthersIDesc`, `txtPg3Pt6I17OthersIAmt`) and Schedule 2 Item 4 (`txtPg3Pt6S2I4Description`, `txtPg3Pt6S2I4Amount`) are refused as `"computed by BIR's own formulas"` rather than naming the attached schedule, because `EX_ATTACHED_ROWS` in `crates/bir-core/src/forms/agent/draft_locks.rs` only lists `Item 49`, `Schedule 1 items 17e–17h`, and `Schedule 2 rows 2–3`. (Rows `17e–17h` and Schedule 2 rows `2–3` do return the new schedule-name refusal strings.)
3. **[Paper — Rule 1]** Long Registered Address (`Part I Item 9`) is truncated at `BRGY SAN LORENZ` (dropping `O MAKATI CITY`) on `preview/1702ex-p1.png` / `pdf/1702ex-p1.png`; Registered Name in the top header crosses comb ticks on `pdf/1702ex-p2.png` and `pdf/1702ex-p3.png`.

---

## 1. Box-by-Box Comparison Against Official Program & Provenance

### Form 2552 (`2552-v2018`)
Provenance (`html-frozen/2552-2018/provenance.json`): `2552 2018ENCS Final.pdf`, SHA-256 `c0a9d7cd44cf931fb939a010a265d05655b9a1d55d21841ecf0b590594b1742a`, 2 pages, `612×936pt`. *(Note: BIR CDN returned HTTP 403 for all candidate `2552` PDF URLs; `official/2552-p1.png` and `official/2552-p2.png` are rendered from the blank frozen `html-frozen/2552-2018/index.html` template.)*

| Box / Key | Official Rule & Citation | `75dcab06` Behaviour (Form Page & `form.fill`) | Same or Different |
| --- | --- | --- | --- |
| Item 12B Total Outstanding Shares (`frm2552:txtPg1Pt1I12TotShare`) | Initially disabled (`runtime-control-inventory-v796.json` L307–318; official fixture L759); opens when Item 11B IPO (`Primary` or `Secondary`) is selected. | **Form page:** gated by `Gate::Offering2552` (`rows_as_needed.rs`), opens under `Primary`/`Secondary`. **`form.fill`:** accepted in Fresh state and under `LSE` (with `"not_shown"`), and accepted under `Primary`; survives save/reopen (`10000000`). | **Different on `form.fill`** (accepted when gated shut; form page agrees) |
| Part IV Row 2 Amount (`frm2552:txtPg2P4I2Amount`) | Initially disabled (`runtime-control-inventory-v796.json` L823–834; official fixture L1327); opens once Row 1 is populated. | **Form page:** gated by prior row (`rows_as_needed.rs`). **`form.fill`:** accepted on Fresh return before Row 1 is filled; survives save/reopen (`202,345,678.90`). | **Different on `form.fill`** (accepted before Row 1 filled) |
| Part IV Row 3 Amount (`frm2552:txtPg2P4I3Amount`) | Initially disabled (`runtime-control-inventory-v796.json` L847–858; official fixture L1337); opens once Row 2 is populated. | **Form page:** gated by prior row. **`form.fill`:** accepted and survives save/reopen (`303,456,789.12`). | **Same when Rows 1–2 filled** (ungated on `form.fill`) |
| Part IV Row 4 Amount (`frm2552:txtPg2P4I4Amount`) | Initially disabled (`runtime-control-inventory-v796.json` L871–882; official fixture L1347); opens once Row 3 is populated. | **Form page:** gated by prior row. **`form.fill`:** accepted and survives save/reopen (`404,567,890.34`). | **Same when Rows 1–3 filled** (ungated on `form.fill`) |
| Part IV Row 5 Amount (`frm2552:txtPg2P4I5Amount`) | Initially disabled (`runtime-control-inventory-v796.json` L895–906; official fixture L1357); opens once Row 4 is populated. | **Form page:** gated by prior row. **`form.fill`:** accepted and survives save/reopen (`505,678,901.56`). | **Same when Rows 1–4 filled** (ungated on `form.fill`) |

### Form 1702-EX (`1702ex-v2018`)
Provenance (`html-frozen/1702ex-2018/provenance.json`): `1702-EX Jan 2018 ENCS v2.pdf`, SHA-256 `e2dece405bbf48d4be8f50fc73f124b90e574ddaf79361b2f81bacf8831efcfb`, 3 pages (downloaded and verified from BIR CDN -> `official/1702ex-p1..p3.png`).

| Box / Key | Official Rule & Citation | `75dcab06` Live `form.fill` Behaviour & Exact Refusal Text | Same or Different |
| --- | --- | --- | --- |
| Part IV Item 49 Desc & Amt (`txtPg2Pt4I49OtherTaxCrPmtDesc`, `...Amt`) | Disabled on main form (official fixture L1596, L1600); filled via Part IV Item 49 attached-schedule popup. | Refused: `` `frm1702EX:txtPg2Pt4I49OtherTaxCrPmtDesc` filled from the Part IV Item 49 attached schedule; BIR's program keeps this row shut `` (same for `Amt`). | **Same** |
| Schedule 1 Items `17e–17h` Desc & Amt (`txtPg3Pt6I17Others{E..H}{Desc,Amt}`) | Disabled on main form; populated from Schedule 1 Item 17i `(more...)` attached schedule (`draft_locks.rs` `EX_ATTACHED_ROWS`). | Refused: `` `frm1702EX:txtPg3Pt6I17OthersEDesc` filled from the Schedule 1 Item 17i (more...) attached schedule; BIR's program keeps this row shut `` (same for `EAmt`). | **Same** |
| Schedule 1 Item `17i` Desc & Amt (`txtPg3Pt6I17OthersIDesc`, `...IAmt`) | Popup summary / `(more...)` overflow row on main form. | Refused as computed, **not** with schedule name: `` `frm1702EX:txtPg3Pt6I17OthersIDesc` computed by BIR's own formulas `` / `` `frm1702EX:txtPg3Pt6I17OthersIAmt` is computed by BIR's own formulas ``. | **Different refusal reason** (does not name Schedule 1 popup) |
| Schedule 2 Rows 2–3 (`txtPg3Pt6S2I{2,3}{Description,LegalBasis,Amount}`) | Disabled on main form (official fixture L2146–2185); populated from Schedule 2 Item 4 `(more...)` attached schedule. | Refused: `` `frm1702EX:txtPg3Pt6S2I2Description` filled from the Schedule 2 Item 4 (more...) attached schedule; BIR's program keeps this row shut `` (same for `Amount`). | **Same** |
| Schedule 2 Row 4 Desc & Amt (`txtPg3Pt6S2I4Description`, `...Amount`) | Disabled on main form with `(more...)` popup control (official fixture L2195–2203). | Refused as computed, **not** with schedule name: `` `frm1702EX:txtPg3Pt6S2I4Description` computed by BIR's own formulas `` / `` `frm1702EX:txtPg3Pt6S2I4Amount` is computed by BIR's own formulas ``. | **Different refusal reason** |
| Schedule 2 Row 4 Legal Basis (`frm1702EX:txtPg3Pt6S2I4LegalBasis`) | Disabled on main form (official fixture L2195–2203); popup-owned row. | **ACCEPTED** by `form.fill` (`"applied": ["frm1702EX:txtPg3Pt6S2I4LegalBasis"]`). | **Different (BUG)** |

### Form 1702-MX (`1702mx-v2018c`)
Provenance (`html-frozen/1702mx-2018c/provenance.json`): `1702-MX Jan 2018 ENCS Final with OSDv2.pdf`, SHA-256 `81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2`, 4 pages (downloaded and verified from BIR CDN -> `official/1702mx-p1..p4.png`).

| Box / Key | Official Rule & Citation | `75dcab06` Live `form.fill`, Save & Print Behaviour | Same or Different |
| --- | --- | --- | --- |
| Schedule 5 Items `17d–17i` Cols A–D (`txtPg3Sc5Itm17{d..i}{A..D}`) | Disabled on main form (`runtime-control-inventory-v796.json` L4248–4595; official fixture L3377–3592); popup-owned. | Refused: `` `frm1702MX:txtPg3Sc5Itm17dA` filled from the Schedule 5 Item 17d-17i attached schedule; BIR's program keeps this row shut `` (same for `17iC`). | **Same** |
| Schedule 6 Rows 2–4 (`txtPg6Sc6I2*`, `txtPg3Sc6I{3,4}*`) | Disabled on main form (`runtime-control-inventory-v796.json` L4716–4931; official fixture L3715–3791); popup-owned. | Refused: `` `frm1702MX:txtPg6Sc6I2CA` filled from the Schedule 6 (rows 2-4 / more...) attached schedule; BIR's program keeps this row shut `` (same for `txtPg3Sc6I4CC`). | **Same** |
| Schedule 7.1 Rows 4–6 Cols A–D (12 boxes: `txtPg3IShed7_{4,5,6}{A,B,C,D}`) | Open editable whole-peso inputs (official fixture L4011–4058); Col E computed `A - (B+C+D)` (L4022, L4041, L4060). | Accepted by `form.fill`; 15/15 (incl. Year) survive save/dismiss/reopen and display on `form-1702mx-sch7.png`; Col E (`95M`, `96M`, `97M`) and Row 8 (`75M`) compute correctly. **HOWEVER:** `WholePesoInput.raw` remains `""`, so XML serialization and print output leave all 12 boxes **blank** (`pdf/1702mx-p4.png`). | **Different (CRITICAL BUG in XML/print)** |
| Schedule 8.1 Rows 4–6 Cols A–D (12 boxes: `txtPg4IShed8_{4,5,6}{A,B,C,D}`, `txtPg3IShed8_6C`) | Open editable whole-peso inputs (official fixture L4205–4252); Col E computed `A - (B+C+D)` (L4216, L4235, L4254). | Accepted by `form.fill`; 15/15 (incl. Year) survive save/dismiss/reopen and display on `form-1702mx-sch8.png`; Col E (`88M`, `82M`, `75.5M`) and Row 8 (`48M`) compute correctly. **HOWEVER:** `WholePesoInput.raw` remains `""`, so XML serialization and print output leave all 12 boxes **blank** (`pdf/1702mx-p4.png`). | **Different (CRITICAL BUG in XML/print)** |

---

## 2. Extra Conditions Tried Beyond the Claim

- **2552 Fresh state & LSE state:** `form.fill` accepted `frm2552:txtPg1Pt1I12TotShare` with `"not_shown"` and accepted `frm2552:txtPg2P4I2Amount` before Row 1 was filled. Attempting to set individual radio key `frm2552:rdoPg1I11TransactionLSE` was refused with `` `frm2552:rdoPg1I11TransactionLSE` cannot be filled from here: it is written through the form's own editors ``; the grouped radio key `frm2552:rdoPg1I11Transaction` (`"LSE"` / `"Primary"`) switched the option cleanly.
- **2552 Save / dismiss / reopen:** `12/12` filled values (Item 12A/12B + Part IV rows 1–5 classification and amount) survived save, dismiss, and reopen with `0` mismatches.
- **1702MX Save / dismiss / reopen:** `30/30` filled NOLCO values (Schedule 7.1 & 8.1 rows 4–6 Year + A–D) survived save, dismiss, and reopen in `form_drafts.data_json` (`amount` populated), but every single A–D field had `"raw": ""` while computed `unapplied` (E) had `"raw": "95000000"` etc.

---

## 3. Sample-Payload Counts & Diff Reading

### `cargo test --locked -p bir-core --test sample_payloads_test`
- **PR #338 head (`75dcab06`):** `144 passed; 0 failed; 1 ignored`
- **`origin/main` (`93820d0f`):** `144 passed; 0 failed; 1 ignored`
- **Focused `bir-app-api` tests at `75dcab06`:** `38 passed; 0 failed; 0 ignored` (includes 2552 Item 12 & Part IV fill, 1702EX/1702MX refusal strings, 1702MX NOLCO fill, and `agent-carry.json` / `agent-locks.json` freshness checks)

### File-by-File Diff Reading (`git diff -w origin/main...75dcab06`: 10 files, +334 / -104)
- `forms/schema-overrides.json`: **not modified**.
- `crates/bir-core/tests/sample-payloads/`: **not modified** (matches "No sample payload changed").
- `crates/bir-core/data/agent-carry.json` (`-4`): removes `frm2552:txtPg2P4I{2..5}Amount` from 2552 computed carry list — within claim; freshness test passes.
- `crates/bir-core/data/agent-locks.json` (`-5`): removes `frm2552:txtPg1Pt1I12TotShare` and `frm2552:txtPg2P4I{2..5}Amount` from 2552 locks — within claim; freshness test passes.
- `crates/bir-core/data/agent-forms/2552-v2018.json` (`+15/-0` net): maps Item 12B (`outstanding_shares`) and Part IV rows 2–5 (`exempt_transactions/{1..4}/amount`) as optional fillable fields — within claim; passes `agent_forms_are_derived_from_the_samples`.
- `crates/bir-core/data/agent-forms/1702mx-v2018c.json` (`+48/-48` hunks): maps all 24 Schedule 7.1 & 8.1 rows 4–6 A–D fields to `.../amount` (`WholePesoInput.amount`) — within claim, **but causes the `raw: ""` blank-XML/blank-print bug**.
- `crates/bir-core/src/forms/agent/draft_locks.rs` (`+100/-...`): adds schedule-specific lock constants (`EX_ATTACHED_LOCK_ITEM_49`, `EX_ATTACHED_LOCK_SCHED_1`, `EX_ATTACHED_LOCK_SCHED_2`, `MX_ATTACHED_LOCK_SCHED_5`, `MX_ATTACHED_LOCK_SCHED_6`) and `ex_attached_lock()` / `mx_attached_lock()` — within claim, **but `EX_ATTACHED_ROWS` omits Schedule 1 Item `17i` and Schedule 2 Row 4 (`S2I4LegalBasis` leaks through)**.
- `crates/bir-core/src/forms/agent/probe.rs` (`+58/-...`): adds probe helpers (`echoes_whole`) for 2552 Item 12, 2552 Part IV, and 1702MX Schedule 7.1 — within claim (missed the `raw: ""` bug because probe checks `to_official_fields` rather than XML serialization).
- `crates/bir-app-api/src/screen_form_module/rows_as_needed.rs`: merges 2552 Item 12A/12B under `Gate::Offering2552` and gates Part IV rows 2–5 amounts with their classification inputs — within claim.
- `crates/bir-app-api/src/screen_form_module/rows_annual_corporate.rs`: wires `ex_attached_lock` and `mx_attached_lock` into 1702EX/1702MX refusal paths — within claim.
- `crates/bir-app-api/src/screen_form_module/tests/{as_needed.rs,annual_corporate.rs}`: unit tests for all PR #338 claims — within claim.

---

## 4. Evidence Inventory (`form-review/check/338/`)

- `recording-2552.mp4` — 18.0 s, 1440×900 H.264 app-window-only screen recording (`ffmpeg x11grab`): opening 2552 TY 2027, live `form.fill` on Item 12 and Part IV rows 1–5, and opening Print Preview.
- `recording-1702ex.mp4` — 16.7 s, 1440×900 H.264 app-window-only screen recording: opening 1702EX TY 2027, live `form.fill` refusals on Part IV Item 49, Schedule 1, and Schedule 2, and opening Print Preview.
- `recording-1702mx.mp4` — 15.8 s, 1440×900 H.264 app-window-only screen recording: opening 1702MX TY 2027, live `form.fill` refusals on Schedules 5 & 6 and fillable NOLCO rows 4–6 in Schedules 7.1 & 8.1, and opening Print Preview.
- `form-2552-part1.png`, `form-2552-part4.png`, `form-1702ex-part4.png`, `form-1702ex-sch1.png`, `form-1702mx-sch7.png`, `form-1702mx-sch8.png` — Live app form-page screenshots (1400×875 px).
- `saved-2552.pdf`, `saved-1702ex.pdf`, `saved-1702mx.pdf` — Saved PDFs printed from the live app's `sheet.html` for all three forms.
- `preview/{2552,1702ex,1702mx}-p*.png` & `pdf/{2552,1702ex,1702mx}-p*.png` — Paired 1400 px print preview and saved-PDF page renders (`2` pages for 2552, `3` for 1702EX, `4` for 1702MX).
- `official/{2552,1702ex,1702mx}-p*.png` — Official reference page renders (1400 px wide; exact provenance PDFs for 1702EX & 1702MX; blank frozen template for 2552 due to BIR CDN HTTP 403).
- `README.md` — File-by-file description of what each artifact shows and what it does not show.
