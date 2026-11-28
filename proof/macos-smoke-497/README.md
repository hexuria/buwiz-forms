driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent db0e4ac6127078a05751149f0ec6bed68204f911

# macOS desktop smoke at PR #497 head f030fbfd63a47cf2b322ffec6598122ad9fd6936

Machine: macOS 26.6.2 arm64 (M4 Pro). Setup: scripts/setup-macos.sh (exit 0, 7m23s wall) —
gpui-agent CLI 0.5.0 installed via the PREBUILT download route; the digest check passed
(the stamp ~/.cargo/bin/.gpui-agent-cli.buwiz-installed reads 0.5.0 / prebuilt / sha256 of the
binary, and the binary's own sha256 matches it); the prebuilt route is silent by design so no
digest line appears in the setup log. The setup's gpui-agent install step itself takes about a
second; the 7m23s is dominated by the nsh CLI compile.

Run: scripts/e2e/machines/macos.sh with no arguments, DEV test build, scratch database per run,
safety env EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BUWIZ_NO_KEYCHAIN=1 BIR_AUTH_BYPASS=1, never
BIR_SFTP_LIVE. The machine script itself passes --allow-send to the dry-run test build
(repository behavior, unmodified — the Queue recipe exists only in the dry-run build).

Run header: `app f030fbfd, gpui-agent-cli 0.5.0` — both correct.

## sign-up passed on run 1 of a possible 3 — stopped there per instructions.

## run1 table (from report.json)

| recipe | status | note |
|---|---|---|
| setup-start | pass | |
| sign-up | pass | the plug#14 name/address-pill failure did NOT occur this run |
| tax-year | pass | |
| navigate | pass | |
| open-2551q | pass | |
| money-boxes | pass | |
| sheet-press | skipped | "no desktop variant" |
| delete-row | setup | `$table: no node matches {"id_regex": "^(fp-table-.+)-add$", "group": 1}` — suite prerequisite missing, unchanged from earlier runs |
| queue | fail | `wait_until timed out: node fp-send-line not found` |
| birth-date | pass | |
| sign-out | known gap | `wait_until timed out: node locked-screen not found` |

Summary line: 1 fail, 1 known gap, 7 pass, 1 setup, 1 skipped.

## honest notes

- report.json's `cli_version` field reads 0.4.1 although the binary is 0.5.0: the runner's
  `installed_cli_version()` parses `cargo install --list`, which still carries the metadata of an
  old 0.4.1 source install in ~/.cargo/.crates.toml — the prebuilt download replaced the binary but
  not cargo's registry. `needs` gating in the runner therefore evaluates against 0.4.1. No smoke
  recipe needs more than 0.4.1 so nothing was wrongly skipped in this run, but a recipe needing
  0.5.0 would be refused on a prebuilt-installed machine. The machine-script header line (which
  reads the stamp file) is correct; the runner's is not.
- The runner exits non-zero because of the known gap / fail entries; the table above is read from
  report.json as instructed.
