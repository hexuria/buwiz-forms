# Phone rows: after e5336da7 (feat/form-fields-all)

Real macOS windows via `nsh test --target macos`, 1x (non-Retina display). Looks: 320x700 and 390x844, light and dark, plus 1280x900 light.
Items 1-5 mount `FormAnyShot.tsx` (sample mode). Item 6 mounts `ShellDemo.tsx`: the real frame (`shell/Shell.tsx`, the one `App.tsx` mounts, with its Toaster) over the demo taxpayer, start route `form=1706/2025/0`. `App.tsx` itself was not used: under the test host it does not take a start route.
`../after-a04e8f20-1280/` holds the same 1280x900 light scenes on a04e8f20 for the desktop comparison.

## What the pictures show

1. **1702EX item 15** (`*-1-1702ex-part1-13-15-17-*`): at 320 the label is two lines, "Investment Promotion Agency (IPA)/" and "Government Agency (specify) *"; the "*" is no longer alone. Same at 390. Not changed: item 17's date range at 320 is still "From [date] to" with "To [date]" on the next line.
2. **1700 Schedule 1 drawer** (`*-2-1700-sched1-drawer-2-*`): Taxpayer and Spouse are two tick boxes, no "false"; "b. TIN" is one title over one line of four boxes (the last wider). Nothing clipped; all four phone looks.
3. **Long choices** (`*-3-choices-*`, 1702EX 13 in the item 1 files): every option wraps inside its bar at 320 and 390; nothing runs past the right edge (1701A 69 and 75, 1702EX 13, 1702RT 21, 2000OT 12). At 320 the 1701A pictures are all at the page's end, so item 69's title is scrolled off; its options are in view.
4. **Labels beside Edit** (`*-4-edit-*`, 1702RT 17-19 in `*-3-choices-1702rt-21-*`): at 320 the label keeps its line and the button sits under it: 1701MS 7, 8, 9, 10 ("Edit"), 1701MS 28A and 1702RT 17, 18, 19 ("Edit penalties"). 1701MS item 8's label is five lines at 320 (nine before). At 390 the button stays beside the label; item 8's label is five lines there beside the person mark and "Edit".
5. **1700 item 18** (`*-5-1700-18-spouse-refusal-*`): one red sentence, once ("Part II is for a joint return: choose Married at item 15, Yes at item 16 and Joint Filing at item 17 first."), under the grey hint "Needs item 15 = Married, ...". The four boxes are empty and locked, and a "From the tax profile" line shows under them although they hold nothing.
6. **1706 after Check** (`*-6-1706-shell-*`, ShellDemo.tsx): at 320 and 390, light and dark, the "12 items need attention." toast is at the top, right under the "Form 1706" header. The buyer TIN row (7A) is clear with "Please enter the Buyer's TIN." under its boxes (`*-4-aftercheck-row7A-up-*`, and `*-3-*` at 390). At 1280 the toast is at the bottom centre, as on a04e8f20.

## Visual problems seen

- Item 6, phone: while it shows, the top toast covers the part's sticky title ("Part I · Background Information" is not readable) and, in `*-2-aftercheck-*`, the top of the form's title block.
- Item 6, `*-3-aftercheck-row7A-320x700-{light,dark}`: a "From tax profile" hint bubble covers part of the 6A label. It is a hover hint from the real pointer.
- Item 6, 1280: the bottom toast sits over a form row (same on a04e8f20).
- All 320 pictures of items 1-5: the window buttons sit on the sticky part title, as before.
- Item 5: "From the tax profile" under an empty spouse TIN.

## 1280x900 light versus a04e8f20 (pixel compare)

- Identical: all three 1706 pictures, and the first picture of 1700 Schedule 1, 1702RT, 2000OT, 1701MS item 8 and 1701MS Part II.
- Differ only in the scrollbar strip (x 1271-1279, a fading scroll indicator) or the window-button strip (x 9-69, y 13-27): the other scrolled pictures of 1702EX, 1701A, 1702RT, 2000OT, 1701MS.
- Really different, both from the new commits: the 1700 Schedule 1 drawer (tick boxes instead of two "false" boxes; one TIN line of four boxes instead of four stacked boxes) and 1700 item 18 (the refusal sentence once instead of four times).

## Files

### `phone-rows/after-e5336da7/`

- `after-e5336da7-1-1702ex-part1-13-15-17-1-1280x900-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-1-320x700-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-1-320x700-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-1-390x844-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-1-390x844-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-2-1280x900-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-2-320x700-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-2-320x700-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-2-390x844-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-2-390x844-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-3-1280x900-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-3-320x700-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-3-320x700-light.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-3-390x844-dark.png`
- `after-e5336da7-1-1702ex-part1-13-15-17-3-390x844-light.png`
- `after-e5336da7-2-1700-sched1-drawer-1-1280x900-light.png`
- `after-e5336da7-2-1700-sched1-drawer-1-320x700-dark.png`
- `after-e5336da7-2-1700-sched1-drawer-1-320x700-light.png`
- `after-e5336da7-2-1700-sched1-drawer-1-390x844-dark.png`
- `after-e5336da7-2-1700-sched1-drawer-1-390x844-light.png`
- `after-e5336da7-2-1700-sched1-drawer-2-1280x900-light.png`
- `after-e5336da7-2-1700-sched1-drawer-2-320x700-dark.png`
- `after-e5336da7-2-1700-sched1-drawer-2-320x700-light.png`
- `after-e5336da7-2-1700-sched1-drawer-2-390x844-dark.png`
- `after-e5336da7-2-1700-sched1-drawer-2-390x844-light.png`
- `after-e5336da7-3-choices-1701a-69-75-1-1280x900-light.png`
- `after-e5336da7-3-choices-1701a-69-75-1-320x700-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-1-320x700-light.png`
- `after-e5336da7-3-choices-1701a-69-75-1-390x844-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-1-390x844-light.png`
- `after-e5336da7-3-choices-1701a-69-75-2-1280x900-light.png`
- `after-e5336da7-3-choices-1701a-69-75-2-320x700-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-2-320x700-light.png`
- `after-e5336da7-3-choices-1701a-69-75-2-390x844-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-2-390x844-light.png`
- `after-e5336da7-3-choices-1701a-69-75-3-1280x900-light.png`
- `after-e5336da7-3-choices-1701a-69-75-3-320x700-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-3-320x700-light.png`
- `after-e5336da7-3-choices-1701a-69-75-3-390x844-dark.png`
- `after-e5336da7-3-choices-1701a-69-75-3-390x844-light.png`
- `after-e5336da7-3-choices-1702rt-21-1-1280x900-light.png`
- `after-e5336da7-3-choices-1702rt-21-1-320x700-dark.png`
- `after-e5336da7-3-choices-1702rt-21-1-320x700-light.png`
- `after-e5336da7-3-choices-1702rt-21-1-390x844-dark.png`
- `after-e5336da7-3-choices-1702rt-21-1-390x844-light.png`
- `after-e5336da7-3-choices-1702rt-21-2-1280x900-light.png`
- `after-e5336da7-3-choices-1702rt-21-2-320x700-dark.png`
- `after-e5336da7-3-choices-1702rt-21-2-320x700-light.png`
- `after-e5336da7-3-choices-1702rt-21-2-390x844-dark.png`
- `after-e5336da7-3-choices-1702rt-21-2-390x844-light.png`
- `after-e5336da7-3-choices-1702rt-21-3-1280x900-light.png`
- `after-e5336da7-3-choices-1702rt-21-3-320x700-dark.png`
- `after-e5336da7-3-choices-1702rt-21-3-320x700-light.png`
- `after-e5336da7-3-choices-1702rt-21-3-390x844-dark.png`
- `after-e5336da7-3-choices-1702rt-21-3-390x844-light.png`
- `after-e5336da7-3-choices-2000ot-12-1-1280x900-light.png`
- `after-e5336da7-3-choices-2000ot-12-1-320x700-dark.png`
- `after-e5336da7-3-choices-2000ot-12-1-320x700-light.png`
- `after-e5336da7-3-choices-2000ot-12-1-390x844-dark.png`
- `after-e5336da7-3-choices-2000ot-12-1-390x844-light.png`
- `after-e5336da7-3-choices-2000ot-12-2-1280x900-light.png`
- `after-e5336da7-3-choices-2000ot-12-2-320x700-dark.png`
- `after-e5336da7-3-choices-2000ot-12-2-320x700-light.png`
- `after-e5336da7-3-choices-2000ot-12-2-390x844-dark.png`
- `after-e5336da7-3-choices-2000ot-12-2-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-8-1-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-8-1-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-8-1-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-8-1-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-8-1-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-8-2-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-8-2-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-8-2-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-8-2-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-8-2-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-8-3-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-8-3-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-8-3-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-8-3-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-8-3-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-part2-1-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-part2-1-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-1-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-part2-1-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-1-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-part2-2-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-part2-2-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-2-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-part2-2-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-2-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-part2-3-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-part2-3-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-3-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-part2-3-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-3-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-part2-4-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-part2-4-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-4-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-part2-4-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-4-390x844-light.png`
- `after-e5336da7-4-edit-1701ms-part2-5-1280x900-light.png`
- `after-e5336da7-4-edit-1701ms-part2-5-320x700-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-5-320x700-light.png`
- `after-e5336da7-4-edit-1701ms-part2-5-390x844-dark.png`
- `after-e5336da7-4-edit-1701ms-part2-5-390x844-light.png`
- `after-e5336da7-5-1700-18-spouse-refusal-1-1280x900-light.png`
- `after-e5336da7-5-1700-18-spouse-refusal-1-320x700-dark.png`
- `after-e5336da7-5-1700-18-spouse-refusal-1-320x700-light.png`
- `after-e5336da7-5-1700-18-spouse-refusal-1-390x844-dark.png`
- `after-e5336da7-5-1700-18-spouse-refusal-1-390x844-light.png`
- `after-e5336da7-6-1706-shell-1-beforecheck-1280x900-light.png`
- `after-e5336da7-6-1706-shell-1-beforecheck-320x700-dark.png`
- `after-e5336da7-6-1706-shell-1-beforecheck-320x700-light.png`
- `after-e5336da7-6-1706-shell-1-beforecheck-390x844-dark.png`
- `after-e5336da7-6-1706-shell-1-beforecheck-390x844-light.png`
- `after-e5336da7-6-1706-shell-2-aftercheck-1280x900-light.png`
- `after-e5336da7-6-1706-shell-2-aftercheck-320x700-dark.png`
- `after-e5336da7-6-1706-shell-2-aftercheck-320x700-light.png`
- `after-e5336da7-6-1706-shell-2-aftercheck-390x844-dark.png`
- `after-e5336da7-6-1706-shell-2-aftercheck-390x844-light.png`
- `after-e5336da7-6-1706-shell-3-aftercheck-row7A-1280x900-light.png`
- `after-e5336da7-6-1706-shell-3-aftercheck-row7A-320x700-dark.png`
- `after-e5336da7-6-1706-shell-3-aftercheck-row7A-320x700-light.png`
- `after-e5336da7-6-1706-shell-3-aftercheck-row7A-390x844-dark.png`
- `after-e5336da7-6-1706-shell-3-aftercheck-row7A-390x844-light.png`
- `after-e5336da7-6-1706-shell-4-aftercheck-row7A-up-1280x900-light.png`
- `after-e5336da7-6-1706-shell-4-aftercheck-row7A-up-320x700-dark.png`
- `after-e5336da7-6-1706-shell-4-aftercheck-row7A-up-320x700-light.png`
- `after-e5336da7-6-1706-shell-4-aftercheck-row7A-up-390x844-dark.png`
- `after-e5336da7-6-1706-shell-4-aftercheck-row7A-up-390x844-light.png`

### `phone-rows/after-a04e8f20-1280/`

- `after-a04e8f20-1-1702ex-part1-13-15-17-1-1280x900-light.png`
- `after-a04e8f20-1-1702ex-part1-13-15-17-2-1280x900-light.png`
- `after-a04e8f20-1-1702ex-part1-13-15-17-3-1280x900-light.png`
- `after-a04e8f20-2-1700-sched1-drawer-1-1280x900-light.png`
- `after-a04e8f20-2-1700-sched1-drawer-2-1280x900-light.png`
- `after-a04e8f20-3-choices-1701a-69-75-1-1280x900-light.png`
- `after-a04e8f20-3-choices-1701a-69-75-2-1280x900-light.png`
- `after-a04e8f20-3-choices-1701a-69-75-3-1280x900-light.png`
- `after-a04e8f20-3-choices-1702rt-21-1-1280x900-light.png`
- `after-a04e8f20-3-choices-1702rt-21-2-1280x900-light.png`
- `after-a04e8f20-3-choices-1702rt-21-3-1280x900-light.png`
- `after-a04e8f20-3-choices-2000ot-12-1-1280x900-light.png`
- `after-a04e8f20-3-choices-2000ot-12-2-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-8-1-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-8-2-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-8-3-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-part2-1-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-part2-2-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-part2-3-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-part2-4-1280x900-light.png`
- `after-a04e8f20-4-edit-1701ms-part2-5-1280x900-light.png`
- `after-a04e8f20-5-1700-18-spouse-refusal-1-1280x900-light.png`
- `after-a04e8f20-6-1706-shell-1-beforecheck-1280x900-light.png`
- `after-a04e8f20-6-1706-shell-2-aftercheck-1280x900-light.png`
- `after-a04e8f20-6-1706-shell-3-aftercheck-row7A-1280x900-light.png`

