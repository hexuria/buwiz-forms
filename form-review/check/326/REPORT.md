# Independent Check Report — PR #326 (`feat/store-launch-unfillable-rows`)

## 1. Commits Tested & Branch Status

- **PR branch**: `origin/feat/store-launch-unfillable-rows`
- **Head moved during review**: Yes — moved from `13e6abe4` to **`2555a2d8`** (one follow-up commit formatting 1702MX computed NOLCO Column E and Row 8 totals with thousands separators).
- **Exact commits tested**:
  - **Claim 1 (2551Q Schedule 1)**: `13e6abe4` (unchanged in `2555a2d8`)
  - **Claim 2 (five tables: 1600WP, 1601FQ, 1701, 2000, 2553)**: `13e6abe4` (unchanged in `2555a2d8`)
  - **Claim 3 (1702MX NOLCO)**: initially inspected at `13e6abe4` (which revealed missing thousands separators on printed Col E and Row 8 totals), then re-verified end-to-end at **`2555a2d8`**
  - **Part C (`sample_payloads_test` & 3-form regression diff)**: `13e6abe4` vs `origin/main` (`4399ce84` for `sample_payloads_test`, `3aceafe8` for 3-form diff)
- **Safety**: Every run used `nsh-*` recipes, scratch DB `/home/ubuntu/BuwizNsh/check326.db`, demo taxpayer `SANTOS, JUANA` (TIN `000-123-453-00000`), and `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` (`BIR_SFTP_LIVE` unset; Queue never pressed).

---

## 2. Part A — Official-Program & Three-Way Arithmetic Comparison

### 2.1 Claim 1 — 2551Q Schedule 1 (tested at `13e6abe4`)

#### Official ATC Catalog (`rules/forms/2551q-v2018/fixtures/atc-catalog-v796.json`, v7.9.6.0) vs App Dropdown

All **23 entries** (in exact positional order 1–23) match the official BIR package catalog, including the duplicate `PT010` at position 1 (3%) and position 23 (1%):

| Pos | Official ATC | Official Rate | App Dropdown | App Rate | Match |
|---:|---|---:|---|---:|---|
| 1 | `PT010` | 3% | `PT010` | 3% | Yes |
| 2 | `PT040` | 3% | `PT040` | 3% | Yes |
| 3 | `PT041` | 3% | `PT041` | 3% | Yes |
| 4 | `PT060` | 2% | `PT060` | 2% | Yes |
| 5 | `PT070` | 3% | `PT070` | 3% | Yes |
| 6 | `PT090` | 10% | `PT090` | 10% | Yes |
| 7 | `PT101` | 1% | `PT101` | 1% | Yes |
| 8 | `PT102` | 0% | `PT102` | 0% | Yes |
| 9 | `PT103` | 7% | `PT103` | 7% | Yes |
| 10 | `PT104` | 7% | `PT104` | 7% | Yes |
| 11 | `PT105` | 5% | `PT105` | 5% | Yes |
| 12 | `PT113` | 5% | `PT113` | 5% | Yes |
| 13 | `PT114` | 1% | `PT114` | 1% | Yes |
| 14 | `PT115` | 5% | `PT115` | 5% | Yes |
| 15 | `PT120` | 2% | `PT120` | 2% | Yes |
| 16 | `PT130` | 4% | `PT130` | 4% | Yes |
| 17 | `PT132` | 5% | `PT132` | 5% | Yes |
| 18 | `PT140` | 18% | `PT140` | 18% | Yes |
| 19 | `PT150` | 18% | `PT150` | 18% | Yes |
| 20 | `PT160` | 10% | `PT160` | 10% | Yes |
| 21 | `PT170` | 15% | `PT170` | 15% | Yes |
| 22 | `PT180` | 30% | `PT180` | 30% | Yes |
| 23 | `PT010` | 1% | `PT010` | 1% | Yes |

#### Official Edge-Case Behavior (`rules/forms/2551q-v2018/{calculations,validations}.json`) vs App

- **Duplicate code (`PT010` at pos 1 = 3%, pos 23 = 1%)**: Official eBIRForms exposes both because it filters `atcCodes.xml` without deduplicating and serializes `drpATC{n}` as the 1-based list index (`1` vs `23`). The app exposes all 23 positions and serializes the 1-based index (`INDEXED_SELECTS` in `probe.rs`).
- **Blank ATC with nonzero amount**: Official validation raises `"Please select an ATC in Schedule 1."` (and disables amount until ATC is picked). In the running app, typing `50,000` with ATC blank kept amount `50,000` with rate `0.00` and tax due `0.00` on screen (validation fires on Check/submit, not live disable).
- **ATC selected with no amount**: Official program keeps rate from catalog and computes tax due `0.00`. App matches (`PT060` -> amount `0.00`, rate `2.0`, tax due `0.00`).
- **Row cleared/deleted in the middle**: Official program clears the slot without shifting later rows' ATC/amount into it. In the running app, deleting row 2 left row 1 and row 3 intact with row 3 retaining its own ATC and amount.

#### 6-Row Hundreds-of-Millions Return — Independent Arithmetic vs App vs XML vs Paper

| Row / Item | ATC | Taxable Amount | Rate | Independent Calc | App UI | Field-Map XML | Paper (`2551q-pdf-page-{1,2}.png`) |
|---|---|---:|---:|---:|---:|---:|---:|
| Sched 1 Row 1 | `PT010` | `111,111,111.11` | 3% | `3,333,333.33` | `3,333,333.33` | `3,333,333.33` | `3,333,333.33` |
| Sched 1 Row 2 | `PT060` | `222,222,222.22` | 2% | `4,444,444.44` | `4,444,444.44` | `4,444,444.44` | `4,444,444.44` |
| Sched 1 Row 3 | `PT105` | `333,333,333.33` | 5% | `16,666,666.67` | `16,666,666.67` | `16,666,666.67` | `16,666,666.67` |
| Sched 1 Row 4 | `PT103` | `444,444,444.44` | 7% | `31,111,111.11` | `31,111,111.11` | `31,111,111.11` | `31,111,111.11` |
| Sched 1 Row 5 | `PT090` | `555,555,555.55` | 10% | `55,555,555.56` | `55,555,555.56` | `55,555,555.56` | `55,555,555.56` |
| Sched 1 Row 6 | `PT170` | `666,666,666.66` | 15% | `100,000,000.00` | `100,000,000.00` | `100,000,000.00` | `100,000,000.00` |
| Sched 1 Item 7 Total | — | — | — | `211,111,111.11` | `211,111,111.11` | `211,111,111.11` | `211,111,111.11` |
| Part II Item 14 | — | — | — | `211,111,111.11` | `211,111,111.11` | `211,111,111.11` | `211,111,111.11` |
| Part II Item 15 | — | — | — | `12,345,678.90` | `12,345,678.90` | `12,345,678.90` | `12,345,678.90` |
| Part II Item 16 | — | — | — | `0.00` | `0.00` | `0.00` | `0.00` |
| Part II Item 17 | — | — | — | `1,000,000.00` | `1,000,000.00` | `1,000,000.00` | `1,000,000.00` |
| Part II Item 18 | — | — | — | `13,345,678.90` | `13,345,678.90` | `13,345,678.90` | `13,345,678.90` |
| Part II Item 19 | — | — | — | `197,765,432.21` | `197,765,432.21` | `197,765,432.21` | `197,765,432.21` |
| Part II Item 20 (Surcharge) | — | — | — | `49,441,358.05` | `49,441,358.05` | `49,441,358.05` | `49,441,358.05` |
| Part II Item 21 (Interest) | — | — | — | `31,729,160.85` | `31,729,160.85` | `31,729,160.85` | `31,729,160.85` |
| Part II Item 22 (Compromise) | — | — | — | `50,000.00` | `50,000.00` | `50,000.00` | `50,000.00` |
| Part II Item 23 | — | — | — | `81,220,518.90` | `81,220,518.90` | `81,220,518.90` | `81,220,518.90` |
| Part II Item 24 Payable | — | — | — | `278,985,951.11` | `278,985,951.11` | `278,985,951.11` | `278,985,951.11` |

**Beyond-claim finding (pre-existing on `main`)**: Calling `Form2551QDraft::to_bir_xml_payload()` on this hundreds-of-millions return rejects rows 3–6 with `"Schedule 1 row N tax due does not fit the official 7-cell integer field"` (`crates/bir-core/src/forms/form_2551q.rs:2102`), even though the official 2551Q Page 2 Tax Due comb box actually has **11 integer cells** and fits `100,000,000.00` with two cells to spare. PR #326 did not touch `form_2551q.rs`.

#### Official Sample Box-by-Box XML Comparison (`2551Q-122025Q1.official.xml`)

- **Schedule 1 (all 25 boxes `drpATC1..6`, `txtATCAmt1..6`, `txtATCRate1..6`, `txtATCDue1..6`, `txtTotalSched1`) and Part II Items 14–19**: **0 diffs** — exact text match (`drpATC1="1"`, `txtATCAmt1="180,000.00"`, `txtATCRate1="3.0"`, `txtATCDue1="5,400.00"`, untouched rows 2–6: `drpATC*="0"`, `txtATCAmt*="0.00"`, `txtATCRate*="0.00"`, `txtATCDue*="0.00"`, `txtTotalSched1="5,400.00"`).
- **Remaining 23 diffs in `xml/2551q-official-sample-diff.txt`**:
  - `txt20..24` (5 boxes): late-filing penalties auto-computed for Q1 2025 on today's date (`2026-11-25`) vs `0.00` in the on-time sample.
  - `taxRate1` (1 box): test harness JSON patch used `"Graduated"` instead of the serialized enum variant.
  - `txtAgency25..28`, `txtNumber25..28`, `txtDate25..28`, `txtAmount25..28`, `txtParticular28` (17 Part III payment boxes): omitted in `to_bir_field_map()` vs `""` in `.official.xml` (pre-existing on `main`).

---

### 2.2 Claim 3 — 1702MX NOLCO (re-verified at `2555a2d8`)

#### All 24 NOLCO Boxes + Col E + Row 8 Totals — Independent Arithmetic vs App UI vs Upload XML vs `form.fill` vs Paper (`2555a2d8`)

| Sched / Row | Year | A (Amount) | B (Prev) | C (Expired) | D (Current) | E (Unapplied, Calc) | App UI E | UI XML E | `form.fill` XML E | Paper Page 4 E (`2555a2d8`) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 7.1 Row 4 | 2022 | `300,000,000` | `50,000,000` | `20,000,000` | `80,000,000` | `150,000,000` | `150,000,000` | `150,000,000` | `150,000,000` | `150,000,000` |
| 7.1 Row 5 | 2023 | `250,000,000` | `40,000,000` | `10,000,000` | `60,000,000` | `140,000,000` | `140,000,000` | `140,000,000` | `140,000,000` | `140,000,000` |
| 7.1 Row 6 | 2024 | `180,000,000` | `30,000,000` | `15,000,000` | `45,000,000` | `90,000,000` | `90,000,000` | `90,000,000` | `90,000,000` | `90,000,000` |
| **7.1 Row 8 Total D** | — | — | — | — | **`185,000,000`** | — | **`185,000,000`** | **`185,000,000`** | **`185,000,000`** | **`185,000,000`** |
| 8.1 Row 4 | 2022 | `400,000,000` | `60,000,000` | `25,000,000` | `90,000,000` | `225,000,000` | `225,000,000` | `225,000,000` | `225,000,000` | `225,000,000` |
| 8.1 Row 5 | 2023 | `320,000,000` | `70,000,000` | `12,000,000` | `55,000,000` | `183,000,000` | `183,000,000` | `183,000,000` | `183,000,000` | `183,000,000` |
| 8.1 Row 6 | 2024 | `210,000,000` | `20,000,000` | `10,000,000` | `35,000,000` | `145,000,000` | `145,000,000` | `145,000,000` | `145,000,000` | `145,000,000` |
| **8.1 Row 8 Total D** | — | — | — | — | **`180,000,000`** | — | **`180,000,000`** | **`180,000,000`** | **`180,000,000`** | **`180,000,000`** |

- **UI vs `form.fill` at `2555a2d8`**: `refused=[]`, **0 XML diffs**, **0 print-map diffs** across all 50 NOLCO keys (including the official page-3 key quirk `frm1702MX:txtPg3IShed8_6C = "10,000,000.00"`).
- **What `2555a2d8` fixed vs `13e6abe4`**: At `13e6abe4`, `recompute_nolco_rows()` wrote unformatted integers (`"150000000"`, `"185000000"`) into `WholePesoInput.raw`, so the live print preview printed Col E and Row 8 without thousands commas. Commit `2555a2d8` added `set_display()` so both XML and paper now carry thousands commas (`"150,000,000"`, `"185,000,000"`).

#### Official Sample Box-by-Box XML Comparison (`1702MX-1225.official.xml`)

- **All 50 NOLCO boxes (`IShed7_*`, `IShed78D`, `IShed8_*`, `IShed88D`)**: **0 diffs** — exact character-for-character match with `1702MX-1225.official.xml` (`"300,000.00"`, `"100,000.00"`, `"0"`, `"50,000.00"`, `"150,000"`, `"50,000"`, untouched year `" "`, untouched amounts `"0"`).
- **Non-NOLCO diffs in `xml/1702mx-official-sample-diff.txt` (pre-existing on `main`, unrelated to PR #326)**: 48 value diffs (e.g. `drpPg3Sc1I11CB` `"0"` vs `"0.05"`, `txtPg3Sc5Itm17dC` `"0"` vs `"10000"`, title-cased text `"Peza"` vs `"PEZA"`) plus 97 static/modal/payment keys omitted when `to_official_field_map()` is used without `official_xml::write()`.

---

### 2.3 Claim 2 — Five Previously Unfillable Tables (tested at `13e6abe4`)

| Form | Column(s) | What a Filer Actually Reads in the Running App | True of Official BIR Program? | Matches PR #326 Claim? |
|---|---|---|---|---|
| **1600WP** | Nature of Payment, Tax Rate | Nature of Payment: `"Shows the ATC's nature of payment once this row's ATC is chosen."` Tax Rate: read-only numeric display showing **`₱ 0.00`** (peso sign on a percentage rate!). | True that both derive from ATC in official 1600WP; false that Tax Rate is in Pesos (`₱`). | **Partial (tweak)**: `"Can't be filled on this page yet."` is gone, but Tax Rate shows `₱ 0.00` instead of a rate hint or `%`. |
| **1601FQ** | Schedule 1 Nature of Income Payment (D) | Read-only grey box displaying **`-`** (no explanatory sentence at all). | True that Nature of Income Payment follows the chosen ATC in official 1601FQ. | **Partial (tweak)**: `"Can't be filled on this page yet."` is gone, but the filer sees only `-`, not `"Follows the ATC chosen in this row."` |
| **1701** | Schedule 6 Loss Carry-Over Col E | Non-editable computed display **`0.00`** under label `E. Net Operating Loss (Unapplied) [(E)= A – (B+C+D)]`. | True — computed by the form (`A - (B+C+D)`). | **Yes (clean)**: No `"Can't be filled..."`, non-editable computed value matches official program. |
| **2000** | Schedule 1 Tax Rate | `"Shows the ATC's rate once this row's ATC is chosen."` | True — rate follows the chosen DST ATC. | **Yes (clean)**: Clear, accurate filer-visible explanation. |
| **2553** | Rows 14–18 | Rows 14–17: `Printed: Philippine Amusement and Gaming Corp. (Pagcor), OT010, 5%`, `Printed: OT010`, locked `0.00 %` Tax Rate. Row 18: `Shows the classification of the ATC chosen in 18B.`, editable ATC / Taxable Amount / Tax Rate. | True — rows 14–17 are preprinted on official BIR Form 2553; row 18 takes an ATC and derives its classification. (Note: locked `0.00 %` on Row 14 before Taxable Amount is entered is minor.) | **Yes (clean)**: Accurate preprinted / ATC-derived explanations. |

---

## 3. Part B — Running App Persistence & Paper Comparison (`RULES.md`)

- **Save / close / reopen**: Verified for both 2551Q (all 6 rows + Part II persisted) and 1702MX (all 24 NOLCO amounts + years + Col E + Row 8 persisted).
- **Font size check (`RULES 3`, `>= 7.5pt`)**: Zero filled value cells under `7.5pt` on 2551Q or 1702MX (the only `< 7.5pt` text nodes are pre-printed official micro-labels from `html-frozen/*` such as `(MM/DD/YYYY)` at `6.0pt`–`6.96pt`).
- **Paper defects found (ranked by harm, with causality)**:
  1. **1702MX Page 4 — Schedules 7.1 & 8.1 amounts are left-aligned** (`html-frozen/1702mx-2018c/`, `RULES 4`: numbers right-aligned). **Already on `main`**.
  2. **1702MX Page 4 — Typed NOLCO amounts A–D print with `.00` centavos** (`300,000,000.00`) directly below the official header `"DO NOT enter Centavos"`, while Col E and Row 8 print whole pesos (`150,000,000`, `185,000,000`). **Caused by PR #326** (`crates/bir-core/src/forms/agent/mod.rs` writing `official_amount()` into `WholePesoInput.raw`, which is needed for official XML parity because `1702MX-1225.official.xml` itself has `"300,000.00"`, but `to_print_field_map()` prints `raw` verbatim).
  3. **1702MX Page 4 — Untouched Row 7 prints `0`** in A, D, E (`crates/bir-core/src/forms/form_1702mx_official.rs`, `RULES 4`: unused boxes stay blank). **Already on `main`**.
  4. **2551Q Page 1 & 1702MX Page 1 — Email address prints lowercase** (`juana.santos@example.com`) despite header `"Enter all required information in CAPITAL LETTERS"`. **Already on `main`**.

---

## 4. Part C — Nothing Else Moved

### 4.1 `sample_payloads_test` Counts
- **At `13e6abe4`**: **`142 passed; 0 failed; 1 ignored`**
- **At `origin/main` (`4399ce84`)**: **`144 passed; 0 failed; 1 ignored`** (PR branch is 15 commits behind `origin/main` `3aceafe8`, so it lacks two newer tests added on `main`: `form_2550q_schedule_3_rows_0_and_1_are_writable_via_agent_fill` and `form_2552_part_vi_num_shares_rows_2_to_5_are_writable_via_agent_fill`).

### 4.2 File-by-File Diff Reading (`origin/main...2555a2d8`)
14 files touched across the 5 PR commits (`2611c676`, `d9be2c95`, `80d880e4`, `13e6abe4`, `2555a2d8`):
- `crates/bir-core/src/forms/agent/probe.rs`: Adds `INDEXED_SELECTS` for 2551Q `drpATC1..6` and extends `ValueKind::Whole` echo matching to accept `parse_amount(official) == probe_value`.
- `crates/bir-core/src/forms/agent/mod.rs`: Syncs sibling `"raw"` string on `Binding::Whole` writes when path ends in `/amount` and parent JSON object already has `"raw"`. Adds unit tests for 24 NOLCO boxes and clear-to-zero.
- `crates/bir-core/src/forms/form_1702mx_official.rs` (`2555a2d8`): Adds `set_display()` so `recompute_nolco_rows()` and `apply_nolco()` populate `WholePesoInput.raw` with `official_display(value)` (thousands commas).
- `crates/bir-app-api/src/screen_form_module.rs`: Adds `NOTE_FOLLOWS_ATC` and updates `lock_note_for` / 2553 printed-row lock handling.
- `crates/bir-app-api/src/agent/session.rs`: Adds `form_fill_1702mx_nolco_populates_official_xml` and `form_fill_1702mx_nolco_matches_official_sample_xml_byte_for_byte`.
- `crates/bir-core/data/agent-forms/2551q-v2018.json`, `crates/bir-core/data/agent-forms/1702mx-v2018c.json`, `crates/bir-core/data/agent-carry.json`: **Verified regenerated by the repo's generator** (`sample_payloads::agent_forms::agent_forms_are_derived_from_the_samples` passes cleanly).
- `forms/schema-overrides.json`: Metadata updates for Claim 2 columns.
- Tests: `crates/bir-app-api/src/screen_form_module/tests/{dst_pt_vat,unfillable}.rs`, `nsh/screens/form/{atc2551q,nolco1702mx,unfillableF2}.test.tsx`.

### 4.3 Shared Whole-Peso Write Path Audit & Three Other Forms Checked
- **Audit of all 43 forms**: 574 total `Binding::Whole` boxes; **306** end in `/amount` (**196** in `1702MX`, **110** in `1702RT`). Only `Form1702MXDraft` uses `WholePesoInput { amount, raw }`, so the `slot.contains_key("raw")` guard in `agent/mod.rs` is a no-op on every other form.
- **Three other forms checked (`13e6abe4` vs `origin/main` `3aceafe8`)**:
  1. **`1702RT`** (4 `/amount` whole-peso boxes: `txtPg1Pt3I23DebitMemoC4Amount`..`I26OthersC4Amount`) — **0 XML diffs, 0 print HTML diffs**.
  2. **`1701`** (4 Schedule 6 NOLCO whole-peso boxes: `txtPg3IShed6_4Year`, `4A`, `4B`, `4D`) — **0 XML diffs, 0 print HTML diffs**.
  3. **`1701A`** (2 whole-peso boxes: `txtNumberAttachments`, `txtMonth`) — **0 XML diffs, 0 print HTML diffs**.

---

## 5. Verdicts (Ranked by Harm)

- **2551Q**: **`clean`** (for PR #326's changes; one pre-existing `main` fault noted below).
  - Pre-existing `main` fault (high harm for large filers): `crates/bir-core/src/forms/form_2551q.rs:2102` rejects Schedule 1 row tax due > 7 integer digits (`> 9,999,999.99`) when building upload XML, even though official Page 2 has 11 integer cells (**already on `main`**).
  - Pre-existing `main` fault (low harm): Item 11 email prints lowercase on Page 1 (**already on `main`**).
- **1702MX** (at `2555a2d8`): **`tweak`**
  - Fault 1 (medium harm, **caused by PR #326**): `crates/bir-core/src/forms/agent/mod.rs` syncs `WholePesoInput.raw` to `official_amount()` (`.00`), which is right for official XML parity (`1702MX-1225.official.xml` has `.00`), but `to_print_field_map()` prints `.00` on paper under `"DO NOT enter Centavos"` (breaks sheet instruction / consistency with whole-peso Col E).
  - Fault 2 (medium harm, **already on `main`**): `html-frozen/1702mx-2018c/` left-aligns all Schedule 7.1 & 8.1 amounts on Page 4 (breaks `RULES 4`).
  - Fault 3 (low harm, **already on `main`**): `crates/bir-core/src/forms/form_1702mx_official.rs` prints `0` in untouched Row 7 boxes A, D, E on Page 4 (breaks `RULES 4`).
- **1600WP**: **`tweak`** — Tax Rate displays **`₱ 0.00`** (peso sign on a percentage rate) instead of a rate hint or `%` (`crates/bir-app-api/src/screen_form_module.rs` / `nsh` rate widget; **PR #326 / pre-existing widget mismatch with Claim 2 wording**).
- **1601FQ**: **`tweak`** — Schedule 1 Nature of Income Payment (D) shows only **`-`** with no explanatory note, whereas Claim 2 says it reads `"Follows the ATC chosen in this row."` (`crates/bir-app-api/src/screen_form_module.rs`; **caused by PR #326**).
- **1701**: **`clean`** — Schedule 6 Column E is a non-editable computed `0.00` display with formula label `[(E)= A – (B+C+D)]`.
- **2000**: **`clean`** — Schedule 1 Tax Rate reads `"Shows the ATC's rate once this row's ATC is chosen."`, accurate to official Form 2000.
- **2553**: **`clean`** — Rows 14–17 clearly show preprinted classification/ATC and Row 18 explains `"Shows the classification of the ATC chosen in 18B."`
