driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent b559e40d (app 7d155e6d)

# Merged smoke suite proof, Android and Linux

Evidence only. The app was driven with `just e2e android smoke --record` and `just e2e linux smoke`; no adb input, uiautomator, xdotool or fixed-coordinate driver. Android recordings are the runner's per-recipe videos in `android/`. Linux has no gpui-agent screenshots/recording, so `linux/linux-suite.mp4` is the machine's x11grab recording of the passing Linux run with the same recipe fixes. Every run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a scratch database, and no `--allow-send`.

`just e2e check smoke`: `smoke.json: 11 recipes, 30 files validated by gpui-agent-cli 0.4.1, 0 problem(s)`.

## Build facts

| fact | value |
|---|---|
| app commit | `7d155e6df0fdaa7ec28425f6d1f24379c4f4f714` |
| sibling `../nsh` / NSH pin | `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` |
| `nsh --version` | `nsh 0.6.1 (8c78cfd)` |
| Android runtime classes.dex sha256 | `a53652883f334dbb2844bd2844fa5246097d2f49acddf068341c43702c46943c` |
| Android runtime nsh-ref | `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` |
| cargo | `cargo 1.99.0 (5f94df478 2026-08-27)` |
| plug rev | `b559e40dc593bd8e8cd78060ddb5a3507a237767`, read in the lock and from build dep-info paths |
| Android compile | passed: Rust crate, App.tsx, x86_64/armeabi-v7a/arm64 libraries and development host built |
| Linux compile | passed: Rust crate, App.tsx and Linux development host built |

No local plug patch was needed or used: this app's own `nsh/Cargo.toml` and `nsh/Cargo.lock` already pin all plug crates to b559e40d. The first attempt on the raw host was environmental (the Ubuntu 24.04-built nsh ELF cannot run on the 22.04 host); the actual target builds ran through the repo's `nsh2404.sh` image wrapper as in prior jobs.

## Android table (`android/report.json`, `android/report.md`)

| recipe | result | exact step/error or gap |
|---|---|---|
| setup-start | pass | |
| sign-up | pass | |
| tax-year | pass | |
| navigate | pass | |
| open-2551q | pass | |
| money-boxes | pass | the first attempt correctly refused because the floating section pill `fp-pill-list` covered the radio; the recipe now scrolls the form clear before clicking |
| sheet-press | skipped | no phone variant |
| delete-row | pass | |
| queue | refused | `presses a send control: run with --allow-send` (as required: nothing queued or sent) |
| birth-date | pass | |
| sign-out | known gap | s4: `wait_until timed out: node \`locked-screen\` not found` under mandatory `BIR_AUTH_BYPASS=1` |

Totals for the original full Android run were 7 pass, 2 known gap, 1 refused, 1 skipped; after the money-boxes recipe fix the focused Android rerun is 5/5 pass through money-boxes. The remaining full-suite rows are unchanged: sign-out known gap, queue refused, sheet-press skipped.

## Focused Android money-boxes rerun

`android-money-boxes-fix/` is the rerun at the same app/plug with only the prerequisite recipes and money-boxes (`setup-start`, `sign-up`, `tax-year`, `open-2551q`, `money-boxes`). All five passed; money-boxes passed in 26,133 ms. Its recording is `android-money-boxes-fix/06-money-boxes/recording.mp4`.

## Linux table (`linux/report.json`, `linux/report.md`)

| recipe | result | exact step/error or gap |
|---|---|---|
| setup-start | pass | |
| sign-up | pass | |
| tax-year | pass | |
| navigate | pass | |
| open-2551q | pass | |
| money-boxes | pass | |
| sheet-press | skipped | no desktop variant |
| delete-row | pass | |
| queue | refused | `presses a send control: run with --allow-send` (as required: nothing queued or sent) |
| birth-date | pass | |
| sign-out | known gap | s4: `wait_until timed out: node \`locked-screen\` not found` under mandatory `BIR_AUTH_BYPASS=1` |

Totals: 8 pass, 1 known gap, 1 refused, 1 skipped.

## Recipe-only changes

- `sign-up-android.json`: converted the Android variant to JSON and hides the software keyboard after each `set_value`; the low fields/save button otherwise refuse as keyboard-occluded.
- `tax-year-android.json`: same narrow flow with keyboard hides after palette/search input.
- `money-boxes-paced-phone.json`: added the narrow-layout scroll/part-2 navigation already present in the fallback, a keyboard hide before returning to Amended No, and a final `form-scroll` (`dy: 1000`) so the radios are clear of the floating section pill `fp-pill-list` before the click. The pill's 16x16 list icon was the real occluder in the first failure (`occluded_by: 5584`), not a child of the radio.
- `money-boxes-phone.json` / `money-boxes-paced-phone.json`: final position normalized so delete-row starts deterministically.
- `delete-row.json`: converted to JSON, uses the known 2551Q Schedule 1 table id, scrolls the narrow form, opens part IV, then exercises add/fill/delete through the sheet. This fixes the old resolver setup failure caused by the collapsed/culled table.
- `smoke.json`: points Android at the fixed variants and updates stale known-gap wording. No app source was changed.
