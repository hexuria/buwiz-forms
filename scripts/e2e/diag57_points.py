#!/usr/bin/env python3
"""Compute the job-57 diagnostic x/y recipe points from a probe run's J57 logcat and post-type tree."""
import json
import re
import sys
from pathlib import Path

LAYOUT = re.compile(
    r"\b(J57STOCK|J57OURS)\s*: (?:edit=[0-9a-f]+ )?layout \S+ rect=([0-9.-]+),([0-9.-]+),([0-9.-]+),([0-9.-]+)"
    r" padding=([0-9.-]+),([0-9.-]+),([0-9.-]+),([0-9.-]+)"
    r" compoundPadding=([0-9.-]+),([0-9.-]+),([0-9.-]+),([0-9.-]+)"
    r" scroll=([0-9.-]+),([0-9.-]+)"
    r" glyph0=([0-9.eE+-]+|\S+) glyph4=([0-9.eE+-]+|\S+) glyph9=([0-9.eE+-]+|\S+)"
    r" textLength=(\d+)"
)

def walk(node):
    yield node
    for child in node.get("children") or []:
        yield from walk(child)

def tree_nodes(path):
    data = json.loads(Path(path).read_text())
    tree = data.get("tree", data)
    for root in tree.get("nodes") or []:
        yield from walk(root)

def number(text):
    try:
        return float(text)
    except ValueError:
        return None

def metrics(logcat):
    out = {}
    for line in Path(logcat).read_text(errors="replace").splitlines():
        m = LAYOUT.search(line)
        if not m or int(m.group(19)) != 9:
            continue
        vals = [number(x) for x in m.groups()[1:19]]
        if any(v is None for v in vals):
            continue
        out[m.group(1)] = {
            "rect": vals[0:4],
            "padding": vals[4:8],
            "compoundPadding": vals[8:12],
            "scroll": vals[12:14],
            "glyph0": vals[14],
            "glyph4": vals[15],
            "glyph9": vals[16],
            "line": line.strip(),
        }
    return out

def main(probe_dir, repo_root):
    probe = Path(probe_dir)
    root = Path(repo_root)
    logcat = probe / "logcat-j57.txt"
    tree_path = probe / "02-type" / "post-tree.json"
    got = metrics(logcat)
    missing = [tag for tag in ("J57STOCK", "J57OURS") if tag not in got]
    if missing:
        raise SystemExit(f"missing layout metrics with textLength=9 for {', '.join(missing)} in {logcat}")
    tin = None
    for node in tree_nodes(tree_path):
        if node.get("id") == "new-tp-tin":
            tin = node
            break
    if not tin or not tin.get("bounds"):
        raise SystemExit(f"new-tp-tin not found in {tree_path}")
    bounds = tin["bounds"]
    ours = got["J57OURS"]
    px_width = ours["rect"][2] - ours["rect"][0]
    density = px_width / float(bounds["w"])
    result = {"density": density, "tin_bounds_points": bounds, "fields": {}, "part2_target_centre_points": {"x": 185.1, "y": 368.8}}
    for tag, field in (("stock", "J57STOCK"), ("ours", "J57OURS")):
        m = got[field]
        left, top, right, bottom = m["rect"]
        pad_left = m["compoundPadding"][0]
        scroll_x = m["scroll"][0]
        centre_y_px = (top + bottom) / 2
        glyph4_px = left + pad_left + m["glyph4"] - scroll_x
        glyph9_px = left + pad_left + m["glyph9"] - scroll_x
        p2_px = glyph9_px + 45.0
        if p2_px >= right:
            raise SystemExit(f"{field}: glyph9+45 is outside the field ({p2_px} >= {right})")
        item = {
            "metrics": m,
            "p1_px": {"x": glyph4_px, "y": centre_y_px},
            "p2_px": {"x": p2_px, "y": centre_y_px},
            "p1_points": {"x": round(glyph4_px / density, 2), "y": round(centre_y_px / density, 2)},
            "p2_points": {"x": round(p2_px / density, 2), "y": round(centre_y_px / density, 2)},
            "p2_right_of_glyph9_px": 45.0,
        }
        result["fields"][tag] = item
        for point in ("p1", "p2"):
            xy = item[f"{point}_points"]
            recipe = {
                "v": 1,
                "name": f"diag-hold-{tag}-{point}",
                "steps": [{
                    "id": "h1", "op": "long_press", "x": xy["x"], "y": xy["y"],
                    "duration_ms": 1100, "screenshot": True,
                }],
            }
            path = root / "agent-recipes" / "e2e" / f"diag-hold-{tag}-{point}.json"
            path.write_text(json.dumps(recipe, indent=1) + "\n")
    centre = result["part2_target_centre_points"]
    result["fields"]["ours"]["p2_relative_to_part2_centre_points"] = {
        "dx": round(result["fields"]["ours"]["p2_points"]["x"] - centre["x"], 2),
        "dy": round(result["fields"]["ours"]["p2_points"]["y"] - centre["y"], 2),
    }
    (probe / "points.json").write_text(json.dumps(result, indent=1) + "\n")
    print(json.dumps(result, indent=1))

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
