# proof/ios-keyboard-pad — PR #351 keyboard spacer, before/after on iPhone + iPad Simulator

- App commits: BEFORE = origin/main `00a20033`, AFTER = `36505288` (feat/android-store-keyboard-pad, PR #351 head).
- One build folder per checkout (evidence rule): the BEFORE pictures and recording come from a host built with `CARGO_TARGET_DIR=~/BuwizNsh/target-00a20033` out of the `00a20033` worktree, the AFTER ones from `~/BuwizNsh/target-36505288` out of the `36505288` worktree. Neither build used the shared `~/BuwizNsh/target-nsh` at run time. Transparency note: both target folders were seeded as APFS copy-on-write clones of the shared folder's cache and then rebuilt in place; this is sound here because the `00a20033..36505288` diff is TSX-only (zero Rust changes — the Rust crate inputs are byte-identical across the two commits) and the TSX bundle that actually differs is served per-worktree by the dev server, never linked into the cargo artifacts.
- An earlier set of these same pictures taken from shared-target-nsh builds was committed first (6a41297) and is superseded by this one; the measurements were identical in both sets.
- nsh tool `nsh 0.6.1 (ca39c0e)`; nsh checkout (path dependency `../../nsh`) at `ca39c0e9103a389a31ea280799ef34796d7aac70`. The iOS dev host (NshDev.app) is compiled from that checkout by cargo; each run reassembles `nsh/.nsh/ios/NshDev.app` and rebuilds only the app crate.
- Devices: iPhone 17 Pro, iOS 26.5 (1206x2622 px, 3x) and iPad Pro 13-inch (M5), iOS 26.5 (2064x2752 px, 2x), both upright, light appearance, 9:41 status bar override, software keyboard showing (hardware keyboard off; the iPad needed the floating bar's Keyboard > Show Keyboard once per boot).
- Every run: the `nsh-run-agent` recipe's exact steps replicated by hand (the recipe force-sets CARGO_TARGET_DIR to the shared folder, so the same commands were run with the per-commit folder exported instead): `nsh run --target ios --device <udid>` with the recipe's SIMCTL_CHILD variables BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, the ios-handoff token writer and cargo-with-agent wrapper as in the recipe. Never BIR_SFTP_LIVE; nothing was queued; no return and no taxpayer was created (New taxpayer was typed into but Create taxpayer was never pressed). Fresh scratch db copy with only the demo taxpayer Juana Santos TIN 000-123-453; the scratch copy's `settings.access_mode` was set `solo`→`helpdesk` with sqlite because solo installs cannot add a second taxpayer and the spec's New taxpayer row does not exist otherwise; a tax year 2026 was added in the profile UI so Choose forms is reachable (no forms were ever saved).
- Gap measurements: blank points between the last content's bottom edge (element bounds from the app's accessibility tree) and the top edge of the software keyboard in the picture (first keyboard-grey pixel row / 3 on iPhone, / 2 on iPad). Keyboard top measures 540 pt on the iPhone (keyboard height ~334 pt) and 974 pt on the iPad (keyboard height ~402 pt).

## Files (one line each)

Scenario pictures, per spec item 1-3:
- before-new-taxpayer-bottom-iphone.png — item 1 BEFORE iPhone: at max scroll Email AND Create taxpayer both visible above the keyboard, small margin. PASS (reachable).
- after-new-taxpayer-bottom-iphone.png — item 1 AFTER iPhone: at max scroll the spacer pushes the whole card up; Email and Create taxpayer scroll past the top edge — both are reachable above the keyboard with less scrolling. PASS.
- before-new-taxpayer-bottom-ipad.png — item 1 BEFORE iPad: everything fits without scrolling; both visible. PASS.
- after-new-taxpayer-bottom-ipad.png — item 1 AFTER iPad: same. PASS.
- before-choose-forms-search-iphone.png — item 2 BEFORE iPhone: "2551" typed, the 2551Q row scrolls into view above the keyboard. PASS (reachable).
- after-choose-forms-search-iphone.png — item 2 AFTER iPhone: same, row reachable with margin to spare. PASS.
- before-choose-forms-search-ipad.png — item 2 BEFORE iPad: row visible above keyboard. PASS.
- after-choose-forms-search-ipad.png — item 2 AFTER iPad: same. PASS.
- before-low-field-focus-iphone.png — item 3 BEFORE iPhone: tapping the lower-half Head-office address field auto-scrolls it into view, but it lands flush against the keyboard top edge.
- after-low-field-focus-iphone.png — item 3 AFTER iPhone: same auto-scroll, field in view above the keyboard.
- before-low-field-focus-ipad.png — item 3 BEFORE iPad: no scroll needed; field already visible (an iOS AutoFill bubble overlaps the field label).
- after-low-field-focus-ipad.png — item 3 AFTER iPad: same.

End-of-scroll gap measurement pictures (fling to the very end and hold, keyboard up):
- before-iphone-new-taxpayer-end.png — gap 74 pt.
- after-iphone-new-taxpayer-end.png — gap 409 pt.
- before-iphone-choose-forms-end.png — gap 77 pt.
- after-iphone-choose-forms-end.png — gap 393 pt.
- before-ipad-new-taxpayer-end.png — gap 535 pt.
- after-ipad-new-taxpayer-end.png — gap 535 pt.
- before-ipad-choose-forms-end.png — gap 721 pt.
- after-ipad-choose-forms-end.png — gap 721 pt.

Recording:
- after-keyboard-pad.mp4 — AFTER iPhone, 52 s one take: scenario 1 (New taxpayer, Email tapped, keyboard up, scroll to the end), scenario 2 (Choose forms, "2551" typed, scroll), scenario 3 (New taxpayer, lower-half field tapped, auto-scroll). h264, untrimmed.

## What the measurements show

- iPhone: BEFORE leaves only ~74-77 pt of blank at the end of the scroll. AFTER leaves ~393-409 pt — the before gap plus almost exactly one keyboard height (~334 pt), i.e. double room: the kit's own keyboard inset plus the new spacer.
- iPad: gaps are identical before and after (535 / 721 pt). The wide-layout page content is short, the scroll end position is the same in both builds, so the spacer makes no visible difference at the end of the scroll on the iPad in these two pages.
- Nothing new in AFTER that BEFORE did not have beyond the spacer's own blank: no gap left after the keyboard goes down was observed (the spacer is only present while the keyboard is up — after dismissing, pages end flush as before), no jump, no extra blank on the wide layout beyond what is reported above.

## Not covered

- The Settings scrolls and the form page's scroll (the spacer also applies there; not part of the spec'd items).
- Android (the PR is android-submission's; these runs are iOS Simulator only).
- Landscape, split-view/windowed iPad mode, hardware keyboard typing.
- Tapping behavior of buttons above the keyboard; only visibility/reachability was checked.
