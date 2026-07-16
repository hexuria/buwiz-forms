from __future__ import annotations

import base64
import importlib.util
import struct
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "audit_no_legacy.py"
SPEC = importlib.util.spec_from_file_location("audit_no_legacy", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
audit_no_legacy = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit_no_legacy
SPEC.loader.exec_module(audit_no_legacy)


def fake_png(width: int, height: int) -> bytes:
    return b"\x89PNG\r\n\x1a\n" + b"\x00" * 8 + struct.pack(">II", width, height)


class NoLegacyAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative: str, content: str | bytes) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            path.write_bytes(content)
        else:
            path.write_text(content, encoding="utf-8")
        return path

    def test_clean_html_only_fixture_passes(self) -> None:
        self.write(
            "package.json",
            '{"scripts":{"build:forms":"vite build"},"devDependencies":{"vite":"1"}}',
        )
        self.write(
            ".github/workflows/ci.yml",
            "- uses: actions/setup-node@v4\n- run: npm ci\n- run: npm run build:forms\n",
        )
        self.write(
            "packages/form-renderer/src/discrete.ts",
            "export const seal = 'data:image/png;base64,"
            + base64.b64encode(fake_png(80, 80)).decode("ascii")
            + "';\n",
        )
        self.write(
            "packages/form-renderer/references/official-page.png",
            fake_png(1224, 1872),
        )
        self.write("docs/retirement.md", "Typst and the legacy renderer were removed.\n")
        self.write(
            "scripts/tests/test_old_guard.py",
            "assert 'typst' not in package\n",
        )

        result = audit_no_legacy.audit_repository(self.root)

        self.assertTrue(result.passed, audit_no_legacy.format_report(result))

    def test_violations_are_grouped_and_deterministic(self) -> None:
        self.write(
            "crates/print/src/lib.rs",
            'let cmd = Command::new("typst");\n'
            "let viewer = PdfViewerView::new();\n"
            'let resources = find_resource_dir("formtypes");\n',
        )
        self.write(
            "crates/print/src/test_only.rs",
            '#[cfg(test)]\nmod tests { fn old() { Command::new("typst"); } }\n',
        )
        self.write("formtypes/2551Q/template.typ", "#set page(width: 612pt)\n")
        self.write(
            "formtypes/2551Q/pages/page1.svg",
            '<svg width="612pt" height="936pt" viewBox="0 0 612 936"/>',
        )
        self.write(
            "justfile",
            "package:\n"
            "    cp target/pinned-typst/typst release/typst\n"
            "    cp -R formtypes release/formtypes\n"
            "    cp -R node_modules release/node_modules\n",
        )
        self.write("assets/macos/typst.entitlements.plist", "<plist/>\n")

        first = audit_no_legacy.audit_repository(self.root)
        second = audit_no_legacy.audit_repository(self.root)

        self.assertEqual(first.violations, second.violations)
        self.assertFalse(first.passed)
        categories = {violation.category for violation in first.violations}
        self.assertEqual(categories, set(audit_no_legacy.CATEGORIES))
        report = audit_no_legacy.format_report(first)
        for category in audit_no_legacy.CATEGORIES:
            self.assertIn(f"[{category}]", report)

    def test_build_time_node_is_allowed_but_runtime_copy_is_not(self) -> None:
        workflow = self.write(
            ".github/workflows/release.yml",
            "- uses: actions/setup-node@v4\n"
            "- run: npm ci\n"
            "- run: npm run build:forms\n",
        )
        clean = audit_no_legacy.audit_repository(self.root)
        self.assertTrue(clean.passed, audit_no_legacy.format_report(clean))

        workflow.write_text(
            workflow.read_text(encoding="utf-8")
            + "- run: cp -R node_modules package/node_modules\n",
            encoding="utf-8",
        )
        dirty = audit_no_legacy.audit_repository(self.root)
        self.assertEqual(
            [violation.category for violation in dirty.violations],
            ["runtime-node"],
        )

    def test_full_page_embedded_image_fails_but_discrete_artwork_passes(self) -> None:
        discrete = base64.b64encode(fake_png(240, 80)).decode("ascii")
        full_page = base64.b64encode(fake_png(1224, 1872)).decode("ascii")
        source = self.write(
            "packages/form-renderer/src/art.ts",
            f"export const barcode = 'data:image/png;base64,{discrete}';\n",
        )
        self.assertTrue(audit_no_legacy.audit_repository(self.root).passed)

        source.write_text(
            source.read_text(encoding="utf-8")
            + f"export const page = 'data:image/png;base64,{full_page}';\n",
            encoding="utf-8",
        )
        result = audit_no_legacy.audit_repository(self.root)
        self.assertEqual(
            [violation.category for violation in result.violations],
            ["full-page-background"],
        )

    def test_assembled_package_scan_rejects_runtime_payloads(self) -> None:
        package = self.root / "assembled"
        self.write("assembled/bin/node", b"binary")
        self.write("assembled/bin/typst", b"binary")
        self.write("assembled/resources/formtypes/layout.json", "{}")
        self.write("assembled/resources/node_modules/react/index.js", "runtime")
        self.write("assembled/resources/old.typ", "#set page()")
        self.write("assembled/resources/page.png", fake_png(1224, 1872))
        self.write("assembled/assets/app.js", "button.textContent='Open Legacy Preview'")

        result = audit_no_legacy.audit_package_directory(package)

        categories = {violation.category for violation in result.violations}
        self.assertEqual(
            categories,
            {
                "typst-packaging",
                "typ-artifact",
                "runtime-formtypes",
                "full-page-background",
                "legacy-renderer",
                "runtime-node",
            },
        )


if __name__ == "__main__":
    unittest.main()
