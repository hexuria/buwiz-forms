# os-login Windows Hello proof

App commit `cbf8fc0814e2e20aa0792a64bb64d65461a593e7`; sibling NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; `nsh 0.6.1 (8c78cfd)`. Built on drive C: (Windows); the only other drive, D:, is a tiny config drive.

## Files

- `machine-facts.txt` — machine, OS, TPM, Sign-in options, checkout, build folder, commit and drive facts. It does not prove runtime behavior by itself.
- `signin-options.png` — full-screen Settings > Accounts > Sign-in options page. “Windows Hello PIN” is present but unavailable on this machine. It does not prove whether Hello would work on a machine with enrolled Hello hardware.
- `step1-os-login-tests.txt` — full output of `cargo test --manifest-path nsh/Cargo.toml --lib os_login -- --nocapture` with `EBIR_TEST_ENV=1`. Shows 11 passed, 0 failed, 1 ignored (the real OS test). It does not exercise real Hello.
- `step1-clippy.txt` — full output of `cargo clippy --manifest-path nsh/Cargo.toml --lib --tests` with `EBIR_TEST_ENV=1`. Shows clippy exited 0; the os_login warnings are two `cloned_ref_to_slice_refs` warnings in test code. It is not a compile failure.
- `real-during.png` — full-screen screenshot while the first real OS-call run was in progress, with proof that `EBIR_TEST_ENV` was absent in the proof parent. No Hello, Windows Security, credentials, or password box is visible.
- `real-after.png` — full-screen screenshot after all three real OS-call runs passed. No prompt or credentials box is visible.
- `real-run-1.txt`, `real-run-2.txt`, `real-run-3.txt` — full outputs of the three real ignored-test runs. Each prints `owner_check=None`, `lock_window=None`, `prove_owner=Unavailable`, exit code 0.
- `os-login-proof.mp4` — one continuous 4.67-second full-screen recording covering the three real OS-call runs. It contains no Windows Hello prompt, Windows Security/credentials box, password box, or hang.
- `video-contact.png` — supplemental contact sheet/inspection frame image made while checking the recording. It is evidence review aid, not an additional test state.

## What this proof does not show

It does not run the eBIRForms app, sign in to any account, send anything to BIR/relay, or test a machine with Windows Hello available/enrolled. It proves only this no-Hello Windows Server 2022 machine: the real OS calls answer no owner method/Unavailable quickly, with no prompt and no password box.
