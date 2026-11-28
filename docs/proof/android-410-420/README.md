driver: gpui-agent-cli 0.4.1
recipes: agent-recipes/e2e/android-410.json, agent-recipes/e2e/android-420.json

# Android #410/#420 proof

App checkout: `hexuria/buwiz-forms` main `58621b4fe160aa8b4b47392ae5ee800bcfeca937` plus this branch's fix.
Kit checkout: `hexuria/nsh` pinned at `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, unedited.

## Kit findings

- `kit-410-adjustresize.patch`: exact proposed kit change for the bottom-sheet keyboard inset being applied once by Android `adjustResize` and again by kit `keyboard_lift()`.
- `kit-420-input-composition.patch`: exact proposed kit change for input composition replay through a fresh Android `InputConnection`.
- `kit-issue-drafts.md`: issue text for both kit changes. `hexuria/nsh` has GitHub issues disabled, so they could not be filed.

## Evidence

Pending final device evidence; this README is completed with before/after folders, screenshots, recordings, test counts, and the final heads before push.
