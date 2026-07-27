# Worktree cleanup inventory — 2026-07-28

Recorded before removing 40 auxiliary worktree directories, per the
`macos-handoff.md` cleanup procedure (preserve both checkouts, inventory
every registration and unique commit, never delete a branch because its
worktree was removed).

Retained checkouts:

- `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir` — main worktree, holds the object database
- `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity` — active checkout

Every branch below still exists as a ref. Removing a worktree deletes only
its directory. Branches carrying commits not reachable from `1b77fc9` also
have a `rescue/<branch>` tag pinning the exact commit.

| Unique commits vs 1b77fc9 | Branch | HEAD | Rescue tag |
| ---: | --- | --- | --- |
| 5 | `codex/2551q-final-parity-root` | `00a2ff85878f` | `rescue/codex/2551q-final-parity-root` |
| 4 | `codex/0605-final-parity` | `2f5d29338646` | `rescue/codex/0605-final-parity` |
| 2 | `codex/html-renderer-docs` | `fad73222d079` | `rescue/codex/html-renderer-docs` |
| 2 | `codex/certify-1701q-xml` | `95896661cba9` | `rescue/codex/certify-1701q-xml` |
| 2 | `codex/1702mx-page4-final` | `081ca1bf90f7` | `rescue/codex/1702mx-page4-final` |
| 2 | `codex/1701-page2-final` | `66ef7eb01a59` | `rescue/codex/1701-page2-final` |
| 1 | `codex/macos-native-timeout` | `f76a877b9d07` | `rescue/codex/macos-native-timeout` |
| 1 | `codex/mac-print-ax-fix` | `0915fb4ab7b6` | `rescue/codex/mac-print-ax-fix` |
| 1 | `codex/linux-candidate-certification` | `5ffee7a4cc1b` | `rescue/codex/linux-candidate-certification` |
| 1 | `codex/2551q-raw-parity-pass2` | `72f44e29e383` | `rescue/codex/2551q-raw-parity-pass2` |
| 1 | `codex/2551q-final-parity` | `ff5a120da5e5` | `rescue/codex/2551q-final-parity` |
| 1 | `codex/2551q-diff-regions` | `c3a9d5ad4e17` | `rescue/codex/2551q-diff-regions` |
| 1 | `codex/2550q-parity-pass` | `95fd86eb90bf` | `rescue/codex/2550q-parity-pass` |
| 1 | `codex/1702rt-page4-parity` | `5e4c02d71ab4` | `rescue/codex/1702rt-page4-parity` |
| 1 | `codex/1702rt-final-parity2` | `5d31b9792742` | `rescue/codex/1702rt-final-parity2` |
| 1 | `codex/1702mx-strict-parity` | `12cda3f6ea1d` | `rescue/codex/1702mx-strict-parity` |
| 1 | `codex/1702mx-2018c-parity-pass` | `8fe5c78ce471` | `rescue/codex/1702mx-2018c-parity-pass` |
| 1 | `codex/1701q-visual-parity-pass` | `77695dc88510` | `rescue/codex/1701q-visual-parity-pass` |
| 1 | `codex/1701-annual-parity-pass` | `ea25070529e6` | `rescue/codex/1701-annual-parity-pass` |
| 1 | `codex/1601c-parity-pass` | `4187b86d9b4a` | `rescue/codex/1601c-parity-pass` |
| 1 | `codex/1601c-final-parity` | `55c4027ee422` | `rescue/codex/1601c-final-parity` |
| 1 | `codex/0619f-final-parity` | `74c0230dfe99` | `rescue/codex/0619f-final-parity` |
| 1 | `codex/0619e-final-parity` | `f500f1a2fa70` | `rescue/codex/0619e-final-parity` |
| 1 | `codex/0619-parity-pass` | `8f4de758b945` | `rescue/codex/0619-parity-pass` |
| 1 | `codex/0619-family-final-parity` | `aba721d0a136` | `rescue/codex/0619-family-final-parity` |
| 1 | `codex/0605-parity-pass` | `c58096c447e8` | `rescue/codex/0605-parity-pass` |
| 0 | `codex/poppler-raster-audit` | `9bc840619824` | `—` |
| 0 | `codex/2551q-font-mapping` | `9bc840619824` | `—` |
| 0 | `codex/2551q-final-parity2` | `969aa4052f09` | `—` |
| 0 | `codex/2550q2024-final-parity` | `7feb0952cb25` | `—` |
| 0 | `codex/1702rt-parity-pass-root` | `addac2803ec5` | `—` |
| 0 | `codex/1702mx-final-parity2` | `7feb0952cb25` | `—` |
| 0 | `codex/1701q-final-parity2` | `7feb0952cb25` | `—` |
| 0 | `codex/1701-final-parity3` | `7feb0952cb25` | `—` |
| 0 | `codex/0605-final-parity2` | `7feb0952cb25` | `—` |
| 0 | `/private/tmp/ebir-native-evidence-879554d-w1I01R` | `` | `—` |
| 0 | `/private/tmp/ebir-final-macos-diag-923878c` | `` | `—` |
| 0 | `/private/tmp/bir-2551q-page1-glyph` | `` | `—` |
| 0 | `/private/tmp/bir-1601c-queue-submission-b8ce` | `` | `—` |
| 0 | `/private/tmp/bir-1601c-queue` | `` | `—` |
