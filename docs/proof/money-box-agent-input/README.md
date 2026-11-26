# PART 1 money-box agent-input probe (draft)

Sha `542e484401494623a7ccbae9e25832985739a3a8`, NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, debug macOS window launched with:

```sh
GPUI_AGENT_TOKEN=p1money-local-token GPUI_AGENT_ADDR=127.0.0.1:17421 CARGO_TARGET_DIR=/Users/devin/p1money/target BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 just nsh-run-agent macos /Users/devin/p1money/scratch/probe.db
```

(environment also used stock rustup cargo 1.99.0 first on PATH; `BIR_SFTP_LIVE` unset). No Queue/Send pressed. Loopback planes: window `127.0.0.1:17421`, data `127.0.0.1:17422`. Taxpayer `12345678800000` created through data-plane `profile.save`; 1601C periods in 2025 used one fresh return per case.

BOX A = `frm1601c:txtTax14` (Total amount of compensation), initially empty/zero. BOX B = `frm1601c:txtTax16` (MWE holiday/overtime/night/hazard pay), intended prefill `1234.56` by real keys. Important probe limitation: repeated gpui-agent `key` calls did not behave like one continuous hardware typing episode; each call appears to re-target/re-position the caret, producing reversed/decimal-shifted text (`1234.56` became visible as `65.4321`, and the real-key comparison `500.25` became `52.005`). Thus the BOX B precondition was not achieved as specified; results below are the exact observed behavior, not a clean comparison.

| box | call | 2025 period | before | after focused / after leaving | focus stayed | stored amount | onChange evidence | oddity/error |
|---|---:|---:|---|---|---|---|---|---|
| A | type | 4 | `` | `500.25` / left `500.25` | True | `500.25` | True |  |
| A | set-value | 5 | `` | `500.25` / left `500.25` | True | `500.25` | True |  |
| A | paste | 6 | `` | `` / left `` | False | `0.0` | False | unsupported_op: this host does not list `paste` in hello.features |
| B | type | 7 | `65.4321` | `500.2565.4321` / left `500.2565.4321` | True | `65.43` | True |  |
| B | set-value | 8 | `65.4321` | `500.25` / left `500.25` | True | `500.25` | True |  |
| B | paste | 9 | `1` | `1` / left `1` | True | `1.0` | True | unsupported_op: this host does not list `paste` in hello.features |
| A | realkeys | 11 | `` | `52.005` / left `52.005` | True | `52.005` | False |  |
| A | form.fill | 12 | `` | `500.25` / left `500.25` | False | `500.25` | True | app-owned fill applied and unfocused; data-plane invoke |

## Exact call shape

Window-plane calls were made with `gpui-agent --addr 127.0.0.1:17421 <op> <target> 500.25`; data-plane calls with `gpui-agent --addr 127.0.0.1:17422 invoke ...`. Full stdout/stderr JSON for automated cases is in `out/calls/*.json`; `results.json` records the exact command/output snippets.

## Unavailable / negative measurements

- `paste` was unavailable on this macOS window plane: host answered `unsupported_op: this host does not list paste in hello.features`; nothing was delivered.
- `press` was also unsupported on the window plane, so real keyboard focusing used `key` directly; this is a material limitation.
- No dedicated page `onChange` debug readout was found. `onChange evidence` above is inferred from dependent draft totals in SQLite (`tax_22_total_taxable` for Tax14, `tax_21_total_non_taxable` for Tax16), not from a direct JS event log.
- Stored values are direct SQLite `form_drafts.data_json` reads after moving focus to the sheets box.
- `form.fill` initially failed in the driver because nested JSON was encoded incorrectly; it was rerun correctly in period 12 and is the valid comparison shown.

## Files

PNGs are in `out/png/` with `<box>-<call>-p<period>-before/after` names. Machine-readable results: `out/results.json`. Launch log: `../logs/nsh-run-agent.log`; driver log: `../logs/probe-driver.log`.
