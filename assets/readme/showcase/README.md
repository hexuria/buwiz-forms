# README showcase

`assets/readme/showcase.png` shows the app on three devices: iPhone (sign-in), Mac (home), and iPad (BIR Form 2551Q). The devices are drawn in `showcase.html` around real screenshots of the app:

- `iphone.jpg`: iPhone 16 Pro Simulator, the sign-in page after signing out.
- `desktop.jpg`: the macOS window at 1440 × 900, home with sample taxpayers.
- `ipad.jpg`: iPad Pro 13" Simulator, from the App Store set, 2551Q Part II.

To redraw it, swap a screenshot and run this from this folder. It needs Chrome and ImageMagick:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --hide-scrollbars \
  --force-device-scale-factor=2 --default-background-color=00000000 --window-size=1900,760 \
  --screenshot=/tmp/showcase-raw.png "file://$PWD/showcase.html"
magick /tmp/showcase-raw.png -trim +repage -bordercolor none -border 24 ../showcase.png
```

The background is transparent, so the picture works on GitHub's light and dark themes.
