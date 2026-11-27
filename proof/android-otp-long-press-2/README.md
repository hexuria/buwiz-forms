driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent b559e40d

# Android OTP long press proof, part 2

This folder is evidence only. The app was driven with `just e2e android otp` and the gpui-agent recipes in `agent-recipes/e2e/`; no adb input, uiautomator, xdotool or fixed-coordinate driver was used. adb was used only for install/reverse/logcat and the runner's recording support. Every run used `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a scratch database, and no Queue/Send/Submit.

## Builds

| fact | kit A | kit B |
|---|---|---|
| kit/runtime commit | `6f2afaa8e0e8041deb7533ea9ae0228dd717a62e` | `95e6868e4b4d1c7e0ea62964a6b54c471ec2a904` |
| sibling `../nsh` | `6f2afaa8e0e8041deb7533ea9ae0228dd717a62e` | `95e6868e4b4d1c7e0ea62964a6b54c471ec2a904` |
| `nsh --version` | `nsh 0.6.1 (6f2afaa)` | `nsh 0.6.1 (95e6868)` |
| runtime classes.dex sha256 | `8a67503a7dd194bd1a81541ddfccef0c580011e8d71128b57077ecb62c6a0376` | `8a67503a7dd194bd1a81541ddfccef0c580011e8d71128b57077ecb62c6a0376` |
| plug rev read from build | `b559e40dc593bd8e8cd78060ddb5a3507a237767` | `b559e40dc593bd8e8cd78060ddb5a3507a237767` |
| cargo | `1.99.0 (5f94df478 2026-08-27)`, pinned rustup cargo | same |
| build folder | `~/BuwizNsh/target-j57-A-part2` | `~/BuwizNsh/target-j57-B-part2` |
| API / long_press_timeout | API 35 / 400 ms | API 35 / 400 ms |

The build-time checkout was proof head `dd77c820` with uncommitted harness edits. `dd77c820` is app main `517374de` plus proof/harness commits only; `git diff 517374de dd77c820 -- crates nsh` is empty. The runtime code under test is therefore app main `517374de`. The plug was wired as local uncommitted edits of the `nsh-gpui-agent*` rev lines in `nsh/Cargo.toml`; that is a deviation from the requested literal `[patch]`/path form, but nothing of it is committed. Cargo, not a hand edit, rewrote `nsh/Cargo.lock`; it is also not committed. `kit-A/buildfacts.txt` and `kit-B/buildfacts.txt` contain the build lines and dep-info paths showing checkout `b559e40`.

## TIN matrix, centre long press

Typing used recipe `type` at `key_delay_ms` 80 and produced the typed order. The verdicts below are read from the saved gpui-agent trees (`post-tree.json`), not from pixels. Whole code means `selection: 0-9`.

| state | build | n/3 `selection: 0-9` | per-run selection after long press | menu kind/items after long press | result |
|---|---|---:|---|---|---|
| first long press | A | 0/3 | `9-9`, `9-9`, `9-9` | insertion: `paste/select-all/autofill(disabled)` in 3/3 | caret menu, no whole selection |
| after one tap | A | 0/3 | `9-9`, `9-9`, `9-9` | insertion: `paste/select-all/autofill(disabled)` in 3/3 | caret menu, no whole selection |
| first long press | B | 0/3 | `0-0`, `0-0`, `0-0` | no `system-text-menu` in 3/3 | no selection, no menu |
| after one tap | B | 0/3 | `0-0`, `0-0`, `0-0` | no `system-text-menu` in 3/3 | no selection, no menu |

Exact tree files: `kit-A/first-run1/03-lp-first/post-tree.json` through `first-run3`, `kit-A/after-tap-run1/05-lp-after-tap/post-tree.json` through `after-tap-run3`; corresponding B paths are under `kit-B/*/03-lp-first/post-tree.json` and `kit-B/*/05-lp-after-tap/post-tree.json`.

The recipe failures in these cells are measurement failures, not harness crashes:

- A failed the selection-menu assertion with `node \`system-text-menu-copy\` not found`, because the menu present was insertion-kind.
- B failed the menu wait with `wait_until timed out: node \`system-text-menu\` not found`.

## Wrapper device check

`kit-B/menu-dismiss-run1` passed: the plain-field long press produced a `system-text-menu` node, a recipe press elsewhere dismissed it, and the node was absent afterwards. Its recipe asserts the node's existence and disappearance; the final tree has no menu.

## Scroll starting on the TIN at B

| run | selection before scroll | after scroll | after one tap |
|---|---|---|---|
| 1 | `9-9` | `9-9` | `4-4` |
| 2 | `9-9` | `9-9` | `4-4` |
| 3 | `9-9` | `9-9` | `4-4` |

The scroll left the selection unchanged in 3/3 runs. The subsequent tap placed the caret at `selection: 4-4` in 3/3 runs.

## Copy chain at B

Required chain: type TIN → `long_press new-tp-tin` → `selection: 0-9` + copy in menu → `click system-text-menu-copy` → paste into `new-tp-name` → assert value `123456788`.

| run | after TIN long press | copychain-select | copychain-paste |
|---|---|---|---|
| 1 | `selection: 0-0`, no menu | failed at `c1w`: `wait_until timed out: node \`system-text-menu\` not found` | failed at `p1`: `no text menu shows` |
| 2 | `selection: 0-0`, no menu | same | same |
| 3 | `selection: 0-0`, no menu | same | same |

Copy and paste never became reachable because the selection menu never appeared. The paste value assertion was therefore not reached in any run.

## Foreign clipboard refusal at B

`kit-B/clipboard-seed-run1` first put `seedclip` on the clipboard through the app's own text menu. On the next fresh app run, `kit-B/foreign-paste-run2`, a long press on `new-tp-name` showed insertion menu items `paste/autofill(disabled)`, and `click system-text-menu-paste` was refused with:

`the clipboard was not set by this run: Paste would put in what a person or another app copied (cut or copy through the text menu first)`

An earlier unseeded control run had no Paste item at all and failed with `the insertion text menu has no \`paste\` (it has: autofill)`; that run is not used as the foreign-clipboard verdict.

## Recipes

The recipes that drove these runs are committed under `agent-recipes/e2e/`: `otp-prep.json`, `otp-tin-first.json`, `otp-tin-after-tap.json`, `otp-lp-first.json`, `otp-lp-after-tap.json`, `otp-scroll-only.json`, `otp-scroll-tap.json`, `otp-menu-dismiss.json`, `otp-copychain-select.json`, `otp-copychain-paste.json`, `otp-clipboard-seed.json`, and `otp-foreign-paste.json`. `just e2e check otp` validated 14 recipes / 28 files with gpui-agent-cli 0.4.1 and reported 0 problems.

Each run folder contains the runner's own `report.json`, `report.md`, `run.log`, per-recipe `receipt.json`, instantiated recipe copy, screenshots, `recording.mp4`, and the post-recipe gpui-agent tree (`post-tree.json`). `summary.json` is the machine extraction used for the tables above.

## Known gaps and limits

- Asserting a node state directly is not expressible in gpui-agent-cli 0.4.1. Exact refusal from a probe recipe: `error: recipe json: step \`a\` has unknown field \`state\` for op \`assert\``. The selection verdicts therefore come from saved snapshot trees.
- Aiming a long press at the first or last cell inside `new-tp-tin` is still not expressible; only the target centre was tested, as instructed.
- PIN and sign-in-code boxes remain blocked by the suite safety scanner. Exact scanner format: `<file> types into or keys '<target>': never a password, PIN, passcode, secret, token, code box or the software keyboard`.
- Android system text-menu contents are exposed read-only in the tree; enabled in-app items can be clicked by id in this plug. System handles are not tree nodes.
- These runs measure behavior only. They do not patch app runtime code.
