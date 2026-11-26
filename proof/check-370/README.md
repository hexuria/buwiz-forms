# check-370 — independent Windows check of PR #370 (fix/2551q-amount-decimals @ a70b3edd)

Report-only validation. Nothing fixed, nothing pushed to the PR branch. Probes never pressed
Queue/Submit/Final Copy; BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 on every run;
demo taxpayer (Juana Santos, TIN 000-123-453) and invented amounts only.

## 1. The deciding question — which rounding runs officially — SETTLED on the pinned files

Official evidence: hexuria/buwiz-validation, the byte-exact BIR public download
(Offline_eBIRForms_Package_v7.9.6.2.1), hash-checked here against the repository manifest
`manifests/7.9.6.2.1.json`:

| file | pinned sha256 (manifest) | extracted sha256 | match |
|---|---|---|---|
| package/forms/BIR-Form2551Qv2018.hta | dc5c19710400e9bd15ceccd1bbeafc5290074b17b2d460af5f8921c6a7f93186 | dc5c1971…3186 | YES |
| package/js/string-util.js | b0aaae7ccf6f0839d0aff54b0b221fbebf67adc7f660a58ebc03b26e1fb40ff5 | b0aaae7c…ff5 | YES |

(copies committed here as pinned-BIR-Form2551Qv2018.hta / pinned-string-util.js / pinned-sha256.txt)

**The money box's blur is `round()`, never `blockletter()`.** Pinned HTA:

```
L1971-1972  <input type="text" style="width: 95%; text-align: right;" id="txtATCAmt1" value="0.00" size="17" maxlength="12"
            onkeypress="return numbersonly(this, event)"
            onblur="round(this,2); computeATC(1); computeAll();" />
L1084-1085  <input type="text" value="0.00" name="frm2551Qv2018:txt15" maxlength="15" id="frm2551Qv2018:txt15" …
            onblur="round(this,2); computeAll();" />
```

`round()` (pinned string-util.js L286-318): strips commas, truncates the integer part to 12 digits,
then `num = Math.floor(num * 100 + 0.50000000001);` — half-up with the epsilon — comma-formats back into the box.
If the integer part exceeds the allowed precision the box becomes "0.00".

**Tax and totals use the same rounding.** computeATC (pinned HTA L2998-3001):
`taxDue = NumWithComma(txtATCAmtN) * rate/100`, written with `formatCurrency(taxDue)`;
the Schedule-1 total and items 18/19/23/24 are likewise written with `formatCurrency` (L3007-3042).
`formatCurrency` (pinned string-util.js L321+): same `Math.floor(num * 100 + 0.50000000001)` (truncating at 15 integer digits).

**blockletter is dead code.** `blockletter` (L4421-4430) is the `parseFloat(e.value).toFixed(2)` function the
validations.json source-ref names — but `blockletter(` occurs exactly ONCE in the entire HTA: its own definition.
Zero controls call it. (Its sibling `blockletterWithout2Decimal` has exactly one caller, txtYear L469 — a year field.)
So the validations.json source-ref `official-hta-runtime#blockletter:L4421-L4430` is a misattribution:
the money boxes never run toFixed(2); they run round(), and the computation runs formatCurrency — both floor(x*100+0.50000000001).
**On binary ties the official behaviour is half-up: 1.005→1.01, 2.675→2.68 — the same as the PR's `cents()`. The PR's premise is correct.**

**Mirror comparison:** the kapitanluffy/ebirforms mirror copies used in the first pass are NOT byte-identical
(HTA sha256 ad2d34e9… vs pinned dc5c1971…; string-util.js 9460116f… vs b0aaae7c…, 293 diff lines — the mirror is an
older package version), but the rounding logic in both files is line-for-line identical, so all probe evaluations stand.

Method note: the official program itself does not run on this machine (it is not installed and the installer
was not run); the blur handler and shared rounding were taken from the pinned official runtime source and
evaluated in node (probe.js, results in probe-output.txt). Officially, `numbersonly` rejects everything but
digits and '.' at keypress, so "1,000.005" and " 12.345 " cannot be typed into the official box — they are
paste-only probes there; our app accepts them typed.

## 2. Probe table — official (pinned-source evaluation) vs our app @ a70b3edd (real Windows window)

Probes into Schedule 1 row 1 taxable amount (txtATCAmt1, ATC PT010 3%) and item 15
(frm2551Qv2018:txt15, Creditable Percentage Tax Withheld). "official used in tax" = what computeATC computes with. Our
stored values read from the draft database (form_drafts.data_json).

| probe | official shown | official used in tax | ours shown | ours stored | match |
|---|---|---|---|---|---|
| 2000.005 | 2,000.01 | 2,000.01 (due 60.00) | 2,000.01 | 2000.01 | YES |
| 0.005 | 0.01 | 0.01 (due 0.00) | 0.01 | 0.01 | YES |
| 1.005 | 1.01 | 1.01 (due 0.03) | 1.01 | 1.01 | YES (toFixed-counterfactual 1.00 wrong) |
| 2.675 | 2.68 | 2.68 (due 0.08) | 2.68 | 2.68 | YES (toFixed-counterfactual 2.67 wrong) |
| 1234.565 | 1,234.57 | 1,234.57 (due 37.04) | 1,234.57 | 1234.57 | YES |
| 99999999.995 | 100,000,000.00 | 100,000,000.00 (due 3,000,000.00) | pasted: 100,000,000.00 | 100000000.00 | YES when pasted — TYPED mangled, see §5 |
| 1,000.005 | 1,000.01 | 1,000.01 (due 30.00) | 1,000.01 | 1000.01 | YES |
| " 12.345 " | 12.35 | 12.35 (due 0.37) | 12.35 | 12.35 | YES |
| 100.0049 | 100.00 | 100.00 (due 3.00) | 100.00 | 100.00 | YES |

Item-15 probes (txt15) gave the same shown/stored values (stored as creditable_tax_withheld in the draft),
including the paste-correct / typed-mangled split for 99999999.995. No probe triggered a refusal —
none of the nine is negative or over-limit after rounding — so no refusal sentences were observable in this matrix.

## 3. Tests at a70b3edd (logs committed)

- `cargo test -p bir-core form_2551q_entry_check_and_queue_agree_on_amount_limits` — PASS, 1 passed, 795 filtered (test-2551q-amount-limits.log)
- `cargo test -p bir-app-api --lib` — PASS, 789 passed; 0 failed; 7 ignored; 211.67s (test-bir-app-api-lib.log)

## 4. Diff review (a8aeaca8..a70b3edd)

- The diff touches only the listed 2551Q boxes: `is_2551q_amount_entry_key` covers exactly txtATCAmt1..6 and
  frm2551Qv2018:txt15/16/17/20/21/22 (crates/bir-app-api/src/screen_form_module.rs L5435).
- Typed entry and an assistant's form.fill go through the SAME path: `ChoiceStep::Fill(value)` routes to
  `form_2551q_amount_entry_value` (L5355), which trims commas/whitespace, parses f64, applies `cents()` before the write.
- Refusals after rounding: txtATCAmt keys refuse when the rounded value fails `fits_taxable_amount_input`
  (12-typed-character limit: at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos) or is negative
  ("Must be zero or greater; enter a non-negative amount."); txt15-22 keys use the 12-integer-digit comb limit
  (`amount_comb_limit_message`). Nothing outside the 2551Q amount-entry path changed in behavior.

## 5. NEW FAULT characterized — keystroke mangling of large typed amounts (report-only; NOT caused by #370's rounding logic)

Typing `99999999.99` or `99999999.995` key by key into txtATCAmt1 silently stores a WRONG amount.
Pasting the identical string stores the correct amount. Key-by-key trace (keytrace/key1..12.png, real OS keys,
box read after each key; keys of "99999999.995"):

| after key | keys pressed | box shows |
|---|---|---|
| 1-3 | 9 / 99 / 999 | 9 / 99 / 999 |
| 4 | 9999 | 9,999 (live comma formatting correct) |
| 5-7 | … | 99,999 / 999,999 / 9,999,999 correct |
| 8 | 99999999 | text correct "99,999,999" but the CARET sits BEFORE the final 9 |
| 9 | . | "9,999,999.9" — the dot inserted before the last 9; the 8th nine is stranded after the decimal point |
| 10 | 9 | "9,999,999.99", caret again before the final digit |
| 11 | 9 | box rewrites to "10,000,000.00" mid-edit |
| 12 | 5 | "10,000,000.01" |
| Tab out | | stored taxable_amount **10000000.01**, tax due 300,000.00 (draft DB) — meant 99,999,999.995→100,000,000.00/3,000,000.00 |

The same probe typed again produced 10,000,000.00 — the mangling is NON-DETERMINISTIC (timing-dependent caret race).

Boundary probes (typed the same way; keytrace/b*-lastkey.png and b*-aftertab.png):

| typed | stored | tax due | correct? |
|---|---|---|---|
| 12345678.90 | 12345678.90 | 370,370.37 | YES correct |
| 123456789.12 | 123456789.12 | 3,703,703.67 | YES correct |
| 99999999.99 | **10000000.00** | 300,000.00 | NO — mangled even with only TWO decimals |
| 1000000.005 | **0.01** | 0.00 | NO — catastrophic loss |

Because 99999999.99 (no third decimal anywhere) mangles, the fault is NOT in the rounding this PR adds;
it is in the money input's live comma reformatting/caret restoration: once the string reformats at the
8th keystroke the caret lands one position early, and every later key inserts at the wrong position.
Whether a filer can end up with a stored amount different from what they meant without noticing: YES —
the box shows a plausible formatted amount, no error or refusal appears, the wrong value is written to the
draft and the tax due is computed from it. (Screenshots: keytrace/, amt1-p6-typed-mangled vs pasted for both boxes.)

## STATUS — what is done and what is not (pushed mid-job on orchestrator request)

DONE: hash-checked pinned-source re-read and verdict (section 1); all 9 probes x both boxes (txtATCAmt1 AND txt15)
at a70b3edd with screenshots and draft-DB stored values; both test lines (section 3); diff review (section 4);
typing-fault characterization at a70b3edd: full key-by-key trace, boundary probes, non-determinism, silent-wrong-value answer.
NOT DONE: the comparison build/run of main 1b18d668 (build was started but not finished when stopped — whether main
mangles identically, and so whether #370 is involved, is UNANSWERED here; the fault is in the input control's caret
handling which the #370 diff does not touch, but that is inference, not measurement). Refusal sentences for
negative/over-limit amounts were not probe-triggered (read from source only).

## 6. Provenance

- App: hexuria/buwiz-forms a70b3edd0a0b0be85de25846227611c5a8a929b8, fresh clone, own build folder,
  RUSTC_WRAPPER empty, toolchain cargo 1.99.0 first on PATH, kit at pin 8c78cfd6, `nsh update` never run.
- Official source: hexuria/buwiz-validation pinned files (section 1), evaluated with node v24 — probe.js/probe-output.txt committed.
- Machine: Windows Server 2022 Standard build 20348, UAC off; app driven with real OS-level keystrokes
  (gpui-agent key events do not reach the in-flight edit control — a driving caveat, not app behaviour).
