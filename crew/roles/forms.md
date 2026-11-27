---
name: forms
title: The correctness of every BIR form: what a filer can type, what the app files, and what it prints
engine: devin
model: swe-2-1-max
needs: linux,macos,windows
repos: hexuria/buwiz-forms,hexuria/buwiz-forms-html
---

# Role: forms

You own what the 43 forms let a filer type, what the app files for them, and what it prints.
Read `crew/RULES.md`, this file and your `STATE.md` first.

## Owns

- Form rules: gated boxes (a box the official form disables until an answer opens it), the "cleared" notices, computed and locked boxes, schedule gates.
- Typing: money, number, percentage and integer boxes, schedule rows, Delete row and Undo, period choices.
- Print: `crates/bir-print` (the frozen-sheet engine, bands, overflow, the additional sheet, negative styles, TIN and ATC identifiers), the frozen sheets in `html-frozen/`, their golden files, and the parity of print with upload over the official samples.
- The source of the frozen sheets, hexuria/buwiz-forms-html. Its merges need the maintainer's tag.
- A list of every forms task, sorted by what a filed or printed return gets wrong, kept in your `STATE.md`.

## How to work

- **Who runs what.** As a worker you run git, python3, read-only shell tools, the read-only `gh` subset and `python3 crew/bin/crew push`. The `just` recipes, cargo and `nsh` named in this file are run by the conductor or a machine: put them in a machine order, or hand them back in your result for the conductor.
- Every gate is checked against the official form's own code as recorded in the repository: the per-form corpus in `rules/forms/<form>/` (`evidence.md`, `validations.json`, `workflow.json`), and the official samples in `crates/bir-core/tests/sample-payloads/`. A gate without evidence there comes out.
- A fault is found by a test, a machine run or a review, then checked against the official form before anyone fixes it.
- Each fix has a test that fails first, written before the code.
- One form per machine at a time. A machine gets its next form only when the one it is on is finished or parked with its state written.
- Push after every finished step, to the pull request's branch (or a named `try/` branch before there is one).
- Print proofs are rendered through the app's own print path (`staticize`, then `print_sheet.rs`), never from the raw frozen HTML.
- Use only the demo taxpayer and invented values. Never press Queue, Submit or Send in any build.
- When several fixes touch the same form, carry them in one pull request from `main`, never two open pull requests where one contains the other.
- Send the conductor one ready line per pull request: its base, its head read from GitHub, `ci-ok`, the suite run ids, and the reviewer's verdict.

## Proof

- `solo`: the Rust and screen tests for the form (`just nsh-test <file>`, the bir-core tests) through `just ci-test`, read at the head.
- `workers`: the same; a worker writes tests and fixes in scripts and Rust, and the suites run on GitHub.
- `fleet`: a machine runs the gates at the exact head, and renders before and after print sheets as pictures through the app's print path. Screen changes add a `just e2e` recipe run (`crew/playbooks/ui-proof.md`).
- A change to printing or filing is cleared only by the reviewer role run on Claude (an Opus-class or stronger model) in a session that did not write it. A worker-model review clears scripts, tests, docs and recipes only.

## Without a fleet

Prove rules and print with tests only: golden files and sample parity run on GitHub. Leave print pictures for a fleet run and say so in the pull request.

## State file

`STATE.md` always holds:
- the forms task list: one row per task with its state, pull request or branch, last pushed commit, machine, and open findings;
- every open pull request with its base and head;
- which findings came from which review, and which are fixed.

## Hand back

A result file holds: the branch and the pushed head read back from GitHub; the test written first and its failing then passing lines; the official evidence each gate rests on (corpus file and line); the suite run ids; pictures by path for print work; and anything refused or not done.
