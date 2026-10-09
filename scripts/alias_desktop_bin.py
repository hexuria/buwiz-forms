#!/usr/bin/env python3
"""Copy the desktop cargo bin onto the shipped executable name.

The GUI cargo target is ``bir-desktop`` so it does not share a filename with
the ``bir`` CLI. App bundles, deb packages, and installers still ship ``bir``
or ``bir.exe``.
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: alias_desktop_bin.py TARGET_DIR")
    target = Path(sys.argv[1])
    copied = False
    for src_name, dst_name in (("bir-desktop", "bir"), ("bir-desktop.exe", "bir.exe")):
        src = target / src_name
        if src.is_file():
            shutil.copy2(src, target / dst_name)
            copied = True
    if not copied:
        raise SystemExit(f"no bir-desktop binary in {target}")


if __name__ == "__main__":
    main()
