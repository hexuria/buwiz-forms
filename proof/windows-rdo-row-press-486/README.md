driver: gpui-agent-cli 0.4.1

# Proof: PR #486 rdo-row-press suite on Windows

Machine: Windows Server 2022 (DEVINBOX), app driven only through gpui-agent by
`scripts/e2e/machines/windows.ps1` (smoke, then rdo-row-press), no real mouse or keys.
Safety env for every run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1
BIR_SFTP_DRY_RUN=1, never BIR_SFTP_LIVE, fresh scratch database per launch (removed
after each run, see run.log `clean` lines). Nothing was sent anywhere; Queue ran only
inside smoke's queue recipe in this dry-run test build via the machine script.

## Heads

- PR #486 final head tested: `2281b5eab88a65737f3ba2d313a0027db93fd7f0`.
- `git diff 6417adbc 2281b5ea --stat` = exactly one file,
  `agent-recipes/e2e/profile-pick-press.wants` (+5 lines): no app code differs.
- `at-6417adbc-run1/` — first machine run: started at 6417adbc before the new head
  existed; its evidence folders therefore record `app_commit` 6417adbc. The app binary
  is byte-identical code to 2281b5ea (recipes-only diff). profile-pick-press in this
  run executed the 6417adbc recipe (194 steps).
- `at-2281b5ea-run2/` — second machine run, fully at 2281b5ea including the toast-wait
  recipe version (profile-pick-press 196 steps).
- `recipes/` — copies of the two 2281b5ea recipe files the suite ran.

## Results (from each report.json, not exit codes)

Run 1 (at-6417adbc): smoke 10 pass + 1 known gap; rdo-row-press 4/4 pass.
Run 2 (at-2281b5ea): smoke 10 pass + 1 known gap; rdo-row-press 4/4 pass.
The one known gap in both smoke runs: sign-out step s4,
`wait_until timed out: node 'locked-screen' not found` — a gpui-agent/driver-level
known gap, not a product regression; it is why the script exit code is non-zero.

rdo-row-press (81 steps) and profile-pick-press (194/196 steps) passed in full in both
runs, including the two error-line rounds:
- RDO error-line round: after the not-taken text round, the branch section Save press
  lands first click and the reopened RDO box asserts `047 — East Makati`.
- ZIP error-line round: the Save press is itself the blur; reopened ZIP asserts
  `1200 — Makati CPO + Buendia Ave`.

## Layout

- `<head>-run<n>/<suite>/report.json` / `report.md` — machine-readable and human summaries.
- `<head>-run<n>/<suite>/run.log` — one line per action (time, verb, target, result, ms).
- `<head>-run<n>/<suite>/<nn>-<recipe>/receipt.json` — gpui-agent's per-step receipt
  (every step `ok: true` in passing recipes).
- `<head>-run<n>/<suite>/<nn>-<recipe>/*.png` — inline screenshots the recipes asked for.
- `<head>-run<n>/<suite>/app.log` — the app's own output.

## What this does not show

- No video: gpui-agent has no window screenshots/recordings on Windows by design; the
  inline PNGs in the receipts are the app-tree screenshots the recipes requested.
- sign-out's `locked-screen` gap means the lock screen node was not observed within the
  timeout; it does not prove or disprove the lock screen renders, only that the driver
  could not find the node in time.
