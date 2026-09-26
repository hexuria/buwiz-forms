# Plan — fixes from the Mac smoke of buwiz-forms #57 + gpui-agent 8b864fb

Findings are from the Mac run on 2026-09-12 against `f609b6c3` (#57 tip, which
already contains main `aa797d5e` = #52–#59). Every item below was measured, and
items 1–3 were **proven with a throwaway build** (reverted; the worktree is back
at `f609b6c3` clean). PNG evidence is under `/tmp/bir-gpui-shots/`.

Nothing here merges #57. The fixes are meant to land **on top of #57's branch**
(or as a follow-up PR based on it) by whoever owns #57; item 5 is a gpui-agent
change.

---

## Findings

| # | What was seen | Root cause (verified) | Severity |
|---|---|---|---|
| 1 | Stitched form (`form-scrolled.png`, 3040×4788) has a 64 px band of app chrome at the top of **every** tile (rows 0, 1455, 2910) and **drops 32 pt of content per tile** — item 11 "Category of Withholding Agent" is missing between ATC and item 13 | bir `drain.rs::window_size()` passes `window.bounds()` — the **frame**, which on macOS includes the **32.0 pt title bar** (measured: frame 1032 vs content 1000) — but `ScrollHandle::bounds()` is content-relative. gpui-agent's `crop_window_png` computes `extra_top = png_h/scale − window_h` to absorb the title bar and gets **0**, so every crop starts 32 pt too high. Proven: with `window.viewport_size()` the band detector finds no bands and item 11 is back | **High** — silently loses content |
| 2 | Print preview scrolled capture is **1 tile** (`content_height == viewport_height`, 853 or 1049) instead of the 3 A4 sheets | `frozen_html_preview.rs::request_js_metrics` evaluates `JSON.stringify({...})`; wry's `evaluate_script_with_callback` **already** serialises the result to JSON, so bir receives a JSON *string* and `value.get("content")` is `None` → metrics `0/0/0` → the `js.content_height > gpui + 1` guard fails → the GPUI shell's metrics win. Proven: returning the object gives `{"content":3428,"viewport":853,"offset":…}`, **5 tiles**, 2400×6856, offsets round-trip | **High** — feature does not work for the WebView |
| 3 | Even with 5 correct tiles the preview PNG is **blank** (dark canvas, only rounded sheet corners). `print-scrolled-front.png` had content **once** in ~6 attempts | `screencapture -l <CGWindowID>` does not reliably include the **WKWebView layer** of this window. A direct `screencapture -l` run from the shell is equally blank, and the preview being first in z-order does not help — so it is **not occlusion** (my earlier report said occlusion; that was wrong). The one success was a freshly created second preview window whose layer happened to be composited | **High** — WebView capture needs a native path |
| 4 | `assert --in-viewport` → `in_viewport_unavailable: node … bounds are zero` for every node | bir's `host.rs::tree()` never calls `UiNode::with_bounds`; every node is `0,0,0,0` (visible in `snapshot`). gpui-agent correctly refuses to judge viewport membership on zero bounds | Medium — #40 feature unusable in bir |
| 5 | `visible` never appears in the snapshot; `assert --visible false` correctly fails, but a **collapsed** sidebar still reports `visible: true` with 9 children | `visible` is serialised only when `false` (`skip_serializing_if = "is_visible"`) — fine. bir never calls `with_visible(false)`: no sidebar-collapsed state, no overlay-hidden state reaches the tree | Medium — #40 half-wired |
| 6 | `keybinding --id … --scope focused` **without** `--activate` → `keybinding_unavailable: app not focused` | By design: `authorize_keybinding_op(op, catalog, window.is_window_active())` requires the OS-focused window for a `focused`-scope binding; `--activate` calls `activate_window()` first. Not a bug; the smoke doc should say "use `--activate` from scripts" and the error could suggest it | Low — docs/UX |
| 7 | First smoke pass reported Linux-style chords (`ctrl-b`, `super-m`) and a `cfg(not(macos))` screenshot error from a Mac build | `127.0.0.1:17421` was held by a **Cursor extension host** (another agent's forward to a Linux box). The Mac bir logged `gpui-agent failed to bind` and the CLI silently talked to the other host | Low — environment; worth a guard |

Confirmed working on the Mac build: `hello`, `keybindings` (mac chords), `keybinding --activate` (sidebar really collapses, `sidebar-front-A/B.png`), viewport screenshot (Screen Recording already granted for this binary), `form.pdf`, `wait-until` (both success and clean timeout), scroll offset restore after a job.

Answer to "is this the correct way to do scrolling screenshots?": **yes for GPUI-drawn content** — semantic `ScrollHandle` offset → wait one paint → `screencapture -l` the window → crop the scroller's viewport → stitch, restore. Items 1 and 2 are two small bugs in that pipeline, not a design problem. **For the WebView it is the wrong tool** (item 3): the WebView must snapshot itself.

---

## Work items (in order)

### 1. bir — crop from the content size, not the frame  (`drain.rs`)
```rust
fn window_size(window: &Window) -> (f32, f32) {
    let size = window.viewport_size();          // was window.bounds().size
    (f32::from(size.width), f32::from(size.height))
}
```
Test: gpui-agent `scroll_capture.rs` has no `crop_window_png` test with a title
bar — add one: a 100×132 PNG for a "100×100 window" (`extra_top = 32`), clip
`y = 10`, assert the crop starts at row 42. In bir, keep `window_size` as the
single source for both scrollers (it already is).
Verify: `screenshot --mode scrolled --target form-1601c-scroll`, then the row
scanner (`/tmp/wkprint/rows`) reports **no chrome bands**, and item 11 is
present between ATC and item 13.

### 2. bir — WebView metrics: return the object  (`frozen_html_preview.rs`)
Drop `JSON.stringify(...)`; evaluate `({content, viewport, offset})` and let wry
encode it once. Keep a defensive unwrap: if the parsed value is a
`Value::String`, parse its contents (covers a wry that changes behaviour).
Also make `print_preview_metrics` **wait** for the WebView's numbers instead of
accepting the GPUI shell's on the first frame: `(Ok(_), None) => Err(scroll_unavailable("waiting for WebView metrics"))` — the job already retries
for `METRICS_WAIT_FRAMES` (90) on `Err`.
Test: a unit test on the callback parser with both encodings
(`{"content":3428,…}` and `"{\"content\":3428,…}"`).
Verify: `tiles ≥ 3`, `content_height ≈ 3 × 841.89 + gaps`, last tile offset
= `content − viewport`.

### 3. bir — native WebView snapshot for `print-preview-scroll`  (macOS)
Do not `screencapture` the preview window. Per tile, after `window.scrollTo`,
call **`WKWebView.takeSnapshotWithConfiguration:completionHandler:`** on
`webview.raw().webview()` (`wry::WebViewExtMacOS` → `Retained<WryWebView>`,
a `WKWebView`), with `WKSnapshotConfiguration.rect = viewport` and
`snapshotWidth` = viewport width × scale. The completion handler returns an
`NSImage`; convert to RGBA (`NSBitmapImageRep`) and feed the existing
`stitch_tiles_vertically`. Bindings: `objc2-web-kit` is already a dependency
with `WKWebView` + `WKPDFConfiguration`; add the `WKSnapshotConfiguration`
feature. The callback is async → add a `WaitSnapshot` phase to
`ScrolledShotJob` (store the tile when the block fires, `cx.notify()`).
Simpler alternative worth offering as a second op: **`createPDFWithConfiguration:`**
returns the whole document in one call — expose it as `screenshot --mode document`
(or `form.pdf --rendered`) and let agents rasterise. Either path removes the
compositing lottery and the chrome entirely.
Verify: stitched preview shows three white A4 sheets top-to-bottom with the
receipt page third, no dark tiles, regardless of window z-order.

### 4. bir — populate `bounds` and `visible` in the semantic tree  (`host.rs`, `drain.rs`)
- Scrollers: `ScrollHandle::bounds()` for `form-1601c-scroll` and
  `print-preview-scroll` (already read for metrics — pass them into the tree).
- Page roots: the window `viewport_size()` minus the sidebar width.
- Sidebar: capture its painted bounds once per frame (a `canvas`/prepaint hook
  or `element.on_prepaint` storing into `AppState`) and mark
  `with_visible(false)` when collapsed (the toggle-sidebar state already exists
  in `AppState`). Overlays (`overlay-admin-auth`, command palette): `visible`
  follows their open flag.
Test: host tests asserting `sidebar` is `visible: false` after the collapse
flag is set, and that scroller nodes carry non-zero bounds when the handle has
them.
Verify: `assert --id sidebar --in-viewport true` returns ok; after
`keybinding app.toggle_sidebar --activate`, `assert --id sidebar --visible false`
passes and `wait-until --visible true` after a second toggle passes.

### 5. gpui-agent — `hello` should identify the OS  (`protocol.rs`)
Add `os: "macos" | "linux" | "windows"` next to `platform`, so a smoke script
can assert it is talking to the machine it thinks it is. Keep `platform` as is.
Also make `keybinding_unavailable: app not focused` end with
"(pass `--activate` to OS-activate the window first)".
Test: hello serialisation; CLI prints `os`.

### 6. Smoke script hygiene  (docs only)
Before anything else: `lsof -nP -iTCP:17421 -sTCP:LISTEN` and refuse to run if
the listener is not `bir`; prefer a per-machine `GPUI_AGENT_ADDR`. State that
`keybinding` from a background shell needs `--activate`. Note that Screen
Recording is per-binary-path (a rebuild at the same path keeps the grant; a
different worktree does not).

---

## Order and cost

1 → 2 → 3 are one branch on top of #57 (`drain.rs`, `frozen_html_preview.rs`,
`Cargo.toml`), about half a day including the async snapshot phase; 1 and 2
are ~10 lines each and already proven. 4 is a separate PR (tree plumbing,
~half a day). 5 and 6 are small and independent.

Re-run the full Mac smoke after 1–3 with the pass criteria from the original
brief plus: no chrome bands, ≥3 preview tiles, no blank tiles.
