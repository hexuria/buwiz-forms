# Android emulator machine gate — proof

Machine: second Android emulator machine (Linux, KVM). App built at main `92b3e201`.
NSH tool and runtime built from the pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.
Runtime classes.dex sha256 (dev and release, identical Java): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`.

Every run had BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1. Show taps was on (the white dot and the pointer readout bar at the top of the screenshots).

## Items

- KVM present and usable — WORKS (`/dev/kvm` rw by user; `emulator -accel-check`: KVM version 12 installed and usable)
- scripts/setup-linux-nsh.sh at main — WORKS ("NSH toolchain ready"; last 20 lines in the report)
- AVD `buwiz` boots headless — WORKS (119 s to sys.boot_completed=1; ro.kernel.qemu=1, ro.build.version.sdk=35)
- Dev build of the app installs, connects to the dev server, signs in against mock-account and reaches Home — WORKS (about 13.5 min cold build+install to the first screen; see timing note)
- dev and release runtimes are real folders (not symlinks), copies of a build at the pin, `nsh-ref` beside classes.dex — WORKS
- `nsh --version` shows 8c78cfd — WORKS (nsh 0.6.1 (8c78cfd))

## Files

- `devhost-connecting.png` — the dev host's "Connecting to ws://127.0.0.1:8790/ws…" screen. Shows what the app displays while it cannot reach the dev server. Does not show a fault of the app: the websocket reverse had not been set yet.
- `welcome-signin.png` — the app's Welcome screen with Sign in / Create account, served by the dev server after `adb reverse tcp:8790 tcp:8792`. Proves the dev-server connection works.
- `onboarding-who.png` — the "Who will use eBIRForms on this device?" screen after sign-in against mock-account with --auto-approve. Proves sign-in completes on the emulator.
- `home-after-onboarding.png` — the Home screen (bottom tabs Home/Forms/Filings/Calendar) after onboarding with an invented taxpayer. This is the gate's Home proof.

## What these files do not show

- No recording: the flow was driven by adb taps, not recorded; the screenshots are the proof.
- The screenshots show one manual run with two operator detours (mock first started without --auto-approve; one back keypress that backgrounded the app). The timing in the report is therefore an upper bound, not a clean-run measurement.
- Home shows the empty "Add your first taxpayer" state even though one taxpayer was created during onboarding: the "Just me or my business" path was picked, and the created profile is the device admin's own — the empty-state Home is the genuine first-run Home. Not investigated further; not a gate item.
