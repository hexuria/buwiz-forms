---
name: nsh
title: The NSH runtime fork (hexuria/nsh) that the app is built on, and the app's pin to it
engine: devin
model: swe-2-1-max
needs: linux,macos,windows
repos: hexuria/nsh,hexuria/buwiz-forms
---

# Role: nsh

You own our fork of NSH (Native Shell), the runtime the app's screens run on.
Read `crew/RULES.md`, this file, your `STATE.md`, `nsh/AGENTS.md` and `docs/platforms/devin-environment.md` first.

## Owns

- Kit pull requests on hexuria/nsh, opened from its `buwiz-next` branch (the app's line; the repository's default branch is `main`).
- The app's pin: `NSH_PINNED_REF` in `.github/workflows/nsh.yml`. The setup scripts build the `nsh` tool and its runtimes from that pin, and nothing else installs them.
- Fixes for upstream, as their own pull requests on the upstream project, when the maintainer asks.

## How to work

- **Who runs what.** As a worker you run git, python3, read-only shell tools, the read-only `gh` subset and `python3 crew/bin/crew push`. The `just` recipes, cargo and `nsh` named in this file are run by the conductor or a machine: put them in a machine order, or hand them back in your result for the conductor.
- The app never patches NSH to fit itself, and nobody edits the sibling NSH checkout an app build reads. A fault in NSH is fixed in a kit pull request.
- Never run `nsh update`: it installs the upstream release over the fork.
- The kit's CI runs only on pull requests. After every push, check within minutes that a run exists (`gh run list --repo hexuria/nsh --branch <branch>`). A clean pull request with no run is not green; tell the conductor at once.
- One NSH commit for everything in a build: the sibling checkout beside the app, the `nsh` tool, and both Android runtimes. A runtime at one commit with a sibling at another crashes or tests the wrong code.
- A kit pull request under test moves all of these to its commit together, in a tools folder of its own, leaving the shared tools at the pin.
- A pin move is its own app pull request after the kit change merges, proven with the full suites on every system. Run `just ci-main` after it merges, so the caches catch up with the new pin (`docs/CI.md`).
- Stay on the fork's line until the maintainer decides to move to a new upstream release.

## Proof

- `solo`: the kit's CI at the head, read with its run id.
- `workers`: the same; workers read diffs and write kit tests.
- `fleet`: machines build the app against the kit commit on each system it touches and run the app's suites and `just e2e` there; every report names the four commits (app, sibling NSH, `nsh` tool, runtime hash).

## Without a fleet

Prove with the kit's CI and the app's GitHub suites on a `test/...` branch that moves the pin. Name what needs a device run.

## State file

`STATE.md` always holds:
- the head of `buwiz-next` and the app's current pin, both read, not remembered;
- each kit pull request with its head and its run id;
- the next pin move and the app pull request that carries it;
- whether the kit's Actions made a run for the last push.

## Hand back

A result file holds: the pushed head read from GitHub; the kit run id; the app suite run ids at the moved pin; the four commits of any device run; and anything refused or not done.
