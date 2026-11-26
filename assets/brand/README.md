# Buwiz Forms logo and the app icons

`buwiz-forms.svg` is the one source for every app icon. The logo is always the whole logo with its words, never the
eagle alone.

## A new logo: the whole procedure

1. `just icons /path/to/new-logo.svg`. It copies the SVG to `assets/brand/buwiz-forms.svg`, writes every icon below
   again, and lists each file it wrote.
2. Look at a few: `assets/ios/icon-1024.png`, `assets/macos/icon-1024.png`, `assets/android/icon.png`,
   `assets/icon-app.targetsize-16.png`. `just icons-check` should then say every icon matches.
3. Commit the SVG and every file the command changed, and open a pull request of them. Nothing else changes: no code, no
   `nsh/nsh.json`, no manifest.

The next build of each app carries the new icon. The App Store shows the icon from the build. So does the Microsoft
Store, unless Partner Center has store logos uploaded of its own: replace those too. Google Play's store listing icon is
uploaded by hand in Play Console: `assets/android/play-icon-512.png`.

The other commands:

    just icons                        # rebuilds everything from the SVG already here
    just icons-check                  # exit 1 when a committed icon is not what the SVG gives

This needs `uv` (it fetches Pillow) and `rsvg-convert` (`brew install librsvg`). `make_icons.py` writes each file from
the vector at its own size, prints what it wrote, and removes the old icons nothing ships any more. Placement uses only what
each render shows: the logo's bounding box, or for Android the smallest circle around it. So a redrawn SVG with other
proportions lands right with no change to the script. `icons-check` is a local command, not a CI job, because another
machine's librsvg can draw edges a little differently.

| Platform | Files | How |
| --- | --- | --- |
| iPhone, iPad | `assets/ios/icon-1024.png` | white square, opaque, no alpha channel; the logo's box at 80% (the size the owner picked) |
| Mac | `assets/macos/icon-1024.png`, `assets/AppIcon.icns` | the Mac's shape: white rounded tile 824 at (100, 100), radius 185, soft shadow; logo at 80% of the tile |
| Linux, web | `assets/icon.png`, `assets/linux/icons/hicolor/*/apps/dev.goldcoders.bir.png` | transparent, logo at 92% |
| Windows exe | `assets/icon-window.png` (nsh.json `icon.windows`; NSH makes the .ico) | transparent, logo at 96% |
| Windows MSIX | `assets/icon-store.*`, `icon-tile.*`, `icon-wide.*`, `icon-app.*` (scale and targetsize, plated and unplated) | plated: white, logo at 80%; unplated and lightunplated: transparent, 96% |
| Android | `assets/android/icon.png` (legacy, 1024), `adaptive-foreground.png` (432), `play-icon-512.png` | the logo's circle at radius 0.45, 128 px and 0.47 of the side; plated ones are opaque white |
| In the app | `nsh/shell/desktop/tray.png` (72), `nsh/shell/app-mark.png` (48, the Windows menu row) | transparent, the whole logo |
