driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent 4839f70e9e700835cd93bd579249d524d9ba5e2d

# Proof for hexuria/nsh-gpui-agent#26 review fixes (T1, T2, the open verbs)

App: hexuria/buwiz-forms pull request #497's head f030fbfd63a47cf2b322ffec6598122ad9fd6936 (pins the plug at
db0e4ac6127078a05751149f0ec6bed68204f911). The plug's pull request is hexuria/nsh-gpui-agent#26, branch
fix/type-focus-proof, head 4839f70e9e700835cd93bd579249d524d9ba5e2d (the reviewed 3e78d303 plus a merge of the
plug's main 4a52dd91 and the review fixes). Every run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1
BIR_SFTP_DRY_RUN=1, never BIR_SFTP_LIVE, a fresh scratch database, the DEV test build only, gpui-agent recipes
only, Queue/Send never pressed.

## How the app's plug was pointed at the fix

`nsh/Cargo.toml` carried a LOCAL, uncommitted override - the five plug git deps' `rev` set to the sha under test
instead of the pinned db0e4ac6 - never committed. (`[patch]` does not reach the generated .nsh/rust workspace; the
sibling ../nsh is a real copy at the NSH pin, not a symlink.) The override reached the build: the after-build's
macOS host binary carries the string "the focus: node" (the new error's wording), absent at the pin.

## Probes

- `sign-up-bare.wants` / `p26bare.json`: the smoke suite's sign-up with the `click new-tp-name` workaround line
  REMOVED - a bare type right after the segmented TIN control, the exact failing pattern of plug issue #14/#26
  (from try/plug-14-macos-type-focus, unchanged).
- `sign-up-keyclear.wants` / `keyclear.json`: types the TIN boxes, types a wrong name, then `key new-tp-name mod+a`
  (a key with a target right after the TIN boxes) and `set_value` (whose clear is select-all + Backspace inside the
  same press-then-keys ordering) to replace it - exercises the key_node and set_value ordering fixes.
- `p26signup.json`: the smoke suite's OWN sign-up (unchanged recipe files) plus its setup-start prerequisite.

## macOS desktop (per-recipe statuses read from each report.json)

| run | plug | result | evidence |
|---|---|---|---|
| before: bare probe | db0e4ac6 (the pin) | sign-up FAIL s20: `context-pill` expected "Tax profile: E2E SMOKE TAXPAYER", got "Tax profile: 1 TEST ST" - the name never landed | macos/macos-20261127-162016/, macos-before-failure.png |
| after: smoke sign-up x5 | 4839f70e | pass 5 of 5 | macos/macos-20261127-162330/ ... -162442/ |
| after: bare probe x3 | 4839f70e | pass 3 of 3 (plus two more passes in the video runs) | macos/macos-20261127-162109/, -162502/, -162518/ |
| after: keyclear | 4839f70e | pass: wrong name selected-all and retyped correctly | macos/macos-20261127-162532/, macos/keyclear-shots/ |

macOS video: macos/sign-up-bare.mp4 - a screen recording of one bare-probe run after the fix (2 of 2 pass,
evidence in macos/mp4run/). The runner records video only on Simulator/emulator - desktops have no recorder in
scripts/e2e/runner.py - so the desktop mp4 is a screen capture of the same recipe run.

## iPhone Simulator (iPhone 17, iOS 27)

| run | plug | result | evidence |
|---|---|---|---|
| before | 3e78d303 (#26's reviewed head, WITHOUT the T1 fix) | sign-up FAIL step 6: `a press at (185, 399) did not give node 3 the focus` - T1's exact prediction: the wrapper dropped focused(), so the type check reads None forever | iphone/before.report.json, iphone/before-failure.png, iphone/sign-up-before.mp4 |
| after | 4839f70e | sign-up pass 28.9s (2 of 2) | iphone/after-run/, iphone/sign-up-after.mp4 |
| control | 4a52dd91 (plug main, no #26) | sign-up pass 22.7s - main is not regressed; the merge content is iOS-clean | iphone/main-run/ |

WARNING for whoever re-runs this (an infrastructure finding, not the plug): the iOS token hand-off
(nsh/agent/ios-handoff.sh) writes the run's GPUI_AGENT_TOKEN into the simulator container only if no token file
exists. A finished run leaves its token file behind, so every later run on the same simulator reads the STALE
token: the plane answers the challenge and then rejects the handshake ("invalid automation token"), and the runner
reports "the window's control plane never answered" forever. Three proof attempts were lost to this before it was
diagnosed; deleting Library/Caches/gpui-agent-token in the app container before a run is the workaround.
