# P5 email-code Paste — app 134c2e4e, nsh 8c78cfd6

Free disk first: **6.3 GiB free** on `/System/Volumes/Data` at completion.

Build/setup: buwiz-forms `134c2e4e1d159feafebf500a4030c8b64a3f4ac6`; sibling nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; scratch CLI `~/BuwizNsh/tools-main-8c78cfd`; fresh target `~/BuwizNsh/target-p5-main-134c2e4e`; rustup cargo 1.99.0; `CARGO_INCREMENTAL=0`. Scratch DBs only. For the email-code flow `BIR_AUTH_BYPASS` was unset; the app was pointed at the mock with `BUWIZ_SERVER_URL=http://localhost:3008` and `BUWIZ_ACCOUNT_URL=http://localhost:3008`. No Queue, no real service, no real account, never `BIR_SFTP_LIVE`.

## Results

### iPad Pro 13-inch

- Direct empty-box long-press: **Paste menu appeared**; tapping Paste inserted all six digits in order and sign-in completed. Evidence: `artifacts/ipad-direct-longpress-menu.png`, `artifacts/ipad-direct-outcome.png`.
- Tap once first, then long-press: **Paste menu appeared**; tapping Paste inserted all six digits in order and sign-in completed. Evidence: `artifacts/ipad-tapfirst-outcome.png`.
- One-time-code suggestion: **not offered by the Simulator**. Tried focusing the empty code boxes after the mock sent the code; the numeric/software keyboard appeared with no one-time-code or “From Mail” suggestion. Evidence: `artifacts/ipad-tap-focus-no-otc.png`.
- Hardware keyboard Cmd+V control: **passed** after enabling Simulator’s Connect Hardware Keyboard. The first Cmd+V before enabling it did nothing; after enabling it, one Cmd+V inserted all six digits in order and sign-in completed. Evidence: `artifacts/ipad-hardware-outcome.png`.

### iPhone 17 Pro

- Direct empty-box long-press: **no Paste menu appeared**. The long-press focused the first empty box and opened the numeric keyboard; because no menu appeared there was nothing to tap and sign-in did not complete by this path. Evidence: `artifacts/iphone-direct-longpress-no-menu.png`.
- Tap once first, then long-press: **Paste menu appeared**; tapping Paste inserted all six digits in order and sign-in completed. Completion is proven by `/auth/email/verify`, `/me`, `/api/devices` in the mock log and the app advancing past the cloud gate to the local Sign-in screen. Evidence: `artifacts/iphone-tapfirst-menu.png`, `artifacts/iphone-tapfirst-outcome.png`.
- One-time-code suggestion: **not offered by the Simulator**. Tried tapping/focusing the empty code boxes after the mock sent the code; the numeric keyboard appeared with no one-time-code or “From Mail” suggestion. Evidence: `artifacts/iphone-tap-focus-no-otc.png`.
- Hardware keyboard Cmd+V control: **passed**. The I/O > Keyboard menu showed Connect Hardware Keyboard unchecked; after enabling it, one Cmd+V inserted all six digits in order and sign-in completed. Evidence: `artifacts/iphone-hardware-outcome.png`.

## Mock-auth evidence and caveat

Mock log: `artifacts/mock-account.log`. Every mock server instance listened on `127.0.0.1:3008` only. The relevant successful email lines are five `/auth/email/start` + code lines followed by `/auth/email/verify` for the five passing paste/control paths. One additional start (`246811`) belongs to the iPhone direct-empty attempt that did not paste and has no verify.

Literal caveat: the complete log also contains local mock `GET /me`, `POST /api/devices`, `POST /oauth/token`, and one `POST /api/auth/logout` caused by sign-in/sign-out behavior. No external or real service appears or was used.

## Notes

- iPad sign-in pages were reached through Settings > Account > Sign in after unlocking the scratch local profile; later iPad repetitions used Sign out of Buwiz and a fresh mock server/sign-in.
- iPhone repetitions used fresh app processes, fresh scratch DB copies, unique `BUWIZ_CLOUD_ACCOUNT_FILE` paths, and deletion of the scratch pending-sign-in file so each path started empty.
- The iPhone outcome screenshots show the local Sign-in screen rather than Home because the scratch database’s local app lock is the next gate after the cloud sign-in completes.
