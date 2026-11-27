# iOS: App Store package

The iOS app is the NSH app in `nsh/`, packaged by `nsh package --target ios`. Development runs use the Simulator
(`just nsh-run-agent ios`, [devin-environment.md](devin-environment.md)). This page covers the App Store package.

## What the package carries

| Piece | Where | Why |
| --- | --- | --- |
| Devices | `nsh/nsh.json` `ios.infoPlist.UIDeviceFamily` | `[1, 2]`: iPhone and iPad. NSH gives iPad all four orientations (iPad multitasking needs them) and iPhone portrait and both landscapes; the launch screen and the icon serve both (`actool` makes the iPad sizes from the 1024 image). iPad needs its own 13-inch screenshots. |
| Icon | `assets/ios/icon-1024.png`, made with every other app icon by `just icons` from `assets/brand/buwiz-forms.svg` ([assets/brand/README.md](../../assets/brand/README.md)) | Apple wants 1024×1024, square, with no alpha and no rounded corners (iOS rounds them). The desktop `assets/icon.png` (512, transparent) fails `actool`. The logo always keeps its words: no eagle-only cut. |
| Launch screen | `nsh/nsh.json` `ios.launchScreen` | The app's own background, light and dark, so launch shows no flash. |
| Export compliance | `ios.infoPlist.ITSAppUsesNonExemptEncryption` | `false`, as the Mac App Store build says. `false` only while France is not in App Store availability and all cryptography is standard (AES, SHA-256, PBKDF2, SSH, TLS, HPKE); France needs a French encryption declaration, then `true` and Apple's code ([Apple's table](https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption/)). |
| Privacy manifest | `assets/ios/PrivacyInfo.xcprivacy`, copied to the bundle root | Required-reason APIs (UserDefaults, boot time, file timestamps) and the account's email and user id. Keep it in step with App Privacy in App Store Connect. |
| Print fixes | `print-overrides/`, copied beside the executable | `bir-app-api` reads them there (`packaged.rs`); a debug run reads the checkout's. |

The recipe also removes the updater's configuration (an iOS app does not update itself; the updater never starts on a
phone), refuses a bundle whose `Info.plist` has the desktop agent's screen-capture text, and makes every file readable
by everyone before signing (App Store Connect refuses owner-only files, which a umask of 077 produces).

Sending: a store build files through the Buwiz relay, as on the desktops. A debug or development build, a test
environment or an agent launch only simulates; it never sends or registers through production (`nsh/src/guards.rs`
`restricted_launch`).

## Build and upload

Needs an **Apple Distribution** certificate in the keychain, and the **iOS App Store** provisioning profile for
`dev.goldcoders.bir` copied to `assets/ios/embedded.mobileprovision` at build time (the profile is kept outside the
repository; `.gitignore` keeps it out of commits). The App Store Connect API key is the one
`.env` already names for the Mac uploads (`APP_STORE_KEY_ID`, `APP_STORE_ISSUER_ID`, `APP_STORE_P8_PATH`).

```sh
export NSH_TOOLS=~/BuwizNsh/store-tools                         # the nsh tool built at the NSH pin (below)
CODESIGN_IDENTITY="Apple Distribution: …" just ios-appstore   # target/release-artifacts/eBIRForms-iOS-<version>.ipa
just ios-appstore-upload                                       # nsh publish ios, with the same tool
```

A store package comes only from the pinned NSH, `NSH_PINNED_REF` in `.github/workflows/nsh.yml`:
- the `nsh` tool in `$NSH_TOOLS/bin` must be built at that commit;
- the `../nsh` checkout beside this one must be at that commit;
- otherwise `just ios-appstore` (and the Mac App Store package, `just app`) refuses to start.

The shared `~/BuwizNsh/tools` is the everyday tool. It is not rebuilt for a store build, because other sessions use it. The pinned tool lives side by side in `~/BuwizNsh/store-tools`; its `nsh-ref` file names the commit it was built at. It is shared with the Google Play build (`just play-bundle`), so it is rebuilt only when the pin moves. Point `NSH_TOOLS` at it.

Each package builds in the checkout's own folder (`target/nsh-store`), not the shared `~/BuwizNsh/target-nsh`. Cargo cannot tell two checkouts of the same crate version apart, so a shared folder could put another branch's form data in the package.

The recipe builds with the pinned toolchain's own cargo (`rustup which cargo`, put first on PATH), which honours the folder, and refuses when the cargo it finds is the owner's fork (`huacnlee/cargo`): that cargo ignores `CARGO_TARGET_DIR` and shares one build folder between every checkout. The fork stays the everyday cargo. The recipe prints `build: app <commit>, nsh <commit> beside it, tool <nsh --version>, <cargo --version>, folder <folder>` and keeps that line, with the build number, in `eBIRForms-iOS-<version>.provenance.txt` beside the package.

TestFlight build 28 (0.2.1) was made before this rule with the cargo fork, whose shared build folder can hand one checkout another's compiled crates, so its form data cannot be certified. It is internal TestFlight only, must not be promoted, and the next phone build, made by these recipes, replaces it.

`nsh package` needs an iOS Simulator runtime that matches the Xcode SDK installed, even for a device package
(`actool` refuses without one).

## Simulator check: sign in, relaunch, reinstall

On iPhone and iPad the saved sign-in (`buwiz-cloud-account.json`), the device key and the server's device id
(`buwiz_submit_device_key.txt`, `buwiz_submit_device_id.txt`) are files beside the database, never Keychain items:
deleting the app deletes them. Each is written with the file protection "complete until first user authentication",
so a send can run while the phone is locked once it was unlocked after a restart. The app's data folder
(`Library/Application Support/dev.goldcoders.bir`) is excluded from backups and carries the same protection, set on
every launch. The Keychain holds one item, the install id, which survives a reinstall; development and test runs
keep the Keychain off (`BUWIZ_NO_KEYCHAIN=1`); a test run (`EBIR_TEST_ENV=1`) uses a random id in its own file
beside the database (`buwiz-install-id`). An email sign-in waiting for its code is a file beside the database too
(`buwiz-pending-sign-in.json`), gone once the code is used, the request ends, or the person signs out.

The database must be named, or a test run gets a new scratch folder in `tmp/` on every launch and always starts
signed out. Name one under the app container's `Documents/`: the door's guard lets a test run open it
(`nsh/src/guards.rs`, test `a_test_run_may_open_a_database_in_the_containers_documents`), while the app's own data
folder stays refused. No `BIR_AUTH_BYPASS`: with it a test run skips the sign-in gate.

```sh
just mock-account --port 3008                                  # in another terminal; the code is printed in the mock's output
cd nsh
export PATH="$HOME/BuwizNsh/tools/bin:$PATH" CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh"
export SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
  SIMCTL_CHILD_BUWIZ_SERVER_URL=http://localhost:3008
# A development build makes its relay client (and so adds the device after the sign-in) only with a test trust key;
# the mock serves no trust list, so nothing can be sent.
export SIMCTL_CHILD_BUWIZ_TRUST_KEYS=dev-test-key-not-production
unset SIMCTL_CHILD_BIR_SFTP_LIVE SIMCTL_CHILD_BIR_AUTH_BYPASS
# 1. Install and launch once (scratch database), then quit: the app's container exists now.
nsh run --target ios
# 2. Name the stable database in it. A reinstall gets a new container: read it again.
C="$(xcrun simctl get_app_container booted dev.goldcoders.bir data)"
export SIMCTL_CHILD_BIR_DATABASE_PATH="$C/Documents/buwiz-check/bir_data.db"
nsh run --target ios
```

| Step | What it should show |
| --- | --- |
| Step 2 opens on the email page: type an address, Continue with email; on the code page type the code printed in the mock's output ("sign-in code for <email>: <code>") | Home; `buwiz-cloud-account.json` beside the database; the mock prints `POST /api/devices`. |
| Relaunch, same variables | Still signed in. |
| Delete the app (`xcrun simctl uninstall booted dev.goldcoders.bir`), then steps 1 and 2 again | Signed out, on the email page: the files went with the app. |

The app's log (a test run's is in the container's `tmp/ebirforms-logs-<pid>/logs/`) has, once per launch:
`App storage at launch: backup exclusion set; file protection set; old Keychain items skipped, the Keychain is off`
(with the Keychain on, the last part is `device-key deleted, device-id absent, sign-in absent` once, then
`removed at an earlier launch`). A failure reads `NOT set (<reason>)` and is a warning.

To read the backup exclusion in the container (on the Mac running the Simulator):
`xattr -l "$C/Library/Application Support/dev.goldcoders.bir"` should list
`com.apple.metadata:com_apple_backup_excludeItem`. The file protection cannot be read from the Mac: the Simulator
keeps its files on the Mac's disk and does not enforce data protection. The launch line above says whether it was
set; only a device run (for example `FileManager.attributesOfItem(atPath:)[.protectionKey]` in a test build) shows
the class itself.

## Screenshots

App Store Connect requires the iPhone 6.1"/6.3" set (1206×2622 or 1179×2556, e.g. iPhone 18 Pro) and the iPad 13" set (2064×2752 or 2048×2732). No alpha. Previews are 886×1920, 15–30 s, at most 30 fps.
