import struct
import tempfile
import unittest
from pathlib import Path

from scripts import windows_smoke


def synthetic_pe(stack_reserve=0x800000):
    data = bytearray(0x200)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<H", data, 0x80 + 20, 0xF0)
    struct.pack_into("<H", data, 0x80 + 24, 0x20B)
    struct.pack_into("<Q", data, 0x80 + 24 + 72, stack_reserve)
    return bytes(data)


class WindowsSmokeTests(unittest.TestCase):
    def test_read_stack_reserve(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.exe"
            path.write_bytes(synthetic_pe(0x800000))
            self.assertEqual(windows_smoke.read_stack_reserve(path), 0x800000)

    def test_scan_logs(self):
        self.assertEqual(
            windows_smoke.scan_logs("panicked at foo", ""),
            "runtime marker: panicked at",
        )
        self.assertEqual(
            windows_smoke.scan_logs("", "thread overflowed its stack"),
            "runtime marker: overflowed its stack",
        )
        self.assertIsNone(windows_smoke.scan_logs("normal", "normal"))

    def test_classify_exit(self):
        self.assertIsNone(windows_smoke.classify_exit(0))
        self.assertIn("stack overflow", windows_smoke.classify_exit(0xC00000FD))
        self.assertIn("access violation", windows_smoke.classify_exit(-1073741819))
        self.assertIn("non-zero exit", windows_smoke.classify_exit(7))

    def test_dark_pixel_ratio_all_white(self):
        self.assertEqual(windows_smoke.dark_pixel_ratio(bytes([0xFF, 0xFF, 0xFF, 0x00]) * 8), 0.0)

    def test_dark_pixel_ratio_half_black(self):
        pixels = bytes([0x00, 0x00, 0x00, 0x00]) * 4 + bytes([0xFF, 0xFF, 0xFF, 0x00]) * 4
        self.assertEqual(windows_smoke.dark_pixel_ratio(pixels), 0.5)

    def test_smoke_env_strips_sftp(self):
        with tempfile.TemporaryDirectory() as directory:
            env = windows_smoke.smoke_env(
                {
                    "BIR_SFTP_LIVE": "1",
                    "BIR_SFTP_HOST": "should-not-survive",
                    "OTHER": "kept",
                },
                Path(directory),
                "127.0.0.1:1234",
                "token",
                "connect-id",
            )
        self.assertEqual(env["OTHER"], "kept")
        self.assertEqual(env["BIR_SFTP_DRY_RUN"], "1")
        self.assertNotIn("BIR_SFTP_LIVE", env)
        self.assertNotIn("BIR_SFTP_HOST", env)


if __name__ == "__main__":
    unittest.main()
