driver: gpui-agent-cli 0.4.1
recipe: agent-recipes/e2e/rdo-row-press.wants (suite rdo-row-press)

Windows proof of the #425 RDO row-press case, driven only through gpui-agent. No real mouse or keys, UI Automation, SendInput, or other machine-native input was used. The screen videos are recordings only.

App commits:

- BEFORE `c42626973461a21a2e8ab58caca2405ff0891252`
- AFTER `fef911cf8baf979e5cb399cf0bf93a4593e214e5`

Runner/recipe commit: `f9e543cee9366bfcf06ec9808f0d432be8283385` (`feat/rdo-row-press`). NSH sibling/tool pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Every app launch used a fresh scratch database, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, and no `BIR_SFTP_LIVE` or Google OAuth environment variables.

gpui-agent-cli version note: `cargo install --list` reports `gpui-agent-cli v0.4.1`; the 0.4.1 binary itself rejects `gpui-agent --version` (`unexpected argument '--version' found`). The runner detected CLI 0.4.1 and used the paced primary recipe in all four runs; the unpaced fallback was not used.

## Results

| folder | app commit | setup-start | rdo-row-press | first failure |
|---|---|---|---|---|
| `before-1/` | `c4262697` | PASS | FAIL | round 1, `s6`, `assert`, `new-tp-rdo_code-search` |
| `before-2/` | `c4262697` | PASS | FAIL | round 1, `s6`, `assert`, `new-tp-rdo_code-search` |
| `after-1/` | `fef911cf` | PASS | FAIL | round 2, `s10`, `wait_until`, `new-tp-rdo_code-opt-048` |
| `after-2/` | `fef911cf` | PASS | FAIL | round 2, `s10`, `wait_until`, `new-tp-rdo_code-opt-048` |

BEFORE failure in both runs:

- line: `assert new-tp-rdo_code-search value="047 — East Makati"`
- gpui-agent error: `node new-tp-rdo_code-search value: expected "047 — East Makati", got Some("")`
- box held: empty (`""`)
- evidence: `02-rdo-row-press/failure.png` and `02-rdo-row-press/failure-tree.json` inside each before run folder

AFTER failure in both runs:

- line: `wait_until new-tp-rdo_code-opt-048 --timeout-ms 5000`
- gpui-agent error: `wait_until timed out: node new-tp-rdo_code-opt-048 not found`
- box held: `047 — East Makatimakati` (the first pick succeeded; the recipe's second paced `type makati` appended to the existing pick, so row `048` never appeared)
- evidence: `02-rdo-row-press/failure.png` and `02-rdo-row-press/failure-tree.json` inside each after run folder

## Files

- `before-1/`, `before-2/`, `after-1/`, `after-2/`: `report.json`, `report.md`, `run.log`, receipts, screenshots, failure trees, runner output, and app console output for each run.
- `before.mp4`: screen recording of both BEFORE runs.
- `after.mp4`: screen recording of both AFTER runs.
- `rdo-row-press.json`, `rdo-row-press.wants`, `rdo-row-press-unpaced.wants`, `setup-start.wants`: recipe files copied beside the evidence.
