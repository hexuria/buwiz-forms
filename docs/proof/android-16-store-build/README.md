# Android 16 proof — store-shaped build of PR #332 (229c65a5)

Built with `nsh package --target android --abi x86_64 --id com.buwiz.ebirforms --run --out .nsh/out-play-x86`
from the app's `nsh` folder at 229c65a5, NSH sibling at pin 8c78cfd6, no development variables
(BUWIZ_ANDROID_DEV / BUWIZ_ANDROID_DEV_ENV unset). Auto debug key only. Installed on an Android 16
(API 36) emulator: phone first, then a Pixel Tablet, one at a time.

Device note: this SDK's cmdline-tools bundle has no `pixel_8` device profile, so the phone AVD uses
the `pixel_7` profile (same 1080x2400/420 class) with the android-36 image; the tablet uses `pixel_tablet`.

## Files

| file | what it shows |
|---|---|
| dumpsys-package.txt | `adb shell dumpsys package com.buwiz.ebirforms`: versionCode=27 minSdk=26 targetSdk=36; flags lines without ALLOW_BACKUP |
| bmgr-backupnow.txt | `bmgr enable true` then `bmgr backupnow com.buwiz.ebirforms` → "Backup is not allowed" — allowBackup=false is in effect |
| dumpsys-backup-grep.txt | note: `dumpsys backup | grep -i ebirforms` prints nothing (consistent with refusal) |
| first-screen-portrait.png | phone, first screen portrait: nothing drawn under or cut by the status/navigation bars |
| first-screen-landscape.png | phone, first screen landscape: same, layout correct |
| android16-phone.mp4 | phone: cold launch, first screen, rotation portrait→landscape→portrait, back gesture to the launcher |
| android16-tablet.mp4 | tablet: launch in landscape, rotation to portrait and back |
| tablet-landscape.png / tablet-portrait.png | tablet first screen both ways: layout follows both orientations |

Rotation capture note: adb screenrecord fixes the video frame at the orientation active when the
recording starts, so mid-video rotations appear as the rotation animation inside the fixed frame;
the stable rotated layouts are proven by the full-size screenshots. Rotation on the emulator needs
`settings put system accelerometer_rotation 0` plus a change of `user_rotation`, and only re-evaluates
when the foreground app allows rotation (the launcher does not).
