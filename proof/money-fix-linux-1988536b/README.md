Free disk after cleanup: 46 GiB.

# Independent Linux real-window money-box validation

**Verdict: NO — not every case saved what was typed.** The principal failure is the Tab-in replacement case: with the box holding `1,234.56`, Tab-in followed by `15000.50` inserted into the existing digits (`1234.515000506`) and saved `1234.52`, not `15000.5`. The click/Ctrl+A paths and the stress/rounding/editing/burst/paste cases passed.

## Target and build

- Repository/worktree: `/home/ubuntu/forms-validation/money-1988536b`
- Exact commit verified: `1988536bfd41b22ee77d6f1e39ef29a61d4bbc87`
- Git status after removing the temporary seeding test: clean (0 changed files)
- Build/target folder used: `/home/ubuntu/forms-validation/target-money-1988536b` (removed after validation per cleanup rules)
- Linux host binary used: `/home/ubuntu/forms-validation/target-money-1988536b/debug/nsh-app-nsh-app-ebirforms-linux-host` (removed with the build folder after validation)
- Validation database: `/home/ubuntu/forms-validation/money-box-proof/money-validation.db`
- Pinned NSH sibling used read-only: `/home/ubuntu/forms-validation/nsh` (`../nsh` from the worktree)
- Containerized Ubuntu 24.04 NSH runner: `/home/ubuntu/forms-validation/nsh2404-money-1988536b.sh` (host Ubuntu 22.04 cannot run the pinned NSH binary directly)
- Toolchain: rustup toolchain cargo first in PATH (`cargo 1.99.0`), `RUSTC_WRAPPER` empty, no huacnlee cargo
- Run safety environment: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no live BIR send, no Queue press, demo taxpayer only
- Extra Linux rendering variables needed for a visible window on this software-GPU VM: `VK_ICD_FILENAMES=/dev/null GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1`

## Method

The real visible X11 window on `DISPLAY=:0` was maximized and driven by real `xdotool` mouse and key events. Core digit entry used `xdotool key --delay 120` except the explicit burst case, which used `--delay 0`. Paste used xclip plus a real Ctrl+V key event. Semantic gpui-agent snapshots were used only to locate controls and read shown values; SQLite was used only to read persisted draft/profile values.

The 2551Q draft stores typed model JSON, not flat field keys. Schedule 1 row 1 taxable amount was read as:

```text
form_drafts.data_json -> schedule_1[0].taxable_amount
```

using the row for TIN `00012345300000`, form `2551Q`, year `2025`, quarter `1`.

## Money matrix results

| Case | Keys/action | Shown focused | Shown after blur | Saved draft value | Result |
|---|---|---:|---:|---:|---|
| (a) zero box | click, type `15000.50` | `15000.50` | `15,000.50` | `15000.5` | PASS |
| (b) existing `1,234.56` | click, Ctrl+A, type `15000.50` | `15000.50` | `15,000.50` | `15000.5` | PASS |
| (c) existing `1,234.56` | click, Ctrl+A, BackSpace, type `15000.50` | `15000.50` | `15,000.50` | `15000.5` | PASS |
| (d) existing `1,234.56` | Tab in, type `15000.50` | `1234.515000506` | `1,234.52` | `1234.52` | FAIL / anomaly |
| (d2 repeat) existing `1,234.56` | close/reopen editor, Tab in, type `15000.50` | `1234.515000506` | `1,234.52` | `1234.52` | FAIL / anomaly repeated |
| stress | from `0.00`, type `99999.99` | `99999.99` | `99,999.99` | `99999.99` | PASS |
| rounding | from `0.00`, type `1000000.005` | `1000000.005` | `1,000,000.01` | `1000000.01` | PASS |
| editing keys | type `1234567890`, Left Left BackSpace | `123456790` | `123,456,790.00` | `123456790.0` | PASS |
| repeated nines run 1 | type `99999999.99` | `99999999.99` | `99,999,999.99` | `99999999.99` | PASS |
| repeated nines run 2 | type `99999999.99` | `99999999.99` | `99,999,999.99` | `99999999.99` | PASS |
| repeated nines run 3 | type `99999999.99` | `99999999.99` | `99,999,999.99` | `99999999.99` | PASS |
| burst | type `99999999.99` with xdotool delay 0 | `99999999.99` | `99,999,999.99` | `99999999.99` | PASS |
| paste | paste `99999999.99` with real Ctrl+V | `99999999.99` | `99,999,999.99` | `99999999.99` | PASS |

Important interpretation of case (d): the owner matrix did not explicitly state whether Tab-in must select the existing amount. I treated replacement with `15000.50` as the expected outcome because that is how the case is grouped with (a)-(c), and because the source comment in `MaskedField` says Tab focus keeps the whole text selected. In the real Linux window, typing after Tab-in inserted into the existing digits instead. This produced `1234.515000506`, which then blurred/saved as `1,234.52`. The same result repeated after closing and reopening the row editor, so it was not only a caret left by the setup click. This is the principal money-box finding.

## Other masked controls

| Control | Test | Result |
|---|---|---|
| Whole number (`Number of Sheet/s`, `fp-frm2551Qv2018:txtSheets`) | real-key `12` and repeated-digit `11` | PASS: shown and saved exactly (`number_of_attached_sheets` = 12, then 11). Longer input is capped by the field's two-digit maximum; initial over-length attempts were therefore not valid mask failures. |
| Date | profile birth date, real-key `2011-11-11` | PASS after adapting to the profile field's required ISO format: focused, blurred, and saved profile value were all `2011-11-11`. The owner example `11/11/2026` is not appropriate for this birth-date field (format and future-date validation). |
| Date in 2551Q Part III | inspection | NOT APPLICABLE: `txtDate25`-`txtDate28` are explicitly closed/locked until payment in 2551Q, so they could not be typed. |
| TIN | attempted in taxpayer profile | NOT REPRESENTATIVE / BLOCKED: the profile TIN editor is a `DigitCells` control, not the shared `TinInput` masked-field implementation. Real-key batch entry did not replace the nine digits; it changed only the final digit. In 2551Q itself the TIN is profile-locked, so no editable shared `TinInput` was available there. |
| RDO code | inspection | NOT APPLICABLE in this surface: 2551Q RDO is profile-locked; taxpayer profile RDO is a dropdown pick, not a typed shared masked input. |
| ATC | inspection | NOT APPLICABLE in 2551Q Schedule 1: ATC is a dropdown trigger, not typed text. |

## Artifacts

- Recording: `/home/ubuntu/forms-validation/money-box-proof/money-box-linux-validation.mp4`
- Machine-readable rows: `/home/ubuntu/forms-validation/money-box-proof/money-results.jsonl`
- CSV rows: `/home/ubuntu/forms-validation/money-box-proof/money-results.csv`
- Screenshots: `/home/ubuntu/forms-validation/money-box-proof/*-focused.png` and `*-blurred.png`
- Driver helpers (test-only artifacts): `/home/ubuntu/forms-validation/money-box-proof/drive_money.py`, `/home/ubuntu/forms-validation/money-box-proof/run_money_cases.py`
- Seed/build/run logs: `/home/ubuntu/forms-validation/money-box-proof/seed-build.log`, `/home/ubuntu/forms-validation/money-box-proof/nsh-run.log`

## Caveats and untested/blocked items

1. No shared editable `TinInput` surface was found within the time available: 2551Q TIN is locked and taxpayer profile uses a different digit-cell control.
2. RDO and ATC are not typed shared masked inputs in the inspected 2551Q/profile surfaces.
3. The paste matrix item did not specify a value; I used `99999999.99` to match the stress cases.
4. The burst matrix item did not specify a value; I used `99999999.99`.
5. The first launch was invisible/transparent on this software-GPU VM until the Linux renderer variables above were added; after that the real window was visibly rendered and recorded.
6. While establishing the run, the first `nsh run` printed that it replaced an existing NSH dev session on port 8792. The Android emulator processes remained up, and the final validation run used explicit port 8899.
7. During the non-representative profile digit-cell TIN probe, the scratch validation profile TIN changed from `000123453` to `000123451`. This is only in the disposable validation database and happened after all money rows had already been read; it did not affect the money matrix.
