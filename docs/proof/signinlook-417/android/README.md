# Job 42 — Android proof of PR #417 (the sign-in look)

Phone emulator, API 35, x86_64. Debug APKs, kit pin 8c78cfd6 (`~/BuwizNsh/tools`), docs/relay.md dev recipe,
`EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1` plus loopback URLs only. Local mock account server only
(`adb reverse tcp:3008`). Network cut throughout: `ping 8.8.8.8` → "Network is unreachable" before the first sign-in.
Nothing non-local in the mock's log. Invented values only. Never queued anything.

- AFTER: 80fa1f98af8b2290afde32956d4fd3b031f7fbfe (feat/relay-filing-signinlook), APK sha256
  `c3cade6922de805f8ee0b085a6721ff7ac960ad67c374e6e99bae7276cf34b63`
- BEFORE: main 979714b2a4211e237083e2a504a24b953a58fb43, APK sha256
  `a79c39ba3829744b6ac7278f6b4d35818d0bd46bf273bb8e51f15fbdc5e4e09d`

## BIR_AUTH_BYPASS absent

`run-as` cannot read the package ("not debuggable"), so the keys are confirmed two ways instead:
1. The baked recipe (above, verbatim) lists exactly these keys: EBIR_TEST_ENV, BUWIZ_NO_KEYCHAIN, BIR_SFTP_DRY_RUN,
   BUWIZ_SERVER_URL, BUWIZ_ACCOUNT_URL, BUWIZ_TRUST_KEYS, BIR_DATABASE_PATH, BIR_RELAY_URL — no BIR_AUTH_BYPASS.
2. `BIR_AUTH_BYPASS=1` does not occur in the packaged libnsh.so, and behaviorally the app gates on the sign-in
   pages (a bypassed build opens Home with no account). Sign-in was completed with the mock's emailed code.

## Pictures

AFTER, light and dark, plus the extras. BEFORE, light.

| page | light | dark | notes |
|---|---|---|---|
| first page | after-light-01 | after-dark-01 | logo whole with its words, button close under it |
| email empty | after-light-02 (+02b) | after-dark-02 | |
| email keyboard up | after-light-03 | after-dark-03 | Gboard; field and Continue both in view |
| email bad address | after-light-05 (+04 typing) | after-dark-05 | red border + "That email address does not look right…" |
| code page just opened | after-light-06 (+06b) | after-dark-06 | caret in box 1, app keypad |
| wrong code | after-light-07-wrong1..4 | after-dark-07-wrong1 | "That code is not right. N tries left." |
| too many tries | after-light-07-wrong5 | after-dark-07-toomany | "Too many wrong codes. Send a new code to try again." |
| Checking… | after-light-10 | after-dark-10 | extracted from the sign-in videos' frames (screencap is too slow for the state) |
| after sign-in welcome | after-light-09 | after-dark-09 | green banner "Welcome. Your Buwiz account is ready." |
| BEFORE first/email/code | before-light-01/02/03 | — | old card look; no separate first page — the card IS the first page; code page reached |

## Extras at AFTER

- Keyboard up: after-light-03 / after-dark-03 (email), and the code page always shows its keypad (06 shots).
- System Back from the code page: first press dismisses the keypad, second goes to the email page **with the
  address kept** (nav-back-to-email2.png; nav-back-to-email.png shows the intermediate keypad-dismissed state).
- Back from the email page → the first page (nav-back-to-first.png).
- Back on the first page leaves the app: topResumedActivity becomes the previous app, eBIRForms no longer
  foreground (nav-back-leaves.png).

## Video

`after-video-email-code-home.mp4`: email → code (typed from the mock's emailed code) → Home/setup, one take.
The "Checking…" state is visible in its frames (after-light-10 is frame f0934). `after-dark-video.mp4` is the
dark sign-in (checking frame after-dark-10).

## The honest check on every AFTER picture

- Logo whole, with its words: **FAILS on the keyboard-up email page** — in after-light-03-emailpage-kbup.png,
  after-dark-03-emailpage-kbup.png and after-light-04-emailpage-badaddr-kb.png the logo's top (sun rays and the
  eagle's head) is cut off flat just under the Back row. The logo IS whole on the first page, the code page, the
  keyboard-down email page, the welcome page and the TIN+PIN pages in both themes.
  after-light-02b-emailpage-empty-kb.png shows the logo whole but lower on the page with no logo in the top part:
  that frame caught the layout still moving (mid keyboard-opening animation); the settled keyboard-up state is
  the clipped one in 03/04.
- Back control top left, inside the safe area, same place on every page, never floating above the logo:
  **yes** — "‹ Back" / "‹ Change email" sit at the same top-left spot on the email and code pages in both themes.
- Nothing falls off the bottom; with the keyboard up the field and its button are in view: **yes** —
  after-light-03/after-dark-03 show the email field and Continue above Gboard; the code page reserves its keypad.
- On the first page the button sits close under the logo: **yes** (01 shots, both themes).

Failing pictures: after-light-03-emailpage-kbup.png, after-dark-03-emailpage-kbup.png,
after-light-04-emailpage-badaddr-kb.png — all fail the "logo is whole" check with the keyboard up.

## TIN + PIN page

Reached after completing setup: taxpayer created through the UI (invented values: Demo, TIN 123-456-788-00000,
ZIP 1004 Malate, RDO 050 South Makati, phone 09171234567), then a 4-digit PIN set in Settings → Account →
Security ("PIN saved. Sign-in asks for it from now on."), then sign-out. The lock page replaced the email gate:

| mode | light | dark |
|---|---|---|
| TIN + PIN base ("Enter your TIN and your PIN.", both links) | after-light-12-tinpin.png | after-dark-12-tinpin.png |
| "Use an authenticator code" mode (6 authenticator boxes, "Use the PIN instead") | after-light-13-authenticator.png | after-dark-13-authenticator.png |
| "Forgot PIN?" mode ("The admin can open your account with their PIN.", admin-PIN boxes, "I remember my PIN") | after-light-14-forgotpin.png | after-dark-14-forgotpin.png |

One setup step: after-light-11-setup-step.png (the taxpayer created, Forms tab, "Demo added" toast).
The same checks pass on these pages: logo whole with its words, nothing falls off the bottom, links reachable.

## Method notes

- Taps inside each control's own bounds; digit boxes tapped in their own bounds.
- The wrong-code shots used 111111 against the mock's real code; tries decrement 4→3→2→1 then the too-many state.
- BEFORE has no separate first page in the old look: the sign-in card is the first page (before-light-01 == 02
  because navigation never left it on the first capture; both files are the card page).
