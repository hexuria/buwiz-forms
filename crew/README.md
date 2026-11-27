# The crew

The crew is a set of AI agents that ships eBIRForms from this repository.
One conductor orders the work, gates it, batches it and merges it. One reviewer reads every pull request.
Workers do the tasks of one track each (forms, accounts, the stores, the driver, the runtime, the tools).
Every role is a file in `crew/roles/`, every rule is in `crew/RULES.md`, and the live state is a folder of files.
So one session can run everything, and closing every session loses nothing: a new conductor starts again from the files.

## Start it

| Engine | How |
| --- | --- |
| Claude Code | Open it in the repository root and type `/crew`. |
| Devin CLI | Run `devin` in the repository root, then type `/crew`. |
| Either, from a shell | `just crew start` (add `--engine claude` or `--engine devin` to pick one). |

The `/crew` skill runs `crew doctor` first, then follows `crew/playbooks/restart.md` as the conductor.
Both engines read the root `AGENTS.md` (Claude Code through `CLAUDE.md`) and the skill in `.claude/skills/crew/`.

## Three tiers

Pick the tier that matches what you have: `crew doctor` reports which tier this checkout can run. Your own settings go in `crew.local.ini` (git-ignored; `crew/local.example.ini` lists the keys). Two environment variables override it for one shell: `CREW_STATE_DIR` (the state folder, over `[crew] state_dir`) and `CREW_ENGINE` (over `[crew] engine`). Whether builds may run on the conductor's computer is not a setting: tell the conductor when you start it.

| Tier | What you have | What it can prove | What it cannot prove |
| --- | --- | --- | --- |
| `solo` | One agent session. No workers. | Code and script tests; the quick checks (`ci-ok`) and the Linux and Windows test suites on GitHub (`just ci-test`, a Unix-only recipe). It opens pull requests and stops: it never merges into the default branch without a human review. | Anything on a screen of a platform you do not have; macOS suites (GitHub's Mac runners wait hours). |
| `workers` | The conductor plus headless local workers (`crew run`). | The same as `solo`, with tracks moving in parallel. Workers write scripts, tests, docs and small code, and read diffs. | Builds and app runs: a worker does not run cargo, `nsh` or `just` recipes; those go to the conductor or a machine. |
| `fleet` | Workers plus cloud machines per platform (Linux, macOS, Windows). | Builds and full suites at a pull request's exact head, on-screen proof through `just e2e` with pictures and a recording, the iOS Simulator, the Android emulator, store packages. | Nothing a person must do: a store's Submit, a password, a signing key. |

## Who may do what

The maintainer is the person named in the repository's ownership. For this repository that is the owner of the hexuria organisation. A person who starts the crew on their own fork is the maintainer of that fork's crew only.

GitHub's permissions and branch protection are the gate, not these files. The launcher never merges, tags or deploys (`crew/RULES.md`, "Ready, merge and tag").

| Who | Merge | Tag (publishes) | Deploy to production | Push |
| --- | --- | --- | --- | --- |
| This repository's maintainers | yes | yes | yes | yes |
| A conductor its maintainer told to merge, running with that person's GitHub rights | runs the printed merge commands for ready pull requests | only when told, with the tag in its `STATE.md` first | no | its own branches |
| Any other conductor | no: it prepares, reviews, and passes on the printed merge commands | no | no | its own branches |
| A worker | no | no | no | its own branch, through `python3 crew/bin/crew push` only |
| The launcher itself | never: `crew batchwatch` waits and prints the merge commands | never | never | the state remote, and a worker's branch through `crew push` |
| A contributor's own crew on a fork | only in the fork, where GitHub lets them | only in the fork | no | the fork; pull requests here like any contributor |

## One turn

```
wake on an event → read state → decide → order a worker or a machine → review → gate → batch → merge → write state
```

An event is a result file landing, a pull request's check settling, a held machine stopping, or a batch finishing.
`crew watch` waits for the next one. `crew status` shows the state at a glance.

## Folder map

```
AGENTS.md                  for any agent opening this repository
CLAUDE.md                  one line: @AGENTS.md
.claude/skills/crew/       the /crew skill (both engines read it)
crew/README.md             this file
crew/GOAL.md               the goal, the definition of done, the tracks
crew/RULES.md              the rules for every crew member
crew/REPOS.md              the five repositories, their pins and the order of a cross-repo change
crew/roles/<role>.md       one file per role: what it owns, how it works, its proof, its state, its hand-back
crew/playbooks/            how to do the recurring jobs: restart, batch, review, worker-task, machine-order,
                           ui-proof, release, state, sandbox
crew/bin/crew              the launcher: doctor, prompt, start, run, status, watch, batchwatch, state, sync, lint
```

## Live state

The live state lives in a state folder, `.crew/state` by default, laid out like this:

```
roles/<role>/STATE.md      what the role has open and what comes next
roles/<role>/inbox/        task files (NNN-<name>.md)
roles/<role>/outbox/       result files (NNN-<name>.md)
queue.json                 pull requests waiting for the gate, and the running batch
LOG.md                     one line per decision, newest last
HELD.md                    what is waiting for the maintainer; never re-sent
```

It is not on `main` because it changes every few minutes, and `main` changes only by reviewed pull request.
A state remote must be a different repository from the code, and its branch name starts with `crew-state`.
`crew state pull` and `crew state push` move it to and from a separate remote, if you set one.
`crew/playbooks/state.md` says what each file holds.

## Changing a role or a rule

Open a pull request that edits the file in `crew/`. It is reviewed like code. `crew lint` checks the public-safety rules.
A rule that lives only in a chat is not a rule: the next session will not see it.

## Without a fleet

Run the `solo` or `workers` tier. Prove code with tests and the GitHub suites (`just ci-test`).
For screen work, run `just e2e <target>` on a machine you own, if you have told the conductor that builds may run there.
Leave work you cannot prove as a draft pull request that says what proof is missing.
Each role file has a "Without a fleet" section that says what that role does instead.
