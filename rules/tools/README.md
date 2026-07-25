# `rules/tools/` — what still runs, and what is a provenance record

This directory holds 70 PowerShell scripts. **Most of them can no longer run
anywhere, including on Windows**, and that is expected rather than broken. Read
this before assuming a script here is a tool you can invoke.

## The structural fact

`rules/` tracks only `.json`, `.ps1` and `.md`. It contains no `.hta`, no
`atcCodes.xml`, no savefile XML, no `BIRForms.exe`, and no extracted installer
directory. Every one of those inputs lives on a Windows machine with the Offline
eBIRForms package installed, pinned by SHA-256 at a machine-local path in each
form's `manifest.json` — never copied into the repository.

So any script whose input is an extracted package, an HTA, a catalog XML or a
savefile **cannot run from a clone on any operating system**. It is a record of
how a piece of evidence was originally derived, kept so the derivation is
auditable. It is not a tool.

## Live tooling

Five scripts were the maintained regenerate-and-verify loop. All are now
`bir-rules-codegen` subcommands, which run anywhere `cargo` runs.

| script | replaced by |
| --- | --- |
| `../validate.ps1` *(removed)* | `validate-v1` |
| `../validate-json-schema.ps1` *(removed)* | folded into `validate-v1` |
| `build-2550q-serialization-bindings.ps1` | `build-2550q-bindings` |
| `update-2550q-v2-static-projections.ps1` | `project-2550q` |
| `update-2550q-v2-group-projections.ps1` | **not ported — spent**, see below |

The three remaining `.ps1` files are kept as provenance for how the current
snapshot was produced. Prefer the subcommands; they are cross-platform, emit
canonical JSON, and are covered by tests.

Both validators have been removed. They were Windows-only in practice: each
partitioned the v2 trees with literal `\` separators, so on macOS and Linux the
exclusion never matched and the wrong file set was audited. `validate.ps1`
exited 1 on macOS; `validate-json-schema.ps1` was worse, because it *succeeded*
while validating the wrong set — a broken gate that reported success.

They were kept for a while pending a CI run of the replacement on all three
operating systems. That was the wrong trade: leaving a validator that reports
success over the wrong file set is more dangerous than removing it, and the
Rust replacement reproduces all eight corpus counts and is exercised on every
platform by the same job.

For the record, since the reasoning outlived the files: the defects were at
`:5-6` (`.TrimEnd('\') + '\'` appends a literal backslash, so the v2 exclusion
never matched off Windows), `:97` (`-notmatch '\\schema\\'`) and `:112`
(unreachable — `-RequireJsonSchema` could never fire). `Join-Path` normalizes
`\` to `/` on Unix, so the lines that *looked* wrong were fine.

### The two projectors are not peers — one is already spent

Tested empirically against a scratch copy of the current corpus:

- **`update-2550q-v2-group-projections.ps1` cannot run again.** It asserts a
  post-condition of 7 groups and **60** total candidate fields; the corpus has
  **94**, so it throws immediately. It was a one-shot migration that partitioned
  32 singletons plus 28 repeated-family descriptors into 60 fields, after which
  the static projector added the remaining 34. Having been applied, it is
  archaeology like the scripts below — not a tool. Do not "fix" the assertion to
  make it run; it would re-partition an already-projected rule set.
- **`update-2550q-v2-static-projections.ps1` is idempotent.** Re-running it
  reproduces `rule-set.json` and all 121 fixtures semantically identically
  (verified with `jq -S -c`), and it already works on macOS under pwsh 7.

So the ordering hazard once recorded here is moot: there is no ordering, because
only one of the two can execute. The static projector's byte formatting is also
inert — `source_set_sha256` is computed over *parsed* canonical JSON, and the
generated Rust embeds canonical JSON, so neither depends on how the file is
laid out.

`update-2550q-v2-static-projections.ps1` also contains unreachable branches
(`:160-166`, `:209-237`, `:241-252`, all past the `continue` at `:159`) and a
closing message at `:384` claiming 87 emissions where 34 occur. Neither is
ported.

## Provenance records — cannot run from a clone

The remaining 65 scripts fall into three groups.

**Package builders** (`build-<form>-package.ps1`, ~35 scripts) read an extracted
installer temp directory, a downloads folder and often `C:\eBIRForms\BIRForms.exe`,
and emit a form's complete `rules/forms/<form>/` evidence set. Their defaults are
one developer's absolute Windows paths.

**Extractors and inspectors** (`extract-*.ps1`, `inspect-*.ps1`, `audit-*.ps1`,
`discover-form-assets.ps1`, `read-extracted-form-lines.ps1`) read a specific
`.hta`, savefile or catalog XML and emit or print one inventory.
`audit-ebir-package-resources.ps1` additionally P/Invokes `LoadLibraryEx` and
`FindResource`, so it is hard-bound to Win32.

Several are subroutines of the package builders rather than independent tools:
`extract-encrypted-field-keys.ps1`, `extract-hta-function-inventory.ps1`,
`inspect-hta-controls.ps1` and `audit-1701-encrypted-fields.ps1` are invoked by
builders and embedded as provenance strings in emitted `evidence.md` files.

**Frozen literal emitters** (`build-1600pt-validations.ps1`,
`build-1600vt-validations.ps1`, `build-1601eq-validations.ps1`,
`build-1601fq-validations.ps1`, `build-1603q-validations.ps1`,
`build-1702q-validations.ps1`, `build-1702q-calculations.ps1`) read nothing and
emit a hard-coded rule table. They *execute* anywhere, but re-running one only
rewrites a file already in the tree. They are not part of any loop.

## Known hazards if you do reuse one

`rules/UPDATING.md:33-36` records the hazard: these builders write **directly**
into a historical `rules/forms/...` directory and mutate `rules/index.json`, so
pointing one at the canonical corpus for a new release overwrites the snapshot
documenting the old one.

The ported subcommands now have the guard — `project-2550q --staging-root`
mirrors the corpus layout beneath a staging root and refuses a target that
already exists. See `STAGING.md`. **The remaining `.ps1` builders do not**, so
the warning still stands for them.

`ConvertTo-Json` formats differently under Windows PowerShell 5.1 and pwsh 7
(5.1 emits two spaces after each colon and aligns nested objects under the
parent key's column). 43 of these scripts also append `[Environment]::NewLine`,
which is CRLF on Windows and LF elsewhere. Any file regenerated by a different
interpreter is therefore byte-different while semantically identical. Compare
with `jq -S -c`, not with `diff`, when judging whether evidence actually changed.

`run-full-audit-background.ps1` was removed. It hardcoded `R:\`,
`C:\Users\uriah\AppData\Local\Temp` and `powershell.exe`, so it encoded one
machine as a default and could not run anywhere else.
