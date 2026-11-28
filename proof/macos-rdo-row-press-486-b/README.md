driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent 3e78d30346208249e09bc440902c95e87c0bcf82 (local override)

# PR #486 (6417adbcf3f1d75d81af28ebf1ce49880a27ee38): the rdo-row-press suite on macOS, three machine runs

App at the PR's exact head 6417adbc (confirmed against the PR head before cloning), hexuria/nsh beside it (real copy)
at that commit's NSH_PINNED_REF 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf. `bash scripts/e2e/machines/macos.sh`
unmodified, recipes untouched, UI only through gpui-agent; every run BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1
BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 with a fresh scratch database.

## The override (runs 1 and 2 only)

This branch pins the plug at b559e40d, which still has hexuria/nsh-gpui-agent#14 (a bare type after a focused box can
fire its keys before the focusing press is delivered on macOS). For runs 1 and 2 the plug dependency was overridden
LOCALLY and uncommitted: the five plug git deps' `rev` in nsh/Cargo.toml set to 3e78d30346208249e09bc440902c95e87c0bcf82
(the head of plug PR #26, the #14 fix). Verified in the binary: the dep-info of the compiled libnsh_agent_actions quotes
`.cargo/git/checkouts/nsh-gpui-agent-430c25c17b0dac6b/3e78d30/crates/nsh-agent-actions/src/lib.rs`. Run 3 reverted the
override (`git checkout -- nsh/Cargo.toml`) and rebuilt at the pin; the pin's objects
(`.../b559e40/...`) are the fresh ones of that build.

## Results (step counts are the receipt's steps ok/total)

### Runs 1 and 2, plug at 3e78d303 (override) - the two runs were IDENTICAL in outcome

smoke (run1 evidence macos-20261127-153748, run2 macos-20261127-154604):

| recipe | result | steps | ms (run1/run2) |
|---|---|---|---|
| setup-start | pass | 3/3 | 632 / 680 |
| sign-up | pass | 20/20 | 8394 / 8526 |
| tax-year | pass | 15/15 | 6667 / 6522 |
| navigate | pass | 29/29 | 9360 / 9069 |
| open-2551q | pass | 5/5 | 1936 / 1896 |
| money-boxes | pass | 17/17 | 17232 / 17245 |
| sheet-press | skipped | - | no desktop variant |
| delete-row | setup | - | no fp-table-*-add node at resolve time |
| queue | fail | 17/18 | s18 `wait_until fp-send-line` 20s timeout |
| birth-date | pass | 13/13 | 4204 / 4360 |
| sign-out | known gap | 3/4 | no lock gate exists under the mandatory BIR_AUTH_BYPASS=1 |

rdo-row-press (run1 macos-20261127-154123, run2 macos-20261127-154748):

| recipe | result | steps | ms (run1/run2) |
|---|---|---|---|
| setup-start | pass | 3/3 | 725 / 715 |
| rdo-row-press | pass | 81/81 | 36387 / 35394 |
| sign-up | pass | 20/20 | 8592 / 8361 |
| profile-pick-press | fail | 66/67 | s67 `click tp-branch-name` occluded_by 12098 |

### Run 3, plug at the app's own pin b559e40d (no override; evidence macos-20261127-155601 smoke, macos-20261127-155752 rdo-row-press)

smoke: same table as above INCLUDING sign-up pass 20/20 (7609ms) - the race did not bite in this smoke run.
rdo-row-press suite: setup-start pass 3/3; rdo-row-press pass 81/81 (34490ms); sign-up FAIL 19/20 - s20
`assert context-pill name="Tax profile: $name"`: got "Tax profile: E2E SMOKE TAXPAYER1 TEST ST" (the #14 concatenation);
profile-pick-press not run.

## The questions asked

- rdo-row-press step count: the receipt holds 81 steps, all ok in all three runs (the recipe file has 81 non-comment lines).
- profile-pick-press error-line rounds: NOT REACHED in any run. Steps 1-66 pass in both override runs - every RDO row
  press (047/048/049/050/047) and every chevron round - but s67, the first click-away press on tp-branch-name, is
  occluded by the RDO box's own open dropdown list (node 12098): in the macOS window the branch editor sits low, the
  list opens upward and covers the branch-name box (failure.png in each run's folder). gpui-agent refuses the press, as
  it should. The error-line rounds (ZIP "1200 - Makati CPO + Buendia Ave" after the Save-as-blur, RDO "047 - East
  Makati" reopened) sit after the click-away rounds and were never executed. In run 3 sign-up failed before them.
- queue in smoke reaches s18 (`wait_until fp-send-line`) and times out in all three runs; sign-out is the documented
  no-lock-gate gap; delete-row resolves no fp-table-*-add (documented in earlier proof).

## sign-up at the pin b559e40d across today's macOS runs (unmodified sign-up.wants)

| suite | pass | fail (s20, pill shows "...TAXPAYER1 TEST ST") |
|---|---|---|
| smoke | 1 (run-3-pin at 6417adbc) | 2 (both runs at 04fa6dab) |
| rdo-row-press | 1 (run 1 at 04fa6dab) | 2 (run 2 at 04fa6dab, run-3-pin) |

(The #14 proof's before-run is not counted here: it ran the probe variant sign-up-bare.wants without the click-first
workaround, not the suite's unmodified sign-up.wants - it failed with the name box empty.)
