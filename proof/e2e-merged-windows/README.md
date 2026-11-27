# proof/e2e-merged-windows — merged gpui-agent smoke suite on Windows

Final run of the merged e2e smoke suite (PR hexuria/buwiz-forms#452, head 7d3b77fe6aeea20ebd5b6fecd5d956058ec80e77,
plus the recipe fixes of this branch) on Windows Server 2022 (build 20348, EnableLUA=0, 1600x1200 @ 100%).
The app was driven only through gpui-agent recipes; gpui-agent-cli 0.4.1 from crates.io
(`cargo install gpui-agent-cli --version 0.4.1 --force`) ran the suite via `just e2e windows smoke --allow-send`.
Safety env for every run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, scratch database,
mock relay. No PIN or password box was ever typed into; Queue was pressed only inside the suite against the local mock.

## Result of the final run (windows-20261127-105813)

8 of 10 recipes pass, including the full SEND path of queue: validation passes, the confirm dialog appears,
Continue is pressed, and the dry-run worker sends the return — the form ends in the "Dry run · read-only" state
(07-queue screenshots). delete-row never starts (`setup`): its pre-recipe resolve finds no fp-table-*-add because
the schedule part is collapsed and out of view after the send (and the return is read-only then); on Windows the
recipe is a known gap anyway (the sheet gap). sign-out is the documented known gap (Log out needs an admin PIN
the suite may not type).

## Files
- `report.md` / `report.json` — the suite's own summary and machine-readable results.
- `run.log`, `app.log` — runner console output and the app log.
- `<nn>-<recipe>/` — per-recipe step screenshots and receipt.json; failure.png + failure-tree.json where a step failed
  (10-sign-out's known gap, 08-delete-row's resolve).
- This README.

## What the files do not show
- No video (the suite records none; screenshots only, all well under 5 MB). Nothing outside the scratch database
  was touched; no app source (nsh/, crates/) was changed — only agent-recipes/e2e/ files.
