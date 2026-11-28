# Kit issue drafts

`hexuria/nsh` has issues disabled, so these could not be filed with `gh issue create --repo hexuria/nsh`.

## Draft 1: Android adjustResize counts the keyboard inset twice

A bottom sheet whose field opens the keyboard is lifted one keyboard-height too high.

The Android activity uses `adjustResize`. The native content view is already shortened to end at the keyboard's top, but `NshView.publishEnvironment` still reports the raw IME bottom inset as though the content occupied the full window height. Kit bottom panels then apply `keyboard_lift()` and move up by the same keyboard height a second time.

Exact proposed change: `kit-410-adjustresize.patch` in this folder. It detects `SOFT_INPUT_ADJUST_RESIZE` in `NshView.java` and reports an IME overlap of zero for the already-shortened content view.

## Draft 2: Android input composition can be replayed through a fresh InputConnection

Going Home and returning while an amount input is focused can corrupt the amount, including shifting its decimal point one place.

`InputBox.KeyConnection` infers synthetic keys by comparing newly composing or committed text with its cached `composing` field. After the input method restarts, that field is empty even though the text itself still carries a composing span; replayed composition can therefore be mistaken for new user keys.

Exact proposed change: `kit-420-input-composition.patch` in this folder. It seeds composition comparison from the actual composing span in the editable text before falling back to the connection's cached value.
