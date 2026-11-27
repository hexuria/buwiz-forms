# Playbook: a worker in a sandbox (experimental)

This is experimental. Use it, measure it, and change this file by pull request when you learn something.

## What it is

```sh
crew run <role> <task-file> --sandbox
```

starts the worker inside a Docker `sbx` sandbox instead of on the computer itself. Without a sandbox, a worker is limited by an allow-list of commands (`crew/RULES.md`, "Local workers"). In a sandbox the worker may run any command, and the sandbox's own limits decide what that command can reach. Before you trust it with a task, check what your sandbox can see: which folders, which network, which credentials.

## What it is for

- Work that needs commands the allow-list refuses but that stay small: a script with a pipe, a test that writes temporary files, a linter, a formatter.
- A single-crate `cargo check` or `cargo test` on a small crate.
- Work where the plain-command rule of the allow-list gets in the way (environment variables in front of a command).

## Its limits

- Small disk. Script tests, lint, single-crate checks: yes. Full app builds, the NSH tool, store packages: no.
- No emulator, no Simulator, no window. Anything on a screen goes to a cloud machine (`crew/playbooks/machine-order.md`).
- The sandbox does not change the rules. The hard limits in `crew/RULES.md` hold inside it: no secrets, the safety variables, no production.
- Pushes from the sandbox use the crew's commit identity, checked before the push, as anywhere else.
- A result from the sandbox says so in its first line, so the reader knows what it was proven on.

## When to use the allow-list instead

For most scripts-and-tests tasks the allow-list is enough and simpler: no image to pull, no disk to size. Use the sandbox when a task keeps being refused for commands that are harmless inside it.
