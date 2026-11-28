# Playbook: re-instating a conductor from nothing

Use this when a conductor starts with no memory: a new session, a new engine, or after every session was closed.
Everything you need is in files and on GitHub. Do the steps in order.

The role word is fixed: the conducting role is "conductor". In a register, a session may be NAMED differently (for example "orchestrator"); that session name is the holder or machine-register identity, not a different role.

## 1. Check the setup

```sh
python3 crew/bin/crew doctor
python3 crew/bin/crew state pull
```

`crew doctor` says which engine and tier this is, where the state folder is, which `crew.local.ini` it used (or where it looked), and what is missing. Two things are not settings. Whether builds may run on this computer is the word of the person who started you. Whether you may run merge commands is the maintainer's word (`crew/RULES.md`, "The maintainer": the person named in the repository's ownership), and GitHub must let you; if the person who started you is not the maintainer, you prepare and pass the merge commands on. Write both answers into your `STATE.md`.
`crew state pull` brings the live state from its remote, if `crew.local.ini` names one. Without a remote, the state folder on this computer is the state.

A read-only restart drill does not pull: `crew state pull` writes the state checkout. Read the state exactly as it is, and record both heads you can read (`python3 crew/bin/crew git -C <state> log` and `python3 crew/bin/crew git ls-remote`).

## 2. Read, in this order

`HELD.md`, `queue.json`, `LOG.md` and `TRACKS.md` are at the state folder's ROOT, not under `roles/conductor/`; `roles/conductor/STATE.md` is the only one of these under the role folder.

1. `crew/GOAL.md`: what done means.
2. `crew/RULES.md`: the rules you enforce.
3. `crew/roles/conductor.md`: your role.
4. `roles/conductor/STATE.md` in the state folder: what was open when the last conductor stopped.
5. `<state>/HELD.md`: what waits for the maintainer. Never re-send anything listed there.
6. `<state>/queue.json`: the pull requests waiting for the gate, and the batch that was running.
7. `<state>/LOG.md` and `<state>/TRACKS.md` when they exist.

## 3. Sweep the truth from GitHub

The files say what was true when they were written. GitHub says what is true now. Where they differ, GitHub wins; correct the file.

- Open pull requests on each repository in `crew/REPOS.md`, with each one's head sha and its checks (`gh pr list`, then `gh pr checks <n>`).
- The running batch, if `queue.json` names one: `just ci-batch-status <branch>`. A green batch not yet merged goes to `crew/playbooks/batch.md` step 5: read every job before any merge.
- Each role's newest result in `roles/<role>/outbox/`, and its `STATE.md`. A result newer than the role's state is read first.
- The heads of `main` (and `buwiz-next` on hexuria/nsh), read by full sha.

## A read-only sweep

A read-only drill uses one simple command at a time. Do not use a composite shell line, a pipeline, `crew state pull`, or `crew watch` (the watch writes its seen file). The commands the read-only conductor profile is meant to allow are:

```sh
python3 crew/bin/crew doctor
python3 crew/bin/crew status
python3 crew/bin/crew state path
python3 crew/bin/crew lint
python3 crew/bin/crew sync --check
gh pr list --repo <owner>/<repo> --state open --json number,headRefOid
gh pr checks <n> --repo <owner>/<repo> --json name,state,bucket,link
python3 crew/bin/crew api GET repos/<owner>/<repo>/branches/<branch>
python3 crew/bin/crew git ls-remote origin refs/heads/main
python3 crew/bin/crew git -C <state> log -1 --format=%cI%x20%H%x20%s
python3 crew/bin/crew git -C <state> status --short
```

Use the same read verbs for the other repositories and branches. `crew api GET` is the only GitHub API route in this sweep: it fixes `gh api --method GET`, so no other method, no field or input argument, and no graphql path reaches `gh`. Git reads go through `python3 crew/bin/crew git`, which adds `--no-textconv --no-ext-diff` to `log`, `show` and `diff` and refuses output and write-tree arguments; the verbs stay to `log`, `show`, `diff`, `status`, `ls-remote`, `rev-parse` and `merge-tree` in the worktree or the state folder.

## 4. Sweep the fleet, if there is one

- For a normal restart, read each machine the state says the crew holds: read its last message with the whole history, not "new only".
- A machine that is suspended or waiting has finished or asked something. Read it, record the result, release it.
- A machine nobody holds is free.
- In a read-only drill, do not call a machine bridge's `read_messages`: read_messages advances its last-seen mark, so the call itself changes what the next conductor sees as new. Do not claim, queue, release, send, or follow up either. Record the bridge's machine-listing rows and every machine whose messages could not be read without mutation as "not swept".

## 5. Write and continue

- Write the first `LOG.md` line: the time, "conductor restarted", and the engine.
- Correct the conductor's `STATE.md` with what the sweep found.
- Start `crew watch` in the background.
- Then take the next step for every track at once: give each role with nothing in flight its next task.

A read-only drill stops after its result file: it does not write `LOG.md` or `STATE.md`, start the watch, or hand out work.

## Never

- Never re-send an action listed in `HELD.md`. It waits for the maintainer.
- Never act on a sha you did not read from GitHub in this session.
- Never take over a machine the state does not say the crew holds.
