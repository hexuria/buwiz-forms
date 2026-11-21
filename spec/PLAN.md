# Form fields: plan from the owner's picks (2026-11-19)

Picks page (real before shots, working mockups, picks stored in its db): https://claude.ai/artifact/FrcmZWBzqoGAwCgcBjdajn
Local copy: `/Volumes/goldcoders/features/form-fields/picks/index.html` (open in a browser; images in `/Volumes/goldcoders/features/form-fields/picks/img/`), the owner's answers in `/Volumes/goldcoders/features/form-fields/picks/picks.txt`.
Probe evidence (root causes with file:line, before PNGs, critic reviews) — in `/Volumes/goldcoders/features/form-fields/evidence/`:
`evidence/probes.md`, `evidence/plan-draft.json`, `evidence/critics.json`, `evidence/before/*.png`.
Read `/Volumes/goldcoders/features/form-fields/evidence/critics.json` before building: it corrects several line numbers and claims in the draft plan.

## The picks (owner's answers; do not re-ask)

| Q | Pick | Means |
|---|---|---|
| Q1 | P1b | Missing tax-profile answer: ONE box (the item's own). No card, no second input. After the first non-empty value is typed and the box is left, ask once (Dialog on desktop, bottom Sheet on phone): "Save line of business to the tax profile?" → **Save to profile** / **Only this return**. |
| Q1·a | year | The answer is remembered per taxpayer **and tax year**, in a host setting (not browser storage). Not asked again for that TIN+year. |
| Q1·b | F2 | Facts with no box on the form (EOPT tier, birth date on 1700, income type, …): a small card **at the item it affects**, choices inline (Segmented/Select, never a second free-text box), one status chip ("Missing in tax profile" → "Saved to profile"). |
| Q2 | P2b | Dates are ONE field mapped to the month/day/year keys. Desktop: typed field with the calendar icon inside (P2a). Phone (< 600 pt): a 44 pt touch row; tapping opens a Sheet with the typed field + month grid + Done/Clear. |
| Q2·a | refuse | Two-digit years are refused: "Type the four-digit year, for example 08/17/1988." |
| Q2·b | block | Half-typed text on blur stays red and **Queue refuses** until fixed (the draft never silently keeps the previous date). |
| Q2·c | all | "August 17, 1988" (long month) everywhere in the app, not only forms. |
| Q3 | P3a | Month+year periods are ONE MM/YYYY field ("December 2023" when blurred) with a 12-month picker; parts locked by the return show as plain text. |
| Q3·a | R1 | From–to ranges are two date fields "From … to …"; each calendar is bounded by the other; wrong order = one sentence. 1600WP item 1 (six int boxes) becomes one range. |
| Q4 | C2b | 2–4 one-of choices → Segmented control; labels from the schema (not hard-coded Yes/No). |
| Q4·a | C5b | 5+ choices → Dropdown; more than 12 → SearchPick. "Others (Specify)" reveals its box right under it. |
| Q5 | P5a | Message line only when there is a message; each box's message under that box. |
| Q6 | P6a | One plain sentence under the box that is wrong; item number + what to do; never a key, path or backtick. |
| Q7 | P7k | Money rows: no change this round. |
| Q8 | P8a | TIN: keep four boxes; on phone they fill the row (grid 1:1:1:1.4); locked ones show "From the tax profile". |
| Q9 | G1 | Gated boxes are **hidden** until the answer that opens them, then appear right under it. A write to a gated box is refused at the host with its reason (no silent 0). |
| Q10 | T1 | To-do count and red from the start: no change. |
| Q11 | all | Fix B1–B21. |
| Q12 | look | eBIR Online username/secret keys inside 0605/2550Q/1701Q drafts: investigate and report to the owner. **Change nothing.** |
| Q13 | split | Wrong row titles: give the box its own row with the paper's title (1601C item 8, 0619E item 19, 0619F item 8, 1600PT, 1600VT, 1601FQ, 1700 DOB/address). Add a schema test over all 43 forms. |
| Q14 | schedule, int, pct, contact, select | Next round: probe these kinds and publish a new picks page. Do not build them. |
| Q15 | bugs-first | One bug-fix PR first, then one PR per question. |
| Q16 | push, pr, shots, ci | Agents may push branches, open PRs, run real-window shots, re-run CI on runner timeouts. **Never merge.** |

## Decisions the critics forced (settled here, apply everywhere)

1. **A cleared date part is blank, never `00`.** In the draft it is absent/None; a partial date prints blank parts. The draft's date object is `None` until all three parts are set. (B1 and B15 agree on this.)
2. **One-date calendar uses the split date's guards:** pass `dateRange` min/max into the one-date `CalendarPicker`, run typed and picked dates through the same calendar-year and shut-part rules, refuse with the existing sentence. The calendar opens on the box's month, else on the return's period month (not today).
3. **Profile writes happen only on an explicit answer** (the Q1 dialog's Save to profile, or an F2 card choice), never from autosave, never with half-typed text.
4. **Card removal is limited to facts that have a profile write route.** Land the routes (B9: birth date via update_taxpayer; EOPT, income type, start date, withholding, usual ATC via set_year_facts; option→value map for radio/checkbox) **before** removing any card.
5. **One message per problem:** when the fact check covers a box, drop the form's own duplicate required sentence for that key.
6. **ONE_DATES is looked up by form code everywhere** (incl. the 2552 identity lock at rows_as_needed.rs ~1262-1270 and the test array's type). Remove the unexplained 2552 skip in `refuse_dates` unless a test proves it is needed; add a 02/30 test for 2552.
7. **0605 ATC (Item 6) and Tax Type (Item 8) become catalog picks** that write the whole indexed code (Form0605IndexedCode::official), not free text into `/atc/code`.
8. Two-choice groups: once answered, a Segmented pair cannot be unanswered (current rule); groups of 3+ that are optional keep a Clear.
9. Measure row heights at 420 and 640 pt with shots before/after (shots/pixels.ts); state the change in each PR.
10. Accessibility on every new control: accessible name from the item label ("Pick Due Date", not "Open the calendar"), error tied to its box, keyboard type stated (date: numbers-and-punctuation; int: number-pad; TIN: number-pad).

## PRs (stacked, in merge order; each branch starts from the one before)

| # | Branch | Contents |
|---|---|---|
| 1 | `feat/form-fields` (from main) | **Bugs** B1–B4, B6, B8–B10, B12–B21 (B5/B7/B11 are Q1's UX → PR 4). Q6/P6a is B3+B4's sentence table, so it lands here. Q13 split rows = B13. B14 includes long month everywhere (Q2·c). Q12 investigation report goes in the PR body (no code change). **Tests first:** B18's tests land red on the current tree, then each fix turns them green. |
| 2 | `…-dates` | Q2 P2b + Q2·a + Q2·b: one-date for 0605 items 4 and 7 and every split M/D/Y group (22 by `dateParts` with overrides, incl. 1600WP, 1604C, 1606, 1706 items 1 and 7, 1707A, 1800, 1801, 0619E item 2); phone Sheet; Queue blocks on unparsed text. |
| 3 | `…-periods` | Q3 P3a (two-key mode + month picker; 16 pairs plus 1600PT, 1701, 1701A, 1701MS item 1, 0619E item 1) and Q3·a R1 ranges (1600WP item 1, 2550Q item 4, 1702EX item 17, 1701MS item 16). |
| 4 | `…-profile` | Q1 P1b + Q1·a + Q1·b F2, with B5, B7, B11 (no switch: the dialog replaces it), B9 routes already in PR 1. 1700's unbound "Container" box: count 1700 as card-only (F2). |
| 5 | `…-choices` | Q4 C2b + Q4·a C5b. Kit: add `testID` to SegmentOption and its Toggle. |
| 6 | `…-rows` | Q5 P5a (message line only when needed). |
| 7 | `…-gated` | Q9 G1 (+ B16's host refusal is already in PR 1). |
| 8 | `…-tin` | Q8 P8a. |
| — | no PR | Q7 (keep), Q10 (keep). |
| 9 | no code | Q14: probe schedule tables, int, pct, email/phone, code lists; publish a new /eli5 picks page (same shape as the first: real before shots, working after mockups, db picks). |

Record every branch and PR in `/Volumes/goldcoders/features/form-fields/FEATURE.md` (merge order).

**Stacked PRs must not strand.** On 2026-11-18 #234 and #236 were merged into base branches that had already been squash-merged into main, and never reached main (re-landed as #240, #242). Every stacked PR body starts with: "Stacked on #N. Merge #N first; this PR then retargets to main (check the base says `main` before merging)." Whenever a PR in the stack merges, the run (or whoever is watching) retargets the next one with `gh pr edit <next> --base main` and rebases it onto main.

## Gate for every PR (all green, no unexplained warning, before push)

From the PR's worktree root, NSH CLI 0.6.0 (`~/BuwizNsh/tools/bin`, matching main's pin `95b5600c`; `../nsh` is at that commit), dev env per nsh/AGENTS.md:
`just nsh-check -v && just nsh-build && just nsh-test` and `just core-plain-test`, plus the targeted Rust suites the PR touches (`cargo test -p bir-app-api screen_form_module::`).
Real-window shots: the PR's own `nsh/shots/<pr>.test.tsx` at 420×900 and 1280×900, light and dark, BEFORE (from the parent branch) and AFTER. Publish one private before/after artifact per PR and link it in the PR body.

## Test plan (from the probes; every PR adds its slice)

A. Field-kind matrix: `nsh/screens/form/fieldKinds.test.ts` (table-driven, FormSession) and `fieldPage.test.tsx` (page, 390 and 1440 pt). For each kind: type per keystroke; mid-text edit (pure reconcile tests in ui/mask.test.ts); paste (dashes, spaces, letters, leading minus); clear; blur formatting; draft-key round trip (save, close, reopen); print mapping; refusal sentence (no backtick, `/`, `draft has no`, `frm`); layout at 390/640 (no overflow, one message line per box).
B. Rust: `screen_form_module/tests/` fill+validate per kind; new `fields_0605.rs`; extend `every_open_date_box_takes_a_typed_date_on_every_form` (screen_form_module.rs ~15496) to split and one-date boxes and blank-seeded drafts; `agent/probe.rs` fails on any refused write for a blank seed; official field-map tests assert 0605 due date / return period keys.
C. Remove vacuous tests: early returns in ui/fields.select.test.tsx, `.catch(() => {})` around presses, the `if (ask)` guard in session.fixes. Move real-window-only cases to the shots lane.
D. Run the ignored all-forms page-vs-print audit (`screen_form_audit.rs audit_all_screen_forms`) in CI.

## Open items for the owner (collect in the final report; do not block on them)

- Q12 findings (credential keys in return drafts).
- Whether the owner's stored 0605 already has a due date (the calendar "worked" for them; on a new return it does not).
- Anything a PR could not do as picked, with the reason and what was done instead.
