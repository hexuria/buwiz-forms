# Android emulator machine proof

Machine: `devin-box`, Ubuntu 22.04.5 LTS.

This branch contains only the visual proof artifacts:

- `android-first-screen.png` — first eBIRForms screen captured with `adb exec-out screencap -p` after `nsh run --target android --port 8792` installed and connected the development host.
- `android-emulator-proof.mp4` — screen recording of the emulator relaunching the cached Android build, showing the welcome screen and a visible tap on **Sign in**. The local mock sign-in code is redacted because it is an authentication code, even though the server is loopback-only and disposable.

## Result summary

- `/dev/kvm`: present; `KVM (version 12) is installed and usable.`
- Emulator: `buwiz` AVD booted in 149 seconds; the QEMU process holds `/dev/kvm`.
- GPU: the emulator warned that host GPU drivers may have a bug and automatically chose software rendering. No manual `-gpu swiftshader_indirect` flag was needed because KVM CPU acceleration worked.
- Free disk after setup and proofs: 58 GB.
- NSH: `nsh 0.6.1 (8c78cfd)` at pinned ref `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.
- `xvfb-run -a just test`: PASS after running with the required test env (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`).
- `~/BuwizNsh/nsh2404.sh nsh check`: PASS.
- `just mock-account` plus `adb reverse tcp:3008 tcp:3008`: PASS; server listens only on `127.0.0.1:3008`.
- `~/BuwizNsh/nsh2404.sh nsh run --target android --port 8792`: PASS; built all three ABIs on the first run, installed the development host, and connected the emulator.

## Safety

All Android and desktop proof runs used the required test/dry-run environment. `BIR_SFTP_LIVE` was never set. The mock account server was not exposed beyond loopback.
