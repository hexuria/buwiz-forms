//! In-app print preview: rasterize the static print HTML with Blitz
//! (stylo + taffy + parley, vello_cpu) into one RGBA bitmap per `.sheet`.
use std::sync::Arc;

use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{DocumentConfig, util::Color};
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};

/// Screen-only extras in `SCREEN_CSS` (grey desk, gaps, shadow, label) are
/// neutralised so each sheet paints exactly as on paper.
const BLITZ_CSS: &str = "<style id=\"blitz-view\">html{background:#fff !important;width:auto !important}\
body{padding:0 !important;background:#fff !important}\
.sheet{margin:0 !important;box-shadow:none !important}.sheet::after{content:none !important}</style>";

/// Synchronous provider for the only resources the frozen sheets reference:
/// `data:` fonts (base64) and, defensively, `file:` URLs.
struct LocalProvider;
impl NetProvider for LocalProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url;
        let bytes = match url.scheme() {
            "data" => decode_data_url(url.as_str()),
            "file" => url.to_file_path().ok().and_then(|p| std::fs::read(p).ok()),
            _ => None,
        };
        if let Some(b) = bytes {
            handler.bytes(url.to_string(), Bytes::from(b));
        }
    }
}

fn decode_data_url(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let body = s.strip_prefix("data:")?;
    let (meta, data) = body.split_once(',')?;
    if meta.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .ok()
    } else {
        Some(percent_decode(data))
    }
}
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

pub struct Page {
    /// Device pixels (CSS px × scale), BGRA premultiplied (GPUI order).
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// CSS px size, for display at 100 %.
    pub css_w: f32,
    pub css_h: f32,
}

pub struct Rendered {
    /// `None` where the sheet is unchanged (not repainted).
    pub pages: Vec<Option<Page>>,
}

fn build(static_html: &str, scale: f32) -> HtmlDocument {
    let html = match static_html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &static_html[..at], BLITZ_CSS, &static_html[at..]),
        None => format!("{BLITZ_CSS}{static_html}"),
    };
    // A4 sheet in CSS px (595.28pt × 96/72).
    let sheet_w = (crate::print_sheet::PAPER_WIDTH_PT * 96.0 / 72.0).ceil();
    let sheet_h = (crate::print_sheet::PAPER_HEIGHT_PT * 96.0 / 72.0).ceil();
    let mut doc = HtmlDocument::from_html(
        &html,
        DocumentConfig {
            net_provider: Some(Arc::new(LocalProvider) as _),
            viewport: Some(Viewport::new(
                (sheet_w * scale) as u32,
                (sheet_h * scale) as u32,
                scale,
                ColorScheme::Light,
            )),
            ..Default::default()
        },
    );
    for _ in 0..4 {
        doc.resolve(0.0);
    }
    doc
}

fn sheet_ids(doc: &HtmlDocument) -> Vec<blitz_dom::NodeId> {
    doc.as_ref()
        .query_selector_all(".sheet")
        .map(|v| v.to_vec())
        .unwrap_or_default()
}

fn paint_sheets(doc: &mut HtmlDocument, scale: f32, only: Option<&[bool]>) -> Vec<Option<Page>> {
    let sheets = sheet_ids(doc);
    let base = doc.as_mut();
    // Bounding rects depend on the current scroll; measure from the top.
    base.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 0.0 });
    let rects: Vec<_> = sheets
        .iter()
        .filter_map(|id| base.get_client_bounding_rect(*id))
        .collect();
    let mut pages = Vec::with_capacity(rects.len());
    for (i, r) in rects.into_iter().enumerate() {
        if only.is_some_and(|m| !m.get(i).copied().unwrap_or(true)) {
            pages.push(None);
            continue;
        }
        let (cw, ch) = (r.width.max(1.0), r.height.max(1.0));
        let (w, h) = (
            (cw * scale as f64).round() as u32,
            (ch * scale as f64).round() as u32,
        );
        base.set_viewport(Viewport::new(w, h, scale, ColorScheme::Light));
        base.resolve(0.0);
        base.set_viewport_scroll(blitz_dom::Point {
            x: r.x.max(0.0),
            y: r.y,
        });
        let rgba = render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| {
                scene.fill(
                    peniko::Fill::NonZero,
                    Default::default(),
                    Color::WHITE,
                    Default::default(),
                    &peniko::kurbo::Rect::new(0.0, 0.0, w as f64, h as f64),
                );
                paint_scene(scene, base, scale as f64, w, h, 0, 0)
            },
            w,
            h,
        );
        let mut rgba = rgba;
        // vello_cpu: premultiplied RGBA; GPUI wants premultiplied BGRA. Done
        // here, on the worker thread.
        rgba_to_bgra(&mut rgba);
        pages.push(Some(Page {
            width: w,
            height: h,
            rgba,
            css_w: cw as f32,
            css_h: ch as f32,
        }));
        base.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 0.0 });
    }
    pages
}

/// Set the text of field spans in place; `None` if a span is missing (then
/// the caller rebuilds). Returns which sheets hold a changed span.
fn patch(doc: &mut HtmlDocument, changes: &[(String, String)]) -> Option<Vec<bool>> {
    let sheets = sheet_ids(doc);
    let mut dirty = vec![false; sheets.len()];
    let base = doc.as_mut();
    let mut targets = Vec::with_capacity(changes.len());
    for (id, text) in changes {
        let node = base.query_selector(&format!("#{id}")).ok().flatten()?;
        // which sheet?
        let mut cur = Some(node);
        while let Some(n) = cur {
            if let Some(i) = sheets.iter().position(|s| *s == n) {
                dirty[i] = true;
                break;
            }
            cur = base.get_node(n).and_then(|n| n.parent);
        }
        let child = base
            .get_node(node)?
            .children
            .iter()
            .copied()
            .find(|c| base.get_node(*c).is_some_and(|n| n.is_text_node()));
        targets.push((node, child, decode_entities(text)));
    }
    {
        let mut m = base.mutate();
        for (node, child, text) in &targets {
            match child {
                Some(c) => m.set_node_text(*c, text),
                None if !text.is_empty() => {
                    let t = m.create_text_node(text);
                    m.append_children(*node, &[t]);
                }
                None => {}
            }
        }
    }
    base.resolve(0.0);
    Some(dirty)
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

pub enum Job {
    /// Rebuild from this document; repaint only the sheets marked `true`
    /// (all of them when `None`).
    Full(String, Option<Vec<bool>>),
    /// Changed `(span id, attribute-escaped text)` pairs, plus the whole
    /// document as the fallback if patching is impossible.
    Patch(Vec<(String, String)>, String),
}

/// Keeps one Blitz document alive per preview so live edits are text
/// patches + incremental restyle/relayout instead of a reparse.
pub struct Worker {
    tx: std::sync::mpsc::Sender<(Job, std::sync::mpsc::Sender<Rendered>)>,
}

impl Worker {
    pub fn spawn(scale: f32) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(Job, std::sync::mpsc::Sender<Rendered>)>();
        std::thread::Builder::new()
            .name("blitz-preview".into())
            .spawn(move || {
                let mut doc: Option<HtmlDocument> = None;
                for (job, reply) in rx {
                    let patched = match (&job, doc.as_mut()) {
                        (Job::Patch(changes, _), Some(d)) => {
                            patch(d, changes).map(|mask| (d, mask))
                        }
                        _ => None,
                    };
                    let r = match patched {
                        Some((d, mask)) => Rendered {
                            pages: paint_sheets(d, scale, Some(&mask)),
                        },
                        None => {
                            let (html, only) = match job {
                                Job::Full(h, only) => (h, only),
                                Job::Patch(_, h) => (h, None),
                            };
                            let mut d = build(&html, scale);
                            let pages = paint_sheets(&mut d, scale, only.as_deref());
                            doc = Some(d);
                            Rendered { pages }
                        }
                    };
                    let _ = reply.send(r);
                }
            })
            .expect("spawn blitz worker");
        Self { tx }
    }

    pub fn submit(&self, job: Job) -> std::sync::mpsc::Receiver<Rendered> {
        let (rtx, rrx) = std::sync::mpsc::channel();
        let _ = self.tx.send((job, rtx));
        rrx
    }
}

/// Head + one chunk per `.sheet`, for change detection between live updates.
pub fn segments(static_html: &str) -> Vec<String> {
    let mut cuts: Vec<usize> = static_html
        .match_indices("<div class=\"sheet")
        .map(|(i, _)| i)
        .collect();
    cuts.insert(0, 0);
    cuts.push(static_html.len());
    cuts.windows(2)
        .map(|w| static_html[w[0]..w[1]].to_string())
        .collect()
}

/// Which sheets differ (all of them if the head or the sheet count changed).
pub fn dirty_mask(old: &[String], new: &[String]) -> Vec<bool> {
    let n = new.len().saturating_sub(1);
    if old.len() != new.len() || old.first() != new.first() {
        return vec![true; n];
    }
    (1..new.len()).map(|i| old[i] != new[i]).collect()
}

/// R<->B swap, one u32 per pixel (little-endian 0xAABBGGRR -> 0xAARRGGBB).
fn rgba_to_bgra(buf: &mut [u8]) {
    // SAFETY: u8 -> u32 reinterpretation; align_to_mut handles the edges.
    let (head, mid, tail) = unsafe { buf.align_to_mut::<u32>() };
    for px in head
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .chain(tail.as_chunks_mut::<4>().0.iter_mut())
    {
        px.swap(0, 2);
    }
    for x in mid.iter_mut() {
        let v = *x;
        *x = (v & 0xFF00_FF00) | ((v & 0xFF) << 16) | ((v >> 16) & 0xFF);
    }
}
