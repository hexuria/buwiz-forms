app code 971bb0855482838870b0c4bbc88e79b615cecb69, kit c186d324ccfc4c4798a8054be4d7fcaead446d4f, plug rev db0e4ac6127078a05751149f0ec6bed68204f911

# #515 inset-fix sheet keyboard proof

Android emulator API 35, 1080 × 2400 px, density 420 dpi (2.625). The app was the development build with the safety
environment (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`), against a scratch database.
The sibling NSH checkout was exactly `c186d324ccfc4c4798a8054be4d7fcaead446d4f`. The CLI was `nsh 0.6.1 (c186d32)`;
the Android dev and release runtimes used for the emulator run were built from that checkout with `cargo xtask android --dev --abi x86_64`
and `cargo xtask android --release --abi x86_64` (the emulator's ABI). Their `classes.dex` sha256 was
`53d397565cd5a38bafa7409fb9d333b1467f5a78db4e37dac4b65fcce5e918ff`.

Input was driven only through gpui-agent: choose the first-launch solo setup, press `new-tp-rdo_code-trigger`, and use
`system keyboard-hide` only for the keyboard-down padding measurement. No `adb shell input` or uiautomator was used.
The full-device PNGs are read-only `adb exec-out screencap -p` captures.

The navigation-mode configuration commands were exactly:

```sh
adb shell cmd overlay enable com.android.internal.systemui.navbar.threebutton
adb shell cmd overlay enable com.android.internal.systemui.navbar.gestural
```

The driver's `new-tp-rdo_code-sheet` test id is the zero-height anchor in the form, not the drawn panel. The measured
sheet root is the first full-width, partial-height panel below that anchor (`n33` in these snapshots). Numeric node ids
are unstable across openings. The status bar bottom is read-only window state because this driver reports the keyboard
frame but no status-bar node.

## Seen failing first, then passing

With `sheetBottomPad` temporarily mutated to the old behavior (`return safeBottom`), `just nsh-test ui/sheetCap.test.ts` failed:

```text
✓ ui/sheetCap.test.ts › a tall sheet with the keyboard up never extends under the keyboard or off the top (0ms)
✗ ui/sheetCap.test.ts › with the keyboard up on Android a sheet does not pad for the navigation bar the keyboard covers (0ms)
    expected 48 to be 0
    at <anonymous> (ui/sheetCap.test.ts:55:40)
✓ ui/sheetCap.test.ts › iOS and the desktop keep today's sheet body exactly (0ms)
error 3 tests: 2 passed, 1 failed (1m 27s)
error: recipe `nsh-test` failed with exit code 1
```

After restoring the real code:

```text
✓ ui/sheetCap.test.ts › a tall sheet with the keyboard up never extends under the keyboard or off the top (1ms)
✓ ui/sheetCap.test.ts › with the keyboard up on Android a sheet does not pad for the navigation bar the keyboard covers (0ms)
✓ ui/sheetCap.test.ts › iOS and the desktop keep today's sheet body exactly (0ms)
✓ 3 tests: 3 passed, 0 failed (624ms)
```

## Checks and focused tests

`just nsh-check`:

```text
✓ checked nsh-app-ebirforms (1m 16s, 1 warning: -v shows it)
✓ built the Rust crate nsh-app-ebirforms (409ms, 1 warning: -v shows it)
CHECK_RC=0
```

The verbose check also exited 0; its warning was `page_gate_refusal` dead code in `crates/bir-app-api/src/screen_form_module.rs`.

The test files were run one at a time because this `nsh test` accepts one path. `grep -rl "masked.fixture" nsh --include=*.test.*`
found only `ui/fields.test.tsx`; no additional quick test file imports `layers`' `Sheet` beyond `ui/sheetCap.test.ts` itself.

| test file | result | failures |
|---|---|---|
| `ui/sheetCap.test.ts` | 3 passed, 0 failed | none |
| `screens/form/controls/pick.test.tsx` | 20 passed, 0 failed | none |
| `ui/fields.test.tsx` | 102 passed, 2 failed | `phone: a plain paste, a third decimal and a refused key send no focused write` — also on e73e6c9f; `desktop: a plain paste, a third decimal and a refused key send no focused write` — also on e73e6c9f |

Both `ui/fields.test.tsx` failures expect `"599999999.99"` to be `"99999999.995"` at line 366. The e73e6c9f baseline run had
the same two failures and the same count: 102 passed, 2 failed. No failure was new.

## Keyboard-up measurements

All bounds are dp unless px is named. Row spacing is 52.190 dp (row height 52.190 dp plus a 0.000 dp gap).
Assertion (a) is sheet top at least status-bar bottom + 24 dp, so the minimum is 48 dp. Assertion (b) is sheet bottom equal
to keyboard top within ±1 px. Assertion (c) is list bottom → keyboard top no more than row spacing. Assertion (d) is no
clipped last row, or whole-row scrolling.

| navigation | sheet top px (dp) | sheet bottom px (dp) | keyboard top px (dp) | (a) | (b) | (c) | (d) |
|---|---:|---:|---:|---|---|---|---|
| three-button | 199.000 (75.810) | 1517.000 (577.905) | 1517.000 (577.905) | pass | pass | pass | fail |
| gesture | 199.000 (75.810) | 1517.000 (577.905) | 1517.000 (577.905) | pass | pass | pass | fail |

| navigation | list h | body h | content h | list bottom → keyboard top | last visible row visible/clipped | bottom inset down | bottom inset up |
|---|---:|---:|---:|---:|---:|---:|---:|
| three-button | 333.333 | 424.000 | 393.143 | 14.857 dp = 39.000 px | row 007: 20.191 / 31.999 | 48.000 dp | 0.000 dp |
| gesture | 333.333 | 424.000 | 393.143 | 14.857 dp = 39.000 px | row 007: 20.191 / 31.999 | 24.000 dp | 0.000 dp |

The exact sheet bottom minus keyboard top was −0.000066 px in both modes. The keyboard-down three-button bottom padding was
47.99997 dp = 125.99992 px, i.e. the unchanged 48 dp navigation inset. In gesture it was 23.99997 dp = 62.99992 px.

(d) fails on the driver's node bounds: row 007's top is inside the list viewport, but 31.99995 dp of its 52.19048 dp height
lies beyond the list bottom. A three-button scroll probe also moved the rows fractionally, not by whole rows: after
`scroll new-tp-rdo_code-list --dy 30`, row 001 moved from y 229.714 to y 209.143 (20.571 dp). Thus the alternative
"list scrolls whole rows" did not apply. The visible part of row 007 is only its blank top padding in the screenshots; row 006,
the last row with visible text, is complete.

Files: `threebutton.png`, `gesture.png`, `measurements.json`, `internal-geometry.json`, `threebutton-snapshot.json`,
`gesture-snapshot.json`, `threebutton-keydown-snapshot.json`, `gesture-keydown-snapshot.json`, and
`threebutton-scrolltest-snapshot.json`.
