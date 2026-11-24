# iOS: App Store package

The iOS app is the NSH app in `nsh/`, packaged by `nsh package --target ios`. Development runs use the Simulator
(`just nsh-run-agent ios`, [devin-environment.md](devin-environment.md)). This page covers the App Store package.

## What the package carries

| Piece | Where | Why |
| --- | --- | --- |
| Devices | `nsh/nsh.json` `ios.infoPlist.UIDeviceFamily` | `[1, 2]`: iPhone and iPad. NSH gives iPad all four orientations (iPad multitasking needs them) and iPhone portrait and both landscapes; the launch screen and the icon serve both (`actool` makes the iPad sizes from the 1024 image). iPad needs its own 13-inch screenshots. |
| Icon | `assets/ios/icon-1024.png` (from `icon-ios.svg`) | Apple wants 1024×1024 with no alpha. The desktop `assets/icon.png` (512, transparent) fails `actool`. |
| Launch screen | `nsh/nsh.json` `ios.launchScreen` | The app's own background, light and dark, so launch shows no flash. |
| Export compliance | `ios.infoPlist.ITSAppUsesNonExemptEncryption` | `false`, as the Mac App Store build says. |
| Privacy manifest | `assets/ios/PrivacyInfo.xcprivacy`, copied to the bundle root | Required-reason APIs (UserDefaults, boot time, file timestamps) and the account email. Keep it in step with App Privacy in App Store Connect. |
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

## Screenshots

App Store Connect requires the iPhone 6.1"/6.3" set (1206×2622 or 1179×2556, e.g. iPhone 18 Pro) and the iPad 13" set (2064×2752 or 2048×2732). No alpha. Previews are 886×1920, 15–30 s, at most 30 fps.
