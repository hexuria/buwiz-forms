#!/usr/bin/env python3
"""Copy an app cargo binary to the packaged executable name."""

from __future__ import annotations

import shutil
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) not in (2, 3):
        raise SystemExit("usage: alias_app_bin.py TARGET_DIR [SRC_NAME]")
    target = Path(sys.argv[1])
    src_base = sys.argv[2] if len(sys.argv) == 3 else "bir-shell"
    copied = False
    for src_name, dst_name in ((src_base, "bir"), (f"{src_base}.exe", "bir.exe")):
        src = target / src_name
        if src.is_file():
            shutil.copy2(src, target / dst_name)
            copied = True
    if not copied:
        raise SystemExit(f"no {src_base} binary in {target}")


if __name__ == "__main__":
    main()
