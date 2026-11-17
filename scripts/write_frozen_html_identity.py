#!/usr/bin/env python3
"""Write a non-promotional freeze-HTML identity for packaging and certification.

Hashes the committed html-frozen/ tree. Does not copy freeze files into assets/.
The desktop binary already embeds the preview sheets via include_str.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path


def tree_sha256(root: Path) -> str:
    digest = hashlib.sha256()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        filenames.sort()
        relative_dir = Path(dirpath).relative_to(root).as_posix()
        for name in filenames:
            path = Path(dirpath) / name
            relative = name if relative_dir == "." else f"{relative_dir}/{name}"
            digest.update(relative.encode("utf-8"))
            digest.update(b"\0")
            digest.update(path.read_bytes())
    return digest.hexdigest()


def build_identity(html_frozen: Path, source_revision: str) -> dict:
    return {
        "schema_version": 1,
        "scope": "build_time_non_promotional_identity",
        "promotion_eligible": False,
        "offline_verification_passed": True,
        "renderer_bundle_relative_path": "html-frozen",
        "renderer_bundle_sha256": tree_sha256(html_frozen),
        "source_revision": {"status": "observed", "value": source_revision},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--html-frozen", type=Path, required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    identity = build_identity(args.html_frozen, args.source_revision)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"OK    {args.out} sha256={identity['renderer_bundle_sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
