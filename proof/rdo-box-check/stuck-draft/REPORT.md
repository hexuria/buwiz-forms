# Stuck 1601C RDO draft validation — report only

Exact revisions:
- PR #399/deef worktree `/home/ubuntu/forms-validation/rdo-stuck-deef60d8`: `deef60d83efbe1bf6b82a476d407595973d1d82f`
- MAIN worktree `/home/ubuntu/forms-validation/main-134c2e4e`: `134c2e4e1d159feafebf500a4030c8b64a3f4ac6`

Targets/proof:
- deef target `/home/ubuntu/forms-validation/target-rdo-stuck-deef60d8`
- main target `/home/ubuntu/forms-validation/target-main-134c2e4e`
- proof `/home/ubuntu/forms-validation/rdo-stuck-proof`

Real visible maximized Linux X11 NSH windows were used. The semantic tree located/read controls; clicks were real coordinate clicks. Safety environment only (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`); no `BIR_SFTP_LIVE`, no Queue, invented values only, no token printed/stored. Android emulator/watch untouched. No product files changed, no commit/push.

## Data setup, exactly

Starting point was `/home/ubuntu/forms-validation/rdo-fix-proof/rdo-fix-validation.db`, copied to:

- `/home/ubuntu/forms-validation/rdo-stuck-proof/main-validation.db`
- `/home/ubuntu/forms-validation/rdo-stuck-proof/deef-validation.db`

In both scratch DBs: drafts deleted; Invented NoRdo Person `00011124500000` profile `rdo_code` set empty. Then MAIN was opened in the real window, a new 1601C November 2026 draft was created, and the old picker was used to pick `000`. That exact row was exported to `main-source-draft.json` before manipulation.

Direct scratch setup then:
- cloned that row to January 2026, RDO `000`, and removed `profile_baseline`/`profile_edits` to make S1 an old-style pre-baseline draft;
- cloned it to February 2026, RDO empty, baseline `/rdo_code` empty, no profile edit, for S2;
- copied January/February rows into the deef DB;
- created March 2026 in the deef DB with RDO `040`, baseline `/rdo_code` empty, and `profile_edits=["frm1601c:txtRDOCode"]`, representing a filer-edited RDO;
- set profile `rdo_code` to `050` in both DBs.

Caveat: S2/S3 were exact state-machine fixtures rather than being typed/picked interactively in separate forms. This is material: runtime open behavior was real, but creation of those two states was direct scratch setup as permitted. S1's `000` itself really was picked on MAIN before the legacy metadata was stripped.

## S1 — deef, old-style draft holding 000

Before open: stored `rdo_code` `000`, no profile baseline/edit metadata.

On reopen after profile became `050`:
- top notice: `Differs from your profile: RDO 000 (profile 050)`
- the page offered `Use profile values`
- Item 7 was folded/locked and visually a blank grey box; semantically `RDO code, required: empty, from tax profile`
- `Edit RDO Code on this return` existed
- validation after Check: exactly `Please enter a valid RDO Code on Item 7.`
- merely opening did not change the stored draft: it stayed `000`

Through Edit, the searchable real catalog opened. Search found `050`; picking `050 — South Makati` was accepted. After the pick:
- the RDO validation message disappeared;
- stored draft became `050`;
- after leaving/reopening, Item 7 still showed `050 — South Makati` locked/from profile.

So the return can be made valid without leaving the form, either by Edit/catalog or by the offered `Use profile values` path. The important difference from MAIN is that deef's Edit catalog offers the real RDO list.

Evidence: `deef-s1-open-rdo.png`, `deef-s1-profile-expanded.png`, `deef-s1-validation-before-fix.png`, `deef-s1-edit-open-control.png`, `deef-s1-edit-catalog-050.png`, `deef-s1-after-pick050.png`, `deef-s1-validation-after-pick050.png`, `deef-s1-reopen-after-fix.png`.

## S2 — deef, empty unedited RDO while profile had no RDO

Before open: stored RDO empty, baseline `/rdo_code` empty, no edit.

On reopen after profile became `050`:
- top notice: `Updated from your tax profile: RDO blank → 050`
- Item 7 showed `050 — South Makati`, locked/from profile, with Edit available
- the stored draft changed to `050` and baseline became `/rdo_code: "050"`

So an unedited empty draft follows the profile by itself.

Evidence: `deef-s2-open-rdo.png`.

## S3 — deef, filer-picked 040 while profile had no RDO

Fixture state: stored `040`, baseline `/rdo_code` empty, `frm1601c:txtRDOCode` in `profile_edits`.

On reopen after profile became `050`:
- draft kept `040`; it did not change to `050`
- Item 7 showed `040 — Cubao`, locked/folded, with Edit available
- no top notice told the filer that the profile was now `050` or that the return differed from it
- misleadingly, the box/profile chrome still used the same `from tax profile` wording even though `040` was not the current profile value

Evidence: `deef-s3-open-rdo.png`.

## S4 — deef, new 1601C after profile has 050

A new 1601C opened with:
- Item 7 `050 — South Makati`
- locked/folded in the `From tax profile` group
- note `From tax profile. Edit changes this return only; the profile stays as it is.`
- `Edit RDO Code on this return` available

Because a brand-new draft does not autosave before any edit, this was visual/semantic open-state evidence; no November row existed yet.

Evidence: `deef-s4-new-rdo.png`.

## S5 — MAIN comparison

MAIN at `134c2e4e` already has the same baseline/follow machinery.

S1 legacy `000` on MAIN:
- stayed `000` on open;
- showed `Differs from your profile: RDO 000 (profile 050)` and `Use profile values`;
- Item 7 visibly showed `000` locked/from profile (unlike deef, whose invalid catalog value displayed as blank);
- `Edit RDO Code on this return` existed, but its list offered only `Choose…` and `000`, no search and no `050`.

Thus the stuck state is present on MAIN too, and MAIN's Edit path cannot choose the real profile RDO; the separate `Use profile values` button is the visible in-form repair there.

S2 empty unedited on MAIN:
- followed the profile exactly as deef did: notice `Updated from your tax profile: RDO blank → 050`;
- Item 7 became `050` locked/from profile;
- stored draft changed to `050`.

Evidence: `main-s1-open-rdo.png`, `main-s1-edit-options.png`, `main-s2-open-rdo.png`.

## S6 — code explanation, concise

In `/home/ubuntu/forms-validation/rdo-stuck-deef60d8/crates/bir-app-api/src/screen_form_module.rs`:

- `screen_form_open` builds the draft and a separate `from_profile` draft. For a brand-new return it first applies carry-forward, then calls `follow_profile` on every non-sample open, not only at creation.
- `follow_profile` only acts for a Draft that has `profile_baseline`. For each profile-backed box not listed in `profile_edits`, it applies the current profile value only when the profile changed since baseline and the draft still holds the baseline value. A value in `profile_edits` is never replaced. If a stored draft changed, open quietly saves it and the page receives `profile_updates` such as `RDO blank → 050`.
- `profile_differs` handles a Draft with no baseline: it never changes the draft on open. It lists differing unedited profile boxes for the page banner (`RDO 000 (profile 050)`) and the page offers `Use profile values`; `use_profile_values` applies current profile values and records the baseline.
- Locking is host+page split. The host emits `locks`, `profile`, and `profile_edit` in `opened_json`. In `nsh/screens/form/session.ts`, `isLocked` honors a host lock unless the key is in `profileEdit` and is currently in the session's `editing` set. `startProfileEdit` adds the item's keys to `editing`/`opened`; ending Edit keeps the value but restores the locked/profile look.
- Therefore a locked box differing from profile has two in-form repairs when the metadata allows them: the banner's `Use profile values`, and item-level `Edit ... on this return`. On deef, Edit opens the real catalog; on MAIN's 1601C RDO list it did not.

## Machine-readable results

`results.json` contains exact before/after draft metadata, UI strings, method, and code summary.

Recording copied here as `rdo-stuck-validation.mp4`.
