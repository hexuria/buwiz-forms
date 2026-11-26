# Android comma-key addendum

Branch: `fix/money-box-keep-text-on-focus`. Requested production commit: `4e29caf043a19ed608168101e5ba0617d0c2ae18`. The checkout during this addendum was `7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f`; `git diff 4e29caf..7eacb1d` contains only the previously committed proof documents/images, so the production code under test is the same.

Device: `emulator-5554`; target: demo taxpayer, 2551Q Schedule 1 row 1 Taxable Amount. No Queue button was pressed and no live sending was enabled.

Input method: real touchscreen taps on the emulator's visible numeric keypad (`adb shell input swipe`, 150 ms holds). No `adb shell input text` and no hardware key events were used. The comma-key coordinate was `(168,2188)`, the bottom-row key immediately left of `0`; `comma-key-contact.png` catches that key grey/pressed while the amount remains unchanged.

## Result

**Comma-key result: FAIL for typed-comma entry.** The comma key visibly depresses, but no comma appears in the amount box. The per-key evidence below shows the text unchanged after each comma tap. Done/save formatting itself works, but the saved rows do not distinguish “comma accepted then stripped” from “comma never entered”; the per-key strings are the important evidence.

### Keys `1 5 , 5 0`

| key | visible text after key |
|---|---|
| `1` | `1` |
| `5` | `15` |
| `,` | `15` — unchanged |
| `5` | `155` |
| `0` | `1550` |

- Focused after the sequence: `1550` (`keys-15comma50-keyed.png`, enlarged `keys-15comma50-keyed-crop.png`).
- After Done/reopen, amount box shown: `1,550.00` (`keys-15comma50-reopened-shown.png`).
- Saved row: `PT010 · 1,550.00 · 3.0 · 46.50` (`keys-15comma50-saved.png`).
- Saved amount extracted from row: `1,550.00`.

### Keys `1 , 5 0 0`

| key | visible text after key |
|---|---|
| `1` | `1` |
| `,` | `1` — unchanged |
| `5` | `15` |
| `0` | `150` |
| `0` | `1500` |

- Focused after the sequence: `1500` (`keys-1comma500-keyed.png`, enlarged `keys-1comma500-keyed-crop.png`).
- After Done/reopen, amount box shown: `1,500.00` (`keys-1comma500-reopened-shown.png`).
- Saved row: `PT010 · 1,500.00 · 3.0 · 45.00` (`keys-1comma500-saved.png`).
- Saved amount extracted from row: `1,500.00`.

A long press on the comma key in the already filled focused box showing `1,500.00` also left the text exactly `1,500.00`; see `comma-key-contact.png` and the bottom-keyboard crop `comma-key-contact-crop.png`.

## Likely platform cause

`MoneyInput` uses `keyboardType="decimal-pad"`. In the pinned Android host, that becomes `InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL`. On this English/US emulator, the numeric decimal keypad visibly offers a comma grouping key, but Android/Gboard does not commit that comma to the `EditText`. Thus the TypeScript `moneyEntryMask` never receives a typed comma on Android, even though it treats commas already in the text as significant. This cause is inferred from the pinned host input type plus the runtime evidence; it was not separately instrumented.

## Commands and harness

Build/run command, from `/home/ubuntu/forms-validation/buwiz-keep/nsh`:

```sh
HOME=/home/ubuntu/forms-validation/home RUSTC_WRAPPER= \
  /home/ubuntu/forms-validation/nsh2404-keep.sh nsh run --target android --port 8792
```

The app was started with `adb reverse tcp:8792 tcp:8792` and the guarded database `/data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir/test-data/bir_data.db`. The same temporary fixture harness as the earlier proof opened `screens/form/FormPageAny` at Schedule 1 row 1. Both temporary harness edits were restored immediately after the run; `git diff -- nsh/App.tsx nsh/screens/form/FormPageAny.tsx` is empty.

Runner copied with the evidence: `comma_key_proof.py`. Machine log: `comma-key-proof.log`, ending `ANDROID_COMMA_KEY_FAIL` because the expected typed commas did not appear.
