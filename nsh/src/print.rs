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
//! 3. **The PDF.** macOS: a hidden WKWebView loads the same HTML and `createPDF` captures each `.sheet` as one A4
//!    page; bir-print normalizes the pages and merges them (the reference app's way, see `internal/DECISIONS.md`
//!    D24). Windows: a hidden WebView2 of this crate's own loads the sheet and `PrintToPdf` prints it to a PDF by the
//!    page rule below, one A4 page per sheet, which is checked. Save as PDF uses the host save panel.
//! 4. **Print.** macOS: the system print panel for that PDF (PDFKit's print operation), A4, no scaling, as a sheet of
//!    the key window, so the file has the pages the printer gets. Windows: the system print dialog (printer, copies,
//!    pages, sides, colour), then WebView2 prints the sheet there by the same page rule as its PDF.
//!
//! **What the window shows.** macOS: the sheet's page in a web view, which the live update patches by script. Windows
//! and Linux: one picture per sheet, drawn on the CPU by Blitz and vello_cpu (`sheets.rs`), asked again at every
//! revision. No web view and no GPU are involved there: WebView2's page never reaches the screen on a computer
//! without a GPU, and a browser process is slow to start. The PDF and the printout still come from the web engine.
//!
//! **The page rule** (kept from the old shell): paper is always A4, 595.28 x 841.89 pt, zero margins. A sheet
//! whose own `@page` size is another (the frozen BIR forms are Folio, 612 x 936 pt) is scaled by
//! min((595.28 - 20) / w, (841.89 - 20) / h, 1) with 10 pt of safety above it: down to fit, never up. One printed
//! page per form page (`.sheet`).
//!
//! Linux: WebKitGTK's print operation makes the PDF (GTK's "Print to File" printer, the same page rule, checked as
//! Windows' is) and prints after GTK's print dialog. Other systems answer "not available" honestly.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use bir_app_api::hooks::PreviewProducer;
use bir_app_api::print_sheet::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};
use bir_print::html_output::{PdfExpectation, merge_single_page_pdfs, normalize_wkpdf_page_from_css_pixels};
use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::prelude::{Callback, JsObject, PropEnum, Stop, blocking, module, pick_save_path, try_os};

/// The sentence of a device that cannot open a second window.
pub const UNAVAILABLE: &str = "Print preview is not available on this device.";

/// The retired fixed name of the sheet inside a preview's folder; tests still use it for generic files.
#[cfg(test)]
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
    /// The session generation and request that own this document, never reused for a replacement.
    pub request_id: String,
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
/// (`<style id="preview-paper">`), whose `.sheet>.page` rule holds the scale — `transform:scale` on
/// WebKit hosts, `zoom` on Chromium ones. A document with no such style prints on A4 at scale 1.
pub(crate) fn page_info(html: &str) -> PageInfo {
    let marker = html.find("id=\"preview-paper\"");
    let (head, paper_css) = match marker {
        Some(at) => (&html[..at], &html[at..]),
        None => (html, ""),
    };
    let paper = page_size_in(paper_css).unwrap_or((PAPER_WIDTH_PT, PAPER_HEIGHT_PT));
    let native = page_size_in(head).unwrap_or(paper);
    let zoom = [".sheet>.page{transform:scale(", ".sheet>.page{zoom:"]
        .iter()
        .find_map(|rule| {
            let at = paper_css.find(rule)?;
            let rest = &paper_css[at + rule.len()..];
            rest[..rest.find([';', '}', ')'])?]
                .trim()
                .parse::<f64>()
                .ok()
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

/// Whether a process with this id is running.
#[cfg(all(windows, target_env = "msvc"))]
fn process_alive(pid: u32) -> bool {
    windows_print::process_alive(pid)
}

/// Where a process cannot be asked here every one counts as running, so [`Print::listen`] does not sweep at all
/// there: a stale folder stays until the system cleans its temp folder. (Nothing outside the tests calls it there, so
/// it may stand unused.)
#[cfg(not(any(unix, all(windows, target_env = "msvc"))))]
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
    file_url(&path.to_string_lossy(), cfg!(windows))
}

/// [`url_for`] for a path's text, on Windows (`windows`) or not. A Windows path's backslashes are its separators and
/// its drive keeps the colon, so `C:\Users\x\sheet.html` is `file:///C:/Users/x/sheet.html`, the URL WebView2 loads
/// (encoding both gave `file://C%3A%5CUsers...`, which names no file); a verbatim path (`\\?\C:\...`, what
/// `canonicalize` gives) loses its prefix, and a share (`\\server\share\...`) puts its server in the URL's host.
/// Elsewhere a backslash is a character of a name and is encoded with the rest.
fn file_url(path: &str, windows: bool) -> String {
    let mut url = String::from("file://");
    let plain: String;
    let mut rest = path;
    if windows {
        plain = match path.strip_prefix(r"\\?\UNC\") {
            Some(share) => format!(r"\\{share}"),
            None => path.strip_prefix(r"\\?\").unwrap_or(path).to_owned(),
        }
        .replace('\\', "/");
        rest = &plain;
        if let Some(share) = rest.strip_prefix("//") {
            rest = share;
        } else if rest.len() >= 2 && rest.as_bytes()[0].is_ascii_alphabetic() && rest.as_bytes()[1] == b':' {
            url.push('/');
            url.push_str(&rest[..2]);
            rest = &rest[2..];
        }
    }
    for byte in rest.bytes() {
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

/// A random id for one print artifact. The id, never taxpayer text, makes the
/// file name unique without revealing when it was made.
fn random_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// The file/folder stem of one print artifact: form code plus random id.
fn artifact_stem(form: &str) -> String {
    format!("{}-{}", sanitize(form), random_id())
}

fn sheet_file_name(stem: &str) -> String {
    format!("{stem}-sheet.html")
}

/// The live print workspace inside the app's private previews folder. The
/// launch sweep empties the parent; a closing preview removes its own artifact
/// folder.
fn print_work_dir() -> PathBuf {
    bir_core::platform::print_previews_dir().join("print")
}

/// What the OS print queue may call the job: the form number only, never the
/// window title (which may name its own user).
fn print_job_name(form: &str) -> String {
    sanitize(form)
}

fn artifact_paths(root: &Path, stem: &str) -> (PathBuf, PathBuf) {
    let dir = root.join(stem);
    let sheet = dir.join(sheet_file_name(stem));
    (dir, sheet)
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

/// Writes the print PDF of a preview into its private folder, named by form
/// plus a random id rather than the taxpayer-bearing window title.
// Windows and Linux print the sheet themselves (`windows_print::print_document`, `linux_print::print_document`), so
// only the tests write a PDF there.
#[cfg_attr(all(any(all(windows, target_env = "msvc"), target_os = "linux"), not(test)), allow(dead_code))]
fn write_pdf(dir: &Path, form: &str, pdf: &[u8]) -> std::io::Result<PathBuf> {
    private_dir(dir)?;
    let path = dir.join(format!("{}.pdf", artifact_stem(form)));
    write_private(&path, pdf)?;
    Ok(path)
}

// ------------------------------------------------------------------------------------------------ the previews

/// One open preview.
struct Preview {
    request: PreviewRequest,
    dir: PathBuf,
    sheet: PathBuf,
    title: String,
    /// The document the window shows now (the file holds the same).
    html: String,
    texts: Vec<String>,
    shape: String,
    revision: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct PreviewRequest {
    id: String,
    live: Arc<AtomicBool>,
}

impl PreviewRequest {
    pub(crate) fn is_live(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }

    fn check(&self) -> Result<(), String> {
        if self.is_live() { Ok(()) } else { Err("The preview is closed or was replaced.".to_owned()) }
    }

    fn revoke(&self) {
        self.live.store(false, Ordering::SeqCst);
    }
}

fn next_generation() -> u64 {
    static GENERATION: AtomicU64 = AtomicU64::new(1);
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Where events go: the TSX listener in the app, a recorder in tests.
trait Backend {
    fn emit(&self, event: PrintEvent);
}

struct State {
    generation: u64,
    sequence: u64,
    root: PathBuf,
    backend: Option<(u64, Rc<dyn Backend>)>,
    previews: HashMap<String, Preview>,
}

fn event(kind: PrintEventKind, key: &str, title: &str, url: &str, script: String, revision: u32, html: &str) -> PrintEvent {
    let info = page_info(html);
    PrintEvent {
        kind,
        key: key.to_owned(),
        request_id: String::new(),
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
        State { generation: next_generation(), sequence: 0, root, backend: None, previews: HashMap::new() }
    }

    #[cfg(all(test, target_os = "android"))]
    fn sheet_path(&self, key: &str) -> PathBuf {
        self.previews
            .get(key)
            .map(|open| open.sheet.clone())
            .unwrap_or_else(|| self.root.join(sanitize(key)).join(SHEET))
    }

    /// A preview was asked for: the events the host hears. A key already open gets a newer document in place.
    fn request(&mut self, key: &str, title: String, html: String) -> Result<Vec<PrintEvent>, String> {
        // Print.listen can run before the door has named this engine's scratch
        // database. Choose the live previews folder only when the first artifact
        // is written, never at listener startup.
        if self.root.as_os_str().is_empty() {
            self.root = print_work_dir();
        }
        let (dir, path) = match self.previews.get(key) {
            Some(open) => (open.dir.clone(), open.sheet.clone()),
            None => artifact_paths(&self.root, &artifact_stem(key)),
        };
        private_dir(&self.root).map_err(|error| format!("Could not write the preview: {error}"))?;
        write_atomic(&path, &html).map_err(|error| format!("Could not write the preview: {error}"))?;
        let url = url_for(&path);
        let doc = parse_doc(&html);
        self.sequence += 1;
        let request = PreviewRequest {
            id: format!("{}-{}", self.generation, self.sequence),
            live: Arc::new(AtomicBool::new(true)),
        };
        let mut events = Vec::new();
        match self.previews.get_mut(key) {
            None => {
                events.push(event(PrintEventKind::Open, key, &title, &url, String::new(), 1, &html));
                self.previews.insert(
                    key.to_owned(),
                    Preview {
                        request: request.clone(),
                        dir,
                        sheet: path,
                        title,
                        html,
                        texts: doc.texts,
                        shape: doc.shape,
                        revision: 1,
                    },
                );
            }
            Some(open) => {
                open.request.revoke();
                open.request = request.clone();
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
        for event in &mut events {
            event.request_id = request.id.clone();
        }
        Ok(events)
    }

    /// The document changed (an edit in the form): the event that brings the window up to it. None when nothing came
    /// (a failed fill is empty), when it is the document already shown, or when no preview of that key is open.
    fn update(&mut self, key: &str, html: String) -> Vec<PrintEvent> {
        let Some(open) = self.previews.get_mut(key) else { return Vec::new() };
        let path = open.sheet.clone();
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
        let mut update = event(PrintEventKind::Update, key, &open.title, &url_for(&path), script, open.revision, &html);
        update.request_id = open.request.id.clone();
        open.html = html;
        open.texts = doc.texts;
        open.shape = doc.shape;
        vec![update]
    }

    /// The window closed: forget the preview and delete its files.
    fn close(&mut self, key: &str) {
        if let Some(open) = self.previews.remove(key) {
            open.request.revoke();
            let _ = std::fs::remove_dir_all(&open.dir);
        }
        if self.previews.is_empty() {
            let _ = std::fs::remove_dir(&self.root);
        }
    }

    /// Every preview is over (the app quits): forget them all and delete the whole folder.
    fn close_all(&mut self) {
        self.generation = next_generation();
        for open in self.previews.values() {
            open.request.revoke();
        }
        self.previews.clear();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.request.revoke();
    }
}

thread_local! {
    /// The previews of this engine (hooks and TSX calls run on its thread).
    static STATE: RefCell<State> = RefCell::new(State::new(PathBuf::new()));
    /// What ends the last `Print.listen` (to tell it from an older one).
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; Clippy misreads Android's thread_local expansion"))]
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
/// (bir-app-api's `data_dir()/previews/bir-<form>-<year>-<ts>-preview.html`, a filled return) is this host's to dispose of: it is read once
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
    #[cfg(target_os = "android")]
    if Print::native_preview() {
        let result = read.map_err(|error| format!("Could not read the print sheet: {error}"))
            .and_then(|html| {
                if !has_sheets(&html) {
                    return Err("The print sheet is empty or has no pages.".to_owned());
                }
                crate::print_android::invalidate();
                STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.close_all();
                    state.request(&key, title.clone(), bir_app_api::static_html::staticize_if_needed(html))
                })
            });
        match result {
            Ok(events) => events.into_iter().for_each(|event| backend.emit(event)),
            Err(error) => {
                let mut said = event(PrintEventKind::Unavailable, &key, &title, "", String::new(), 0, "");
                said.message = Some(error);
                backend.emit(said);
            }
        }
        return;
    }
    if !device_has_windows() && !previews_natively() {
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

/// Whether this device shows the preview in the system's own print sheet instead of a window of the app (iOS:
/// UIPrintInteractionController). The host then prints the preview at once ([`Print::print`]) instead of opening a window.
fn previews_natively() -> bool {
    cfg!(target_os = "ios")
}

/// `print_preview_has_live`: a window is open for `key` and follows edits.
pub(crate) fn has_live(key: &str) -> bool {
    if cfg!(target_os = "android") {
        return false;
    }
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

/// The document and title of `key`'s preview, and its sheet file.
fn snapshot(key: &str) -> Option<(String, String, PathBuf)> {
    STATE.with(|state| {
        let state = state.borrow();
        let open = state.previews.get(key)?;
        Some((open.html.clone(), open.title.clone(), open.sheet.clone()))
    })
}

// ------------------------------------------------------------------------------------------------ the PDF

// The five items below are what the macOS PDF maker (`mac`) and the tests use; elsewhere nothing makes a PDF yet
// (`platform_pdf`), so they are allowed to stand unused there instead of warning.

/// CSS pixels per PDF point: WebKit captures one point per CSS pixel, the sheets are laid out at 96 per inch.
#[cfg_attr(not(any(test, target_os = "macos", target_os = "ios")), allow(dead_code))]
const CSS_PER_PT: f64 = 96.0 / 72.0;

/// The capture rectangle of one sheet in CSS pixels: A4, rounded down to whole pixels as WebKit emits it.
#[cfg_attr(not(any(test, target_os = "macos", target_os = "ios")), allow(dead_code))]
fn capture_size() -> (f64, f64) {
    ((f64::from(PAPER_WIDTH_PT) * CSS_PER_PT).floor(), (f64::from(PAPER_HEIGHT_PT) * CSS_PER_PT).floor())
}

/// Pins every sheet to A4 with no screen gaps, so that sheet i starts at i times the paper height and each
/// capture is exactly one page.
#[cfg_attr(not(any(test, target_os = "macos", target_os = "ios")), allow(dead_code))]
const CAPTURE_CSS: &str = "<style id=\"pdf-capture\">html,body{width:595.28pt !important;margin:0 !important;padding:0 !important;background:#fff !important}\
.sheet{margin:0 !important;box-shadow:none !important;height:841.89pt !important}.sheet::after{content:none !important}</style>";

/// The document with the capture style last in its head.
#[cfg_attr(not(any(test, target_os = "macos", target_os = "ios")), allow(dead_code))]
fn for_capture(html: &str) -> String {
    match html.rfind("</head>") {
        Some(at) => format!("{}{CAPTURE_CSS}{}", &html[..at], &html[at..]),
        None => format!("{CAPTURE_CSS}{html}"),
    }
}

/// Turns one single-page capture per sheet (WebKit's CSS-pixel pages) into one A4 PDF with a page each.
#[cfg_attr(not(any(test, target_os = "macos", target_os = "ios")), allow(dead_code))]
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

/// One sheet of a preview as a picture (where the app draws the sheets itself: [`Print::draws_sheets`]).
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct SheetImage {
    /// The picture: a PNG in a `data:` URI.
    pub src: String,
    /// Its width at 100%, in points of the window (CSS pixels of the sheet).
    pub width: f64,
    /// Its height at 100%.
    pub height: f64,
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
    /// The text of the pages as the system reads it (macOS: PDFKit; elsewhere the fonts' ToUnicode maps), the first
    /// [`PROBE_TEXT_CHARS`] characters.
    pub text: String,
}

/// How much of a PDF's text a probe keeps. A form's field values can come after every label of its page in the text
/// (WebView2 writes them last), and the 2551Q's labels alone pass 3000 characters.
const PROBE_TEXT_CHARS: usize = 20_000;

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
    let text = mac::pdf_text(path).chars().take(PROBE_TEXT_CHARS).collect();
    #[cfg(not(target_os = "macos"))]
    let text = pdf_text(&document).chars().take(PROBE_TEXT_CHARS).collect();
    Ok(PdfProbe {
        pages: pages.len() as u32,
        bytes: bytes.min(u64::from(u32::MAX)) as u32,
        content_bytes: content_bytes.min(u32::MAX as usize) as u32,
        width_pt,
        height_pt,
        text,
    })
}

/// The text of a PDF's pages read through each font's `ToUnicode` map: the strings its pages show, in order, a line per
/// text object. (WebView2's PDFs draw text in Type3 fonts whose maps lopdf's own text reader does not parse.)
#[cfg(not(target_os = "macos"))]
fn pdf_text(document: &lopdf::Document) -> String {
    let mut text = String::new();
    for page in document.get_pages().values() {
        let maps: HashMap<Vec<u8>, UnicodeMap> = document
            .get_page_fonts(*page)
            .map(|fonts| fonts.into_iter().map(|(name, font)| (name, unicode_map(document, font))).collect())
            .unwrap_or_default();
        let Ok(content) = document.get_and_decode_page_content(*page) else {
            continue;
        };
        let mut font: Option<&UnicodeMap> = None;
        for operation in &content.operations {
            match operation.operator.as_str() {
                "Tf" => font = operation.operands.first().and_then(|name| name.as_name().ok()).and_then(|name| maps.get(name)),
                "Tj" | "TJ" | "'" | "\"" => {
                    for operand in &operation.operands {
                        show(&mut text, font, operand);
                    }
                }
                "ET" => text.push('\n'),
                _ => {}
            }
        }
    }
    text
}

/// A font's codes (`width` bytes each) and the text each stands for.
#[cfg(not(target_os = "macos"))]
#[derive(Default)]
struct UnicodeMap {
    width: usize,
    codes: HashMap<u32, String>,
}

#[cfg(not(target_os = "macos"))]
fn show(text: &mut String, font: Option<&UnicodeMap>, operand: &lopdf::Object) {
    let Some(font) = font.filter(|font| font.width > 0) else {
        return;
    };
    match operand {
        lopdf::Object::String(bytes, _) => {
            for code in bytes.chunks(font.width) {
                let code = code.iter().fold(0u32, |n, &b| (n << 8) | u32::from(b));
                if let Some(chars) = font.codes.get(&code) {
                    text.push_str(chars);
                }
            }
        }
        lopdf::Object::Array(items) => {
            for item in items {
                show(text, Some(font), item);
            }
        }
        _ => {}
    }
}

#[cfg(not(target_os = "macos"))]
fn unicode_map(document: &lopdf::Document, font: &lopdf::Dictionary) -> UnicodeMap {
    font.get_deref(b"ToUnicode", document)
        .ok()
        .and_then(|object| object.as_stream().ok())
        .map(|stream| parse_unicode_map(&String::from_utf8_lossy(&stream.decompressed_content().unwrap_or_else(|_| stream.content.clone()))))
        .unwrap_or_default()
}

/// A `ToUnicode` CMap's `bfchar` and `bfrange` entries (a range's destination counts up from its first, or is a list).
#[cfg(not(target_os = "macos"))]
fn parse_unicode_map(cmap: &str) -> UnicodeMap {
    fn hex(token: &str) -> Option<Vec<u8>> {
        let digits = token.strip_prefix('<')?.strip_suffix('>')?;
        (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok()).collect()
    }
    fn code(bytes: &[u8]) -> u32 {
        bytes.iter().fold(0u32, |n, &b| (n << 8) | u32::from(b))
    }
    fn utf16(bytes: &[u8], plus: u16) -> String {
        let mut units: Vec<u16> = bytes.chunks(2).map(|pair| u16::from_be_bytes([pair[0], *pair.get(1).unwrap_or(&0)])).collect();
        if let Some(last) = units.last_mut() {
            *last = last.wrapping_add(plus);
        }
        String::from_utf16_lossy(&units)
    }
    let spaced = cmap.replace('<', " <").replace('>', "> ").replace('[', " [ ").replace(']', " ] ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut map = UnicodeMap::default();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "beginbfchar" => {
                i += 1;
                while i + 1 < tokens.len() && tokens[i] != "endbfchar" {
                    if let (Some(source), Some(target)) = (hex(tokens[i]), hex(tokens[i + 1])) {
                        map.width = source.len();
                        map.codes.insert(code(&source), utf16(&target, 0));
                    }
                    i += 2;
                }
            }
            "beginbfrange" => {
                i += 1;
                while i + 2 < tokens.len() && tokens[i] != "endbfrange" {
                    let (Some(low), Some(high)) = (hex(tokens[i]), hex(tokens[i + 1])) else {
                        i += 1;
                        continue;
                    };
                    map.width = low.len();
                    let (low, high) = (code(&low), code(&high));
                    if tokens[i + 2] == "[" {
                        let mut at = i + 3;
                        let mut n = low;
                        while at < tokens.len() && tokens[at] != "]" {
                            if let Some(target) = hex(tokens[at]) {
                                map.codes.insert(n, utf16(&target, 0));
                            }
                            n += 1;
                            at += 1;
                        }
                        i = at + 1;
                    } else {
                        if let Some(target) = hex(tokens[i + 2]) {
                            for n in low..=high.min(low.saturating_add(0xFFFF)) {
                                map.codes.insert(n, utf16(&target, (n - low) as u16));
                            }
                        }
                        i += 3;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    map
}

/// The PDF of `html` (the document of a preview), made by the platform's web engine.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_pdf(_form: &str, html: &str, _root: PathBuf) -> Result<Vec<u8>, String> {
    mac::make_pdf(html)
}

#[cfg(all(windows, target_env = "msvc"))]
fn platform_pdf(form: &str, html: &str, root: PathBuf) -> Result<Vec<u8>, String> {
    windows_print::make_pdf(form, html, root)
}

#[cfg(target_os = "linux")]
fn platform_pdf(form: &str, html: &str, root: PathBuf) -> Result<Vec<u8>, String> {
    linux_print::make_pdf(form, html, root)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", all(windows, target_env = "msvc"), target_os = "linux")))]
fn platform_pdf(_form: &str, _html: &str, _root: PathBuf) -> Result<Vec<u8>, String> {
    Err("Print and Save as PDF are available on macOS, Windows and Linux only for now.".to_owned())
}

/// The system print panel for the PDF at `path`: up when this returns.
#[cfg(target_os = "macos")]
fn platform_print(path: &Path, form: &str, request: PreviewRequest) -> Result<(), String> {
    mac::print_pdf(path, form, request)
}

/// The system print sheet (UIPrintInteractionController) for the PDF at `path`, until it is dismissed or finished.
#[cfg(target_os = "ios")]
fn platform_print(path: &Path, form: &str, request: PreviewRequest) -> Result<(), String> {
    let pdf = std::fs::read(path).map_err(|error| format!("Could not read the PDF: {error}"))?;
    ios_print::present(pdf, print_job_name(form), request)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", all(windows, target_env = "msvc"), target_os = "linux")))]
fn platform_print(_path: &Path, _form: &str, _request: PreviewRequest) -> Result<(), String> {
    Err("Print and Save as PDF are available on macOS, Windows and Linux only for now.".to_owned())
}

/// What Windows and Linux share: their web engine (WebView2, WebKitGTK) prints the sheet HTML itself, by one page rule,
/// from a job folder of its own, and the PDF it makes is checked to be one A4 page per sheet.
#[cfg(any(all(windows, target_env = "msvc"), target_os = "linux"))]
mod engine_print {
    use std::path::PathBuf;

    use super::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT};

    /// The sheets as printed pages: no margin, one A4 page per sheet and nothing else. The paper style gives a sheet
    /// its safety padding on top of its height, which would make it taller than the page and spill each one onto a
    /// second page: here the padding is inside the height, which stays just under the page's, and the only page
    /// breaks are the ones between sheets.
    pub(super) const PRINT_CSS: &str = "<style id=\"pdf-print\">@page{size:A4;margin:0}html,body{width:595.28pt !important;\
    margin:0 !important;padding:0 !important;background:#fff !important}.sheet{margin:0 !important;box-shadow:none !important;\
    box-sizing:border-box !important;width:595.28pt !important;height:841pt !important;overflow:hidden !important;\
    break-inside:avoid;break-after:auto !important;page-break-after:auto !important}\
    .sheet+.sheet{break-before:page;page-break-before:always}.sheet::after{content:none !important}</style>";

    /// The document with the print style and the form-only document title last
    /// in its head. Browser-based print pipelines use that title as the spooler
    /// job name, so it must never be the taxpayer-bearing window title.
    pub(super) fn for_print(form: &str, html: &str) -> String {
        let html = without_html_titles(html);
        let injection = format!("{}<title>{}</title>", PRINT_CSS, super::print_job_name(form));
        match html.rfind("</head>") {
            Some(at) => format!("{}{injection}{}", &html[..at], &html[at..]),
            None => format!("{injection}{html}"),
        }
    }

    /// Remove any document title already in the sheet. Browsers use the first
    /// title as the spooler job name; the form-only title added for printing
    /// must not lose to a taxpayer-bearing one.
    fn without_html_titles(html: &str) -> String {
        let lower = html.to_ascii_lowercase();
        let mut out = String::with_capacity(html.len());
        let mut at = 0;
        while let Some(relative) = lower[at..].find("<title") {
            let start = at + relative;
            let Some(open_end) = lower[start..].find('>').map(|end| start + end + 1) else { break };
            let Some(close) = lower[open_end..].find("</title>").map(|end| open_end + end) else { break };
            out.push_str(&html[at..start]);
            at = close + "</title>".len();
        }
        out.push_str(&html[at..]);
        out
    }

    /// A folder of this job's own in the private print workspace chosen on
    /// the engine thread, with the document written under a form-plus-random-id name.
    pub(super) fn job_folder_in(root: &std::path::Path, form: &str, html: &str) -> Result<(PathBuf, PathBuf), String> {
        let stem = super::artifact_stem(form);
        let folder = root.join(format!("job-{stem}"));
        super::private_dir(&folder).map_err(|error| format!("Could not make the print folder: {error}"))?;
        let sheet = folder.join(super::sheet_file_name(&stem));
        let printed = for_print(form, html);
        super::write_private(&sheet, printed.as_bytes()).map_err(|error| format!("Could not write the sheet: {error}"))?;
        Ok((folder, sheet))
    }

    /// The PDF has one A4 page per sheet: what the printer gets and what the file holds are the same pages.
    pub(super) fn check_a4(bytes: &[u8], sheets: u32) -> Result<(), String> {
        let document = lopdf::Document::load_mem(bytes).map_err(|error| format!("Not a PDF: {error}"))?;
        let pages = document.get_pages();
        if pages.len() as u32 != sheets {
            return Err(format!("A page did not come out as A4: {} pages for {sheets} sheets.", pages.len()));
        }
        for id in pages.values() {
            let size = document
                .get_dictionary(*id)
                .ok()
                .and_then(|page| page.get(b"MediaBox").ok())
                .and_then(|media| media.as_array().ok())
                .map(|media| {
                    let at = |i: usize| media.get(i).and_then(|n| n.as_float().ok()).map_or(0.0, f64::from);
                    (at(2) - at(0), at(3) - at(1))
                })
                .unwrap_or((0.0, 0.0));
            let (width, height) = (f64::from(PAPER_WIDTH_PT), f64::from(PAPER_HEIGHT_PT));
            if (size.0 - width).abs() > 1.5 || (size.1 - height).abs() > 1.5 {
                return Err(format!("A page did not come out as A4: {:.1} x {:.1} pt.", size.0, size.1));
            }
        }
        Ok(())
    }
}

/// The Linux half: WebKitGTK's print operation, on GTK's main thread, where WebKit and GTK live. Each job loads the
/// sheet into a hidden web view of its own (never shown, so nsh's web views are never touched) and prints it by the
/// page rule: A4, no margins, scale 1. Save as PDF prints to GTK's own "Print to File" printer (built into GTK 4, as
/// is CUPS), which writes a vector PDF that is then checked as Windows' is; Print shows GTK's print dialog (the
/// printers CUPS knows, copies, pages, sides) over the preview window and prints there. Callers are worker threads
/// (`blocking`), never the main thread, which would wait on itself. A run without GTK (headless tests) answers with a
/// sentence instead of waiting.
#[cfg(target_os = "linux")]
mod linux_print {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::{Mutex, mpsc};
    use std::time::Duration;

    use gtk4::{self as gtk, glib, prelude::*};
    use webkit6::prelude::*;

    use super::engine_print::{check_a4, job_folder_in};
    use super::{page_info, url_for};

    /// One print job at a time.
    static ONE: Mutex<()> = Mutex::new(());

    /// How long making a PDF may take before it gives up (loading the sheet and printing it is a second or two).
    const PDF_TIME: Duration = Duration::from_secs(120);

    type Answer<T> = mpsc::Sender<Result<T, String>>;

    /// Runs `job` on GTK's main thread and waits for what it sends, at most `limit`.
    fn on_main<T: Send + 'static>(limit: Option<Duration>, job: impl FnOnce(Answer<T>) + Send + 'static) -> Result<T, String> {
        if !gtk::is_initialized() {
            return Err("Print and Save as PDF need the app's window, which this run does not have.".to_owned());
        }
        let (tx, rx) = mpsc::channel();
        glib::MainContext::default().invoke(move || job(tx));
        match limit {
            Some(limit) => rx.recv_timeout(limit).map_err(|_| "The print job did not finish in time.".to_owned())?,
            None => rx.recv().map_err(|_| "The print job ended without an answer.".to_owned())?,
        }
    }

    /// Sends `answer` once, whichever of the job's callbacks comes first.
    fn once<T: 'static>(tx: Answer<T>) -> impl Fn(Result<T, String>) + Clone + 'static {
        let tx = Rc::new(RefCell::new(Some(tx)));
        move |answer| {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(answer);
            }
        }
    }

    /// A4 portrait with no margins: the paper of the page rule.
    fn page_setup() -> gtk::PageSetup {
        let setup = gtk::PageSetup::new();
        setup.set_paper_size(&gtk::PaperSize::new(Some(gtk::PAPER_NAME_A4)));
        setup.set_orientation(gtk::PageOrientation::Portrait);
        setup.set_top_margin(0.0, gtk::Unit::Points);
        setup.set_bottom_margin(0.0, gtk::Unit::Points);
        setup.set_left_margin(0.0, gtk::Unit::Points);
        setup.set_right_margin(0.0, gtk::Unit::Points);
        setup
    }

    /// Loads `url` into a hidden web view and calls `ready` with it once the page is in (or why it is not).
    fn load(url: &str, ready: impl FnOnce(Result<webkit6::WebView, String>) + 'static) {
        let view = webkit6::WebView::new();
        // WebKit loads only into a view in a window: a window of its own, realized and never shown.
        let window = gtk::Window::new();
        window.set_child(Some(&view));
        window.set_default_size(800, 1100);
        WidgetExt::realize(&window);
        let ready = Rc::new(RefCell::new(Some(ready)));
        let failed = ready.clone();
        view.connect_load_failed(move |_, _, _, error| {
            if let Some(ready) = failed.borrow_mut().take() {
                ready(Err(format!("The sheet did not load: {error}")));
            }
            true
        });
        view.connect_load_changed(move |view, event| {
            if event == webkit6::LoadEvent::Finished
                && let Some(ready) = ready.borrow_mut().take()
            {
                ready(Ok(view.clone()));
            }
        });
        view.load_uri(url);
    }

    /// Prints `view` with `settings` by the page rule: straight away (`parent` none), or after GTK's print dialog over
    /// `parent`. Answers true once the job is done, false when the person cancelled the dialog.
    fn print_view(view: webkit6::WebView, settings: gtk::PrintSettings, parent: Option<gtk::Window>, answer: impl Fn(Result<bool, String>) + Clone + 'static) {
        let operation = webkit6::PrintOperation::new(&view);
        operation.set_print_settings(&settings);
        operation.set_page_setup(&page_setup());
        // The view and the operation live until the job ends.
        let kept = Rc::new(RefCell::new(Some((view, operation.clone()))));
        let failed = answer.clone();
        let released = kept.clone();
        operation.connect_failed(move |_, error| {
            failed(Err(format!("The document could not be printed: {error}")));
            released.borrow_mut().take();
        });
        let done = answer.clone();
        operation.connect_finished(move |_| {
            done(Ok(true));
            kept.borrow_mut().take();
        });
        match parent {
            None => operation.print(),
            Some(parent) => {
                if operation.run_dialog(Some(&parent)) == webkit6::PrintOperationResponse::Cancel {
                    answer(Ok(false));
                }
            }
        }
    }

    /// GTK's "Print to File" printer (its name is translated, so it is found by what it is: the virtual printer that
    /// takes PDF). `found` gets it, or None once every printer was listed without it.
    fn file_printer(found: impl FnOnce(Option<gtk::Printer>) + 'static) {
        let found = Rc::new(RefCell::new(Some(found)));
        let last = found.clone();
        // GTK calls this on the main thread, but its binding asks for a thread-safe function: the guard says so,
        // and would panic if it were ever touched elsewhere.
        let guarded = glib::thread_guard::ThreadGuard::new(found);
        gtk::enumerate_printers(
            move |printer| {
                if printer.is_virtual() && printer.accepts_pdf() {
                    if let Some(found) = guarded.get_ref().borrow_mut().take() {
                        found(Some(printer.clone()));
                    }
                    return true;
                }
                false
            },
            false,
        );
        // The file printer is GTK's own and listed at once; the CUPS ones may take a while and are not waited for.
        glib::timeout_add_local_once(Duration::from_secs(10), move || {
            if let Some(found) = last.borrow_mut().take() {
                found(None);
            }
        });
    }

    /// The PDF of `html`: WebKitGTK prints the document to GTK's "Print to File" printer, A4 with no margins, one page
    /// per sheet, checked to be that (as Windows' is).
    pub(super) fn make_pdf(form: &str, html: &str, root: PathBuf) -> Result<Vec<u8>, String> {
        let _one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let expected = page_info(html).pages.max(1);
        let (folder, sheet) = job_folder_in(&root, form, html)?;
        let out = folder.join(format!("{}.pdf", super::artifact_stem(form)));
        let (url, out_url) = (url_for(&sheet), url_for(&out));
        let made = on_main(Some(PDF_TIME), move |tx| {
            let answer = once(tx);
            load(&url, move |view| {
                let view = match view {
                    Ok(view) => view,
                    Err(error) => return answer(Err(error)),
                };
                file_printer(move |printer| {
                    let Some(printer) = printer else {
                        return answer(Err("GTK's Print to File printer is not there to make the PDF.".to_owned()));
                    };
                    let settings = gtk::PrintSettings::new();
                    settings.set_printer(&printer.name());
                    settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&out_url));
                    settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
                    settings.set_scale(100.0);
                    print_view(view, settings, None, answer);
                });
            });
        });
        let bytes = made.and_then(|_| std::fs::read(&out).map_err(|error| format!("Could not read the PDF: {error}")));
        let _ = std::fs::remove_dir_all(&folder);
        let bytes = bytes?;
        check_a4(&bytes, expected)?;
        Ok(bytes)
    }

    /// Prints `html`: GTK's print dialog over the front window (the preview, which asked), then WebKitGTK prints the
    /// document there by the same page rule as the PDF. Resolves once the job is with the printer, or when the person
    /// cancels the dialog; rejects with a sentence.
    pub(super) fn print_document(form: &str, html: &str, root: PathBuf) -> Result<(), String> {
        let _one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (folder, sheet) = job_folder_in(&root, form, html)?;
        let url = url_for(&sheet);
        let printed = on_main(None, move |tx| {
            let answer = once(tx);
            load(&url, move |view| {
                let view = match view {
                    Ok(view) => view,
                    Err(error) => return answer(Err(error)),
                };
                let windows = gtk::Window::list_toplevels();
                let parent = windows
                    .iter()
                    .filter_map(|w| w.clone().downcast::<gtk::Window>().ok())
                    .find(|w| w.is_active())
                    .or_else(|| windows.iter().filter_map(|w| w.clone().downcast::<gtk::Window>().ok()).find(|w| w.is_visible()));
                let Some(parent) = parent else {
                    return answer(Err("There is no window to show the print dialog over.".to_owned()));
                };
                let settings = gtk::PrintSettings::new();
                settings.set_scale(100.0);
                print_view(view, settings, Some(parent), answer);
            });
        });
        let _ = std::fs::remove_dir_all(&folder);
        printed.map(|_| ())
    }
}

/// The Windows half: WebView2 and the system print dialog, on a thread of its own. WebView2 wants a single-threaded COM
/// apartment with a message loop, which any thread can be, so each job runs on a fresh one with a hidden window that
/// hosts a hidden web view (its own profile folder in the temp folder, so nsh's web views are never touched), waits for
/// it, and ends. Callers are worker threads, never the main thread and never the engine's.
#[cfg(all(windows, target_env = "msvc"))]
mod windows_print {
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::{Mutex, mpsc};

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PRINT_COLLATION_COLLATED, COREWEBVIEW2_PRINT_COLLATION_UNCOLLATED,
        COREWEBVIEW2_PRINT_COLOR_MODE_COLOR, COREWEBVIEW2_PRINT_COLOR_MODE_GRAYSCALE, COREWEBVIEW2_PRINT_DUPLEX_DEFAULT,
        COREWEBVIEW2_PRINT_DUPLEX_ONE_SIDED, COREWEBVIEW2_PRINT_DUPLEX_TWO_SIDED_LONG_EDGE,
        COREWEBVIEW2_PRINT_DUPLEX_TWO_SIDED_SHORT_EDGE, COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
        COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE, COREWEBVIEW2_PRINT_STATUS_SUCCEEDED, ICoreWebView2,
        ICoreWebView2_7, ICoreWebView2_16, ICoreWebView2Controller, ICoreWebView2Environment,
        ICoreWebView2Environment6, ICoreWebView2PrintSettings, ICoreWebView2PrintSettings2,
    };
    use webview2_com::{
        CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
        NavigationCompletedEventHandler, PrintCompletedHandler, PrintToPdfCompletedHandler, wait_with_pump,
    };
    use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{DEVMODEW, DMCOLOR_MONOCHROME, DMCOLLATE_TRUE, DMDUP_HORIZONTAL, DMDUP_SIMPLEX, DMDUP_VERTICAL};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::UI::Controls::Dialogs::{
        DEVNAMES, PD_NOCURRENTPAGE, PD_NOSELECTION, PD_PAGENUMS, PD_RESULT_PRINT, PD_USEDEVMODECOPIESANDCOLLATE,
        PRINTDLGEXW, PRINTPAGERANGE, PrintDlgExW, START_PAGE_GENERAL,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow, RegisterClassW, WINDOW_EX_STYLE, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{HRESULT, HSTRING, Interface, PCWSTR, w};

    pub(super) use super::engine_print::check_a4;
    use super::engine_print::job_folder_in;
    use super::{PAPER_HEIGHT_PT, PAPER_WIDTH_PT, page_info, url_for};

    // The loader is linked statically, as nsh's Windows backend does (`WebView2LoaderStatic.lib`, which
    // webview2-com-sys ships and puts on the link path), so the app ships no `WebView2Loader.dll`.
    #[link(name = "WebView2LoaderStatic", kind = "static")]
    unsafe extern "system" {
        #[link_name = "CreateCoreWebView2EnvironmentWithOptions"]
        fn create_environment(
            browser_folder: PCWSTR,
            user_data_folder: PCWSTR,
            options: *mut c_void,
            handler: *mut c_void,
        ) -> HRESULT;
    }

    /// One print job at a time.
    static ONE: Mutex<()> = Mutex::new(());

    fn text(error: impl std::fmt::Display) -> String {
        error.to_string()
    }

    /// The PDF of `html`: WebView2's `PrintToPdf` of the document, A4 with no margins, one page per sheet, checked
    /// to be that (as the macOS capture is).
    pub(super) fn make_pdf(form: &str, html: &str, root: PathBuf) -> Result<Vec<u8>, String> {
        let _one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let expected = page_info(html).pages.max(1);
        let (folder, sheet) = job_folder_in(&root, form, html)?;
        let out = folder.join(format!("{}.pdf", super::artifact_stem(form)));
        let target = out.clone();
        let made = on_print_thread(move |page| {
            page.load(&url_for(&sheet))?;
            let settings = page.settings()?;
            let web: ICoreWebView2_7 = page.web.cast().map_err(|_| "This WebView2 cannot make PDFs; update the WebView2 Runtime.".to_owned())?;
            let (tx, rx) = mpsc::channel();
            let handler = PrintToPdfCompletedHandler::create(Box::new(move |result, ok| {
                let _ = tx.send(result.is_ok() && ok);
                Ok(())
            }));
            // SAFETY: live objects of this thread; the handler is kept until it is called (the wait below).
            unsafe { web.PrintToPdf(&HSTRING::from(target.as_os_str()), &settings, &handler) }.map_err(text)?;
            if wait_with_pump(rx).map_err(text)? { Ok(()) } else { Err("WebView2 could not make the PDF.".to_owned()) }
        });
        let bytes = made.and_then(|()| std::fs::read(&out).map_err(|error| format!("Could not read the PDF: {error}")));
        let _ = std::fs::remove_dir_all(&folder);
        let bytes = bytes?;
        check_a4(&bytes, expected)?;
        Ok(bytes)
    }

    /// Prints `html`: the system print dialog (printer, copies, pages, sides, colour), then WebView2 prints the
    /// document there by the same page rule as the PDF. Resolves once the job is with the printer, or when the
    /// person cancels the dialog; rejects with a sentence.
    pub(super) fn print_document(form: &str, html: &str, root: PathBuf) -> Result<(), String> {
        let _one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let pages = page_info(html).pages.max(1);
        let (folder, sheet) = job_folder_in(&root, form, html)?;
        // The front window owns the dialog (the preview, which asked).
        // SAFETY: plain queries.
        let owner = unsafe { GetForegroundWindow() };
        let owner = owner.0 as isize;
        let printed = on_print_thread(move |page| {
            let owner = if owner == 0 { page.window } else { HWND(owner as *mut c_void) };
            let Some(choice) = choose_printer(owner, pages)? else {
                return Ok(());
            };
            page.load(&url_for(&sheet))?;
            let settings = page.settings()?;
            choice.apply(&settings)?;
            let web: ICoreWebView2_16 =
                page.web.cast().map_err(|_| "This WebView2 cannot print; update the WebView2 Runtime.".to_owned())?;
            let (tx, rx) = mpsc::channel();
            let handler = PrintCompletedHandler::create(Box::new(move |result, status| {
                let _ = tx.send(result.map(|()| status));
                Ok(())
            }));
            // SAFETY: live objects of this thread; the handler is kept until it is called (the wait below).
            unsafe { web.Print(&settings, &handler) }.map_err(text)?;
            match wait_with_pump(rx).map_err(text)?.map_err(text)? {
                status if status == COREWEBVIEW2_PRINT_STATUS_SUCCEEDED => Ok(()),
                status if status == COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE => {
                    Err(format!("The printer {} is not available.", choice.printer))
                }
                _ => Err("The document could not be printed.".to_owned()),
            }
        });
        let _ = std::fs::remove_dir_all(&folder);
        printed
    }

    /// What the person chose in the print dialog.
    struct Choice {
        printer: String,
        copies: i32,
        collate: bool,
        /// `"1-2,4"`, or empty for every page.
        ranges: String,
        duplex: Option<i16>,
        mono: bool,
    }

    impl Choice {
        fn apply(&self, settings: &ICoreWebView2PrintSettings) -> Result<(), String> {
            let settings: ICoreWebView2PrintSettings2 =
                settings.cast().map_err(|_| "This WebView2 cannot print; update the WebView2 Runtime.".to_owned())?;
            // SAFETY: setters of a live settings object.
            unsafe {
                settings.SetPrinterName(&HSTRING::from(&self.printer)).map_err(text)?;
                settings.SetCopies(self.copies.max(1)).map_err(text)?;
                settings
                    .SetCollation(if self.collate { COREWEBVIEW2_PRINT_COLLATION_COLLATED } else { COREWEBVIEW2_PRINT_COLLATION_UNCOLLATED })
                    .map_err(text)?;
                if !self.ranges.is_empty() {
                    settings.SetPageRanges(&HSTRING::from(&self.ranges)).map_err(text)?;
                }
                let duplex = match self.duplex {
                    Some(d) if d == DMDUP_SIMPLEX.0 as i16 => COREWEBVIEW2_PRINT_DUPLEX_ONE_SIDED,
                    Some(d) if d == DMDUP_VERTICAL.0 as i16 => COREWEBVIEW2_PRINT_DUPLEX_TWO_SIDED_LONG_EDGE,
                    Some(d) if d == DMDUP_HORIZONTAL.0 as i16 => COREWEBVIEW2_PRINT_DUPLEX_TWO_SIDED_SHORT_EDGE,
                    _ => COREWEBVIEW2_PRINT_DUPLEX_DEFAULT,
                };
                settings.SetDuplex(duplex).map_err(text)?;
                settings
                    .SetColorMode(if self.mono { COREWEBVIEW2_PRINT_COLOR_MODE_GRAYSCALE } else { COREWEBVIEW2_PRINT_COLOR_MODE_COLOR })
                    .map_err(text)?;
            }
            Ok(())
        }
    }

    /// The system print dialog over `owner`, for a document of `pages` pages. `None` when the person cancels.
    fn choose_printer(owner: HWND, pages: u32) -> Result<Option<Choice>, String> {
        let mut ranges = [PRINTPAGERANGE { nFromPage: 1, nToPage: pages }; 32];
        let mut dialog = PRINTDLGEXW {
            lStructSize: std::mem::size_of::<PRINTDLGEXW>() as u32,
            hwndOwner: owner,
            Flags: PD_NOSELECTION | PD_NOCURRENTPAGE | PD_USEDEVMODECOPIESANDCOLLATE,
            nMaxPageRanges: ranges.len() as u32,
            lpPageRanges: ranges.as_mut_ptr(),
            nMinPage: 1,
            nMaxPage: pages,
            nCopies: 1,
            nStartPage: START_PAGE_GENERAL,
            ..Default::default()
        };
        // SAFETY: `dialog` is a filled PRINTDLGEXW whose page-range buffer outlives the call; the handles it returns
        // are read and freed below.
        unsafe { PrintDlgExW(&mut dialog) }.map_err(|error| format!("The print dialog could not open: {error}"))?;
        let devmode = dialog.hDevMode;
        let devnames = dialog.hDevNames;
        let choice = if dialog.dwResultAction == PD_RESULT_PRINT {
            // SAFETY: the handles the dialog returned, locked while read.
            let printer = unsafe { device_name(devnames) };
            let (copies, collate, duplex, mono) = unsafe { device_mode(devmode) };
            let ranges = if dialog.Flags.contains(PD_PAGENUMS) {
                ranges[..dialog.nPageRanges as usize]
                    .iter()
                    .map(|r| if r.nFromPage == r.nToPage { r.nFromPage.to_string() } else { format!("{}-{}", r.nFromPage, r.nToPage) })
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                String::new()
            };
            Some(Choice { printer, copies, collate, ranges, duplex, mono })
        } else {
            None
        };
        // SAFETY: the dialog's handles are the caller's to free, once.
        unsafe {
            if !devmode.is_invalid() {
                let _ = GlobalFree(Some(devmode));
            }
            if !devnames.is_invalid() {
                let _ = GlobalFree(Some(devnames));
            }
        }
        Ok(choice)
    }

    /// The printer's name in a DEVNAMES block.
    unsafe fn device_name(handle: HGLOBAL) -> String {
        // SAFETY: a DEVNAMES block from the dialog; its offsets count UTF-16 units from its start.
        unsafe {
            let names = GlobalLock(handle) as *const DEVNAMES;
            if names.is_null() {
                return String::new();
            }
            let start = (names as *const u16).add((*names).wDeviceOffset as usize);
            let name = PCWSTR(start).to_string().unwrap_or_default();
            let _ = GlobalUnlock(handle);
            name
        }
    }

    /// Copies, collation, sides and colour from a DEVMODE block.
    unsafe fn device_mode(handle: HGLOBAL) -> (i32, bool, Option<i16>, bool) {
        // SAFETY: a DEVMODEW block from the dialog, read while locked.
        unsafe {
            let mode = GlobalLock(handle) as *const DEVMODEW;
            if mode.is_null() {
                return (1, true, None, false);
            }
            let mode = &*mode;
            let printer = mode.Anonymous1.Anonymous1;
            let read = (
                i32::from(printer.dmCopies),
                mode.dmCollate == DMCOLLATE_TRUE,
                Some(mode.dmDuplex.0 as i16),
                mode.dmColor == DMCOLOR_MONOCHROME,
            );
            let _ = GlobalUnlock(handle);
            read
        }
    }

    /// A hidden web view on a hidden window, on the print thread.
    struct Page {
        window: HWND,
        environment: ICoreWebView2Environment,
        controller: ICoreWebView2Controller,
        web: ICoreWebView2,
    }

    impl Page {
        fn open() -> Result<Page, String> {
            let window = hidden_window()?;
            let opened = Self::web_view(window);
            if opened.is_err() {
                // SAFETY: the window this thread made.
                unsafe {
                    let _ = DestroyWindow(window);
                }
            }
            opened
        }

        fn web_view(window: HWND) -> Result<Page, String> {
            let folder = HSTRING::from(std::env::temp_dir().join("ebirforms-print-webview2").as_os_str());
            let (tx, rx) = mpsc::channel();
            let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |result, environment| {
                let _ = tx.send(result.ok().and(environment));
                Ok(())
            }));
            // SAFETY: valid strings; the handler is an owned COM object kept until the loader calls it (the wait below).
            unsafe { create_environment(PCWSTR::null(), PCWSTR(folder.as_ptr()), std::ptr::null_mut(), handler.as_raw()) }
                .ok()
                .map_err(|error| format!("WebView2 is not available on this computer: {error}"))?;
            let environment =
                wait_with_pump(rx).map_err(text)?.ok_or("WebView2 is not available on this computer.".to_owned())?;
            let (tx, rx) = mpsc::channel();
            let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |result, controller| {
                let _ = tx.send(result.map(|()| controller));
                Ok(())
            }));
            // SAFETY: a live environment and this thread's window; the handler is kept until it is called.
            unsafe { environment.CreateCoreWebView2Controller(window, &handler) }.map_err(text)?;
            let controller = wait_with_pump(rx)
                .map_err(text)?
                .map_err(|error| format!("WebView2 could not open a web view: {error}"))?
                .ok_or("WebView2 could not open a web view.".to_owned())?;
            // SAFETY: a live controller.
            let web = unsafe {
                let _ = controller.SetBounds(RECT { left: 0, top: 0, right: 794, bottom: 1123 });
                controller.CoreWebView2()
            }
            .map_err(text)?;
            Ok(Page { window, environment, controller, web })
        }

        /// Loads `url` and waits until it has loaded.
        fn load(&self, url: &str) -> Result<(), String> {
            let (tx, rx) = mpsc::channel();
            let handler = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                let mut ok = windows::core::BOOL(0);
                if let Some(args) = args {
                    // SAFETY: the event's live arguments.
                    unsafe {
                        let _ = args.IsSuccess(&mut ok);
                    }
                }
                let _ = tx.send(ok.as_bool());
                Ok(())
            }));
            let mut token = 0i64;
            // SAFETY: a live web view of this thread; the handler is removed once it has answered.
            unsafe {
                self.web.add_NavigationCompleted(&handler, &mut token).map_err(text)?;
                self.web.Navigate(&HSTRING::from(url)).map_err(text)?;
            }
            let loaded = wait_with_pump(rx).map_err(text)?;
            // SAFETY: the token of the handler added above.
            unsafe {
                let _ = self.web.remove_NavigationCompleted(token);
            }
            if loaded { Ok(()) } else { Err("The sheet did not load.".to_owned()) }
        }

        /// The page rule: A4 portrait, no margins, no scaling, backgrounds, no header or footer.
        fn settings(&self) -> Result<ICoreWebView2PrintSettings, String> {
            let environment: ICoreWebView2Environment6 =
                self.environment.cast().map_err(|_| "This WebView2 cannot print; update the WebView2 Runtime.".to_owned())?;
            // SAFETY: a live environment, and setters of the settings object it made.
            unsafe {
                let settings = environment.CreatePrintSettings().map_err(text)?;
                settings.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT).map_err(text)?;
                settings.SetPageWidth(f64::from(PAPER_WIDTH_PT) / 72.0).map_err(text)?;
                settings.SetPageHeight(f64::from(PAPER_HEIGHT_PT) / 72.0).map_err(text)?;
                settings.SetMarginTop(0.0).map_err(text)?;
                settings.SetMarginBottom(0.0).map_err(text)?;
                settings.SetMarginLeft(0.0).map_err(text)?;
                settings.SetMarginRight(0.0).map_err(text)?;
                settings.SetScaleFactor(1.0).map_err(text)?;
                settings.SetShouldPrintBackgrounds(true).map_err(text)?;
                settings.SetShouldPrintHeaderAndFooter(false).map_err(text)?;
                Ok(settings)
            }
        }

        fn close(self) {
            // SAFETY: the controller and window this thread made, closed once.
            unsafe {
                let _ = self.controller.Close();
                let _ = DestroyWindow(self.window);
            }
        }
    }

    unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: the default handling of a message for a window of this class.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    /// A window that is never shown, to host the web view.
    fn hidden_window() -> Result<HWND, String> {
        // SAFETY: registering a class (again is harmless) and making a window of it on this thread.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(text)?;
            let class = w!("EbirformsPrintHost");
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!("eBIRForms print"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                794,
                1123,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|error| format!("Could not make the print window: {error}"))
        }
    }

    /// Runs `job` with a hidden web view on a thread of its own (a single-threaded apartment, as WebView2 needs) and
    /// waits for it.
    fn on_print_thread(job: impl FnOnce(&Page) -> Result<(), String> + Send + 'static) -> Result<(), String> {
        std::thread::Builder::new()
            .name("print".into())
            .spawn(move || {
                // SAFETY: this thread's own apartment, ended below.
                if unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_err() {
                    return Err("Could not start printing (COM).".to_owned());
                }
                let result = Page::open().and_then(|page| {
                    let done = job(&page);
                    page.close();
                    done
                });
                // SAFETY: the apartment begun above.
                unsafe { CoUninitialize() };
                result
            })
            .map_err(|error| format!("Could not start printing: {error}"))?
            .join()
            .map_err(|_| "Printing stopped unexpectedly.".to_owned())?
    }

    /// Whether this process runs in session 0 (a service, or a remote shell such as SSH), which has no desktop:
    /// WebView2 cannot host a web view there (`0x80070578`, an invalid window handle). The app's own windows never
    /// run there.
    #[cfg(test)]
    pub(super) fn in_session_zero() -> bool {
        use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
        let mut session = 0u32;
        // SAFETY: asks for this process's own session.
        unsafe { ProcessIdToSessionId(std::process::id(), &mut session) }.is_ok() && session == 0
    }

    /// Whether the process `pid` is running.
    pub(super) fn process_alive(pid: u32) -> bool {
        use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        // SAFETY: opening a process to ask for its exit code, and closing the handle.
        unsafe {
            let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(process) => process,
                // Gone, or not ours to ask: one we may not open (another account's) counts as running.
                Err(error) => return error.code() == windows::Win32::Foundation::E_ACCESSDENIED,
            };
            let mut code = 0u32;
            let running = GetExitCodeProcess(process, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
            let _ = CloseHandle(process);
            running
        }
    }

}


/// The iOS print sheet: UIKit's UIPrintInteractionController with the preview's PDF (the PDF the `mac` half's hidden
/// WKWebView makes, as on macOS) as its printing item, so the system shows its own page previews, printer choice and
/// copies. UIKit is linked by the NSH host; its classes are looked up at run time.
#[cfg(target_os = "ios")]
mod ios_print {
    use std::sync::mpsc;

    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::{NSData, NSString};

    /// `UIPrintInfoOutputGeneral`: text and graphics.
    const OUTPUT_GENERAL: isize = 0;

    /// Presents the print sheet for `pdf` on the main thread; returns once it closes (or could not come up).
    pub(super) fn present(pdf: Vec<u8>, title: String, request: super::PreviewRequest) -> Result<(), String> {
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        DispatchQueue::main().exec_async(move || {
            // SAFETY: UIKit's documented API, on the main thread. The completion block runs on the main thread once,
            // when the sheet is dismissed.
            let answer = unsafe {
                (|| {
                    request.check()?;
                    let class = AnyClass::get(c"UIPrintInteractionController").ok_or("UIKit printing is missing.")?;
                    let available: Bool = msg_send![class, isPrintingAvailable];
                    if !available.as_bool() {
                        return Err("Printing is not available on this device.".to_owned());
                    }
                    let controller: Option<Retained<AnyObject>> = msg_send![class, sharedPrintController];
                    let controller = controller.ok_or("UIKit gave no print controller.")?;
                    let info_class = AnyClass::get(c"UIPrintInfo").ok_or("UIPrintInfo is missing.")?;
                    let info: Retained<AnyObject> = msg_send![info_class, printInfo];
                    let _: () = msg_send![&info, setOutputType: OUTPUT_GENERAL];
                    let name = NSString::from_str(&title);
                    let _: () = msg_send![&info, setJobName: &*name];
                    let _: () = msg_send![&controller, setPrintInfo: &*info];
                    let data = NSData::with_bytes(&pdf);
                    let _: () = msg_send![&controller, setPrintingItem: &*data];
                    let finished = tx.clone();
                    let done = RcBlock::new(move |_: *mut AnyObject, completed: Bool, error: *mut AnyObject| {
                        let error = error.as_ref().map(|error| {
                            let text: Option<Retained<NSString>> = msg_send![error, localizedDescription];
                            text.map(|text| text.to_string()).unwrap_or_default()
                        });
                        tracing::info!(completed = completed.as_bool(), ?error, "print preview: the iOS print sheet closed");
                        let _ = finished.send(error.map_or(Ok(()), Err));
                    });
                    request.check()?;
                    let shown: Bool = msg_send![&controller, presentAnimated: true, completionHandler: &*done];
                    if shown.as_bool() { Ok(()) } else { Err("The print sheet could not open.".to_owned()) }
                })()
            };
            if let Err(error) = answer {
                let _ = tx.send(Err(error));
            }
        });
        rx.recv().map_err(|_| "The print sheet closed without a result.".to_owned())?
    }

    /// Dismisses the shared print controller's sheet, if it is up; returns at once.
    pub(super) fn dismiss() {
        DispatchQueue::main().exec_async(|| {
            // SAFETY: UIKit's documented API, on the main thread; dismissing a controller that shows nothing does nothing.
            unsafe {
                let Some(class) = AnyClass::get(c"UIPrintInteractionController") else { return };
                let controller: Option<Retained<AnyObject>> = msg_send![class, sharedPrintController];
                if let Some(controller) = controller {
                    let _: () = msg_send![&controller, dismissAnimated: true];
                }
            }
        });
    }
}

/// The macOS half: WebKit and PDFKit through objc2, on the main thread (every AppKit call is posted to the main
/// queue and waited for; callers are worker threads, never the main thread and never the engine's).
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod mac {
    use std::cell::RefCell;
    #[cfg(target_os = "macos")]
    use std::path::Path;
    use std::sync::{Mutex, mpsc};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use dispatch2::DispatchQueue;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    #[cfg(target_os = "ios")]
    use objc2_core_foundation::{CGPoint as NSPoint, CGRect as NSRect, CGSize as NSSize};
    use objc2_foundation::{NSData, NSString, NSURL};
    #[cfg(target_os = "macos")]
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use super::{CSS_PER_PT, PAPER_HEIGHT_PT, PAPER_WIDTH_PT, assemble_pdf, capture_size, for_capture};

    #[link(name = "WebKit", kind = "framework")]
    unsafe extern "C" {}
    #[cfg(target_os = "macos")]
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
    #[cfg(target_os = "macos")]
    pub(super) fn print_operation(path: &str, job_name: &str) -> Result<Retained<AnyObject>, String> {
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
            let job_name = NSString::from_str(job_name);
            let _: () = msg_send![&operation, setJobTitle: &*job_name];
            let _: () = msg_send![&operation, setShowsPrintPanel: Bool::YES];
            Ok(operation)
        }
    }

    /// The system print panel for the PDF at `path`: A4, no scaling, as a sheet of the key window (a panel of
    /// its own when no window is key). Returns once the panel is up.
    #[cfg(target_os = "macos")]
    pub(super) fn print_pdf(path: &Path, form: &str, request: super::PreviewRequest) -> Result<(), String> {
        let path = path.display().to_string();
        let job_name = super::print_job_name(form);
        let (tx, rx) = mpsc::channel();
        DispatchQueue::main().exec_async(move || {
            // SAFETY: AppKit's documented API, on the main thread.
            let answer = unsafe {
                (|| -> Result<(), String> {
                    request.check()?;
                    let operation = print_operation(&path, &job_name)?;
                    let app_class = AnyClass::get(c"NSApplication").ok_or("AppKit has no NSApplication.")?;
                    let app: Retained<AnyObject> = msg_send![app_class, sharedApplication];
                    let key: Option<Retained<AnyObject>> = msg_send![&app, keyWindow];
                    let window = match key {
                        Some(window) => Some(window),
                        None => msg_send![&app, mainWindow],
                    };
                    request.check()?;
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
    #[cfg(target_os = "macos")]
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
async fn pdf_with<F: Future<Output = Result<Vec<u8>, String>>>(
    key: &str,
    render: impl FnOnce(String) -> F,
) -> Result<(Vec<u8>, String, PathBuf, PreviewRequest), String> {
    let (html, title, dir, request) = STATE.with(|state| {
        let state = state.borrow();
        let open = state.previews.get(key)?;
        Some((open.html.clone(), open.title.clone(), open.dir.clone(), open.request.clone()))
    }).ok_or("The preview is closed.")?;
    let pdf = render(html).await?;
    request.check()?;
    Ok((pdf, title, dir, request))
}

async fn pdf_of(key: &str) -> Result<(Vec<u8>, String, PathBuf, PreviewRequest), String> {
    let form = key.to_owned();
    let print_root = print_work_dir();
    pdf_with(key, |html| async move {
        blocking(move || platform_pdf(&form, &html, print_root)).await.map_err(|error| error.to_string())?
    }).await
}

/// The print preview's door to the desktop: what the preview window (`screens/print`) and its host call.
/// NSH has no print API, so the system print panel, the PDF and the live document are this crate's own.
#[module]
pub struct Print;

#[module]
impl Print {
    /// Hears every preview event until the returned function is called: a sheet to open (or to focus), a newer
    /// document for an open one, a device that has no window for it. One host listens at a time; the latest wins.
    pub fn listen(#[args(event)] listener: Callback<PrintEvent>, _system: nsh_core::prelude::System) -> Stop {
        // Android's retired preview workspace was data_dir()/tmp/ebirforms-print-<pid>.
        #[cfg(target_os = "android")]
        if let Some(data_dir) = &_system.data_dir {
            sweep_stale(&data_dir.join("tmp"), process_alive);
        }
        #[cfg(all(not(target_os = "android"), any(unix, all(windows, target_env = "msvc"))))]
        sweep_stale(&std::env::temp_dir(), process_alive);
        let id = install(Rc::new(HostBackend { listener }));
        Stop::new(move || uninstall(id))
    }

    /// The window of the form code `key` closed (or could not open): forgets its preview and deletes its files.
    pub fn closed(key: String, _cx: nsh_core::prelude::CallCx<'_>) {
        #[cfg(target_os = "android")]
        _cx.start(crate::print_android::close());
        forget(&key);
    }

    /// Whether this exact document still belongs to the current session.
    pub fn current(key: String, request_id: String) -> bool {
        STATE.with(|state| state.borrow().previews.get(&key).is_some_and(|open| open.request.id == request_id && open.request.is_live()))
    }

    /// Ends only this request; a late callback must not close its replacement.
    pub fn closed_request(key: String, request_id: String, cx: nsh_core::prelude::CallCx<'_>) {
        if Self::current(key.clone(), request_id) {
            Self::closed(key, cx);
        }
    }

    /// Revokes the session, including requests whose open events have not reached the host yet.
    pub fn reset(_cx: nsh_core::prelude::CallCx<'_>) {
        remove_all();
        #[cfg(target_os = "android")]
        _cx.start(crate::print_android::close());
    }

    /// Prints this exact native-preview request, refusing a late event from a previous session.
    pub async fn print_request(key: String, request_id: String) -> Result<(), String> {
        if !Self::current(key.clone(), request_id) {
            return Err("The preview is closed or was replaced.".to_owned());
        }
        Self::print(key).await
    }

    /// Whether this device previews in the system's print sheet (iOS) instead of a preview window: the host calls
    /// [`Print::print`] when a preview opens, then [`Print::closed`].
    pub fn previews_natively() -> bool {
        previews_natively()
    }

    /// Android uses a native system preview instead of a desktop window.
    pub fn native_preview() -> bool {
        cfg!(target_os = "android")
    }

    /// Opens Android's system print preview for the retained sheet snapshot.
    pub async fn open_native(key: String, title: String, request_id: String) -> Result<(), String> {
        if !Self::current(key.clone(), request_id) {
            return Err("The preview is closed or was replaced.".to_owned());
        }
        #[cfg(target_os = "android")]
        {
            let generation = crate::print_android::generation();
            let (_, _, sheet) = snapshot(&key).ok_or("The print preview is no longer available.")?;
            let request = STATE.with(|state| state.borrow().previews[&key].request.clone());
            // PrintManager.print's job name is this string. It must be the
            // form number only; the taxpayer-bearing window title is not a print name.
            let _ = title;
            crate::print_android::preview(url_for(&sheet), print_job_name(&key), generation, request).await
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (key, title);
            Err("Native system preview is available on Android only.".to_owned())
        }
    }

    /// Takes down the system print sheet if one is up (iOS; elsewhere nothing): a lock or sign-out must not leave a
    /// taxpayer's return on screen.
    pub fn dismiss_native() {
        #[cfg(target_os = "ios")]
        ios_print::dismiss();
    }

    /// Whether this system draws the preview's sheets itself, as pictures (Windows and Linux: on the CPU, see
    /// `sheets.rs`), in place of a web view showing the sheet's page (macOS).
    pub fn draws_sheets() -> bool {
        cfg!(any(windows, target_os = "linux"))
    }

    /// The sheets of `key`'s document as pictures, one per printed page, where [`Print::draws_sheets`] says so. Asked
    /// again after every change of the document: only the sheets that changed are drawn again.
    pub async fn sheets(key: String) -> Result<Vec<SheetImage>, String> {
        let (html, _, _) = snapshot(&key).ok_or("The preview is closed.")?;
        #[cfg(any(windows, target_os = "linux"))]
        {
            blocking(move || crate::sheets::images(&key, &html)).await.map_err(|error| error.to_string())?
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = html;
            Err("This system shows the sheet in a web view.".to_owned())
        }
    }

    /// Prints the preview of `key`. macOS: makes the PDF of its document, then shows the system print panel (A4, no
    /// scaling) as a sheet of the front window, and resolves once the panel is up. Windows: shows the system print
    /// dialog, then prints the document by the PDF's page rule, and resolves once the job is with the printer (or the
    /// dialog was cancelled). iOS: makes the same PDF, then shows the system print sheet with its page previews, and
    /// resolves once the sheet is dismissed or finished. Rejects with a sentence.
    pub async fn print(key: String) -> Result<(), String> {
        logged("Print", print_now(key).await)
    }

    /// Saves the PDF of `key`'s document: asks where with the system save panel and resolves with the path it
    /// wrote, or null when the person cancelled. A path given stands in for the panel in a test environment
    /// (EBIR_TEST_ENV) and is ignored anywhere else.
    pub async fn save_pdf(key: String, test_path: Option<String>) -> Result<Option<String>, String> {
        logged("Save as PDF", save_pdf_now(key, test_path).await)
    }

    /// The person pressed Cancel on the preview's wait for `what` ("Print", "Save as PDF"): a panel that never answered
    /// leaves the wait open for ever, so the preview offers a way out, and the app's log says it was used.
    pub fn stopped_waiting(what: String) {
        tracing::warn!("{what}: the person stopped waiting for it to finish");
    }

    /// What the PDF at `path` holds: pages, size, bytes of drawing, and (macOS) the text the system reads in it.
    /// A probe for tests and checks.
    pub fn inspect_pdf(path: String) -> Result<PdfProbe, String> {
        probe_pdf(Path::new(&path))
    }
}

/// [`Print::print`]'s work.
async fn print_now(key: String) -> Result<(), String> {
    #[cfg(all(windows, target_env = "msvc"))]
    {
        let form = key.clone();
        let print_root = print_work_dir();
        let (html, _, _) = snapshot(&key).ok_or("The preview is closed.")?;
        blocking(move || windows_print::print_document(&form, &html, print_root)).await.map_err(|error| error.to_string())?
    }
    #[cfg(target_os = "linux")]
    {
        let form = key.clone();
        let print_root = print_work_dir();
        let (html, _, _) = snapshot(&key).ok_or("The preview is closed.")?;
        blocking(move || linux_print::print_document(&form, &html, print_root)).await.map_err(|error| error.to_string())?
    }
    #[cfg(not(any(all(windows, target_env = "msvc"), target_os = "linux")))]
    {
        let (pdf, _title, dir, request) = pdf_of(&key).await?;
        let path = write_pdf(&dir, &key, &pdf).map_err(|error| format!("Could not write the PDF: {error}"))?;
        let form = key.clone();
        blocking(move || platform_print(&path, &form, request)).await.map_err(|error| error.to_string())?
    }
}

/// [`Print::save_pdf`]'s work.
async fn save_pdf_now(key: String, test_path: Option<String>) -> Result<Option<String>, String> {
    let (pdf, title, _, request) = pdf_of(&key).await?;
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
    request.check()?;
    std::fs::write(&target, &pdf).map_err(|error| format!("Could not save the PDF: {error}"))?;
    Ok(Some(target.display().to_string()))
}

/// `result` as it is, with a failure also written to the app's log file: the preview shows the person the sentence,
/// the log keeps it for support. `what` names the button ("Print", "Save as PDF"). A cancel is no failure.
fn logged<T>(what: &str, result: Result<T, String>) -> Result<T, String> {
    if let Err(error) = &result {
        tracing::warn!("{what} failed: {error}");
    }
    result
}

fn forget(key: &str) {
    STATE.with(|state| state.borrow_mut().close(key));
    #[cfg(any(windows, target_os = "linux"))]
    crate::sheets::forget(key);
}

/// The app quits: every preview of this engine thread is forgotten and its folder (`ebirforms-print-<pid>`) deleted. The door's
/// `shutdown` calls it; a quit that skips it leaves the folder to the next start's sweep.
pub(crate) fn remove_all() {
    #[cfg(target_os = "android")]
    crate::print_android::invalidate();
    #[cfg(any(windows, target_os = "linux"))]
    crate::sheets::forget_all();
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

    /// Windows: WebView2 makes the PDF of a real sheet document (two Folio form pages scaled onto A4 papers): one A4
    /// page per sheet, with drawing on them.
    #[cfg(all(windows, target_env = "msvc"))]
    #[test]
    fn windows_makes_the_pdf_one_a4_page_per_sheet() {
        if windows_print::in_session_zero() {
            eprintln!("session 0 has no desktop for WebView2: run this test in a desktop session");
            return;
        }
        let html = sheet_doc(&["Juana Santos", "000-123-453-00000"]);
        let pdf = platform_pdf("2551Q", &html, folder.path().join("print")).expect("the PDF");
        let folder = tempfile::tempdir().expect("temp");
        let path = folder.path().join("preview.pdf");
        std::fs::write(&path, &pdf).expect("write");
        let probe = probe_pdf(&path).expect("probe");
        assert_eq!(probe.pages, page_info(&html).pages, "{probe:?}");
        assert!((probe.width_pt - 595.28).abs() < 1.5 && (probe.height_pt - 841.89).abs() < 1.5, "{probe:?}");
        assert!(probe.content_bytes > 0, "blank pages: {probe:?}");
        assert!(probe.text.contains("Juana Santos"), "the form's text: {probe:?}");
    }

    /// Windows: a PDF that is not one A4 page per sheet is refused, as the macOS capture refuses one.
    #[cfg(all(windows, target_env = "msvc"))]
    #[test]
    fn windows_refuses_a_pdf_that_is_not_one_a4_page_per_sheet() {
        let not_a4 = captured_page("x");
        assert!(windows_print::check_a4(&not_a4, 1).is_err(), "a CSS-pixel page is not A4");
        let two = assemble_pdf(&[captured_page("a"), captured_page("b")]).expect("two A4 pages");
        assert_eq!(windows_print::check_a4(&two, 2), Ok(()));
        let why = windows_print::check_a4(&two, 3).expect_err("three sheets, two pages");
        assert!(why.contains("2 pages for 3 sheets"), "{why}");
    }

    /// Windows: the sweep of stale preview folders can tell a running process from a gone one.
    #[cfg(all(windows, target_env = "msvc"))]
    #[test]
    fn windows_tells_a_running_process_from_a_gone_one() {
        assert!(process_alive(std::process::id()));
        // Windows process ids are multiples of four: this one never names a process.
        assert!(!process_alive(4_000_001));
    }


    /// The ToUnicode maps WebView2 writes for its Type3 fonts (one-byte codes, single codes and ranges) read back.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_type3_fonts_unicode_map_is_read() {
        let map = parse_unicode_map(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap 1 begincodespacerange <00> <FF> endcodespacerange \
             9 beginbfchar <03> <0020> <37> <0054> <44> <0061> <46> <0063> <48> <0065> <4A> <0067> <4F> <006C> <51> <006E> \
             <55> <0072> endbfchar 3 beginbfrange <33> <35> <0050> <57> <58> <0074> <5B> <5C> <0078> \
             <60> <61> [<0041> <0042>] endbfrange endcmap",
        );
        assert_eq!(map.width, 1);
        let read = |codes: &[u32]| codes.iter().filter_map(|c| map.codes.get(c).cloned()).collect::<String>();
        assert_eq!(read(&[0x37, 0x48, 0x5B, 0x57]), "Text");
        assert_eq!(read(&[0x33, 0x34, 0x35, 0x03, 0x60, 0x61]), "PQR AB");
    }

    /// Windows and Linux: the sheets of a real document come out as one picture each, drawn on the CPU, and an edit
    /// draws only the sheet it changed again.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn the_sheets_are_drawn_as_pictures_and_an_edit_redraws_only_its_sheet() {
        let html = sheet_doc(&["Juana Santos", "000-123-453-00000"]);
        let started = std::time::Instant::now();
        let first = crate::sheets::images("pictures-test", &html).expect("pictures");
        let opened = started.elapsed();
        assert_eq!(first.len() as u32, page_info(&html).pages);
        for image in &first {
            assert!(image.src.starts_with("data:image/png;base64,"), "{}", &image.src[..40.min(image.src.len())]);
            // A4 at 96 CSS pixels per inch, the paper style's 1 pt under the page aside.
            assert!((image.width - 793.7).abs() < 2.0 && (image.height - 1122.5).abs() < 14.0, "{} x {}", image.width, image.height);
            assert!(image.src.len() > 20_000, "a blank picture: {} bytes", image.src.len());
        }
        // The same document again: nothing is drawn, the pictures are the same.
        assert_eq!(crate::sheets::images("pictures-test", &html).expect("again"), first);
        // An edit on the first sheet: its picture changes, the second sheet's does not.
        let edited = html.replacen("Juana Santos", "Maria Clara", 1);
        assert_ne!(edited, html, "the fixture holds the name");
        let started = std::time::Instant::now();
        let second = crate::sheets::images("pictures-test", &edited).expect("edited");
        eprintln!("sheets: opened in {opened:?}, edit redrawn in {:?}", started.elapsed());
        assert_eq!(second.len(), first.len());
        assert_eq!(second.iter().zip(&first).filter(|(now, before)| now.src != before.src).count(), 1);
        crate::sheets::forget("pictures-test");
    }

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
    fn windows_paths_become_file_urls_webview2_loads() {
        assert_eq!(
            file_url(r"C:\Users\x\AppData\Local\Temp\ebirforms-print-1\2551Q\sheet.html", true),
            "file:///C:/Users/x/AppData/Local/Temp/ebirforms-print-1/2551Q/sheet.html"
        );
        assert_eq!(file_url(r"C:\Users\ADMINI~1\a b\é.html", true), "file:///C:/Users/ADMINI~1/a%20b/%C3%A9.html");
        assert_eq!(file_url(r"\\?\D:\x\sheet.html", true), "file:///D:/x/sheet.html");
        assert_eq!(file_url(r"\\server\share\x\sheet.html", true), "file://server/share/x/sheet.html");
        assert_eq!(file_url(r"\\?\UNC\server\share\sheet.html", true), "file://server/share/sheet.html");
        // Not on Windows: a backslash is part of a name, and a path is not rewritten.
        assert_eq!(file_url(r"/tmp/a\b.html", false), "file:///tmp/a%5Cb.html");
        assert_eq!(file_url("/tmp/a b/é.html", false), "file:///tmp/a%20b/%C3%A9.html");
    }

    /// The folder previews are written to, as this system names it, is a URL with nothing of the path encoded but
    /// what a URL needs.
    #[cfg(windows)]
    #[test]
    fn the_preview_folder_is_a_loadable_file_url_here() {
        let url = url_for(&std::env::temp_dir().join("ebirforms-print-1").join("sheet.html"));
        assert!(url.starts_with("file:///") && url.as_bytes().get(9) == Some(&b':'), "{url}");
        assert!(!url.contains("%5C") && !url.contains("%3A"), "{url}");
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
        let preview_dir = state.previews["2551Q"].dir.clone();
        let sheet = state.previews["2551Q"].sheet.clone();
        assert_eq!(mode_of(&preview_dir), 0o700, "the folder of the preview");
        assert_eq!(mode_of(&sheet), 0o600, "the sheet");
        state.update("2551Q", sheet_doc(&["Santos"]));
        assert_eq!(mode_of(&sheet), 0o600, "the sheet rewritten by an edit");
        let pdf = write_pdf(&preview_dir, "2551Q", b"%PDF-1.4").unwrap();
        let pdf_name = pdf.file_name().unwrap().to_string_lossy();
        assert!(pdf_name.starts_with("2551Q-") && pdf_name.ends_with(".pdf"), "{pdf_name}");
        assert!(pdf.parent() == Some(preview_dir.as_path()));
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

    /// A listener that starts before the door names a scratch database must
    /// still put the first artifact beside that database, never in the real
    /// installed app's folder.
    #[test]
    fn the_live_preview_root_is_chosen_when_the_first_artifact_is_written() {
        let scratch = tempfile::tempdir().unwrap();
        let database = scratch.path().join("room").join("bir_data.db");
        bir_core::db::set_database_path_override(Some(database));
        let mut state = State::new(PathBuf::new());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        let sheet = state.previews["2551Q"].sheet.clone();
        assert!(sheet.starts_with(scratch.path().join("room").join("previews").join("print")), "{sheet:?}");
        bir_core::db::set_database_path_override(None);
        state.close_all();
    }

    #[test]
    fn preview_and_pdf_file_names_use_the_form_and_a_random_id_never_the_taxpayer() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("previews").join("print");
        let mut state = State::new(root.clone());
        state
            .request("2551Q", "BIR Form 2551Q — Juana Santos".to_owned(), sheet_doc(&["Juana Santos"]))
            .unwrap();
        let entries: Vec<_> = std::fs::read_dir(&root).unwrap().map(|entry| entry.unwrap().path()).collect();
        assert_eq!(entries.len(), 1);
        let preview_dir = &entries[0];
        let dir_name = preview_dir.file_name().unwrap().to_string_lossy();
        assert!(dir_name.starts_with("2551Q-"), "{dir_name}");
        assert_ne!(dir_name, "2551Q");
        assert!(!dir_name.contains("Juana"), "{dir_name}");
        let sheets: Vec<_> = std::fs::read_dir(preview_dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        assert_eq!(sheets.len(), 1);
        assert!(sheets[0].starts_with("2551Q-"), "{}", sheets[0]);
        assert!(sheets[0].ends_with("-sheet.html"), "{}", sheets[0]);
        assert!(!sheets[0].contains("Juana"), "{}", sheets[0]);

        let pdf = write_pdf(preview_dir, "2551Q", b"%PDF-1.4").unwrap();
        let pdf_name = pdf.file_name().unwrap().to_string_lossy();
        assert!(pdf_name.starts_with("2551Q-"), "{pdf_name}");
        assert!(pdf_name.ends_with(".pdf"), "{pdf_name}");
        assert!(!pdf_name.contains("Juana"), "{pdf_name}");
        assert!(!pdf_name.contains("Santos"), "{pdf_name}");
        assert_eq!(print_job_name("2551Q"), "2551Q", "the OS print job name carries no taxpayer text");
    }

    #[cfg(any(all(windows, target_env = "msvc"), target_os = "linux"))]
    #[test]
    fn print_job_files_live_under_previews_and_use_the_form_and_a_random_id() {
        assert_eq!(print_work_dir().parent().and_then(|path| path.file_name()).unwrap(), "previews");
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("previews").join("print");
        let titled = sheet_doc(&["Juana Santos"]).replace("<head>", "<head><title>BIR Form 2551Q — Juana Santos</title>");
        let (folder, sheet) = engine_print::job_folder_in(&root, "2551Q", &titled).unwrap();
        let folder_name = folder.file_name().unwrap().to_string_lossy();
        assert!(folder_name.starts_with("job-2551Q-"), "{folder_name}");
        let sheet_name = sheet.file_name().unwrap().to_string_lossy();
        assert!(sheet_name.starts_with("2551Q-"), "{sheet_name}");
        assert!(sheet_name.ends_with("-sheet.html"), "{sheet_name}");
        assert!(!sheet_name.contains("Juana"), "{sheet_name}");
        let printed = std::fs::read_to_string(&sheet).unwrap();
        assert!(printed.contains("<title>2551Q</title>"), "browser print job names use the form only");
        assert!(!printed.contains("<title>BIR Form 2551Q"), "a taxpayer-bearing sheet title does not reach the print document");
        assert_eq!(printed.matches("<title>").count(), 1);
        #[cfg(unix)]
        {
            assert_eq!(mode_of(&folder), 0o700, "the print-job folder is private");
            assert_eq!(mode_of(&sheet), 0o600, "the print-job sheet is private");
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn quitting_deletes_every_preview_folder() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("root");
        let mut state = State::new(root.clone());
        state.request("2551Q", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        state.request("1601C", "t".to_owned(), sheet_doc(&["a"])).unwrap();
        let pdf_dir = state.previews["1601C"].dir.clone();
        write_pdf(&pdf_dir, "1601C", b"%PDF").unwrap();
        state.close_all();
        assert!(!root.exists(), "the run's folder, every preview and PDF in it");
        assert!(state.previews.is_empty());
        // The hook's thread state too, through what the door's shutdown calls.
        let _recorder = host(folder.path());
        let key = key();
        request(sheet_file(folder.path(), &["a"]), "t".to_owned(), key.clone());
        assert!(std::fs::read_dir(folder.path()).unwrap().next().is_some(), "the preview artifact folder exists");
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
        assert!(!source.exists(), "the filled return is not left in the previews folder");
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
    fn a_pdf_in_flight_never_survives_a_session_change_or_same_form_replacement() {
        for session_change in [false, true] {
            let folder = tempfile::tempdir().unwrap();
            STATE.with(|state| {
                *state.borrow_mut() = State::new(folder.path().to_path_buf());
                state.borrow_mut().request("2551Q", "Old taxpayer".into(), sheet_doc(&["old"])).unwrap();
            });
            futures::executor::block_on(async {
                let (send, receive) = futures::channel::oneshot::channel();
                let mut pending = Box::pin(pdf_with("2551Q", |html| async move {
                    assert!(html.contains("old"));
                    receive.await.unwrap();
                    Ok(vec![1, 2, 3])
                }));
                assert!(futures::poll!(pending.as_mut()).is_pending());
                let old = STATE.with(|state| state.borrow().previews["2551Q"].request.clone());
                STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if session_change { state.close_all(); }
                    state.request("2551Q", "New taxpayer".into(), sheet_doc(&["new"])).unwrap();
                    assert_ne!(old.id, state.previews["2551Q"].request.id);
                });
                send.send(()).unwrap();
                assert!(pending.await.unwrap_err().contains("closed or was replaced"));
                assert!(!old.is_live(), "a queued main-thread presentation also refuses the old request");
                let (pdf, title, _, current) = pdf_with("2551Q", |_| async { Ok(vec![4]) }).await.unwrap();
                assert_eq!(pdf, vec![4]);
                assert_eq!(title, "New taxpayer");
                assert!(current.is_live());
                STATE.with(|state| state.borrow_mut().close_all());
                assert!(current.check().is_err(), "revocation after PDF completion still prevents presentation");
            });
        }
    }

    #[test]
    fn missing_previews_and_failed_pdf_generation_do_not_produce_a_document() {
        let folder = tempfile::tempdir().unwrap();
        STATE.with(|state| *state.borrow_mut() = State::new(folder.path().to_path_buf()));
        futures::executor::block_on(async {
            assert!(pdf_with("2551Q", |_| async { panic!("a missing preview must not be rendered") }).await.is_err());
            STATE.with(|state| { state.borrow_mut().request("2551Q", "t".into(), sheet_doc(&["x"])).unwrap(); });
            assert_eq!(pdf_with("2551Q", |_| async { Err("PDF failed".into()) }).await.unwrap_err(), "PDF failed");
            STATE.with(|state| state.borrow_mut().close_all());
        });
    }

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
        let file = state.previews["2551Q"].sheet.clone();
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
        let on_disk = std::fs::read_to_string(&state.previews["2551Q"].sheet).unwrap();
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
        let first_dir = state.previews["2551Q"].dir.clone();
        let second_dir = state.previews["1601C"].dir.clone();
        state.close("2551Q");
        assert!(!first_dir.exists() && second_dir.exists());
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

    #[cfg(not(target_os = "android"))]
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
        assert_eq!(has_live(&key), !cfg!(target_os = "android"));
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

    #[cfg(not(target_os = "android"))]
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

    #[cfg(target_os = "android")]
    #[test]
    fn android_keeps_a_snapshot_until_closed_the_next_request_or_shutdown() {
        let folder = tempfile::tempdir().unwrap();
        let recorder = host(&folder.path().join("previews"));
        let first = key();
        request(sheet_file(folder.path(), &["a"]), "First".into(), first.clone());
        let first_path = STATE.with(|state| state.borrow().sheet_path(&first));
        assert!(first_path.exists());
        assert!(!has_live(&first));
        let generation = crate::print_android::generation();
        let cleanup = crate::print_android::close();
        forget(&first);
        assert_ne!(crate::print_android::generation(), generation, "revoked before Java cleanup runs");
        assert!(!first_path.exists());
        assert!(futures::executor::block_on(Print::open_native(first, "Closed".into(), "closed-request".into())).is_err());
        futures::executor::block_on(cleanup);

        let second = key();
        request(sheet_file(folder.path(), &["b"]), "Second".into(), second.clone());
        assert!(!first_path.exists());
        let second_path = STATE.with(|state| state.borrow().sheet_path(&second));
        assert!(second_path.exists());
        assert_eq!(kinds(&recorder), vec![PrintEventKind::Open, PrintEventKind::Open]);

        let empty = folder.path().join("empty.html");
        std::fs::write(&empty, "").unwrap();
        request(empty, "Empty".into(), key());
        assert_eq!(recorder.events.borrow().last().unwrap().kind, PrintEventKind::Unavailable);
        request(folder.path().join("missing.html"), "Missing".into(), key());
        assert_eq!(recorder.events.borrow().last().unwrap().kind, PrintEventKind::Unavailable);
        assert!(futures::executor::block_on(Print::open_native(key(), "Missing".into(), "missing-request".into())).is_err());
        remove_all();
        assert!(!second_path.exists());
    }

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

    #[test]
    fn a_failed_print_or_save_is_written_to_the_log_and_a_cancel_is_not() {
        let folder = tempfile::tempdir().unwrap();
        // The app's own default filter (door.rs install_logging): this crate at info.
        let options = bir_core::logging::InstallOptions {
            developer_mode: false,
            default_filter: "nsh_app_ebirforms=info,bir_app_api=info,bir_core=info,panic=error",
        };
        temp_env::with_vars([("BIR_LOG_DIR", Some(folder.path().as_os_str())), ("RUST_LOG", None)], || {
            bir_core::logging::with_thread_subscriber(options, || {
                let failed: Result<(), String> = logged("Print", Err("The printer Office is not available.".into()));
                assert_eq!(failed, Err("The printer Office is not available.".to_owned()));
                let cancelled: Result<Option<String>, String> = logged("Save as PDF", Ok(None));
                assert_eq!(cancelled, Ok(None));
                let saved = logged("Save as PDF", Ok(Some("C:/x.pdf".to_owned())));
                assert_eq!(saved, Ok(Some("C:/x.pdf".to_owned())));
                let _: Result<Option<String>, String> =
                    logged("Save as PDF", Err("The save panel could not open: no owner window".into()));
                Print::stopped_waiting("Save as PDF".to_owned());
            });
        });
        let log: String = std::fs::read_dir(folder.path())
            .unwrap()
            .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
            .collect();
        let line = |needle: &str| {
            log.lines().find(|line| line.contains(needle)).unwrap_or_else(|| panic!("{needle:?} not in {log}"))
        };
        assert!(line("Print failed: The printer Office is not available.").contains("WARN"));
        assert!(line("Save as PDF failed: The save panel could not open: no owner window").contains("WARN"));
        assert!(line("Save as PDF: the person stopped waiting for it to finish").contains("WARN"));
        assert_eq!(log.matches(" failed: ").count(), 2, "{log}");
    }
}
