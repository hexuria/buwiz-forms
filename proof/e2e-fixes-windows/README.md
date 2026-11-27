# e2e smoke suite on Windows — evidence of the final run, try/e2e-fixes-windows

The final `scripts/e2e/machines/windows.ps1` run on Windows Server 2022 (1600x1200, 100% scale), branch
`try/e2e-fixes-windows` rebased onto main `6bd529d05879a92ebc1ec618095b13a87c7f67a5`, driven only through
gpui-agent-cli **0.4.1** (hexuria/gpui-agent `5867457a364b5c27d208fb729e374983c50cf8fb`) — earlier iterations on the
pre-merge head `ad7e456a` used the Cargo.lock-pinned 0.4.0, which skipped the paced money-box variants.
Safety environment for every run: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`,
a scratch database, a random GPUI_AGENT_TOKEN per run (never stored), never BIR_SFTP_LIVE.

## What each file shows

- `report.md` / `report.json` — the suite's own summary: per recipe pass/fail/known gap, time, failing step and error.
- `run.log` — the runner's log lines (resolve results, per-recipe outcomes, scratch database cleanup).
- `app.log` — the app build and the dev host's console (ends in the known cosmetic `$LASTEXITCODE` fragment of the
  nsh-run-agent PowerShell wrapper, after the run itself finished).
- `<nn>-<recipe>/receipt.json` — gpui-agent's per-step receipt: delivery status and elapsed time of every step.
- `<nn>-<recipe>/NNN-sN.png` — step screenshots the recipes ask for (`--screenshot`).
- `<nn>-<recipe>/failure.png` + `failure-tree.json` — the screen and the full node tree at the failing step. These are
  the evidence for the report's (b)/(c) classification: e.g. `07-queue/001-s1.png` is the to-do list overlay naming
  the sections validation still wants fixed, `004-s4`-era trees show the inline income-type box flipping to
  "Saved to profile".

## What they do not show

- No real keystrokes or mouse input: every step went through gpui-agent's recipe runner, per the suite rules.
- No sign-out proof: the sign-out recipe is a documented known gap (logout needs an admin PIN the suite rules forbid
  typing), not a pass.
- delete-row never ran its steps in the final run: its `$table` resolve did not find `fp-table-*-add` in the tree
  after money-boxes (the schedule part's expansion leaves the Add-row button outside the rendered window on this
  build); the drawer's contents are unreachable regardless — see the report's (b) item.

## Left out of the integration PR

`fix/e2e-recipes-all-platforms` carries this folder without the per-step screenshots (`NNN-sN.png`, 4.9 MB) except
`07-queue/001-s1.png`: they stay on `try/e2e-fixes-windows` at `c8ce9008`. `run.log` and `app.log` were not pushed
with the run.
