driver: gpui-agent-cli 0.4.1

# PR #486 (04fa6dabf1f04af78c6be51cc3b131272cc316a8): the rdo-row-press suite on macOS, twice

Machine: macOS arm64 (Devin VM). App at the PR's exact head 04fa6dab (confirmed against the PR head before cloning),
hexuria/nsh beside it at that commit's NSH_PINNED_REF 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf. The branch pins the
plug (nsh-gpui-agent) at b559e40d - WITHOUT the type-focus fix of hexuria/nsh-gpui-agent#26 (still open, not merged).
Every run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, fresh scratch database,
`bash scripts/e2e/machines/macos.sh` unmodified (smoke then rdo-row-press, --allow-send inside smoke only), UI driven
only through gpui-agent. Recipes untouched. Folder layout: run1/, run2/ each with the smoke and rdo-row-press report
folders as the runner wrote them (report.md, report.json, run.log, receipts, screenshots); recipes/ has copies of the
recipes as run.

## Results

Run x suite x recipe -> result (first failing step / note). ms in the report.md tables inside each folder.

### Run 1 (evidence macos-20261127-150712 smoke, macos-20261127-150928 rdo-row-press)

smoke: setup-start pass; sign-up FAIL s20 (`assert context-pill name="Tax profile: $name"`): got
"Tax profile: E2E SMOKE TAXPAYER1 TEST ST" - the address typed after the name landed appended INSIDE the name box;
tax-year, navigate, open-2551q, money-boxes, sheet-press, delete-row, queue, birth-date, sign-out: not run
(sign-up failed and the suite needs it).

rdo-row-press: setup-start pass; rdo-row-press PASS 35198ms - all five row presses after typing "makati" (047, 048,
049, 050, 047), all three chevron rounds, all three presses away with the note; sign-up pass 7735ms; profile-pick-press
FAIL s7 (`click tp-branch-rdo_code-search`): occluded_by 7416 - the failure screenshot shows the sign-up toast
("E2E SMOKE TAXPAYER added. Add a tax year to choose its forms.") still covering the branch editor's RDO box.

### Run 2 (evidence macos-20261127-151652 smoke, macos-20261127-151705 rdo-row-press)

smoke: identical to run 1 - sign-up FAIL s20 with the same concatenated name, the rest not run.

rdo-row-press: setup-start pass; rdo-row-press PASS 34222ms; sign-up FAIL s20 same concatenation;
profile-pick-press not run.

## The two error-line rounds the job asks about

They live in profile-pick-press (the profile RDO and ZIP sections), which never reached them: in run 1 it stopped at
s7 (toast occlusion, before the first RDO round); in run 2 it did not run at all. NOT PROVEN on this machine:
the ZIP error-line round (Save as the blur, reopened ZIP shows "1200 - Makati CPO + Buendia Ave") and the RDO
error-line round (reopened RDO shows "047 - East Makati").

## What the failures are

- sign-up's concatenated name is the known macOS strict-adapter type-focus fault hexuria/nsh-gpui-agent#14, unfixed at
  this branch's pin b559e40d: the bare `type new-tp-registered_address` after `type new-tp-name` fires its keys before
  the focusing press is delivered, so the address appends into the still-focused name box (macOS posts mouse events to
  the app queue but sends keys synchronously). 3 of 4 sign-ups across the two runs hit it. PR #26 of the plug fixes it;
  this branch does not include it.
- profile-pick-press s7 in run 1 is a real occlusion a person would also hit: the added-taxpayer toast overlaps the
  editor's RDO row at this window size. gpui-agent correctly refuses the press.
- rdo-row-press itself - the subject of PR #486 - passes on macOS in both runs (2/2).
