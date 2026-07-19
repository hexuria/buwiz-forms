# Linux HTML Preview, Print, and PDF Host

Status: implemented behind the HTML renderer support gate; packaged X11 and
Wayland evidence is still required before release promotion.

## Host selection

Linux never opens a form in an external browser.

- X11 uses the GPUI/Wry child WebView.
- Wayland uses a separate eBIRForms-owned GTK3/WebKitGTK window because Wry's
  raw child embedding is documented as X11-only.
- `WAYLAND_DISPLAY` wins over `DISPLAY`, since Wayland sessions commonly also
  expose an XWayland display.
- No detected display fails closed.
- `EBIRFORMS_HTML_LINUX_HOST=gtk` is a diagnostic override.
- `EBIRFORMS_HTML_LINUX_HOST=child` is accepted on X11 and rejected on
  Wayland.

The pure selector and startup/shutdown lifecycle live in
`crates/bir-desktop/src/views/linux_html_preview.rs` and are unit tested on
every host. Linux runtime code is isolated under
`linux_html_preview/runtime.rs`.

## Security and document identity

Both Linux hosts use the same prepared immutable `RenderEnvelopeV1`, custom
`ebirforms://localhost` protocol, CSP, Permissions-Policy, path traversal
guard, nonpersistent WebView, navigation allow-list, worker/media/network
guards, geometry report, and nonce-bound print-mode preflight as macOS and
Windows. The renderer bundle is loaded from packaged application resources;
there is no HTTP server and no browser fallback.

Print and export stay disabled until the host validates page count, 612 x 936
point paper geometry, page rectangles, and all clipping/overflow counters.
Each operation consumes a fresh nonce after the renderer has waited for fonts
and stable geometry.

## Native output

System Print uses `WebKitPrintOperation` with portrait 8.5 x 13 inch paper,
zero margins, scale 100%, backgrounds enabled, and the currently displayed
WebKit document.

Export PDF configures the GTK `Print to File` backend against that same
document. It writes to a unique sibling temporary file created by
`bir_print::html_output::create_pdf_export_temp`. The file is stamped and
validated by `finalize_pdf_export`; the selected destination is replaced only
after page count, MediaBox/CropBox, rotation, content, and envelope evidence
pass. Failure removes the temporary file and preserves an existing
destination.

## Development observation schema

`DevelopmentNativeOutputObservationV1` has a Linux platform variant paired
only with the source-backed `WebKitGtkPrintOperationPdf` backend. WebKitGTK's
`PrintOperation` exposes the completed PDF file rather than one callback
payload per page, so a Linux diagnostic observation must record native page
payloads as explicitly unavailable with a concrete reason. The validator
rejects Linux observations paired with the macOS WKPDF or Windows WebView2
backend.

This shape remains a development diagnostic: it requires
`promotion_eligible: false` and at least one strict-verifier gap. Schema
validation does not register a collector, does not attest either the X11 or
Wayland host, and cannot satisfy the packaged Linux release evidence below.

## Build dependencies

Ubuntu/Debian builders require:

```text
libgtk-3-dev
libwebkit2gtk-4.1-dev
```

Runtime packages require `libgtk-3-0` and `libwebkit2gtk-4.1-0`. The Debian
metadata lists them explicitly in addition to `$auto` shared-library
detection. CI and release builders install the development packages.

## Required Linux release evidence

Run these gates on Linux, not through a macOS cross-check:

```sh
rtk cargo test --locked -p bir-desktop linux_html_preview
rtk cargo check --locked -p bir-desktop --all-targets
rtk cargo clippy --locked -p bir-desktop --all-targets -- -D warnings
rtk npm run build:forms
rtk npm run verify:forms:offline
rtk just _package-linux
```

Then exercise the exact packaged binary twice:

1. X11/Xvfb: assert the `GpuiWryChild` strategy, preview readiness, close and
   reopen, system print, PDF export, and clean process shutdown.
2. Wayland/Weston: assert the `GtkTopLevel` strategy, app-owned window title,
   preview readiness, close and reopen, system print, PDF export, and clean
   process shutdown.

For each export, verify page count and 612 x 936 point MediaBox/CropBox. Repeat
with networking denied and with Typst absent from `PATH`. Record the package
hash, display backend, compositor version, WebKitGTK version, renderer bundle
hash, envelope hash, output hash, and screenshots/logs. Until both packaged
drivers produce this evidence, Linux platform release flags must remain false.

The current non-promotional binding and dual-host attestation/verifier
foundation is documented in
[Linux candidate certification collector/verifier foundation](linux-candidate-certification.md).
Its external operator collector validates separately retained X11 and Wayland
runs, three distinct output/readiness nonces on one immutable document,
pre-existing rollback evidence, packaged-offline evidence, and completed CUPS
jobs without driving the UI or submitting a print. It can verify the portable
workflow candidate without changing a release flag, but it deliberately cannot
substitute that candidate for final `.deb` and release-tarball installation
evidence.

macOS cannot compile or certify the GTK/WebKit runtime because the required
system libraries and display servers are Linux-only. Host builds still compile
the selector/lifecycle tests and all Linux runtime code remains behind
`cfg(target_os = "linux")`; Linux CI is the authoritative compile and runtime
gate.
