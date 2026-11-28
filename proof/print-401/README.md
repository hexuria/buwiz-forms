driver: none — no gpui-agent recipe; the app's own print path (print_sheet prepare_html + static_html staticize) rendered to pictures by headless Chrome.

# Print proof for buwiz-forms#502 (issue #401: a test send must never read as filed)

Branch fix/test-send-filed-401, head see below. The HTML was produced by the crate's own code:
`bir_print::frozen_html` filled document -> `bir_app_api::print_sheet::prepare_html_for` -> `bir_app_api::static_html::staticize`,
exactly what the app's print preview prints. Form: 2552-2018 (multi-page). Pictures: headless Chrome screenshots of the staticized pages.

## Files
- before-test-send-p1.png / before-test-send-p2.png — how a test send printed on main: no warning on ANY page, no receipt; the paper reads exactly like a filed return. (Reproduced through `filled_document_with_receipt` with no receipt — what main's print path produced for a dry run, which is why issue #401 existed.)
- test-send-p1.png / test-send-p2.png — after this pull request: every page carries "Test send: not filed. Nothing reached the BIR." in the sheet's reserved top strip, over a white background, never covering form content. No receipt page (a test send has none).
- real-filing-p1.png / real-filing-p2.png — a real filing after this pull request: no warning, receipt page present, byte-identical pipeline to main.
- hashes.txt — sha256 of each staticized document and its sheet count.

## Why a real filing is byte-identical to main
`git diff 58621b4fe160aa8b4b47392ae5ee800bcfeca937 HEAD -- crates/bir-print/src/frozen_html.rs crates/bir-app-api/src/print_sheet.rs crates/bir-app-api/src/static_html.rs` is insertions-only except one line in `wrap_sheets` that is behavior-identical when no test-send warning exists (the warning move is gated on `before.ends_with(TEST_SEND_WARNING_HTML)`, always false for a real filing). `print_sheet.rs` changes are test-only additions; `frozen_html.rs` adds the test-send wrapper without touching `filled_document_with_receipt`; the injected paper CSS is unchanged (`body>:not(.sheet){display:none !important}`, verified by `paper_css_is_exactly_the_real_filings_css`).
