driver: gpui-agent-cli 0.4.1
recipes: agent-recipes/e2e/android-410.json, agent-recipes/e2e/android-420.json

# Android #410/#420 proof

App branch: `fix/android-410-420`.
Kit pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; neither kit PR is in that pin.

## Current truth

- The app-side #420 change is in `nsh/ui/fields.tsx` and covers every `MaskedField` (money, TIN, percent, integer and date masks), because they all share that component.
- On Android `background` only, a live IME composition is ended without running the semantic leave: no validation, reformat, rollback or `onBlur`. The typed committed text is kept exactly as it is. `inactive` does nothing.
- Focus is restored when the app returns to `active`; the keyboard reopens with that focus. A later real leave still settles the field.
- The TIN is covered by a headless lifecycle test as the non-amount masked field.
- #410 and the replay half of #420 are kit changes, now in hexuria/nsh draft PRs #57 and #58. They are based on `buwiz-next`, 15 commits past the app pin, so they do not reach the app until the pin moves.

## Emulator evidence

Not yet run. The Linux proof must run `python3 scripts/e2e check android-410`, `python3 scripts/e2e check android-420`, then `python3 scripts/e2e android android-410 --out <out410>` and `python3 scripts/e2e android android-420 --out <out420>` before/after the kit branches, with gpui-agent screenshots and one mp4. `android-420` currently has a known gap: gpui-agent-cli 0.4.1 has no composition op, so the recipe cannot truly create a live IME composition. It must not fall back to adb or OS input.
