//! Native WKWebView snapshot for the print preview's scrolled screenshot.
//!
//! `screencapture -l <window>` returns the window as the compositor last
//! showed it, and the WebView's own layer is not reliably part of that image:
//! the preview tiles came back as the toolbar over a dark canvas even with the
//! window in front. WebKit can render its own content on request, independent
//! of window compositing — `takeSnapshotWithConfiguration:` — so the preview
//! target tiles from that instead. The result arrives asynchronously on the
//! main thread; the drain polls the slot each frame.

use std::sync::{Arc, Mutex};

use block2::RcBlock;
use gpui_agent::RgbaImage;
use objc2_app_kit_modern::{NSBitmapFormat, NSBitmapImageRep, NSImage};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation_modern::{NSError, NSNumber};
use objc2_modern::{AllocAnyThread, MainThreadMarker};
use objc2_web_kit_modern::WKSnapshotConfiguration;
use wry::WebViewExtMacOS;

/// Filled once by the completion handler: the tile, or why there is none.
pub type SnapshotSlot = Arc<Mutex<Option<Result<RgbaImage, String>>>>;

pub fn new_slot() -> SnapshotSlot {
    Arc::new(Mutex::new(None))
}

/// Ask WebKit for `rect` (WebView coordinates, points) at `rect.w` points wide —
/// WebKit renders at the backing scale, so a 1200 pt request yields 2400 px on
/// a Retina display, the same scale `screencapture` tiles have. The image is
/// converted to RGBA inside the handler and stored in `slot`.
pub fn take_snapshot(
    webview: &wry::WebView,
    rect: (f32, f32, f32, f32),
    slot: SnapshotSlot,
) -> Result<(), String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "WebView snapshot must start on the main thread".to_string())?;
    let wk = webview.webview();
    // SAFETY: main thread (checked above); the configuration is a plain
    // value object and every setter takes owned scalars or a retained NSNumber.
    let config = unsafe { WKSnapshotConfiguration::new(mtm) };
    unsafe {
        config.setRect(CGRect::new(
            CGPoint::new(f64::from(rect.0), f64::from(rect.1)),
            CGSize::new(f64::from(rect.2), f64::from(rect.3)),
        ));
        config.setSnapshotWidth(Some(&NSNumber::numberWithDouble(f64::from(rect.2))));
        // The tile is requested right after `window.scrollTo`; make WebKit
        // flush that layout before it renders.
        config.setAfterScreenUpdates(true);
    }
    let handler = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        let result = unsafe { image_to_rgba(image, error) };
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
    unsafe { wk.takeSnapshotWithConfiguration_completionHandler(Some(&config), &handler) };
    Ok(())
}

/// # Safety
/// `image` and `error` are the raw pointers WebKit passes to the completion
/// handler; either may be null, and both are valid for the call's duration.
unsafe fn image_to_rgba(image: *mut NSImage, error: *mut NSError) -> Result<RgbaImage, String> {
    if image.is_null() {
        let detail = if error.is_null() {
            "WebKit returned no image".to_string()
        } else {
            unsafe { &*error }.localizedDescription().to_string()
        };
        return Err(gpui_agent::screenshot_unavailable(format!(
            "WebView snapshot failed: {detail}"
        )));
    }
    let image = unsafe { &*image };
    let tiff = image
        .TIFFRepresentation()
        .ok_or_else(|| gpui_agent::screenshot_unavailable("WebView snapshot has no bitmap"))?;
    let rep = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff)
        .ok_or_else(|| gpui_agent::screenshot_unavailable("WebView snapshot bitmap unreadable"))?;
    bitmap_to_rgba(&rep)
}

fn bitmap_to_rgba(rep: &NSBitmapImageRep) -> Result<RgbaImage, String> {
    let width = rep.pixelsWide();
    let height = rep.pixelsHigh();
    let spp = rep.samplesPerPixel();
    let bps = rep.bitsPerSample();
    let bytes_per_row = rep.bytesPerRow();
    if width <= 0 || height <= 0 {
        return Err(gpui_agent::screenshot_unavailable(
            "WebView snapshot is empty",
        ));
    }
    if rep.isPlanar() || bps != 8 || !(3..=4).contains(&spp) {
        return Err(gpui_agent::screenshot_unavailable(format!(
            "WebView snapshot bitmap layout unsupported (planar={}, bits={bps}, samples={spp})",
            rep.isPlanar()
        )));
    }
    let alpha_first = rep.bitmapFormat().contains(NSBitmapFormat::AlphaFirst);
    let data = rep.bitmapData();
    if data.is_null() {
        return Err(gpui_agent::screenshot_unavailable(
            "WebView snapshot has no pixel data",
        ));
    }
    let (width, height, spp, bytes_per_row) = (
        width as usize,
        height as usize,
        spp as usize,
        bytes_per_row as usize,
    );
    // SAFETY: `bitmapData` points at `pixelsHigh * bytesPerRow` bytes owned by
    // `rep`, which outlives this function; rows are read strictly in bounds.
    let bytes = unsafe { std::slice::from_raw_parts(data, height * bytes_per_row) };
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let row = &bytes[y * bytes_per_row..y * bytes_per_row + width * spp];
        for px in row.chunks_exact(spp) {
            match (spp, alpha_first) {
                (4, true) => pixels.extend_from_slice(&[px[1], px[2], px[3], px[0]]),
                (4, false) => pixels.extend_from_slice(px),
                _ => pixels.extend_from_slice(&[px[0], px[1], px[2], 255]),
            }
        }
    }
    RgbaImage::new(width as u32, height as u32, pixels)
}

/// Rows `skip_top..skip_top+take` of a tile, in logical pixels of a `viewport_h`
/// tall snapshot (WebKit rendered it at `scale = height / viewport_h`).
pub fn crop_tile(
    tile: &RgbaImage,
    viewport_h: f32,
    skip_top_px: f32,
    take_height_px: f32,
) -> Result<RgbaImage, String> {
    if viewport_h < 1.0 {
        return Err(gpui_agent::scroll_unavailable(
            "viewport height must be >= 1",
        ));
    }
    let scale = tile.height as f32 / viewport_h;
    let y = ((skip_top_px * scale).round() as i64).max(0) as u32;
    let h = ((take_height_px * scale).round() as u32).max(1);
    let y = y.min(tile.height.saturating_sub(1));
    let h = h.min(tile.height - y);
    if h == 0 {
        return Err(gpui_agent::screenshot_unavailable(
            "WebView tile crop is empty",
        ));
    }
    let row = tile.width as usize * 4;
    let start = y as usize * row;
    let end = start + h as usize * row;
    RgbaImage::new(tile.width, h, tile.pixels[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(w: u32, h: u32) -> RgbaImage {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for _ in 0..w {
                px.extend_from_slice(&[y as u8, 0, 0, 255]);
            }
        }
        RgbaImage::new(w, h, px).unwrap()
    }

    #[test]
    fn crop_tile_scales_logical_rows_by_the_snapshot_scale() {
        // 10 logical px tall, rendered at 2x = 20 rows.
        let tile = rows(4, 20);
        let slice = crop_tile(&tile, 10.0, 3.0, 4.0).unwrap();
        assert_eq!((slice.width, slice.height), (4, 8));
        assert_eq!(slice.pixels[0], 6, "starts at logical row 3 = pixel row 6");
        assert_eq!(slice.pixels[(7 * 4) * 4], 13, "ends at pixel row 13");
    }

    #[test]
    fn crop_tile_clamps_to_the_tile() {
        let tile = rows(4, 20);
        let slice = crop_tile(&tile, 10.0, 8.0, 10.0).unwrap();
        assert_eq!(slice.height, 4, "only rows 16..20 exist");
        assert!(crop_tile(&tile, 0.0, 0.0, 1.0).is_err());
    }
}
