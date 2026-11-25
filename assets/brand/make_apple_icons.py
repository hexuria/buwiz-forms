#!/usr/bin/env python3
"""The Apple app icons, made from the Buwiz Forms logo (the whole logo, words included; never the eagle alone).

Sources, both in this folder:
  buwiz-forms.svg    the vector logo (transparent, with its white outline)
  app-icon-1024.png  the master the owner picked: a white square with the logo at 88%

Outputs (run from the repository root: `python3 assets/brand/make_apple_icons.py`):
  assets/ios/icon-1024.png    iPhone and iPad: the master as it is. Square, opaque, no rounded corners;
                              iOS cuts the corners itself.
  assets/macos/icon-1024.png  the Mac: the Mac's own shape on a 1024 canvas. A white rounded-square tile of 824
                              at (100, 100) with a 185 corner radius and a soft shadow below. The logo sits on it at
                              88% of the tile, as on iOS. It is drawn from the vector at 4x and scaled down.
  assets/AppIcon.icns         the same Mac icon as an icns (every size iconutil wants)

Needs rsvg-convert, Pillow, and on macOS sips and iconutil.
"""
import os
import shutil
import subprocess
import tempfile

from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SVG = os.path.join(HERE, "buwiz-forms.svg")
MASTER = os.path.join(HERE, "app-icon-1024.png")

SCALE = 4  # draw at 4096, then scale down: every size comes from the vector, never from a smaller picture
CANVAS = 1024
TILE = 824
TILE_AT = 100
RADIUS = 185
LOGO = 0.88  # the logo's square on the tile, as on the master
SHADOW_DY = 12
SHADOW_BLUR = 14
SHADOW_ALPHA = 0.30


def ios() -> None:
    master = Image.open(MASTER)
    assert master.size == (CANVAS, CANVAS), master.size
    assert master.mode == "RGB", f"the master must have no alpha channel, it is {master.mode}"
    out = os.path.join(ROOT, "assets/ios/icon-1024.png")
    shutil.copyfile(MASTER, out)


def render_logo(px: int, tmp: str) -> Image.Image:
    path = os.path.join(tmp, f"logo-{px}.png")
    subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), SVG, "-o", path], check=True)
    return Image.open(path).convert("RGBA")


def macos(tmp: str) -> str:
    s = SCALE
    size, tile, at, radius = CANVAS * s, TILE * s, TILE_AT * s, RADIUS * s
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((at, at, at + tile - 1, at + tile - 1), radius=radius, fill=255)

    # The shadow: the tile's shape, blurred, dropped a little, at 30% black.
    shadow_mask = Image.new("L", (size, size), 0)
    shadow_mask.paste(mask, (0, SHADOW_DY * s))
    shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(SHADOW_BLUR * s))
    shadow_mask = shadow_mask.point(lambda v: int(v * SHADOW_ALPHA))
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.putalpha(shadow_mask)

    # The tile: white, with the logo centred on it.
    face = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    logo_px = round(tile * LOGO)
    logo = render_logo(logo_px, tmp)
    face.alpha_composite(logo, (at + (tile - logo_px) // 2, at + (tile - logo_px) // 2))
    face.putalpha(ImageChops.multiply(face.getchannel("A"), mask))
    icon = Image.alpha_composite(icon, face)

    icon = icon.resize((CANVAS, CANVAS), Image.Resampling.LANCZOS)
    out = os.path.join(ROOT, "assets/macos/icon-1024.png")
    icon.save(out, optimize=True)
    return out


def icns(png: str, tmp: str) -> None:
    if shutil.which("iconutil") is None:
        print("no iconutil here: assets/AppIcon.icns not remade")
        return
    big = Image.open(png)
    iconset = os.path.join(tmp, "AppIcon.iconset")
    os.makedirs(iconset)
    for pt in (16, 32, 128, 256, 512):
        for mul in (1, 2):
            px = pt * mul
            name = f"icon_{pt}x{pt}{'@2x' if mul == 2 else ''}.png"
            big.resize((px, px), Image.Resampling.LANCZOS).save(os.path.join(iconset, name))
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(ROOT, "assets/AppIcon.icns")], check=True)


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        ios()
        mac = macos(tmp)
        icns(mac, tmp)
    print("made assets/ios/icon-1024.png, assets/macos/icon-1024.png, assets/AppIcon.icns")


if __name__ == "__main__":
    main()
