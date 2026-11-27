# Android: Google Play bundle

The Android app is the NSH app in `nsh/`, packaged by `nsh package --target android --format aab`. Development runs use
an emulator (`just nsh-run-agent android`, or `scripts/run-android-windows.ps1` on Windows,
[devin-environment.md](devin-environment.md)). This page covers the Google Play bundle: what it carries, how it is built
and signed, and what the Play Console needs.

## What the bundle carries

| Piece | Where | Why |
| --- | --- | --- |
| Application id | `--id com.buwiz.ebirforms` in `just play-bundle` | Google Play's id for the app, fixed forever once published. `nsh/nsh.json` keeps `dev.goldcoders.bir`, the id of every other system and of development builds, so a development build installs beside the store app. The id names the app's data folder inside its sandbox (`files/nsh/com.buwiz.ebirforms`); `bir_core::platform::NSH_APP_ID` does not, on Android. |
| Target API level | `nsh/nsh.json` `android.targetSdk` (36) | Google Play requires new apps and updates to target Android 16 (API 36) since 31 August 2026 ([Google's page](https://support.google.com/googleplay/android-developer/answer/11926878)). The SDK needs `platforms;android-36` (`scripts/setup-android-windows.ps1` installs it). The oldest Android the app runs on is 8.0 (API 26), NSH's minimum. |
| Backup off | `nsh/nsh.json` `android.allowBackup` (`false`) | Returns, the sign-in and the device key stay on the phone: no copy to the user's Google account and none to a new phone. NSH writes `allowBackup="false"`, `fullBackupContent="false"` and data extraction rules that exclude every storage domain from both (Android 12 and later read only the rules). |
| Native code | `--abi arm64-v8a,armeabi-v7a` | Phones and tablets. Google Play makes one download per ABI from the bundle. NSH links `libnsh.so` for 16 KB memory pages, which Android 15 devices need. |
| Version | `versionName` = the version, `versionCode` = the build number | [VERSIONING.md](../VERSIONING.md). |
| No updater | `scripts/android_bundle.py prepare` removes `nsh-updater.json` | Google Play updates the app. The updater never starts on a phone (`DesktopHost` mounts only on a desktop); its source goes too, as in the iOS package. |

### Size

`libnsh.so` is nearly all of the package. NSH builds it in the workspace it generates under `nsh/.nsh/`, and that
workspace has no `[profile.release]`: the release profile in this repository's `Cargo.toml` (LTO, one codegen unit,
`opt-level = "z"`, `panic = "abort"`, strip) never applies to it, so Cargo's defaults do. That is very likely true of
every NSH package, not only Android's. `just play-bundle` therefore sets the profile through Cargo's environment
variables. Measured on `main` at `d7fc83be`, arm64-v8a, stripped as NSH ships it:

| Settings | `libnsh.so` | Deflated (about the download) |
| --- | --- | --- |
| Cargo's defaults (opt-level 3, no LTO, 16 codegen units) | 115.6 MB | 32.2 MB |
| Fat LTO, one codegen unit | 100.7 MB | 30.7 MB |
| **The same with `opt-level = "s"`: what `play-bundle` sets** | **84.5 MB** | **23.6 MB** |
| The same with `opt-level = "z"` | 77.5 MB | 22.3 MB |
| `opt-level = "z"` and `panic = "abort"` | 72.2 MB | 19.6 MB |

`z` and `panic = "abort"` wait until such a build has been used on a real phone (`z` trades speed in layout and
drawing for size) and until the dependencies' use of `catch_unwind` is known: the app and NSH's Android crates have
none, but with `abort` a panic that a dependency would have caught ends the app. Google Play's limits are far away
either way: 500 MB for the base module's compressed download, and a notice to users on mobile data above 200 MB
([Google's page](https://support.google.com/googleplay/android-developer/answer/9859372)).

Not in the package yet: the per-form print fixes (`print-overrides/`). `bir-app-api` reads them from a folder beside
the executable on the desktop (`packaged.rs`), and an Android app has no such folder. Until the app reads them from its
own assets, a printed return on Android has no per-form fixes.

Sending: a release build files through the Buwiz relay, as on the desktops. A debug or development build, a test
environment (`EBIR_TEST_ENV`) or an agent launch (`GPUI_AGENT`) only simulates, or sends through a Buwiz server on
the same computer; it never sends or registers through production (`nsh/src/guards.rs` `restricted_launch`).

## Build, sign, upload

Three steps on the owner's Mac. The first needs no secret; the second is the only one that touches the upload key.

1. **Build** (`just play-bundle --allow-dotenv` for App Store Connect's next number, which it reads with `.env`, or
   `just play-bundle 28` to choose the number). `just` loads a `.env` into every recipe and the app bakes some variables
   in at compile time, so the recipe refuses to build while a `.env` is in the checkout or a folder above it unless
   `--allow-dotenv` says it is meant; it never passes a server address (`BUWIZ_SERVER_URL`, `BUWIZ_ACCOUNT_URL`) or a
   development switch to the build (`scripts/play_bundle_guard.py`). It builds the bundle, removes
   the debug signature `nsh package` puts on it and the updater's source, and checks the result: package
   `com.buwiz.ebirforms`, the expected `versionCode`, `targetSdkVersion` from `nsh.json`, backup off, not debuggable,
   phone ABIs only, and no permission beyond those the store listing declares (`STORE_PERMISSIONS`: `INTERNET`,
   `POST_NOTIFICATIONS`) (`scripts/android_bundle.py check`). It builds in the checkout's own `target/play-bundle` unless
   `CARGO_TARGET_DIR` is set, so a bundle never links crates compiled from another worktree. It writes
   `target/release-artifacts/eBIRForms-Android-<version>-<build>.aab`, unsigned.
2. **Sign** (`just play-sign`): the owner types the upload key's password when `jarsigner` asks. It writes
   `…-signed.aab` beside the unsigned file, verifies it, and shows it in Finder.
3. **Upload**: in the Play Console, the app > Test and release > the track (Internal testing first) > Create new
   release, then drag the signed file into the box. The first upload asks about app signing: keep **Google-generated
   app signing key** (Play App Signing). Google then signs what users install, and our key only proves the upload is
   ours.

### On a Mac, once

`scripts/setup-android-macos.sh` makes this Mac ready for `just play-bundle`: it checks the Android SDK (the
`platforms;android-<targetSdk>` of `nsh/nsh.json`, build-tools, an NDK), a JDK and the Rust Android targets, and reports
the command for anything missing; then it moves the NSH checkout beside this repository to the pin, and builds the `nsh`
CLI and NSH's Android release runtime at the pin into `$NSH_TOOLS` (default `~/BuwizNsh/tools`). `--check` only checks.

`just play-bundle` uses the same `$NSH_TOOLS` and refuses to build when its `nsh` is not at the NSH pin, or when the
Android release runtime beside it is not recorded as built at the pin (`runtime/android/release/nsh-ref`, written by the
setup script), so a bundle is never made with another NSH than CI tested. `nsh package` takes that release runtime even
for a development build. The recipe prints the runtime's `classes.dex` SHA-256 with the CLI's version. On the owner's
Mac the shared tool stays as other work needs it, and the pinned store tool lives beside it in `~/BuwizNsh/store-tools`
(shared with the Apple store recipes; only the setup script writes there):

```sh
export NSH_TOOLS=~/BuwizNsh/store-tools
scripts/setup-android-macos.sh   # once, and again when the pin moves
just play-bundle --allow-dotenv  # App Store Connect's next number; or: just play-bundle 28 (with no .env present)
```

The app's Rust toolchain (`rust-toolchain.toml`) needs both Android targets; the setup script names the
`rustup target add` for a missing one. The vendored OpenSSL needs the NDK's `llvm-ranlib`, which the recipe passes as
`RANLIB_<target>`.

A run on the owner's Mac, 26 November 2026, app `d1a8c2c4` (this pull request). The store tool and runtime were
rebuilt by the setup script with rustup's cargo 1.99.1; the bundle was built by the app's pinned cargo 1.99.0 in
`target/play-bundle`. The tool is `nsh 0.6.1 (8c78cfd)`, and the runtime `classes.dex` SHA-256 is `f70a1b02…`, the same
bytes inside the bundle. The unsigned bundle (build 27, never uploaded) was 162.3 MB. `libnsh.so` is 86.6 MB on arm64-v8a
and 74.6 MB on armeabi-v7a; the bundle stores them uncompressed. At `nice 15` the setup's runtime took about 40 minutes
and the bundle's two libraries 16 and 11 minutes.

Machines never sign or upload the store bundle. They build development and test packages with the Android debug key
that `nsh package` makes on its own, and a Linux machine may build the unsigned store bundle as a verification build
(below): the bundle that goes to Google Play is still the one built and signed on the owner's Mac.

### On Linux, a verification build

The same recipe runs on Linux. The pinned `nsh` is built in the Ubuntu 24.04 image (GTK 4), so the recipe runs it
through `~/BuwizNsh/nsh2404.sh` (or `$NSH_WRAPPER`), which `scripts/setup-linux-nsh.sh` writes for one checkout.
Before building, the recipe refuses a wrapper written for another checkout, NSH tools the container cannot see, and a
container that did not get the release profile (`CARGO_PROFILE_RELEASE_LTO`, `_CODEGEN_UNITS`, `_OPT_LEVEL`, read with
`env` inside it): without the profile Cargo builds its defaults silently, a 123 MB arm64 library instead of 88 MB.

```sh
NSH_TOOLS=~/store-tools scripts/setup-linux-nsh.sh   # the CLI and runtimes in a folder of their own, at the pin
NSH_TOOLS=~/store-tools just play-bundle 1           # a number, so .env is not needed
```

The setup installs the Android platform `nsh/nsh.json` targets (`android.targetSdk`). A run on a Linux machine, 28
November 2026, app `50798351`, NSH `8c78cfd`: package `com.buwiz.ebirforms`, versionCode 1, versionName `0.2.1`,
minSdk 26, targetSdk 36, not debuggable, `allowBackup` false, `INTERNET` and `POST_NOTIFICATIONS` only, `arm64-v8a`
and `armeabi-v7a` only, no updater asset, unsigned; `libnsh.so` 88.4 MB (arm64) and 76.1 MB (armeabi-v7a); 166.0 MB
in all (bundletool 1.18.3 and aapt2 36.0.0 read it independently).

### The upload key

Made once by the owner, on their Mac:

1. In the password manager, make an entry "eBIRForms Android upload key" with a new long password.
2. `just play-upload-key` and type that password twice when asked. It writes
   `~/Keys/ebirforms-android/upload.jks` (or `$PLAY_UPLOAD_KEYSTORE`), readable only by the owner.
3. Attach `upload.jks` to the same password manager entry.

The key is never committed, copied to a build machine, printed or kept as a machine secret. If it is lost, the owner
asks Google for an upload key reset in the Play Console (app integrity); the app's signing key is Google's and is not
affected. `jarsigner` and `keytool` come from `$JAVA_HOME`, Homebrew's `openjdk` or Android Studio's JDK; macOS's
`/usr/bin/jarsigner` is a stub without Java.

## The Play listing

| Field | Value |
| --- | --- |
| App name | eBIRForms by Buwiz |
| Developer account | Hexuria (organization; Google also shows the organization's legal name and address) |
| Contact email | support@buwiz.com |
| Website | https://ebirforms.buwiz.com |
| Privacy policy | https://ebirforms.buwiz.com/privacy |
| Category | Finance; free, no ads |
| Countries | Philippines only |
| Languages | English and Filipino |
| Audience | Adults (18 and over) |

The listing and the app say plainly that eBIRForms is independent and not affiliated with the Bureau of Internal
Revenue, and name bir.gov.ph as the source of the forms.

## Checks before an upload

- `just play-bundle` ends with `android_bundle: … backup off`.
- On an emulator with the same build: `adb shell dumpsys package com.buwiz.ebirforms` lists no `ALLOW_BACKUP` in its
  flags, and `adb shell bmgr backupnow com.buwiz.ebirforms` backs nothing up.
- The tests: `python3 -m unittest scripts.tests.test_android_bundle`.
