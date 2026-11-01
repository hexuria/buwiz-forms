#!/usr/bin/env python3
"""Check a PE binary's SizeOfStackReserve.

Issue #96 regression gate: the shipped bir-shell.exe must carry the linker
policy emitted by crates/bir-shell/build.rs. Reverting that policy must fail
this check in CI; raising it above the measured minimum without data must fail
it too.

Usage: check_pe_stack_reserve.py EXE [--expect BYTES]

Exit 0 when the reserve equals --expect (default 8 MiB). Reports actual bytes
on failure. Header layout: e_lfanew at 0x3c, then PE32+ optional header where
SizeOfStackReserve is a QWORD at offset 0x48 from the optional header start
(0x18 signature + 0x14 COFF header => 0x3c + 4 + 20 + 72).
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

DEFAULT_EXPECT = 8 * 1024 * 1024


def read_stack_reserve(path: Path) -> int:
    data = path.read_bytes()
    if len(data) < 0x40 or data[:2] != b"MZ":
        raise ValueError(f"{path}: not a PE file")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe : pe + 4] != b"PE\0\0":
        raise ValueError(f"{path}: missing PE signature")
    magic = struct.unpack_from("<H", data, pe + 24)[0]
    if magic != 0x20B:
        raise ValueError(f"{path}: not PE32+ (magic {magic:#x})")
    return struct.unpack_from("<Q", data, pe + 24 + 72)[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("exe", type=Path)
    parser.add_argument("--expect", type=lambda s: int(s, 0), default=DEFAULT_EXPECT,
                        help="expected SizeOfStackReserve in bytes (default 8388608)")
    args = parser.parse_args()
    actual = read_stack_reserve(args.exe)
    if actual != args.expect:
        print(
            f"{args.exe}: SizeOfStackReserve {actual} != expected {args.expect} "
            f"({actual / 1048576:.2f} MiB vs {args.expect / 1048576:.2f} MiB). "
            "Windows linker policy regression (#96).",
            file=sys.stderr,
        )
        return 1
    print(f"{args.exe}: SizeOfStackReserve {actual} ({actual / 1048576:.0f} MiB) OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
