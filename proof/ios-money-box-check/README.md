# iOS money box check — 2551Q Schedule 1 Taxable Amount, iPhone 17 Pro

Commit 47ab5025 (main). Built with `nsh package --target ios-simulator`: nsh 0.6.1, ../nsh at the pin 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf, cargo 1.99.0 (rustup toolchain 1.99.0), build folder ~/BuwizNsh/target-moneybox (deleted afterwards). iOS 26.5 simulator, macOS 26.6.2. Signed in through the local mock_account (port 3008) because nsh package builds a release runner and release builds compile the auth bypass out; scratch database with only the demo taxpayer Juana Santos (TIN 000-123-453-00000, Individual, RDO 039, ZIP 1100). Nothing queued, no real data. Every digit was tapped on the iOS on-screen decimal pad as real taps through the Simulator window — no HID typing, no agent typing.

ANSWER: **NO — no wrong value is saved on iOS.** Commas appear while typing (the fourth digit writes "9,999") and the caret stays at the end: every digit landed in the order typed, in all three values.

- 99999999.99 → while typing and after leaving: 99,999,999.99; saved value after reopening the row: 99,999,999.99. Correct.
- 1000000.005 → the digits again landed in order: the box showed 1,000,000.005 — but with a loud red error "Exceeds the official 12-character input limit (at most 999,999,999,999 without centavos, or 999,999,999.99 with centavos); enter a smaller amount." and **Done greyed out**: the sheet cannot be closed with it, so nothing — right or wrong — can be saved. Completely unlike the Windows report (silent 0.01).
- 12345678.90 → 12,345,678.90 shown; saved after reopening: 12,345,678.90. Correct.

The iPad round was not done: it costs a full second install + sign-in + taxpayer setup and this session is at its conversation limit — not "cheap". The behaviors above are in shared UI code, not device-specific.

## Files
- val1-typing.mp4 — screen recording (simctl recordVideo, head trimmed): the decimal pad up, 99999999.99 tapped key by key with commas appearing as typed, Done, the row reading 99,999,999.99.
- val1-99999999-after-leaving.png — after leaving the box: row reads PT010 · 99,999,999.99 · 3,000,000.00 tax due. Saved value verified identical on reopening.
- val2-1000000.005-error-done-blocked.png — 1,000,000.005 as typed, the 12-character-limit error in red, Done disabled.
- val3-12345678.90-after-leaving.png — row reads PT010 · 12,345,678.90 · 370,370.37. Saved value verified identical on reopening.
