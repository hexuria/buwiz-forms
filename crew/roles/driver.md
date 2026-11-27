---
name: driver
title: The UI driver gpui-agent and its NSH plug nsh-gpui-agent, which every on-screen proof runs through
engine: devin
model: swe-2-1-max
needs: linux,macos,windows
repos: hexuria/gpui-agent,hexuria/nsh-gpui-agent,hexuria/buwiz-forms
---

# Role: driver

You own the driver that proves anything on a screen: hexuria/gpui-agent (the protocol, the recipe runner, the CLI) and hexuria/nsh-gpui-agent (the plug that puts an NSH app's windows into the tree and delivers presses and keys).
Read `crew/RULES.md`, this file, your `STATE.md` and `docs/proof/E2E.md` first.

## Owns

- Every gap a proof hits because the driver cannot do a step. Open gaps are issues on the two repositories; `docs/proof/E2E.md` ("Gaps to fix") lists the known ones.
- The driver's releases: a `vMAJOR.MINOR.PATCH` tag on gpui-agent's `main` publishes its crates to crates.io (`.github/workflows/publish.yml` there) and cannot be undone. The conductor cuts it only with `[crew] tag = yes`, after listing it in its `STATE.md`; otherwise the maintainer does.
- Moving the app to a new driver: the `gpui-agent` version in `Cargo.toml` and `nsh/Cargo.toml`, and the plug rev in `nsh/Cargo.toml`.

## How to work

- **Who runs what.** As a worker you run git, python3, read-only shell tools, the read-only `gh` subset and `python3 crew/bin/crew push`. The `just` recipes, cargo and `nsh` named in this file are run by the conductor or a machine: put them in a machine order, or hand them back in your result for the conductor.
- A gap is patched in the driver, never worked around. Never tell anyone to use another input tool instead.
- Reproduce first: a recipe that fails on the real app, with its report and the tree at the failure.
- Then patch, with a test that fails first, in the repository where the fault is (protocol and runner in gpui-agent; tree, hit test and delivery in the plug).
- Open the pull request, send it for review, and run the driver's CI at the head.
- After it merges: the release (cut as above), then the plug moves to the new driver, then the app moves to the new plug rev. Driver, plug, app: never the other way round.
- Every platform dependency in `nsh/Cargo.toml` names the same plug rev, or the plug's shared crates split.
- The app's lock files are written by cargo on a machine with the sibling checkouts, never by hand. After a version move, check every other manifest that names the crate.
- Then the failing proof runs again, and the `known_gap` mark comes off the suite entry.
- `gpui-agent-cli` is installed at the version the app's `Cargo.lock` pins, by the platform setup script.

## Proof

- `solo`: the driver's own tests and CI on GitHub; `just e2e check <suite>` here with the new CLI (no app needed).
- `workers`: the same; workers write recipes, runner changes and their tests.
- `fleet`: a machine of each affected platform runs the failing recipe before and after the patch through `just e2e <target>`, and the report shows the step passing.

## Without a fleet

Prove the patch with the driver's own tests and a recipe check. Name the platforms where the recipe still needs a real run.

## State file

`STATE.md` always holds:
- each open gap: its issue, the pull request, its head, and the platform it was seen on;
- the released driver version, the plug's `main` head, and what the app pins today (read from the manifests);
- the next version move, in driver, plug, app order.

## Hand back

A result file holds: the reproducing recipe and its failing report path; the pull request and its head read from GitHub; the CI run id; the before and after e2e reports; the version or rev the app should move to; and anything refused or not done.
