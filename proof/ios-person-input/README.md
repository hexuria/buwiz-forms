# iOS person-input checks — money box digit entry and the row-editor sheet

App commit: 2e152f0fc06f40d54c2984d13381f6f34f83d478 (feat/ios-store-tablet)
nsh --version: nsh 0.6.1 (ca39c0e)
../nsh commit: ca39c0e9103a389a31ea280799ef34796d7aac70
cargo --version: cargo 1.99.0 (5f94df478 2026-08-27)
Build folder: ~/BuwizNsh/target-2e152f0f (fresh per commit, deleted after reporting)
Safety: SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1, SIMCTL_CHILD_EBIR_TEST_ENV=1, SIMCTL_CHILD_BIR_AUTH_BYPASS=1, SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 throughout; scratch database holding only the demo taxpayer Juana Santos; Queue never confirmed.

## Check 2 — what a person gets in a money box (iPhone 17 Pro, iOS 26.5)

Driven through the accessibility bridge's on-screen key taps (the keyboard's digit
keys pressed as accessibility buttons, the same mechanism used for the "q" press in #345).

- `check2-after-tap.png` — right after the tap on Part II item 15: the box is focused with the insertion cursor at the END of the pre-filled "0.00"; the text is NOT selected/highlighted.
- `check2-tap-with-kb.png` — the same moment with the software keyboard up (summoned with Device > Keyboard > Toggle Software Keyboard).
- `check2-after-keys.png` — after tapping the on-screen keys 1, 0, 0: the box reads "100". The first digit REPLACES the zero-fill ("1" appeared, item 18 computed 1.00), further digits append normally. A person never gets "0.00100"; that string is an artifact of synthetic type() in gpui-agent/the headless harness, which types into the raw text instead of through the field's digit-entry path.

## Check 3 — the row-editor wedge (iPad Pro 13-inch (M5), iOS 26.5)

IMPORTANT INPUT NOTE: these steps were driven with REAL MOUSE INPUT in the
Simulator device window (clicks and drags go through the simulator's normal touch
hit-testing — the closest thing to a finger), because the accessibility bridge
could not observe the iPad at all ("Failed to get frontmost application" even with
the app running frontmost; the same bridge worked on the iPhone). simctl has no
tap-injection verb on this Xcode and idb is not installable on this machine.

- `check3-editor-open.png` — the Schedule 1 row editor sheet open on the 2551Q draft (Row 1, ATC PT010, Taxable Amount 0.00).
- `check3-typing.png` — typing the amount with real taps on the on-screen keyboard: the digit "1" landed in Taxable Amount and Tax Due computed 0.03 live — the sheet content is alive under real input.
- `check3-sheet-closed.png` — after pressing Done with a real tap: the sheet closed cleanly and the app is fully responsive.

Verdict: a person is not stuck in the row editor. The wedge (every press reported
"offscreen"/"occluded" and the sheet unclosable) is specific to gpui-agent's
synthesized input channel, not something a person hits. Real mouse-drag scrolling
also works everywhere on both devices, while gpui-agent's swipes often did not.
