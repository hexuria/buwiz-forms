#!/usr/bin/env python3
"""Every app icon, made from one file: assets/brand/buwiz-forms.svg, the Buwiz Forms logo.

    just icons                 rebuild every icon from assets/brand/buwiz-forms.svg
    just icons path/to/new.svg copy that SVG in as the new source first, then rebuild
    just icons-check           exit 1 when a committed icon differs from what the SVG gives

or directly: uv run --no-project --with pillow python assets/brand/make_icons.py [--check]

The logo is always the whole logo with its words; no file here is ever a cut of it. Each picture is drawn from the vector at
its own size (rsvg-convert renders the SVG at the width the layout needs), never scaled from another picture. Placement uses
only what a render shows, measured afresh on every run, so a redrawn SVG with other proportions still lands right:

- box:    the logo's bounding box (every pixel it paints, its white outline included) fitted into a share of the canvas and
          centred. "PLATED" is the size of the master the owner picked (the logo's box at 80% of the square, which is the
          whole drawing at 88%).
- circle: Android's placement. The smallest circle around what shows on white (alpha above 25% and darker than 235, so
          the white outline is left out), centred on the canvas at a given radius.

Needs: Python 3 with Pillow (uv fetches it), and rsvg-convert (Homebrew `librsvg`, Debian `librsvg2-bin`). Nothing else:
the .icns and the Windows .ico are written by Pillow, so no Xcode tool is needed.

The logo itself is made from its eight parts in assets/brand/parts/ (`just logo`, see the README there); --check also
checks that buwiz-forms.svg is byte for byte what those parts give.

--check renders everything into a scratch folder and compares pixels with the committed files, allowing the small
anti-aliasing differences between rsvg-convert versions. It is a local command, not a CI job: another machine's librsvg and
cairo can draw edges differently enough to be noise, and CI would then fail on nothing.
"""
from __future__ import annotations

import argparse
import functools
import math
import os
import random
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageMath

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SVG = os.path.join(HERE, "buwiz-forms.svg")

PLATED = 0.80  # the master's size: the logo's box at 80% of a white square
UNPLATED = 0.96  # transparent icons that stand alone (taskbar, Start, a window's title bar)
SHARED = 0.92  # assets/icon.png and the Linux icons: transparent, a little margin
REF = 2048  # the width the logo is measured at

WHITE = (255, 255, 255, 255)
SCALES = (100, 125, 150, 200, 400)
TARGETSIZES = (16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256)

# Files an earlier icon set left that nothing should ship any more. --check fails while one is there.
GONE = (
    "assets/icon.ico",
    "assets/icon.targetsize-44.png",
    "assets/icon.targetsize-44_altform-unplated.png",
    "assets/icon.targetsize-48_altform-unplated.png",
    "assets/icon.targetsize-256_altform-unplated.png",
    "assets/ios/icon-ios.svg",
    "nsh/assets/icons/e_logo.svg",
)


# ---- the logo, rendered and measured ---------------------------------------------------------------------------------


class Logo:
    def __init__(self, svg: str, tmp: str):
        self.svg = svg
        self.tmp = tmp
        ref = self.render(REF)
        self.aspect = ref.height / ref.width
        alpha = ref.getchannel("A")
        box = alpha.point(lambda v: 255 if v >= 8 else 0).getbbox()
        if box is None:
            raise SystemExit(f"{svg}: the logo paints nothing")
        # The bounding box, as fractions of the render's width and height.
        self.box = (box[0] / ref.width, box[1] / ref.height, box[2] / ref.width, box[3] / ref.height)
        self.circle = enclosing_circle(ref)  # (cx, cy, r) as fractions of the render's width

    @functools.lru_cache(maxsize=None)
    def render(self, width: int) -> Image.Image:
        path = os.path.join(self.tmp, f"logo-{width}.png")
        subprocess.run(["rsvg-convert", "--width", str(width), "--keep-aspect-ratio", self.svg, "-o", path], check=True)
        im = Image.open(path).convert("RGBA")
        im.load()
        return im

    def boxed(self, canvas: Image.Image, area: tuple[float, float, float, float], fill: float) -> None:
        """The logo's bounding box fitted into `fill` of `area` (x, y, w, h on the canvas), centred there."""
        ax, ay, aw, ah = area
        bw = self.box[2] - self.box[0]
        bh = (self.box[3] - self.box[1]) * self.aspect
        width = max(1, round(min(fill * aw / bw, fill * ah / bh)))
        im = self.render(width)
        cx = (self.box[0] + self.box[2]) / 2 * im.width
        cy = (self.box[1] + self.box[3]) / 2 * im.height
        canvas.alpha_composite(im, (round(ax + aw / 2 - cx), round(ay + ah / 2 - cy)))

    def circled(self, canvas: Image.Image, radius: float) -> None:
        """The logo's circle (see enclosing_circle) at `radius` pixels, centred on the canvas."""
        cx, cy, r = self.circle
        width = max(1, round(radius / r))
        im = self.render(width)
        canvas.alpha_composite(im, (round(canvas.width / 2 - cx * width), round(canvas.height / 2 - cy * width)))


def enclosing_circle(ref: Image.Image) -> tuple[float, float, float]:
    """The smallest circle around what shows on white: alpha above 25% and not near-white (the outline is left out).
    Only the extreme pixel of each row and column can lie on it, so those, reduced to their convex hull, go to Welzl."""
    w, h = ref.size
    r, g, b, a = ref.split()
    shows = ImageMath.lambda_eval(
        lambda v: ((v["r"] + v["g"] + v["b"]) < 705) & (v["a"] > 64), r=r, g=g, b=b, a=a
    ).convert("L").point(lambda v: 255 if v else 0)  # point() on an "I" image is linear only: convert first
    points = set()
    for y in range(h):
        row = shows.crop((0, y, w, y + 1)).getbbox()
        if row:
            points.add((row[0], y))
            points.add((row[2] - 1, y))
    for x in range(w):
        col = shows.crop((x, 0, x + 1, h)).getbbox()
        if col:
            points.add((x, col[1]))
            points.add((x, col[3] - 1))
    if not points:
        raise SystemExit("the logo shows nothing darker than its outline")
    hull = convex_hull(sorted(points))
    rng = random.Random(0)
    rng.shuffle(hull)
    cx, cy, r = welzl(hull)
    # Pixel centres: +0.5. The radius grows by half a pixel's diagonal so every pixel is inside.
    return ((cx + 0.5) / w, (cy + 0.5) / w, (r + 0.71) / w)


def convex_hull(points: list[tuple[int, int]]) -> list[tuple[int, int]]:
    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in points:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(points):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def welzl(points: list[tuple[int, int]]) -> tuple[float, float, float]:
    def inside(c, p):
        return math.hypot(p[0] - c[0], p[1] - c[1]) <= c[2] + 1e-9

    def two(a, b):
        cx, cy = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
        return (cx, cy, math.hypot(a[0] - cx, a[1] - cy))

    def three(a, b, c):
        d = 2 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]))
        if abs(d) < 1e-12:
            return max((two(a, b), two(a, c), two(b, c)), key=lambda k: k[2])
        ux = ((a[0] ** 2 + a[1] ** 2) * (b[1] - c[1]) + (b[0] ** 2 + b[1] ** 2) * (c[1] - a[1]) + (c[0] ** 2 + c[1] ** 2) * (a[1] - b[1])) / d
        uy = ((a[0] ** 2 + a[1] ** 2) * (c[0] - b[0]) + (b[0] ** 2 + b[1] ** 2) * (a[0] - c[0]) + (c[0] ** 2 + c[1] ** 2) * (b[0] - a[0])) / d
        return (ux, uy, math.hypot(a[0] - ux, a[1] - uy))

    c = (points[0][0], points[0][1], 0.0)
    for i, p in enumerate(points):
        if inside(c, p):
            continue
        c = (p[0], p[1], 0.0)
        for j in range(i):
            q = points[j]
            if inside(c, q):
                continue
            c = two(p, q)
            for k in range(j):
                if not inside(c, points[k]):
                    c = three(p, q, points[k])
    return c


# ---- the layouts -----------------------------------------------------------------------------------------------------


def transparent(logo: Logo, w: int, h: int, fill: float) -> Image.Image:
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    logo.boxed(im, (0, 0, w, h), fill)
    return im


def plated(logo: Logo, w: int, h: int, fill: float = PLATED) -> Image.Image:
    """White, opaque, the logo boxed. A wide tile fits the logo by its height."""
    im = Image.new("RGBA", (w, h), WHITE)
    logo.boxed(im, (0, 0, w, h), fill)
    return im


def circled(logo: Logo, side: int, radius: float, white: bool) -> Image.Image:
    im = Image.new("RGBA", (side, side), WHITE if white else (0, 0, 0, 0))
    logo.circled(im, radius)
    return im


def mac(logo: Logo, side: int) -> Image.Image:
    """The Mac's own shape (Apple's grid on a 1024 canvas): a white rounded tile of 824 at (100, 100), corner radius 185,
    a soft shadow below it, the logo boxed on the tile. Drawn at 4x and reduced, so the tile's edge and shadow are smooth;
    the logo itself is rendered from the vector at that 4x size, never enlarged."""
    k = side / 1024
    s = 4
    big = side * s
    tile, at, radius = 824 * k * s, 100 * k * s, 185 * k * s
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle((at, at, at + tile - 1, at + tile - 1), radius=radius, fill=255)
    shadow = Image.new("L", (big, big), 0)
    shadow.paste(mask, (0, round(12 * k * s)))
    shadow = shadow.filter(ImageFilter.GaussianBlur(max(0.5, 14 * k * s))).point(lambda v: int(v * 0.30))
    im = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    im.putalpha(shadow)
    face = Image.new("RGBA", (big, big), WHITE)
    logo.boxed(face, (at, at, tile, tile), PLATED)
    face.putalpha(ImageChops.multiply(face.getchannel("A"), mask))
    im = Image.alpha_composite(im, face)
    return im.resize((side, side), Image.Resampling.LANCZOS)


# ---- every file ------------------------------------------------------------------------------------------------------


def outputs(logo: Logo) -> dict[str, object]:
    """Path → a PNG image, or ("icns" | "ico", {size: image}). Paths are from the repository root."""
    out: dict[str, object] = {}

    # Apple. iPhone and iPad: square, opaque, no alpha channel at all (actool refuses one); iOS rounds the corners.
    out["assets/ios/icon-1024.png"] = plated(logo, 1024, 1024).convert("RGB")
    out["assets/macos/icon-1024.png"] = mac(logo, 1024)
    out["assets/AppIcon.icns"] = ("icns", {n: mac(logo, n) for n in (16, 32, 64, 128, 256, 512, 1024)})

    # Shared: nsh.json's icon for Linux and the web, and the Linux desktop's own sizes.
    out["assets/icon.png"] = transparent(logo, 512, 512, SHARED)
    for n in (48, 64, 128, 256, 512):
        out[f"assets/linux/icons/hicolor/{n}x{n}/apps/dev.goldcoders.bir.png"] = transparent(logo, n, n, SHARED)

    # Windows. The exe's icon (nsh.json icon.windows; NSH makes the .ico from it) and the MSIX logos
    # (assets/windows/AppxManifest.xml). package_msix.ps1 copies assets/icon*.png into the package.
    out["assets/icon-window.png"] = transparent(logo, 512, 512, UNPLATED)
    for scale, n in zip(SCALES, (50, 63, 75, 100, 200)):
        out[f"assets/icon-store.scale-{scale}.png"] = plated(logo, n, n)
    for scale, n in zip(SCALES, (150, 188, 225, 300, 600)):
        out[f"assets/icon-tile.scale-{scale}.png"] = plated(logo, n, n)
    for scale, (w, h) in zip(SCALES, ((310, 150), (388, 188), (465, 225), (620, 300), (1240, 600))):
        out[f"assets/icon-wide.scale-{scale}.png"] = plated(logo, w, h)
    for scale, n in zip(SCALES, (44, 55, 66, 88, 176)):
        out[f"assets/icon-app.scale-{scale}.png"] = plated(logo, n, n)
    for n in TARGETSIZES:
        out[f"assets/icon-app.targetsize-{n}.png"] = plated(logo, n, n)
        out[f"assets/icon-app.targetsize-{n}_altform-unplated.png"] = transparent(logo, n, n, UNPLATED)
        out[f"assets/icon-app.targetsize-{n}_altform-lightunplated.png"] = transparent(logo, n, n, UNPLATED)

    # Android. The legacy icon NSH turns into ic_launcher (plated; Android 8+ wraps it in its mask on white), the
    # adaptive foreground (108 dp at xxxhdpi, background #ffffff; inside the 66 dp safe circle), and Google Play's 512
    # (opaque, but RGBA: Play asks for a 32-bit PNG; Play rounds the corners).
    out["assets/android/icon.png"] = circled(logo, 1024, 0.45 * 1024, white=True).convert("RGB")
    out["assets/android/adaptive-foreground.png"] = circled(logo, 432, 128, white=False)
    out["assets/android/play-icon-512.png"] = circled(logo, 512, 0.47 * 512, white=True)

    # Inside the app. The tray (every desktop; NSH fits it in 18 points on the Mac, so 72 px serves up to @4x) and the
    # Windows menu row's mark (16 points, 48 px for @3x). Both the whole logo, in colour.
    out["nsh/shell/desktop/tray.png"] = transparent(logo, 72, 72, 1.0)
    out["nsh/shell/app-mark.png"] = transparent(logo, 48, 48, 1.0)
    return out


def write(path: str, item: object) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if isinstance(item, tuple):
        kind, frames = item
        sizes = sorted(frames)
        largest = frames[sizes[-1]]
        others = [frames[n] for n in sizes[:-1]]
        if kind == "icns":
            largest.save(path, format="ICNS", append_images=others)
        else:
            largest.save(path, format="ICO", sizes=[(n, n) for n in sizes], append_images=others)
    else:
        item.save(path, format="PNG")


def describe(item: object) -> str:
    if isinstance(item, tuple):
        kind, frames = item
        return f"{kind} " + ", ".join(str(n) for n in sorted(frames))
    return f"{item.width}x{item.height} {item.mode}"


# ---- --check ---------------------------------------------------------------------------------------------------------


def frames_of(path: str) -> dict[tuple[int, int], Image.Image]:
    im = Image.open(path)
    if im.format == "ICNS":
        found = {}
        for w, h, scale in sorted(im.info.get("sizes", [])):
            frame = Image.open(path)
            frame.size = (w, h)
            frame.load(scale=scale)
            found.setdefault(frame.size, frame.convert("RGBA").copy())
        return found
    if im.format == "ICO":
        return {size: im.ico.getimage(size).convert("RGBA") for size in im.ico.sizes()}
    return {im.size: im.convert("RGBA")}


def same(a: Image.Image, b: Image.Image) -> bool:
    """Equal up to anti-aliasing noise: a mean difference under 1 level and under 0.5% of pixels off by more than 32."""
    if a.size != b.size:
        return False
    diff = ImageChops.difference(a, b)
    total = 0
    far = 0
    for band in diff.split():
        hist = band.histogram()
        total += sum(i * c for i, c in enumerate(hist))
        far += sum(hist[33:])
    pixels = a.width * a.height * 4
    return total / pixels < 1.0 and far / pixels < 0.005


def logo_check() -> list[str]:
    """The logo must be what its parts give: assets/brand/parts/merge.py, run into a scratch file, byte for byte. Skipped
    (with a line) when the parts are not here or NumPy is missing; `just icons-check` brings NumPy."""
    merge = os.path.join(HERE, "parts", "merge.py")
    if not os.path.exists(merge):
        return []
    try:
        import numpy  # noqa: F401  (merge.py needs it)
    except ImportError:
        print("logo not checked against its parts: NumPy is missing (run `just icons-check`)")
        return []
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "logo.svg")
        subprocess.run([sys.executable, merge, out], check=True, stdout=subprocess.DEVNULL)
        with open(out, "rb") as a, open(SVG, "rb") as b:
            if a.read() != b.read():
                return ["assets/brand/buwiz-forms.svg is not what assets/brand/parts/ give: run `just logo`, then `just icons`"]
    print("assets/brand/buwiz-forms.svg is exactly what assets/brand/parts/ give")
    return []


def check(made: dict[str, str]) -> int:
    bad = []
    for rel, fresh in sorted(made.items()):
        committed = os.path.join(ROOT, rel)
        if not os.path.exists(committed):
            bad.append(f"missing: {rel}")
            continue
        want, have = frames_of(fresh), frames_of(committed)
        if set(want) != set(have):
            bad.append(f"sizes differ: {rel}: {sorted(have)} committed, {sorted(want)} from the SVG")
            continue
        for size in want:
            if not same(want[size], have[size]):
                bad.append(f"differs from the SVG: {rel} ({size[0]}x{size[1]})")
    for rel in GONE:
        if os.path.exists(os.path.join(ROOT, rel)):
            bad.append(f"left over, delete it: {rel}")
    bad += logo_check()
    if bad:
        print("\n".join(bad))
        print(f"{len(bad)} icon(s) out of step with assets/brand/buwiz-forms.svg: run `just icons`.")
        return 1
    print(f"all {len(made)} icons match assets/brand/buwiz-forms.svg")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="compare the committed icons with the SVG; change nothing")
    args = parser.parse_args()
    if shutil.which("rsvg-convert") is None:
        raise SystemExit("rsvg-convert is needed: brew install librsvg (macOS) or apt install librsvg2-bin (Linux)")
    with tempfile.TemporaryDirectory() as tmp:
        logo = Logo(SVG, tmp)
        cx, cy, r = logo.circle
        print(f"logo: box {tuple(round(v, 4) for v in logo.box)}, circle centre ({cx:.4f}, {cy:.4f}) radius {r:.4f}")
        items = outputs(logo)
        if args.check:
            made = {}
            for rel, item in items.items():
                path = os.path.join(tmp, "out", rel)
                write(path, item)
                made[rel] = path
            return check(made)
        for rel, item in items.items():
            path = os.path.join(ROOT, rel)
            write(path, item)
            print(f"wrote {rel}  {describe(item)}  {os.path.getsize(path):,} bytes")
        for rel in GONE:
            path = os.path.join(ROOT, rel)
            if os.path.exists(path):
                os.remove(path)
                print(f"removed {rel}")
    print(f"{len(items)} files from {os.path.relpath(SVG, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
