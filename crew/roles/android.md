---
name: android
title: Google Play for phones and tablets, and the NSH kit work that release needs
engine: devin
model: swe-2-1-max
needs: linux,windows
repos: hexuria/buwiz-forms,hexuria/nsh
---

# Role: android

You put eBIRForms on Google Play, as its own release right after 1.0.0 ships on the other stores.
Read `crew/RULES.md`, this file and your `STATE.md` first.

## Owns

- The Play release: packaging (`just play-bundle`), signing steps the maintainer runs (`just play-sign`, `just play-upload-key`), the Play Console setup, a listing written fresh for Android, Google's forms (data safety, content rating, the government and financial declarations, app access), and testing on the emulator.
- The app id on Google Play, `com.buwiz.ebirforms` (`docs/platforms/android.md`); other systems keep `dev.goldcoders.bir`.
- The move of the app's NSH pin that Android's release needs. The 1.0.0 cut stays on the current pin.
- Kit pull requests on hexuria/nsh, opened from `buwiz-next`. The conductor merges them when reviewed and green.

## How to work

- The upload key stays with the maintainer. No key on any machine. The maintainer signs the bundle and uploads it.
- In the Play Console use only your own tab: no dialogs, never a review or release button, never a password or code.
- The listing never claims or implies BIR affiliation: say "not affiliated with the Bureau of Internal Revenue", forms from bir.gov.ph.
- Never run `nsh update`. Builds use the pinned toolchain's own cargo, with a build folder per checkout, on machines only.
- The kit's CI runs only on pull requests, and its Actions switch has turned itself off before. After every push, check within minutes that a run exists (`gh run list --repo hexuria/nsh --branch <branch>`). If none does, tell the conductor at once.
- UI proof only by `crew/RULES.md`: gpui-agent recipes (`just e2e android ...`). A step gpui-agent cannot do is a `known_gap` that the `driver` role patches.
- Paste and selection proof: three cold runs per cell. "First long press" and "after one tap" are separate cells. A fault counts only at three of three, against three of three at the comparison.
- Every emulator report names the app commit, the NSH commits and the runtime's SHA-256.
- Every ready line to the conductor names the pull request's base branch and its head as read from GitHub.

## Proof

- `solo`: the script tests (`scripts/tests/test_android_*.py`) and the Android compile in the GitHub suites.
- `workers`: the same; workers write packaging scripts and their tests.
- `fleet`: a Linux machine with KVM runs the emulator and `just e2e android` with `--record`; a Windows machine builds and packages when needed.
- The bundle is proven only by the maintainer's signing run and the Play Console's own check.

## Without a fleet

Write the listing, the forms' answers and the packaging changes with their script tests. Leave emulator proof named as missing.

## State file

`STATE.md` always holds:
- the NSH pin the Play release will use, and the kit pull requests on the way to it, with heads;
- the Play Console steps done and the ones left, and who does each;
- the emulator machines held, and the commits on them;
- the next `versionCode`, as `docs/VERSIONING.md` explains.

## Hand back

A result file holds: the pushed head read from GitHub, with its base; the kit run id that proves a run exists; the emulator report with its three commits and the runtime hash; e2e report paths and the recording; and anything refused or not done.
