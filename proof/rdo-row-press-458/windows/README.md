driver: gpui-agent-cli 0.4.1
recipe: agent-recipes/e2e/rdo-row-press.json (rdo-row-press.wants, profile-pick-press.wants)

Commits:

- R `4fa8ac8b3d2185f3658df693bf580caf46681d8d` (suite/recipe source)
- BEFORE `c42626973461a21a2e8ab58caca2405ff0891252`
- AFTER `96e9ed4ea9d9aa1fb290947b9f7261b271a4086a` (PR #458 head)

Press path: gpui-agent's press on Windows is `nsh-platform-windows/src/agent.rs:121-133` (`click`), which calls `synth::pointer` (`synth.rs:21-39`), which calls `crate::input::pointer`, whose press path calls `focus_target`/`focus_pointer` (`input.rs:1707-1719`). I checked those lines in the kit checkout at `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; they match. This is the same pointer/focus path as a mouse click, not a direct press or a UIA Invoke.

No real mouse or keys, UI Automation, SendInput, or machine-native input was used. Screen videos are observation only. Every app launch used a fresh scratch database, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, no `BIR_SFTP_LIVE`, and no Google OAuth environment variables. The runner detected gpui-agent-cli 0.4.1 and used the paced primary recipes; the unpaced fallback was not used.

## Suite results

| folder | app commit | setup-start | rdo-row-press | sign-up | profile-pick-press |
|---|---|---|---|---|---|
| `before-1/` | `c4262697` | PASS | FAIL | PASS | FAIL |
| `before-2/` | `c4262697` | PASS | FAIL | PASS | FAIL |
| `after-1/` | `96e9ed4e` | PASS | PASS | PASS | FAIL |
| `after-2/` | `96e9ed4e` | PASS | PASS | PASS | FAIL |

BEFORE, both runs:

- `rdo-row-press` first failure: round/row press 1, step `s7`, op `assert`, target `new-tp-rdo_code-search`; line `assert new-tp-rdo_code-search value="047 — East Makati"`; error `expected "047 — East Makati", got Some("")`; box held empty.
- `profile-pick-press` first failure: first profile RDO row press, step `s13`, op `assert`, target `tp-branch-rdo_code-search`; error `expected "047 — East Makati", got Some("040 — Cubao")`; box held `040 — Cubao`.

AFTER, both runs:

- `rdo-row-press` PASSED: rows 5/5, chevron 3/3, click-away 3/3 with the note.
- `profile-pick-press` first failure: step `s89`, op `wait_until`, target `tp-branch-rdo_code-list`; line `wait_until tp-branch-rdo_code-list --timeout-ms 3000`; error `wait_until timed out: node tp-branch-rdo_code-list not found`; box held `zzzz`.
- The RDO row/chevron/click-away rounds completed before `s89`; the failure is the RDO error-line round. The ZIP rounds did not run in either AFTER run.

## 2551Q SearchPick placement check

`after-form/open-2551q-receipt.json` shows `open-2551q-windows.wants` failed before opening a return: step `s2`, `wait_until new-return-2551Q --timeout-ms 5000`, error `wait_until timed out: node new-return-2551Q not found`.

`after-form/form-snapshot.json` is therefore a snapshot of the page gpui-agent actually reached (the taxpayer profile page), not a 2551Q form page. The analyzer found one id with both `-search` and `-trigger`: `tp-branch-rdo_code`. Its ancestor chain is in `after-form/form-searchpicks.md` and `.json`; the chain reaches `main-scroll`, and the tree does not report press-taking or focusable fields beyond role/enabled/focused. Because the 2551Q form did not open, form SearchPick rounds are `known_gap`; no OS input was used. `after-form/form-searchpick-rounds.wants` records that known gap.

## Files

- `before-1/`, `before-2/`, `after-1/`, `after-2/`: `report.json`, `report.md`, `run.log`, receipts, screenshots, failure trees, runner output, and app console output.
- `after-form/`: open-2551Q receipt, snapshot, SearchPick chain analysis, and the known-gap custom recipe note.
- `before.mp4` and `after.mp4`: observe-only screen recordings of the two runs for each commit.
- Recipe copies: `rdo-row-press.json`, paced/unpaced wants files, `setup-start.wants`, `sign-up.wants`, and `open-2551q-windows.wants`.
