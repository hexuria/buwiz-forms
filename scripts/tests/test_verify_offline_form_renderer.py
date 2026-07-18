from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "verify_offline_form_renderer.py"
SPEC = importlib.util.spec_from_file_location("verify_offline_form_renderer", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
verifier = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verifier
SPEC.loader.exec_module(verifier)


CSP = (
    "default-src 'self'; connect-src 'none'; img-src 'self' data:; "
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

    def valid_seal_asset(self, source: Path, workspace: Path) -> dict:
        payload = b"\x89PNG\r\n\x1a\nexact-native-seal"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(payload)
        return {
            "asset": "government_seal",
            "derived_png_sha256": hashlib.sha256(payload).hexdigest(),
            "embedded_as": "lossless_official_pdf_xobject_png",
            "embedded_in": source.relative_to(workspace).as_posix(),
            "source_page": 1,
            "source_pdf_object_id": [41, 0],
            "source_pixel_dimensions": [95, 83],
            "source_bbox_top_left_points": [244.8, 21.84, 28.8, 25.2],
            "treatment": "lossless extraction of the exact official PDF image XObject without crop, resampling, recoloring, or substitution",
        }

    def valid_pdf417_asset(
        self,
        source: Path,
        workspace: Path,
        *,
        page: int = 1,
        payload: str = "TEST 01/18ENCS P1",
    ) -> dict:
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(f'export const payload = "{payload}";\n', encoding="utf-8")
        return {
            "asset": f"static_form_pdf417_page_{page}",
            "decoded_payload": payload,
            "embedded_as": "reviewed_inline_svg_module_matrix_with_live_caption",
            "embedded_in": source.relative_to(workspace).as_posix(),
            "source_page": page,
            "source_pdf_object_id": [42 + page, 0],
            "source_pixel_dimensions": [240, 63],
            "source_png_sha256": "b" * 64,
            "symbology": "PDF417",
            "logical_dimensions": [120, 7],
            "logical_matrix_sha256": "c" * 64,
            "caption_text": payload,
            "caption_render_font": "eBIRForms Arimo",
            "caption_font_size_points": 8.04,
            "caption_bbox_top_left_points": [500.0, 90.0, 90.0, 9.0],
            "encoder_proof": {"module_differences": 0, "rows": 7},
            "decoder_evidence": [
                {
                    "decoder": "test decoder",
                    "payload": payload,
                    "symbology": "PDF417",
                }
            ],
        }

    def write_artwork_manifest(self, path: Path, forms: list[dict]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"forms": forms}), encoding="utf-8")

    def artwork_form(
        self,
        *,
        code: str,
        revision: str,
        assets: list[dict],
        page_count: int = 1,
        official_source_sha256: str = "d" * 64,
    ) -> dict:
        return {
            "code": code,
            "revision": revision,
            "page_count": page_count,
            "official_source_sha256": official_source_sha256,
            "pages": [
                {
                    "page": page,
                    "reference_width_px": 1_224,
                    "reference_height_px": 1_872,
                    "reference_png_sha256": f"{page:x}" * 64,
                }
                for page in range(1, page_count + 1)
            ],
            "runtime_discrete_assets": assets,
        }

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
        self.assertTrue(first["policy"]["embedded_data_images_allowed"])
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

    def test_build_identity_is_deterministic_external_and_non_promotional(self) -> None:
        self.write_valid_bundle()
        evidence = verifier.build_evidence(self.root, verifier.verify_renderer(self.root))
        source_revision = {"status": "observed", "value": "a" * 40}

        first = verifier.build_renderer_identity(evidence, source_revision)
        second = verifier.build_renderer_identity(evidence, dict(source_revision))

        self.assertEqual(first, second)
        self.assertFalse(first["promotion_eligible"])
        self.assertTrue(first["offline_verification_passed"])
        self.assertEqual(first["renderer_bundle_sha256"], evidence["bundle_sha256"])
        self.assertEqual(first["renderer_bundle_relative_path"], "form-renderer")
        self.assertEqual(first["source_revision"], source_revision)

        first_path = Path(self.temporary_directory.name) / "first-identity.json"
        second_path = Path(self.temporary_directory.name) / "second-identity.json"
        verifier.write_build_identity(first_path, first, self.root)
        verifier.write_build_identity(second_path, second, self.root)
        self.assertEqual(first_path.read_bytes(), second_path.read_bytes())
        with self.assertRaisesRegex(ValueError, "outside the renderer bundle"):
            verifier.write_build_identity(self.root / "identity.json", first, self.root)

    def test_build_identity_keeps_dirty_source_explicitly_unavailable(self) -> None:
        self.write_valid_bundle()
        evidence = verifier.build_evidence(self.root, verifier.verify_renderer(self.root))
        unavailable = {
            "status": "unavailable",
            "reason": "curated renderer source is dirty",
        }

        identity = verifier.build_renderer_identity(evidence, unavailable)

        self.assertEqual(identity["source_revision"], unavailable)
        with self.assertRaisesRegex(ValueError, "canonical Git commit"):
            verifier.build_renderer_identity(
                evidence,
                {"status": "observed", "value": "not-a-revision"},
            )

    def test_source_revision_is_resolved_by_clean_migration_audit(self) -> None:
        completed = mock.Mock(
            returncode=0,
            stdout=f"{'b' * 40}\n",
        )
        with mock.patch.object(verifier.subprocess, "run", return_value=completed) as run:
            availability = verifier.resolve_curated_source_revision(
                Path(self.temporary_directory.name)
            )

        self.assertEqual(
            availability,
            {"status": "observed", "value": "b" * 40},
        )
        arguments = run.call_args.args[0]
        self.assertIn("--require-clean-source", arguments)
        self.assertIn("--print-source-revision", arguments)

    def test_source_revision_never_reuses_last_commit_for_dirty_source(self) -> None:
        completed = mock.Mock(returncode=1, stdout="")
        with mock.patch.object(verifier.subprocess, "run", return_value=completed):
            availability = verifier.resolve_curated_source_revision(
                Path(self.temporary_directory.name)
            )

        self.assertEqual(availability["status"], "unavailable")
        self.assertIn("dirty", availability["reason"])

    def test_package_identity_requires_clean_source_and_removes_stale_output(self) -> None:
        self.write_valid_bundle()
        identity_path = Path(self.temporary_directory.name) / "build-identity.json"
        unavailable = {
            "status": "unavailable",
            "reason": "curated renderer source is dirty",
        }
        standard_arguments = [
            str(SCRIPT_PATH),
            str(self.root),
            "--build-identity-out",
            str(identity_path),
        ]
        with (
            mock.patch.object(verifier.sys, "argv", standard_arguments),
            mock.patch.object(
                verifier,
                "resolve_curated_source_revision",
                return_value=unavailable,
            ),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(verifier.main(), 0)
        self.assertEqual(
            json.loads(identity_path.read_text(encoding="utf-8"))["source_revision"],
            unavailable,
        )

        strict_arguments = [*standard_arguments, "--require-source-revision"]
        with (
            mock.patch.object(verifier.sys, "argv", strict_arguments),
            mock.patch.object(
                verifier,
                "resolve_curated_source_revision",
                return_value=unavailable,
            ),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(verifier.main(), 1)
        self.assertFalse(identity_path.exists(), "strict failure must remove stale identity")

    def test_allows_reachable_reviewed_font_license_and_provenance_documents(self) -> None:
        self.write_valid_bundle()
        arimo_notices = self.root / "third-party" / "arimo"
        arimo_notices.mkdir(parents=True)
        (arimo_notices / "LICENSE.txt").write_text("Apache-2.0 license\n", encoding="utf-8")
        (arimo_notices / "PROVENANCE.json").write_text("{}\n", encoding="utf-8")
        condensed_notices = self.root / "third-party" / "roboto-condensed"
        condensed_notices.mkdir(parents=True)
        (condensed_notices / "LICENSE.txt").write_text("OFL-1.1 license\n", encoding="utf-8")
        (condensed_notices / "PROVENANCE.json").write_text("{}\n", encoding="utf-8")
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                "</head>",
                '<link rel="license" href="./third-party/arimo/LICENSE.txt">\n'
                '<link rel="alternate" href="./third-party/arimo/PROVENANCE.json">\n'
                '<link rel="license" href="./third-party/roboto-condensed/LICENSE.txt">\n'
                '<link rel="alternate" href="./third-party/roboto-condensed/PROVENANCE.json">\n'
                "</head>",
            ),
            encoding="utf-8",
        )

        self.assertEqual(verifier.verify_renderer(self.root), [])

    def test_rejects_non_woff2_font_assets(self) -> None:
        self.write_valid_bundle()
        css = self.assets / "app.css"
        css.write_text(
            "@font-face{font-family:Owned;src:url('./owned.woff')}\n",
            encoding="utf-8",
        )
        (self.assets / "owned.woff2").unlink()
        (self.assets / "owned.woff").write_bytes(b"legacy-font")

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(
            errors,
            "renderer bundle contains an unauthorized asset type: assets/owned.woff",
        )

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

    def test_rejects_import_meta_asset_url_in_reachable_classic_bundle(self) -> None:
        self.write_valid_bundle()
        (self.assets / "app.js").write_text(
            'const barcode = new URL("./owned.woff2", import.meta.url);\n',
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(
            errors,
            "reachable classic script assets/app.js contains import.meta",
        )
        self.assert_has_error(
            errors,
            "reachable classic script assets/app.js contains new URL(..., import.meta.url)",
        )

    def test_module_script_keeps_explicit_module_semantics(self) -> None:
        self.write_valid_bundle()
        index = self.root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                '<script defer src="./assets/app.js"></script>',
                '<script type="module" src="./assets/app.js"></script>',
            ),
            encoding="utf-8",
        )
        (self.assets / "app.js").write_text(
            'const barcode = new URL("./owned.woff2", import.meta.url);\n',
            encoding="utf-8",
        )

        self.assertEqual(verifier.verify_renderer(self.root), [])

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

    def test_accepts_only_a_reviewed_embedded_discrete_image(self) -> None:
        self.write_valid_bundle()
        payload = b"reviewed-discrete-artwork"
        digest = hashlib.sha256(payload).hexdigest()
        original = verifier.AUTHORIZED_EMBEDDED_IMAGE_SHA256
        verifier.AUTHORIZED_EMBEDDED_IMAGE_SHA256 = original | {digest}
        self.addCleanup(
            lambda: setattr(verifier, "AUTHORIZED_EMBEDDED_IMAGE_SHA256", original)
        )
        import base64

        encoded = base64.b64encode(payload).decode("ascii")
        (self.assets / "app.js").write_text(
            f'const artwork="data:image/png;base64,{encoded}";\n',
            encoding="utf-8",
        )

        self.assertEqual(verifier.verify_renderer(self.root), [])

    def test_reviewed_image_cannot_mask_an_unreviewed_data_uri(self) -> None:
        self.write_valid_bundle()
        payload = b"reviewed-discrete-artwork"
        digest = hashlib.sha256(payload).hexdigest()
        original = verifier.AUTHORIZED_EMBEDDED_IMAGE_SHA256
        verifier.AUTHORIZED_EMBEDDED_IMAGE_SHA256 = original | {digest}
        self.addCleanup(
            lambda: setattr(verifier, "AUTHORIZED_EMBEDDED_IMAGE_SHA256", original)
        )
        import base64

        encoded = base64.b64encode(payload).decode("ascii")
        (self.assets / "app.js").write_text(
            f'const artwork="data:image/png;base64,{encoded}";\n'
            'const hidden="data:image/svg+xml,%3Csvg%3E";\n',
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(
            errors,
            "only reviewed base64 image payloads are allowed",
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

    def test_commons_seal_candidate_is_rejected_even_after_rename(self) -> None:
        self.write_valid_bundle()
        artwork_root = (
            SCRIPT_PATH.resolve().parents[1]
            / "packages/form-renderer/references/artwork"
        )
        for name in (
            "bir-seal-commons-original.svg",
            "bir-seal-commons-binary-exact-white.svg",
            "bir-seal-2551q-2018-candidate.svg",
        ):
            with self.subTest(name=name):
                renamed = self.assets / "innocent.bin"
                renamed.write_bytes((artwork_root / name).read_bytes())

                errors = verifier.verify_renderer(self.root)

                self.assert_has_error(
                    errors,
                    "matches an unapproved calibration-only artwork candidate",
                )

    def test_commons_seal_source_marker_is_rejected_from_scripts(self) -> None:
        self.write_valid_bundle()
        (self.assets / "app.js").write_text(
            "const artwork = 'bir-seal-commons-binary-exact-white.svg';\n",
            encoding="utf-8",
        )

        errors = verifier.verify_renderer(self.root)

        self.assert_has_error(errors, "forbidden official-source marker")

    def test_runtime_artwork_authorization_is_derived_from_valid_manifest_entries(self) -> None:
        workspace = Path(self.temporary_directory.name) / "artwork-workspace"
        seal = self.valid_seal_asset(
            workspace / "packages/form-renderer/src/forms/assets/reviewed.png",
            workspace,
        )
        symbol = self.valid_pdf417_asset(
            workspace / "packages/form-renderer/src/forms/officialTestAssets.ts",
            workspace,
        )
        manifest_path = workspace / "packages/form-renderer/references/manifest.json"
        self.write_artwork_manifest(
            manifest_path,
            [
                self.artwork_form(
                    code="TEST",
                    revision="2018",
                    assets=[seal, symbol],
                )
            ],
        )

        authorization, errors = verifier._load_runtime_artwork_authorization(
            manifest_path, workspace
        )

        self.assertEqual(errors, ())
        self.assertEqual(authorization.raster_sha256, {seal["derived_png_sha256"]})
        self.assertEqual(len(authorization.forms), 1)
        self.assertEqual(authorization.forms[0].pdf417_pages, (1,))
        self.assertFalse(authorization.forms[0].audited_no_symbol)
        self.assertNotIn(symbol["source_png_sha256"], authorization.raster_sha256)

    def test_0605_is_the_only_explicit_audited_no_symbol_form(self) -> None:
        workspace = Path(self.temporary_directory.name) / "no-symbol-workspace"
        seal = self.valid_seal_asset(
            workspace / "packages/form-renderer/src/forms/assets/0605-seal.png",
            workspace,
        )
        manifest_path = workspace / "packages/form-renderer/references/manifest.json"
        self.write_artwork_manifest(
            manifest_path,
            [
                self.artwork_form(
                    code="0605",
                    revision="1999",
                    assets=[seal],
                    page_count=2,
                    official_source_sha256=verifier.AUDITED_NO_SYMBOL_FORMS[
                        ("0605", "1999")
                    ],
                )
            ],
        )

        authorization, errors = verifier._load_runtime_artwork_authorization(
            manifest_path, workspace
        )

        self.assertEqual(errors, ())
        self.assertEqual(authorization.forms[0].pdf417_pages, ())
        self.assertTrue(authorization.forms[0].audited_no_symbol)

    def test_identical_native_seal_bytes_may_be_shared_by_reviewed_forms(self) -> None:
        workspace = Path(self.temporary_directory.name) / "shared-seal-workspace"
        first_seal = self.valid_seal_asset(
            workspace / "packages/form-renderer/src/forms/assets/first-seal.png",
            workspace,
        )
        second_seal = self.valid_seal_asset(
            workspace / "packages/form-renderer/src/forms/assets/second-seal.png",
            workspace,
        )
        first_symbol = self.valid_pdf417_asset(
            workspace / "packages/form-renderer/src/forms/officialFirstAssets.ts",
            workspace,
            payload="FIRST 01/18ENCS P1",
        )
        second_symbol = self.valid_pdf417_asset(
            workspace / "packages/form-renderer/src/forms/officialSecondAssets.ts",
            workspace,
            payload="SECOND 01/18ENCS P1",
        )
        manifest_path = workspace / "packages/form-renderer/references/manifest.json"
        self.write_artwork_manifest(
            manifest_path,
            [
                self.artwork_form(
                    code="FIRST",
                    revision="2018",
                    assets=[first_seal, first_symbol],
                ),
                self.artwork_form(
                    code="SECOND",
                    revision="2018",
                    assets=[second_seal, second_symbol],
                ),
            ],
        )

        authorization, errors = verifier._load_runtime_artwork_authorization(
            manifest_path, workspace
        )

        self.assertEqual(errors, ())
        self.assertEqual(len(authorization.raster_sha256), 1)
        self.assertEqual(len(authorization.forms), 2)

    def test_checked_in_manifest_models_native_seals_vectors_and_0605_absence(self) -> None:
        self.assertEqual(verifier.RUNTIME_ARTWORK_MANIFEST_ERRORS, ())
        self.assertEqual(len(verifier.RUNTIME_ARTWORK_AUTHORIZATION.forms), 10)
        no_symbol = [
            form
            for form in verifier.RUNTIME_ARTWORK_AUTHORIZATION.forms
            if form.audited_no_symbol
        ]
        self.assertEqual(
            [(form.code, form.revision, form.pdf417_pages) for form in no_symbol],
            [("0605", "1999", ())],
        )
        self.assertEqual(len(verifier.AUTHORIZED_RUNTIME_RASTER_SHA256), 8)

    def test_runtime_artwork_manifest_fails_closed_on_invalid_entries(self) -> None:
        workspace = Path(self.temporary_directory.name) / "invalid-artwork-workspace"
        source = workspace / "packages/form-renderer/src/forms/assets/reviewed.png"
        base_seal = self.valid_seal_asset(source, workspace)
        symbol_source = workspace / "packages/form-renderer/src/forms/officialTestAssets.ts"
        base_symbol = self.valid_pdf417_asset(symbol_source, workspace)
        manifest_path = workspace / "packages/form-renderer/references/manifest.json"

        invalid_cases = {
            "missing seal hash": [
                {
                    key: value
                    for key, value in base_seal.items()
                    if key != "derived_png_sha256"
                },
                base_symbol,
            ],
            "duplicate asset name": [base_seal, base_seal, base_symbol],
            "invalid path": [
                {**base_seal, "embedded_in": "../references/full-page.png"},
                base_symbol,
            ],
            "missing path": [
                {
                    **base_seal,
                    "embedded_in": "packages/form-renderer/src/forms/assets/missing.png",
                },
                base_symbol,
            ],
            "obsolete crop": [
                {**base_seal, "crop_box_px": [10, 10, 30, 30]},
                base_symbol,
            ],
            "generic logo": [
                base_seal,
                {**base_symbol, "asset": "generic_downloaded_logo"},
            ],
            "wrong live caption": [
                base_seal,
                {**base_symbol, "caption_text": "GUESS"},
            ],
            "nonzero matrix differences": [
                base_seal,
                {**base_symbol, "encoder_proof": {"module_differences": 1, "rows": 7}},
            ],
        }
        for name, assets in invalid_cases.items():
            with self.subTest(name=name):
                self.write_artwork_manifest(
                    manifest_path,
                    [
                        self.artwork_form(
                            code="TEST",
                            revision="2018",
                            assets=assets,
                        )
                    ],
                )
                authorization, errors = verifier._load_runtime_artwork_authorization(
                    manifest_path, workspace
                )
                self.assertEqual(
                    authorization,
                    verifier.EMPTY_RUNTIME_ARTWORK_AUTHORIZATION,
                )
                self.assertNotEqual(errors, ())


if __name__ == "__main__":
    unittest.main()
