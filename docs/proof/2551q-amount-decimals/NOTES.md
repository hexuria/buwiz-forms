# 2551Q amount decimals

## Official behavior

The official program rounds a third decimal to centavos when a money box loses focus; it neither refuses the amount nor cuts the third digit off.

Evidence:

- `rules/forms/2551q-v2018/validations.json:36-56`: rule `2551q-input-money`, phase `blur/change`, official behavior `parseFloat/toFixed(2); NaN becomes 0.00.`, source `official-hta-runtime#blockletter:L4421-L4430`.
- `rules/forms/2551q-v2018/fields.json:2380-2404`: `txtATCAmt1` is `decimal-money`, maximum input length 12, normalization `NumWithComma` / `formatCurrency`.
- `rules/forms/2551q-v2018/fixtures/runtime-control-inventory-v796.json:861-868`: the running control `txtATCAmt1` starts as `0.00` and has `maxlength` `12`.
- `rules/forms/2550q-v2024/evidence.md:40-44`: the pinned shared script's `formatCurrency`/`round` uses binary-number arithmetic followed by `Math.floor(value * 100 + 0.50000000001)`, the package's half-up cent rounding.
- `rules/forms/1601eq-v2018/fixtures/calculation-boundaries.json:6-7`: the package fixture records 0.005 reaching 0.01 and 0.0005 reaching 0.00 under that `formatCurrency` rule.

For the case in this work, `2000.005` therefore becomes `2000.01` (displayed `2,000.01`). The same money normalization applies to Items 15-17 and 20-22: `rules/tools/build-2551q-package.ps1:102-104` classifies `txt14`-`txt24`, `Amount`, `ATCAmt`, `ATCRate`, `ATCDue`, and `TotalSched` as decimal money normalized through `NumWithComma`/`formatCurrency`.

## Change

`crates/bir-app-api/src/screen_form_module.rs` now normalizes writes to the 2551Q money boxes through the official cent rounding before writing and before the amount-limit refusal. Consequently:

- `2000.005` is accepted and stored/uploaded as `2,000.01`;
- `2000.5` and `2000.50` remain accepted;
- `1000000000.01` still receives the amount-size sentence after rounding;
- `-100` still receives the non-negative sentence;
- Items 15-17 and 20-22 use the same normalization.

## Validation so far

`cargo test -p bir-app-api --lib form_2551q_entry_check_and_queue_agree_on_amount_limits -- --nocapture`: PASS, 1 passed / 0 failed. Full output: `rust-test.log` in this directory.

`cargo check -p bir-app-api`: PASS. Output: `cargo-check.log` in this directory.

Real-window hand proof for `2000.005` is in `manual-proof/`: the box rounded the typed value to `2,000.01`, Done closed the row, the Schedule 1 total became `60.00`, and the scratch database stored `2000.01` / `60.0`.
