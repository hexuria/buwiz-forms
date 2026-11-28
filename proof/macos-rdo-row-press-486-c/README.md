driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent 3e78d30346208249e09bc440902c95e87c0bcf82 (local override)

# PR #486 (2281b5eab88a65737f3ba2d313a0027db93fd7f0): the rdo-row-press suite on macOS, three machine runs

App at the PR's exact head 2281b5ea (confirmed against the PR head before cloning; this head adds the
`click tp-sec-taxpayer-toggle` step in profile-pick-press), hexuria/nsh beside it (real copy) at NSH_PINNED_REF
8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf. `bash scripts/e2e/machines/macos.sh` unmodified, recipes untouched, UI only
through gpui-agent; every run BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, fresh scratch
database. Runs 1-2: the plug dependency overridden LOCALLY and uncommitted to 3e78d303 (plug PR #26, the
hexuria/nsh-gpui-agent#14 fix; the branch pins b559e40d) - verified in the binary through the compiled dep-info paths.
Run 3: override reverted, rebuilt at the pin (the fresh objects are the b559e40 ones), same everything else.

## Results (steps ok/total from each receipt)

### Runs 1 and 2, override - identical outcomes

smoke (run1 macos-20261127-161145, run2 macos-20261127-162117): setup-start pass 3/3, sign-up pass 20/20, tax-year
pass 15/15, navigate pass 29/29, open-2551q pass 5/5, money-boxes pass 17/17, sheet-press skipped (no desktop variant),
delete-row setup (no fp-table-*-add at resolve), queue fail 17/18 (s18 fp-send-line 20s timeout), birth-date pass 13/13,
sign-out known gap 3/4 (no lock gate exists under the mandatory BIR_AUTH_BYPASS=1).

rdo-row-press (run1 macos-20261127-161517, run2 macos-20261127-162259):

| recipe | result | steps | ms (run1/run2) |
|---|---|---|---|
| setup-start | pass | 3/3 | 725 / 702 |
| rdo-row-press | pass | 81/81 | 35783 / 35000 |
| sign-up | pass | 20/20 | 8653 / 8486 |
| profile-pick-press | fail | 162/163 | s163 `click tp-address-phone` occluded_by 18500 |

### Run 3 at the pin b559e40d (macos-20261127-163058 smoke, macos-20261127-163245 rdo-row-press)

smoke: identical to the override runs, sign-up pass 20/20 (7679ms).
rdo-row-press suite: setup-start pass 3/3; rdo-row-press pass 81/81 (36067ms); sign-up FAIL 18/19 - s19
`wait_until nav-home` 10s timeout (the known nav-home flake, NOT the #14 concatenation; the taxpayer itself was fine);
profile-pick-press not run.

## profile-pick-press in detail (the override runs, 162/163 steps ok)

- The taxpayer-toggle fix WORKS: all three RDO click-away rounds now pass (the RDO list opens downward clear of
  tp-branch-name), after all five RDO row presses and the chevron rounds.
- RDO error-line round PASSED in both runs: "zzzz" typed, the nothing-line shows, tp-branch-save pressed, the editor
  reopened and the RDO box shows "047 - East Makati" (step 99's assert, ok).
- ZIP row presses and chevron rounds pass (through step 158, including the "1200 - Makati CPO + Buendia Ave" asserts).
- s163, the first ZIP click-away press on tp-address-phone, is occluded by the ZIP box's own open dropdown
  (node 18500) - failure.png in each run's folder: the address editor sits low in the macOS window, the open list
  covers the phone box, gpui-agent refuses the press. Same geometry the taxpayer-toggle fixed for the branch editor,
  now in the address editor. Deterministic: identical in both runs.
- The ZIP error-line round (steps 183-192: "zzzz", Save-as-blur at step 187, reopened ZIP shows
  "1200 - Makati CPO + Buendia Ave") is after the click-aways and was NOT REACHED.

## Answers asked

- rdo-row-press: 81 steps, all ok, in all three runs (5/5 passes counting the 486-b runs).
- RDO error-line round: PASSED both override runs. ZIP error-line round: not reached - blocked by the s163 occlusion.
- queue/sign-out/delete-row in smoke: unchanged known states, not this PR's scope.
