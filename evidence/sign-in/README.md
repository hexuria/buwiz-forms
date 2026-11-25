# Sign-in: every state, as drawn

Made by `just nsh-shots signin` (the test `nsh/shots/signin.test.tsx`, real windows, fake host answers from
`nsh/screens/auth/testEmail.ts`; nothing reaches a server). File names are `signin-<look>-<state>.png`.

The sign-in is two pages, then Home: the email page (one page for signing in and creating an account) and the code page.
There is no other screen: no start card and no browser step.

- Looks: `wide-light`, `wide-dark` (1440×900), `narrow-light`, `narrow-dark` (390×844).
- The email page on the gate:
  - `email`: empty, as the app opens.
  - `email-bad-address`: an address that is not one, said under the field (no request).
  - `email-busy`: "Continue with email" with its spinner while its request runs.
  - `email-refused`: the server's sentence (here `code_already_sent`).
  - `email-not-available`: the server does not offer it ("Sign-in is not available right now. Try again in a few minutes.").
  - `email-no-connection`: "No connection. Check your internet and try again."
- The code page on the gate:
  - `code`: just opened ("We sent a 6-digit code to …", Resend locked with its countdown, Change email).
  - `code-expired-resend` shows the same control unlocked as "Resend code".
  - `code-typing`: three digits in.
  - `code-checking`: "Checking…" while the sixth digit's check runs.
  - `code-wrong`: a wrong code, with the tries left.
  - `code-expired-resend`: the code expired; Resend unlocked.
  - `code-too-many`: five wrong tries; the boxes stay, dimmed, and the same control now says "Send a new code".
  - `code-restored`: opened straight on the code page after a relaunch, with what is left of the countdown.
  - `email-after-change`, `code-after-change`: Change email, then Continue with the same address: back on the code page with
    the countdown going on (nothing sent).
- Settings > Account (the sign-in pages are the gate's only): `settings-account-signed-in` ("Buwiz account", the email,
  "Sign out of Buwiz"; under it "Profile on this computer"), `settings-account-sign-out-question` (the question before it
  signs out), `settings-account-signed-out` ("Not signed in to Buwiz" with Sign in, which shows the gate).

What these pictures do not show:

- The narrow looks are drawn by the macOS build at phone width, with a pointer's sizes (32-point controls). The touch look
  cannot be drawn here: `nsh test --target` draws a phone only in a Simulator or on an emulator (`--target catalyst` is not
  a test target, and a phone platform on `--target macos` is refused). A real phone says "this device" where these say
  "this computer", draws the field, the buttons and Change email 44 points tall and the boxes 48 (the kit's touch sizes),
  stacks "Resend…" over "Change email" at full width, anchors the card near the top instead of centring it, brings up the
  email keyboard and, on the code page, the number pad with iOS's one-time-code suggestion.
- Home after the right code: it is the app's Home, pictured with the other screens.

## Before / after (2026-11-25, `before/` holds the earlier narrow pictures)

Each pair is `before/signin-<look>-<state>.png` against `signin-<look>-<state>.png`, for `narrow-light` and `narrow-dark`.

- `code`, `code-typing`: "Change email" and "Resend" were right-aligned under centred boxes, Change email first; now the one
  new-code control and Change email sit in a centred row, the new-code control first. The empty line under the boxes keeps
  its room.
- `code-wrong`: the red line used to push the card's top up as it appeared (the card is centred); the line now fills room
  kept for it, so the boxes stay where they were (compare the card's top with `code`).
- `code-expired-resend`: a small right-aligned "Resend" became the same control, in the same place, saying "Resend code".
- `code-too-many`: the boxes vanished and a full-width "Send a new code" took their place with no Change email; now the
  boxes stay (dimmed), the same control says "Send a new code", and Change email is still there.
- `email`, `email-refused`: the line under the field keeps its room, so the sentence appearing no longer moves the field
  and the button; the refusal says its wait.
- `settings-email`, `settings-code` are gone (Settings has no sign-in pages now); in their place
  `before/signin-<look>-settings-account-signed-in.png` / `-signed-out.png` (wide and narrow) against the new
  `settings-account-signed-in`, `settings-account-signed-out`: the heading "Buwiz" became "Buwiz account", "Sign out"
  became "Sign out of Buwiz" (it now asks first: `settings-account-sign-out-question`), "Not signed in" became "Not signed in
  to Buwiz", and the profile card has its own heading, "Profile on this computer".
