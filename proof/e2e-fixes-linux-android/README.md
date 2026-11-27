# e2e smoke suite fixes, Linux + Android (job report)

App: hexuria/buwiz-forms main 6bd529d0 + this branch. Suite: `just e2e <target> smoke --allow-send` (PR #428 merged).
CLI: gpui-agent-cli v0.4.1 built from hexuria/gpui-agent rev 5867457a364b5c27d208fb729e374983c50cf8fb for BOTH suites
(the paced money-box variants ran, never the fallback).
Safety on every run: EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1, scratch database per run
(removed after), mock relay only, nothing external. The GPUI_AGENT_TOKEN was random per shell, never printed or stored.

`android-final/` and `linux-final/` are the evidence folders of the last full pair of runs (report.md inside each).

## What changed in the recipes, and why (all recipe-wrong unless noted)

1. `ui-sign-up-phone.wants`: the ZIP/RDO sheet steps removed — on Android NO node inside a bottom sheet is pressable
   (gap 1, see below). Empty zip/rdo are valid (factError returns null on empty), so the taxpayer is created without them.
   Ending fixed: after Create the app lands on the taxpayer's OWN page (not Home), so click tab-home, then assert the tile.
2. `sign-up.wants` (desktop): same landing fix. `type` into the TIN digit cells RACES on Linux (evidence: the cells read
   124-567-880 after typing 12345678800000, check-digit error, sign-up blocked — linux-20261127-083229/02-sign-up/failure.png);
   all fills are set_value now, as the phone variant already did.
3. New suite param `tin_root` (123456788): home-tp-<id> uses the 9-digit root, the recipes used the 14-digit param.
4. `open-2551q-phone.wants` rewritten: the narrow palette searches TAX PROFILES ONLY (never forms) — the old palette path
   could never reach a form. New path: nb-search > type the name > Enter opens the taxpayer (the Home tile itself is
   OFFSCREEN below the fold and the adapter refuses it: "`node` is not pressable", virtual AND semantic, so the palette
   is the only way back); sign-up started from "Nothing yet", so a tax year is added and 2551Q ticked in Edit forms first;
   then nb-new > new-return-2551Q > Open.
5. `birth-date-phone.wants`: same palette path to the taxpayer.
6. `smoke.json`: linux variants added for navigate/birth-date/open-2551q/sign-out pointing at the narrow recipes — the
   Linux window is 420x834 (photo evidence), the app is responsive and shows the NARROW layout, so the wide-desktop ids
   (nav-home, tb-panel, sb-new) do not exist on Linux. `scripts/e2e/runner.py` learned the `linux` variant key
   (one-line mirror of the windows key); the wide desktop files are untouched for real desktops.
7. Suite order: birth-date moved behind queue. Its known-gap failure leaves the date sheet OPEN, and the next recipe's
   first click is then occluded_by the sheet (both platforms, evidence in the 083229 runs).
8. `justfile` (the review's loopback fix): host check is now `^https?://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?(/|$)` —
   the old glob passed `http://localhost:80@evil.example` (userinfo hole) and localhost.evil.example.
9. `scripts/setup-linux-nsh.sh` + the live wrapper: the caller's launch environment passes through the docker wrapper
   (without it the Linux-built nsh ran WITHOUT the agent feature and the control plane never answered).

## (b) gpui-agent / adapter cannot — with evidence

1. GAP 1, sheets on Android: nothing inside a bottom sheet has a native view. `set_value` on the ZIP search AND a direct
   click on a VISIBLE option row both refuse "`node N` is offscreen (it has no native view)"; `--delivery semantic` the same;
   `in_viewport` is unavailable ("no window clip"). (android-final birth-date, and runs 073044/073929.)
   NOTE the nuance: in the birth-date sheet `set_value` into the sheet input DID land (s13 passed) — read/write works,
   only PRESSES refuse. s14 `click tp-taxpayer-birth_date-sheet-done` = "`node` is offscreen".
2. Same on Linux GTK: birth-date s14 sheet-done click "`node` is offscreen" — so on linux birth-date is a REAL fail
   (no gap marked for desktop), not gap 1 as the suite assumed phone-only.
3. No scroll op exists in the recipe engine at all, and clicks on below-the-fold nodes refuse "not pressable" —
   the Home taxpayer tile is unreachable once the home page scrolls it under the tab bar (both platforms).
4. A recipe ending mid-gap leaves the app dirty: the open sheet occludes the NEXT recipe's first press (occluded_by).

## (c) app misbehaves

1. TIN digit cells lose/reorder fast typed digits on Linux (typed 12345678800000, cells show 124-567-880) —
   same class as the money-box key race. Worked around in the recipe with set_value; the app behaviour stays.
2. The Linux window is 420 px wide regardless of the 1440x900 window config — the app is well-behaved (responsive),
   but every Linux run is effectively the phone layout.


## Final results — one full run per platform after the fixes (evidence folders here; gpui-agent-cli 0.4.1 rev 5867457a ran BOTH)

### Android (KVM emulator, API 35 x86_64, phone layout)
| recipe | result | time | failing step |
|---|---|---|---|
| setup-start | pass | 3.1 s | |
| sign-up | pass | 11.0 s | |
| navigate | pass | 5.8 s | |
| open-2551q | pass | 14.4 s | |
| money-boxes | fail | 4.0 s | s2 target_gone fp-frm2551Qv2018:txt16 |
| delete-row | setup | | $table regex matched nothing (no fp-table in the culled tree) |
| queue | fail | 15.6 s | queue-continue never appears: the empty return stops at its to-do list, as the suite's own precondition warns |
| birth-date | known gap | 12.2 s | s14 sheet-done click offscreen (gap 1) |
| sign-out | known gap | 2.4 s | s1 occluded_by the date sheet birth-date left open |

### Linux desktop (GTK window, but 420 px wide so the NARROW layout)
| recipe | result | time | failing step |
|---|---|---|---|
| setup-start | pass | 0.5 s | |
| sign-up | pass | 9.7 s | |
| navigate | pass | 2.0 s | |
| open-2551q | pass | 7.4 s | |
| money-boxes | fail | 1.0 s | s2 target_gone fp-frm2551Qv2018:txt16 |
| delete-row | setup | | same culling |
| queue | fail | 12.9 s | same to-do stop |
| birth-date | FAIL | 5.1 s | s14 sheet-done click offscreen — gap 1 is NOT phone-only, Linux sheets have it too |
| sign-out | fail | 0.3 s | occluded_by the leftover date sheet |

## Remaining root causes (all (b) agent-cannot unless noted)

1. THE one blocker behind money-boxes/delete-row/queue on both platforms: the adapter's tree culls nodes outside the
   viewport's reach and the recipe engine has NO scroll/swipe op (the host features list scroll/swipe/drag, the recipe
   language cannot use them). txt15/txt16 sit far down the long form, behind the profile-prerequisite banners
   (evidence: android-final/05-money-boxes/failure-tree.json stops at fp-set-profile-business_start_date; the form
   fields never enter the tree). Until recipes can scroll, long forms are unreachable on any narrow viewport.
   queue's failure is a CASCADE: with no amounts typed the return fails Check and Queue stops at the to-do list —
   the suite's own precondition sentence predicts exactly this.
2. Gap 1 (sheet presses) is real on BOTH platforms, not only phones: the birth-date sheet input takes set_value but
   every press inside the sheet refuses "offscreen". Sheets left open by a gap-ending recipe occlude the next recipe
   (sign-out's occluded_by). On Android sign-up's ZIP/RDO sheets are entirely unpressable (even a visible option row,
   even semantic delivery); on Linux the desktop comboboxes work because they are popovers, not sheets.
3. (c) app: the TIN digit cells reorder fast-typed digits on Linux (typed 12345678800000 -> 124-567-880 in the cells,
   check-digit error blocks Create; linux-20261127-083229/02-sign-up/failure.png in the run history). Worked around in
   the recipe with set_value; the reordering itself looks like the same key race the money-box work targeted.
4. (c) or infra: the Linux window is 420x834 although nsh.json asks 1440x900 — every Linux desktop run is the narrow
   layout. Wide-desktop recipes (nav-home, tb-panel, sb-new) can never run there; the suite now picks narrow variants
   for linux through the new runner variant key.

## Left out of the integration PR

`fix/e2e-recipes-all-platforms` carries this folder without the recordings (`recording.mp4`) and the per-step
screenshots (`NNN-sN.png`), 14.4 MB together: they stay on `try/e2e-fixes-linux-android` at `53e241c5`. The
`linux-20261127-083229` and other earlier runs cited above were never pushed.
