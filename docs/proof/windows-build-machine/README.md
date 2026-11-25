# Windows build machine proof

- Machine: `devinbox`
- Windows: Microsoft Windows Server 2022 Standard build 20348
- Free disk when prepared: 51 GB
- buwiz-forms: `main` at `92b3e201` (`hexuria/buwiz-forms`)
- NSH version: `nsh 0.6.1 (8c78cfd)`
- NSH pinned ref: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
- Safety environment for proof runs: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` and `BIR_DATABASE_PATH` unset.

| Step | Result | Log |
|---|---|---|
| `just nsh-check` | PASS | [nsh-check.log](nsh-check.log) |
| `just nsh-build` | PASS | [nsh-build.log](nsh-build.log) |
| `cargo test --locked -p bir-core` | PASS | [cargo-test-bir-core.log](cargo-test-bir-core.log) |

## Log tails

### `just nsh-check`

```text
✓ checked nsh-app-ebirforms (947ms)
✓ built the Rust crate nsh-app-ebirforms (8m 26s)
```

### `just nsh-build`

```text
✓ built the Rust crate nsh-app-ebirforms (1.3s)
✓ built App.tsx into .nsh\out (1.1 MB js, 3.2 MB bytecode, 62 assets, 1.1s)
```

### `cargo test --locked -p bir-core`

```text
test sample_payloads::agent_forms::the_page_writes_profile_boxes_and_never_identity_boxes ... ok
test sample_payloads::agent_forms::filling_a_box_shows_its_value_in_the_official_box ... ok
test sample_payloads::agent_forms::agent_forms_are_derived_from_the_samples has been running for over 60 seconds
test sample_payloads::agent_forms::agent_forms_are_derived_from_the_samples ... ok

test result: ok. 146 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 144.28s

     Running tests\temp_test.rs (C:\Users\Administrator\BuwizNsh\target-nsh\debug\deps\temp_test-2af2bae830027b46.exe)

running 1 test
test compensation_profile_resolves_only_the_annual_return ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\test_key_never_quarantines_test.rs (C:\Users\Administrator\BuwizNsh\target-nsh\debug\deps\test_key_never_quarantines_test-b4d87a5cfe76e931.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bir_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Setup logs

- `setup-windows.ps1`: PASS — [setup-windows.log](setup-windows.log)
- `setup-android-windows.ps1 -NoEmulator`: PASS — [setup-android-windows.log](setup-android-windows.log)
