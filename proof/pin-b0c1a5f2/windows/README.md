driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b

# Windows proof for PR #584 at b3bddd4d252d0daac05b322f18ce6e45e6c1fa18

The repository pin file names gpui-agent-cli 0.5.0. This machine's CLI was installed with the pinned cargo-install route (`cargo install gpui-agent-cli --version 0.5.0 --locked`); `gpui-agent --version` prints `gpui-agent-cli 0.5.0`.

## Lock check, before the build

`cargo metadata --locked --format-version 1 > NUL`: exit 0.

`cargo tree --locked --all-features --target x86_64-pc-windows-msvc -i windows@0.62.2`: exit 0. Lines naming gpu-allocator:

```text
├── gpu-allocator v0.28.0
```

`cargo tree --locked --all-features --target x86_64-pc-windows-msvc -i windows-core@0.62.2`: exit 0. Lines naming iana-time-zone:

```text
(none: iana-time-zone does not appear in this Windows target inverse tree)
```

Full output: `lock-windows.txt`, `lock-windows-core.txt`. Both were captured before the long build and have machine-local paths replaced with `<home>`.

## Run 1 smoke

| recipe | status | scrolled | note |
| --- | --- | --- | --- |
| setup-start | pass | {} |  |
| sign-up | pass | {} |  |
| tax-year | pass | {} |  |
| navigate | pass | {} |  |
| open-2551q | pass | {} |  |
| money-boxes | pass | {} |  |
| sheet-press | pass | {} |  |
| delete-row | pass | {} |  |
| queue | pass | {} |  |
| birth-date | pass | {} |  |
| sign-out | known gap | {} | wait_until timed out: node `locked-screen` not found |

## Run 1 rdo-row-press

| recipe | status | scrolled | note |
| --- | --- | --- | --- |
| setup-start | pass | {} |  |
| rdo-row-press | pass | {} |  |
| sign-up | pass | {} |  |
| profile-pick-press | pass | {} |  |

## Run 2 smoke

| recipe | status | scrolled | note |
| --- | --- | --- | --- |
| setup-start | pass | {} |  |
| sign-up | pass | {} |  |
| tax-year | pass | {} |  |
| navigate | pass | {} |  |
| open-2551q | pass | {} |  |
| money-boxes | pass | {} |  |
| sheet-press | pass | {} |  |
| delete-row | pass | {} |  |
| queue | pass | {} |  |
| birth-date | pass | {} |  |
| sign-out | known gap | {} | wait_until timed out: node `locked-screen` not found |

## Run 2 rdo-row-press

| recipe | status | scrolled | note |
| --- | --- | --- | --- |
| setup-start | pass | {} |  |
| rdo-row-press | pass | {} |  |
| sign-up | pass | {} |  |
| profile-pick-press | pass | {} |  |

No step reported `scrolled` in either run; there is no error_code `scrolled` finding.

Both machine-script invocations ended nonzero because the existing `sign-out` recipe is a known gap (`locked-screen` not found); every other recipe passed. The smoke queue recipe ran only in the script's dry-run test environment.

## Print preview

No print-preview press recipe for Windows exists in `agent-recipes/e2e` at this head (no file or step naming print preview), so none was run and none was written.

Evidence folders are copies of the runner output with machine-local paths replaced with `<home>`.
