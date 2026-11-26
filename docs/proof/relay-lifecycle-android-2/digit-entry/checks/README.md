# Digit-entry checks — verdict: the failures were MISSED TAPS, not an input fault

Free disk and context in the session report. Everything here is on the e5f7a504 debug APK,
emulator-5554 (API 35), no reboot during these checks, outside network cut, mocks over adb reverse.

## Verdict first

**The digit boxes work. Every pre-boot and cold-boot failure was caused by taps at y=763, about
50 px BELOW the boxes.** The boxes' real bounds are [146,590][934,716] (the backing EditText,
from the uiautomator dump). y=763 is in the gap above "Resend". The first tap at a box centre,
(204,652), immediately produced the blue focus ring, a caret in box 1, and switched Gboard
from full QWERTY to the number pad. `input text` then filled the boxes in order (screenshot:
digits 2 4 6 8 1 in boxes 1-5, caret in box 6), KEYCODE_DEL deleted the last digit (screenshot:
boxes 2 4 6 8, caret in box 5), and the correct mock code (typed as digits) submitted and the
sign-in completed. The "POSSIBLE V1 BLOCKER" is withdrawn as an app fault: digit entry on
Android works when the touch reaches the box.

## 1. Exact tap coordinates used in every earlier run

Pre-boot runs on e5f7a504 (all BEFORE any long press): blind keyevents with no tap; then taps at
(180,763), (200,763), (300,763), (540,763) — all y=763, the empty TEXT nodes' uiautomator bounds
centre (bounds [*,737][*,789]), which is BELOW the visible boxes. Cold-boot cb02 run: same y=763
taps, same wrong target. The one correct-height evidence I had was misread: the hidden EditText's
own bounds ([146,590][934,716]) were in the dump all along.

## 2. uiautomator dump of the code page (`codepage-uiautomator.xml`)

One EditText, the backing input for all six boxes:
class=android.widget.EditText, focusable=true, clickable=true, enabled=true, long-clickable=true,
focused=false (before the tap), bounds=[146,590][934,716] — the whole strip. Box centres are
y=652, x≈204/335/466/613/744/876.

Overlay check (c): after the EditText in dump order come 8 ViewGroups covering exactly the strip —
one per visual box ([146,590][262,716], [278,590][393,716], [409,590][524,716], [556,590][671,716],
[687,590][803,716], [818,590][934,716]) plus two grouping containers. They draw the box decoration
on top of the input. They do NOT block the touch: the tap at (204,652) — inside box 1's ViewGroup —
focused the EditText (evidence below). Touches pass through the overlays or the overlay forwards them.

(a) window focus before the tap: `before-windowfocus.txt` — the app's window focused normally.
(b) EditText attrs: `before-edittext-attrs.txt`.

## 3. Clean arrival at the code page, NO tap at all (the added record)

`clean-arrival.png` + `clean-arrival-inputmethod.txt` + `clean-arrival-windowfocus.txt`:
no caret in box 1, no focus ring, and the keyboard up is the EMAIL keyboard — Gboard's full
QWERTY with number row. mInputShown=true, mServedView=com.longbridge.nsh.NshView (the whole
custom view). This confirms on Android exactly what the app's code predicts: **the code page never
focuses the boxes by itself.** The user must tap a box; only then does focus move and the number
pad appear.

## 4. Tap at the box centre (204,652)

`tap-center-focused.png` + `tap-center-inputmethod.txt`: blue focus ring + caret in box 1,
keyboard switched to the number pad. inputType 0xc001 (number class) appears in the served editor
info — the OTP field's own input type, absent before the tap.

## 5. Digits and Backspace

- `five-digits.png`: after `input text 24681` — boxes show 2 4 6 8 1 in order, caret in box 6.
- `after-del.png`: after KEYCODE_DEL — the 1 is gone, boxes show 2 4 6 8, caret in box 5. Backspace works.
- Sign-in completion: typing the correct mock code submitted automatically on the 6th digit; the
  app passed the OTP gate (evidence: the session then opened — see the session report; an earlier
  wrong-code submission rendered "That code is not right. 4 tries left." in `tap204-typed.png`,
  which also proves submission fires).

## 6. What this means for the earlier finding

The pre-boot `mFallbackInputConnection`/BaseInputConnection observation is real but it is the
state of the page when NO box is focused: keystrokes then go to the whole-view fallback and are
discarded. It was never evidence of a broken input connection for the boxes — once a box is
focused, inputType 0xc001 is served and text lands. The money-box symptoms ("." dropped, Backspace
dead) should be re-tested with taps verified inside the boxes before treating them as an app fault.
The one REAL app-side issue this evidence supports: the code page does not focus the first box on
arrival (section 3), which on a phone means one extra tap — a UX finding, not a blocker.

Timing note (ordered): the boxes first worked at 06:22 device time, emulator uptime ~43 min,
the moment the first correct-centre tap landed.
