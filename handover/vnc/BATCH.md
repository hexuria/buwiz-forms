# Render-check batch (display :98 only)
Tools: .../scratchpad/vnc/xdo98 <xdotool args>; .../scratchpad/vnc/shot98 <out.png> [x,y,w,h]. Never use xdo/shot without "98".
Display :98 = 1440x900, no window manager, screenshot px == xdotool coords. Window id: `xdo98 search --name e-BIRForms | head -1`.
Read NAV.md ("Display :98 setup" section) for profiles, which year holds which form, library positions, and gotchas.
For each form in your batch:
1. Open its draft from the Tax Form Library (right profile + tax year; monthly → Dec, quarterly → Q4, annual → its card / "Open certification draft"). Do not click Submit, Generate XML, Manual/External Filing, Revert, Discard.
2. Screenshot at 1280x860, 900x860 and 390x844 (`xdo98 windowmove W 0 0 windowsize W <w> <h>`, wait ~1.5 s). At 390 scroll inside the window in 5-notch steps (mousemove into the form, `click 5` x5) and shoot after each step until the footer is reached (cover the whole form). At 1280 also take 2 more shots after 6-notch steps.
3. Judge: (a) content fits the width, nothing cut off at the right edge or overlapping; (b) at 390 label/input pairs stack; (c) anything unreadable/overlapping. Note the status stepper wraps (expected).
4. Restore 1280x860 and go Back to the library before the next form.
Save shots under .../scratchpad/vnc/render/<FORM>_<width>[_sN].png. Report a table: form | width | fits | stacks@390 | problems (file-agnostic, concrete: which section/field) | shot paths; then a list of defects worst-first. Don't edit any files except your screenshots; don't start agents.
