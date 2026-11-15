//! Print preview, the desktop glue (track D3).
//!
//! NSH has no print API, so this file is ours. What the host functions ask (`print_preview_request`,
//! `print_preview_has_live`, `print_preview_live_update`: see `hooks.rs`, which only delegates here) and what the
//! preview window (TSX, `screens/print/`) needs are answered in four parts:
//!
//! 1. **The document.** The filled sheet arrives as static HTML (bir-app-api's `print_sheet` + `static_html`: one A4
//!    paper per form page, script-free). It is copied into a folder of its own, one per form code, because the
//!    macOS web view may read a file URL's own folder and nothing above it. Edits arrive as a producer; each
//!    answer rewrites the file (atomically) and tells the window what changed.
//! 2. **The live update.** The window is told by a script it runs in its page: the text of every field box, by
//!    position, when the sheet has the same shape as the one shown, else the whole body. The page is never
//!    reloaded, so the scroll position stays. A host function is a synchronous call of the engine (no task, no
//!    reactive runtime), so the fill runs inside it and the events are posted to the TSX listener.
//! 3. **The PDF.** A hidden WKWebView loads the same HTML and `createPDF` captures each `.sheet` as one A4 page;
//!    bir-print normalizes the pages and merges them (the reference app's way, see `internal/DECISIONS.md` D24).
//!    Print and Save as PDF both start from these bytes, so the file has the pages the printer gets.
//! 4. **Print.** The system print panel for that PDF (PDFKit's print operation), A4, no scaling, as a sheet of
//!    the key window. Save as PDF uses the host save panel.
//!
//! **The page rule** (kept from the old shell): paper is always A4, 595.28 x 841.89 pt, zero margins. A sheet
//! whose own `@page` size is another (the frozen BIR forms are Folio, 612 x 936 pt) is scaled by
//! min((595.28 - 20) / w, (841.89 - 20) / h, 1) with 10 pt of safety above it: down to fit, never up. One printed
//! page per form page (`.sheet`).
//!
//! macOS only, for now: other systems answer "not available" honestly (D7 brings Windows and Linux).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use bir_app_api::hooks::PreviewProducer;
use bir_app_api::print_sheet::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};
use bir_print::html_output::{PdfExpectation, merge_single_page_pdfs, normalize_wkpdf_page_from_css_pixels};
use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::prelude::{Callback, JsObject, PropEnum, Stop, blocking, module, pick_save_path, try_os};

/// The sentence of a device that cannot open a second window.
pub const UNAVAILABLE: &str = "Print preview is not available on this device.";

/// The name of the sheet inside a preview's folder.
const SHEET: &str = "sheet.html";

/// What happened to a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PropEnum)]
pub enum PrintEventKind {
    /// A preview was asked for: open its window, or focus the one that is open (the title may have changed).
    Open,
    /// The open preview has a newer document: run the event's script in its page.
    Update,
    /// This device cannot open a preview window: say the event's message.
    Unavailable,
}

/// One thing the preview host (TSX) hears from the Rust side.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct PrintEvent {
    /// What happened.
    pub kind: PrintEventKind,
    /// The form code the preview belongs to; one window per key.
    pub key: String,
    /// The window title the host function chose ("BIR Form 2551Q - Juana Santos").
    pub title: String,
    /// The file URL of the sheet: what the window's web view loads.
    pub url: String,
    /// A script that brings the open page to the newest document (`update`); empty otherwise.
    pub script: String,
    /// How many documents this preview has shown; grows with every change.
    pub revision: u32,
    /// How many printed pages the document has.
    pub pages: u32,
    /// The paper it prints on ("A4").
    pub paper: String,
    /// The size the form page itself is laid out in ("Folio"), before it is scaled onto the paper.
    pub sheet: String,
    /// The scale that fits the form page onto the paper (1 when it already fits).
    pub zoom: f64,
    /// A sentence for the person (`unavailable`).
    pub message: Option<String>,
}

// ------------------------------------------------------------------------------------------------ the page rule

/// What the printed document is made of.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageInfo {
    /// The paper's name ("A4").
    pub paper: String,
    /// The name of the form page's own size ("Folio").
    pub sheet: String,
    /// The scale of the form page on the paper.
    pub zoom: f64,
    /// Printed pages (one per `.sheet`).
    pub pages: u32,
}

/// The size an `@page { size: Wpt Hpt }` rule gives, the first one in `css`.
fn page_size_in(css: &str) -> Option<(f32, f32)> {
    let rest = &css[css.find("@page")?..];
    let rest = &rest[rest.find("size:")? + 5..];
    let end = rest.find([';', '}'])?;
    let mut parts = rest[..end].split_whitespace();
    let w = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    let h = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    Some((w, h))
}

/// The name of a paper or sheet size in points (1 pt of slack).
fn size_name(width: f32, height: f32) -> &'static str {
    let is = |w: f32, h: f32| (width - w).abs() < 1.0 && (height - h).abs() < 1.0;
    if is(PAPER_WIDTH_PT, PAPER_HEIGHT_PT) {
        "A4"
    } else if is(612.0, 936.0) {
        "Folio"
    } else if is(612.0, 792.0) {
        "Letter"
    } else if is(612.0, 1008.0) {
        "Legal"
    } else {
        "Custom"
    }
}

/// The paper, the form page's own size and the scale of one on the other, as the document says them: the
/// sheet's own `@page` is the first rule of the page, the paper is the rule `print_sheet::prepare_html` added
/// (`<style id="preview-paper">`), whose `zoom` is the scale. A document with no such style prints on A4 at scale 1.
pub(crate) fn page_info(html: &str) -> PageInfo {
    let marker = html.find("id=\"preview-paper\"");
    let (head, paper_css) = match marker {
        Some(at) => (&html[..at], &html[at..]),
        None => (html, ""),
    };
    let paper = page_size_in(paper_css).unwrap_or((PAPER_WIDTH_PT, PAPER_HEIGHT_PT));
    let native = page_size_in(head).unwrap_or(paper);
    let zoom = paper_css
        .find(".sheet>.page{zoom:")
        .and_then(|at| {
            let rest = &paper_css[at + ".sheet>.page{zoom:".len()..];
            rest[..rest.find([';', '}'])?].trim().parse::<f64>().ok()
        })
        .unwrap_or(1.0);
    PageInfo {
        paper: size_name(paper.0, paper.1).to_owned(),
        sheet: size_name(native.0, native.1).to_owned(),
        zoom,
        pages: html.matches("<div class=\"sheet").count() as u32,
    }
}

// ------------------------------------------------------------------------------------------------ the document

/// A document taken apart for live updates.
#[derive(Debug, Clone, PartialEq)]
struct Doc {
    /// What is between `<body>` and `</body>`.
    body: String,
    /// The text of every field box (`span.fs`), in order.
    texts: Vec<String>,
    /// The body with those texts taken out: two documents of one shape differ only in their texts.
    shape: String,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    let end = tag[at..].find('"')?;
    Some(&tag[at..at + end])
}

fn parse_doc(html: &str) -> Doc {
    let start = html
        .find("<body")
        .and_then(|at| html[at..].find('>').map(|end| at + end + 1))
        .unwrap_or(0);
    let end = html.rfind("</body>").unwrap_or(html.len()).max(start);
    let body = &html[start..end];
    let (mut texts, mut shape, mut at) = (Vec::new(), String::with_capacity(body.len()), 0);
    while let Some(found) = body[at..].find("<span ") {
        let open = at + found;
        let Some(close) = body[open..].find('>') else { break };
        let tag = &body[open..open + close + 1];
        let content = open + close + 1;
        shape.push_str(&body[at..content]);
        at = content;
        if attr(tag, "class").is_some_and(|class| class.split_whitespace().any(|c| c == "fs")) {
            let Some(span_end) = body[content..].find("</span>") else { break };
            texts.push(body[content..content + span_end].to_owned());
            at = content + span_end;
        }
    }
    shape.push_str(&body[at..]);
    Doc { body: body.to_owned(), texts, shape }
}

/// Whether `html` is a document with printed pages (a failed fill is empty).
fn has_sheets(html: &str) -> bool {
    html.contains("<div class=\"sheet")
}

/// The script that sets every field box of the open page to `texts` (the page must have as many), then marks the revision.
fn patch_script(texts: &[String], revision: u32) -> String {
    let texts = serde_json::to_string(texts).unwrap_or_else(|_| "[]".to_owned());
    format!(
        "(function(t,r){{var n=document.querySelectorAll(\"span.fs\");if(n.length!==t.length)return \"shape\";\
for(var i=0;i<t.length;i++){{if(n[i].innerHTML!==t[i])n[i].innerHTML=t[i];}}\
document.documentElement.setAttribute(\"data-rev\",String(r));return \"patch\";}})({texts},{revision});"
    )
}

/// The script that replaces the open page's body (a sheet of another shape: a receipt page grew or shrank).
fn body_script(body: &str, revision: u32) -> String {
    let body = serde_json::to_string(body).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        "(function(h,r){{document.body.innerHTML=h;document.documentElement.setAttribute(\"data-rev\",String(r));\
return \"body\";}})({body},{revision});"
    )
}

/// Whether a process with this id is running.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// Where a process cannot be asked here (Windows: no OpenProcess binding in this crate) every one counts as running, so
/// [`Print::listen`] does not sweep at all there: a stale folder stays until the system cleans its temp folder. (Nothing
/// outside the tests calls it there, so it may stand unused.)
#[cfg(not(unix))]
#[cfg_attr(not(test), allow(dead_code))]
fn process_alive(_pid: u32) -> bool {
    true
}

/// Deletes the `ebirforms-print-<pid>` folders in `parent` of processes that are gone (a quit with a preview open leaves
/// its folder: the window's close event may not run). A folder of a running process, and anything else, stays.
#[cfg_attr(not(any(test, unix)), allow(dead_code))]
fn sweep_stale(parent: &Path, alive: impl Fn(u32) -> bool) {
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.strip_prefix("ebirforms-print-").and_then(|pid| pid.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !alive(pid) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// `path` as a file URL (everything but unreserved characters and `/` is percent-encoded).
fn url_for(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => url.push(byte as char),
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

/// A folder name for a form code.
fn sanitize(key: &str) -> String {
    let name: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if name.is_empty() { "preview".to_owned() } else { name }
}

/// A file name for the sheet: `BIR Form 1701Q - Juana Santos` for "BIR Form 1701Q - Juana Santos" with an em dash.
fn file_stem(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| if matches!(c, '/' | ':' | '\\' | '\u{2014}') { '-' } else { c })
        .collect();
    let stem = stem.trim();
    if stem.is_empty() { "Print preview".to_owned() } else { stem.to_owned() }
}

/// Makes `dir` (and any folder above it that is missing) readable by this user only (0700 where modes exist). A folder that
/// is there already must be a real folder of this user (not a link someone left at the predictable name); it is made 0700 too.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(dir)?;
        let meta = std::fs::symlink_metadata(dir)?;
        // SAFETY: geteuid only reads the process's own id.
        if !meta.file_type().is_dir() || meta.uid() != unsafe { libc::geteuid() } {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("{} is not a folder of this user", dir.display())));
        }
        if meta.mode() & 0o777 != 0o700 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    builder.create(dir)
}

/// Writes `bytes` to a new file at `path` that only this user can read (0600 where modes exist), replacing one there.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let _ = std::fs::remove_file(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

/// Writes `text` to `path` so that a reader sees the old file or the new one, never half of it. The folder and the file are
/// this user's only (a filled return is in them).
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    private_dir(dir)?;
    let temp = path.with_extension("tmp");
    write_private(&temp, text.as_bytes())?;
    std::fs::rename(&temp, path)
}

/// Writes the PDF of a preview into its folder (`<title>.pdf`, this user's only) and answers its path.
fn write_pdf(dir: &Path, title: &str, pdf: &[u8]) -> std::io::Result<PathBuf> {
    private_dir(dir)?;
    let path = dir.join(format!("{}.pdf", file_stem(title)));
    write_private(&path, pdf)?;
    Ok(path)
}

// ------------------------------------------------------------------------------------------------ the previews

/// One open preview.
struct Preview {
    dir: PathBuf,
    title: String,
    /// The document the window shows now (the file holds the same).
    html: String,
    texts: Vec<String>,
    shape: String,
    revision: u32,
}

/// Where events go: the TSX listener in the app, a recorder in tests.
trait Backend {
    fn emit(&self, event: PrintEvent);
}

struct State {
    root: PathBuf,
    backend: Option<(u64, Rc<dyn Backend>)>,
    previews: HashMap<String, Preview>,
}

fn event(kind: PrintEventKind, key: &str, title: &str, url: &str, script: String, revision: u32, html: &str) -> PrintEvent {
    let info = page_info(html);
    PrintEvent {
        kind,
        key: key.to_owned(),
        title: title.to_owned(),
        url: url.to_owned(),
        script,
        revision,
        pages: info.pages,
        paper: info.paper,
        sheet: info.sheet,
        zoom: info.zoom,
        message: None,
    }
}

impl State {
    fn new(root: PathBuf) -> Self {
        State { root, backend: None, previews: HashMap::new() }
    }

    fn sheet_path(&self, key: &str) -> PathBuf {
        self.root.join(sanitize(key)).join(SHEET)
    }

    /// A preview was asked for: the events the host hears. A key already open gets a newer document in place.
    fn request(&mut self, key: &str, title: String, html: String) -> Result<Vec<PrintEvent>, String> {
        let path = self.sheet_path(key);
        private_dir(&self.root).map_err(|error| format!("Could not write the preview: {error}"))?;
        write_atomic(&path, &html).map_err(|error| format!("Could not write the preview: {error}"))?;
        let url = url_for(&path);
        let doc = parse_doc(&html);
        let mut events = Vec::new();
        match self.previews.get_mut(key) {
            None => {
                events.push(event(PrintEventKind::Open, key, &title, &url, String::new(), 1, &html));
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                self.previews.insert(
                    key.to_owned(),
                    Preview {
                        dir,
                        title,
                        html,
                        texts: doc.texts,
                        shape: doc.shape,
                        revision: 1,
                    },
                );
            }
            Some(open) => {
                open.revision += 1;
                let script = if open.shape == doc.shape && open.texts.len() == doc.texts.len() {
                    patch_script(&doc.texts, open.revision)
                } else {
                    body_script(&doc.body, open.revision)
                };
                events.push(event(PrintEventKind::Open, key, &title, &url, String::new(), open.revision, &html));
                events.push(event(PrintEventKind::Update, key, &title, &url, script, open.revision, &html));
                open.title = title;
                open.html = html;
                open.texts = doc.texts;
                open.shape = doc.shape;
            }
        }
        Ok(events)
    }

    /// The document changed (an edit in the form): the event that brings the window up to it. None when nothing came
    /// (a failed fill is empty), when it is the document already shown, or when no preview of that key is open.
    fn update(&mut self, key: &str, html: String) -> Vec<PrintEvent> {
        let path = self.sheet_path(key);
        let Some(open) = self.previews.get_mut(key) else { return Vec::new() };
        // A fill that failed answers nothing: what is shown stays.
        if !has_sheets(&html) || html == open.html {
            return Vec::new();
        }
        if let Err(error) = write_atomic(&path, &html) {
            tracing::warn!(%error, "print preview: could not write the new document");
            return Vec::new();
        }
        let doc = parse_doc(&html);
        open.revision += 1;
        let script = if open.shape == doc.shape && open.texts.len() == doc.texts.len() {
            patch_script(&doc.texts, open.revision)
        } else {
            body_script(&doc.body, open.revision)
        };
        let update = event(PrintEventKind::Update, key, &open.title, &url_for(&path), script, open.revision, &html);
        open.html = html;
        open.texts = doc.texts;
        open.shape = doc.shape;
        vec![update]
    }

    /// The window closed: forget the preview and delete its files.
    fn close(&mut self, key: &str) {
        if let Some(open) = self.previews.remove(key) {
            let _ = std::fs::remove_dir_all(&open.dir);
        }
        if self.previews.is_empty() {
            let _ = std::fs::remove_dir(&self.root);
        }
    }

    /// Every preview is over (the app quits): forget them all and delete the whole folder.
    fn close_all(&mut self) {
        self.previews.clear();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

thread_local! {
    /// The previews of this engine (hooks and TSX calls run on its thread).
    static STATE: RefCell<State> = RefCell::new(State::new(
        std::env::temp_dir().join(format!("ebirforms-print-{}", std::process::id())),
    ));
    /// What ends the last `Print.listen` (to tell it from an older one).
    static NEXT_BACKEND: Cell<u64> = const { Cell::new(1) };
}

/// The host: events go to the TSX listener (`post` works in a synchronous host call: the listener runs in the next turn).
struct HostBackend {
    listener: Callback<PrintEvent>,
}

impl Backend for HostBackend {
    fn emit(&self, event: PrintEvent) {
        self.listener.post(event);
    }
}

fn backend() -> Option<Rc<dyn Backend>> {
    STATE.with(|state| state.borrow().backend.as_ref().map(|(_, backend)| Rc::clone(backend)))
}

/// Makes `backend` the one that hears events; the id tells it from a later one.
fn install(backend: Rc<dyn Backend>) -> u64 {
    let id = NEXT_BACKEND.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    STATE.with(|state| state.borrow_mut().backend = Some((id, backend)));
    id
}

/// Ends what `install` with this `id` began (a newer host stays).
fn uninstall(id: u64) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.backend.as_ref().is_some_and(|(current, _)| *current == id) {
            state.backend = None;
        }
    });
}

// What the hooks call. ---------------------------------------------------------------------------------------------

/// `print_preview_request`: open the preview of the sheet at `path`, or refresh the one open for `key`. The file at `path`
/// (bir-app-api's `$TMP/bir-<form>-<year>-<ts>-preview.html`, a filled return) is this host's to dispose of: it is read once
/// and deleted at once, whatever comes next (the old shell's own hook keeps its file; this one is only the NSH app's).
pub(crate) fn request(path: PathBuf, title: String, key: String) {
    let read = std::fs::read_to_string(&path);
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %path.display(), %error, "print preview: cannot delete the source sheet");
        }
        _ => {}
    }
    let Some(backend) = backend() else {
        tracing::warn!(path = %path.display(), "the print preview has no window host (screens/print/host is not loaded)");
        return;
    };
    if !device_has_windows() {
        let mut said = event(PrintEventKind::Unavailable, &key, &title, "", String::new(), 0, "");
        said.message = Some(UNAVAILABLE.to_owned());
        backend.emit(said);
        return;
    }
    let html = match read {
        Ok(html) => bir_app_api::static_html::staticize_if_needed(html),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "print preview: cannot read the sheet");
            return;
        }
    };
    match STATE.with(|state| state.borrow_mut().request(&key, title, html)) {
        Ok(events) => events.into_iter().for_each(|event| backend.emit(event)),
        Err(error) => tracing::warn!(%error, "print preview"),
    }
}

/// Whether this device can open a second window for the sheet: the desktops. A phone build says no at compile time;
/// when the engine's own answer is at hand (a task, not a synchronous call: those have no engine context) it decides
/// too. The TSX host asks `isAvailable` of `nsh/desktop` as well, which is the engine's answer in every call.
fn device_has_windows() -> bool {
    !cfg!(any(target_os = "ios", target_os = "android")) && try_os().is_none_or(|os| os.is_desktop())
}

/// `print_preview_has_live`: a window is open for `key` and follows edits.
pub(crate) fn has_live(key: &str) -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        state.backend.is_some() && state.previews.contains_key(key)
    })
}

/// `print_preview_live_update`: make the newer document, then bring the window up to it. A host function is a synchronous
/// call of the engine, with no task to hand work to, so the fill runs here: about 35 ms for a 2551Q (808 boxes, 490 KB)
/// in a debug build, less in a release one. The page itself is patched by script, not reloaded.
pub(crate) fn live_update(key: &str, produce: PreviewProducer, started: Instant) {
    let Some(backend) = backend() else { return };
    if !STATE.with(|state| state.borrow().previews.contains_key(key)) {
        return;
    }
    let html = produce();
    let events = STATE.with(|state| state.borrow_mut().update(key, html));
    tracing::debug!(key, elapsed_ms = started.elapsed().as_millis() as u64, sent = events.len(), "print preview followed an edit");
    events.into_iter().for_each(|event| backend.emit(event));
}

/// The document and title of `key`'s preview, and the folder its files are in.
fn snapshot(key: &str) -> Option<(String, String, PathBuf)> {
    STATE.with(|state| {
        let state = state.borrow();
        let open = state.previews.get(key)?;
        Some((open.html.clone(), open.title.clone(), open.dir.clone()))
    })
}

// ------------------------------------------------------------------------------------------------ the PDF

// The five items below are what the macOS PDF maker (`mac`) and the tests use; elsewhere nothing makes a PDF yet
// (`platform_pdf`), so they are allowed to stand unused there instead of warning.

/// CSS pixels per PDF point: WebKit captures one point per CSS pixel, the sheets are laid out at 96 per inch.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
const CSS_PER_PT: f64 = 96.0 / 72.0;

/// The capture rectangle of one sheet in CSS pixels: A4, rounded down to whole pixels as WebKit emits it.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn capture_size() -> (f64, f64) {
    ((f64::from(PAPER_WIDTH_PT) * CSS_PER_PT).floor(), (f64::from(PAPER_HEIGHT_PT) * CSS_PER_PT).floor())
}

/// Pins every sheet to A4 with no screen gaps, so that sheet i starts at i times the paper height and each
/// capture is exactly one page.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
const CAPTURE_CSS: &str = "<style id=\"pdf-capture\">html,body{width:595.28pt !important;margin:0 !important;padding:0 !important;background:#fff !important}\
.sheet{margin:0 !important;box-shadow:none !important;height:841.89pt !important}.sheet::after{content:none !important}</style>";

/// The document with the capture style last in its head.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn for_capture(html: &str) -> String {
    match html.rfind("</head>") {
        Some(at) => format!("{}{CAPTURE_CSS}{}", &html[..at], &html[at..]),
        None => format!("{CAPTURE_CSS}{html}"),
    }
}

/// Turns one single-page capture per sheet (WebKit's CSS-pixel pages) into one A4 PDF with a page each.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub(crate) fn assemble_pdf(pages: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let (width, height) = capture_size();
    let expectation = PdfExpectation {
        form_code: "preview".to_owned(),
        revision: "preview".to_owned(),
        envelope_hash: "0".repeat(64),
        expected_page_count: pages.len(),
        width_points: width / CSS_PER_PT,
        height_points: height / CSS_PER_PT,
    };
    let normalized = pages
        .iter()
        .map(|page| normalize_wkpdf_page_from_css_pixels(page, &expectation))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("A page did not come out as A4: {error}"))?;
    merge_single_page_pdfs(&normalized).map_err(|error| error.to_string())
}

/// What a PDF holds (a probe for tests, and for the toolbar's "Saved" note).
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct PdfProbe {
    /// Its pages.
    pub pages: u32,
    /// The file's size in bytes.
    pub bytes: u32,
    /// The bytes of drawing instructions in all its pages (0: every page is blank).
    pub content_bytes: u32,
    /// The first page's width in points.
    pub width_pt: f64,
    /// The first page's height in points.
    pub height_pt: f64,
    /// The text of the pages as the system reads it (macOS: PDFKit), the first 3000 characters.
    pub text: String,
}

fn probe_pdf(path: &Path) -> Result<PdfProbe, String> {
    let bytes = std::fs::metadata(path).map_err(|error| format!("Cannot read {}: {error}", path.display()))?.len();
    let document = lopdf::Document::load(path).map_err(|error| format!("Not a PDF: {error}"))?;
    let pages = document.get_pages();
    let mut content_bytes = 0usize;
    for id in pages.values() {
        content_bytes += document.get_page_content(*id).map(|content| content.len()).unwrap_or(0);
    }
    let (width_pt, height_pt) = pages
        .values()
        .next()
        .and_then(|id| document.get_dictionary(*id).ok())
        .and_then(|page| page.get(b"MediaBox").ok())
        .and_then(|media| media.as_array().ok())
        .map(|media| {
            let number = |at: usize| media.get(at).and_then(|n| n.as_float().ok()).map_or(0.0, f64::from);
            (number(2) - number(0), number(3) - number(1))
        })
        .unwrap_or((0.0, 0.0));
    #[cfg(target_os = "macos")]
    let text = mac::pdf_text(path).chars().take(3000).collect();
    #[cfg(not(target_os = "macos"))]
    let text = String::new();
    Ok(PdfProbe {
        pages: pages.len() as u32,
        bytes: bytes.min(u64::from(u32::MAX)) as u32,
        content_bytes: content_bytes.min(u32::MAX as usize) as u32,
        width_pt,
        height_pt,
        text,
    })
}

/// The PDF of `html` (the document of a preview), made by the platform's web engine.
#[cfg(target_os = "macos")]
fn platform_pdf(html: &str) -> Result<Vec<u8>, String> {
    mac::make_pdf(html)
}

#[cfg(not(target_os = "macos"))]
fn platform_pdf(_html: &str) -> Result<Vec<u8>, String> {
    Err("Print and Save as PDF are available on macOS only for now.".to_owned())
}

/// The system print panel for the PDF at `path`: up when this returns.
#[cfg(target_os = "macos")]
fn platform_print(path: &Path) -> Result<(), String> {
    mac::print_pdf(path)
}

#[cfg(not(target_os = "macos"))]
fn platform_print(_path: &Path) -> Result<(), String> {
    Err("Print and Save as PDF are available on macOS only for now.".to_owned())
}

/// The macOS half: WebKit and PDFKit through objc2, on the main thread (every AppKit call is posted to the main
/// queue and waited for; callers are worker threads, never the main thread and never the engine's).
#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::path::Path;
    use std::sync::{Mutex, mpsc};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString, NSURL};

    use super::{CSS_PER_PT, PAPER_HEIGHT_PT, PAPER_WIDTH_PT, assemble_pdf, capture_size, for_capture};

    #[link(name = "WebKit", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "PDFKit", kind = "framework")]
    unsafe extern "C" {}

    thread_local! {
        /// The hidden web view of the capture running on the main thread.
        static WEB: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
    }

    /// One capture at a time (the hidden web view is shared).
    static CAPTURE: Mutex<()> = Mutex::new(());

    /// The PDF of `html`: a hidden WKWebView loads it, `createPDF` captures each A4 sheet as one page, bir-print
    /// normalizes the pages from CSS pixels to points and merges them.
    pub(super) fn make_pdf(html: &str) -> Result<Vec<u8>, String> {
        let _one = CAPTURE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let pages = capture(for_capture(html));
        DispatchQueue::main().exec_sync(|| WEB.with(|web| web.borrow_mut().take()).map_or((), drop));
        assemble_pdf(&pages?)
    }

    /// Loads `html` in a hidden web view and captures each sheet.
    fn capture(html: String) -> Result<Vec<Vec<u8>>, String> {
        let width = f64::from(PAPER_WIDTH_PT) * CSS_PER_PT;
        let height = f64::from(PAPER_HEIGHT_PT) * CSS_PER_PT;
        let (tx, rx) = mpsc::channel::<bool>();
        DispatchQueue::main().exec_sync(move || {
            // SAFETY: WebKit's documented API, on the main thread.
            let made = unsafe {
                let Some(class) = AnyClass::get(c"WKWebView") else {
                    let _ = tx.send(false);
                    return;
                };
                let web: Retained<AnyObject> = msg_send![class, new];
                let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
                let _: () = msg_send![&web, setFrame: frame];
                let page = NSString::from_str(&html);
                let _: Option<Retained<AnyObject>> =
                    msg_send![&web, loadHTMLString: &*page, baseURL: Option::<&NSURL>::None];
                WEB.with(|cell| *cell.borrow_mut() = Some(web));
                true
            };
            let _ = tx.send(made);
        });
        if !rx.recv().unwrap_or(false) {
            return Err("WebKit is not available.".to_owned());
        }
        // Loaded: not loading and the progress at its end.
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(60));
            let (tx, rx) = mpsc::channel();
            DispatchQueue::main().exec_sync(move || {
                let ready = WEB.with(|cell| {
                    cell.borrow().as_ref().map(|web| {
                        // SAFETY: WebKit's documented API, on the main thread.
                        unsafe {
                            let loading: Bool = msg_send![&**web, isLoading];
                            let progress: f64 = msg_send![&**web, estimatedProgress];
                            !loading.as_bool() && progress >= 1.0
                        }
                    })
                });
                let _ = tx.send(ready);
            });
            match rx.recv().ok().flatten() {
                None => return Err("The sheet's web view went away.".to_owned()),
                Some(true) => break,
                Some(false) if started.elapsed() > Duration::from_secs(20) => {
                    return Err("The sheet took too long to load.".to_owned());
                }
                Some(false) => {}
            }
        }
        let count = eval_number("document.querySelectorAll('.sheet').length")?.max(1.0) as usize;
        let (capture_width, capture_height) = capture_size();
        let mut pages = Vec::with_capacity(count);
        for index in 0..count {
            let (tx, rx) = mpsc::channel::<Result<Vec<u8>, String>>();
            DispatchQueue::main().exec_sync(move || {
                WEB.with(|cell| {
                    let Some(web) = cell.borrow().clone() else {
                        let _ = tx.send(Err("The sheet's web view went away.".to_owned()));
                        return;
                    };
                    // SAFETY: WebKit's documented API, on the main thread; the block runs on the main thread
                    // once, with the page's data or an error.
                    unsafe {
                        let Some(class) = AnyClass::get(c"WKPDFConfiguration") else {
                            let _ = tx.send(Err("WKPDFConfiguration is missing (macOS 11 or later).".to_owned()));
                            return;
                        };
                        let config: Retained<AnyObject> = msg_send![class, new];
                        let rect = NSRect::new(
                            NSPoint::new(0.0, (height * index as f64).round()),
                            NSSize::new(capture_width, capture_height),
                        );
                        let _: () = msg_send![&config, setRect: rect];
                        let done = RcBlock::new(move |data: *mut NSData, error: *mut AnyObject| {
                            let answer = match data.as_ref() {
                                Some(data) => Ok(data.to_vec()),
                                None if !error.is_null() => {
                                    let text: Option<Retained<NSString>> = msg_send![&*error, localizedDescription];
                                    Err(text.map(|t| t.to_string()).unwrap_or_else(|| "createPDF failed".to_owned()))
                                }
                                None => Err("createPDF gave nothing".to_owned()),
                            };
                            let _ = tx.send(answer);
                        });
                        let _: () = msg_send![&*web, createPDFWithConfiguration: &*config, completionHandler: &*done];
                    }
                });
            });
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(Ok(bytes)) => pages.push(bytes),
                Ok(Err(why)) => return Err(format!("Page {}: {why}", index + 1)),
                Err(_) => return Err(format!("Page {} timed out.", index + 1)),
            }
        }
        Ok(pages)
    }

    /// Evaluates `script` in the hidden web view: its number.
    fn eval_number(script: &'static str) -> Result<f64, String> {
        let (tx, rx) = mpsc::channel::<Option<f64>>();
        DispatchQueue::main().exec_sync(move || {
            WEB.with(|cell| {
                let Some(web) = cell.borrow().clone() else {
                    let _ = tx.send(None);
                    return;
                };
                // SAFETY: WebKit's documented API on the main thread; a number result is an NSNumber.
                unsafe {
                    let js = NSString::from_str(script);
                    let done = RcBlock::new(move |result: *mut AnyObject, _error: *mut AnyObject| {
                        let number = result.as_ref().map(|r| {
                            let value: f64 = msg_send![r, doubleValue];
                            value
                        });
                        let _ = tx.send(number);
                    });
                    let _: () = msg_send![&*web, evaluateJavaScript: &*js, completionHandler: &*done];
                }
            });
        });
        rx.recv_timeout(Duration::from_secs(10))
            .ok()
            .flatten()
            .ok_or_else(|| "The sheet did not answer.".to_owned())
    }

    /// The print operation for the PDF at `path`: PDFKit's, A4 (595.28 x 841.89 pt), no margins, no scaling, with the
    /// print panel. Main-thread work (AppKit objects), so it is built and run in the one block of `print_pdf`.
    pub(super) fn print_operation(path: &str) -> Result<Retained<AnyObject>, String> {
        // SAFETY: AppKit's and PDFKit's documented API.
        unsafe {
            let document_class = AnyClass::get(c"PDFDocument").ok_or("PDFKit is missing.")?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            let alloc: objc2::rc::Allocated<AnyObject> = msg_send![document_class, alloc];
            let document: Option<Retained<AnyObject>> = msg_send![alloc, initWithURL: &*url];
            let document = document.ok_or("The PDF did not open.")?;
            let info_class = AnyClass::get(c"NSPrintInfo").ok_or("AppKit has no NSPrintInfo.")?;
            let info: Retained<AnyObject> = msg_send![info_class, new];
            let paper = NSSize::new(f64::from(PAPER_WIDTH_PT), f64::from(PAPER_HEIGHT_PT));
            let _: () = msg_send![&info, setPaperSize: paper];
            let _: () = msg_send![&info, setTopMargin: 0.0f64];
            let _: () = msg_send![&info, setBottomMargin: 0.0f64];
            let _: () = msg_send![&info, setLeftMargin: 0.0f64];
            let _: () = msg_send![&info, setRightMargin: 0.0f64];
            // kPDFPrintPageScaleNone = 0.
            let operation: Option<Retained<AnyObject>> = msg_send![
                &document,
                printOperationForPrintInfo: &*info,
                scalingMode: 0isize,
                autoRotate: Bool::NO
            ];
            let operation = operation.ok_or("PDFKit gave no print operation.")?;
            let _: () = msg_send![&operation, setShowsPrintPanel: Bool::YES];
            Ok(operation)
        }
    }

    /// The system print panel for the PDF at `path`: A4, no scaling, as a sheet of the key window (a panel of
    /// its own when no window is key). Returns once the panel is up.
    pub(super) fn print_pdf(path: &Path) -> Result<(), String> {
        let path = path.display().to_string();
        let (tx, rx) = mpsc::channel();
        DispatchQueue::main().exec_async(move || {
            // SAFETY: AppKit's documented API, on the main thread.
            let answer = unsafe {
                (|| -> Result<(), String> {
                    let operation = print_operation(&path)?;
                    let app_class = AnyClass::get(c"NSApplication").ok_or("AppKit has no NSApplication.")?;
                    let app: Retained<AnyObject> = msg_send![app_class, sharedApplication];
                    let key: Option<Retained<AnyObject>> = msg_send![&app, keyWindow];
                    let window = match key {
                        Some(window) => Some(window),
                        None => msg_send![&app, mainWindow],
                    };
                    match window {
                        Some(window) => {
                            let none: *const AnyObject = std::ptr::null();
                            let _: () = msg_send![
                                &operation,
                                runOperationModalForWindow: &*window,
                                delegate: none,
                                didRunSelector: Option::<objc2::runtime::Sel>::None,
                                contextInfo: std::ptr::null_mut::<std::ffi::c_void>()
                            ];
                        }
                        None => {
                            let _: Bool = msg_send![&operation, runOperation];
                        }
                    }
                    Ok(())
                })()
            };
            let _ = tx.send(answer);
        });
        rx.recv_timeout(Duration::from_secs(10)).unwrap_or(Ok(()))
    }

    /// The text of the PDF at `path` as PDFKit reads it; empty when it cannot.
    pub(super) fn pdf_text(path: &Path) -> String {
        // SAFETY: PDFKit's documented API; reading a document's text is not main-thread work.
        unsafe {
            let Some(class) = AnyClass::get(c"PDFDocument") else { return String::new() };
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string()));
            let alloc: objc2::rc::Allocated<AnyObject> = msg_send![class, alloc];
            let document: Option<Retained<AnyObject>> = msg_send![alloc, initWithURL: &*url];
            let Some(document) = document else { return String::new() };
            let text: Option<Retained<NSString>> = msg_send![&document, string];
            text.map(|text| text.to_string()).unwrap_or_default()
        }
    }
}

// ------------------------------------------------------------------------------------------------ what TSX calls

/// Where the save panel starts: the person's Documents folder when there is one. The home folder is std's: `$HOME` on
/// Unix, the profile folder on Windows (where `HOME` is usually not set).
fn documents() -> Option<PathBuf> {
    std::env::home_dir()
        .map(|home| home.join("Documents"))
        .filter(|folder| folder.is_dir())
}

/// A path given for a save is only honoured in a test environment (`EBIR_TEST_ENV`: every run, test and shot
/// recipe sets it), where it stands in for the save panel; anywhere else the person is asked.
fn save_target(test_environment: bool, given: Option<&str>) -> Option<PathBuf> {
    given.filter(|path| test_environment && !path.is_empty()).map(PathBuf::from)
}

/// `path` with the `.pdf` ending a save panel may have left off.
fn with_pdf_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pdf")) { path } else { path.with_extension("pdf") }
}

/// The PDF of the document `key` shows now, its title and its folder (the document is the one the window has: edits
/// are applied as they come, in order).
async fn pdf_of(key: &str) -> Result<(Vec<u8>, String, PathBuf), String> {
    let (html, title, dir) = snapshot(key).ok_or("The preview is closed.")?;
    let pdf = blocking(move || platform_pdf(&html)).await.map_err(|error| error.to_string())??;
    Ok((pdf, title, dir))
}

/// The print preview's door to the desktop: what the preview window (`screens/print`) and its host call.
/// NSH has no print API, so the system print panel, the PDF and the live document are this crate's own.
#[module]
pub struct Print;

#[module]
impl Print {
    /// Hears every preview event until the returned function is called: a sheet to open (or to focus), a newer
    /// document for an open one, a device that has no window for it. One host listens at a time; the latest wins.
    pub fn listen(#[args(event)] listener: Callback<PrintEvent>) -> Stop {
        // Where a process cannot be asked (not unix), nothing is swept: see process_alive.
        #[cfg(unix)]
        sweep_stale(&std::env::temp_dir(), process_alive);
        let id = install(Rc::new(HostBackend { listener }));
        Stop::new(move || uninstall(id))
    }

    /// The window of the form code `key` closed (or could not open): forgets its preview and deletes its files.
    pub fn closed(key: String) {
        forget(&key);
    }

    /// Prints the preview of `key`: makes the PDF of its document, then shows the system print panel (A4, no
    /// scaling) as a sheet of the front window. Resolves once the panel is up; rejects with a sentence.
    pub async fn print(key: String) -> Result<(), String> {
        let (pdf, title, dir) = pdf_of(&key).await?;
        let path = write_pdf(&dir, &title, &pdf).map_err(|error| format!("Could not write the PDF: {error}"))?;
        blocking(move || platform_print(&path)).await.map_err(|error| error.to_string())?
    }

    /// Saves the PDF of `key`'s document: asks where with the system save panel and resolves with the path it
    /// wrote, or null when the person cancelled. A path given stands in for the panel in a test environment
    /// (EBIR_TEST_ENV) and is ignored anywhere else.
    pub async fn save_pdf(key: String, test_path: Option<String>) -> Result<Option<String>, String> {
        let (pdf, title, _) = pdf_of(&key).await?;
        let target = match save_target(crate::door::test_environment(), test_path.as_deref()) {
            Some(path) => path,
            None => {
                let mut dialog = SaveDialog::new()
                    .file_name(format!("{}.pdf", file_stem(&title)))
                    .filter(FileFilter::new("PDF", ["pdf"]))
                    .title("Save as PDF")
                    .button_label("Save");
                if let Some(folder) = documents() {
                    dialog = dialog.directory(folder);
                }
                match pick_save_path(dialog).await {
                    Ok(Some(path)) => with_pdf_extension(path),
                    Ok(None) => return Ok(None),
                    Err(error) => return Err(format!("The save panel could not open: {error}")),
                }
            }
        };
        std::fs::write(&target, &pdf).map_err(|error| format!("Could not save the PDF: {error}"))?;
        Ok(Some(target.display().to_string()))
    }

    /// What the PDF at `path` holds: pages, size, bytes of drawing, and (macOS) the text the system reads in it.
    /// A probe for tests and checks.
    pub fn inspect_pdf(path: String) -> Result<PdfProbe, String> {
        probe_pdf(Path::new(&path))
    }
}

fn forget(key: &str) {
    STATE.with(|state| state.borrow_mut().close(key));
}

/// The app quits: every preview of this engine thread is forgotten and its folder (`ebirforms-print-<pid>`) deleted. The door's
/// `shutdown` calls it; a quit that skips it leaves the folder to the next start's sweep.
pub(crate) fn remove_all() {
    let _ = STATE.try_with(|state| state.borrow_mut().close_all());
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// A filled sheet as the host functions hand it over: a Folio form of two pages, prepared and made static.
    fn sheet_doc(fields: &[&str]) -> String {
        let inputs: String = fields
            .iter()
            .enumerate()
            .map(|(i, value)| format!("<input type=\"text\" class=\"fi fh4\" id=\"f{i}\" value=\"{value}\">"))
            .collect();
        let raw = format!(
            "<html><head><style>@page {{ size: 612pt 936pt; }}</style></head><body><div class=\"page page-1\">{inputs}</div><div class=\"page page-2\"></div></body></html>"
        );
        bir_app_api::static_html::staticize(&bir_app_api::print_sheet::prepare_html(&raw))
    }

    /// A key no other test uses (every test thread has its own state, but the folders are shared).
    fn key() -> String {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        format!("T{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// Hears events.
    #[derive(Default)]
    struct Recorder {
        events: RefCell<Vec<PrintEvent>>,
    }

    impl Backend for Recorder {
        fn emit(&self, event: PrintEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    fn kinds(recorder: &Recorder) -> Vec<PrintEventKind> {
        recorder.events.borrow().iter().map(|event| event.kind).collect()
    }

    // The page rule ----------------------------------------------------------------------------------------------

    #[test]
    fn a_folio_sheet_prints_on_a4_scaled_down_to_fit() {
        let info = page_info(&sheet_doc(&["x"]));
        assert_eq!(info.paper, "A4");
        assert_eq!(info.sheet, "Folio");
        assert_eq!(info.pages, 2);
        // min((595.28 - 20) / 612, (841.89 - 20) / 936) = 0.8782: the height decides.
        assert!((info.zoom - (841.89 - 20.0) / 936.0).abs() < 1e-3, "zoom {}", info.zoom);
    }

    #[test]
    fn a_sheet_that_fits_is_never_scaled_up() {
        let raw = "<html><head><style>@page{size:400pt 500pt}</style></head><body><div class=\"page\"></div></body></html>";
        let info = page_info(&bir_app_api::static_html::staticize(&bir_app_api::print_sheet::prepare_html(raw)));
        assert_eq!((info.paper.as_str(), info.sheet.as_str(), info.zoom, info.pages), ("A4", "Custom", 1.0, 1));
    }

    #[test]
    fn a_document_without_the_paper_style_is_a4_at_scale_one() {
        let info = page_info("<html><body><div class=\"sheet\"></div></body></html>");
        assert_eq!((info.paper.as_str(), info.sheet.as_str(), info.zoom, info.pages), ("A4", "A4", 1.0, 1));
    }

    #[test]
    fn size_names() {
        assert_eq!(size_name(595.28, 841.89), "A4");
        assert_eq!(size_name(612.0, 936.0), "Folio");
        assert_eq!(size_name(612.0, 792.0), "Letter");
        assert_eq!(size_name(612.0, 1008.0), "Legal");
        assert_eq!(size_name(100.0, 100.0), "Custom");
    }

    // The document ------------------------------------------------------------------------------------------------

    #[test]
    fn the_field_texts_come_out_of_the_shape() {
        let a = parse_doc(&sheet_doc(&["Juana", "Santos"]));
        let b = parse_doc(&sheet_doc(&["Maria", ""]));
        assert_eq!(a.texts, vec!["Juana", "Santos"]);
        assert_eq!(b.texts, vec!["Maria", ""]);
        assert_eq!(a.shape, b.shape, "two fills of one sheet have one shape");
        let c = parse_doc(&sheet_doc(&["Juana", "Santos", "extra"]));
        assert_ne!(a.shape, c.shape, "another field is another shape");
        assert!(!a.shape.contains("Juana"));
        assert!(a.body.contains("Juana") && a.body.contains("page-2"));
    }

    #[test]
    fn an_escaped_text_stays_escaped() {
        let doc = parse_doc(&sheet_doc(&["A&amp;B"]));
        assert_eq!(doc.texts, vec!["A&amp;B"]);
        assert!(patch_script(&doc.texts, 3).contains("A&amp;B"));
    }

    #[test]
    fn the_patch_script_sets_every_box_and_marks_the_revision() {
        let script = patch_script(&["a".to_owned(), "\"q\"".to_owned()], 7);
        assert!(script.contains("span.fs"));
        assert!(script.ends_with("([\"a\",\"\\\"q\\\"\"],7);"), "{script}");
        let body = body_script("<div class=\"sheet\">\"x\"</div>", 2);
        assert!(body.contains("document.body.innerHTML=h"));
        assert!(body.ends_with("(\"<div class=\\\"sheet\\\">\\\"x\\\"</div>\",2);"), "{body}");
    }

    #[test]
    fn urls_names_and_stems() {
        assert_eq!(url_for(Path::new("/tmp/a b/é.html")), "file:///tmp/a%20b/%C3%A9.html");
        assert_eq!(sanitize("2551Q"), "2551Q");
        assert_eq!(sanitize("../x y"), "---x-y");
        assert_eq!(sanitize(""), "preview");
        assert_eq!(file_stem("BIR Form 1701Q \u{2014} Juana Santos"), "BIR Form 1701Q - Juana Santos");
        assert_eq!(file_stem("a/b:c"), "a-b-c");
        assert_eq!(file_stem("  "), "Print preview");
    }

    #[test]
    fn the_folders_of_gone_processes_are_swept_and_nothing_else() {
        let parent = tempfile::tempdir().unwrap();
        let make = |name: &str| {
            let folder = parent.path().join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(SHEET), "x").unwrap();
            folder
        };
        let gone = make("ebirforms-print-4000001");
        let running = make("ebirforms-print-4000002");
        let mine = make(&format!("ebirforms-print-{}", std::process::id()));
        let other = make("ebirforms-print-notapid");
        let unrelated = make("something-else");
        sweep_stale(parent.path(), |pid| pid == 4_000_002);
        assert!(!gone.exists(), "a gone process's folder is deleted");
        assert!(running.exists() && mine.exists() && other.exists() && unrelated.exists());
        assert!(process_alive(std::process::id()));
        sweep_stale(Path::new("/no/such/folder"), |_| false);
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn the_folders_and_files_of_a_preview_are_this_users_only() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("ebirforms-print-1");
        let mut state = State::new(root.clone());
        state.request("2551Q", "BIR Form 2551Q — Juana".to_owned(), sheet_doc(&["Juana"])).unwrap();
        assert_eq!(mode_of(&root), 0o700, "the folder of the run");
        assert_eq!(mode_of(&root.join("2551Q")), 0o700, "the folder of the preview");
        assert_eq!(mode_of(&root.join("2551Q").join(SHEET)), 0o600, "the sheet");
        state.update("2551Q", sheet_doc(&["Santos"]));
        assert_eq!(mode_of(&root.join("2551Q").join(SHEET)), 0o600, "the sheet rewritten by an edit");
        let pdf = write_pdf(&root.join("2551Q"), "BIR Form 2551Q — Juana", b"%PDF-1.4").unwrap();
        assert_eq!(pdf, root.join("2551Q").join("BIR Form 2551Q - Juana.pdf"));
        assert_eq!(mode_of(&pdf), 0o600, "the PDF Print writes");
        // A folder left at the predictable name by someone else's umask is made private; a link there is refused.
        let open = folder.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        private_dir(&open).unwrap();
        assert_eq!(mode_of(&open), 0o700);
        let link = folder.path().join("link");
        std::os::unix::fs::symlink(&open, &link).unwrap();
        assert!(private_dir(&link).is_err(), "a link at the folder's name is not followed");
        assert!(State::new(link.clone()).request("2551Q", "t".to_owned(), sheet_doc(&["a"])).is_err());
    }

    #[test]
    fn quitting_deletes_every_preview_folder() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("root");
        let mut state = State::new(root.clone());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        state.request("1601C", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        write_pdf(&root.join("1601C"), "t", b"%PDF").unwrap();
        state.close_all();
        assert!(!root.exists(), "the run's folder, every preview and PDF in it");
        assert!(state.previews.is_empty());
        // The hook's thread state too, through what the door's shutdown calls.
        let _recorder = host(folder.path());
        let key = key();
        request(sheet_file(folder.path(), &["a"]), "t".to_owned(), key.clone());
        assert!(folder.path().join(sanitize(&key)).exists());
        remove_all();
        assert!(!has_live(&key));
        assert!(!folder.path().exists(), "the thread's folder is gone");
    }

    #[test]
    fn the_hook_deletes_the_source_sheet_once_it_is_read() {
        let folder = tempfile::tempdir().unwrap();
        let recorder = host(&folder.path().join("previews"));
        let key = key();
        let source = sheet_file(folder.path(), &["Juana", ""]);
        request(source.clone(), "BIR Form".to_owned(), key.clone());
        assert!(!source.exists(), "the filled return is not left in the temp folder");
        assert_eq!(kinds(&recorder), vec![PrintEventKind::Open]);
        // With no host listening the file is not kept either.
        STATE.with(|state| state.borrow_mut().backend = None);
        let unread = sheet_file(folder.path(), &["b"]);
        request(unread.clone(), "t".to_owned(), key.clone());
        assert!(!unread.exists());
        forget(&key);
    }

    #[test]
    fn the_sheet_is_written_whole_and_replaced_whole() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("deep").join(SHEET);
        write_atomic(&path, "one").unwrap();
        write_atomic(&path, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from(SHEET)], "no half-written file is left");
    }

    // The previews ------------------------------------------------------------------------------------------------

    #[test]
    fn a_request_writes_the_sheet_and_opens_the_window_once() {
        let folder = tempfile::tempdir().unwrap();
        let mut state = State::new(folder.path().to_path_buf());
        let html = sheet_doc(&["Juana"]);
        let events = state.request("2551Q", "BIR Form 2551Q \u{2014} Juana".to_owned(), html.clone()).unwrap();
        assert_eq!(events.len(), 1);
        let open = &events[0];
        assert_eq!((open.kind, open.key.as_str(), open.revision, open.pages), (PrintEventKind::Open, "2551Q", 1, 2));
        assert_eq!((open.paper.as_str(), open.sheet.as_str()), ("A4", "Folio"));
        let file = folder.path().join("2551Q").join(SHEET);
        assert_eq!(open.url, url_for(&file));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), html);
        assert!(open.script.is_empty() && open.message.is_none());
    }

    #[test]
    fn asking_again_refreshes_in_place_with_a_patch_or_a_new_body() {
        let folder = tempfile::tempdir().unwrap();
        let mut state = State::new(folder.path().to_path_buf());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["Juana", ""])).unwrap();
        let same = state.request("2551Q", "t2".to_owned(), sheet_doc(&["Juana", "Santos"])).unwrap();
        assert_eq!(same.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![PrintEventKind::Open, PrintEventKind::Update]);
        assert_eq!((same[0].revision, same[0].title.as_str()), (2, "t2"));
        assert!(same[1].script.contains("span.fs") && same[1].script.contains("Santos"), "{}", same[1].script);
        let other = state.request("2551Q", "t2".to_owned(), sheet_doc(&["Juana", "Santos", "More"])).unwrap();
        assert!(other[1].script.contains("document.body.innerHTML"), "a new shape replaces the body");
        assert_eq!(other[1].revision, 3);
    }

    #[test]
    fn an_edit_patches_the_open_preview_and_writes_the_new_document() {
        let folder = tempfile::tempdir().unwrap();
        let mut state = State::new(folder.path().to_path_buf());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["a", ""])).unwrap();
        let events = state.update("2551Q", sheet_doc(&["a", "typed"]));
        assert_eq!(events.len(), 1);
        assert_eq!((events[0].kind, events[0].revision), (PrintEventKind::Update, 2));
        assert!(events[0].script.contains("\"typed\""), "{}", events[0].script);
        let on_disk = std::fs::read_to_string(folder.path().join("2551Q").join(SHEET)).unwrap();
        assert_eq!(parse_doc(&on_disk).texts, vec!["a", "typed"], "the file holds what the window shows");
    }

    #[test]
    fn a_failed_or_unchanged_fill_changes_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let mut state = State::new(folder.path().to_path_buf());
        let html = sheet_doc(&["a"]);
        state.request("2551Q", "t".to_owned(), html.clone()).unwrap();
        assert!(state.update("2551Q", String::new()).is_empty(), "an empty fill is a failed fill");
        assert!(state.update("2551Q", html).is_empty(), "the same document is no news");
        assert_eq!(state.previews["2551Q"].revision, 1);
        assert!(state.update("1601C", sheet_doc(&["x"])).is_empty(), "no window for another form");
    }

    #[test]
    fn closing_deletes_the_files() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("root");
        let mut state = State::new(root.clone());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        state.request("1601C", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        state.close("2551Q");
        assert!(!root.join("2551Q").exists() && root.join("1601C").exists());
        state.close("1601C");
        assert!(!root.exists(), "the last one takes the folder");
        state.close("1601C");
    }

    // What the hooks do ------------------------------------------------------------------------------------------

    /// A thread's state with a recorder as its host and its folders in a temp folder.
    fn host(folder: &Path) -> Rc<Recorder> {
        STATE.with(|state| state.borrow_mut().root = folder.to_path_buf());
        let recorder = Rc::new(Recorder::default());
        install(recorder.clone());
        recorder
    }

    fn sheet_file(folder: &Path, fields: &[&str]) -> PathBuf {
        let path = folder.join(format!("bir-{}-preview.html", fields.join("-")));
        std::fs::write(&path, sheet_doc(fields)).unwrap();
        path
    }

    #[test]
    fn the_hooks_open_one_preview_and_follow_edits() {
        let folder = tempfile::tempdir().unwrap();
        let recorder = host(folder.path());
        let key = key();
        assert!(!has_live(&key));
        request(sheet_file(folder.path(), &["a", ""]), "BIR Form".to_owned(), key.clone());
        assert!(has_live(&key));
        assert_eq!(kinds(&recorder), vec![PrintEventKind::Open]);

        let fill = sheet_doc(&["a", "typed"]);
        live_update(&key, Box::new(move || fill), Instant::now());
        assert_eq!(kinds(&recorder), vec![PrintEventKind::Open, PrintEventKind::Update]);
        assert!(recorder.events.borrow()[1].script.contains("typed"));

        // Another form has no window: nothing to follow.
        live_update("other", Box::new(|| sheet_doc(&["x"])), Instant::now());
        assert_eq!(recorder.events.borrow().len(), 2);

        // Pressing the button again refreshes and focuses (an Open and an Update, no second window of ours).
        request(sheet_file(folder.path(), &["b", ""]), "BIR Form".to_owned(), key.clone());
        assert_eq!(recorder.events.borrow().len(), 4);

        forget(&key);
        assert!(!has_live(&key));
        assert!(!folder.path().join(sanitize(&key)).exists());
    }

    #[test]
    fn a_fill_is_not_made_for_a_form_with_no_window() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;
        let folder = tempfile::tempdir().unwrap();
        let recorder = host(folder.path());
        let made = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&made);
        live_update(
            "nobody",
            Box::new(move || {
                flag.store(true, Ordering::Relaxed);
                String::new()
            }),
            Instant::now(),
        );
        assert!(!made.load(Ordering::Relaxed), "the fill is not even made");
        assert!(recorder.events.borrow().is_empty());
    }

    #[test]
    fn without_a_host_nothing_counts_as_open() {
        let folder = tempfile::tempdir().unwrap();
        let key = key();
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.root = folder.path().to_path_buf();
            state.backend = None;
        });
        request(sheet_file(folder.path(), &["a"]), "t".to_owned(), key.clone());
        assert!(!has_live(&key));
        live_update(&key, Box::new(|| sheet_doc(&["x"])), Instant::now());
        // A host that stops listening (the Stop) takes the preview's liveness with it.
        let recorder = host(folder.path());
        request(sheet_file(folder.path(), &["a"]), "t".to_owned(), key.clone());
        assert!(has_live(&key));
        let id = STATE.with(|state| state.borrow().backend.as_ref().unwrap().0);
        uninstall(id);
        assert!(!has_live(&key));
        assert_eq!(kinds(&recorder).len(), 1);
        STATE.with(|state| state.borrow_mut().close(&key));
    }

    #[test]
    fn a_newer_host_is_not_stopped_by_an_older_stop() {
        let folder = tempfile::tempdir().unwrap();
        let _first = host(folder.path());
        let older = STATE.with(|state| state.borrow().backend.as_ref().unwrap().0);
        let _second = host(folder.path());
        uninstall(older);
        assert!(backend().is_some());
    }

    #[test]
    fn the_hooks_of_the_app_reach_this_module() {
        use bir_app_api::hooks::Hooks;
        let folder = tempfile::tempdir().unwrap();
        let recorder = host(folder.path());
        let hooks = crate::hooks::NshHooks;
        let key = key();
        hooks.print_preview_request(sheet_file(folder.path(), &["a"]), "t".to_owned(), key.clone());
        assert!(hooks.print_preview_has_live(&key));
        let fill = sheet_doc(&["z"]);
        hooks.print_preview_live_update(&key, Box::new(move || fill), Instant::now());
        assert_eq!(kinds(&recorder), vec![PrintEventKind::Open, PrintEventKind::Update]);
        forget(&key);
        assert!(!hooks.print_preview_has_live(&key));
    }

    // Saving ------------------------------------------------------------------------------------------------------

    #[test]
    fn a_save_path_is_only_honoured_in_a_test_environment() {
        assert_eq!(save_target(true, Some("/tmp/x.pdf")), Some(PathBuf::from("/tmp/x.pdf")));
        assert_eq!(save_target(false, Some("/tmp/x.pdf")), None, "outside a test the person is asked");
        assert_eq!(save_target(true, Some("")), None);
        assert_eq!(save_target(true, None), None);
        assert_eq!(with_pdf_extension(PathBuf::from("/a/b")), PathBuf::from("/a/b.pdf"));
        assert_eq!(with_pdf_extension(PathBuf::from("/a/b.PDF")), PathBuf::from("/a/b.PDF"));
    }

    #[test]
    fn the_capture_style_goes_last_in_the_head() {
        let doc = for_capture("<html><head><style>a{}</style></head><body></body></html>");
        assert!(doc.find("pdf-capture").unwrap() > doc.find("a{}").unwrap());
        assert!(doc.find("pdf-capture").unwrap() < doc.find("</head>").unwrap());
        assert!(for_capture("<p>x</p>").starts_with("<style id=\"pdf-capture\">"));
    }

    // The PDF -----------------------------------------------------------------------------------------------------

    /// What WebKit's createPDF gives for one sheet: a single page of 793 x 1122 CSS pixels.
    fn captured_page(content: &str) -> Vec<u8> {
        use lopdf::{Document, Object, Stream, dictionary};
        let mut document = Document::with_version("1.7");
        let pages = document.new_object_id();
        let font = document.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let contents = document.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        let page = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 793.into(), 1122.into()],
            "Contents" => contents,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        document.objects.insert(
            pages,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }),
        );
        let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        document.trailer.set("Root", catalog);
        let mut out = Vec::new();
        document.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn one_capture_per_sheet_becomes_one_a4_page_each() {
        let pages: Vec<Vec<u8>> = ["Juana Santos", "TIN 000-123-453", "Page three"]
            .iter()
            .map(|text| captured_page(&format!("BT /F1 18 Tf 72 700 Td ({text}) Tj ET")))
            .collect();
        let pdf = assemble_pdf(&pages).expect("assembled");
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("sheet.pdf");
        std::fs::write(&path, &pdf).unwrap();
        let probe = probe_pdf(&path).unwrap();
        assert_eq!(probe.pages, 3, "a page per sheet");
        assert!((probe.width_pt - 594.75).abs() < 0.01 && (probe.height_pt - 841.5).abs() < 0.01, "{probe:?}");
        assert!(probe.content_bytes > 0 && probe.bytes as usize == pdf.len());
        #[cfg(target_os = "macos")]
        assert!(probe.text.contains("Juana Santos") && probe.text.contains("Page three"), "text: {:?}", probe.text);
    }

    /// What the system print panel is asked for: PDFKit's operation for the PDF, A4 with no margins, with the panel. The
    /// panel itself is never opened here (a test never shows the real print dialog).
    #[cfg(target_os = "macos")]
    #[test]
    fn the_print_operation_is_a4_with_no_margins_and_the_panel() {
        use objc2::msg_send;
        use objc2::rc::Retained;
        use objc2::runtime::{AnyObject, Bool};
        use objc2_foundation::NSSize;
        let pdf = assemble_pdf(&[captured_page("BT /F1 18 Tf 72 700 Td (Page) Tj ET")]).expect("assembled");
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("sheet.pdf");
        std::fs::write(&path, pdf).unwrap();
        let operation = mac::print_operation(&path.display().to_string()).expect("an operation");
        // SAFETY: reading the operation's settings; nothing is shown.
        unsafe {
            let info: Retained<AnyObject> = msg_send![&operation, printInfo];
            let paper: NSSize = msg_send![&info, paperSize];
            let (top, left): (f64, f64) = (msg_send![&info, topMargin], msg_send![&info, leftMargin]);
            let panel: Bool = msg_send![&operation, showsPrintPanel];
            // A4 to the point: a machine with no printer (a CI runner) reports the sheet in whole points, 595 x 842.
            assert!((paper.width - 595.28).abs() < 1.0 && (paper.height - 841.89).abs() < 1.0, "paper {paper:?}");
            assert_eq!((top, left), (0.0, 0.0));
            assert!(panel.as_bool());
        }
        assert!(mac::print_operation("/no/such/file.pdf").is_err());
    }

    #[test]
    fn a_capture_that_is_not_one_a4_sheet_is_refused() {
        assert!(assemble_pdf(&[]).is_err());
        assert!(assemble_pdf(&[b"not a pdf".to_vec()]).is_err());
    }

    #[test]
    fn a_blank_pdf_is_seen_as_blank() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("blank.pdf");
        std::fs::write(&path, captured_page("")).unwrap();
        let probe = probe_pdf(&path).unwrap();
        assert_eq!((probe.pages, probe.content_bytes), (1, 0));
        assert!(probe_pdf(&folder.path().join("missing.pdf")).is_err());
        std::fs::write(folder.path().join("junk.pdf"), "junk").unwrap();
        assert!(probe_pdf(&folder.path().join("junk.pdf")).is_err());
    }
}
