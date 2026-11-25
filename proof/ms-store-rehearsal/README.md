# MS Store rehearsal proof — buwiz-forms @ 47549f99 (PR #344)

Machine: buwiz-forms-win-3, Windows Server 2022 Standard build 20348, UAC off (EnableLUA=0), 1920x1080.
All app runs: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1. Nothing was sent to the BIR. Only the demo taxpayer. No real sign-in; nothing typed past the sign-in gate; Queue never pressed.

## Files

- `01-release-signin-gate.png` — RELEASE exe (unsigned msix, unpacked layout run with scratch LOCALAPPDATA): the sign-in gate. Shows the exact strings "Sign in with your Buwiz account to use eBIRForms on this computer." and "Buwiz is the account behind eBIRForms. We email you a 6-digit code to sign in; there is no password."
- `02-help-menu-no-self-update.png` — same release app, Help menu open: the ONLY item is "About eBIRForms". There is no "Check for Updates".
- `03-signin-pressed-device-code.png` — after pressing "Sign in" once: app shows "Finish in the browser, then return here. Code CRDB-WGFK.", buttons switch to "Waiting…".
- `04-browser-account-buwiz-device.png` — what the sign-in button opens. The default browser (Edge) was launched with the exact URL `https://account.buwiz.com/login?next=%2Fdevice%3Fuser_code%3DCRDB-WGFK` (verified in Edge's process command line; Edge cannot render any window on this UAC-off Server host — even a direct launch shows no window — so the same URL is shown here loaded in Chrome for Testing). It lands on a Cloudflare "Verify you are human" interstitial, which does not pass in an automation-flagged browser. What this does NOT show: the actual device-login page behind the interstitial. No email was typed anywhere.
- `05-first-run.png` — dev run first screen: "Who will use eBIRForms on this computer?" with "Just me or my business" / "I help other taxpayers".
- `06-new-taxpayer-form.png` — demo taxpayer filled: TIN 000-123-453, Individual, Juana Santos, Quezon City, ZIP 1100, RDO 039 - South Quezon City, 09170000000, juana@example.com.
- `07-form-library.png` — Form library for Juana Santos.
- `08-2551q-filled-saved.png` — 2551Q filled: Calendar, Year Ended 12/2026, Quarter 4th, Amended No; saved as return "2551Q Q4 2026 · Juana Santos" in Recent. Taxable amount 100,000.00 at ATC PT010 3% → tax due 3,000.00 (visible in 10-print-preview).
- `09-after-check.png` — after pressing the check button (its exact label is "Check"). The two attention banners (business start date, annual income-tax election) remain; Part I shows missing required items.
- `10-print-preview.png` — print preview on the official BIR 2551Q layout, A4 2 pages, all data including 3,000.00 totals.
- `11-settings-account.png` / `12-settings-account-security.png` — Settings > Account, both scroll positions: rows are Server (http://localhost:3008 in this dev build), Sign in/Not signed in, profile card with Sign out, Registered name, Email, Gmail + Connect button, Receipt tracking toggle, My PIN + Set PIN, My authenticator + Add, Recovery key. There is NO "Delete account" row anywhere on this page.
- `part4-walk.mp4` — screen recording of the whole Part 4 walk, first run to Settings > Account, sped up 2.5x to 96 seconds (original 250s at 8fps gdigrab). Includes the "Save as PDF" press which produced no dialog and no file (see report).
- `wack-report.xml` — Windows App Certification Kit output for the self-signed COPY of the msix (never the artifact). OVERALL_RESULT="PASS", 22 of 24 tests PASS; the 2 FAILs are both OPTIONAL=TRUE: [45] App resources (icon pngs fail exact tile size restrictions) and [88] Blocked executables (references to CreateProcessW/ShellExecuteW/cmd.exe/powershell.exe strings in eBIRForms.exe and bundle.nshc).
- `AppxManifest.xml` — unpacked from the unsigned artifact msix.
- `SHA256SUMS-windows-x64.txt` — copied verbatim from target/release-artifacts.
- `smoke-msix-output.txt` — full output of smoke_msix.ps1 re-run (same steps as -SmokeMsix): registration=passed; layout-run=passed; packaged-activation=skipped (UAC disabled).

## What this does not show

- No packaged-activation launch: impossible with EnableLUA=0 (smoke script itself skips it; a manual shell:AppsFolder launch attempt also started nothing).
- The real device-login web page (Cloudflare interstitial blocks automation browsers).
- No PDF file: "Save as PDF" in the dev print preview opened no save dialog and wrote no file on this machine (4 attempts, windows enumerated).
- No msix or installer is committed here, per instructions.
