# Native Print and PDF Export

Preview, print, and direct PDF export must consume the same immutable envelope and semantic document.

## Preflight

Before native output:

1. Enter print-mode CSS.
2. Wait for fonts.
3. Require two identical geometry measurements.
4. Validate page rectangles, page count, paper size, clipping, and overflow.
5. Consume a one-use output nonce.

Reject output when readiness fails. Never fall back silently to a legacy renderer.

## Platform expectations

- macOS: use `WKWebView` PDF/print APIs.
- Windows: use `ICoreWebView2_16::Print` for system output and
  `ICoreWebView2_7::PrintToPdf` for direct export. Both operations must use a
  print-settings object derived from the provider's validated point geometry
  (`points / 72` inches), portrait orientation, zero margins, scale 1,
  backgrounds enabled, selection-only disabled, and headers/footers disabled.
  Treat the operation as successful only when the asynchronous callback
  HRESULT succeeds and its print status/PDF result also succeeds. Never fall
  back to `window.print()` when a WebView2 interface, printer, or callback
  fails.
- Linux: use the app-owned WebKitGTK host and print operation under both X11 and Wayland.

## PDF validation

Write to a unique sibling temporary file. Validate PDF syntax, expected page count, MediaBox/CropBox tolerance, rotation, nonempty page content, finite geometry, and form/envelope identity. Replace the destination atomically only after validation; preserve an existing destination on failure.

Record platform and packaged-offline evidence separately. A development WebView success does not prove a signed package.

Source review, macOS unit tests, and a Windows-target compile check are not
Windows runtime evidence. Keep the Windows evidence field unset until a signed
package is exercised on supported Windows versions with the packaged WebView2
host and supported WebView2 runtime: verify each supported paper height,
callback failure behavior, an unavailable printer, successful
physical/virtual-printer output, direct PDF geometry, offline startup, and
shutdown cleanup.
