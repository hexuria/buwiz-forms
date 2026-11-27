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

## Final results — see report.md in each evidence folder for the authoritative tables.
