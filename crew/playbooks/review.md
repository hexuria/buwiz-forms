# Playbook: review

Every pull request is read by the reviewer role, independently of its author, at its exact head.

## When

Send a pull request for review the moment it exists, as a draft too. Do not wait for its checks or its device proof: the review runs alongside them. A read-ahead branch (a `try/` branch before it is a pull request) may be sent the same way.

## What a review request holds

The conductor writes it as a task file in `roles/reviewer/inbox/NNN-<repo>-<n>.md`:

- the repository, the pull request number, its base branch, and its head sha read from GitHub just now;
- for a delta review, the head of the last read and the new head;
- what the change claims to do, in two lines;
- what is already known: earlier findings, open questions, results already read;
- what to check hardest (for example: the state machine, the lock file, the print path, the safety variables);
- the design page or contract the change follows, if any.

## The verdict words

| Word | Meaning | What follows |
| --- | --- | --- |
| `MERGE` | Nothing blocks. | It may enter a batch once its checks pass. |
| `MERGE AFTER <items>` | Small named fixes. | It enters a batch once each named item has landed; no full re-read. |
| `FIX FIRST` | A fault that must be fixed, then read again. | The author fixes; a delta review follows. |
| `NOT YET` | The approach is wrong or the change is incomplete. | Back to the author's role; the conductor may rule on the approach. |

The verdict is the first line of the result. Each finding follows with its file and line, a scenario that shows the fault, a fix, and a label (high, medium, low).

## Delta reviews

When the author pushes fixes, the next request names the old head and the new one. The reviewer reads `git diff <old>..<new>`, says for each earlier finding whether it is fixed, still open or made worse, and lists new findings. A delta that touches more than the findings gets a full read of what it touched.

## An independent read

- The reviewer is never the author's engine session. A pull request written in one session is read in another. When both engines are available, the reviewer runs on the engine the author did not use.
- The reviewer reads the code with git, never from the description or the author's report alone.
- A review by the free worker model is labelled as such in its result. It does not clear a change to accounts, sign-in, filing or printing.
- The reviewer runs nothing that builds. When a claim needs a run, the reviewer asks for one through the conductor.

## After the verdict

The conductor passes each finding to the owning role as a task, records the verdict and the head it covers in its `STATE.md`, and rules on any open design question from the maintainer's recorded decisions. A finding that names an unfixed security fault stays in the result file and the plan row; the public pull request names only that a fix is pending.
