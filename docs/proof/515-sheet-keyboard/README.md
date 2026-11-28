app e73e6c9fa499b1add59af5abdf2fae5891cff86a; kit c186d324ccfc4c4798a8054be4d7fcaead446d4f; plug b559e40dc593bd8e8cd78060ddb5a3507a237767

driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b559e40dc593bd8e8cd78060ddb5a3507a237767

# #515 sheet keyboard proof

Android emulator API 35, 1080 × 2400 px, density 420 dpi (2.625). The app was the development build with the safety
environment (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`), against a scratch database.
The sibling NSH checkout and the Android runtime/CLI used for the build were exactly `c186d324ccfc4c4798a8054be4d7fcaead446d4f`.

Input was driven only through gpui-agent: choose the first-launch solo setup, press `new-tp-rdo_code-trigger`, then press
`new-tp-rdo_code-search`. `system back` through gpui-agent closed the sheet between modes. No `adb shell input` or
uiautomator was used. The full-device PNGs are read-only `adb exec-out screencap -p` captures.

The navigation-mode configuration commands were exactly:

```sh
adb shell cmd overlay enable com.android.internal.systemui.navbar.threebutton
adb shell cmd overlay enable com.android.internal.systemui.navbar.gestural
```

The driver's `new-tp-rdo_code-sheet` test id is the zero-height anchor in the form, not the drawn panel. The measured
sheet root is the first full-width, partial-height popup panel ancestor of `new-tp-rdo_code-search` whose bottom matches
the driver's settled `system-keyboard` node (`n33` in both saved snapshots). The status bar bottom is read-only window
state (`dumpsys window windows`) because this driver reports the keyboard frame but no status-bar node.

## Measurements

| navigation | status bottom px (dp) | sheet top px (dp) | sheet bottom px (dp) | keyboard top px (dp) | density | screen height | result |
|---|---:|---:|---:|---:|---:|---:|---|
| three-button | 63 (24.000) | 199.000 (75.810) | 1517.000 (577.905) | 1517.000 (577.905) | 2.625 | 2400 px | pass |
| gesture | 63 (24.000) | 199.000 (75.810) | 1517.000 (577.905) | 1517.000 (577.905) | 2.625 | 2400 px | pass |

Assertions: sheet top at least status-bar bottom + 24 px (minimum 87 px), and sheet bottom equals keyboard top within
±1 px. Both modes pass. The exact floating-point bottom difference was −0.000066 px in each mode.

Files: `threebutton.png`, `gesture.png`, `measurements.json`, `threebutton-snapshot.json`, `gesture-snapshot.json`.
