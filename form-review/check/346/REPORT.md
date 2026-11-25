# REPORT — independent check of pull request #346 (2551Q amount limits)

**Commit tested:** `e945627c53e5751c75193a9c4b44ba13290d4f87` — branch head verified at start and re-verified before this report; it never moved.
**Compared against:** `origin/main` = `6bb90ad1316e0b2b6dc3c2440af607623dd60e5d`.
The checker did not write the change, fixed nothing, pushed nothing to its branch, commented nowhere.

## 1. The official limits, verified from the sheet and the program

**What the sheet draws** (`html-frozen/2551q-2018/index.html`, `data-comb-capacity`):

| box | cells drawn (integer + centavos) |
|---|---|
| Schedule 1 Taxable Amount, rows 1–6 (page 2, comb left=108) | 12 + 2 |
| Schedule 1 Tax Due, rows 1–6 (comb left=363) | 12 + 2 |
| Schedule 1 total, row 7 (top=272) | 12 + 2 |
| Part II items 14–24, every amount (page 1, cap=12) | 12 + 2 |
| Part III payment amounts (cap=7) | 7 + 2 — narrow, unchanged by the PR, not part of the claim |

**What the official program allows** (`rules/forms/2551q-v2018/`):
- `txtATCAmt1` is a typed box, `maxlength=12` (runtime-control-inventory-v796.json, source line 1971); rows 2–6 start disabled until an ATC is picked. fields.json: maxlen 12, normalization `NumWithComma`/`formatCurrency`, no min/max/pattern anywhere.
- `txtATCDue1–6` and `txtTotalSched1` are computed-only (disabled; maxlen 25/20 is irrelevant — programmatic writes ignore maxlength).
- Part II typed items 15/17/20/21/22: `maxlength=15`, typed. 14/16/18/19/23/24 computed (16 enabled only when amended).
- validations.json: 37 rules, none about amount size. calculations.json rounding: `round(this,2)` on blur — normalization happens when the box is left, NOT during typing (common-formats.json, string-util-runtime `round`).

**Is "12 characters as typed" the right reading? Yes.** Since nothing normalizes during typing, maxlength=12 counts raw keystrokes: digits, the decimal point, and any commas or minus the filer bothers to type. Maximum numerals typeable: `999999999999` (12 digits) whole, `999999999.99` (12 chars) with centavos, `9999999999.9` tenths. A filer typing commas fits strictly fewer digits — never more. So digit+decimal-point counting is the exact maximum envelope. `999999999.999` (13 chars) cannot be typed — the box truncates to `999999999.99`; the official also rounds extra precision away on blur. The app refuses >2-decimal entry instead (pre-existing fdb4c3a0 strictness, same numeric envelope). A value that is 12 chars typed and longer after formatting (e.g. `999,999,999,999.00` = 18 chars) is harmless — maxlength limits typing, not programmatic display. Negatives: the taxable box is an income amount; the app's finite-non-negative invariant (pre-existing) already covers it.

**Box by box — does the new limit accept everything the official program accepts and nothing it could not take:**

| box | official | ours after PR | verdict |
|---|---|---|---|
| taxable amount (typed) | ≤999,999,999,999 whole / ≤999,999,999.99 with centavos (12 typed chars) | identical envelope, digit+dot counting | same |
| tax due per row (computed) | unbounded programmatically; prints in 12+2 cells | 12 integer digits — the sheet's own width | same as the sheet; the only binding constraint the official has |
| Schedule 1 total | same | 12 integer digits | same |
| Part II typed (15/17/20/21/22) | maxlen 15 raw = exactly 12 integer digits + dot + 2 decimals | 12 integer digits + centavos | same numeric envelope |
| Part II computed (14/16/18/19/23/24) | programmatic | 12 integer digits — what prints | same as the sheet |

## 2. The edges, live and in the upload — before and after

Live = real application window, demo taxpayer, entry observed through the page's own scan; probe = `validate()` + `to_bir_xml_payload()` on the same draft. Exact words quoted. Main has no entry refusal at all (that gate is the PR's), so at main everything is "accepted at entry" and the refusal appears only at validate/XML — the complaint the PR describes, confirmed.

| case | main (6bb90ad1) | e945627c |
|---|---|---|
| taxable 999,999,999,998 | validate+XML REFUSED "does not fit the official 11-cell integer field" + 7-cell tax-due refusal | accepted at entry, validate clean, XML builds |
| taxable 999,999,999,999 | same refusals | accepted everywhere; XML `txtATCAmt1=999,999,999,999.00` |
| taxable 1,000,000,000,000 | refused (11-cell) | **refused at entry**, value never commits: "exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount." validate agrees |
| taxable 999,999,999.99 | accepted (fits 11 cells) | accepted |
| taxable 1,000,000,000.01 | accepted at entry then refused | refused at entry, same words |
| taxable pasted "999,999,999,999" | — | accepted; commas tolerated, value correct |
| taxable −500 | accepted at entry; validate "must be finite and non-negative" | same — entry does NOT refuse negatives (see faults) |
| tax due ≈9,999,999,999.99 (under 12 digits, over old 7 cells) | validate REFUSED "does not fit the official 7-cell integer field" | passes |
| tax due 299,999,999,999.70 (12 digits) | refused 7-cell | passes; prints exactly |
| item 15 = 999,999,999,999.99 | accepted at entry live, then Check REFUSED "does not fit the official 11-cell integer field" (live-verified) — refused at validate/XML too | accepted at entry AND passes Check; computed items correct |
| item 15 = 1,000,000,000,000 | — | refused at entry: "exceeds the official 12-digit peso field (at most 999,999,999,999.99); enter a smaller amount." |
| six rows at 999,999,999,999 each, rates summing 101% — total 1,009,999,999,998.99 (13 digits) | every row refused on old widths | rows pass individually; **the totals are refused** on total_tax_due/tax_payable/total_amount_payable — exactly as claimed |
| full top-of-range return that fits | refused at validate (13 errors) | validate 0 errors, XML builds |

**The words**: a filer told "exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount." knows exactly what the limit is and what to do. Entry/Check/Queue agree for every positive in-precision value tested.

## 3. A full return at the top of the range

Six rows, six ATCs (print-evidence set): PT010 999,999,999,999×3% = 29,999,999,999.97; PT180 999,999,999,999×30% = 299,999,999,999.70; PT140 999,999,999.99×18% = 180,000,000.00; PT150 888,888,888.88×18% = 160,000,000.00; PT170 666,666,666.66×15% = 100,000,000.00 (99,999,999.999 rounds half-up — matches the official `round(this,2)`); PT090 555,555,555.55×10% = 55,555,555.56 (.555 rounds up — my first hand-sum said .55; the app is right, half-away-from-zero as the official does). **Total 330,495,555,555.23** — the page's summary lines, the XML `txt14`/`txt19`, and the printed sheet all carry exactly this. (The XML probe used a PT160 sixth row instead of PT010 — same evidence class; `txtTotalSched1=400,495,555,555.16` there also matches its rows exactly.)

**Print** (`print/`): every amount right-aligns at the decimal point in its comb, one digit per cell. Row 2's tax due — 299,999,999,999.70, a value with **exactly 12 integer digits AND centavos** — fills all twelve integer cells exactly and the centavos print in the centavo cells. Nothing clipped, misaligned, or dropped. Part II items print the same way (item 24 at 436,371,894,078.72 with the draft's computed penalties). Compared with the frozen official page beside it, the layout matches cell-for-cell — description from the pictures, not the stylesheet.

**Print-preview caveat (platform, not the PR):** on this Linux machine the dev host's preview window never opens — the toast fires, no window exists, `save_pdf` reports "The preview is closed." The print HTML the same pipeline writes is what I rendered instead; the preview window gap is a pre-existing Linux dev-host limitation (the hosted CI never opens previews on Linux either — shots/base only).

## 4. Nothing else moved

- `cargo test --locked -p bir-core --test sample_payloads_test`: **main 144 passed / 0 failed / 1 ignored; e945627c identical, 144 / 0 / 1.**
- `git diff -w origin/main...e945627c` (two code files): the only validation changes are the two limit replacements — old 11-cell taxable / 7-cell tax-due / 11-cell Part-II rules removed, 12-digit rules added. No other check loosened or removed: finite/non-negative, cent precision, ATC required, rate consistency, election invariants all intact in the diff.
- fdb4c3a0 invariants — verified holding in the diff and exercised live/probe: item-13 election requiredness (fires in every probe run), taxpayer-type/income-election snapshot requiredness, year-end-month December rule, attached-sheets digit fit and schedule-continuation minimum, tax-relief specification, other-credit description length, expected-sales finite-non-negative, overpayment-disposition gating (fires in the −500 probe case), profile-changed-after-queue revalidation. None touched by the diff.
- The new entry refusal in `screen_form_module.rs` is gated `kind == Form2551Q` and chained `.or_else()` after the existing excise refusal — **no other form's entry behavior is touched.**

## 5. Five other forms — is any upload refused narrower than the sheet draws?

Picked: 1601C, 2550Q, 0619E, 1700, 1701Q. Drawn amount-comb capacities: 1601C up to 17, 2550Q 16/12, 0619E 11, 1700 up to 18, 1701Q 16/11. Code search across all five forms' validators and XML builders: **no amount-width validation exists in any of them** — amounts of any width are accepted and uploaded. The author's claim holds, trivially: no form refuses an upload for width at all. (The inverse — every other form accepts amounts wider than its sheet draws — is uniform pre-existing behavior, out of this PR's scope.)

## Faults, ranked by harm

1. **Entry/Check disagree for negative amounts (minor).** Typing −500 in a taxable amount is accepted at entry with no message (the refusal deliberately skips negatives), but Check/validate refuse it as "must be finite and non-negative". The claim says entry, Check and Queue agree. Harm is small — the value is caught before filing and the words are clear — but a filer sees no guidance where they type. Caused by the PR's entry-refusal design (the underlying non-negative rule is fdb4c3a0's). File: `crates/bir-app-api/src/screen_form_module.rs` (`form_2551q_amount_refusal` skips negative values).
2. **Refusal wording inconsistent between entry and Check (cosmetic).** Entry says "enter a smaller amount", Check says "reduce the amount so it fits". Same meaning, different advice. Caused by the PR. Files: `crates/bir-app-api/src/screen_form_module.rs` vs `crates/bir-core/src/forms/form_2551q.rs` message builders.
3. **Refused typed text lingers in the box (cosmetic, arguably correct).** After a refusal the row line and box keep showing the over-limit text while the committed value stays unchanged — visible in the film: the box shows 1,000,000,000,000 while item 18 sums the old 15,000,000,000.00. A filer could misread the sheet as containing the refused number, though nothing files it. Behaviour predates the PR for other refusals; the amount path inherits it.
4. **Linux dev-host print preview does not open (platform, pre-existing on main).** Not caused by the PR; noted so the evidence gap is explained.

## Verdict for 2551Q: **TWEAK** — the change does exactly what it claims and breaks nothing, but two small faults are its own: negative amounts pass entry unguided while Check refuses them (a gap in the claimed entry/Check/Queue agreement), and the entry/Check wording differs. Neither blocks a filing nor misleads a filer into a wrong return. Every claim verified: the old 7/11-cell refusals are gone and were never the BIR's; the new limits match the sheet's drawn cells and the official typed-box envelope exactly; entry, Check and Queue agree for all positive in-precision values; the thirteenth-digit totals refusal works; nothing else moved.

### Environment notes
Evidence gathered on the Linux fleet machine in the Ubuntu 24.04 sidecar with the four safety variables set, never `BIR_SFTP_LIVE`, never Queue pressed, demo taxpayer only, scratch databases. Probes and fixtures were untracked files, never committed to any branch. Main-side live evidence was gathered for the key comparison (shots/c346main-*): at main, item 15 typed 999,999,999,999.99 is accepted at entry with no message (value commits, `msg=-`) and Check then refuses it — "Item 15 creditable tax withheld does not fit the official 11-cell integer field" (plus the cascading 18/19/24 refusals) — the exact complaint the PR describes, reproduced live. At e945627c the same value is accepted at entry AND passes Check. The main-side Schedule-1 row boxes could not be typed through the checker fixture at main (the PR-era row drawer differs); the probe covers those rows at validate/XML level, and the entry path at main provably has no refusal gate at all (the PR adds it).
