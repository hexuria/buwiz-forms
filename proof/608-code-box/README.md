driver: gpui-agent-cli 0.5.0
recipe: signin-code.json (in this folder)

# Proof for #608: `type gate-code-boxes 246810` in the real window

Real-app run on the macOS desktop, driven only by gpui-agent-cli 0.5.0
(recipe `signin-code.json`, receipt `report.json` — all 15 steps pass).

The four conditions of the owner's exception to the bypass rule:

1. The account server is the mock on this machine, loopback only
   (127.0.0.1, port 3008); no other account or server URL anywhere in
   the environment (BUWIZ_SERVER_URL and BUWIZ_ACCOUNT_URL both
   http://127.0.0.1:3008).
2. BUWIZ_NO_KEYCHAIN=1, EBIR_TEST_ENV=1, BIR_SFTP_DRY_RUN=1;
   BIR_SFTP_LIVE never set. BIR_AUTH_BYPASS was unset for this run only.
3. A debug build (never a release build) and a scratch database.
4. The app really starts at the sign-in page (step s1 waits for
   `gate-start`; see 001-s1.png).

## The red evidence (before the fix)

`before-type-fails.log`: on the pre-fix composition, the runner's exact
step `type gate-code-boxes 246810` fails in the real window with
"node 162 is not a text input" (hexuria/gpui-agent#135 — gpui-agent's
`type` needs the node itself to be a text input; the testID sat on
OtpInput's root, a plain view, and the inner input had only a generated
id). `before-code-page.png` shows the app genuinely at the code page in
that before-run.

## The green run (after the fix)

- 001-s1.png — the app starts at the sign-in page (condition 4).
- 006-s6.png — the code page, reached by typing account-a@example.com
  into `gate-email` and continuing.
- 007-s7.png — step s7: `type gate-code-boxes 246810` — the mock's first
  code — into the node with that very id, which is now the text input
  itself. The step passes.
- 015-s15.png — Home (`home-welcome`), after first-run setup
  (help-desk choice, admin PIN typed into `setup-admin-pin` — the same
  PinCells — and the recovery key), completing sign-in.
- proof-run.mp4 — screen recording of the run. An earlier version of this video showed a throwaway recovery key from a scratch database; the seconds showing it are trimmed out (the jump in the setup flow).
- report.json — the gpui-agent receipt: 15/15 steps pass.
