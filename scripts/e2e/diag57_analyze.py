#!/usr/bin/env python3
"""Summarize the job-57 stock/NSH injected-hold diagnostic evidence."""
import json
import re
import sys
from pathlib import Path

SEL = re.compile(r"\b(J57STOCK|J57OURS)\s*: sel (\d+)-(\d+)")
TOUCH = re.compile(r"\b(J57STOCK|J57OURS)\s*: touch (?:edit=[0-9a-f]+ )?action=(\d+) x=([0-9.-]+) y=([0-9.-]+)")
AM = re.compile(r"\b(J57STOCK|J57OURS)\s*: (?:edit=[0-9a-f]+ )?am (create|prepare|destroy) kind=(\S+).*items=\[([^\]]*)\]")

def walk(node):
    yield node
    for child in node.get("children") or []:
        yield from walk(child)

def nodes(tree):
    for root in tree.get("nodes") or []:
        yield from walk(root)

def load_tree(path):
    try:
        data = json.loads(Path(path).read_text())
        return data.get("tree", data)
    except Exception:
        return None

def tree_facts(path):
    tree = load_tree(path)
    if not tree:
        return {}
    out = {}
    for node in nodes(tree):
        if node.get("id") == "new-tp-tin" or (node.get("role") in ("textbox", "textfield") and str(node.get("name", "")).startswith("TIN, 9 digits")):
            for state in node.get("states") or []:
                m = re.fullmatch(r"selection: (\d+)-(\d+)", state)
                if m:
                    out["tree_selection"] = f"{m.group(1)}-{m.group(2)}"
        if node.get("id") == "system-text-menu":
            kind = next((s for s in node.get("states") or [] if s in ("selection", "insertion", "edit")), None)
            items = []
            for child in node.get("children") or []:
                items.append((child.get("id") or child.get("name") or "?")
                             + (":enabled" if child.get("enabled", True) else ":disabled"))
            out["tree_menu"] = {"kind": kind, "items": items}
    return out

def analyze_run(path):
    path = Path(path)
    m = re.fullmatch(r"(stock|ours)-(p1|p2)-run(\d+)", path.name)
    field, point, run = m.group(1), m.group(2), int(m.group(3))
    tag = "J57STOCK" if field == "stock" else "J57OURS"
    logcat = path / "logcat-j57.txt"
    lines = logcat.read_text(errors="replace").splitlines() if logcat.exists() else []
    own = [line for line in lines if re.search(r"\b" + tag + r"\s*:", line)]
    sels = [(int(a), int(b)) for _, a, b in SEL.findall("\n".join(own))]
    touches = [(int(a), float(x), float(y)) for t, a, x, y in TOUCH.findall("\n".join(own)) if t == tag]
    ams = []
    for line in own:
        match = AM.search(line)
        if match and match.group(1) == tag:
            ams.append({"event": match.group(2), "kind": match.group(3), "items": match.group(4), "line": line.strip()})
    creates = [i for i, a in enumerate(ams) if a["event"] == "create"]
    if creates:
        start = creates[-1]
        created = ams[start]
        prepared = next((a for a in ams[start + 1:] if a["event"] == "prepare" and a["kind"] == created["kind"]), None)
        menu = {"kind": created["kind"], "items": (prepared or created)["items"] or "none-logged"}
    else:
        menu = {"kind": "none", "items": ""}
    touch_positions = []
    for index, line in enumerate(lines):
        match = re.search(r"\b" + tag + r"\s*: touch (?:edit=[0-9a-f]+ )?action=(\d+)", line)
        if match:
            touch_positions.append((index, int(match.group(1))))
    hold_start = None
    hold_end = None
    for pos in range(len(touch_positions) - 1, -1, -1):
        index, action = touch_positions[pos]
        if action == 0 and any(a == 1 for _, a in touch_positions[pos + 1:]):
            previous = touch_positions[pos - 1][0] + 1 if pos else 0
            hold_start = previous
            hold_end = next(i for i, a in touch_positions[pos + 1:] if a == 1)
            break
    window = lines[hold_start:hold_end + 1] if hold_start is not None else []
    intercepts = [line.split("J57INTERCEPT:", 1)[-1].strip() for line in window if "J57INTERCEPT" in line]
    cancel = any(re.search(r"\b" + tag + r"\s*: cancel ", line) for line in window)
    hold_dir = next(iter(sorted(path.glob(f"*-hold-{field}-{point}"))), None)
    facts = tree_facts(hold_dir / "post-wait-tree.json") if hold_dir else {}
    report = {}
    try:
        report = json.loads((path / "report.json").read_text())
    except Exception:
        pass
    recipe_status = {}
    for recipe in report.get("recipes") or []:
        recipe_status[recipe.get("name")] = {"status": recipe.get("status"), "step": recipe.get("step"), "error": recipe.get("error")}
    return {
        "field": field,
        "point": point,
        "run": run,
        "selection_log": f"{sels[-1][0]}-{sels[-1][1]}" if sels else "none-logged",
        "selection_events": [f"{a}-{b}" for a, b in sels],
        "touch_count": len(touches),
        "cancel": cancel,
        "intercepts": intercepts,
        "touch_actions": [a for a, _, _ in touches],
        "menu": menu,
        "tree": facts,
        "recipes": recipe_status,
        "hold_dir": str(hold_dir) if hold_dir else None,
        "logcat": str(logcat),
        "screenshot": str(hold_dir / "post-wait.png") if hold_dir else None,
        "video": str(hold_dir / "recording.mp4") if hold_dir else None,
    }

def props(root):
    candidates = [Path(root) / "probe-run1" / "logcat-j57.txt"] + sorted(Path(root).glob("*-run1/logcat-j57.txt"))
    for path in candidates:
        if not path.exists():
            continue
        lines = path.read_text(errors="replace").splitlines()
        out = {}
        for tag in ("J57STOCK", "J57OURS"):
            props_line = next((line.split(":", 5)[-1].strip() for line in lines if re.search(tag + r"\s*: (?:edit=[0-9a-f]+ )?props ", line)), None)
            parents_line = next((line.split(":", 5)[-1].strip() for line in lines if re.search(tag + r"\s*: (?:edit=[0-9a-f]+ )?parents ", line)), None)
            if props_line or parents_line:
                out[tag] = {"props": props_line, "parents": parents_line}
        if len(out) == 2:
            return out
    return out

def main(root):
    root = Path(root)
    runs = []
    for path in sorted(root.iterdir()):
        if path.is_dir() and re.fullmatch(r"(stock|ours)-(p1|p2)-run\d+", path.name):
            try:
                runs.append(analyze_run(path))
            except Exception as e:
                runs.append({"dir": path.name, "error": str(e)})
    result = {"runs": runs, "props": props(root)}
    (root / "summary.json").write_text(json.dumps(result, indent=1) + "\n")
    print(json.dumps(result, indent=1))

if __name__ == "__main__":
    main(sys.argv[1])
