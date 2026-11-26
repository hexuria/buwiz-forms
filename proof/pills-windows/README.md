# Pill click check on Windows — hexuria/buwiz-forms d4f2852a

Same build as the #403 proof: app `d4f2852a28c323e3f6da85eae4f61d970d84ff0e`, nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, nsh CLI `0.6.1 (8c78cfd)`, cargo 1.99.0, build folder `C:\ci\check403\buwiz-forms\target-check403`. Machine: Windows Server 2022, 1600x1200, 96 DPI.

All test clicks are real OS input (pyautogui). gpui-agent used only for navigation/setup, agent-press comparison tests, and reading back state.

## Files

- `click-log.txt` — every click with its exact screen point, the before/after checked state of every pill, and the step-4 diagnostics (cursor position and WindowFromPoint per point).
- `shot-00b-pills-hover-crop.png` — magnified crop of the Amended Yes/No pills while the cursor hovers on "Yes": no hover fill is visible.
- `shot-01..07` — hover shot, post-click states, the quarter-pills test end state, contrast clicks.
- `recording-pills-4x.mp4` — screen recording sped up 4x. Caveat: gdigrab on this VM is capture-bound (~0.5x realtime) so the video is a slowed-down partial capture of the session, not a frame-complete record; the click log is the authoritative evidence.

## What the files do not show

- The contrast textbox click line is missing from the log section (the box was scrolled out of view at that moment); the navigator contrast click is logged, and the entire #403 proof is built on real clicks landing in text boxes.
- The video does not cover every click (see caveat); nothing in the proof depends solely on the video.
