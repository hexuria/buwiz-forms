# iOS: App Store package

The iOS app is the NSH app in `nsh/`, packaged by `nsh package --target ios`. Development runs use the Simulator
(`just nsh-run-agent ios`, [devin-environment.md](devin-environment.md)). This page covers the App Store package.

## What the package carries

| Piece | Where | Why |
| --- | --- | --- |
| Devices | `nsh/nsh.json` `ios.infoPlist.UIDeviceFamily` | `[1, 2]`: iPhone and iPad. NSH gives iPad all four orientations (iPad multitasking needs them) and iPhone portrait and both landscapes; the launch screen and the icon serve both (`actool` makes the iPad sizes from the 1024 image). iPad needs its own 13-inch screenshots. |
| Icon | `assets/ios/icon-1024.png` (from `icon-ios.svg`) | Apple wants 1024×1024 with no alpha. The desktop `assets/icon.png` (512, transparent) fails `actool`. |
| Launch screen | `nsh/nsh.json` `ios.launchScreen` | The app's own background, light and dark, so launch shows no flash. |
| Export compliance | `ios.infoPlist.ITSAppUsesNonExemptEncryption` | `false`, as the Mac App Store build says. `false` only while France is not in App Store availability and all cryptography is standard (AES, SHA-256, PBKDF2, SSH, TLS, HPKE); France needs a French encryption declaration, then `true` and Apple's code ([Apple's table](https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption/)). |
| Privacy manifest | `assets/ios/PrivacyInfo.xcprivacy`, copied to the bundle root | Required-reason APIs (UserDefaults, boot time, file timestamps) and the account's email and user id. Keep it in step with App Privacy in App Store Connect. |
| Print fixes | `print-overrides/`, copied beside the executable | `bir-app-api` reads them there (`packaged.rs`); a debug run reads the checkout's. |

The recipe also removes the updater's configuration (an iOS app does not update itself; the updater never starts on a
phone), refuses a bundle whose `Info.plist` has the desktop agent's screen-capture text, and makes every file readable
by everyone before signing (App Store Connect refuses owner-only files, which a umask of 077 produces).

Sending: on iOS the send guard forces a dry run (`nsh/src/guards.rs`), so a store build prepares and prints returns
and sends nothing until relay filing ships.

## Build and upload

Needs, from the owner: an **Apple Distribution** certificate in the keychain, and the **iOS App Store** provisioning
profile for `dev.goldcoders.bir` at `assets/ios/embedded.mobileprovision`. The App Store Connect API key is the one
`.env` already names for the Mac uploads (`APP_STORE_KEY_ID`, `APP_STORE_ISSUER_ID`, `APP_STORE_P8_PATH`).

```sh
CODESIGN_IDENTITY="Apple Distribution: …" just ios-appstore   # target/release-artifacts/eBIRForms-iOS-<version>.ipa
just ios-appstore-upload                                       # nsh publish ios
```

`nsh package` needs an iOS Simulator runtime that matches the Xcode SDK installed, even for a device package
(`actool` refuses without one).

## Simulator check: sign in, relaunch, reinstall

On iPhone and iPad the saved sign-in (`buwiz-cloud-account.json`), the device key and the server's device id
(`buwiz_submit_device_key.txt`, `buwiz_submit_device_id.txt`) are files beside the database, never Keychain items:
deleting the app deletes them. Each is written with the file protection "complete until first user authentication",
so a send can run while the phone is locked once it was unlocked after a restart. The app's data folder
(`Library/Application Support/dev.goldcoders.bir`) is excluded from backups and carries the same protection, set on
every launch. The Keychain holds one item, the install id, which survives a reinstall; development and test runs
keep the Keychain off (`BUWIZ_NO_KEYCHAIN=1`), so this check uses its file (`buwiz_install_id.txt`, beside the
database).

The database must be named, or a test run gets a new scratch folder in `tmp/` on every launch and always starts
signed out. Name one under the app container's `Documents/`: the door's guard lets a test run open it
(`nsh/src/guards.rs`, test `a_test_run_may_open_a_database_in_the_containers_documents`), while the app's own data
folder stays refused. No `BIR_AUTH_BYPASS`: with it a test run skips the sign-in gate.

```sh
just mock-account --port 3008 --auto-approve                  # in another terminal
cd nsh
export PATH="$HOME/BuwizNsh/tools/bin:$PATH" CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh"
export SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
  SIMCTL_CHILD_BUWIZ_SERVER_URL=http://localhost:3008
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
| Sign in through the gate (step 2); the mock approves by itself | Signed in; `buwiz-cloud-account.json` beside the database. |
| Relaunch, same variables | Still signed in. |
| Delete the app (`xcrun simctl uninstall booted dev.goldcoders.bir`), then steps 1 and 2 again | Signed out: the files went with the app. |

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
