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

## Follow-up: inner list geometry and bottom insets

Node ids below are those in the saved snapshots; numeric ids are unstable across openings. All bounds are dp unless px is named.

| navigation | list scroll node/bounds | sheet body below search (`n466`) | inner content (`n468`) | row 006 bounds | row 006 visible/clipped | list-bottom → keyboard-top gap |
|---|---|---|---|---|---:|---:|
| three-button | `new-tp-rdo_code-list`, x 17.143 y 229.714 w 377.143 h 285.333 | x 1.143 y 153.905 w 409.143 h 376.000 | x 17.143 y 169.905 w 377.143 h 345.143 | x 17.143 y 490.667 w 377.143 h 52.190 | 24.381 / 27.810 | 62.857 dp = 165.000 px |
| gesture | `new-tp-rdo_code-list`, x 17.143 y 229.714 w 377.143 h 309.333 | x 1.143 y 153.905 w 409.143 h 400.000 | x 17.143 y 169.905 w 377.143 h 369.143 | x 17.143 y 490.667 w 377.143 h 52.190 | 48.381 / 3.810 | 38.857 dp = 102.000 px |

The semantic rows are laid out beyond the scroll viewport; row 006's visible height above is clipped at the scroll container bottom. This reproduces the screenshot difference: the text is half-hidden in three-button and effectively full in gesture.

Bottom inset measurements:

| navigation | keyboard down, window navigation bar bottom | keyboard up, sheet bottom inset from driver bounds | driver's keyboard frame height |
|---|---:|---:|---:|
| three-button | 126 px = 48 dp | 126 px = 48 dp | 883 px = 336.381 dp |
| gesture | 63 px = 24 dp | 63 px = 24 dp | 883 px = 336.381 dp |

The keyboard-down values are read-only window-inset state (`navigationBars` frame/mInsetsHint). The keyboard-up values are measured directly in the saved driver snapshots as panel bottom minus sheet-body bottom, the padding produced by `pb-[env(safe-area-inset-bottom)]`. They are unchanged. The driver's keyboard frame reaches the screen bottom in both modes, so the navigation area is already inside that bottom keyboard frame; the sheet body then subtracts the same bottom safe-area inset again when sizing the list. The double count is 48 dp in three-button and 24 dp in gesture, and the 24 dp difference is exactly the list-height difference (309.333 − 285.333).

Machine-readable numbers are in `internal-geometry.json`.
