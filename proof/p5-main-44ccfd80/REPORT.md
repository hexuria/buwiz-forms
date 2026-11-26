# P5 recheck: iPhone 17 Pro at 44ccfd80

Environment: buwiz-forms `44ccfd8080f3028528077cd9a7fa64733846f309`, sibling nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, iPhone 17 Pro, Connect Hardware Keyboard OFF, local mock only `127.0.0.1:3008`, `juana@example.com`. No real account/service/Queue/`BIR_SFTP_LIVE`.

Result: all three fresh email-code-page arrivals passed.

| Attempt | Fresh-arrival observation | First long-press | Paste result | Sign-in |
|---|---|---|---|---|
| 1 | First box already focused; numeric keypad already up | Paste menu appeared | all 6 digits in order | completed |
| 2 | First box already focused; numeric keypad already up | Paste menu appeared | all 6 digits in order | completed |
| 3 | First box already focused; numeric keypad already up | Paste menu appeared | all 6 digits in order | completed |

Codes were mock-generated `246810`, `246811`, `246812` and copied to pasteboard only with `xcrun simctl pbcopy`. Reset per attempt was sign-out of Buwiz followed by a fresh sign-in request. The first email address was pasted once as setup; no hardware keyboard input was used.

Artifacts:

- video: `artifacts/p5-iphone-44ccfd80.mp4`
- first-menu frame: `artifacts/attempt1-first-longpress-menu.png`
- third-menu frame: `artifacts/attempt3-first-longpress-menu.png`
- signed-in frame: `artifacts/attempt3-signed-in.png`
- mock log: `artifacts/mock.log`

Note: first-run setup was completed once as fixture before reaching Settings > Account. The app displayed server `http://127.0.0.1:3008` from `BUWIZ_SERVER_URL`.
