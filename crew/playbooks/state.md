# Playbook: the state folder

The live state is a folder of files, `.crew/state` by default (`crew state path` prints it). It is not on `main`: it changes every few minutes, and `main` changes only by reviewed pull request. `crew state pull` and `crew state push` move it to and from a separate remote, if `crew.local.ini` names one.

```
roles/<role>/STATE.md      what the role has open and what comes next
roles/<role>/inbox/        task files, NNN-<name>.md
roles/<role>/outbox/       result files, NNN-<name>.md
queue.json                 pull requests waiting for the gate, and the running batch
LOG.md                     one line per decision, newest last
HELD.md                    what waits for the maintainer
KNOWN-FLAKES.md            each known flaky test, and what to do until it is fixed
```

`crew state init` makes the folder with empty files.

## Public safety

If the state remote is public, the state follows the same rules as the repository: no e-mail address other than `support@buwiz.com` or the crew's noreply commit identity, no local absolute paths, no session links or ids, no account, organisation or browser profile names, no machine aliases (write `<alias>`), no test or practice account names, nothing about secrets beyond "never print them", no detail of an unfixed security fault (name the pull request or plan row only), no other session's or person's name. `crew lint` checks these.

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

## LOG.md

One line per decision or event that changed something, newest last. Time, who, what, and the evidence.

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

The conductor edits it. `waiting` lists pull requests that may enter the next batch; `batch` is the running one.

```json
{
  "waiting": [512, 515, 518],
  "note": "pull requests waiting for the gate; the conductor edits this list",
  "batch": {"branch": "test/linux-windows/batch-507-509", "prs": [507, 509]}
}
```

## KNOWN-FLAKES.md

One entry per flaky test: where it was seen, the cause if known, and what to do until the fix lands. A batch whose only red matches an entry is re-run once, and the run is added to the entry.

```markdown
## `screens/auth/setup.test.tsx` › "<test name>": "<error text>" (Windows, shard 1)
Seen: 2026-12-01, batch-<n>-<m>, run <id>, job <id>. None of the batch's pull requests touch this file.
Cause: the test presses the next box while the previous sheet is still closing. The app is right.
Until fixed (#<k>): re-run the failed job once.
Re-runs: run <id> green.
```
