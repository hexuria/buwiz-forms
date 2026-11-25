# Proof: no Google client id means no Gmail sign-in (PR #343)

Desktop app (macOS), demo taxpayer Juana Santos (TIN 000-123-453), fresh scratch
databases, the four safety variables on every run. Connect was never pressed;
nothing was signed in to.

| File | Commit | Shows |
|---|---|---|
| a-before-inbox.png | 4399ce84 (main) | Tax profile > Confirmation inbox, no client id: the method choices "Google account (OAuth2)" / "App password" |
| a-before-settings-google.png | 4399ce84 (main) | Settings > Account > Google, no client id: "Gmail — Not connected. Reads BIR confirmation mail for Head office." with a Connect button |
| b-after-inbox.png | fede0133 | The same inbox, no client id: no method choices and the sentence "Google sign-in is not available in this version. Use an app password." |
| b-after-settings-google.png | fede0133 | Settings > Account > Google, no client id: the same sentence, no Connect button |
| c-after-with-id-inbox.png | fede0133 | The same inbox with GOOGLE_CLIENT_ID/GOOGLE_CLIENT_SECRET set to made-up values: the method choices are back, as on main |
| c-after-with-id-settings-google.png | fede0133 | Settings > Account > Google with the made-up values: Connect is back |
| state-b-flow.mp4 | fede0133 | 73 s, the app's window only: open the app, open the taxpayer's profile, the Confirmation inbox with the sentence, then Settings > Account > Google |

What this does not cover: pressing Connect (never pressed in any state), a real
Google sign-in, the mobile apps, and state A/C beyond these two screens.
