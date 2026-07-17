from __future__ import annotations

import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
HTML_HOST_PATH = (
    REPOSITORY_ROOT / "crates/bir-desktop/src/views/html_form_preview.rs"
)
FORM_VIEW_PATH = REPOSITORY_ROOT / "crates/bir-desktop/src/views/form_2551q_view.rs"
LINUX_HTML_HOST_PATH = (
    REPOSITORY_ROOT / "crates/bir-desktop/src/views/linux_html_preview/runtime.rs"
)
RENDERER_ENTRY_PATH = REPOSITORY_ROOT / "apps/form-preview/src/main.tsx"


class NativeRendererSecurityPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.html_host = HTML_HOST_PATH.read_text(encoding="utf-8")
        cls.form_view = FORM_VIEW_PATH.read_text(encoding="utf-8")
        cls.linux_html_host = LINUX_HTML_HOST_PATH.read_text(encoding="utf-8")
        cls.renderer_entry = RENDERER_ENTRY_PATH.read_text(encoding="utf-8")

    def test_native_host_blocks_unvalidated_browser_print_surfaces(self) -> None:
        self.assertIn('Object.defineProperty(window, "print"', self.html_host)
        self.assertIn("Script-initiated printing is disabled", self.html_host)
        self.assertNotIn(
            'Object.defineProperty(window, "authorizeEbirNativePrint"', self.html_host
        )
        self.assertNotIn("native_authorized_print_script", self.html_host)
        self.assertNotIn("const originalRendererPrint", self.html_host)
        self.assertIn("ICoreWebView2_16", self.html_host)
        self.assertIn("PrintCompletedHandler", self.html_host)
        self.assertIn("start_windows_system_print", self.html_host)
        self.assertIn("Native print guard installation failed", self.html_host)
        self.assertIn('window.addEventListener("DOMContentLoaded"', self.html_host)
        self.assertIn("with_browser_accelerator_keys(false)", self.html_host)
        self.assertIn("with_default_context_menus(false)", self.html_host)
        self.assertIn('document.addEventListener("contextmenu"', self.html_host)

    def test_renderer_logs_do_not_emit_raw_ipc_or_taxpayer_values(self) -> None:
        self.assertNotIn("body = request.body()", self.html_host)
        self.assertNotIn("tracing::error!(%error", self.html_host)
        for forbidden in (
            "tin = %self.draft.tin",
            "name = %self.draft.taxpayer_name",
            "total = self.draft.total_amount_payable",
            "name = %render_draft.taxpayer_name",
            "total = render_draft.total_amount_payable",
            "error = %error, \"Failed to persist queued 2551Q draft\"",
            "error = %error, \"2551Q queue cancellation was rejected\"",
            'Err(format!("Database lock is unavailable: {error}"))',
            'tracing::error!("PDF viewer failed to open: {err}")',
            'tracing::error!("PDF generation failed: {err}")',
            'tracing::warn!("Failed to copy receipt: {e}")',
        ):
            self.assertNotIn(forbidden, self.form_view)

    def test_linux_host_consumes_the_renderer_stable_geometry_protocol(self) -> None:
        self.assertIn(
            "geometry_reports: [RendererGeometryReportMessage; 2]",
            self.linux_html_host,
        )
        self.assertIn("let [first, second] = geometry_reports.map", self.linux_html_host)
        self.assertIn("if first != second", self.linux_html_host)
        self.assertNotIn(
            "PageCount {\n        render_epoch: u64,\n        page_count: usize,",
            self.linux_html_host,
        )
        self.assertIn(
            "geometry_reports: [previousMeasurement, measurement]",
            self.renderer_entry,
        )

    def test_every_native_host_requires_the_same_document_identity_handshake(self) -> None:
        for host in (self.html_host, self.linux_html_host):
            self.assertIn("struct RendererIpcMessage", host)
            self.assertIn("document_run_id: String", host)
            self.assertIn("envelope_hash: String", host)
            self.assertIn("RendererBoot", host)
            self.assertIn("document_boot_accepted", host)
            self.assertIn("document_identity_rejected", host)
            self.assertIn("arrived before the host document identity boot handshake", host)
            self.assertIn("was replayed by a reload or replacement document", host)

        self.assertIn(
            "renderer_document_identity_script(",
            self.linux_html_host,
        )
        self.assertIn("&document_identity", self.linux_html_host)
        self.assertIn(
            "document_identity: RendererDocumentIdentity",
            self.linux_html_host,
        )
        self.assertIn(
            "document identity changed after native output started",
            self.linux_html_host,
        )
        self.assertIn(
            "Secure Linux renderer retry requires closing and reopening the preview",
            self.linux_html_host,
        )


if __name__ == "__main__":
    unittest.main()
