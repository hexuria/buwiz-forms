import struct
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

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

    def test_capture_requires_foreground_preview(self):
        user32 = Mock()
        user32.GetForegroundWindow.return_value = 9
        rect = SimpleNamespace(left=10, top=20, right=30, bottom=40)
        with self.assertRaisesRegex(windows_smoke.PaintCaptureError, "foregrounded"):
            windows_smoke.ensure_preview_capture_visible(user32, 8, rect)
        user32.SetForegroundWindow.assert_called_once_with(8)
        user32.WindowFromPoint.assert_not_called()

    def test_capture_rejects_occlusion_at_every_sample(self):
        rect = SimpleNamespace(left=10, top=20, right=30, bottom=40)
        for index in range(5):
            with self.subTest(index=index):
                user32 = Mock()
                user32.GetForegroundWindow.return_value = 8
                roots = [8] * 5
                roots[index] = 9
                user32.GetAncestor.side_effect = roots
                with self.assertRaisesRegex(windows_smoke.PaintCaptureError, "obscured"):
                    windows_smoke.ensure_preview_capture_visible(user32, 8, rect)

    def test_capture_accepts_only_owned_visible_area(self):
        user32 = Mock()
        user32.GetForegroundWindow.return_value = 8
        user32.GetAncestor.return_value = 8
        rect = SimpleNamespace(left=10, top=20, right=30, bottom=40)
        windows_smoke.ensure_preview_capture_visible(user32, 8, rect)
        points = [(call.args[0].x, call.args[0].y) for call in user32.WindowFromPoint.call_args_list]
        self.assertEqual(points, [(10, 20), (29, 20), (10, 39), (29, 39), (20, 30)])

    def test_wait_preview_retries_mailbox_drain_timeout(self):
        proc = Mock()
        proc.poll.return_value = None
        client = Mock()
        client.request.side_effect = [
            {"ok": False, "error": windows_smoke.MAILBOX_DRAIN_TIMEOUT},
            {
                "ok": True,
                "tree": {
                    "id": "root",
                    "nodes": [{"id": "print-preview-state", "value": "ready"}],
                },
            },
        ]
        with patch.object(windows_smoke.time, "sleep"):
            windows_smoke.wait_preview(client, proc)
        self.assertEqual(client.request.call_count, 2)

    def test_wait_preview_does_not_retry_other_agent_errors(self):
        proc = Mock()
        proc.poll.return_value = None
        client = Mock()
        client.request.return_value = {"ok": False, "error": "bad request"}
        with self.assertRaisesRegex(windows_smoke.SmokeError, "snapshot returned no tree"):
            windows_smoke.wait_preview(client, proc)

    def test_wait_preview_still_has_a_bounded_deadline(self):
        proc = Mock()
        proc.poll.return_value = None
        client = Mock()
        client.request.return_value = {
            "ok": False,
            "error": windows_smoke.MAILBOX_DRAIN_TIMEOUT,
        }
        with (
            patch.object(windows_smoke.time, "monotonic", side_effect=[0, 0, 31]),
            patch.object(windows_smoke.time, "sleep"),
        ):
            with self.assertRaisesRegex(windows_smoke.SmokeError, "within 30s"):
                windows_smoke.wait_preview(client, proc)

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
