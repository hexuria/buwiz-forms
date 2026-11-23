//! The print preview's sheets as pictures, drawn on the CPU (Windows and Linux).
//!
//! The preview window used to show the sheet in a web view. On a computer without a GPU (a virtual machine, a
//! remote server) WebView2's page never reaches the screen, and a web view costs a browser process to start. So here
//! the same static HTML (bir-app-api's `print_sheet` + `static_html`: one A4 paper per form page) is laid out by Blitz
//! (stylo, taffy, parley) and painted by vello_cpu into one picture per `.sheet`, which the window shows as images.
//! No web view, no GPU. The PDF and the printout still come from the platform's web engine (`print.rs`), so they are
//! what a browser makes of the sheet; the pictures are for looking at. Ported from the older app's
//! the retired gpui shell's `src/blitz_render.rs`.
//!
//! An edit lays the document out again and repaints only the sheets whose HTML changed; the others keep their
//! pictures.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use base64::Engine as _;
use bir_app_api::print_sheet::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};
use blitz_dom::{DocumentConfig, util::Color};
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};

use crate::print::SheetImage;

/// Device pixels per CSS pixel of the pictures: sharp at "Fit width" on an ordinary screen and at 100% on a dense one.
const SCALE: f32 = 2.0;

/// The screen-only extras of the sheet's style (grey desk, gaps, shadow, label) are taken out, so each sheet paints
/// exactly as on paper; the window draws its own desk.
const SHEET_CSS: &str = "<style id=\"sheet-view\">html{background:#fff !important;width:auto !important}\
body{padding:0 !important;background:#fff !important}\
.sheet{margin:0 !important;box-shadow:none !important}.sheet::after{content:none !important}</style>";

/// The only resources the frozen sheets name: `data:` fonts and, defensively, `file:` URLs. Nothing is fetched.
struct LocalProvider;

impl NetProvider for LocalProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url;
        let bytes = match url.scheme() {
            "data" => decode_data_url(url.as_str()),
            "file" => url.to_file_path().ok().and_then(|path| std::fs::read(path).ok()),
            _ => None,
        };
        if let Some(bytes) = bytes {
            handler.bytes(url.to_string(), Bytes::from(bytes));
        }
    }
}

fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
    if meta.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()
    } else {
        Some(percent_decode(data))
    }
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(byte) = text.get(i + 1..i + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// The document laid out, one A4 viewport wide.
fn build(static_html: &str) -> HtmlDocument {
    let html = match static_html.rfind("</head>") {
        Some(at) => format!("{}{SHEET_CSS}{}", &static_html[..at], &static_html[at..]),
        None => format!("{SHEET_CSS}{static_html}"),
    };
    let width = (PAPER_WIDTH_PT * 96.0 / 72.0).ceil();
    let height = (PAPER_HEIGHT_PT * 96.0 / 72.0).ceil();
    let mut document = HtmlDocument::from_html(
        &html,
        DocumentConfig {
            net_provider: Some(Arc::new(LocalProvider) as _),
            viewport: Some(Viewport::new((width * SCALE) as u32, (height * SCALE) as u32, SCALE, ColorScheme::Light)),
            ..Default::default()
        },
    );
    // Fonts arrive (from `data:` URLs) while the first passes run: a few passes settle the layout.
    for _ in 0..4 {
        document.resolve(0.0);
    }
    document
}

/// A painted sheet: straight RGBA pixels and its size in CSS pixels.
struct Painted {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    css_width: f32,
    css_height: f32,
}

/// Paints the sheets `only` marks (all of them without it); `None` stands where one was left alone.
fn paint(document: &mut HtmlDocument, only: Option<&[bool]>) -> Vec<Option<Painted>> {
    let sheets = document.as_ref().query_selector_all(".sheet").map(|ids| ids.to_vec()).unwrap_or_default();
    let base = document.as_mut();
    // Bounding boxes depend on the scroll: measure from the top.
    base.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 0.0 });
    let boxes: Vec<_> = sheets.iter().filter_map(|id| base.get_client_bounding_rect(*id)).collect();
    let mut painted = Vec::with_capacity(boxes.len());
    for (i, sheet) in boxes.into_iter().enumerate() {
        if only.is_some_and(|mask| !mask.get(i).copied().unwrap_or(true)) {
            painted.push(None);
            continue;
        }
        let (css_width, css_height) = (sheet.width.max(1.0), sheet.height.max(1.0));
        let width = (css_width * f64::from(SCALE)).round() as u32;
        let height = (css_height * f64::from(SCALE)).round() as u32;
        base.set_viewport(Viewport::new(width, height, SCALE, ColorScheme::Light));
        base.resolve(0.0);
        base.set_viewport_scroll(blitz_dom::Point { x: sheet.x.max(0.0), y: sheet.y });
        let rgba = render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| {
                scene.fill(
                    peniko::Fill::NonZero,
                    Default::default(),
                    Color::WHITE,
                    Default::default(),
                    &peniko::kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
                );
                paint_scene(scene, base, f64::from(SCALE), width, height, 0, 0)
            },
            width,
            height,
        );
        painted.push(Some(Painted { width, height, rgba, css_width: css_width as f32, css_height: css_height as f32 }));
        base.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 0.0 });
    }
    painted
}

/// A painted sheet as a PNG in a `data:` URI, which an `Image` shows without a request. The paper is opaque, so
/// the premultiplied pixels vello_cpu gives are the straight ones a PNG holds.
fn picture(sheet: &Painted) -> Result<SheetImage, String> {
    let mut png = Vec::with_capacity(sheet.rgba.len() / 8);
    {
        let mut encoder = png::Encoder::new(&mut png, sheet.width, sheet.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|error| format!("The sheet could not be drawn: {error}"))?;
        writer.write_image_data(&sheet.rgba).map_err(|error| format!("The sheet could not be drawn: {error}"))?;
    }
    let mut src = String::with_capacity(png.len() * 4 / 3 + 32);
    src.push_str("data:image/png;base64,");
    base64::engine::general_purpose::STANDARD.encode_string(&png, &mut src);
    Ok(SheetImage { src, width: f64::from(sheet.css_width), height: f64::from(sheet.css_height) })
}

/// The document's head, then one piece per `.sheet`: what tells which sheets an edit changed.
fn segments(static_html: &str) -> Vec<String> {
    let mut cuts: Vec<usize> = static_html.match_indices("<div class=\"sheet").map(|(at, _)| at).collect();
    cuts.insert(0, 0);
    cuts.push(static_html.len());
    cuts.windows(2).map(|pair| static_html[pair[0]..pair[1]].to_owned()).collect()
}

/// Which sheets differ: all of them when the head or the number of sheets changed.
fn dirty_mask(old: &[String], new: &[String]) -> Vec<bool> {
    let sheets = new.len().saturating_sub(1);
    if old.len() != new.len() || old.first() != new.first() {
        return vec![true; sheets];
    }
    (1..new.len()).map(|i| old[i] != new[i]).collect()
}

/// What was last drawn for a preview.
#[derive(Default)]
struct Drawn {
    segments: Vec<String>,
    images: Vec<SheetImage>,
}

/// The pictures of each open preview, by form code.
static DRAWN: LazyLock<Mutex<HashMap<String, Drawn>>> = LazyLock::new(Default::default);

/// The sheets of `html`, the document of the preview of `key`, as pictures: only the sheets that changed since the
/// last call are painted again. Called off the engine's thread (it takes tens to hundreds of milliseconds).
pub(crate) fn images(key: &str, html: &str) -> Result<Vec<SheetImage>, String> {
    let mut drawn = DRAWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = drawn.entry(key.to_owned()).or_default();
    let now = segments(html);
    let mask = dirty_mask(&entry.segments, &now);
    let known = entry.images.len() == mask.len();
    if !known || mask.iter().any(|changed| *changed) {
        let mut document = build(html);
        let painted = paint(&mut document, known.then_some(mask.as_slice()));
        let mut images = Vec::with_capacity(painted.len());
        for (i, sheet) in painted.iter().enumerate() {
            images.push(match sheet {
                Some(sheet) => picture(sheet)?,
                None => entry.images.get(i).cloned().ok_or("The sheets could not be drawn.")?,
            });
        }
        entry.images = images;
    }
    entry.segments = now;
    Ok(entry.images.clone())
}

/// The preview of `key` closed: its pictures go.
pub(crate) fn forget(key: &str) {
    DRAWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(key);
}

/// Every preview closed (the app quits).
pub(crate) fn forget_all() {
    DRAWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_marks_only_its_sheet() {
        let old = segments("<head>h</head><div class=\"sheet\">a</div><div class=\"sheet\">b</div>");
        let new = segments("<head>h</head><div class=\"sheet\">a</div><div class=\"sheet\">B</div>");
        assert_eq!(dirty_mask(&old, &new), [false, true]);
        assert_eq!(dirty_mask(&old, &old), [false, false]);
        // A changed head, or another number of sheets, is every sheet.
        let head = segments("<head>H</head><div class=\"sheet\">a</div><div class=\"sheet\">b</div>");
        assert_eq!(dirty_mask(&old, &head), [true, true]);
        let more = segments("<head>h</head><div class=\"sheet\">a</div><div class=\"sheet\">b</div><div class=\"sheet\">c</div>");
        assert_eq!(dirty_mask(&old, &more), [true, true, true]);
        assert_eq!(dirty_mask(&[], &old), [true, true]);
    }

    #[test]
    fn data_urls_decode() {
        assert_eq!(decode_data_url("data:font/ttf;base64,aGk="), Some(b"hi".to_vec()));
        assert_eq!(decode_data_url("data:text/plain,a%20b"), Some(b"a b".to_vec()));
        assert_eq!(decode_data_url("file:///x"), None);
    }
}
