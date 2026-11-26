# Android sign-in code boxes input check

Commit under test: `8a507e47d96e5799c160fe6dbabcd7077e72cdec` on `main`.
Emulator: Android 15, API 35, `sdk_gphone64_x86_64`, system image `system-images/android-35/google_apis/x86_64/`, ABI `x86_64`.
Mock account server only; invented addresses `probe.one@example.test` and `probe.two@example.test`.

## Tap coordinates and bounds

The code-box tap was the same before and after the cold boot:

```sh
adb shell input tap 540 650
```

That point is inside the native field's bounds `[146,590][934,716]` in both uiautomator dumps. It is near the middle of the six-cell group, not the centre of cell 1 (about `204,652`), but it is unambiguously inside the transparent input field.

Other tap coordinates:

- Pre-cold, while the alphanumeric keyboard was visible: digit 1 at `(58,1572)`, digit 2 at `(160,1572)`. These same coordinates were tried both before and after the `(540,650)` box tap.
- Post-cold, before tapping the boxes: the same initial keyboard digit coordinates `(58,1572)` and `(160,1572)`.
- Post-cold, after the boxes were focused and the numeric keypad was visible: digit 1 at `(128,1727)`, digit 2 at `(404,1727)`.

The layout/bounds were the same in both dumps: the OTP `android.widget.EditText` was `[146,590][934,716]`.

## Result

Pre-cold, the `(540,650)` tap was inside the field bounds but the field still did not focus. That is a real fault, not a missed tap:

- focused node: full-screen `android.widget.FrameLayout`, bounds `[0,0][1080,2400]`
- OTP `android.widget.EditText`: bounds `[146,590][934,716]`, `focused=false`
- keyboard collapsed (`mInputShown=false` after settling); digits 3/4 by `input keyevent` and `56` by `input text` left every cell empty

After a true cold boot, the same tap focused the field:

- focused node: `android.widget.EditText`, bounds `[146,590][934,716]`
- `mInputShown=true`
- keyevent digits 3/4 and input text 56 filled the first four cells; tapping numeric-keypad 1/2 completed an intentionally wrong code and the app rejected it, proving on-screen digit entry

## Files

- `pre-cold-uiautomator-otp.xml` and `cold-uiautomator-otp.xml`: focus evidence
- `pre-cold-after-boxtap-keyboard-collapsed.png`: pre-cold tap result, keyboard gone and cells empty
- `pre-cold-after-keyevent-and-inputtext-empty.png`: pre-cold keyevent/input-text result, cells empty
- `cold-after-inputtext-56.png`: post-cold keyevent/input text result, first four cells filled
- `cold-after-focused-softkeys-12.png`: post-cold soft-key completion caused the expected wrong-code error
