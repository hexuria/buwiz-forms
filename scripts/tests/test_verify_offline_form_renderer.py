from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "verify_offline_form_renderer.py"
SPEC = importlib.util.spec_from_file_location("verify_offline_form_renderer", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
verifier = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verifier
SPEC.loader.exec_module(verifier)


CSP = (
    "default-src 'self'; connect-src 'none'; img-src 'self'; "
    "font-src 'self'; style-src 'self' 'unsafe-inline'; "
    "script-src 'self' ebirforms: http://ebirforms.localhost; "
    "object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; "
    "child-src 'none'; worker-src 'none'"
)


class VerifyOfflineFormRendererTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name) / "renderer"
        self.assets = self.root / "assets"
        self.assets.mkdir(parents=True)

    def write_valid_bundle(self) -> None:
        (self.root / "index.html").write_text(
            f"""<!doctype html>
<html><head>
<meta http-equiv="Content-Security-Policy" content="{CSP}">
<link rel="stylesheet" href="./assets/app.css">
<script defer src="./assets/app.js"></script>
</head><body><div id="root"></div></body></html>
""",
            encoding="utf-8",
        )
        (self.assets / "app.css").write_text(
            "@font-face{font-family:Owned;src:url('./owned.woff2')}\n",
            encoding="utf-8",
        )
        (self.assets / "app.js").write_text("document.body.dataset.ready='true';\n")
        (self.assets / "owned.woff2").write_bytes(b"owned-font")

    def assert_has_error(self, errors: list[str], expected: str) -> None:
        self.assertTrue(
            any(expected in error for error in errors),
            f"expected {expected!r} in errors: {errors!r}",
        )

    def test_valid_bundle_and_evidence_are_deterministic_and_non_promoting(self) -> None:
        self.write_valid_bundle()

        errors = verifier.verify_renderer(self.root)
        first = verifier.build_evidence(self.root, errors)
        second = verifier.build_evidence(self.root, list(reversed(errors)))

        self.assertEqual(errors, [])
        self.assertEqual(first, second)
        self.assertTrue(first["passed"])
        self.assertFalse(first["network_runtime_exercised"])
        self.assertFalse(first["packaged_runtime_promotion_satisfied"])
        self.assertEqual(first["scope"], "static_source_bundle_inspection")
        self.assertTrue(first["policy"]["complete_bundle_reachability_required"])
        self.assertFalse(first["policy"]["runtime_raster_assets_allowed"])
        self.assertEqual(
            [item["path"] for item in first["files"]],
            [
                "assets/app.css",
                "assets/app.js",
                "assets/owned.woff2",
                "index.html",
            ],
        )

        first_path = Path(self.temporary_directory.name) / "first.json"
        second_path = Path(self.temporary_directory.name) / "second.json"
        verifier.write_evidence(first_path, first, self.root)
        verifier.write_evidence(second_path, second, self.root)
        self.assertEqual(first_path.read_bytes(), second_path.read_bytes())
        self.assertEqual(json.loads(first_path.read_text()), first)

    def test_requires_one_root_index_and_exact_local_only_csp(self) -> None:
        self.write_valid_bundle()
        nested = self.root / "nested"
        nested.mkdir()
        (nested / "other.html").write_text("<html></html>", encoding="utf-8")
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "connect-src 'none'", "connect-src 'self'"
            ),
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "expected exactly one root index.html")
        self.assert_has_error(errors, "Content-Security-Policy requires connect-src 'none'")

    def test_requires_worker_and_child_sources_to_be_disabled(self) -> None:
        self.write_valid_bundle()
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8")
            .replace("child-src 'none'; ", "")
            .replace("worker-src 'none'", "worker-src 'self'"),
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(
            errors, "Content-Security-Policy is missing directives: child-src"
        )
        self.assert_has_error(errors, "Content-Security-Policy requires worker-src 'none'")

    def test_rejects_remote_file_protocol_relative_missing_and_traversal_references(self) -> None:
        self.write_valid_bundle()
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "</body>",
                """
<img src="https://example.test/remote.png">
<img src="//example.test/protocol-relative.png">
<a href="file:///tmp/private">file</a>
<img src="./assets/missing.png">
<img src="../outside.png">
</body>""",
            ),
            encoding="utf-8",
        )
        (self.assets / "app.css").write_text(
            "body{background:url('https://example.test/remote.png')}\n",
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "external or custom-scheme URL is not allowed")
        self.assert_has_error(errors, "protocol-relative URL is not allowed")
        self.assert_has_error(errors, "file URL is not allowed")
        self.assert_has_error(errors, "referenced file does not exist")
        self.assert_has_error(errors, "parent-directory traversal is not allowed")

    def test_reachable_asset_graph_covers_every_shipped_asset(self) -> None:
        self.write_valid_bundle()
        (self.assets / "app.js").write_text("import('./chunk.js');\n", encoding="utf-8")
        (self.assets / "chunk.js").write_text("export const value = 1;\n", encoding="utf-8")
        (self.assets / "app.css").write_text(
            "@import './nested.css';\n", encoding="utf-8"
        )
        (self.assets / "nested.css").write_text("body{color:#000}\n", encoding="utf-8")
        (self.assets / "stale.js").write_text("void 0;\n", encoding="utf-8")
        (self.assets / "stale.css").write_text("body{}\n", encoding="utf-8")
        (self.assets / "stale.woff2").write_bytes(b"unreachable-font")

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "unreferenced bundle asset: assets/stale.js")
        self.assert_has_error(errors, "unreferenced bundle asset: assets/stale.css")
        self.assert_has_error(errors, "unreferenced bundle asset: assets/stale.woff2")
        self.assertFalse(any("chunk.js" in error for error in errors))
        self.assertFalse(any("nested.css" in error for error in errors))

    def test_rejects_calibration_artwork_and_official_source_metadata(self) -> None:
        self.write_valid_bundle()
        (self.assets / "page1.svg").write_text("<svg></svg>\n", encoding="utf-8")
        (self.assets / "renamed-folio.svg").write_text(
            '<svg width="612" height="936" viewBox="0 0 612 936"></svg>\n',
            encoding="utf-8",
        )
        (self.assets / "2551q-2018-page-1.png").write_bytes(b"not-a-runtime-image")
        (self.assets / "official.pdf").write_bytes(b"not-a-runtime-pdf")
        (self.assets / "app.js").write_text(
            'const metadata={"official_source":'
            '"https://bir-cdn.bir.gov.ph/local/pdf/2551Q.pdf"};\n',
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "forbidden runtime artwork assets/page1.svg")
        self.assert_has_error(
            errors, "forbidden runtime artwork assets/renamed-folio.svg"
        )
        self.assert_has_error(
            errors, "forbidden runtime artwork assets/2551q-2018-page-1.png"
        )
        self.assert_has_error(errors, "forbidden runtime artwork assets/official.pdf")
        self.assert_has_error(errors, "forbidden official-source marker in assets/app.js")

    def test_rejects_raster_suffixes_even_when_payload_is_malformed(self) -> None:
        self.write_valid_bundle()
        for name in ("page-photo.jpg", "page-photo.webp", "page-photo.avif"):
            (self.assets / name).write_bytes(b"renamed-full-page-raster")

        errors = verifier.verify_renderer(self.root)

        for name in ("page-photo.jpg", "page-photo.webp", "page-photo.avif"):
            self.assert_has_error(
                errors,
                f"forbidden runtime artwork assets/{name}: unauthorized or malformed",
            )

    def test_magic_sniffs_rasters_after_rename(self) -> None:
        self.write_valid_bundle()
        disguised = {
            "page-png.bin": b"\x89PNG\r\n\x1a\n" + b"payload",
            "page-jpeg.bin": b"\xff\xd8\xff" + b"payload",
            "page-webp.bin": b"RIFF\x04\x00\x00\x00WEBPpayload",
            "page-avif.bin": b"\x00\x00\x00\x18ftypavifpayload",
            "page-gif.bin": b"GIF89apayload",
        }
        for name, payload in disguised.items():
            (self.assets / name).write_bytes(payload)

        errors = verifier.verify_renderer(self.root)

        for name, raster_format in (
            ("page-png.bin", "PNG"),
            ("page-jpeg.bin", "JPEG"),
            ("page-webp.bin", "WebP"),
            ("page-avif.bin", "AVIF"),
            ("page-gif.bin", "GIF"),
        ):
            self.assert_has_error(
                errors,
                f"forbidden runtime artwork assets/{name}: unauthorized {raster_format}",
            )

    def test_rejects_embedded_data_image_backgrounds_in_css_html_and_js(self) -> None:
        self.write_valid_bundle()
        (self.assets / "app.css").write_text(
            "body{background-image:url(data:image/webp;base64,AAAA)}\n",
            encoding="utf-8",
        )
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "<body>",
                '<body style="background:url(data:image/jpeg;base64,AAAA)">',
            ),
            encoding="utf-8",
        )
        (self.assets / "app.js").write_text(
            "const background = 'data:image/avif;base64,AAAA';\n",
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        for relative in ("index.html", "assets/app.css", "assets/app.js"):
            self.assert_has_error(
                errors,
                f"forbidden embedded data-image runtime artwork in {relative}",
            )

    def test_rejects_html_entity_encoded_data_image_reference(self) -> None:
        self.write_valid_bundle()
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "<body>",
                '<body><img src="data&#58;image/png;base64,AAAA" alt="hidden page">',
            ),
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "embedded data-image URL is not allowed")

    def test_rejects_development_sample_data_in_shipped_bundle(self) -> None:
        self.write_valid_bundle()
        (self.assets / "app.js").write_text(
            'const taxpayer="Renderer Preview Corporation";\n',
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "development sample marker in shipped bundle")

    def test_rejects_evidence_written_inside_verified_bundle(self) -> None:
        self.write_valid_bundle()
        evidence = verifier.build_evidence(self.root, [])

        with self.assertRaisesRegex(
            ValueError, "evidence output must remain outside the renderer bundle"
        ):
            verifier.write_evidence(self.root / "evidence.json", evidence, self.root)

    def test_rejects_symlinks_and_hashes_the_link_target_in_evidence(self) -> None:
        self.write_valid_bundle()
        link = self.assets / "linked-font.woff2"
        link.symlink_to("owned.woff2")

        errors = verifier.verify_renderer(self.root)
        evidence = verifier.build_evidence(self.root, errors)

        self.assert_has_error(errors, "renderer bundle must not contain symlinks")
        item = next(item for item in evidence["files"] if item["path"].endswith("linked-font.woff2"))
        self.assertEqual(item["type"], "symlink")
        self.assertEqual(
            item["sha256"], hashlib.sha256(b"owned.woff2").hexdigest()
        )

    def test_pinned_artwork_hash_is_rejected_even_after_rename(self) -> None:
        self.write_valid_bundle()
        renamed = self.assets / "innocent.bin"
        renamed.write_bytes(b"pinned-reference-content")
        digest = hashlib.sha256(renamed.read_bytes()).hexdigest()
        original = verifier.OFFICIAL_ARTWORK_SHA256
        verifier.OFFICIAL_ARTWORK_SHA256 = original | {digest}
        self.addCleanup(
            lambda: setattr(verifier, "OFFICIAL_ARTWORK_SHA256", original)
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "matches a pinned official/reference page hash")


if __name__ == "__main__":
    unittest.main()
