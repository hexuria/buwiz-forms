# Goal: finish issue #247 (form fields) on top of PR #259

Repo: `hexuria/buwiz-forms` (eBIRForms app; NSH UI in `nsh/`, Rust in `crates/`).
Tracking issue: https://github.com/hexuria/buwiz-forms/issues/247
Main PR: https://github.com/hexuria/buwiz-forms/pull/259 (branch `feat/form-fields-combined`, covers boxes 1–6)

Work unattended from step 0 to step 9. Stop only when every step's **Done when** holds, or a step is truly blocked (then finish
the other steps and say why in the final report). Do not stop to ask.

## Inputs (all in branch `handoff/form-fields-247`)

| File | What it is |
|---|---|
| `GOAL.md` | This file |
| `spec/PLAN.md` | The owner's picks (Q1–Q16) and the settled decisions. **Final: do not re-ask or redesign.** Its `/Volumes/...` paths are the owner's machine; ignore them and use the files in this branch. |
| `spec/picks/picks.txt` | The owner's answer to each question |
| `spec/picks/index.html` + `img/` | The picks page: real BEFORE shots and working AFTER mockups (JS in the page: `MOCKS.<pick id>`, e.g. `MOCKS.G1`, `MOCKS.P8a`). Open it in a browser. The picked mockup is the target look. |
| `spec/bugs.md` | B1–B21: each bug's title, planned fix and planned test |
| `review/pr259-review.md` | Review of #259: checklist gaps and code findings F1–F4 |
| `briefs/pr7-gated.md` | Spec for PR 7 (Q9 = G1) |
| `briefs/pr8-tin.md` | Spec for PR 8 (Q8 = P8a) |

The briefs mention `briefs/00-rules.md` and worktree folders. Those are not included; the **Rules** section below replaces them.
Line numbers in any of these files can be stale. **The code wins.**

Get them with:
```
git fetch origin handoff/form-fields-247
git worktree add ../handoff origin/handoff/form-fields-247   # or: git show origin/handoff/form-fields-247:GOAL.md
```

Branches you will work on (all already on GitHub):

| Branch | State |
|---|---|
| `feat/form-fields-combined` | PR #259. Boxes 1–6. You add fixes here (steps 1–3). |
| `feat/form-fields-gated` | PR 7 work: 3 commits (G1). Based on an older tip; lacks the last 3 commits of `feat/form-fields-combined` (#251, #252 and a merge of main). No PR yet. |
| `feat/form-fields-tin` | PR 8 work: the 3 gated commits + 6 TIN commits. Same old base. No PR yet. |

## Rules

- **Never merge a PR. Never force-push.** Bring branches up to date with `git merge`, not rebase.
- **Never edit NSH** (`hexuria/nsh`). It is a read-only dependency.
- **Never touch the eBIR Online credential fields** (`ebirOnlineUsername`, `ebirOnlineSecret`, `ebirOnlineConfirmUsername`,
  `txtFinalFlag`, `txtEnroll`). Q12 is the owner's. Do not investigate it or write about it.
- **Safety variables** on every run, test and screenshot: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`.
  Never set `BIR_SFTP_LIVE`. Tests use the demo taxpayer only. Nothing is ever sent to the BIR.
- Follow `nsh/AGENTS.md` (the reactive rule, kit controls from `nsh/ui`, tokens, no raw colours, text only inside `<Text>`).
- A message the filer sees never contains a draft key, a JSON path, a backtick or an `frm…` name.
- Generated files (`nsh/bir/types.d.ts`, `nsh/bir/functions.ts`): regenerate with `bash nsh/bir/regenerate.sh`, never hand-edit.
- Never weaken or delete a test to get green unless it is vacuous or pinned to old behaviour. Say which in the PR.
- Tests first: every fix starts with a test that fails on the current tree for the right reason.
- Allowed without asking: push commits to the three branches above, open PRs, comment on and close the superseded PRs listed in
  step 7, edit the #247 issue body, re-run CI jobs that failed on runner capacity ("The job was not acquired by Runner").

## Step 0: set up

1. Clone `hexuria/buwiz-forms`, and `hexuria/nsh` **beside it** (so `../nsh` resolves) checked out at the `NSH_PINNED_REF` in
   `.github/workflows/nsh.yml` (currently `95b5600c6acff85dce02e4690398705f2f5b059c`).
2. Mirror the CI setup in `.github/workflows/nsh.yml`: Rust toolchain as pinned there, `cargo install just --version 1.45.0 --locked`,
   and the NSH command line with `cargo install --locked --path crates/nsh-cli --root "$HOME/BuwizNsh/tools"` run inside `../nsh`;
   put `$HOME/BuwizNsh/tools/bin` on `PATH`.
3. Check out `feat/form-fields-combined` and run the gate once to get a baseline. Note any failure that already exists.

**The gate** (from the repo root, all green, every warning explained; known benign noise is in `nsh/AGENTS.md` pitfall 1):
```
just nsh-check -v && just nsh-build && just nsh-test
just core-plain-test
cargo test -p bir-app-api --no-default-features screen_form_module::    # plus any other Rust suite you touched
```
If your machine is not macOS, run what runs, and let CI cover macOS and Windows. Say which platform you ran on.

**Done when:** both repos are cloned side by side, `nsh -V` works, and the baseline gate result is written down.

## Step 1: fix F1, the typed "0" in one-date boxes (important)

Details in `review/pr259-review.md` § F1. On `feat/form-fields-combined`:
1. Add failing tests: a day typed "0" on 0619E's due date, and on 0605 Item 4, is refused with the plain date sentence at that box,
   and the Queue gate refuses.
2. Keep zero-as-blank only for 1707's whole-number parts. Fix both places (judging around line 756, the date helper around line 857
   of `rows_as_needed.rs`).
3. Run the gate. Commit and push.

**Done when:** the new tests were red before and green after, the 1707 tests still pass, and the commit is pushed.

## Step 2: fix F2, F3 and F4 (minor)

Details in `review/pr259-review.md`. On `feat/form-fields-combined`, one commit each, with a test each:
- F2: `profile_asked` returns a `Result` and the failure is logged or surfaced, not swallowed.
- F3: `settings_with_prefix` matches the prefix exactly (`substr(key, 1, length(?1)) = ?1`), not with `LIKE`.
- F4: `screen_form_profile_asked` refuses a tin that is not a TIN (use the app's existing TIN check if one exists).

**Done when:** three tested commits are pushed and the gate is green.

## Step 3: trace B11, B12, B18, B21 and Q6 to code and tests

For each, read its entry in `spec/bugs.md` (Q6 is the "Q6" row in `spec/PLAN.md`; it is B3 + B4's sentence table) and find the code
and the test that prove it on `feat/form-fields-combined`.
- B18 ("the test suite let these bugs ship") means `spec/PLAN.md` § Test plan items B–D: the sweep tests exist, the vacuous tests are
  gone (early returns in `ui/fields.select.test.tsx`, `.catch(() => {})` around presses, the `if (ask)` guard in session fixes), and
  the all-forms page-vs-print audit runs in CI.
- If one is fixed, write down the test name. If one is missing or partial, add the test first, then the fix, then commit and push.

**Done when:** each of the five has a test name in your notes, and any missing ones are built, tested and pushed.

## Step 4: PR 7, gated boxes hidden until their answer (box 7, Q9 = G1)

1. `git checkout feat/form-fields-gated && git merge origin/feat/form-fields-combined`. Resolve conflicts and keep both sides' intent.
2. Read `briefs/pr7-gated.md` and the `MOCKS.G1` mockup. Check each point of the brief against the 3 existing commits. Finish what is
   missing, tests first (session tests per gate shape, the all-forms sweep, page tests at 390 and 1440 pt).
3. Shots: the brief's `nsh/shots/ffgated.test.tsx` (`just nsh-shots ffgated`) at 420×900 and 1280×900, light and dark, BEFORE (from
   `feat/form-fields-combined`) and AFTER. If you cannot run real-window shots, use the page tests at 390 and 1440 pt as the proof
   and say so. Look at every PNG: a clipped, overlapping or invisible control is a failure.
4. Run the gate. Push. Open the PR against `feat/form-fields-combined`:
   - title `Form fields 7/8: gated boxes hidden until their answer (Q9 G1) (#247)`;
   - the body starts with: `Stacked on #259. Merge #259 first; this PR then retargets to main (check the base says main before merging).`
   - the body covers: what changed, the brief's points done or partial, tests added, row and part height change at 420 and 640 pt,
     shots, and anything not done and why.

**Done when:** the PR is open, every point of the brief is marked done or partial with a reason, and CI is green (or failing only on
something that already failed in the step 0 baseline, stated in the PR).

## Step 5: PR 8, TIN boxes fill the row on a phone (box 8, Q8 = P8a)

1. `git checkout feat/form-fields-tin && git merge origin/feat/form-fields-gated` (after step 4's push).
2. Read `briefs/pr8-tin.md` and the `MOCKS.P8a` mockup. Check each point against the existing commits. Finish what is missing,
   tests first (widths 1:1:1:1.4 at 320, 390 and 1440 pt; dashes only on desktop; "From the tax profile" only on a locked TIN; a typed
   TIN takes digits only; the all-forms TIN shape sweep).
3. Shots: `nsh/shots/fftin.test.tsx` at 320×700, 420×900 and 1280×900, light and dark, BEFORE (from `feat/form-fields-gated`) and AFTER,
   with the same fallback as step 4.
4. Run the gate. Push. Open the PR against `feat/form-fields-gated`, title
   `Form fields 8/8: TIN boxes fill the row on a phone (Q8 P8a) (#247)`, body starting with
   `Stacked on #<PR 7>. Merge #<PR 7> first; this PR then retargets to main (check the base says main before merging).`, same body
   sections as step 4.

**Done when:** same as step 4, for PR 8.

## Step 6: re-check #259 after your pushes

Look at #259's CI after steps 1–3. Fix any failure your commits caused. Re-run jobs that failed only on runner capacity.
Edit #259's description: add a short "Review fixes" section listing F1–F4 and the step 3 items with their commits and tests.

**Done when:** #259's CI is green (or any red is shown to be pre-existing), and the description is updated.

## Step 7: close the superseded PRs

For each of #250, #253, #254, #255, #256 and #257: comment `Superseded by #259, which carries this change. Closing so it can't be
merged into an already-squashed base.` and close it. Do **not** delete their branches.

**Done when:** all six are closed with that comment.

## Step 8: update issue #247

Edit the issue body's checklist so each box names the PR that carries it: boxes 1–6 → #259, box 7 → PR 7, box 8 → PR 8.
Do **not** tick any box: a box is ticked only when its PR has merged into `main`, which is the owner's call.

**Done when:** the checklist names a PR for every box, and no box is ticked.

## Step 9: final report

Post one comment on #247 with:
1. Each PR (#259, PR 7, PR 8): link, CI state, shots link or the fallback used.
2. F1–F4: fixed, with commit and test name.
3. B11, B12, B18, B21, Q6: already fixed or newly fixed, with the test name.
4. Each point of the PR 7 and PR 8 briefs: done or partial, with the reason.
5. Merge order for the owner: #259 → PR 7 → PR 8. After each squash-merge, retarget the next PR to `main` (`gh pr edit <n> --base main`)
   and merge `main` into its branch before merging it. Never merge a PR whose base was already merged.
6. Anything built differently from the pick, and why. Anything blocked.
7. A line for the owner: "The `handoff/form-fields-247` branch can be deleted now."

**Done when:** the comment is posted. Then stop. Do not delete the handoff branch yourself.
