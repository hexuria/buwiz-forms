driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent df2ee9791ad019ef6824e798bd6b1be517b48678
recipes: agent-recipes/e2e/android-410.json, agent-recipes/e2e/android-420.json

# Android #410/#420 proof

App branch: `fix/android-410-420`.
Kit pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; neither kit PR is in that pin.

## Current truth

- **#420: this branch does nothing for it — the fix is hexuria/nsh#58.** On a decimal pad the input method composes
  nothing (the probe showed zero composition events), so the earlier end-composition machinery was dead code and is
  removed. #420's real cause is the input method replaying its cached composition into the fresh connection after a
  background restart, and that is the kit fix. Not reproduced in the real app: the test driver cannot type through the
  input method (the driver extension is hexuria/buwiz-forms#646).
- **#410**: the sheet lift is the kit fix hexuria/nsh#57. This branch carries the keyboard-aware sheet maximum height
  (`sheetCap` in `nsh/ui/layers.tsx`): the Android keyboard inset already includes the navigation bar it covers, and
  counting the bottom safe area off it again left a tall sheet one navigation-bar too tall — under the status bar with
  3-button navigation.
- `android-420`'s Home steps are `system background`/`system foreground`, gated on the host listing `system.background`:
  a host without the op (every pinned host today) skips the recipe with a plain line instead of failing unsupported_op.

## Emulator evidence

#410 before/after is on branch `try/android-410-proof` (screenshots both navigation modes, one mp4): before, the RDO
sheet floated a keyboard-height too high; after (this branch's base with the kit built from hexuria/nsh#57), the sheet
sits on the keyboard in 3-button and gesture navigation. #420 has no end-to-end proof: the driver cannot type through
the input method, so the replay cannot be reproduced (hexuria/buwiz-forms#646).
