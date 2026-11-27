# Buwiz Forms logo and the app icons

`buwiz-forms.svg` is the one source for every app icon. The logo itself is made from its eight parts in
[`parts/`](parts/README.md). The logo is always the whole logo with its words, never the eagle alone.

## Changing the logo: the whole procedure

### (a) A part changes

1. Edit the part's file in `assets/brand/parts/`. [The parts README](parts/README.md) says what each part is and what
   to keep.
2. `just logo` rebuilds `assets/brand/buwiz-forms.svg` from the eight parts.
3. `just icons` writes every icon below again from that logo, and lists each file it wrote.
4. Look at a few: `assets/ios/icon-1024.png`, `assets/macos/icon-1024.png`, `assets/android/icon.png`,
   `assets/icon-app.targetsize-16.png`. Then `just icons-check` says the logo is exactly what its parts give and every
   icon matches the logo.
5. Commit the part, the logo and every file the commands changed, and open a pull request of them. Nothing else
   changes: no code, no `nsh/nsh.json`, no manifest.

### (b) A whole new drawing arrives as one SVG

1. `just icons /path/to/new-logo.svg` copies it to `assets/brand/buwiz-forms.svg` and writes every icon again from it.
2. `just icons-check` then fails, on purpose, with one line: "assets/brand/buwiz-forms.svg is not what
   assets/brand/parts/ give". The icons themselves match the new logo; only the parts are out of date. **Do not follow
   that line's hint to run `just logo`**, because that rebuilds the logo from the old parts and undoes the new drawing.
3. Put the new drawing's eight parts in `assets/brand/parts/`; the artwork session has them. `just logo` then gives
   the new SVG byte for byte, and `just icons-check` passes. Until the parts are replaced, the check keeps failing.
4. Commit the SVG, the parts and the icons, and open a pull request of them.

## The stores

The next build of each app carries the new icon. The App Store shows the icon from the build. So does the Microsoft
Store, unless Partner Center has store logos uploaded of its own: replace those too. Google Play's store listing icon is
uploaded by hand in Play Console: `assets/android/play-icon-512.png`.

## The commands

    just logo                         # rebuilds assets/brand/buwiz-forms.svg from assets/brand/parts/
    just icons                        # rebuilds every icon from assets/brand/buwiz-forms.svg
    just icons path/to/new-logo.svg   # takes in a new SVG first, then rebuilds every icon
    just icons-check                  # exit 1 when the logo is not what its parts give, or an icon not what the logo gives

These need `uv` (it fetches Pillow and NumPy) and `rsvg-convert` (`brew install librsvg`). `make_icons.py` writes each file from
the vector at its own size, prints what it wrote, and removes the old icons nothing ships any more. Placement uses only what
each render shows: the logo's bounding box, or for Android the smallest circle around it. So a redrawn SVG with other
proportions lands right with no change to the script. `icons-check` is a local command, not a CI job. Its icon comparison allows for another machine's librsvg drawing edges a
little differently, so it is a judgement, not an exact match. The logo-from-parts check is exact, because `merge.py`
pins its viewBox.

| Platform | Files | How |
| --- | --- | --- |
| iPhone, iPad | `assets/ios/icon-1024.png` | white square, opaque, no alpha channel; the logo's box at 80% (the size the owner picked) |
| Mac | `assets/macos/icon-1024.png`, `assets/AppIcon.icns` | the Mac's shape: white rounded tile 824 at (100, 100), radius 185, soft shadow; logo at 80% of the tile |
| Linux, web | `assets/icon.png`, `assets/linux/icons/hicolor/*/apps/dev.goldcoders.bir.png` | transparent, logo at 92% |
| Windows exe | `assets/icon-window.png` (nsh.json `icon.windows`; NSH makes the .ico) | transparent, logo at 96% |
| Windows MSIX | `assets/icon-store.*`, `icon-tile.*`, `icon-wide.*`, `icon-app.*` (scale and targetsize, plated and unplated) | plated: white, logo at 80%; unplated and lightunplated: transparent, 96% |
| Android | `assets/android/icon.png` (legacy, 1024), `adaptive-foreground.png` (432), `play-icon-512.png` | the logo's circle at radius 0.45, 128 px and 0.47 of the side; plated ones are opaque white |
| In the app | `nsh/shell/desktop/tray.png` (72), `nsh/shell/app-mark.png` (48, the Windows menu row) | transparent, the whole logo |
| Pages before the app | `nsh/assets/brand/buwiz-forms-logo.png`, `@2x`, `@3x` (166 by 180, 332 by 360, 498 by 540) | transparent, the whole drawing 180 points high (sign-in, code, PIN, setup and locked pages) |
