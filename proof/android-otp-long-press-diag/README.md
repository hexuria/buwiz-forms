driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent b559e40d (app 7d155e6d, kit 6f2afaa8 + diag.patch)

# Android OTP long-press diagnostic

## Setup facts

- App base commit: `7d155e6df0fdaa7ec28425f6d1f24379c4f4f714`; the measurement build line read `c21de7ef` because this proof/harness commit sits on that base. No app runtime source under `crates/` or `nsh/` is changed by that commit.
- Sibling NSH checkout: `6f2afaa8e0e8041deb7533ea9ae0228dd717a62e`, plus the local uncommitted diagnostic patch. Plug checkout: `b559e40dc593bd8e8cd78060ddb5a3507a237767`, plus the local uncommitted gate patch. Both diffs are concatenated in `diag.patch`; neither was pushed to hexuria/nsh or hexuria/nsh-gpui-agent.
- Plug rev read from `nsh/Cargo.lock`: `b559e40dc593bd8e8cd78060ddb5a3507a237767`.
- `nsh --version`: `nsh 0.6.1 (6f2afaa)`.
- Diagnostic Android runtime `classes.dex` sha256: `29f8553410ec8fbf2735e50a0eaa3b75abe0ca380391b368126e57181cd8bd7e`.
- Cargo: `cargo 1.99.0 (5f94df478 2026-08-27)`; `RUSTC_WRAPPER` empty.
- Emulator API 35; every run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, no `--allow-send`.
- Hold duration: 1100 ms. Points are the probe points already recorded in `points.json`: stock P1 `(97.14,54.48)` pt, stock P2 `(163.81,54.48)` pt, ours P1 `(64.38,371.05)` pt, ours P2 `(122.09,371.05)` pt. Ours P2 is `-63.01` pt horizontally and `+2.25` pt vertically from part 2's centre `(185.1,368.8)` pt.

## Diagnostic plug gate

The plug patch adds one diagnostic-only path:

1. `AgentBridge.hitAt` receives the requested foreign tag string and returns a distinct sentinel only when the topmost native hit view's `getTag()` exactly equals that string. The activity patch sets the stock EditText tag to `J57STOCK`.
2. `checked_point_target` admits that sentinel only for an x/y long press when `J57_DIAG_FOREIGN_TAG` is nonempty. The code reads the process environment first and `option_env!("J57_DIAG_FOREIGN_TAG")` second. Because an Android app process has no launch environment, the matrix builds had `J57_DIAG_FOREIGN_TAG=J57STOCK` in the plug compile environment; the runner also exported it for every matrix run. The no-var control had it unset in both places.
3. After the gate the original path is unchanged: `AndroidNative.long_press` → `tap` → `TestAgent.touch` DOWN, sleep, UP → activity-root `dispatchTouchEvent`.

No-var control: `followup-control/stock-p1-run1` failed at `h1` with `target_gone: coordinate has no semantic target`, receipt `delivery_status: not_delivered`, and zero `J57STOCK` touch lines. With the var, all 12 matrix recipes passed and every hold logged DOWN/UP at the intended field.

## Matrix table

| field | point | run | selection after the hold | action mode + items | notes |
|---|---|---:|---|---|---|
| ours | p1 | 1 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; tree selection 0-9; system-text-menu selection [system-text-menu-assist:disabled, system-text-menu-cut:enabled, system-text-menu-copy:enabled, system-text-menu-share:disabled, system-text-menu-other:disabled, system-text-menu-other-2:disabled]; Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1; CANCEL no |
| ours | p1 | 2 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; tree selection 0-9; system-text-menu selection [system-text-menu-assist:disabled, system-text-menu-cut:enabled, system-text-menu-copy:enabled, system-text-menu-share:disabled, system-text-menu-other:disabled, system-text-menu-other-2:disabled]; Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1; CANCEL no |
| ours | p1 | 3 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; tree selection 0-9; system-text-menu selection [system-text-menu-assist:disabled, system-text-menu-cut:enabled, system-text-menu-copy:enabled, system-text-menu-share:disabled, system-text-menu-other:disabled, system-text-menu-other-2:disabled]; Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1; CANCEL no |
| ours | p2 | 1 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; tree selection 9-9; system-text-menu insertion [system-text-menu-select-all:enabled, system-text-menu-autofill:disabled]; Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1; CANCEL no |
| ours | p2 | 2 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; tree selection 9-9; system-text-menu insertion [system-text-menu-select-all:enabled, system-text-menu-autofill:disabled]; Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1; CANCEL no |
| ours | p2 | 3 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; tree selection 9-9; system-text-menu insertion [system-text-menu-select-all:enabled, system-text-menu-autofill:disabled]; Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1; CANCEL no |
| stock | p1 | 1 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |
| stock | p1 | 2 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |
| stock | p1 | 3 | `0-9` | selection [Call, Cut, Copy, Share, Add, Message] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |
| stock | p2 | 1 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |
| stock | p2 | 2 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |
| stock | p2 | 3 | `9-9` | insertion [Select all, Autofill] | touch DOWN/UP logged; foreign-field hold delivered through the diagnostic gate; CANCEL no |

## Intercept and CANCEL lines for ours

The synthetic hold sends DOWN and UP only; no MOVE event is injected, so no MOVE intercept line exists. Repeated identical parent-chain lines are counted here; the raw lines are in each run's `logcat-j57.txt`.

| point | run | intercept lines in the hold window | EditText ACTION_CANCEL |
|---|---:|---|---|
| p1 | 1 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p1 | 2 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p1 | 3 | Box action=0 return=false ×19; ScrollBox$VScroll action=0 return=false ×1 | no |
| p2 | 1 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |
| p2 | 2 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |
| p2 | 3 | Box action=0 return=false ×19; Box action=1 return=false ×12; ScrollBox$VScroll action=0 return=false ×1; ScrollBox$VScroll action=1 return=false ×1 | no |

Thus `com.longbridge.nsh.Box.onInterceptTouchEvent` and `ScrollBox$VScroll.onInterceptTouchEvent` returned false in every logged interception. No `ACTION_CANCEL` reached our EditText in any ours run.

## Injection path

At plug `b559e40d`, an x/y recipe long press first goes through gpui-agent's semantic point check: `Agent::long_press_at` calls `checked_point_target`; that calls Android strict native `hit_at` and then requires the hit native view to be a semantic node or a hidden descendant of one. Without the diagnostic env/tag path, the foreign stock `EditText` raises `target_gone: coordinate has no semantic target` before `hold_long_press` runs.

With the diagnostic tag match, only that semantic gate is bypassed. `AndroidStrictNative.long_press` delegates to `AndroidNative.long_press`; `AndroidNative.long_press` calls `tap`, which sends DOWN, sleeps for the hold duration, then sends UP. Each event is JNI-called into `com.longbridge.nsh.TestAgent.touch`; `TestAgent.touch` creates a touchscreen `MotionEvent` and calls `root.dispatchTouchEvent(e)`, where `root` is the touch target chosen by `target(host)` — without a modal/dialog, `host.getRootView()`. Both the stock overlay and the kit field are descendants of that activity root. `AgentBridge.java` at this plug rev is not the touch injector; it has no touch/dispatchTouchEvent method.

## Evidence map

- `diag.patch`: exact local uncommitted kit patch followed by the local uncommitted plug patch.
- `followup-control/stock-p1-run1`: no-var control, still refused before delivery.
- `followup-matrix/<field>-<point>-run<n>/`: the 12 cold matrix runs, each with recipe receipts, report.json/report.md, screenshots, recording, post-wait snapshot tree, and filtered `J57STOCK`/`J57OURS`/`J57INTERCEPT` logcat.
- `followup-summary.json`: machine-generated matrix summary.
- `dumps.md`: property/parent-chain dump plus the intercept summary.
- The earlier round-one directories remain as evidence of the unpatched gate's refusal.
