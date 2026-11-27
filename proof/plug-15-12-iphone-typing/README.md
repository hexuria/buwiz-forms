driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent 5f85663f15c2aa012971b6372610d23ed6bc4e89

# Proof: nsh-gpui-agent occlusion scroll-retry on the iPhone Simulator

App: hexuria/buwiz-forms main 58621b4f (plug pinned at b559e40d), the AFTER build with a
local uncommitted path override of the plug crates to the branch of hexuria/nsh-gpui-agent#28
(head 5f85663f15c2aa012971b6372610d23ed6bc4e89). iPhone 17 Simulator, iOS 27.0, debug build
with the agent feature, every run with SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_EBIR_TEST_ENV=1
SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1, loopback and mocks only.

## Layout

- `before/` — `just e2e ios smoke` (report.md, report.json, per-recipe evidence) with the plug at the pin b559e40d.
- `after/` — the same suite with the plug at 5f85663 (the scroll-retry fix). Same summary line,
  different composition: money-boxes now passes step s1 (the amended-return click the pager pill
  occluded) and reaches the typing steps.
- `probe/` — the `mbprobe` recipe (agent-recipes/e2e/mbprobe-phone.json, the owner's probe) added
  after money-boxes in a copy of the smoke suite: it scrolls the form, expands part 2 and types
  paced digits into the money boxes. `probe/07-mbprobe/recording.mp4` is the recording of the
  money box being typed.

## Results

| recipe | before | after |
|---|---|---|
| setup-start | pass | pass |
| sign-up | pass | pass |
| tax-year | pass | pass |
| navigate | pass | pass |
| open-2551q | pass | pass |
| money-boxes | known gap, s1 `occluded_by: 5040` (pager pill over the radio) | known gap, s2 `target_gone: txt16` — the click now lands; the failure moved past the occlusion |
| sheet-press | skipped (no phone variant) | skipped |
| delete-row | setup ($table no node matches) | setup |
| queue | fail s5 `occluded_by: 537` | fail s5 `occluded_by: 537` — a real cover, see below |
| birth-date | known gap (date sheet) | known gap |
| sign-out | known gap (sheet left open) | known gap |

## What the evidence shows

1. **The occlusion fix works on device.** Before: the money-boxes click on the amended-return
   radio was refused `occluded_by` — the floating pager pill covers the radio's middle, and
   scroll-into-view stopped at "technically inside the window". After: the agent scrolls the
   target towards the window's middle and the press lands; the recipe advances.
2. **hexuria/nsh-gpui-agent#15 is real and not fixed.** The probe types paced `500.25` into
   Item 16 with the click landing correctly and the readback is `.52005` (probe receipt s9) —
   the digits all arrive but at scrambled caret positions. This persists after buwiz-forms#403,
   so the cause is not the reformatting the issue first assumed: it is the injection racing the
   box's caret management. The software keyboard is not in the semantic tree on iOS (no keyboard
   nodes in the failure tree), so the person-like path the issue asks for does not exist yet.
3. **queue's `occluded_by: 537` is a correct refusal.** The occluder is a day button of the
   business-start-date sheet, which the phone editor opens over the field the recipe aims at;
   the press through it would be wrong, and no scroll moves a sheet away. The recipe's flow
   fits the desktop form factor; on the phone the date is set in the sheet's own input.
4. **#12's TIN/code occlusion does not reproduce at the pin.** sign-up types the TIN through
   the OtpInput cells and passes in both runs (the overlay-pass in the pinned plug covers
   type/set_value). `click` on such a box, the issue's still-open item, is not exercised by
   any smoke recipe.
