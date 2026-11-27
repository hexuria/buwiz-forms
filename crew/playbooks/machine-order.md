# Playbook: an order to a cloud machine

A cloud machine builds, runs suites, drives the app and packages. It may have forgotten everything since its last job, so every order is self-contained: it holds the rules that matter, the commit, the folder and the report you want.
This is the `fleet` tier only.

## Before you send

1. **Claim the machine.** Ask the fleet's register for a free machine of the kind the task needs (`linux`, `macos`, `windows`), claim it under your role, then send. No free machine: wait in line. Never send to a machine someone else holds, and never interrupt a busy one with a new job.
2. **Follow-ups** go only to a machine you hold, and only for the same piece of work: an answer, a correction, the next step of the same job.
3. Ask for a machine by what the task needs, not by its `<alias>`.

## What an order holds, in this order

1. **The disk line.** "First line of your reply: free disk in GB. Under 20 GB, delete build folders of merged or abandoned commits and pushed recordings before you build. Never delete the installed tools or their own build folders."
2. **The state file.** "Read `~/STATE.md` on this machine first, and update it at the end of every step: the rules below, this job, its commit, what you pushed and where, what is next."
3. **The rules that matter for this job,** copied in: the four safety variables (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, never `BIR_SFTP_LIVE`), mocks only, never `nsh update`, gpui-agent only for UI proof, one build folder per checkout with the pinned toolchain's own cargo, compiler caches off (`RUSTC_WRAPPER=` empty).
4. **The commit identity.** "In the checkout, set `user.name` and `user.email` to <the identity the conductor's settings name> (`[crew] commit_identity`, a GitHub noreply address). Before any push, check `git log -n<k> --format='%an <%ae> | %cn <%ce>'` with the same git binary that will push. A commit with any other address is re-committed before the push." If the machine's account forces another address, it pushes nothing and returns its work as text or a patch.
5. **The job:** the repository, the branch, the exact commit (full sha read from GitHub), and the numbered steps. Commands written out in full, as `.github/workflows` runs them when the job is a CI run.
6. **The budget and the halfway line.** "This job should take about <N> minutes. At the halfway mark, push what you have and send one line: what is done, what is left." A job that runs past its budget stops and reports.
7. **Push after every finished step,** to the named branch. Work that exists only on the machine is not done.
8. **The final report shape** (below).

## The final report

```
<free GB>
commit: <full sha built>     sibling nsh: <sha>     nsh tool: <sha>     cargo: <cargo --version>
build folder: <path, one per checkout>
| job | commit | PASS/FAIL | counts | log |
pushed: <branch> <sha>    files: <each proof file by path>
not done: <what, and why>
```

UI work adds the e2e report folder, the pictures and the recording, all on the pushed branch with a README (`crew/playbooks/ui-proof.md`). Emulator work adds the runtime's SHA-256.

## After the report

1. Read the machine's last message with the whole history, not "new only". A machine that is suspended or waiting has finished or asked something.
2. Copy what counts into the role's result file: shas, run ids, counts, file paths.
3. Release the machine in the register.
4. A failed test is re-run alone on the same commit before it counts (a few times if it looks like timing, and once on `main`).

A session that has run long is kept, not replaced: its state file and your self-contained orders carry the work. Only a machine that no longer answers at all is reported to the maintainer.
