# Independent check — pull request #315 (feat/store-launch-2550m-compute)

Commit tested: **6f5ddf724664c18aa866dd160c8624b15d7e4386** — the branch head was
exactly this when the work started (`git ls-remote` matched). Checker: a Devin
machine that did not write the change. Report only; nothing was fixed, pushed to
the pull request's branch, or commented on it.

## Verdict: **clean**, with two pre-existing paper faults and one borderline rounding note, none caused by this pull request.

The claim verifies end to end: leaving 13A fills 13B with 12% and the Schedule 4
standard input tax with 7% (5.00 until then); leaving each Item 18 base fills its
input tax with 12%; the seven boxes stay editable; a typed-over value is kept
until its base changes again; totals stay locked and are recomputed; a pre-change
draft keeps its old 0.00; rounding is half away from zero to two decimals.

## 1. The box table — official program vs the form page at 6f5ddf72

Source for "official": the inventory of the pinned official HTA
(rules/forms/2550m-v2007/fixtures/runtime-control-inventory-v796.json, HTA sha256
72f1422d…) and the formulas in calculations.json. Every static control in the
official HTA loads enabled (disabled=false, readonly=false) — the official
program never hard-locks a box in markup; the real distinction is behavioral:
totals are overwritten by the compute chain on every change, while the seven
boxes are written only when their base blurs.

| box | official program | form page at 6f5ddf72 | same? |
|---|---|---|---|
| 12A/12B | computed from Schedule 1 rows (sum amounts / sum output tax, L722/L726, `2550m-s1-totals`) | locked; computed from Schedule 1 rows | same |
| 13A | typed (L735) | typed | same |
| **13B** | filled 12% on 13A blur by compute13B (L6528–31), stays editable (inventory L738) | filled 12% on 13A entry, stays editable | same — **the fix** |
| 14, 15 | typed | typed | same |
| 16A/16B | computed totals (compute16A/16B) | locked; recomputed | same |
| 17A–17E | typed | typed | same |
| 17F | computed sum | locked; recomputed | same |
| 18A/18B | Schedule 2 totals carried up | filled from Schedule 2 rows; row input tax starts at 12% and stays editable | same |
| 18C/18D | Schedule 3A totals carried up | filled from Schedule 3A rows | same |
| 18E/G/I/K/N bases | typed | typed | same |
| **18F/H/J/L/O** | filled 12% on base blur by compute18TransactionbyRow (L6649–64), stay editable (inventory L857/866/876/886/903) | filled 12% on base entry, stay editable | same — **the fix** |
| 18M | typed | typed | same |
| 18P, 19 | computed totals | locked; recomputed | same |
| 20A/20B/20C | computed from Schedules 3/4/5 | computed from the schedule data | same |
| 20D/20E | typed | typed | same |
| 20F, 21, 22 | computed | locked; recomputed | same |
| 23A/23B/23C | computed from Schedules 6/7/8 | computed from the schedule data | same |
| 23D | typed, amended returns only, cleared when un-amended | typed when amended; cleared when un-amended (verified) | same |
| 23E/23F | typed | typed | same |
| 23G, 24 | computed | locked; recomputed | same |
| 25A–25C | enabled in the official inventory (L1065/1068/1071) | **computed from the due date in this app; direct entry refused** ("choose Edit penalties") | different — pre-existing design, not this PR |
| 25D, 26 | computed | locked; recomputed | same |
| txtlessStdTaxSched4 | static default 5.00, overwritten with 7% of 13A by changedTxtTax13A (L6688–90), enabled | same: 5.00 until 13A is entered, then 7%; editable | same — **the fix** |

Schedules 1–4 row boxes behave alike: row amount typed, row input tax filled at
12% on amount blur and editable after (verified for Schedule 2/3A by the PR's own
rows_2550m tests and the sample XML).

## 2. My arithmetic beside the app's and the XML's

Method: an independent harness (exact decimal arithmetic, integer cents, half
away from zero) against drafts filled through the screen write path
(`agent::fill_as_user` — the same writer the page saves with) at the commit, then
the upload XML. Amounts in the hundreds of millions with centavos, chosen on
rounding edges (x.005, x.004). Every computed box matched exact decimal
expectations:

| probe | input | exact decimal | app | XML |
|---|---|---|---|---|
| 13B of 13A = 300,000,000.005 | .005 edge | 36,000,000.00 | 36,000,000.00 | — |
| Sched4 std of same at 7% | .005 edge | 21,000,000.00 | 21,000,000.00 | — |
| 18F of 200,000,000.005 | .005 edge | 24,000,000.00 | 24,000,000.00 | — |
| 18H of 188,888,888.88 | | 22,666,666.67 | 22,666,666.67 | — |
| 18J of 133,333,333.33 | x.xx9996 edge | 16,000,000.00 | 16,000,000.00 | — |
| 18L of 100,000,000.004 | .004 edge | 12,000,000.00 | 12,000,000.00 | — |
| 13B with override | 250,000 base, 13B typed 29,999.99 | override wins | 29,999.99 | **29,999.99 in the payload** |
| 16B recomputed | 12,000.00 + 29,999.99 | 41,999.99 | 41,999.99 | **41,999.99 in the payload** |
| 18F of 40,000 | | 4,800.00 | 4,800.00 | **4,800.00 in the payload** |

Full-return chain (hundreds of millions, schedules 1/2/3A/4/5 populated): every
total hand-verified — 16A = 482,050,000.70, 16B = 51,606,000.00, 17F =
1,925,000.26, 18P = 36,205,001.04 (correctly includes the Schedule 3 amount as
18C), 19, 21, 22, 23G, 24, 26 all exact to the cent. Negative totals verified
both in the chain (22 negative when input tax exceeds output tax) and on paper
(pdf-page-1: 22 = -3,657,315,617.53, 24/26 = -4,061,662,112.76).

Two observations, neither a fault of this PR:
- **Float-level cent noise is inherent**: 20F came out one cent off the exact
  decimal sum (19,617.60 vs 19,617.59) because the inputs are binary floats
  added in sequence. The official JScript program computes in the same float64
  and would produce the same class of variance; no implementation in float64
  can do better. Not a defect — a documented property.
- **Rounding at exact .005 edges**: input 300,000,000.005 stores as
  300,000,000.01 (the epsilon-adjusted half-away rounding reproduces exact
  decimal behavior). Smaller .005 values like 10.005 can land as 10.00 because
  the double is genuinely below the edge. Whether the official program agrees
  at these exact edges is a documented evidence gap in the rules folder
  ("executable numeric vectors remain a documented gap") — the behavior is the
  best evidenced interpretation of formatCurrency.

## 3. What I tried beyond the claim

| probe | result |
|---|---|
| type over 13B, then unrelated edit | override kept (9,999.99 stays) |
| type over 13B, then re-enter base | 13B recomputed (24,000.00) — as claimed |
| same for 18E/18F pair | override kept, recomputed on base change |
| clear a filled base (empty) | base 0.00, paired tax 0.00, Sched4 std 0.00 — the overwrite-on-blur reading of the official handlers |
| 13A typed 0 explicitly | same: 13B 0.00, Sched4 std 0.00 (no longer the 5.00 default) |
| untouched 13A | Sched4 std stays the static 5.00 — as claimed |
| amended return | 23D editable, value kept; un-amending clears it (official optAmendFunc) |
| comma-formatted input "1,234,567.89" | accepted, normalized; 13B = 148,148.15 exact |
| padded input "  5000.5  " | trimmed, accepted |
| pre-change draft (13A set, 13B = 0.00) opened at 6f5ddf72 | 0.00 kept — verified at the compute path; the Check notice logic exists in rows_2550m::notices (fires only when base ≠ 0 and the paired box is 0.00, or Sched4 std still 5.00), is additive to the validation list as the established notice pattern, is dismissable and does not block |
| save → close → reopen | every value kept, including overrides (verified at database level and in the print) |
| official samples | reproduce at the head — see below |

The typed-over 18F (11,111,111.11) and 13B overrides reached the print preview
and the saved PDF — the paper shows what the user typed, the totals recomputed
around it.

## 4. Sample payloads at the head and at origin/main

`cargo test --locked -p bir-core --test sample_payloads_test`:
- at 6f5ddf72: **146 passed, 0 failed** (177 s)
- at origin/main (93820d0f): **144 passed, 0 failed** (178 s)

The pull request adds exactly two tests, both in
crates/bir-core/tests/sample_payloads/form_2550m.rs:
`form_2550m_screen_path_editable_tax_boxes_compute_and_preserve_overrides` and
`form_2550m_screen_path_computed_totals_stay_locked`. Every pre-existing official
sample reproduces at both commits — and the rebuilt samples now produce the same
pinned official XML when filled **through the screen path**, which is itself
strong evidence the screen path now agrees with the typed model.

## 5. The diff, file by file (`git diff -w origin/main...6f5ddf72`, 8 files)

- `crates/bir-core/data/agent-forms/2550m-v2007.json` — the seven boxes lose
  `computed:true` and gain amount bindings to their draft paths; consistent with
  the claim, entry by entry correct.
- `forms/schema-overrides.json` — matching evidence entries (computed:false)
  with citations to the inventory lines and probe.rs; texts agree with probe.rs.
- `crates/bir-core/data/agent-carry.json` — the seven keys drop out of the
  generated money list. Correct: amounts never carry, and they now carry the
  amount binding that decides that by rule. The file is generated
  (agent_carry_table) and matches its inputs.
- `crates/bir-core/src/forms/agent/mod.rs` — the write hook: 13A/18E/G/I/K/N
  writes route through the typed setters (`write_2550m_base_amount`), gated
  `kind == Form2550M`. Empty text writes 0.0. No other form's path changes.
- `crates/bir-core/src/forms/agent/probe.rs` — six additive FILER_TYPED evidence
  entries for the 2550M keys. Probe entries are per-key; other forms untouched.
- `crates/bir-app-api/src/screen_form_module.rs` — one line adding
  rows_2550m::notices to the notice list; the function itself returns empty for
  any other form.
- `crates/bir-app-api/src/screen_form_module/rows_2550m.rs` — the edit-order
  pins (base before paired tax box, schedule row amount before its input tax),
  the notices, and their tests. Edit-order is gated on form_code == "2550M".
- `crates/bir-core/tests/sample_payloads/form_2550m.rs` — samples rebuilt
  through the screen path plus the two new tests.

Generated files: agent-carry.json and the agent-forms bindings are generated
artifacts and are consistent with their sources; the workspace's staleness gates
pass on this commit family. Nothing in the diff reaches beyond the claim.
**No other form's upload or behavior changes**: the shared-file edits are either
gated on Form2550M or purely additive for 2550M keys, and every official sample
of every form reproduces identically at both commits.

## 6. Paper (print preview and saved PDF vs the official frozen pages)

The full invented return prints on two pages; every amount sits right-aligned in
its box, the character combs (TIN, period, RDO, ZIP) are filled digit-per-slot,
nothing is clipped or overlapping on page 1, totals print once in the right
boxes, negative totals print with a minus. Schedule 4 correctly shows the 7%
standard input tax (8,641,975.24 for the print return's 13A).

Faults visible on paper (neither caused by this pull request — it touches no
print path; both are the current print engine on main):
1. **Negative overpayment prints with a minus sign where the owner's print rules
   say brackets** — item 24 "Tax Still Payable/(Overpayment)" and item 26 print
   -4,061,662,112.76; form-review/RULES.md section 7 says brackets where the
   caption says "(Overpayment)". Severity: looks wrong on paper; pre-existing.
2. **Schedule headings on page 2 are crowded** against the blank official
   rendering, noticeably around the "(Attach additional sheet…)" parentheticals.
   Severity: cosmetic; pre-existing.

Environment limit, not an app fault: the desktop's real Save-as-PDF system
dialog cannot run inside this docker sandbox (bubblewrap user namespaces); the
PDF here came through the app's own save-PDF action on the test save path.

One design note outside the claim: items 25A–25C are computed-from-the-due-date
boxes in this app and refuse direct typing — the official inventory has them
enabled. That is pre-existing app design (the Edit penalties flow), unchanged by
this pull request; the Edit penalties button did not open inputs in the
June-2022 scenario tried, so penalties printed 0.00. Worth its own look
outside this check.

## 7. Fault ranking for 2550M at 6f5ddf72

| # | fault | harm | caused by this PR? |
|---|---|---|---|
| 1 | negative overpayment prints with minus, not brackets | looks wrong on paper | no — pre-existing print engine |
| 2 | crowded schedule headings on printed page 2 | cosmetic | no — pre-existing |
| 3 | cent-level float noise in multi-term totals (20F example) | inherent float64, matches official's own arithmetic class | no — documented property |
| 4 | penalty boxes not directly typeable; Edit penalties flow not reached in the June-2022 scenario | a return that needs manual penalties may not be completable — unverified either way | no — pre-existing design, needs its own check |

Nothing found that files a wrong number, blocks completing a return, or breaks
the claim. For 2550M the pull request is **clean**.
