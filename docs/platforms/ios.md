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

## Simulator check: the Keychain, a relaunch, a reinstall

Every development, test and agent run keeps the OS Keychain off (`BUWIZ_NO_KEYCHAIN=1`, the owner's standing rule):
a debug build or `EBIR_TEST_ENV` switches it off in `nsh/src/door.rs` whatever the caller set. This check is the one
exception. `BUWIZ_KEYCHAIN=1` lets a debug or test run use the Keychain **on iOS only** (the Simulator's own, or a
test device's); on every other system it does nothing, and `BUWIZ_NO_KEYCHAIN=1` still wins over it. Use it for this
check and nothing else.

What it proves: the saved sign-in is stored "after first unlock, this device
only" (`kSecAttrAccessible` = `cku`), an older item gets that class in place, a fresh install clears them, and the
app's data folder is excluded from backups (`bir_submit::ios_storage`).

The database must be named, or a test run gets a new scratch folder in `tmp/` on every launch and always starts
signed out. Name one under the app container's `Documents/`: the door's guard lets a test run open it
(`nsh/src/guards.rs`, test `a_test_run_may_open_a_database_in_the_containers_documents`), while the app's own data
folder `Library/Application Support/dev.goldcoders.bir` stays refused. The sign-in file
(`buwiz-cloud-account.json`) and any device-key fallback file go beside that database. The install marker
(`keychain-install-marker`) goes in `Library/Application Support/dev.goldcoders.bir`. The logs of a test run are in
the container's `tmp/ebirforms-logs-<pid>/logs/`.

```sh
just mock-account --port 3008 --auto-approve                  # in another terminal
cd nsh
export PATH="$HOME/BuwizNsh/tools/bin:$PATH" CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh"
export SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
  SIMCTL_CHILD_BUWIZ_KEYCHAIN=1 SIMCTL_CHILD_BUWIZ_SERVER_URL=http://localhost:3008
# No BIR_AUTH_BYPASS: with it the app skips the account gate, and this check signs in through the gate.
unset SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN SIMCTL_CHILD_BIR_SFTP_LIVE SIMCTL_CHILD_BIR_AUTH_BYPASS
# 1. Install and launch once (scratch database), then quit. This is the fresh install.
nsh run --target ios
# 2. The container exists now; name the stable database in it. A reinstall gets a new container: read it again.
C="$(xcrun simctl get_app_container booted dev.goldcoders.bir data)"
export SIMCTL_CHILD_BIR_DATABASE_PATH="$C/Documents/buwiz-keychain-check/bir_data.db"
nsh run --target ios
```

`BUWIZ_KEYCHAIN=1 just nsh-run-agent ios` passes the same switch through the agent recipe.

The Simulator's Keychain is a file. Read the class of the items with
`sqlite3 ~/Library/Developer/CoreSimulator/Devices/<udid>/data/Library/Keychains/keychain-2-debug.db "select pdmn, agrp from genp"`.
Each row of this app should show `pdmn` = `cku` (`ak` is the old "when unlocked" class).

| Step | What it should show |
| --- | --- |
| First launch after install (step 1) | Log `Keychain at launch: Cleared`; the marker file exists; the data folder has the excluded-from-backup attribute. |
| Sign in through the gate (step 2); the mock approves by itself | Signed in. A row `cku` for the sign-in. No return is queued or sent in this check. |
| Relaunch, same variables | Still signed in. Log `Keychain at launch: Relaunch`; the rows are unchanged. |
| Delete the app (`xcrun simctl uninstall booted dev.goldcoders.bir`) and run steps 1 and 2 again | Signed out (new container). Log `Cleared`; the marker is written again; this app's rows are gone until you sign in again. |
| Upgrade from a build without the marker (or delete the marker file, then relaunch) | Log `Cleared` once: the three Keychain items go and the device key is new. The sign-in can still come back from `buwiz-cloud-account.json` beside the database, which the app always writes as well. |

The device key and its id are not created by signing in on this branch (they are made when a device is registered), so
this check shows the sign-in row only. Their rows are checked in the Simulator by the sign-in change that registers the
device right after the code is accepted, against `mock-account`'s devices endpoint: the same recipe, and after signing
in there are three `cku` rows, which a reinstall replaces. This check also runs inside the NSH development host in the
Simulator (bundle id `com.longbridge.nsh.host`): use that id for the container lookup and the uninstall, and note that
the rows sit in the host's access group, not the shipped app's, which only a TestFlight build shows.

## Screenshots

App Store Connect requires the iPhone 6.1"/6.3" set (1206×2622 or 1179×2556, e.g. iPhone 18 Pro) and the iPad 13" set (2064×2752 or 2048×2732). No alpha. Previews are 886×1920, 15–30 s, at most 30 fps.
