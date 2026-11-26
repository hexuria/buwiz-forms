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

Sending: on iOS the send guard forces a dry run (`nsh/src/guards.rs`), so a store build prepares and prints returns
and sends nothing until relay filing ships.

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

## Screenshots

App Store Connect requires the iPhone 6.1"/6.3" set (1206×2622 or 1179×2556, e.g. iPhone 18 Pro) and the iPad 13" set (2064×2752 or 2048×2732). No alpha. Previews are 886×1920, 15–30 s, at most 30 fps.
