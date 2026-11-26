# RDO box desktop validation at main 219da036

Free disk after final cleanup: 45 GiB.

**Answer line: YES — with an empty profile RDO, the 1601C RDO box is unusable on the desktop too.** It is editable, but its list contains only `Choose…` and `000`, so no valid RDO can be picked.

## Exact setup

- Repository: `hexuria/buwiz-forms`, detached commit `219da036370d46afcc4e0814a653912436d066d0` (verified with `git rev-parse HEAD`).
- Worktree: `/home/ubuntu/forms-validation/rdo-219da036`; no product files were modified (`git status --short` empty after temporary seed files were removed).
- Build target dir used: `/home/ubuntu/forms-validation/target-rdo-219da036`; removed after validation.
- Pinned NSH sibling: `/home/ubuntu/forms-validation/nsh` at pin `8c78cfd6`; not edited; no `nsh update`.
- Explicit NSH port `8898`; the Android dev session/watch on port `8792` and the emulator were not touched.
- Visible maximized Linux X11 window on `DISPLAY=:0`, launched with `VK_ICD_FILENAMES=/dev/null GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1`.
- Safety environment only: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no live send, no token printed, Queue never pressed.
- Scratch database: `rdo-validation.db` in this proof folder; invented values only.
- Invented profiles:
  - `00012345300000`, Juana Santos, RDO `039` (demo fixture).
  - `00011124500000`, Invented NoRdo Person, RDO empty string.

## 1. Can a taxpayer exist with no RDO?

**Yes.** The database/host path can represent it: the no-RDO profile above was created through the app's host/test fixture in the scratch validation database, calling `bir.create_taxpayer` with `head_office.rdo_code: ""`. The database creation code validates a non-empty taxpayer name but does not require a non-empty RDO.

The interactive New taxpayer flow was attempted in the real window. The native Profiles menu showed `New Tax Profile...`, but activating it in this Linux dev-host run did not navigate to the page, so the click-through creation is **not proven**. Source inspection is nevertheless explicit: `nsh/screens/taxpayer/logic.ts` documents “Empty is fine: none is required”; `factError()` returns null for an empty RDO, and `NewTaxpayer.tsx` passes `cur.rdo_code.trim()` to `create_taxpayer`. Thus the New taxpayer flow is designed to allow an empty RDO even though this run did not prove the click path.

Other ways a profile can lack an RDO follow from the same optional field: an imported or older profile with the fact absent/empty, or any profile edit/import path that writes an empty branch `rdo_code`. The scratch host-created profile is direct proof that the current database schema and host accept that state.

## 2. No-RDO taxpayer, form 1601C

Evidence screenshots:

- `1601c-nordo-part1.png`
- `1601c-nordo-rdo-list.png`
- `1601c-nordo-validation.png`
- `1601c-nordo-rdo-000-picked.png`
- `1601c-nordo-rdo-000-validation.png`

Results:

1. The RDO box is **open/editable**, not locked, because the profile did not fill it.
2. The list has exactly two rows: `Choose…` and `000`. There is no search field and no real RDO list.
3. No real RDO can be picked because none is offered.
4. Before choosing, Check/validation says `RDO is required`.
5. Picking `000` is possible but invalid: validation says `Please enter a valid RDO Code on Item 7.`
6. No literal `Review` label/page was found in this UI; `Check` is the validation entry point. The return remained incomplete because other invented mandatory fields were also empty.
7. Unexpectedly, the Queue button was enabled while validation errors were visible. It was never pressed.

## 3. Taxpayer with profile RDO, form 1601C

Evidence screenshots:

- `juana-selected.png`
- `1601c-hasrdo-part1.png`
- `1601c-hasrdo-profile-unfolded.png`

With Juana Santos (`00012345300000`, profile RDO `039`), the RDO row is folded into `From tax profile · 6 filled boxes`. Expanding that group shows `RDO Code *` as `039, from tax profile`, with an `Edit RDO Code on this return` button. So the profile-filled state is locked/folded as intended.

## 4. Other forms with no profile RDO

Evidence screenshots and semantic captures:

| Form | RDO control with empty profile RDO | List/content observed | Real RDO pickable? | Evidence |
|---|---|---|---|---|
| 1601C | editable combobox | exactly `Choose…`, `000` | No | `1601c-nordo-rdo-list.png` |
| 2551Q | editable search picker | full searchable RDO list (`001 Laoag City`, `002 Vigan City`, …); reference data has 138 RDOs | Yes, offered; pick-and-save not separately completed | `2551q-nordo-rdo-list.png` |
| 0619E | editable search picker | full searchable RDO list, same 001/002/003 sequence | Yes, offered; pick-and-save not separately completed | `0619e-nordo-rdo-list.png` |
| 1701Q | editable combobox | exactly `Choose…` and malformed-looking `[]`; no real RDOs | No | `1701q-nordo-rdo-list.png` |
| 2550Q | plain editable text box | no dropdown; typing `039` showed and saved draft `rdo_code:"039"` | Yes, by free-text typing, not list | `2550q-nordo-rdo-typed.png` |

Code rule found:

- `catalogs.rs` offers the full RDO catalog when the field's binding is a Text binding whose path leaf is `rdo_code` or ends `_rdo_code`.
- But `catalogs::options()` only supplies missing lists for schema fields of type `sel` whose schema `o` list is absent or empty.
- In `forms/forms-schema.json`:
  - `1601C` `frm1601c:txtRDOCode` is `sel` with raw options `['000', '" + rdo.description + "']`; the template entry is dropped by the UI's `optionOf`, leaving only `000`.
  - `1701Q` `frm1701q:txtRDOCode` and `frm1701q:txtSpouseRDOCode` are `sel` with raw option `['[]']`; the malformed literal remains as an option.
  - `2551Q` and `0619E` are `sel` with no schema `o`, so the host supplies the full catalog.
  - `2550Q` is `text`, so the list mechanism is not used at all.

I did not treat the raw extracted schema as a reliable table for all 43 forms because the runtime builder applies corrections/reconstruction. The quick tested exceptions and the governing rule above are the reliable result: nonempty leftover schema script/options block the host catalog; empty schema lists receive the host catalog; text boxes are free text.

## 5. Intended behaviour and smallest fix

The code's own intent is that a profile-filled RDO is profile-backed and folded/locked unless explicitly edited on the return, while an editable empty profile RDO must still be completable because form validation requires a valid official RDO. A locked blank box with no path forward would make the return impossible.

The smallest fix consistent with the host design is the one described in the follow-up fix: treat schema option lists made only of extraction leftovers/template text/dummy placeholders as no list, so `catalogs::options()` supplies the host's full official RDO catalog for the broken `sel` boxes. The alternative UX (keep it locked and direct the filer to set RDO in the taxpayer profile) is larger and conflicts with the existing “Edit on this return” design; merely offering the full list is the smaller correction.

## Artifacts

- Video: `rdo-box-linux-validation.mp4`
- Semantic capture/scripts: `drive_rdo.py`, `probe_other_forms.py`, `probe-other-output.txt`, `other-forms-results.json`, `drive-log.jsonl`
- Runner/build/seed logs: `nsh-run.log`, `seed-build.log`, `seed-fixture.log`
- Scratch database retained locally but not pushed as evidence.

## Untested / caveats

- Click-through creation of a taxpayer with empty RDO is unproven because the Linux dev-host native menu item did not activate in this run; source and host behavior still establish that empty RDO is allowed by design.
- Picking a real RDO in 2551Q/0619E all the way through save/validation was not separately completed; the visible full list conclusively includes real RDOs.
- Error-free Review/readiness was not reached because the returns still had other mandatory fields empty.
- Android emulator and `/home/ubuntu/forms-validation/android-codebox-watch` were not touched.
