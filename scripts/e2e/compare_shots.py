"""Compare two e2e tour runs picture by picture (the before-and-after look).

Two run folders in (the runner's evidence directories, each holding one folder per recipe with
NNN-sN.png screenshots), a table out: one line per pair that exists in both, the screen, whether it
is identical, the differing-pixel count and the bounding box of the differences. For each differing
pair a side-by-side picture is written. Needs Pillow (`uv run -q --no-project --with 'pillow>=11.1'
python scripts/e2e/compare_shots.py BEFORE AFTER [--out DIR] [--mask-top N]`, the same way the icons
recipe fetches it).

Noise: the status bar strip at the top carries the clock; `--mask-top N` ignores the topmost N rows
of pixels (and the same strip in the side-by-side). Toasts and the text caret still differ run to
run: read small counts in their area as noise, not layout.
"""

import json
import os
import sys

from PIL import Image, ImageChops


def collect(root):
    """Every screenshot of a run folder keyed '<recipe>/<shot-file>' (slice subfolders flattened)."""
    shots = {}
    for dirpath, _dirs, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith(".png"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, name), root)
            parts = rel.split(os.sep)
            shots.setdefault(parts[0] + "/" + parts[-1], os.path.join(dirpath, name))
    return shots


def compare_pair(before_path, after_path, mask_top=0):
    a = Image.open(before_path).convert("RGB")
    b = Image.open(after_path).convert("RGB")
    if a.size != b.size:
        return {"same_size": False, "size_before": list(a.size), "size_after": list(b.size)}
    diff = ImageChops.difference(a, b)
    if mask_top:
        width, height = diff.size
        black = Image.new("RGB", (width, min(mask_top, height)))
        diff.paste(black, (0, 0))
    bbox = diff.getbbox()
    if not bbox:
        return {"same_size": True, "identical": True, "diff_pixels": 0}
    gray = diff.convert("L")
    nonzero = sum(gray.histogram()[1:])
    return {"same_size": True, "identical": False, "diff_pixels": nonzero, "bbox": list(bbox),
            "diff_fraction": round(nonzero / (a.size[0] * a.size[1]), 6)}


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


def compare_runs(before_root, after_root, out_dir=None, mask_top=0):
    before, after = collect(before_root), collect(after_root)
    rows = []
    for key in sorted(set(before) & set(after)):
        row = {"pair": key, **compare_pair(before[key], after[key], mask_top)}
        rows.append(row)
        if out_dir and row.get("same_size") and not row.get("identical", True):
            os.makedirs(out_dir, exist_ok=True)
            side_by_side(before[key], after[key], os.path.join(out_dir, key.replace("/", "_")))
    result = {"rows": rows,
            "only_before": sorted(set(before) - set(after)),
            "only_after": sorted(set(after) - set(before))}
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
        with open(os.path.join(out_dir, "compare.json"), "w") as fh:
            json.dump(result, fh, indent=1)
    return result


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    before, after = argv[1], argv[2]
    out_dir = None
    mask_top = 0
    args = argv[3:]
    while args:
        if args[0] == "--out":
            out_dir = args[1]
            args = args[2:]
        elif args[0] == "--mask-top":
            mask_top = int(args[1])
            args = args[2:]
        else:
            args = args[1:]
    result = compare_runs(before, after, out_dir, mask_top)
    print(f"{'pair':40s} {'result':10s} {'diff-px':>9s}  box")
    for row in result["rows"]:
        tag = "identical" if row.get("identical") else ("differs" if row.get("same_size") else "SIZE-DIFF")
        print(f"{row['pair']:40s} {tag:10s} {row.get('diff_pixels', 0):>9d}  {row.get('bbox', '-')}")
    if result["only_before"] or result["only_after"]:
        print(f"only before: {len(result['only_before'])}, only after: {len(result['only_after'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
