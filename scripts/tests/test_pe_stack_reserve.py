"""Tests for scripts/check_pe_stack_reserve.py."""

import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from check_pe_stack_reserve import DEFAULT_EXPECT, read_stack_reserve


def make_pe(reserve: int) -> bytes:
    data = bytearray(0x400)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80 : 0x84] = b"PE\0\0"
    struct.pack_into("<H", data, 0x80 + 24, 0x20B)  # PE32+
    struct.pack_into("<Q", data, 0x80 + 24 + 72, reserve)
    return bytes(data)


class ReadStackReserveTests(unittest.TestCase):
    def roundtrip(self, reserve: int) -> int:
        with tempfile.NamedTemporaryFile(suffix=".exe", delete=False) as f:
            f.write(make_pe(reserve))
            path = Path(f.name)
        try:
            return read_stack_reserve(path)
        finally:
            path.unlink()

    def test_reads_sizeofstackreserve(self):
        self.assertEqual(self.roundtrip(DEFAULT_EXPECT), DEFAULT_EXPECT)

    def test_reads_one_mib(self):
        self.assertEqual(self.roundtrip(1024 * 1024), 1024 * 1024)

    def test_rejects_non_pe(self):
        with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as f:
            f.write(b"\x7fELF" + b"\0" * 64)
            path = Path(f.name)
        try:
            with self.assertRaises(ValueError):
                read_stack_reserve(path)
        finally:
            path.unlink()


if __name__ == "__main__":
    unittest.main()
