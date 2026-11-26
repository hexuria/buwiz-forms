# Windows focus/selection fault at 2bc1e89d: traces, cause, and a proven local fix

Probe machine: buwiz-forms-win-3 (Windows Server 2022), real OS key events, 2551Q Schedule 1 row 1
taxable amount holding 1,234.56, one key `9` after each entry. MFTRACE lines are millisecond-timestamped
events logged inside MaskedField (nsh/ui/fields.tsx) by a local uncommitted instrumentation.

## Files
- traceA-tab.txt .. traceE-click-left.txt: the five entries ordered, exactly as logged
- traceX-click-clean-state.txt: a click with pointerDown false at focus (shows the pressdown ordering)
- fields.tsx.diff: the tested fix against 2bc1e89d, WITHOUT the logging

## The cause (three compounding faults)
1. `byFocus` is dead on Windows for real keyboard entry: it is set only inside the wrapped programmatic
   focus() (handle().focus()). A real Tab/Shift+Tab never passes through it, so selectWhole is false at
   every keyboard entry.
2. onPressDown arrives AFTER onFocus on Windows (same ms or +7ms), never before; pointerDown is reset only
   in onFocus, never in onBlur, so the late pressdown poisons the NEXT entry: every later entry (even pure
   keyboard) is misread as a pointer entry.
3. What destroys Windows' native select-all on Tab: the pre-focus caret is in the GROUPED text's coordinates;
   with stale pointerDown it is mapped through caretAfterSameDigits to focusCaret 7; the timeout takes the
   caret branch writeCaret(7,7) and wipes the selection. On clicks the click's caret either never arrives or
   arrives in grouped coordinates and is mis-mapped (trace D: 6 -> 5, a mid-decimal splice).

## The fix (fields.tsx.diff), tested on this machine
(a) a filled amount is never rewritten on focus: the text stays exactly as laid out, separators included;
    the platform's own focus behaviour stands (keyboard focus selects all natively; a click keeps whatever
    the host gives it). Zero amounts keep the existing rewrite-to-empty path.
(b) the focused pass-through test compares significant characters, not the raw text, so separators already
    in the text neither reject the edit nor count against it.

## Results with the fix (real keys, saved values from the window state)
- T1 Tab into 1,234.56, type 15000.50: the amount is selected whole ON ENTRY, typing replaces, after leaving 15,000.50. RIGHT
- T2 Shift+Tab: same. RIGHT
- T4 Tab into 12,345,678.90, type 15000.50: RIGHT (15,000.50)
- (a) click 0.00, type 15000.50: RIGHT (15,000.50)
- zero box 99999.99: RIGHT (99,999.99)
- zero box 1000000.005: RIGHT (1,000,000.01, half up)
- zero box 1234567890 Left Left Backspace: RIGHT (123,456,790.00)
- Clicks into a filled box: STILL SELECT ALL (no caret placement). The click caret event never reaches JS:
  the GPUI host pointer-down handler does move_to(hit(x,y)), but no SelectionChange for it is ever delivered
  (see traceX and the second-click probe: pressdown arrives, no selection event follows). This is kit-level
  and needs the author's look; the failure mode is now safe (typing replaces the visible selection rather than
  silently splicing digits into the amount).
