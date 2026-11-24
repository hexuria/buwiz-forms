# Reviewer walk — findings (eBIRForms on Windows, main 01942369, WebView2 158.0.4370.33)

Severity: reviewer-blocker / ugly / nit. Format: severity | step | what was seen | screenshot.

- ugly | Settings → Account | Dev-only strings a reviewer will see: "Server — Chosen for this build." pointing at http://localhost:3008 | 11-settings-account.png
- ugly | Form library | Library opens scoped to "Filed in this year (0)" → "0 of 43" empty page on first visit even though "All forms (43)" exists one tab over; a reviewer's first library view looks empty | 18-form-library.png
- ugly | After taxpayer creation | Registered name renders doubled — "Juana SantosJuana Santos" in the header pill, the library empty-state copy and the bottom-left account row (observed after profile creation; earlier run's 04-home.png shows it once — regression or entry-path dependent, worth checking) | 18-form-library.png
- nit | Sign-in gate | Gate asks to "Sign in" but nowhere says what Buwiz/eBIRForms is or why an account is needed; no version string anywhere on the screen | 01-signin-gate.png
- nit | Schedule 1 table | Grid cells render "Can't be filled on this page yet." — reads like a bug to a first-time user (values are entered via the row drawer instead) | 06-part2-totals.png
- nit | Settings → Account | Duplicated label "Not signed in / Not signed in." (heading repeats itself as the body text) | 11-settings-account.png
- nit | Settings → Account | "Gmail — Connect" would open a browser sign-in (not clicked); label gives no hint it leaves the app | 11-settings-account.png
- nit | Settings → Computer | Data folder shows dev-id path dev.goldcoders.bir; logs folder lives under Temp with 8.3-mangled user dir (C:\Users\ADMINI~1\...ebirforms-logs-7888\logs) — both read like dev artifacts | 15-settings-computer.png
- nit | Shortcuts | Ctrl+P and Ctrl+W do nothing on a form (no print, no close); Ctrl+K works (command palette), Esc works in dialogs, Alt+F4 works | 05-2551q-sample.png
- nit | Close flow | Alt+F4/× shows a "Close eBIRForms? Quit / Hide to tray" Ask dialog — works, but "Hide to tray" leaves a blue-"e" tray icon that looks like Microsoft Edge and sits in the overflow, hard to spot | 17-close-ask.png
- nit | Help → Check for Updates | Only feedback is a toast "Automatic updates are not configured in this build, so nothing is checked" — dev-build phrasing; fine for dev, odd in a shipped build | (toast; transient, no shot)
- nit | Help menu | "Contact Support…" would send email and "eBIRForms Website" would open a browser — noted, not clicked | 16-about.png (menu items listed under Help)
- nit | New taxpayer | RDO/ZIP pickers present a long scrollable sheet list; only discoverable workaround is typing in the search box — fine once found, but the row click targets don't respond to synthetic clicks | 03-new-taxpayer.png

Verified OK (no finding): first-run chooser wording clear; taxpayer creation (Juana Santos) smooth; Part II totals recompute correctly after Schedule 1 entry (250,000.00 @ 3% → 7,500.00); Check runs and reports results; print preview opens, scrolls to page 2 and closes cleanly (WebView2 installed); Ctrl+Q quits fully (window + tray icon + process all gone); About dialog shows "eBIRForms — Version 0.2.1 — © 2026 Goldcoders Corp"; Esc dismisses dialogs.
