driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent 3e78d30346208249e09bc440902c95e87c0bcf82 (local override)

# PR #486 (669996d91f3fc478b652cf78309a116f0b2bf730): the rdo-row-press suite on macOS, three machine runs - ALL GREEN

App at the PR's exact head 669996d9 (confirmed against the PR head before cloning; this head moves the ZIP click-away
presses to tp-address-registered_address), hexuria/nsh beside it (real copy) at NSH_PINNED_REF
8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf. `bash scripts/e2e/machines/macos.sh` unmodified, recipes untouched, UI only
through gpui-agent; every run BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, fresh scratch
database. Runs 1-2: plug dependency overridden LOCALLY and uncommitted to 3e78d303 (plug PR #26, the
hexuria/nsh-gpui-agent#14 fix; the branch pins b559e40d), verified through the compiled dep-info paths. Run 3: override
reverted, rebuilt at the pin (the fresh dep-info quotes the b559e40 checkout).

## Results (steps ok/total from each receipt) - the rdo-row-press suite passes END TO END in all three runs

smoke, all three runs identical in outcome (run1 macos-20261127-164501, run2 ...-165432, run3-pin ...-170253):
setup-start pass 3/3, sign-up pass 20/20, tax-year pass 15/15, navigate pass 29/29, open-2551q pass 5/5, money-boxes
pass 17/17, sheet-press skipped (no desktop variant), delete-row setup (no fp-table-*-add at resolve), queue fail 17/18
(s18 fp-send-line 20s timeout), birth-date pass 13/13, sign-out known gap 3/4 (no lock gate under BIR_AUTH_BYPASS=1).
The queue/sign-out/delete-row states are the ones documented in earlier proofs, not this PR's scope.

rdo-row-press (run1 macos-20261127-164828, run2 ...-165613, run3-pin ...-170438):

| recipe | result | steps | ms (run1 / run2 / run3-pin) |
|---|---|---|---|
| setup-start | pass | 3/3 | 644 / 653 / 645 |
| rdo-row-press | pass | 81/81 | 34446 / 33874 / 33836 |
| sign-up | pass | 20/20 | 8222 / 8036 / 7311 |
| profile-pick-press | **pass** | **196/196** | 107044 / 105939 / 105440 |

## The questions asked

- The three ZIP click-away rounds: PASS in all three runs - pressing tp-address-registered_address (the full-width box
  above ZIP) closes the list, the box keeps "1200 - Makati CPO + Buendia Ave", the note shows; nothing occluded.
- The ZIP error-line round: PASS in all three runs - "zzzz" typed, the Nothing line open, Save pressed straight while it
  is open (the Save press is itself the blur), the toast waited out, the editor reopened and the ZIP box shows
  "1200 - Makati CPO + Buendia Ave" (step 192's assert ok). The RDO error-line round passed likewise (047 - East Makati).
- No step was occluded in any of the three runs; no failure.png exists for profile-pick-press in this round.
- Run 3 at the pin b559e40d passes the same end to end; sign-up's #14 race did not bite in these three sign-ups
  (it is intermittent - see the 486-a/486-b proofs where it did).
