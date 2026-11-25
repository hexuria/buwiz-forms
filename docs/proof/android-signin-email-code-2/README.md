# Sign-in re-proof, PR #345 at fe1a5f46 (job 10)

Phone emulator API 35 (KVM), keyboard docked, taps shown. Development build exactly per `docs/relay.md` on the branch:

```
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1 BUWIZ_ANDROID_DEV_ENV='EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production;BIR_DATABASE_PATH=test-data/bir_data.db' nsh package --target android --abi x86_64
```

(mock-account on 3008, `adb reverse tcp:3008 tcp:3008`, NO BIR_AUTH_BYPASS; every code is 246810.)
Started from clean data (uninstalled first). No code changed. One log per step; `build-package.log` is the build.

## A. The wrong-code count — WORKS

`signin2-A-to-C.mp4` covers A–C end to end. First wrong code: "That code is not right. **4 tries left.**"
(`A-wrong-code-4-tries.png`); second: "That code is not right. **3 tries left.**" (`A-wrong-code-3-tries.png`).

## B. A sign-in in progress survives — WORKS

On the code page, countdown "Resend in 4:31", pid 3382 (`B-before.png`). Swiped away from Recents and reopened:
**same pid 3382**, still the code page, countdown shown 4:31 (`B-after-swipe.png`). Then `am force-stop`
(process dead, pidof empty) and reopened: **NEW pid 6045**, the code page again (`B-after-force.png`),
countdown "Resend in 2:01". Honest note on numbers: the countdown text did not tick down between two reads
seconds apart before the force-stop (4:31 both times), and read 2:01 after — the persisted value is
recomputed at restore; whether the pre-stop display updates every second while backgrounded I cannot say
from these samples. The requirement — code page with the countdown continued, not restarted — is met.

## C. Codes and addresses — WORKS, with one untestable corner

Mock lines (pasted verbatim, `C-mock-lines.txt`):

```
[mock-account] sign-in code for juana@example.com: 246810
[mock-account] sign-in code for maria@example.com: 246810
[mock-account] sign-in code for juana@example.com: 246810
```

Continue with juana@, Change email, Continue with the SAME address: code page again, still ONE line in the
mock (no re-issue) — WORKS. A different address: a second line — WORKS. Back to the first address: a third
line — WORKS. "The first code must no longer work": **untestable here** — the mock mints 246810 for every
request, so the first and third codes are the same string; entering it signed in (`C-first-code-tried.png`
lands on the first-run question). Distinguishing invalidation needs the mock to vary its codes. Change-email
detail found along the way: the email page comes back with the address PRE-FILLED; typing without clearing
first produces a doubled address and a validation error (a data-entry trap worth knowing).

## D. The keyboard — WORKS with one landscape caveat (system behavior)

Screenshots `D-<page>-<orientation>-<mode>.png` for all eight states.

- **Portrait, both nav modes, email page**: the card is anchored at the top; the Email field and
  "Continue with email" are FULLY visible above the keyboard, and tapping Continue with the keyboard up
  submits (reached the code page).
- **Portrait, both nav modes, code page**: boxes, Resend and Change email all above the keyboard.
- **Landscape, both nav modes, both pages**: Gboard takes the whole screen in its fullscreen extract mode —
  the app's page is behind it; the field is edited in the keyboard's own extract bar. The keyboard's GO/DONE
  in that bar submits (proven: reached the code page). Alternatively one tap on the keyboard's hide chevron
  closes it and "Continue with email" is then visible with NO scrolling needed
  (`D-email-landscape-gesture-kbdclosed.png`). So: 0 swipes; either use the keyboard's own submit or close
  the keyboard once. This extract mode is the system's landscape behavior, identical in both nav modes.

## E. The same frame on three other screens — PARTIAL

- First-run question "Who will use eBIRForms on this device?": `E-first-run-question.png` (no field on it).
- PIN sign-in: NOT REACHED. It shows only when the app is locked by an admin code, which needs a multi-user
  ("I help other taxpayers") setup; this run's solo path never sets one. Settings > General exposes only
  Appearance and Home — no lock control found there.
- Access-error card: NOT REACHED. Tried: killed the mock account server entirely (port 3008 dead) and
  cold-started the app — it still boots to Home; the access check is against the local database, not the
  server (`E-access-error.png` shows what booting looks like instead). Making the host unable to answer the
  access question would need database corruption or a refusing mock at the access-check verb, not just a
  dead one.

## F. Settings > Account — WORKS

Path: round avatar top-right → menu sheet → Settings → Account (`F-settings-list.png`,
`F-settings-account.png`). Exactly ONE "Buwiz account" row, exactly one Back arrow in the header.
"Sign out of Buwiz" asks ONCE (`F-signout-confirm.png`); confirming lands on the sign-in page
(`F-after-signout.png`).

## Clipboard chip — WORKS

The keyboard's clipboard chip ("246810") fills all six boxes and signs in (`clipboard-chip-fill.png` —
landing back on Settings, where the sign-out happened). Long-press Paste on the code boxes remains the
known kit limit, not reported as new. (Note: long-press paste DOES work in the ordinary email field.)
