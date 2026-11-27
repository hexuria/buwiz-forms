# e2e smoke dry-run on Windows — PR #428 (ad7e456a)

A dry run of the new gpui-agent e2e smoke suite on Windows, run exactly as
`scripts/e2e/machines/windows.ps1` does it, at hexuria/buwiz-forms **ad7e456a4dc1c1db96a6cef77e196749c6b7e1a0**
(PR #428 head, detached worktree), sibling hexuria/nsh at the pinned `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.

- Machine: Windows Server 2022 Standard build 20348, EnableLUA=0, 1600×1200 @ 100% scale.
- gpui-agent-cli **0.4.0** (the version Cargo.lock pins at this commit), cargo 1.99.0 MSVC, nsh 0.6.1 (8c78cfd).
- Safety env throughout: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1,
  never BIR_SFTP_LIVE; scratch database only; random GPUI_AGENT_TOKEN generated for the run's
  shell only, never printed or written.
- CARGO_TARGET_DIR was an empty isolated folder, deleted after the run.
- The app was driven ONLY through the runner (gpui-agent); no improvised clicks or keystrokes.

## What each file shows

- `report.md` / `report.json` — the runner's own report: per recipe, result, time, failing step
  with its `.wants` line, error, and links into the failure evidence.
- `run.log` — one line per action (time, verb, target, result, ms), including the launch, the
  plane waits, the per-recipe results, and the cleanup lines.
- `app.log` — the app's own output across the run, with the build lines (crate 18m10s, dev host
  7m22s) and a trailing PowerShell wrapper error fragment (`nsh-run-agent.ps1:1533`) that
  appeared when the runner stopped the app, after all recipes were done.
- `NN-<recipe>/receipt.json` — gpui-agent's receipt per recipe.
- `NN-<recipe>/*.png` — step screenshots the suite marks, plus `failure.png` at the failing step
  (the Windows host DOES carry screenshots here: `screenshot_inline` in the feature list, despite
  the machine script's comment saying screenshot_unavailable).
- `NN-<recipe>/failure-tree.json` — the tree at the failure.

## What it does not show

- No video recording: gpui-agent records none by design, and the dry-run order did not ask for
  the machine's own recorder.
- Recipes 6–9 never ran (open-2551q is required), so nothing is proven about money-boxes,
  delete-row, queue or sign-out in this run.
- The money-boxes paced variant would not have run anyway: gpui-agent-cli here is 0.4.0, so the
  unpaced fallback was selected by the suite (see the `skip` lines in the check output).

## Result summary

`just e2e check smoke`: 9 recipes, 14 files validated, 0 problems.

| # | recipe | result | failing step |
|---|---|---|---|
| 1 | setup-start | pass | |
| 2 | sign-up | pass | |
| 3 | navigate | fail | s12: `wait_until tp-profile-years` not found |
| 4 | birth-date | fail | s2: `wait_until panel-open-profile` not found |
| 5 | open-2551q | fail | s2: `wait_until new-return-2551Q` not found |
| 6–9 | money-boxes, delete-row, queue, sign-out | not run | open-2551q failed and the suite needs it |

No `known gap` result appears anywhere in this run — the suite's known gaps are phone/macOS
labels and this is a Windows desktop run; these three failures are unlabelled fails with their
exact error, screenshot and tree recorded.

Runner-level notes: after the app was stopped, the scratch database files
(`db`, `-wal`, `-shm`) could not be removed (WinError 32, still held by a process) — a Windows
cleanup gap in the runner, not a test failure.
