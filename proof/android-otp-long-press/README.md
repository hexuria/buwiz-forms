driver: gpui-agent-cli 0.4.1

# Job 57 — the Android long press on the digit boxes: kit A (base) vs kit B (candidate)

Recipes: `otp-prep.json`, `otp-tin-first.json`, `otp-lp-first.json`, `otp-tin-after-tap.json`, `otp-lp-after-tap.json`, `otp-tin-scroll.json`, `otp-probe.json` (the suite `otp.json`). Driven only by `just e2e android otp --record`; adb used for install/reverse/logcat only. Every run is a cold relaunch on a scratch database with `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`.

## Builds

| fact | kit A (base) | kit B (candidate) |
|---|---|---|
| app commit | 517374de (main) | 517374de (main) |
| sibling ../nsh | 6f2afaa8e0e8041deb7533ea9ae0228dd717a62e | 95e6868e4b4d1c7e0ea62964a6b54c471ec2a904 |
| `nsh --version` | nsh 0.6.1 (6f2afaa) | nsh 0.6.1 (95e6868) |
| runtime classes.dex sha256 | 8a67503a7dd194bd1a81541ddfccef0c580011e8d71128b57077ecb62c6a0376 | 8a67503a7dd194bd1a81541ddfccef0c580011e8d71128b57077ecb62c6a0376 |
| runtime nsh-ref | 6f2afaa8… | 95e6868e… |
| cargo --version | cargo 1.100.0 (3acc79c0c 2026-11-05) — does not say huacnlee | same |
| which -a cargo | /home/ubuntu/.cargo/bin/cargo (only) | same |
| ~/.cargo/config.toml | does not exist | same |
| ~/.cargo/targets | does not exist | same |
| API level | 35 | 35 |
| long_press_timeout | 400 | 400 |

The classes.dex sha256 is the same for both kits because the candidate changes only `crates/nsh-base/src/otp_input.rs` (Rust, in the native library); the dex is the unchanged JVM scaffolding. The kit difference is carried by the runtime's `nsh-ref` file and the native library. Runs at each kit used `NSH_TOOLS=tools-<kit>-<ref>` and `NSH_RUNTIME=tools-<kit>-<ref>/runtime`, verified in each run's app.log ("nsh android release runtime: <ref>").

## Step 0 (headless, in the kit checkouts)

| command | kit A | kit B |
|---|---|---|
| cargo test -p nsh-base --test base otp | 49/49 pass | 49/49 pass |
| cargo test -p nsh-testing | 115/115 pass | 115/115 pass |

B failed nothing, so the matrix ran.

## How the recipes work

The TIN field (`new-tp-tin`, a textbox "TIN, 9 digits") is typed at `key_delay_ms` 80. The long press uses gpui-agent's x/y form, with the point computed **in the same run** from the field's frame in the tree (its centre) — never typed in by hand. The runner injects `<param>_cx`/`_cy` from the resolved node's bounds and writes the instantiated recipe (`otp-*.instantiated.json`, in each evidence folder) with the real numbers. Because the layout shifts when the keyboard opens during typing, the type steps and the long press are separate recipes in one suite run (the app state persists; the resolve re-runs against the settled tree). A `wait` step (2500 ms) after the long press lets the menu lifecycle play out inside the recording.

## Verdicts

Typed digits came out in the order typed (123-456-788) in every run at both kits — no reordering at `key_delay_ms` 80.

### TIN, long press at the centre (x/y computed in-run from the field's tree frame)

| cell state | build | whole code selected | menu seen | handles seen |
|---|---|---|---|---|
| first long press | A | 0/3 | insertion menu "Select all" + overflow, 3/3 (late frames) | one caret handle under cell 3, 3/3 |
| after one tap | A | 0/3 | insertion menu "Select all" + overflow, 3/3 | one caret handle under cell 3, 3/3 |
| first long press | B | consistent with whole selection in 3/3 (grey shading across all 9 cells post-press; selection-type menu "Cut Copy Share" captured in run 1; selection start handle at the left edge of cell 1 in 3/3) | selection menu "Cut Copy Share", captured in 1/3 (menu timing vs recording length) | start handle at cell-1 left edge 3/3 |
| after one tap | B | same evidence pattern in 3/3 | menu not captured in the frames (timing) | start handle at cell-1 left edge 3/3 |

Reading of the evidence: at A the long press lands on the hidden field as a caret (insertion menu, no selection) — the reported bug. At B the press selects: the cells shade across the whole code right after the press, the selection start handle sits at position 0, and the menu is the selection kind (Cut/Copy/Share), which Android only shows when text is selected. The end handle is not clearly visible in the frames; whether the grey shading is the selection itself or a transient is exactly what part 2's tree-read `selection:` state settles unambiguously.

### Scroll cell (B): a scroll starting on the TIN selects nothing (no shading in the post-scroll frames); a tap on the field after the scroll places the caret (evidence in the tin-scroll recordings). At A the scroll cell behaves the same visibly.

### Exact gap/refusal messages

- `long_press` with `target` on the TIN box: refused `host_error: occluded_by: 3` — the hidden field (node 3) covers the box; evidence for the cause itself. The x/y form was used instead, centre computed in-run.
- Numeric placeholders in recipe x/y fields are impossible through the CLI: validation runs before parameter binding, so `x: "$tin_field_cx"` fails with `error: recipe json: expected a finite number, got "$tin_field_cx"`. The runner works around it by writing an instantiated copy per run (committed in each evidence folder). This one is filable against gpui-agent.
- Secret scanner, typing into PIN/code boxes is refused at suite check with `<file> types into or keys '<target>': never a password, PIN, passcode, secret, token, code box or the software keyboard` — a safety rule, left as it is. The PIN and sign-in-code inputs are gaps because of it.
- Aiming at the first or last cell: not possible — gpui-agent can't aim inside a target; per instruction no offsets were computed.
- `scroll` with a target on the TIN fails `node 3 is not scrollable`; the scroll uses the x/y form starting on the field's centre.

## What each folder shows

- `kit-A/` and `kit-B/`: one folder per cold run (`tin-first-lp-first-runN` = the "first long press" state, `tin-after-tap-lp-after-tap-runN` = the "after one tap" state, `tin-scroll-runN` = the scroll cell). Each holds the runner's `report.json`/`report.md` and, per recipe, the receipt, screenshots, the instantiated recipe and `recording.mp4`.
- The recording is the evidence for everything system-side (selection handles, the text menu): both are Android system UI outside the app's tree.

## What this proof does not show

- The PIN and sign-in-code boxes: the suite's secret scanner refuses `type`/`set_value`/`paste`/`key` into any pin/password/otp/code-box target absolutely (a safety rule, left as it is). Exact scanner message in the report.
- Aiming the long press at the first or last cell: gpui-agent can't aim inside a target; per instruction no cell offsets were computed. Only the centre was pressed.
- Reading the selection from the tree: the plug in these builds (nsh-gpui-agent a6bb130a) has no selection state in the tree (`select` count 0).
