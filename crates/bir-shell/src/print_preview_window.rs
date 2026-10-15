//! In-app Print Preview window: a GPUI window with a custom 40 px title bar
//! (themed from `gpui_base::Theme`) over the filled sheets, rendered to
//! bitmaps by Blitz (`blitz_render`). A hidden WKWebView loads the same
//! static HTML only to produce a vector A4 PDF for Print and Share.
//! Host functions get no `&mut App`, so they enqueue a request; a foreground
//! task started in `host::run` drains it and opens (or refreshes) the window.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use std::sync::Arc;
use std::time::Instant;

use gpui_kit as gpui;

use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, ImageSource, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, RenderImage, SharedString,
    StatefulInteractiveElement, Styled, TitlebarOptions, WeakEntity, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, img, point, px, size,
};

const TITLE_BAR_H: f32 = 40.0;

use crate::print_sheet::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};

struct Request {
    path: PathBuf,
    title: String,
    /// Form code this preview mirrors; live edits for it are pushed here.
    key: Option<String>,
}

thread_local! {
    static QUEUE: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
    static VIEWS: RefCell<Vec<WeakEntity<PreviewView>>> = const { RefCell::new(Vec::new()) };
    /// Live sync: freshly filled static HTML per form code (latest wins).
    static LIVE: RefCell<Vec<(String, Producer, Instant)>> = const { RefCell::new(Vec::new()) };
    static LIVE_KEYS: RefCell<std::collections::HashSet<String>> = RefCell::new(Default::default());
}

pub fn request(path: PathBuf, title: String) {
    QUEUE.with(|q| {
        q.borrow_mut().push(Request {
            path,
            title,
            key: None,
        })
    });
}

pub fn request_keyed(path: PathBuf, title: String, key: String) {
    LIVE_KEYS.with(|k| k.borrow_mut().insert(key.clone()));
    QUEUE.with(|q| {
        q.borrow_mut().push(Request {
            path,
            title,
            key: Some(key),
        })
    });
}

/// Whether a preview window for `key` (a form code) is open.
pub fn has_live(key: &str) -> bool {
    LIVE_KEYS.with(|k| k.borrow().contains(key))
}

/// Builds the static document for a live update (runs on a background thread).
pub type Producer = Box<dyn FnOnce() -> String + Send>;

/// Hand a re-fill (as a producer, run off the UI thread) to the open preview for `key`.
pub fn live_update(key: &str, static_html: Producer, started: Instant) {
    LIVE.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|(k, _, _)| k != key);
        l.push((key.to_string(), static_html, started));
    });
}

pub fn start(cx: &mut App) {
    if let Some(p) = std::env::var_os("BIR_PREVIEW_HTML") {
        let title =
            std::env::var("BIR_PREVIEW_HTML_TITLE").unwrap_or_else(|_| "Print Preview".into());
        request(PathBuf::from(p), title);
    }
    cx.spawn(async move |cx| {
        loop {
            // 16 ms: live edits are picked up within one frame.
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let pending: Vec<Request> = QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()));
            let live: Vec<_> = LIVE.with(|l| std::mem::take(&mut *l.borrow_mut()));
            cx.update(|cx| {
                for r in pending {
                    // One window per form: reuse it when already open.
                    let existing = r.key.as_ref().and_then(|k| {
                        VIEWS.with(|v| {
                            v.borrow()
                                .iter()
                                .find(|w| {
                                    w.upgrade().is_some_and(|e| {
                                        e.read(cx).key.as_deref() == Some(k.as_str())
                                    })
                                })
                                .cloned()
                        })
                    });
                    match existing {
                        Some(w) => {
                            if let Ok(html) = std::fs::read_to_string(&r.path) {
                                let _ = w.update(cx, |v, cx| {
                                    v.apply_live(
                                        crate::static_html::staticize_if_needed(html),
                                        Instant::now(),
                                        cx,
                                    )
                                });
                            }
                        }
                        None => open(r, cx),
                    }
                }
                let views: Vec<_> = VIEWS.with(|v| {
                    v.borrow_mut().retain(|w| w.upgrade().is_some());
                    v.borrow().clone()
                });
                let mut live = live;
                let mut keys = std::collections::HashSet::new();
                for w in &views {
                    let _ = w.update(cx, |view, cx| {
                        view.tick(cx);
                        if let Some(k) = view.key.clone() {
                            if let Some(i) = live.iter().position(|(lk, _, _)| *lk == k) {
                                let (_, produce, t) = live.swap_remove(i);
                                view.produce_live(produce, t, cx);
                            }
                            keys.insert(k);
                        }
                    });
                }
                LIVE_KEYS.with(|k| *k.borrow_mut() = keys);
            });
        }
    })
    .detach();
}

/// Render scale for the Blitz bitmaps (device px per CSS px).
const BLITZ_SCALE: f32 = 2.0;
const ZOOM_STEPS: [f32; 9] = [0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 2.0];

#[derive(Clone, Copy, PartialEq)]
enum PdfState {
    Loading,
    Generating(Instant),
    Ready,
    Failed,
}

struct PreviewView {
    key: Option<String>,
    /// Head + per-sheet chunks of the document currently shown.
    segments: Vec<String>,
    rendering: bool,
    worker: Arc<crate::blitz_render::Worker>,
    bar: gpui::Entity<TitleBar>,
    print_pending: bool,
    live_seq: u64,
    applied_seq: u64,
    fields: Vec<(String, String)>,
    /// Newest live document that arrived while a render was in flight.
    queued: Option<(String, Instant)>,
    /// Static (script-free) HTML both engines load.
    html_path: PathBuf,
    pdf_path: PathBuf,
    pages: Vec<(Arc<RenderImage>, f32, f32)>,
    zoom: f32,
    /// Hidden WKWebView that only exists to print the static HTML to PDF.
    webview: Option<wry::WebView>,
    loaded: Rc<Cell<bool>>,
    pdf: PdfState,
    pdf_last_size: u64,
    pdf_stale: bool,
    pdf_stable_since: Instant,
    last_live: Instant,
    error: Option<String>,
    status: Option<String>,
    focus: FocusHandle,
    #[cfg(target_os = "macos")]
    ns_view: Option<std::ptr::NonNull<std::ffi::c_void>>,
}

impl PreviewView {
    /// Run the producer (template fill) in the background, then apply.
    fn produce_live(&mut self, produce: Producer, started: Instant, cx: &mut Context<Self>) {
        self.live_seq += 1;
        let seq = self.live_seq;
        cx.spawn(async move |this, cx| {
            let html = cx
                .background_executor()
                .spawn(async move { produce() })
                .await;
            let _ = this.update(cx, |v: &mut PreviewView, cx| {
                // Fills can finish out of order; never apply an older one.
                if seq > v.applied_seq {
                    v.applied_seq = seq;
                    v.apply_live(html, started, cx);
                }
            });
        })
        .detach();
    }

    /// Live sync: re-render only the sheets whose markup changed; the scroll
    /// container keeps its id so the scroll position is preserved.
    fn apply_live(&mut self, html: String, started: Instant, cx: &mut Context<Self>) {
        if self.rendering {
            self.queued = Some((html, started));
            return;
        }
        let segments = crate::blitz_render::segments(&html);
        let mask = crate::blitz_render::dirty_mask(&self.segments, &segments);
        if !mask.iter().any(|d| *d) {
            return;
        }
        let fields = crate::static_html::field_map(&html);
        // Same fields, same markup otherwise -> text patch; else full rebuild.
        let same_shape = fields.len() == self.fields.len()
            && fields.iter().zip(&self.fields).all(|(a, b)| a.0 == b.0)
            && strip_field_text(&segments) == strip_field_text(&self.segments);
        let job = if same_shape {
            let changes: Vec<_> = fields
                .iter()
                .zip(&self.fields)
                .filter(|(a, b)| a.1 != b.1)
                .map(|(a, _)| a.clone())
                .collect();
            crate::blitz_render::Job::Patch(changes, html.clone())
        } else {
            crate::blitz_render::Job::Full(html.clone(), Some(mask))
        };
        self.fields = fields;
        self.segments = segments;
        self.rendering = true;
        // Regenerated by `tick` once edits settle; Print waits for it.
        if std::fs::write(&self.html_path, &html).is_ok() {
            self.pdf_stale = true;
        }
        self.last_live = Instant::now();
        let rx = self.worker.submit(job);
        cx.spawn(async move |this, cx| {
            let rendered = match cx
                .background_executor()
                .spawn(async move { rx.recv() })
                .await
            {
                Ok(r) => r,
                Err(_) => return,
            };
            let fresh: Vec<_> = rendered
                .pages
                .into_iter()
                .map(|p| p.map(to_render_image))
                .collect();
            let _ = this.update(cx, |v, cx| {
                if fresh.len() != v.pages.len() {
                    v.pages = fresh.into_iter().flatten().collect();
                } else {
                    for (slot, p) in v.pages.iter_mut().zip(fresh) {
                        if let Some(p) = p {
                            *slot = p;
                        }
                    }
                }
                v.rendering = false;
                cx.notify();
                if let Some((h, t)) = v.queued.take() {
                    v.apply_live(h, t, cx);
                }
            });
        })
        .detach();
    }

    /// Drives the hidden-WebView → PDF pipeline (polled from `start`).
    fn tick(&mut self, cx: &mut Context<Self>) {
        self.tick_pdf(cx);
        self.sync_bar(cx);
        if self.print_pending && self.pdf == PdfState::Ready && !self.pdf_stale {
            self.print(cx);
        }
    }

    fn tick_pdf(&mut self, cx: &mut Context<Self>) {
        if self.pdf_stale
            && !matches!(self.pdf, PdfState::Generating(_))
            && !self.rendering
            && self.last_live.elapsed() > Duration::from_millis(400)
            && let Some(wv) = &self.webview
        {
            self.pdf_stale = false;
            self.loaded.set(false);
            self.pdf = PdfState::Loading;
            let _ = wv.load_url(&format!("file://{}", self.html_path.display()));
            cx.notify();
        }
        match self.pdf {
            PdfState::Loading if self.loaded.get() => {
                let _ = std::fs::remove_file(&self.pdf_path);
                #[cfg(target_os = "macos")]
                match self
                    .webview
                    .as_ref()
                    .map(|w| mac::generate_pdf(w, &self.pdf_path))
                {
                    Some(Ok(())) => self.pdf = PdfState::Generating(Instant::now()),
                    Some(Err(e)) => {
                        self.pdf = PdfState::Failed;
                        self.status = Some(format!("PDF failed: {e}"));
                    }
                    None => {
                        self.pdf = PdfState::Failed;
                    }
                }
                cx.notify();
            }
            PdfState::Generating(t) => {
                let size = std::fs::metadata(&self.pdf_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                // The save runs asynchronously; done once the file stops growing.
                if size != self.pdf_last_size {
                    self.pdf_stable_since = Instant::now();
                }
                let settled =
                    size > 0 && self.pdf_stable_since.elapsed() > Duration::from_millis(300);
                #[cfg(target_os = "macos")]
                let info = if settled {
                    mac::pdf_info(&self.pdf_path)
                } else {
                    None
                };
                #[cfg(not(target_os = "macos"))]
                let info: Option<(usize, f64, f64)> = settled.then_some((0, 0.0, 0.0));
                // Ready only once PDFKit parses it (a half-written file doesn't).
                if info.is_some_and(|(n, _, _)| n > 0 || cfg!(not(target_os = "macos"))) {
                    self.pdf = PdfState::Ready;
                    cx.notify();
                } else if t.elapsed() > Duration::from_secs(30) {
                    self.pdf = PdfState::Failed;
                    self.status = Some("PDF generation timed out".into());
                    cx.notify();
                }
                self.pdf_last_size = size;
            }
            _ => {}
        }
    }

    fn print(&mut self, cx: &mut Context<Self>) {
        if self.pdf != PdfState::Ready || self.pdf_stale {
            // Print stays enabled: remember the click, print once fresh.
            self.print_pending = self.pdf != PdfState::Failed || self.pdf_stale;
            self.sync_bar(cx);
            return;
        }
        self.print_pending = false;
        #[cfg(target_os = "macos")]
        {
            self.status = self
                .webview
                .as_ref()
                .ok_or_else(|| "no window".to_string())
                .and_then(|w| mac::print_pdf(w, &self.pdf_path))
                .err()
                .map(|e| format!("Print failed: {e}"));
        }
        cx.notify();
    }

    fn share(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let target = if self.pdf == PdfState::Ready && !self.pdf_stale {
            self.pdf_path.clone()
        } else {
            self.html_path.clone()
        };
        #[cfg(target_os = "macos")]
        {
            self.status = macos_share(self.ns_view, &target, _window).err();
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.status = Some(format!("Saved at {}", target.display()));
        }
        cx.notify();
    }

    fn zoom_by(&mut self, dir: i32, cx: &mut Context<Self>) {
        let i = ZOOM_STEPS
            .iter()
            .position(|z| (*z - self.zoom).abs() < 0.01)
            .unwrap_or(4) as i32;
        self.set_zoom(
            ZOOM_STEPS[(i + dir).clamp(0, ZOOM_STEPS.len() as i32 - 1) as usize],
            cx,
        );
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom;
        self.sync_bar(cx);
        cx.notify();
    }

    /// Push PDF/zoom state to the title bar; it re-renders only on change.
    fn sync_bar(&mut self, cx: &mut Context<Self>) {
        let pdf = match self.pdf {
            PdfState::Failed if !self.pdf_stale => BarPdf::Failed,
            PdfState::Ready if !self.pdf_stale => BarPdf::Ready,
            _ => BarPdf::Busy,
        };
        let (zoom, pending) = (self.zoom, self.print_pending);
        self.bar.update(cx, |bar, cx| {
            if bar.pdf != pdf || bar.zoom != zoom || bar.print_pending != pending {
                bar.pdf = pdf;
                bar.zoom = zoom;
                bar.print_pending = pending;
                cx.notify();
            }
        });
    }
}

#[cfg(target_os = "macos")]
mod mac {
    //! WebKit vector PDF + PDFKit printing for the split preview.
    use std::path::Path;

    use objc2::AllocAnyThread;
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSPaperOrientation, NSPrintInfo, NSPrintJobSavingURL, NSPrintSaveJob, NSView,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument, PDFPrintScalingMode};
    use wry::WebViewExtMacOS;

    use super::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};

    fn file_url(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string()))
    }

    fn a4_info() -> Retained<NSPrintInfo> {
        let info = NSPrintInfo::new();
        info.setPaperSize(NSSize::new(PAPER_WIDTH_PT as f64, PAPER_HEIGHT_PT as f64));
        info.setOrientation(NSPaperOrientation::Portrait);
        info.setTopMargin(0.0);
        info.setBottomMargin(0.0);
        info.setLeftMargin(0.0);
        info.setRightMargin(0.0);
        info
    }

    /// Print the hidden WKWebView's page to `out` as a paginated vector PDF
    /// (WebKit's own print layout: `@page` A4, `.sheet` page breaks).
    pub fn generate_pdf(webview: &wry::WebView, out: &Path) -> Result<(), String> {
        let _mtm = MainThreadMarker::new().ok_or("not on main thread")?;
        let wk = webview.webview();
        let view: &NSView = &wk;
        let window = view.window().ok_or("hidden webview has no window")?;
        let info = a4_info();
        unsafe {
            info.setJobDisposition(NSPrintSaveJob);
            info.dictionary()
                .setValue_forKey(Some(&file_url(out)), NSPrintJobSavingURL);
        }
        let op = unsafe { wk.printOperationWithPrintInfo(&info) };
        op.setShowsPrintPanel(false);
        op.setShowsProgressPanel(false);
        if let Some(v) = op.view() {
            v.setFrame(NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PAPER_WIDTH_PT as f64, PAPER_HEIGHT_PT as f64),
            ));
        }
        // WKWebView only paginates correctly through the window-modal path.
        unsafe {
            op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                &window,
                None,
                None,
                std::ptr::null_mut(),
            )
        };
        Ok(())
    }

    /// (page count, first page media box width, height) in points.
    pub fn pdf_info(path: &Path) -> Option<(usize, f64, f64)> {
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &file_url(path))? };
        let n = unsafe { doc.pageCount() };
        let page = unsafe { doc.pageAtIndex(0)? };
        let r = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) };
        Some((n, r.size.width, r.size.height))
    }

    /// Native print panel for the generated PDF, A4, no scaling.
    pub fn print_pdf(webview: &wry::WebView, path: &Path) -> Result<(), String> {
        let wk = webview.webview();
        let view: &NSView = &wk;
        let window = view.window().ok_or("no window")?;
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &file_url(path)) }
            .ok_or("cannot open PDF")?;
        let info = a4_info();
        let op = unsafe {
            doc.printOperationForPrintInfo_scalingMode_autoRotate(
                Some(&info),
                PDFPrintScalingMode::PageScaleNone,
                false,
                MainThreadMarker::new().ok_or("not on main thread")?,
            )
        }
        .ok_or("PDFKit gave no print operation")?;
        op.setShowsPrintPanel(true);
        unsafe {
            op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                &window,
                None,
                None,
                std::ptr::null_mut(),
            )
        };
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn macos_share(
    ns_view: Option<std::ptr::NonNull<std::ffi::c_void>>,
    path: &std::path::Path,
    window: &Window,
) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSSharingServicePicker, NSView};
    use objc2_foundation::{NSArray, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL};

    let ns_view = ns_view.ok_or("no native view")?;
    let mtm = MainThreadMarker::new().ok_or("not on main thread")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string()));
    let item: Retained<AnyObject> = Retained::into_super(Retained::into_super(url));
    let items = NSArray::from_retained_slice(&[item]);
    let picker = unsafe { NSSharingServicePicker::initWithItems(mtm.alloc(), &items) };
    let view: &NSView = unsafe { ns_view.cast::<NSView>().as_ref() };
    let w = f32::from(window.viewport_size().width) as f64;
    let h = f32::from(window.viewport_size().height) as f64;
    let rect = NSRect::new(
        NSPoint::new(w - 150.0, h - TITLE_BAR_H as f64),
        NSSize::new(60.0, TITLE_BAR_H as f64),
    );
    picker.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MinY);
    std::mem::forget(picker);
    Ok(())
}

fn to_render_image(p: crate::blitz_render::Page) -> (Arc<RenderImage>, f32, f32) {
    // Already BGRA (swapped on the worker).
    let rgba = image::RgbaImage::from_raw(p.width, p.height, p.rgba).expect("buffer size matches");
    let img = RenderImage::new(smallvec::SmallVec::from_const([image::Frame::new(rgba)]));
    (Arc::new(img), p.css_w, p.css_h)
}

impl Render for PreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = gpui_base::Theme::global(cx).tokens.colors;
        let mut pages = div()
            .id("pages")
            .flex_1()
            .overflow_scroll()
            .bg(gpui::rgb(0x5f6368))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(24.0))
            .py(px(24.0));
        if let Some(e) = self.error.as_ref().or(self.status.as_ref()) {
            pages = pages.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .bg(colors.background)
                    .text_color(colors.foreground)
                    .child(e.clone()),
            );
        }
        if self.pages.is_empty() && self.error.is_none() {
            pages = pages.child(div().text_color(gpui::white()).child("Rendering\u{2026}"));
        }
        let n = self.pages.len();
        for (i, (image, w, h)) in self.pages.iter().enumerate() {
            pages = pages.child(
                div()
                    .flex_none()
                    .relative()
                    .shadow_lg()
                    .bg(gpui::white())
                    .w(px(w * self.zoom))
                    .h(px(h * self.zoom))
                    .child(img(ImageSource::Render(image.clone())).size_full())
                    .child(
                        div()
                            .absolute()
                            .right(px(8.0))
                            .bottom(px(4.0))
                            .text_size(px(9.0))
                            .text_color(gpui::rgb(0x888888))
                            .child(format!("Page {} of {}", i + 1, n)),
                    ),
            );
        }
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let k = &e.keystroke;
                if k.modifiers.platform || k.modifiers.control {
                    match k.key.as_str() {
                        "p" => this.print(cx),
                        "=" | "+" => this.zoom_by(1, cx),
                        "-" => this.zoom_by(-1, cx),
                        "0" => this.set_zoom(1.0, cx),
                        _ => {}
                    }
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .child(
                gpui::AnyView::from(self.bar.clone()).cached(
                    gpui::StyleRefinement::default()
                        .w_full()
                        .h(px(TITLE_BAR_H))
                        .flex_none(),
                ),
            )
            .child(pages)
    }
}

/// Make sure we hold the static variant: files produced by the old path (or
/// `BIR_PREVIEW_HTML`) still carry scripts/inputs; convert them on the fly.
fn static_file(path: &std::path::Path) -> std::io::Result<(PathBuf, String)> {
    let html = std::fs::read_to_string(path)?;
    if !html.contains("<script") && !html.contains("<input") {
        return Ok((path.to_path_buf(), html));
    }
    let html = crate::static_html::staticize(&html);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preview");
    let out = std::env::temp_dir().join(format!("{stem}.static.html"));
    std::fs::write(&out, &html)?;
    Ok((out, html))
}

fn open(r: Request, cx: &mut App) {
    let opened = Instant::now();
    let (html_path, html) = match static_file(&r.path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("preview: cannot read {}: {e}", r.path.display());
            return;
        }
    };
    let pdf_path = html_path.with_extension("pdf");
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(980.0), px(1100.0)),
            cx,
        ))),
        app_id: Some("com.buwiz.bir".to_owned()),
        titlebar: Some(TitlebarOptions {
            title: Some(r.title.clone().into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.0), px(14.0))),
        }),
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    };
    let _ = cx.open_window(options, move |window, cx| {
        let loaded = Rc::new(Cell::new(false));
        let flag = loaded.clone();
        let url = format!("file://{}", html_path.display());
        // Hidden: never shown, only printed to PDF.
        let built = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_visible(false)
            .with_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                size: wry::dpi::LogicalSize::new(PAPER_WIDTH_PT as f64, PAPER_HEIGHT_PT as f64)
                    .into(),
            })
            .with_on_page_load_handler(move |ev, _| {
                if matches!(ev, wry::PageLoadEvent::Finished) {
                    flag.set(true);
                }
            })
            .build_as_child(&*window);
        #[cfg(target_os = "macos")]
        let ns_view = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match window.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::AppKit(h)) => Some(h.ns_view),
                _ => None,
            }
        };
        let view = cx.new(|cx| {
            let (webview, status) = match built {
                Ok(wv) => (Some(wv), None),
                Err(e) => (None, Some(format!("PDF engine failed: {e}"))),
            };
            cx.observe_global::<gpui_base::Theme>(|_: &mut PreviewView, cx| cx.notify())
                .detach();
            let owner = cx.entity().downgrade();
            let bar_title: SharedString = r.title.clone().into();
            let bar = cx.new(|cx| {
                cx.observe_global::<gpui_base::Theme>(|_: &mut TitleBar, cx| cx.notify())
                    .detach();
                TitleBar {
                    title: bar_title,
                    pdf: BarPdf::Busy,
                    zoom: 1.0,
                    print_pending: false,
                    owner,
                }
            });
            let segments = crate::blitz_render::segments(&html);
            let fields = crate::static_html::field_map(&html);
            let worker = Arc::new(crate::blitz_render::Worker::spawn(BLITZ_SCALE));
            let rx = worker.submit(crate::blitz_render::Job::Full(html, None));
            // Blitz layout + raster on the preview's worker thread.
            cx.spawn(async move |this, cx| {
                let Ok(rendered) = cx
                    .background_executor()
                    .spawn(async move { rx.recv() })
                    .await
                else {
                    return;
                };
                let pages: Vec<_> = rendered
                    .pages
                    .into_iter()
                    .flatten()
                    .map(to_render_image)
                    .collect();
                let _ = this.update(cx, |v: &mut PreviewView, cx| {
                    if pages.is_empty() {
                        v.error = Some("Blitz produced no pages".into());
                    }
                    v.pages = pages;
                    v.rendering = false;
                    cx.notify();
                    if let Some((h, t)) = v.queued.take() {
                        v.apply_live(h, t, cx);
                    }
                });
            })
            .detach();
            PreviewView {
                key: r.key,
                segments,
                rendering: true,
                worker,
                bar: bar.clone(),
                print_pending: false,
                live_seq: 0,
                applied_seq: 0,
                fields,
                queued: None,
                html_path,
                pdf_path,
                pages: Vec::new(),
                zoom: 1.0,
                webview,
                loaded,
                pdf: PdfState::Loading,
                pdf_last_size: 0,
                pdf_stale: false,
                pdf_stable_since: opened,
                last_live: opened,
                error: None,
                status,
                focus: cx.focus_handle(),
                #[cfg(target_os = "macos")]
                ns_view,
            }
        });
        view.read(cx).focus.clone().focus(window, cx);
        VIEWS.with(|v| v.borrow_mut().push(view.downgrade()));
        view
    });
}

/// Markup with field texts blanked, to tell "only values changed" apart.
fn strip_field_text(segments: &[String]) -> Vec<String> {
    segments
        .iter()
        .map(|seg| {
            let mut out = String::with_capacity(seg.len());
            let mut rest = seg.as_str();
            while let Some(i) = rest.find(" fs\"") {
                let Some(gt) = rest[i..].find('>') else {
                    break;
                };
                // Only identified spans participate in field_map/Patch.
                // An overflow line has no id: retain its text in the shape
                // so changing it forces a full rebuild instead of an empty patch.
                let patchable = rest[i..i + gt].contains(" id=\"");
                out.push_str(&rest[..i + gt + 1]);
                rest = &rest[i + gt + 1..];
                if !patchable {
                    continue;
                }
                match rest.find("</span>") {
                    Some(c) => rest = &rest[c..],
                    None => break,
                }
            }
            out.push_str(rest);
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::strip_field_text;

    #[test]
    fn real_2000ot_email_overflow_changes_the_render_shape() {
        let filled = |email: &str| {
            let fields =
                std::collections::BTreeMap::from([("txtEmail".to_string(), email.to_string())]);
            let html = bir_print::frozen_html::filled_document("2000-ot-2018", &fields).unwrap();
            crate::static_html::staticize(&crate::print_sheet::prepare_html(&html))
        };
        let old = filled("Andrea.Mae.Galang.Accounting@Example.ph");
        let new = filled("Andrea.Mae.Galang.Accounting@Example.co");
        assert!(new.contains("Andrea.Mae.Galang.Accounting@Example.co"));
        assert_eq!(
            crate::static_html::field_map(&old),
            crate::static_html::field_map(&new)
        );
        assert_ne!(strip_field_text(&[old]), strip_field_text(&[new]));
    }

    #[test]
    fn changed_overflow_text_requires_a_full_render() {
        let filled = |text: &str| {
            crate::static_html::staticize(&format!(
                "<html><head></head><body><div class=\"page\"><input class=\"fi\" readonly value=\"{text}\" style=\"font-size:9.00pt\"><input class=\"fi fc\" id=\"p1c1-s0\" value=\"\"></div></body></html>"
            ))
        };
        let old = filled("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@example.com");
        let new = filled("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb@example.com");
        assert_eq!(
            crate::static_html::field_map(&old),
            crate::static_html::field_map(&new)
        );
        assert_ne!(strip_field_text(&[old]), strip_field_text(&[new]));
    }

    #[test]
    fn identified_text_changes_can_still_use_incremental_patches() {
        let filled = |text: &str| {
            crate::static_html::staticize(&format!(
                "<html><head></head><body><div class=\"page\"><input class=\"fi\" id=\"p1c1-i\" value=\"{text}\"></div></body></html>"
            ))
        };
        let old = filled("OLD NAME");
        let new = filled("NEW NAME");
        assert_ne!(
            crate::static_html::field_map(&old),
            crate::static_html::field_map(&new)
        );
        assert_eq!(strip_field_text(&[old]), strip_field_text(&[new]));
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BarPdf {
    Busy,
    Ready,
    Failed,
}

/// The preview's title bar as its own (cached) view: page re-renders and live
/// sync never touch it; it re-renders only when its own state changes, and
/// every state renders in the same footprint (fixed-width buttons, icons
/// always drawn, title centred independently).
struct TitleBar {
    title: SharedString,
    pdf: BarPdf,
    zoom: f32,
    print_pending: bool,
    owner: WeakEntity<PreviewView>,
}

const PRINT_W: f32 = 96.0;
const SHARE_W: f32 = 78.0;
const ZOOM_W: f32 = 52.0;
/// Right inset + zoom (26+52+26) + spacer 8 + Share + Print + 5 gaps of 6.
const BUTTONS_W: f32 = 12.0 + 26.0 + ZOOM_W + 26.0 + 8.0 + SHARE_W + PRINT_W + 30.0 + 8.0;

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::svg;
        let colors = gpui_base::Theme::global(cx).tokens.colors;
        let owner = self.owner.clone();
        let act = move |f: fn(&mut PreviewView, &mut Window, &mut Context<PreviewView>)| {
            let owner = owner.clone();
            move |_: &gpui::MouseDownEvent, window: &mut Window, cx: &mut App| {
                let _ = owner.update(cx, |v, cx| f(v, window, cx));
            }
        };
        let button = |id: &'static str, w: f32| {
            div()
                .id(id)
                .w(px(w))
                .h(px(24.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(colors.border)
                .bg(colors.secondary)
                .text_color(colors.secondary_foreground)
                .text_size(px(12.0))
                .cursor_pointer()
                .hover(move |s| s.bg(colors.accent).text_color(colors.accent_foreground))
        };
        let icon = |path: &'static str| {
            svg()
                .path(path)
                .size(px(14.0))
                .flex_none()
                .text_color(colors.secondary_foreground)
        };
        let busy = self.pdf == BarPdf::Busy;
        // Printer icon always drawn; while the PDF refreshes it dims and a
        // small spinner sits on its corner (absolute: no layout change).
        let print_icon = div()
            .relative()
            .size(px(14.0))
            .flex_none()
            .child(icon("icons/printer.svg").opacity(if busy { 0.45 } else { 1.0 }))
            .when_some_busy(busy, colors.secondary_foreground)
            .child(
                div()
                    .absolute()
                    .right(px(-4.0))
                    .bottom(px(-4.0))
                    .size(px(8.0))
                    .rounded(px(4.0))
                    .bg(gpui::rgb(0xd93025))
                    .opacity(if self.pdf == BarPdf::Failed { 1.0 } else { 0.0 }),
            );
        let print = button("print", PRINT_W)
            .child(print_icon)
            .child(
                div()
                    .opacity(if self.print_pending { 0.6 } else { 1.0 })
                    .child("Print \u{2318}P"),
            )
            .on_mouse_down(MouseButton::Left, act(|v, _, cx| v.print(cx)));
        let share = button("share", SHARE_W)
            .child(icon("icons/upload.svg"))
            .child("Share")
            .on_mouse_down(MouseButton::Left, act(|v, w, cx| v.share(w, cx)));
        let zoom_btn = |id: &'static str, label: SharedString| button(id, 26.0).child(label);
        div()
            .relative()
            .size_full()
            .bg(colors.surface)
            .border_b_1()
            .border_color(colors.border)
            // Title centred on the whole bar, independent of the buttons.
            .child(
                // Symmetric insets = button group width, so the title stays
                // centred on the bar and never slides under the buttons.
                div()
                    .absolute()
                    .left(px(BUTTONS_W))
                    .right(px(BUTTONS_W))
                    .top(px(0.0))
                    .h(px(TITLE_BAR_H))
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(13.0))
                            .text_color(colors.surface_foreground)
                            .child(self.title.clone()),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.0))
                    .top(px(0.0))
                    .h(px(TITLE_BAR_H))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        zoom_btn("zoom-out", "\u{2212}".into())
                            .on_mouse_down(MouseButton::Left, act(|v, _, cx| v.zoom_by(-1, cx))),
                    )
                    .child(
                        button("zoom-reset", ZOOM_W)
                            .child(format!("{:.0}%", self.zoom * 100.0))
                            .on_mouse_down(MouseButton::Left, act(|v, _, cx| v.set_zoom(1.0, cx))),
                    )
                    .child(
                        zoom_btn("zoom-in", "+".into())
                            .on_mouse_down(MouseButton::Left, act(|v, _, cx| v.zoom_by(1, cx))),
                    )
                    .child(div().w(px(8.0)).flex_none())
                    .child(share)
                    .child(print),
            )
    }
}

trait BusySpinner: Sized {
    fn when_some_busy(self, busy: bool, color: gpui::Hsla) -> Self;
}
impl BusySpinner for gpui::Div {
    /// Small spinning loader over the icon's corner while busy (always in the
    /// tree so the bar's layout never changes; invisible when idle).
    fn when_some_busy(self, busy: bool, color: gpui::Hsla) -> Self {
        use gpui::{Animation, AnimationExt as _, Transformation, percentage, svg};
        let spinner = svg()
            .path("icons/loader.svg")
            .size(px(10.0))
            .text_color(color)
            .opacity(if busy { 1.0 } else { 0.0 });
        let el = div()
            .absolute()
            .right(px(-5.0))
            .top(px(-5.0))
            .size(px(10.0));
        if busy {
            self.child(el.child(spinner.with_animation(
                "pdf-spin",
                Animation::new(Duration::from_millis(900)).repeat(),
                |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
            )))
        } else {
            self.child(el.child(spinner))
        }
    }
}
