# CheckRow and sign-in pictures at 010483b2

- app commit: 010483b2da13773521976b5f6c0037240ae8ff2a (feat/ios-store-tablet with main d6cf0518 merged in — #350's CheckRow plus the tablet layout together)
- `nsh --version`: nsh 0.6.1 (8c78cfd)
- ../nsh commit: 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf (the NSH_PINNED_REF in this commit's .github/workflows/nsh.yml)
- cargo: cargo 1.99.0 (5f94df478 2026-08-27), stock rustup
- build folder: ~/BuwizNsh/target-010483b2 (fresh for this commit, deleted after the run)
- devices: iPhone 17 Pro and iPad Pro 13-inch (M5), both iOS 26.5 simulators; scratch databases only; the four SIMCTL_CHILD safety variables; never BIR_SFTP_LIVE; the mock address juana@example.com was typed but Continue was never pressed — no network sign-in.

## CheckRow pictures (taxpayer Forms tab, edit mode, 2551Q row)

On both devices the row is one control: a single checkbox node spanning the row's full width, 58 points high (the 44 pt minimum is met). Both taps were verified in the live UI tree as well as in the pixels:

- `iphone-checkrow-before.png` / `ipad-checkrow-before.png` — the row as it is in edit mode: box checked (the form is filed in 2026), "1 selected" in the header.
- `iphone-checkrow-text-tap.png` / `ipad-checkrow-text-tap.png` — after one tap on the row's TEXT (the "2551Q" name, not the box): the box is unchecked and the header reads "0 selected". The tap toggled exactly once — the agent's readback confirmed the flip from checked to unchecked, never a double toggle (which would have left it unchanged).
- `iphone-checkrow-right-tap.png` / `ipad-checkrow-right-tap.png` — after one tap on the row's empty right-hand part: the box is checked again — again exactly once.

On the iPad the software keyboard is up because the search field keeps focus; it covers nothing of the row. The iPad's tax profile opens as a dialog over Home in the tablet layout, then expands to the full page.

## Sign-in email page

- `iphone-signin-email.png` / `ipad-signin-email.png` — Settings → Account → Sign in: "Sign in to eBIRForms", the 6-digit-code sentence, the Email field holding the mock address juana@example.com, and "Continue with email" (never pressed). On the iPad the card is width-limited and centered at tablet width.
