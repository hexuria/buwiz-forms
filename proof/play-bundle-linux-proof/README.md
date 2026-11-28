# Play bundle Linux proof at 3ac4d5e0

Evidence only; no app source change. The checkout was `3ac4d5e0217a887ca06ce1e36db45e2a7ba65c84` with sibling NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Nothing was uploaded and no keystore or signing password was used. For this public branch, machine-local home prefixes in copied output are written as `$HOME`; the text is otherwise unchanged.

## A. Script tests

| test | count | result |
|---|---:|---|
| `test_play_bundle_guard.py` | 24 ran | OK, 24 passed |
| `test_android_bundle.py` | 18 ran | OK, 18 passed |
| `test_linux_nsh_setup.py` | 11 ran | 7 passed, 4 failed; none skipped for the platform |
| `test_justfile_build_folders.py` | 6 ran | OK, 6 passed |
| `test_prebuilt_setup.py` | 34 ran | 28 passed, 6 skipped |

`test_linux_nsh_setup.py` failed all four `test_sdk_selects_java_home_then_path_then_install` cases (`home`, `path`, `old`, `missing`) with `environment: line 41: android_api: command not found`. Logs are in `tests/`.

## B. dev_env Rust tests

Command: `$HOME/nsh2404-store-tools-61.sh cargo test -p nsh-app-ebirforms dev_env` from `nsh/`.

Result: 10 passed, 0 failed, 0 ignored, 155 filtered out. `the_sentinel_exists_only_where_the_development_list_is_honoured` passed. Log: `tests/dev-env-cargo-test.log`.

## C. Fresh setup

Command: `NSH_TOOLS=$HOME/store-tools-61 scripts/setup-linux-nsh.sh`.

It wrote `$HOME/nsh2404-store-tools-61.sh` (no hand-made shim). The SDK platform was `android-36`. `--check` output:

```text
  docker                 Docker version 29.9.2, build a82e528
  image:buwiz-nsh-2404   present
  ../nsh                 8c78cfd
  nsh CLI                8c78cfd
  nsh2404.sh wrapper     present
  android ndk            27.2.12479018
  android platform       android-36
  avd:buwiz              present
  nsh android dev runtime 8c78cfd
  nsh android release runtime 8c78cfd
==> NSH toolchain ready
```

## D. Real-artifact controls

### (a) release control

Command: `NSH_TOOLS=$HOME/store-tools-61 just play-bundle 1`

Check output (copied with the home prefix normalized):

```text
android_bundle: $HOME/j61/app/target/release-artifacts/eBIRForms-Android-0.2.1-1.aab is com.buwiz.ebirforms, versionCode 1, target API 36, backup off, release library, Gmail Connect off
```

Library sizes: arm64-v8a 88,411,952 bytes; armeabi-v7a 76,156,504 bytes. AAB size 166,013,061 bytes. AAB SHA-256 `90ac880ccb0996035f27f2b56712053cc6367d5290e9d661967f7497ddc3bff5`.

### (b) release with the development switch

Commands:

```bash
export BUWIZ_ANDROID_DEV=1 CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_TARGET_DIR=$HOME/j61/app/target/play-bundle NSH_TOOLS=$HOME/store-tools-61
(cd nsh && $HOME/nsh2404-store-tools-61.sh nsh package --target android --format aab --abi arm64-v8a,armeabi-v7a --id com.buwiz.ebirforms --build 1 --app-version 0.2.1 -o $HOME/j61/work/devswitch-android)
python3 scripts/android_bundle.py prepare $HOME/j61/work/devswitch-android/eBIRForms.aab $HOME/j61/work/devswitch-eBIRForms-Android-0.2.1-1.aab
python3 scripts/android_bundle.py check $HOME/j61/work/devswitch-eBIRForms-Android-0.2.1-1.aab --package com.buwiz.ebirforms --version-code 1 --target-sdk 36
```

`prepare` output:

```text
android_bundle: $HOME/j61/work/devswitch-eBIRForms-Android-0.2.1-1.aab (left out 4 entries: the signature and the updater's source)
```

`check` exited 1 and printed only the sentinel failure for both ABIs:

```text
android_bundle: arm64-v8a libnsh.so contains 'nsh-dev-env:development-list-compiled-in': a debug or development build
android_bundle: armeabi-v7a libnsh.so contains 'nsh-dev-env:development-list-compiled-in': a debug or development build
```

Production addresses were accepted, `http://localhost:3008` and `unsafe precondition(s) violated` did not appear, and both sizes were under their ceilings. Library sizes: arm64-v8a 88,419,904 bytes; armeabi-v7a 76,166,104 bytes. Prepared AAB SHA-256 `7d8a46a9d612ae1d8f28640df0200a3b413d073c51a3b19bb4f4b09aa22fec47`.

### (c) debug library control

The Android development host was built with `$HOME/nsh2404-store-tools-61.sh nsh run --target android`; its x86_64 `libnsh.so` was extracted before the APK was deleted.

Command: `python3 scripts/android_bundle.py library x86_64 $HOME/j61/work/debug-lib/libnsh-x86_64.so`

Output:

```text
android_bundle: x86_64 libnsh.so contains 'http://localhost:3008': a debug or development build
android_bundle: x86_64 libnsh.so contains 'nsh-dev-env:development-list-compiled-in': a debug or development build
android_bundle: x86_64 libnsh.so contains 'unsafe precondition(s) violated': a debug or development build
```

The library was 375,225,336 bytes.

### (d) job 59 bundle controls

Default-profile bundle command: `python3 scripts/android_bundle.py check $HOME/j59/work/eBIRForms-Android-0.2.1-1.default-profile.aab --package com.buwiz.ebirforms --version-code 1 --target-sdk 36`

Output, failing only on size ceilings:

```text
android_bundle: arm64-v8a libnsh.so is 122,876,080 bytes, over 100,000,000: the release profile did not reach the build
android_bundle: armeabi-v7a libnsh.so is 99,077,356 bytes, over 88,000,000: the release profile did not reach the build
```

Release bundle command: `python3 scripts/android_bundle.py check $HOME/j59/work/eBIRForms-Android-0.2.1-1.aab --package com.buwiz.ebirforms --version-code 1 --target-sdk 36`

Output:

```text
android_bundle: $HOME/j59/work/eBIRForms-Android-0.2.1-1.aab is com.buwiz.ebirforms, versionCode 1, target API 36, backup off, release library, Gmail Connect off
```

## E. Refusals before build

- `.env` present without `--allow-dotenv`: `❌ play-bundle: $HOME/j61/app/.env would be loaded into the build (`set dotenv-load`). With --allow-dotenv the bundle bakes in nothing from it (only GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET with STORE_GMAIL=1, for Gmail Connect); pass it when the file is meant (App Store Connect's next build number reads it), or move the file away`
- wrong wrapper: `❌ play-bundle: $HOME/BuwizNsh/nsh2404.sh is for $HOME/j62/app/nsh, not this checkout's $HOME/j61/app/nsh: run scripts/setup-linux-nsh.sh from this checkout`
- temporary `$HOME/.cargo/config.toml` with `[profile.release]`: `❌ play-bundle: $HOME/.cargo/config.toml:1: [profile.release] changes the release profile` and `❌ play-bundle: $HOME/.cargo/config.toml:2: profile.release.opt-level changes the release profile` (there was no existing file, so nothing had to be restored and no rustc-wrapper line was present)
- `STORE_GMAIL=1` on Linux: `❌ STORE_GMAIL=1 builds only on the owner's Mac: the Linux container gets no Google client.`

Host environment findings: the generated wrapper passes only `CARGO_PROFILE_RELEASE_LTO`, `CARGO_PROFILE_RELEASE_CODEGEN_UNITS` and `CARGO_PROFILE_RELEASE_OPT_LEVEL`; it does not pass `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS`, any `RUSTFLAGS` form, or the Google client variables. Therefore ordinary host runs with `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` or `RUSTFLAGS="-C debug-assertions"` did not reach the container-env refusal and proceeded into `nsh package`; both were terminated within seconds before a package completed. When the same variables are present where the build runs, the guard's exact refusals are:

- `❌ play-bundle: CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS is set where the build runs: a store build takes the recipe's release profile only (debug assertions would make it a debug build)`
- `❌ play-bundle: RUSTFLAGS is set where the build runs: rustflags can change what a release build becomes`

`GOOGLE_CLIENT_ID` without `STORE_GMAIL` has no refusal in the actual path: the wrapper does not pass it, and the recipe unsets it before reading the container environment.
