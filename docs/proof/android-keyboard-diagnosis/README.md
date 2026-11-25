# Job 11 — keyboard padding diagnosis (NSHKBD instrumentation)

App build: `fe1a5f46` (the job-10 #345 build), running against an NSH runtime built
from the pin `8c78cfd6` with three added log lines, tag `NSHKBD`, `android.util.Log.i`.
The patch is local to this machine only — never committed or pushed — and is NOT in
this proof. The three lines, described:

1. `Mounter.inputFocused(InputBox input, boolean focused)`, first line:
   `inputFocused input=<input.id> focused=<focused> scroll=<id of scrollAround(input), or "none">`
2. `Mounter.keyboard(int screenTop)`, first line:
   `keyboard screenTop=<screenTop> focused=<this.focused.id or "null"> scroll=<id of scrollAround(focused), or "none"/"n/a"> avoided=<avoided>`
3. `ScrollBox.setContentPaddingBottom(int px)`, first line:
   `setContentPaddingBottom scroll=<this.id> px=<px>`

How the app was pointed at the instrumented NSH: the patched checkout is
`~/repos/nsh-pin` at `8c78cfd6`; its java runtime was rebuilt with
`cargo xtask build-android --abi x86_64` inside the checkout (target symlinked to the
shared target dir), and the resulting `classes.dex` (Java-only, profile-independent)
was copied into both `runtime/android/dev` and `runtime/android/release` of the pin CLI
distribution `~/BuwizNsh/tools-8c78cfd-bak`. The app was packaged with that CLI
(`nsh package --target android --abi x86_64`, job-10 env line). Note: `nsh package`
uses the RELEASE runtime even for dev-env builds — the first instrumented package
silently had no NSHKBD lines until the release dex was replaced; verified in the APK
with `strings classes.dex | grep -c NSHKBD` = 1 before installing. Phone, API 35,
keyboard docked; `adb logcat -c` before each case.

## Case 1 — New taxpayer, tap Registered name, then Phone (case1-newtaxpayer.logcat)

```
inputFocused input=258 focused=true scroll=68
keyboard screenTop=1517 focused=258 scroll=68 avoided=-1
setContentPaddingBottom scroll=68 px=883
inputFocused input=258 focused=false scroll=68
inputFocused input=378 focused=true scroll=68
```

Classification: **none of the three expected failure shapes.** Focus is registered
before the keyboard arrives, `scrollAround` finds the page's ScrollBox (68), and the
padding IS set (px=883, the full keyboard height). The blur+refocus on the tap is a
clean transfer to the Phone input (378), not a spurious refocus of the same input.
Observed visually: with the padding applied, swipes scrolled the page far enough that
Phone rose to y=824–934 and was tappable — in this build the New taxpayer page scrolls
correctly. (In job 6 at `b51fabe4` it did not.)

## Case 2 — Choose forms, search field, type 2551Q (case2-chooseforms.logcat)

```
inputFocused input=2548 focused=true scroll=2214
keyboard screenTop=1517 focused=2548 scroll=2214 avoided=-1
setContentPaddingBottom scroll=2214 px=605
```

Same healthy shape: focus registered, correct ScrollBox, padding set (605 px — the
part of the scroll under the keyboard; the scroll does not extend to the screen bottom).

**However — the one stuck state observed, and it is the answer shape (d) "something else":**
after the app was backgrounded and foregrounded around other apps with their own
keyboards, the keyboard-close event reset the padding (`setContentPaddingBottom
scroll=2214 px=0`), the search text kept its value and the field still accepted typed
characters (so the input was functionally focused), but no `inputFocused` and no
`keyboard()` line was logged again — the padding stayed 0 while the keyboard was up,
and in that state the 2551Q result row at y=1720–1771 (keyboard top 1517) did not move
at all under repeated swipes (bounds byte-identical, see README discussion). Tapping
the field again produced no `inputFocused` line — Mounter still believed it focused.
Closing the keyboard with the chevron and reopening it re-fired
`keyboard screenTop=1517 focused=2548 scroll=2214` and re-applied the padding
(case2c-reopen.logcat), after which the SAME swipe scrolled the row to y=508–619,
fully above the keyboard.

A clean background/foreground cycle alone does NOT reproduce the stuck state
(case2d-bgfg.logcat): on backgrounding the padding resets to 0 and on return the
keyboard-open event fires and the padding is re-applied (px=605), and scrolling works.
The stuck state involved the keyboard staying open across an app switch to another
keyboard-using app (the kit host); the exact minimal sequence is not yet pinned down,
but the invariant violation is clear: **padding can be 0 while the keyboard is up and
an input is focused, because the padding is only (re)applied on inset-change events,
and no event fires when the ime-visible state never changed from the view's perspective
— while the close that zeroed the padding did arrive.**

Blur/refocus note: no inputFocused(false)→inputFocused(true) pair for the SAME input
was observed around keyboard arrival in any case.

## Case 3 — kit test app, tap Field C (case3-kit.logcat)

```
inputFocused input=20 focused=true scroll=8
keyboard screenTop=1517 focused=20 scroll=8 avoided=-1
setContentPaddingBottom scroll=8 px=648
```

Identical healthy shape — the comparison case matches the app cases exactly.

## Bottom line

At `fe1a5f46` the NSH keyboard mechanism fires correctly in the app for all three
cases — focus is registered, `scrollAround` finds the right ScrollBox, and the padding
is applied — and ordinary scrolling then works. The job-6 hard fault does not reproduce
in this build. The remaining defect is intermittent state loss: the padding is reset
to 0 on keyboard close but is not reliably re-applied when the keyboard is already up
(or reappears) without an inset-change event reaching the view — the scroll then has
zero padding with the keyboard open and nothing scrolls. `keyboard()` applying padding
is purely event-driven; there is no re-application when a view/scroll is remounted or
when the app returns to a keyboard that never closed from its perspective.
