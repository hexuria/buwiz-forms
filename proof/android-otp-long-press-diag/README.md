driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent b559e40d (app 7d155e6d, kit 6f2afaa8 + diag.patch)

# Android OTP long-press diagnostic

## Setup facts

- App commit: `7d155e6df0fdaa7ec28425f6d1f24379c4f4f714`.
- Sibling NSH checkout: `6f2afaa8e0e8041deb7533ea9ae0228dd717a62e`, plus the local uncommitted `diag.patch` copied here. The patch was never pushed to hexuria/nsh.
- Plug rev read from `nsh/Cargo.lock`: `b559e40dc593bd8e8cd78060ddb5a3507a237767`.
- `nsh --version`: `nsh 0.6.1 (6f2afaa)`.
- Diagnostic Android runtime `classes.dex` sha256: `3a8dffbc94847f8216f684c034cb12379a7ca62b38bf8b5e4332d391d6d56491`.
- Stock runtime `classes.dex` sha256 before the diagnostic patch: `8a67503a7dd194bd1a81541ddfccef0c580011e8d71128b57077ecb62c6a0376`.
- Cargo: `cargo 1.99.0 (5f94df478 2026-08-27)`; `RUSTC_WRAPPER` empty.
- The workflow sibling-pin edit to kit `6f2afaa8` stayed local and uncommitted; the pushed proof records the diagnostic patch separately.
- Emulator API 35; every run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, no `--allow-send`.
- Hold duration: 1100 ms. gpui-agent recipe coordinates are points.
- Stock field width: 798 px, the TIN width (`304 pt × density 2.625`). Its probe rect was `141,84-939,202` px; the probe screenshot shows it in the blank header area, covering no pressed control.
- Points generated from the probe (`points.json`): stock P1 `(97.14, 54.48)` pt, stock P2 `(163.81, 54.48)` pt, ours P1 `(64.38, 371.05)` pt, ours P2 `(122.09, 371.05)` pt. P2 is 45 px right of glyph x(9). Ours P2 is `-63.01` pt horizontally and `+2.25` pt vertically from part 2's target-node centre `(185.1, 368.8)` pt.

## Stop condition

The stock field did not receive the injected hold. In both completed cold attempts at stock P1, the recipe failed before delivery with:

```text
target_gone: coordinate has no semantic target
```

The receipt records `delivery_status: "not_delivered"`; filtered logcat has zero `J57STOCK` pass-through touch lines (`touch edit=...`) in each run, while the same run has four `J57OURS` touch lines from the recipe's normal preparation clicks. Per the job order, the matrix stopped there. No other driver was used. The third cold attempt was interrupted after preparation while stopping the matrix and is kept as partial evidence only.

## Matrix table

| field | point | run | selection after the hold | action mode + items | notes |
|---|---|---:|---|---|---|
| stock | P1 | 1 | no hold delivered; last stock selection log `0-0` | none | `long_press` x/y `(97.14,54.48)` failed at `h1`: `target_gone: coordinate has no semantic target`; receipt `delivery_status: not_delivered`; 0 stock touch logs |
| stock | P1 | 2 | no hold delivered; last stock selection log `0-0` | none | same refusal; 0 stock touch logs |
| stock | P1 | 3 | not measured | not measured | interrupted after prep/type while stopping per the stop condition |
| stock | P2 | 1-3 | not measured | not measured | not run after the stop condition |
| ours | P1 | 1-3 | not measured | not measured | not run after the stop condition |
| ours | P2 | 1-3 | not measured | not measured | not run after the stop condition |

Because no hold was delivered to stock at P1, there is no stock word-selection fact under this injected hold.

## Property and parent-chain dumps

| fact | stock EditText | kit InputBox EditText |
|---|---|---|
| properties | `longClickable=true textSelectable=false focusable=true focusableInTouchMode=true movement=android.text.method.ArrowKeyMovementMethod edit.longClickListener=false touchListener=false` | `longClickable=true textSelectable=false focusable=true focusableInTouchMode=true movement=android.text.method.ArrowKeyMovementMethod edit.longClickListener=false touchListener=false` |
| parent chain | `edit > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.LinearLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.android.internal.policy.DecorView[intercept=com.android.internal.policy.DecorView,longClickListener=false touchListener=false] > android.view.ViewRootImpl` | `edit > com.longbridge.nsh.InputBox[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox$Content[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox$VScroll[intercept=com.longbridge.nsh.ScrollBox$VScroll,longClickListener=false touchListener=false] > com.longbridge.nsh.ScrollBox[intercept=com.longbridge.nsh.ScrollBox,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.Box[intercept=com.longbridge.nsh.Box,longClickListener=false touchListener=false] > com.longbridge.nsh.NshView[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.FrameLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > android.widget.LinearLayout[intercept=android.view.ViewGroup,longClickListener=false touchListener=false] > com.android.internal.policy.DecorView[intercept=com.android.internal.policy.DecorView,longClickListener=false touchListener=false] > android.view.ViewRootImpl` |

## Injection path

At plug `b559e40d`, an x/y recipe long press first goes through gpui-agent's semantic point check: `Agent::long_press_at` calls `checked_point_target`; that calls the Android strict native `hit_at` and then requires the hit native view to be a semantic node or a hidden descendant of one. The foreign stock `EditText` is in neither set, so the check raises `target_gone: coordinate has no semantic target` before `hold_long_press` runs. No `MotionEvent` is dispatched on this path.

If the semantic check passes, `AndroidStrictNative.long_press` delegates to `AndroidNative.long_press`; `AndroidNative.long_press` calls `tap`, which sends DOWN, sleeps for the hold duration, then sends UP. Each event is JNI-called into `com.longbridge.nsh.TestAgent.touch`; `TestAgent.touch` creates a touchscreen `MotionEvent` and calls `root.dispatchTouchEvent(e)`, where `root` is the touch target chosen by `target(host)` (without a modal/dialog, `host.getRootView()`). Both the stock overlay and the kit field are descendants of that activity root. `AgentBridge.java` at this plug rev is not the touch injector; it has no touch/dispatchTouchEvent method.

## Evidence map

- `diag.patch`: exact local uncommitted diagnostic patch.
- `recipes/`: suite and instantiated x/y recipe copies used for the diagnostic.
- `probe-run1/`: probe used to generate `points.json`.
- `stock-p1-run1/`, `stock-p1-run2/`: complete cold attempts showing prep/type pass and the refused hold. Each contains the runner report, per-step receipts, screenshots, recordings, filtered `J57STOCK`/`J57OURS` logcat, and post-wait tree.
- `stock-p1-run3-interrupted/`: partial evidence from the interrupted third attempt.
