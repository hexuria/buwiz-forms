# Taffy 0.16 Android/Linux BEFORE-AFTER evidence

App BEFORE `b983b2a4eff06b0c6203fd5829d62f5128db8639` with NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; app AFTER `8f825621dc697f13bea69e4f83a46711d59a42c5` with NSH `bc417406b63479bf137b8327374265a62a2c0f6a`. Tour suite `e2e/tour-suite` head `9dbf0214f3307b7625dbb6c214938859659c05b1`. gpui-agent CLI `gpui-agent-cli 0.5.0`; NSH CLI `nsh 0.6.1`, BEFORE `(8c78cfd)`, AFTER `(bc41740)`.

All runs used the dry-run test environment (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`), scratch databases, gpui-agent recipes only, and debug builds. No fixed-coordinate taps, OS input injection, `BIR_SFTP_LIVE`, production services, or release builds were used.

Android used three-button navigation. Density commands: `adb -s emulator-5554 shell wm density 420` for 2.625 and `adb -s emulator-5554 shell wm density 440` for 2.75. After the Android runs it was reset with `adb -s emulator-5554 shell wm density 420`; readback: `Physical density: 420`.

## Suite adaptations (test harness only)

No product code was changed. The copied tour suite was adapted identically in both states only where Android/Linux driver mechanics blocked it: Android `sign-up`/`tax-year` were converted to equivalent JSON recipes so gpui-agent's `system keyboard_hide` op could be used; a Linux sign-up variant ends at `tab-home` because Linux opens 420 px wide and uses the narrow layout. Screenshots in run folders are before/after pairs under the same adapted recipes.

## Tour summaries

| platform/density | BEFORE | AFTER |
|---|---|---|
| Android 2.625 | 11 pass, 2 fail, 2 skipped | 11 pass, 2 fail, 2 skipped |
| Android 2.75 | 11 pass, 2 fail, 2 skipped | 11 pass, 2 fail, 2 skipped |
| Linux | 3 pass, 2 fail, 3 skipped, 7 not run | 3 pass, 2 fail, 3 skipped, 7 not run |

Android skips: `shell` and `print` have no phone variant. Android failures in both states are the same driver/platform blockers: `sheet-pick` s5 and `sheet-calendar` s3 target nodes are offscreen with no native view, so the bottom-sheet keyboard-up evidence could not be captured by this suite. The calendar grid itself is compared in `07-navigation` screenshots.

Linux skips: `setup-cards`, `home`, and `navigation` have no desktop variant. Linux `shell` fails `target_gone: nav-home` and `new-return` fails `target_gone: sb-new` in both states because the 420 px Linux window uses narrow-layout ids while those desktop recipes expect wide-layout ids; later recipes did not run.

## Comparison: Android 2.625, status bar masked 63 px

| pair | result | diff-px | box |
|---|---:|---:|---|
| `01-setup-start/001-s1.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `02-setup-cards/001-s1.png` | differs | 114624 | [0, 2274, 1080, 2400] |
| `03-sign-up/011-s11.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `03-sign-up/015-s15.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `04-tax-year/017-s16.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `06-home/002-s2.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `07-navigation/002-s2.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `07-navigation/004-s4.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `07-navigation/006-s6.png` | differs | 325619 | [0, 181, 1080, 2400] |
| `07-navigation/008-s8.png` | differs | 326159 | [0, 181, 1080, 2400] |
| `08-new-return/002-s2.png` | differs | 149533 | [0, 265, 1080, 2400] |
| `08-new-return/005-s5.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `09-form-top/001-s1.png` | differs | 145408 | [0, 265, 1080, 2400] |
| `10-form-middle/002-s2.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `11-form-end/004-s4.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `12-schedule-rows/018-s18.png` | differs | 136080 | [0, 2274, 1080, 2400] |
| `13-sheet-pick/failure.png` | differs | 136080 | [0, 2274, 1080, 2400] |

| pair | what moved |
|---|---|
| `01-setup-start/001-s1.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `02-setup-cards/001-s1.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `03-sign-up/011-s11.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `03-sign-up/015-s15.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `04-tax-year/017-s16.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `06-home/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/004-s4.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/006-s6.png` | The bottom inset fill changed as above, and the transient “2026 forms updated” toast was present in AFTER but already gone in BEFORE at capture time. |
| `07-navigation/008-s8.png` | The bottom inset fill changed as above, and the transient “2026 forms updated” toast was present in AFTER but already gone in BEFORE at capture time. |
| `08-new-return/002-s2.png` | The bottom inset fill changed as above; the right-edge scrollbar lane also differs, a timing-sensitive scrollbar visibility/position artifact rather than a measured layout shift. |
| `08-new-return/005-s5.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `09-form-top/001-s1.png` | The bottom inset fill changed as above; the right-edge scrollbar lane also differs, a timing-sensitive scrollbar visibility/position artifact rather than a measured layout shift. |
| `10-form-middle/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `11-form-end/004-s4.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `12-schedule-rows/018-s18.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `13-sheet-pick/failure.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |

## Comparison: Android 2.75, status bar masked 66 px

| pair | result | diff-px | box |
|---|---:|---:|---|
| `01-setup-start/001-s1.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `02-setup-cards/001-s1.png` | differs | 83874 | [0, 2268, 1080, 2400] |
| `03-sign-up/011-s11.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `03-sign-up/015-s15.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `04-tax-year/017-s16.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `06-home/002-s2.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `07-navigation/002-s2.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `07-navigation/004-s4.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `07-navigation/006-s6.png` | differs | 142702 | [0, 379, 1080, 2400] |
| `07-navigation/008-s8.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `08-new-return/002-s2.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `08-new-return/005-s5.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `09-form-top/001-s1.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `10-form-middle/002-s2.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `11-form-end/004-s4.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `12-schedule-rows/018-s18.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `13-sheet-pick/failure.png` | differs | 142560 | [0, 2268, 1080, 2400] |
| `14-sheet-calendar/failure.png` | differs | 142560 | [0, 2268, 1080, 2400] |

| pair | what moved |
|---|---|
| `01-setup-start/001-s1.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `02-setup-cards/001-s1.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `03-sign-up/011-s11.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `03-sign-up/015-s15.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `04-tax-year/017-s16.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `06-home/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/004-s4.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `07-navigation/006-s6.png` | The bottom inset fill changed as above, and the calendar previous-month chevron rasterizes/shifts slightly within a 14×26 px box. |
| `07-navigation/008-s8.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `08-new-return/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `08-new-return/005-s5.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `09-form-top/001-s1.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `10-form-middle/002-s2.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `11-form-end/004-s4.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `12-schedule-rows/018-s18.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `13-sheet-pick/failure.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |
| `14-sheet-calendar/failure.png` | Only the bottom navigation-inset fill changed: BEFORE is white (#fefefe), AFTER the page background (#f4f4f6); app content above the inset is pixel-identical. |

## Comparison: Linux

| pair | result | diff-px | box |
|---|---:|---:|---|
| `01-setup-start/001-s1.png` | identical | 0 | - |
| `03-sign-up/012-s12.png` | identical | 0 | - |
| `03-sign-up/017-s17.png` | identical | 0 | - |
| `03-sign-up/018-s18.png` | differs | 30261 | [11, 695, 409, 773] |
| `03-sign-up/019-s19.png` | differs | 26502 | [16, 695, 404, 765] |
| `04-tax-year/016-s16.png` | identical | 0 | - |
| `05-shell/failure.png` | identical | 0 | - |
| `08-new-return/failure.png` | identical | 0 | - |

| pair | what moved |
|---|---|
| `03-sign-up/018-s18.png` | The toast is at a different fade opacity at capture time; its geometry and the surrounding layout are unchanged. |
| `03-sign-up/019-s19.png` | The toast is at a different fade opacity at capture time; its geometry and the surrounding layout are unchanged. |

## Persistent finding

Across every Android pair at both densities, the bottom navigation-inset band changes from white (#fefefe) BEFORE to the page background (#f4f4f6) AFTER: 126 px at density 2.625 and 132 px at density 2.75. Outside that band, the compared app content is pixel-identical except the explicitly noted transient toast/scrollbar artifacts and the small chevron raster shift at 2.75. No sticky part-title, form scroll-end, row, or calendar-grid layout movement was observed in the screenshots that the suite could capture.

## Smoke summaries

`just e2e android smoke` at AFTER: `1 fail, 9 not run, 1 pass`. Failure: `sign-up` s13 `click tab-home`: `occluded_by: software keyboard (a press on `node 894` at (51.42857, 838.2857) lands on a key; hide the keyboard or scroll the target above it first)`. The focused BEFORE rerun (`setup-start`, `sign-up`) failed identically at the same step with the same error, so this is not new with Taffy 0.16.

`just e2e linux smoke --allow-send` at AFTER: `1 fail, 2 known gap, 6 pass, 1 setup, 1 skipped`. Failure: `queue` s5 `click tp-branch-business_start_date`: `occluded_by: 507`. `birth-date` and `sign-out` are recorded known gaps from the narrow Linux window; `sheet-press` skipped for no desktop variant. The focused BEFORE rerun (`setup-start`, `sign-up`, `tax-year`, `open-2551q`, `queue`) also failed `queue` at s5, with `occluded_by: 399`; this is not new with Taffy 0.16.

## Layout notes for reviewer-named areas

- Sticky form part titles: `09-form-top`, `10-form-middle`, `11-form-end`, and `12-schedule-rows` compared without content movement outside the bottom inset; observed extra differences are transient scrollbar visibility only.
- Bottom sheets with keyboard up: not reached on Android because the sheet target nodes are refused as offscreen/no-native-view in both states; this is a driver/platform blocker, not a BEFORE/AFTER difference.
- End of scroll areas: `11-form-end` and `12-schedule-rows` have identical app content outside the bottom inset.
- Calendar grid: the grid cells and upcoming rows are identical; at 2.75 only the previous-month chevron shows a small raster/shift difference, and at 2.625 the large extra difference is toast timing.

## Cleanup recorded

Deleted only announced rebuildable/completed items: four completed prior-job workdirs; finished BEFORE Android architecture artifacts and matching hardlinked shared-cache copies; then finished BEFORE host/Linux build artifacts and matching hardlinked shared-cache copies. The shared cache was never deleted wholesale; `~/.cargo`, SDK/emulator images, evidence, and other sessions' kept folders were not touched.
