driver: gpui-agent-cli 0.5.0

# 0605 Android print evidence

App commit: 8329d0fe3833c865469b04fd6f37d3dc9c28e2ea (newest main when captured).

The gpui-agent recipes seeded the draft and pressed the app's Print control. Android's system print preview is outside the app's semantic tree and window surface, so the preview was captured passively by adb screencap; no adb input drove the app. This limitation is the known_gap: the system preview cannot be asserted through gpui-agent's semantic tree.

`address-137-print.png` and `address-137-crop.png`: the exact 137-character registered address prints on two complete lines at newest main.

`values-200-print.png`: the 200-character name, address and line-of-business values use the existing two-line ellipsis on the form and continue in full on the additional sheet.

`fill-values.json` holds the exact seeded values. The recipes are copies of the app recipes used for the proof. Safety variables were `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; the demo taxpayer and a scratch database were used; Queue was never pressed.
