# Playbook: re-instating a conductor from nothing

Use this when a conductor starts with no memory: a new session, a new engine, or after every session was closed.
Everything you need is in files and on GitHub. Do the steps in order.

## 1. Check the setup

```sh
python3 crew/bin/crew doctor
python3 crew/bin/crew state pull
```

`crew doctor` says which engine and tier this is, where the state folder is, and what is missing.
`crew state pull` brings the live state from its remote, if `crew.local.ini` names one. Without a remote, the state folder on this computer is the state.

## 2. Read, in this order

1. `crew/GOAL.md`: what done means.
2. `crew/RULES.md`: the rules you enforce.
3. `crew/roles/conductor.md`: your role.
4. The conductor's `STATE.md` in the state folder: what was open when the last conductor stopped.
5. `HELD.md`: what waits for the maintainer. Never re-send anything listed there.
6. `queue.json`: the pull requests waiting for the gate, and the batch that was running.

## 3. Sweep the truth from GitHub

The files say what was true when they were written. GitHub says what is true now. Where they differ, GitHub wins; correct the file.

- Open pull requests on each repository in `crew/REPOS.md`, with each one's head sha and its checks (`gh pr list`, then `gh pr checks <n>`).
- The running batch, if `queue.json` names one: `just ci-batch-status <branch>`. A green batch not yet merged goes to `crew/playbooks/batch.md` step 5: read every job before any merge.
- Each role's newest result in `roles/<role>/outbox/`, and its `STATE.md`. A result newer than the role's state is read first.
- The heads of `main` (and `buwiz-next` on hexuria/nsh), read by full sha.

## 4. Sweep the fleet, if there is one

- For each machine the state says the crew holds: read its last message with the whole history, not "new only".
- A machine that is suspended or waiting has finished or asked something. Read it, record the result, release it.
- A machine nobody holds is free.

## 5. Write and continue

- Write the first `LOG.md` line: the time, "conductor restarted", and the engine.
- Correct the conductor's `STATE.md` with what the sweep found.
- Start `crew watch` in the background.
- Then take the next step for every track at once: give each role with nothing in flight its next task.

## Never

- Never re-send an action listed in `HELD.md`. It waits for the maintainer.
- Never act on a sha you did not read from GitHub in this session.
- Never take over a machine the state does not say the crew holds.
