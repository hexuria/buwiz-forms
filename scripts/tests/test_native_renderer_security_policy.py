from __future__ import annotations

import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
HTML_HOST_PATH = (
    REPOSITORY_ROOT / "crates/bir-desktop/src/views/html_form_preview.rs"
)
FORM_VIEW_PATH = REPOSITORY_ROOT / "crates/bir-desktop/src/views/form_2551q_view.rs"


class NativeRendererSecurityPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.html_host = HTML_HOST_PATH.read_text(encoding="utf-8")
        cls.form_view = FORM_VIEW_PATH.read_text(encoding="utf-8")

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


if __name__ == "__main__":
    unittest.main()
