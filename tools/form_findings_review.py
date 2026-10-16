#!/usr/bin/env python3
"""Record the hand review of one form's findings (docs/form-findings/<CODE>/findings.json).

    python3 tools/form_findings_review.py CODE review.json

review.json: {"notes": "...", "drop": {"R7.0": "reason"},
              "add": [{"section": "R3", "what": "...", "where": "...", "fix": "...",
                       "evidence": "shots/p0-00.jpg", "shot": "p0-00"}]}
Sets `reviewed`, `review_notes`, `dropped` and `manual` (replacing earlier
manual entries), then re-renders page.html.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SECTIONS = {"R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "preview", "validation"}


def main(code, review_path):
    path = ROOT / "docs" / "form-findings" / code / "findings.json"
    data = json.loads(path.read_text())
    review = json.loads(Path(review_path).read_text())
    ids = {f["id"] for s in data["rules"].values() for f in s["findings"]}
    for i in review.get("drop", {}):
        if i not in ids:
            sys.exit(f"{code}: no finding {i} to drop")
    for m in review.get("add", []):
        if m.get("section") not in SECTIONS or not m.get("what") or not m.get("evidence"):
            sys.exit(f"{code}: manual finding needs section, what and evidence: {m}")
    data["dropped"] = review.get("drop", {})
    data["manual"] = review.get("add", [])
    data["review_notes"] = review.get("notes", "").strip()
    data["reviewed"] = bool(data["review_notes"])
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n")
    subprocess.run([sys.executable, str(ROOT / "tools" / "form_findings_page.py"), code], check=True)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
