#!/usr/bin/env python3
"""Dedicated tests for the official-reference preparation helper."""

from __future__ import annotations

import argparse
import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = (
    ROOT
    / "scripts/reference/prepare_official_reference.py"
)
SPEC = importlib.util.spec_from_file_location("conversion_prepare_reference", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
prepare_reference = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prepare_reference)


def png_header(width: int, height: int) -> bytes:
    return (
        b"\x89PNG\r\n\x1a\n"
        + b"\x00\x00\x00\rIHDR"
        + struct.pack(">II", width, height)
    )


class PrepareOfficialReferenceTests(unittest.TestCase):
    def test_identity_and_png_header_validation(self) -> None:
        self.assertEqual(
            prepare_reference.identity("2551q", "2018"),
            ("2551Q", "2018", "2551q", "2551Qv2018"),
        )
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "page.png"
            image.write_bytes(png_header(1224, 1872))
            self.assertEqual(prepare_reference.png_size(image), (1224, 1872))
            image.write_bytes(b"not png")
            with self.assertRaises(ValueError):
                prepare_reference.png_size(image)

    def test_install_reference_requires_explicit_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.png"
            destination = root / "destination.png"
            source.write_bytes(b"new")
            destination.write_bytes(b"old")

            with self.assertRaisesRegex(RuntimeError, "pass --replace"):
                prepare_reference.install_reference(source, destination, False)
            self.assertEqual(destination.read_bytes(), b"old")

            prepare_reference.install_reference(source, destination, True)
            self.assertEqual(destination.read_bytes(), b"new")
            self.assertFalse(source.exists())

    def test_check_only_render_is_deterministic_and_writes_no_reference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pdf = root / "official.pdf"
            pdf.write_bytes(b"%PDF-1.7\nsource")
            output_dir = root / "references"
            args = argparse.Namespace(
                repo=str(root),
                pdf=pdf,
                expected_sha256=prepare_reference.sha256_file(pdf),
                form_code="1601C",
                revision="2018",
                page_width_pt=612.0,
                page_height_pt=936.0,
                output_dir=str(output_dir),
                dpi=144,
                check_only=True,
                replace=False,
                source_url="https://example.invalid/official.pdf",
                manifest_out=None,
            )

            def fake_run(command: list[str], **_: object) -> mock.Mock:
                Path(command[-1]).with_suffix(".png").write_bytes(
                    png_header(1224, 1872)
                )
                return mock.Mock(returncode=0)

            with mock.patch.object(
                prepare_reference, "pdf_metadata", return_value=(2, 612.0, 936.0)
            ), mock.patch.object(
                prepare_reference, "require_tool", return_value="pdftoppm"
            ), mock.patch.object(
                prepare_reference.subprocess, "run", side_effect=fake_run
            ):
                record = prepare_reference.render_references(args)

            self.assertTrue(record["calibration_only"])
            self.assertFalse(record["runtime_background_allowed"])
            self.assertEqual(record["form"]["page_count"], 2)
            self.assertEqual(
                [page["reference_width_px"] for page in record["form"]["pages"]],
                [1224, 1224],
            )
            self.assertFalse(output_dir.exists())

    def test_hash_mismatch_stops_before_poppler(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pdf = root / "official.pdf"
            pdf.write_bytes(b"%PDF")
            args = argparse.Namespace(
                repo=str(root),
                pdf=pdf,
                expected_sha256="0" * 64,
                form_code="1601C",
                revision="2018",
                page_width_pt=None,
                page_height_pt=None,
                output_dir=None,
                dpi=144,
                check_only=True,
                replace=False,
                source_url="https://example.invalid/official.pdf",
                manifest_out=None,
            )
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                prepare_reference.render_references(args)


if __name__ == "__main__":
    unittest.main()
