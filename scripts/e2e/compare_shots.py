"""Compare two e2e tour runs picture by picture (the before-and-after look).

Two run folders in (the runner's evidence directories, each holding one folder per recipe with
NNN-sN.png screenshots), a table out: one line per pair that exists in both, the screen, whether it
is identical, the differing-pixel count and the bounding box of the differences. For each differing
pair a side-by-side picture is written. Needs Pillow (`uv run -q --no-project --with 'pillow>=11.1'
python scripts/e2e/compare_shots.py BEFORE AFTER [--out DIR] [--mask-top N] [--mask-bottom N]
[--allow-diff]`, the same way the icons recipe fetches it). Exits 1 when anything differs
(--allow-diff keeps the old exit-0-always behaviour); --out is refused inside either input folder.

Noise: the status bar strip at the top carries the clock, the bottom strip carries toasts:
`--mask-top N` / `--mask-bottom N` ignore those rows in the comparison (never in the pictures).
Toasts and the text caret still differ run to run: read small counts in their area as noise,
not layout.
"""

import argparse
import json
import os
import sys

from PIL import Image, ImageChops


def collect(root):
    """Every screenshot of a run folder keyed by its path relative to the run folder (slice
    subfolders keep their names, so nothing collides); failure.png is not a tour picture."""
    shots = {}
    for dirpath, _dirs, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith(".png") or name == "failure.png":
                continue
            full = os.path.join(dirpath, name)
            shots.setdefault(os.path.relpath(full, root), full)
    return shots


def compare_pair(before_path, after_path, mask_top=0, mask_bottom=0):
    a = Image.open(before_path).convert("RGBA")
    b = Image.open(after_path).convert("RGBA")
    if a.size != b.size:
        return {"same_size": False, "size_before": list(a.size), "size_after": list(b.size)}
    diff = ImageChops.difference(a, b)
    width, height = diff.size
    if mask_top:
        diff.paste(Image.new("RGBA", (width, min(mask_top, height))), (0, 0))
    if mask_bottom:
        top = max(height - mask_bottom, 0)
        diff.paste(Image.new("RGBA", (width, height - top)), (0, top))
    # a pixel differs when ANY band differs (luminance misses small single-channel moves);
    # the alpha band is opaque in every window capture, so RGB carries the whole picture.
    # NB: getbbox() on an RGBA image keys on the alpha band only, which is why the box
    # comes from the per-band maximum, not the diff image itself.
    bands = diff.split()
    extrema = bands[0]
    for band in bands[1:3]:
        extrema = ImageChops.lighter(extrema, band)
    bbox = extrema.getbbox()
    if not bbox:
        return {"same_size": True, "identical": True, "diff_pixels": 0}
    nonzero = sum(extrema.histogram()[1:])
    return {"same_size": True, "identical": False, "diff_pixels": nonzero, "bbox": list(bbox),
            "diff_fraction": round(nonzero / (width * height), 6)}


def side_by_side(before_path, after_path, out_path):
    a = Image.open(before_path).convert("RGB")
    b = Image.open(after_path).convert("RGB")
    width, height = a.size
    gap = 8
    canvas = Image.new("RGB", (width * 2 + gap, height), (200, 40, 40))
    canvas.paste(a, (0, 0))
    canvas.paste(b, (width + gap, 0))
    canvas.save(out_path)
    return out_path


def compare_runs(before_root, after_root, out_dir=None, mask_top=0, mask_bottom=0):
    before, after = collect(before_root), collect(after_root)
    rows = []
    for key in sorted(set(before) & set(after)):
        row = {"pair": key, **compare_pair(before[key], after[key], mask_top, mask_bottom)}
        rows.append(row)
        if out_dir and row.get("same_size") and not row.get("identical", True):
            os.makedirs(out_dir, exist_ok=True)
            side_by_side(before[key], after[key],
                         os.path.join(out_dir, key.replace(os.sep, "_").replace("/", "_")))
    result = {"rows": rows,
              "only_before": sorted(set(before) - set(after)),
              "only_after": sorted(set(after) - set(before))}
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
        with open(os.path.join(out_dir, "compare.json"), "w") as fh:
            json.dump(result, fh, indent=1)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description="Compare two e2e tour run folders picture by picture.")
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--out")
    parser.add_argument("--mask-top", type=int, default=0)
    parser.add_argument("--mask-bottom", type=int, default=0)
    parser.add_argument("--allow-diff", action="store_true",
                        help="exit 0 even when pairs differ (the old behaviour)")
    args = parser.parse_args(argv)
    for root in (os.path.realpath(args.before), os.path.realpath(args.after)):
        if args.out and os.path.realpath(args.out).startswith(root + os.sep):
            parser.error("--out must not be inside an input folder")
    result = compare_runs(args.before, args.after, args.out, args.mask_top, args.mask_bottom)
    print(f"{'pair':48s} {'result':10s} {'diff-px':>9s}  box")
    for row in result["rows"]:
        tag = "identical" if row.get("identical") else ("differs" if row.get("same_size") else "SIZE-DIFF")
        print(f"{row['pair']:48s} {tag:10s} {row.get('diff_pixels', 0):>9d}  {row.get('bbox', '-')}")
    if result["only_before"] or result["only_after"]:
        print(f"only before: {len(result['only_before'])}, only after: {len(result['only_after'])}")
    differs = any(not r.get("identical") for r in result["rows"]) or result["only_before"] or result["only_after"]
    return 0 if (args.allow_diff or not differs) else 1


if __name__ == "__main__":
    sys.exit(main())
