# Playbook: the state folder

The live state is a folder of files, `.crew/state` by default (`crew state path` prints it). It is not on `main`: it changes every few minutes, and `main` changes only by reviewed pull request. `crew state pull` and `crew state push` move it to and from a separate remote, if `crew.local.ini` names one. That remote must be a different repository from the code, and its branch is `crew-state` (or a name starting with `crew-state` or `crew/`): the state never goes into the code repository, and never onto a default branch.

```
roles/<role>/STATE.md      what the role has open and what comes next
roles/<role>/inbox/        task files, NNN-<name>.md
roles/<role>/outbox/       result files, NNN-<name>.md
queue.json                 pull requests waiting for the gate, and the running batch
LOG.md                     one line per decision, newest last
HELD.md                    what waits for the maintainer
TRACKS.md                  long-lived track state, when the crew keeps it
KNOWN-FLAKES.md            each known flaky test, and what to do until it is fixed
```

`crew state init` makes the folder with empty files.

## Public safety

If the state remote is public, the state follows the same rules as the repository: no e-mail address other than `support@buwiz.com` or the crew's noreply commit identity, no local absolute paths, no session links or ids, no account, organisation or browser profile names, no machine aliases (write `<alias>`), no test or practice account names, nothing about secrets beyond "never print them", no detail of an unfixed security fault (name the pull request or plan row only), no other session's or person's name. `crew lint` checks these.

Before `crew state push` runs the same check, it reads the URL the push will really use with `git -C <state> remote get-url --push origin`. Only when that push URL matches the resolved `state_remote` does it ask GitHub's API for the repository's visibility. A matching remote reported as private skips the check and prints `state remote is private: public-safety check skipped`. Any mismatch, a public remote, a non-GitHub remote, or a remote whose visibility cannot be read keeps the check.

## STATE.md

Written so a fresh session can continue from the file alone. Shas are full and read from GitHub. Times are UTC.

```markdown
# forms: STATE (2026-12-01 09:40 UTC)

Main: hexuria/buwiz-forms main <full sha>.

| PR | Branch | Head | State | Next |
| --- | --- | --- | --- | --- |
| #<n> 1606 installment gate | fix/1606-installment-gate | <sha> | reviewed MERGE | suites at the head |
| #<m> band floor | fix/band-floor | <sha> | to review | machine gates at the head |

Machines held: one linux (<alias>): gates for #<m> at <sha>.
Next: when #<n> lands, start the next form on the task list.
```

## Machine tickets

A machine ticket copied into a state file is one pipe-separated row, not a prose sentence. Use the machine bridge or fleet register's fields in this order, with no line break inside a field:

```
<alias> | <kind> | <bridge state> | <busy-or-free> | <holder> | <ticket-or-session-id> | <task in one line>
```

Example shape only:

```
<alias> | linux | waiting_for_user | busy | conductor | <id> | gates for hexuria/buwiz-forms#<n> at <full sha>
```

The row in `STATE.md` says what the crew believes it holds. The bridge or register is swept for what is true now; a difference is corrected in the state, never explained only in prose.

## LOG.md

`LOG.md` is kept. It is one line per decision or event that changed something, newest last: time, who, what, and the evidence. `crew state push` appends one line when `roles/conductor/STATE.md` changed in that push:

```
2026-12-01 09:12 UTC conductor: STATE.md changed (crew state push)
```

Other decisions keep the same shape:

```
2026-12-01 09:12 UTC conductor: batch test/linux-windows/batch-<n>-<m> green (runs <id>, <id>); merged #<n>, #<m>; verify equal; main <sha>
2026-12-01 09:20 UTC conductor: #<k> held: merge refused by the permission check; in HELD.md
```

## HELD.md

What waits for the maintainer, one entry each: what, why only the maintainer can do it, the exact command or step, and the refusal text when there is one. Nothing listed here is re-sent or tried another way. The maintainer's answer removes the entry.

```markdown
## Merge hexuria/<repo>#<n> at <full sha>
Why: the permission check refused the merge to the conductor.
Command: gh pr merge <n> --repo hexuria/<repo> --squash --match-head-commit <full sha> --body "<title>"
Refusal: "<the refusal text, word for word>"
Since: 2026-12-01 09:20 UTC
```

## queue.json

The conductor edits it. `queue` lists pull requests that may enter the next batch; `batch` is the running one. An empty queue may be exactly `{"queue": [], "batch": null}` and may carry a `note`. The launcher also reads the older key `waiting` so an existing state does not break.

```json
{
  "queue": [512, 515, 518],
  "note": "pull requests waiting for the gate; the conductor edits this list",
  "batch": {"branch": "test/linux-windows/batch-507-509", "prs": [507, 509]}
}
```

## Reviewer verdict files

Every pull-request review writes a machine-readable verdict file beside the reviewer's prose result: `roles/reviewer/outbox/<owner>-<repo>-<n>.verdict` (for example `roles/reviewer/outbox/hexuria-buwiz-forms-512.verdict`). It is five key-value lines:

```
verdict: MERGE
head: <full sha read>
engine: claude
model: <the model that read it>
note: <one line: what was read, what is left>
```

`verdict` is `MERGE`, `FIX FIRST` or `NOT YET`; `head` is the pull-request head the review read, not `main` and not a branch name. Write the file for every verdict. It is a note for people and dashboards, not a gate: GitHub's permissions and branch protection decide who can merge. `crew status` prints `needs a review` for every open pull request with no verdict file or with a verdict `head` different from the pull request's current head.

## KNOWN-FLAKES.md

The launcher does not create this file: the conductor creates it the first time it records a flake. One entry per flaky test, three lines under its heading: where it was seen, the cause if known, and what to do until the fix lands. A batch whose only red matches an entry is re-run once, and the run is added to the entry as a fourth line.

```markdown
## `screens/auth/setup.test.tsx` › "<test name>": "<error text>" (Windows, shard 1)
Seen: 2026-12-01, batch-<n>-<m>, run <id>, job <id>. None of the batch's pull requests touch this file.
Cause: the test presses the next box while the previous sheet is still closing. The app is right.
Until fixed (#<k>): re-run the failed job once.
Re-runs: run <id> green.
```
