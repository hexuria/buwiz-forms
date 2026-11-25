# Sign-in: every state, as drawn

Made by `just nsh-shots signin` (the test `nsh/shots/signin.test.tsx`, real windows, fake host answers; nothing reaches a server). File names are `signin-<look>-<state>.png`.

- Looks: `wide-light`, `wide-dark` (1440×900), `narrow-light`, `narrow-dark` (390×844).
- States on the gate: `gate-start`, `gate-waiting` (the browser opened), `gate-no-browser` (nothing opened), `gate-cancelled`, `gate-expired`, `gate-failed`.
- States in Settings > Account: `settings-waiting`, `settings-no-browser`.

What these pictures do not show:

- The narrow looks are drawn by the macOS build at phone width. A real phone says "this device" where these say "this computer", and stacks the two buttons at full width with touch-size height. Only a Simulator or an emulator shows that.
- The moment a button shows its spinner (one request long) has no picture.
- "Test: approve" and "Test: expire" in the corner belong to the test page, not to the app.
