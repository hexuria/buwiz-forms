# RDO fix validation — PR #399 at deef60d8

Exact commit verified: `deef60d83efbe1bf6b82a476d407595973d1d82f`.
Worktree `/home/ubuntu/forms-validation/rdo-fix-deef60d8`; isolated target `/home/ubuntu/forms-validation/target-rdo-fix-deef60d8`. Real visible maximized Linux X11 window, software GTK rendering, scratch DB only. Safety variables only; no live send, no Queue, invented values only. Android emulator/watch untouched.

## Verdict

HOLDS, with two stated limits: the assistant door accepts `000` as a fill (validation then refuses it), and the 1801 wrong-value path cannot be typed through the page because that control is picker-only.

## 1. 1601C, taxpayer with no profile RDO

- Blank RDO validation showed exactly `RDO is required`.
- The list is the host's real searchable RDO catalog. Catalog source count is 138 codes, first `001 — Laoag City`, last `132 — RDO 132`, no `000`. The page virtualizes the sheet (about 60 semantic rows visible at once), so the visual count was verified through the host/test catalog rather than one snapshot.
- Searching `000` showed `Nothing matches. Try a code or other words.`; searching found `001`, `040`, and `132`; no `rdo.description` or other template text appeared.
- Picked `040 — Cubao`; validation no longer had an RDO-box error.
- Left and reopened the return: `040 — Cubao` was kept.
- The taxpayer profile still had empty `rdo_code` afterward; choosing the RDO on the return did not write the profile.

Screenshots: `1601c-catalog-open.png`, `1601c-catalog-search000.png`, `1601c-blank-validation.png`, `1601c-040-validation.png`, `1601c-040-reopen.png`, `profile-after-1601c.png`.

## 2. 1701Q filer and spouse, no profile RDO

- Filer RDO box has the same catalog behavior: real entries, no `000`, no template text; search `000` found nothing; `001`, `040`, `132` found.
- After Civil Status `Married`, the spouse RDO box appeared and had the same catalog behavior.
- Main evidence from the first part remains in `../main-219da036/`: 1701Q showed `Choose…` and the literal leftover `[]` there.

Screenshots: `1701q-filer-catalog-open.png`, `1701q-filer-catalog-search000.png`, `1701q-married.png`, `1701q-spouse-catalog-open.png`, `1701q-spouse-search000.png`.

## 3. 1601C, taxpayer with profile RDO 039

- The box showed the profile-carried value folded/read-only as `039 — South Quezon City` with the profile note.
- `Edit` opened the same real catalog without `000` or template text.

Screenshots: `1601c-hasrdo-folded.png`, `1601c-hasrdo-edit-list.png`, `1601c-hasrdo-edit-catalog-open.png`.

## 4. Assistant door: `form.fill` of 1601C RDO with `000`

Headless `bir serve` against a scratch copy of the same DB:

- `form.open` succeeded for no-RDO taxpayer `00011124500000`, 1601C 2026 period 11.
- `form.fill` with `{"frm1601c:txtRDOCode":"000"}` was **not refused**. It returned `filled`/`applied` for `frm1601c:txtRDOCode`.
- `form.validate` then refused it as `rdo_code`: `Please enter a valid RDO Code on Item 7.`

Evidence: `test-logs/assistant-open.json`, `assistant-fill000.json`, `assistant-validate.json`. No token is printed or stored in the evidence.

## 5. Required tests and negative reversion

At the exact fixed commit:

- `cargo test --locked -p bir-core --lib form_1801`: **passed**, 8 passed / 0 failed.
- `cargo test --locked -p bir-app-api --lib rdo_lists`: **passed**, 7 passed / 0 failed.
- `nsh test screens/form/FormPageNoRdo.test.tsx`, three runs: **passed each time** (`1 passed, 0 failed`).

Then only the behavioral use of the new `schema_list` in `catalogs.rs` was locally reverted (never committed), leaving the helper itself so tests compiled:

- `cargo test --locked -p bir-app-api --lib rdo_lists`: failed as expected — 4 passed, 3 failed (`every_rdo_select_offers_the_rdo_catalog`, `no_select_offers_template_text_and_only_rdo_lists_are_placeholders`, `form_1601c_rdo_is_locked_from_the_profile_or_open_with_the_catalog`).
- `FormPageNoRdo.test.tsx`: failed as expected; the 001 option never appeared and the screen fell back to `Choose…` / `000`.
- `catalogs.rs` was restored afterward and the worktree is clean.

Logs: `test-logs/cargo-bir-core-form-1801.log`, `cargo-bir-app-api-rdo-lists.log`, `screen-form-page-no-rdo-{1,2,3}.log`, `negative-cargo-bir-app-api-rdo-lists.log`, `negative-screen-form-page-no-rdo.log`, `summary.txt`, `catalogs-restored.txt`.

## 6. 1801 Schedule 4

- Schedule 4 uses `Add row`; the row drawer has name, address, picker-only RDO, exclusive value and conjugal value.
- The RDO control is a picker/sheet, not a typed text box. It opened real catalog entries, had no `000`, and search found `040`; `040 — Cubao` was picked.
- Validation with row RDO `040` showed no Schedule 4 row-RDO error. The screenshot still had the separate main-item message `Please enter a valid RDO Code on Item 6.` because Item 6 itself was blank.
- A wrong/free-text value cannot be entered through this page control, so the new message `Please enter a valid RDO Code on Schedule 4, Row N.` was not triggered through page typing. The core form tests cover that validation path and passed.

Screenshots/data: `1801-sched4-addrow.png`, `1801-sched4-row.png`, `1801-rdo-picker.png`, `1801-040-validation.png`, `1801-result.json`.

## Before/after open-list proof

Before (main): `../main-219da036/1601c-nordo-rdo-list.png` — only `Choose…` and `000`.
After (deef60d8): `1601c-catalog-open.png` — real catalog entries.

Recording: `rdo-fix-deef-validation.mp4` covers the real-window UI pass.
