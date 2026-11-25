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
  - `code-typing`: three digits in.
  - `code-checking`: "Checking…" while the sixth digit's check runs.
  - `code-wrong`: a wrong code, with the tries left.
  - `code-expired-resend`: the code expired; Resend unlocked.
  - `code-too-many`: five wrong tries; the one action "Send a new code".
  - `code-restored`: opened straight on the code page after a relaunch, with what is left of the countdown.
- Settings > Account, signed out: `settings-email` (Sign in leads to the same email page) and `settings-code`.

What these pictures do not show:

- The narrow looks are drawn by the macOS build at phone width. A real phone says "this device" where these say "this
  computer", stacks the buttons at full width with touch-size height, brings up the email keyboard and, on the code page,
  the number pad with iOS's one-time-code suggestion. Only a Simulator or an emulator shows that.
- Home after the right code: it is the app's Home, pictured with the other screens.
