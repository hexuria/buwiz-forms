# Rules for every crew member

These rules hold for every role and every engine (Claude Code or the Devin CLI), local or on a cloud machine.
Read this file, your role file in `crew/roles/` and your role's `STATE.md` before any work. Write `STATE.md` before you stop.

## The maintainer

The maintainer is the person who started the conductor.

- Keep the maintainer out of the loop. Decide, act and verify yourself: no questions, no manual tests to hand over, no progress reports.
- The maintainer hears of only three things:
  1. A permission check refused an action. The conductor passes on the exact command and the refusal text.
  2. Something only a person may do: a store's Submit, a password or sign-in code, a secret or a signing key.
  3. Something the maintainer asked for.
- Workers write to their outbox and never to the maintainer. The conductor decides what, if anything, reaches them.
- Report a refusal so it can be acted on in one read: whose check refused (your own session's permission check, a cloud machine, GitHub, a store), the exact command or tool call, the refusal text, what it is not, and the one action that clears it. A machine that cannot push is reported with the git error text and the cause; the usual cause is that the session has no GitHub token in its environment.
- A refusal, once reported, waits for the maintainer. Do not retry it reworded and do not reach the same end by another route: the refusal is the maintainer's decision until they change it. Record it in `HELD.md` and go on with other work.

## Hard limits (whoever asks)

- Never press Submit in a store. Never type a password, PIN or sign-in code into a real page. A mock server's code, typed by a recipe, is fine.
- Never print, commit or copy a key, certificate, token or `.env` value. Never commit `assets/ios/embedded.mobileprovision`.
- Never send anything to the production relay or the real BIR. Every run of the app uses `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, never `BIR_SFTP_LIVE`, and a scratch database.
- Never press Queue, Send or Submit in a release build: a release build of `main` files for real.
- Use only the demo taxpayer and invented values. Real taxpayer data never enters a test, a proof or a file.
- No builds or app runs on the conductor's computer unless `crew.local.ini` says local builds are allowed. Python, git and reading code are fine anywhere. Builds load the computer and mix build folders; the machines exist for them.
- Never run `nsh update`. It installs the upstream release over our pinned fork, and every later check then tests another NSH than the pin.

## Pull requests

- One pull request per change, opened from the repository's work branch (`main`; `buwiz-next` on the NSH fork). Never open one that contains another open pull request's commits: squash merges make the second conflict or strand. A change that needs an unmerged one is built on it on a `try/` branch and opened after it lands.
- Open as a draft while you are still pushing (a draft runs no checks), and mark it ready when you want its checks.
- Crew members commit as `hexuria <295542929+hexuria@users.noreply.github.com>`. Check `git log -1 --format='%an <%ae> | %cn <%ce>'` before every push. No `Co-authored-by` lines; squash merges carry an explicit `--body`.
- A pull request goes to the reviewer the moment it exists (`crew/playbooks/review.md`).
- A pull request is ready when all three hold at its exact head: `ci-ok` is green, the full test suites passed on every system the change can affect (results read, with run ids), and the reviewer said MERGE. `ci-ok` alone is only the quick checks.
- A pull request that touches the app enters a batch only with its full suite green at its exact head.
- Only the conductor batches and merges (`crew/playbooks/batch.md`). While a batch runs, nobody pushes to its pull requests and nothing else merges into `main`.
- Results are read, never assumed. "Green", "running" or a summary line is not a result: open the run, read each job, and name the run id.
- A red that is a known flake is re-run once and recorded with its run id. Any other red belongs to the author of the pull request.
- When one test fails, re-run that test alone on the same commit (a few times if it looks like timing, and once on `main`), and report every outcome. A full run starts again only when the head changes.
- A lock file is written by cargo, on a machine with the sibling checkouts. Never edit one by hand: a hand-edited entry can pass while another manifest still pins the old version.
- A pin bump goes plug first, then the app (`crew/REPOS.md`).

## Proof of anything on a screen

1. A gpui-agent recipe drives the real app: `just e2e <target> [suite]`, recipes in `agent-recipes/`. The proof branch holds the recipe, `report.json`, `report.md`, and a README whose first line is `driver: gpui-agent-cli <version>`.
2. When gpui-agent cannot do a step, do not fall back to OS-level input (real mouse, CGEvent, `adb shell input`, uiautomator, osascript, simctl taps, xdotool, or a rig of your own). Mark the step `known_gap` with the issue number and send the evidence to the `driver` role, which patches the driver or its plug; then the proof runs again.
3. An OS-level tool only where the app's own tree ends: an OS file picker, a system permission sheet, a store's web page.

- A fault seen only under injected input at machine speed, and contradicted by real use, is dropped.
- UI work also carries screenshots of each changed state and one screen recording (`.mp4`, under two minutes), committed to the branch with a README. A recording is a camera, not a hand, so any recorder is fine. Never record the maintainer's own screen.
- Print proofs are rendered through the app's own print path (`staticize` in `crates/bir-app-api/src/static_html.rs`, then `print_sheet.rs`), never from the raw frozen HTML: the raw sheet is not what a filer prints.
- Every emulator or Simulator report names the app commit, the NSH commit of the `nsh` tool, the sibling NSH checkout's commit, and for Android the SHA-256 of the runtime the package used. All must be one NSH commit.
- A build reported as evidence uses one build folder per checkout and the pinned toolchain's own cargo, with compiler caches off (`RUSTC_WRAPPER=` empty). A shared build folder can link another branch's crates with no error.

## Machines

- Claim a free machine before you send it a task, or wait in line. Send follow-ups only to machines you hold, and only for the same piece of work. Never take over a machine someone else holds.
- A machine that is suspended or waiting has finished or asked something. Read its last message with the whole history (not "new only"), then release it.
- Orders are self-contained (`crew/playbooks/machine-order.md`). The first line of every machine report is its free disk in GB; under 20 GB it cleans its own checkouts before it builds.
- A machine keeps a state file on its own disk and pushes after every finished step. Work that exists only on a machine is not done.
- A machine whose account forces another commit address pushes nothing. The role that holds it carries the commits.
- Keep a machine working: it cleans its disk and keeps its state file. Only a session that no longer answers at all reaches the maintainer.

## Files, not chat

- Your tasks arrive as `roles/<role>/inbox/NNN-<name>.md`. Your results go to `roles/<role>/outbox/NNN-<name>.md`: shas read back from GitHub, counts, run ids, and what you could not do.
- `STATE.md` says what is open, on which branch and machine, and what comes next, so a fresh session can continue from the file alone.
- A decision made in a chat goes into a file in the same turn, or it is lost.

## Local workers (started by `crew run`)

- A worker runs unattended under an allow-list: git, python3, read-only shell tools, `gh pr view|list|checks|diff|create`, `gh run view|list`, `gh issue list|view|create`.
- Refused on purpose: `gh pr merge|close`, `gh run cancel`, cargo, `nsh`, tags and force pushes. A refusal is final: record it in your result and go on.
- Write each command plainly, with no environment variable in front (`git push origin <branch>`, not `VAR=1 git push ...`): a prefixed command does not match the allow-list.
- A worker in a sandbox (`crew run ... --sandbox`) is limited by the sandbox instead (`crew/playbooks/sandbox.md`).

## Engines

| Role | Default engine |
| --- | --- |
| `conductor`, `reviewer`, `accounts` | Claude Code |
| every other role | Devin CLI, model `swe-2-1-max` |

Any role may be run by either engine; when Claude is unavailable the Devin CLI conducts too.
A review done by the free worker model is labelled as such in its result. It does not clear a change to accounts, sign-in, filing or printing: those need a review by the reviewer role on a different engine session from the author's.
