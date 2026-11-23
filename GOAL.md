Ship eBIRForms from the NSH app (`nsh/`) and then delete `crates/bir-shell`, in the repository hexuria/buwiz-forms, exactly as decided in `spec/PLAN.md` on this branch (`handoff/nsh-release`). Work unattended until PR 1 and PR 2 are open with green CI, then stop and report on the tracking issue hexuria/buwiz-forms#269.

## Read first, fully, in this order
1. `spec/PLAN.md` on this branch: the owner's picks (final, do not re-ask), "Decisions made to fill gaps" (final), the tracks of PR 1, the change list of PR 2 and what "done" means. `spec/picks.txt` is the owner's own text.
2. On `main`: `docs/platforms/devin-environment.md` (your Linux, macOS and Windows machines and how each builds and runs the NSH app), then `nsh/AGENTS.md`, then `docs/platforms/nsh.md`. Parts of the last two are stale (they still call the app menu, the OS sign-in prompt and Linux print missing): the code wins.
3. In the NSH checkout beside the repository (`../nsh`, at `NSH_PINNED_REF` from `.github/workflows/nsh.yml`): `website/docs/packaging.md`, `app-updates.md`, `signed-packages.md`, `macos.md`, `windows.md`, `linux.md`. NSH is read-only: never edit it and never open a pull request against it.
4. Pull requests #198 and #203 (merged into the parked branch `rox-mini-app-round2`, never into `main`): the two speed fixes the plan's speed check is about.

## Where
- PR 1: branch `feat/nsh-release`, from current `main`, against `main`.
- PR 2: branch `feat/nsh-release-delete`, from PR 1's branch. Its description starts with "Stacked on #N. Merge #N first; this PR then retargets to main." If PR 1 has merged when you open or update PR 2, run `gh pr edit <PR 2> --base main` and rebase it onto `main` first. Never let a pull request merge into a branch that was already merged: that is how #234 and #236 were lost.
- Both pull requests carry the label `feature:nsh-release`, and each description starts with "Feature: nsh-release — tracking hexuria/buwiz-forms#269".
- Use these branch names, not generated ones.

## Safety (always)
- Set `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1` for every run and test. Never set `BIR_SFTP_LIVE`. Nothing is ever sent to the BIR.
- Run the app only through the `nsh-*` recipes with a scratch database. Never run `just run`, `just run-upstream` or any `bir-shell` binary: those are not forced dry.
- No test run may become the repository's latest release. The installed v0.1.0 app and the new updater both read `releases/latest`. Prove builds with `workflow_dispatch` runs that keep their files as workflow artifacts (add that input to the release workflows if it is missing). Use a draft pre-release only where the publish step itself is under test, never marked latest, and delete it and its tag afterwards.
- Never print, commit or upload a signing key, certificate or secret. The update key's public half is in the plan; its secret half is not yours to have.

## How
PR 1 has tracks A to G in the plan. Do A first: the other tracks read its names. Then, per track:
1. Write the change list (files, functions, tests) from the plan and check it against the code before editing.
2. Tests first where a test can exist (the app tracks, the `bir` crate, the Python workflow-policy tests), failing for the right reason.
3. Implement until green. Gate: the commands `docs/platforms/devin-environment.md` gives for `just nsh-check -v`, `just nsh-build`, `just nsh-test`, `just core-plain-test`, plus the Rust suites and Python script tests you touched. No unexplained warning.
4. Prove it for real. A workflow that was written and never run is not done.
   - Linux and Windows: package on your own Linux and Windows machines, then install each file from pick 4, open the app, open a form, and uninstall. Then run the release workflows and read their logs and artifact lists.
   - macOS: package and open the ad hoc signed app on your macOS machine. Developer ID signing and notarization can only be proven by the release workflow, whose secrets hold the certificate: run it and check the notarization step and the `spctl` check in its log.
   - Self-update: with a throwaway key and a throwaway feed that are never committed, update one test build to the next on at least one system, and say how. The real feed is only proven once the owner adds `NSH_UPDATE_KEY`; until then the workflow must skip `latest.json` with a warning and still publish the downloads.
   - Mac App Store: build the app with the `mas_build` feature and the sandbox entitlements, ad hoc signed, and run it sandboxed on your macOS machine: start, data folder inside the container, open a form, print. The signed store package needs the owner's certificates and provisioning profile: port the recipes, and write the exact commands the owner runs on their Mac.
5. Review your own diff against the plan before pushing.
6. Commit with clear messages, push, open the pull request with `gh pr create --repo hexuria/buwiz-forms`.

Then PR 2 from the plan's change list. Done when no tracked file names `crates/bir-shell` or `app/shell`, CI is green on all three systems, and the release workflows still pass from that branch.

## Allowed / not allowed
Allowed without asking: push the two branches, open the two pull requests, run and re-run workflows (including `workflow_dispatch` on the release workflows), create draft pre-releases and test tags under the rule above and delete them, comment on #269.
Not allowed: merging; a real release tag; anything that becomes the latest release; adding or reading repository secrets; uploading to TestFlight or App Store Connect; editing NSH; deleting `rox-mini-app-round2` or any branch that is not yours; force-pushing anyone else's branch; renaming `agent::APP = "bir-shell"`.

If a pick cannot be built as described, build the closest version that keeps its intent, explain why in the pull request, and keep going. Do not stop to ask.

## When done
Post one report as a comment on #269:
- PR 1 and PR 2 with links and CI state;
- every download in pick 4, with the workflow run that built it and the machine you installed it on;
- what was proven on your machines, what only the workflows proved, and what still needs the owner's Mac (signed store package, TestFlight);
- the speed-check numbers and what was ported;
- every test tag, draft release and workflow artifact you created, and that the tags and drafts are deleted;
- anything built differently from a pick or a decision, and why;
- what the owner does next, as commands: add `NSH_UPDATE_KEY`, merge order, build the store package.
