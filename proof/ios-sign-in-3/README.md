# proof/ios-sign-in-3 — PR #345 sign-in re-check at 140ea4ba, iPhone 17 Pro Simulator

- App commit `140ea4bac783b389f7bcd4c75eed06c640b3de25` (feat/relay-filing-signin head).
- `nsh --version` of the recipe's CLI (~/BuwizNsh/tools/bin/nsh): `nsh 0.6.1 (ca39c0e)` — the CLI was NOT rebuilt for this run; only the kit checkout moved, as instructed.
- ../nsh checkout at build time: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the new pin).
- `cargo --version`: `cargo 1.99.0 (5f94df478 2026-08-27)`.
- Build folder: `~/BuwizNsh/target-140ea4ba`, created empty for this job (cold build; the shared `target-nsh` and the #351 per-commit folders were deleted first, so nothing compiled from another checkout could be linked). The mock account server compiled into `~/BuwizNsh/target-mock` (tooling, not evidence).
- Device: iPhone 17 Pro, iOS 26.5, light appearance, hardware keyboard disconnected (software keyboard shows), 9:41 status bar override.
- Variables at every launch (SIMCTL_CHILD_*, nothing else): EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1, BUWIZ_SERVER_URL=http://localhost:3008, BUWIZ_TRUST_KEYS=dev-test-key-not-production, BIR_DATABASE_PATH inside the app container's Documents. NOT set, on purpose: BIR_AUTH_BYPASS. Never BIR_SFTP_LIVE; nothing was queued; no return created. The only server is the mock on this Mac (`just mock-account --port 3008` from the 140ea4ba worktree).
- Scratch database: a copy of the demo database (Juana Santos, TIN 000-123-453, access_mode helpdesk) checkpointed with `PRAGMA wal_checkpoint(TRUNCATE)` before copying so no rows stay in an unmerged WAL — an earlier copy that skipped this lost the taxpayer rows and the app showed the first-run card after sign-in; reseeding fixed it.
- Codes: the mock now issues one code per request. Observed lines: `sign-in code for juana3@example.com: 246810`, `juana4@example.com: 246811`, `juana5@example.com: 246812` — consecutive per-request values, no longer a constant. Every code typed in this run was read from these lines.

## Files

- `land-email-scrolled.png` — item 1. iPhone sideways, email page, keyboard up, after scrolling the card: "Continue with email" is fully visible above the keyboard. PASS: the card scrolls (at fe1a5f46 it did not); tapping the button right after this shot, with the keyboard still up, submitted and the code page appeared.
- `portrait-email-keyboard.png` — item 2. Upright email page, keyboard up, address typed: field and button both above the keyboard, same as the earlier kb-iphone-email-portrait.png at fe1a5f46 — nothing moved. PASS.
- `keyboard-down-after-sign-in.png` — item 3. Home right after the correct code (typed digit by digit): the keyboard is down. PASS.
- `change-email-selected.png` — item 4. On the code page "Change email" was pressed: back on the email page the whole address is selected (blue highlight, selection handles). Typing a letter replaces it all — verified with a real key tap through the simulator's accessibility bridge: pressing the on-screen "q" turned the field's content from "juana3@example.com" into "q". PASS. Caveat: driving the same letter through gpui-agent's `type` command instead appends (that command inserts text without honoring the selection) — a limitation of the automation tool, not the app; the human-path key tap replaces as expected.
- `flow-landscape-and-change-email.mp4` — 57 s, one take, everything sideways: email page with the keyboard up, the address typed key by key, the scroll that brings "Continue with email" fully out, the tap with the keyboard still up, the code page, Change email (the selection is visible), the address retyped, Continue, the fresh code for juana5@example.com (246812, read live from the mock's output) typed on the number pad, Home with the keyboard down. Tail trimmed only, after Home had been showing; nothing cut inside the flow.

## What this run does not cover

- iPad (the spec for this re-check is iPhone-only).
- Wrong-code countdown behavior, sign-out, PIN lock, reinstall/relaunch persistence — covered in proof/ios-sign-in-2 at fe1a5f46, not re-run here.
- The mock's codes are sequential in this build (246810, 246811, 246812); whether that ordering is intended is the owner's question, not a finding — the spec only required per-request codes read from the output.
