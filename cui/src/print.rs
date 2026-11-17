//! Print preview (track D3) — the honest port for this build.
//!
//! The NSH app's `print.rs` (document management, live updates, the WKWebView PDF path and the
//! system print panel) is macOS-only even there: other systems answer "not available" honestly
//! (`UNAVAILABLE`). CUI has no second window and no HTML renderer, so every platform is in that
//! boat here — but the contract is kept exactly: `request` still reads and deletes the sheet file
//! the host function handed over (it is this host's to dispose of), and the UI hears the same
//! "not available" sentence the NSH page would toast.
//!
//! What is NOT stubbed silently: the unavailable notice is queued for the frame to toast, so the
//! user gets the same feedback NSH gives on Linux.

use std::path::PathBuf;
use std::time::Instant;

use bir_app_api::hooks::PreviewProducer;

/// The sentence of a device that cannot open a preview (the same one NSH answers).
pub const UNAVAILABLE: &str = "Print preview is not available on this device.";

thread_local! {
    /// Preview requests that could not be shown, waiting for the frame to say so (key, title).
    static UNAVAILABLE_NOTICES: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// `print_preview_request`: the sheet file is this host's to dispose of — read once, deleted at
/// once, whatever comes next (nsh/src/print.rs does the same before any platform check). Then the
/// device says it cannot show a preview.
pub(crate) fn request(path: PathBuf, title: String, key: String) {
    let _html = std::fs::read_to_string(&path);
    if let Err(error) = std::fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "print preview: cannot delete the source sheet {}: {error}",
            path.display()
        );
    }
    UNAVAILABLE_NOTICES.with(|notices| notices.borrow_mut().push((key, title)));
}

/// Whether a preview for `key` is open and follows edits: none can be.
pub(crate) fn has_live(_key: &str) -> bool {
    false
}

/// Hand the open preview for `key` a new document: no preview is open, so this is a no-op (as in
/// NSH when the key was never opened).
pub(crate) fn live_update(_key: &str, _produce: PreviewProducer, _started: Instant) {}

/// The print previews' files go with the run: none are kept.
pub(crate) fn remove_all() {
    UNAVAILABLE_NOTICES.with(|notices| notices.borrow_mut().clear());
}

/// What the preview screens read each frame: which previews are open, and the "not available"
/// sentences owed to the user.
#[derive(Debug, Clone, Default)]
pub struct PreviewStates {
    /// The preview keys currently open.
    pub open: Vec<String>,
    /// Requests that found no preview device, with their titles — drained by the frame.
    pub unavailable: Vec<(String, String)>,
}

/// The previews' state for the UI. `unavailable` is drained: each notice is said once.
pub(crate) fn states() -> PreviewStates {
    PreviewStates {
        open: Vec::new(),
        unavailable: UNAVAILABLE_NOTICES.with(|notices| std::mem::take(&mut *notices.borrow_mut())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_disposes_the_sheet_and_says_unavailable_once() {
        let dir = std::env::temp_dir().join(format!("cui-print-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sheet = dir.join("sheet.html");
        std::fs::write(&sheet, "<html>sheet</html>").unwrap();

        request(
            sheet.clone(),
            "2551Q 2024 Q1".to_owned(),
            "2551Q".to_owned(),
        );
        // The file is this host's to dispose of: gone whether or not a preview could show.
        assert!(!sheet.exists());

        let first = states();
        assert_eq!(first.unavailable.len(), 1);
        assert_eq!(first.unavailable[0].0, "2551Q");
        // Drained: the next poll does not say it again.
        assert!(states().unavailable.is_empty());
        assert!(!has_live("2551Q"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ---------------------------------------------------------------- titles.ts (pure preview words)

/// The window id of a preview: the form code and the return it was asked on. Another return of
/// the same form is another window (windowId).
pub fn window_id(key: &str, identity: Option<&str>) -> String {
    match identity {
        Some(id) if !id.is_empty() => {
            let clean: String = id
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            format!("print:{key}:{clean}")
        }
        _ => format!("print:{key}"),
    }
}

/// The window title: the host function's "BIR Form 2551Q — Juana Santos" with the period after
/// the code (previewTitle).
pub fn preview_title(host_title: &str, period: Option<&str>) -> String {
    let Some(period) = period.filter(|p| !p.is_empty()) else {
        return host_title.to_owned();
    };
    // /^(.*?)\s+—\s+(.*)$/
    if let Some(pos) = host_title.find(" — ") {
        let before = &host_title[..pos];
        let rest = &host_title[pos + " — ".len()..];
        if !rest.is_empty() {
            return format!("{before} · {period} — {rest}");
        }
    }
    format!("{host_title} · {period}")
}

/// What the toolbar says: "A4 · 4 pages · Folio sheet at 88%" (pageSummary).
pub fn page_summary(paper: &str, pages: i64, sheet: &str, zoom: f64) -> String {
    let pages_text = format!("{pages} {}", if pages == 1 { "page" } else { "pages" });
    let scaled = if zoom < 0.995 {
        format!(" · {sheet} sheet at {}%", (zoom * 100.0).round() as i64)
    } else {
        String::new()
    };
    format!("{paper} · {pages_text}{scaled}")
}

/// Which return a route shows, as far as a preview cares (formIdentity): code, taxpayer, year,
/// period and whether it is a Form library sample. `target` is the route's `target` param.
pub fn form_identity(view: &str, target: Option<&serde_json::Value>) -> Option<String> {
    if view != "form" {
        return None;
    }
    let t = target?;
    if !t.is_object() {
        return None;
    }
    let field = |k: &str| -> String {
        t.get(k)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    let mode = if field("mode") == "sample" {
        "sample"
    } else {
        "return"
    };
    Some(format!(
        "{}|{}|{}|{}|{}",
        field("formCode"),
        field("tin"),
        field("year"),
        field("period"),
        mode
    ))
}

/// The two ways to size the sheet (ZoomMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomMode {
    Fit,
    Full,
}

/// The script that paints the page's desk in `desk` and sizes the sheets: "100" is their size,
/// "fit" the page's width less 48, between 0.3 and 2.5 times (chromeScript).
pub fn chrome_script(mode: ZoomMode, desk: &str, dark: bool) -> String {
    let shadow = if dark {
        "0 6px 22px rgba(0,0,0,.6)"
    } else {
        "0 1px 3px rgba(0,0,0,.2),0 6px 18px rgba(0,0,0,.16)"
    };
    let css =
        format!("html{{background:{desk} !important}}.sheet{{box-shadow:{shadow} !important}}");
    let mode_json = match mode {
        ZoomMode::Fit => "\"fit\"".to_owned(),
        ZoomMode::Full => "\"100\"".to_owned(),
    };
    let css_json = css.replace('\\', "\\\\").replace('"', "\\\"");
    let script = concat!(
        "(function(m,css){var d=document.documentElement;",
        "var s=document.getElementById(\"preview-chrome\");",
        "if(!s){s=document.createElement(\"style\");s.id=\"preview-chrome\";document.head.appendChild(s);}",
        "s.textContent=css;d.style.zoom=\"1\";",
        "var sh=document.querySelector(\".sheet\");",
        "if(m===\"fit\"&&sh){var w=sh.getBoundingClientRect().width;if(w>0){d.style.zoom=String(Math.max(0.3,Math.min(2.5,(window.innerWidth-48)/w)));}}",
        "return \"chrome\";",
        "})("
    );
    format!("{script}{mode_json},\"{css_json}\");")
}

/// The file name of a path, for "Saved Juana.pdf" (baseName).
pub fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod titles_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn t_window_id() {
        assert_eq!(window_id("2551Q", None), "print:2551Q");
        let a = window_id("2551Q", Some("2551Q|00012345300000|2026|3|return"));
        assert_eq!(a, "print:2551Q:2551Q-00012345300000-2026-3-return");
        for other in [
            "2551Q|00012345300000|2026|2|return",
            "2551Q|00012345300001|2026|3|return",
            "2551Q|00012345300000|2025|3|return",
            "2551Q|00012345300000|2026|3|sample",
        ] {
            assert_ne!(window_id("2551Q", Some(other)), a);
        }
    }

    #[test]
    fn t_preview_title() {
        assert_eq!(
            preview_title("BIR Form 2551Q — Juana Santos", Some("Q1 2026")),
            "BIR Form 2551Q · Q1 2026 — Juana Santos"
        );
        assert_eq!(
            preview_title("BIR Form 2551Q", Some("Q1 2026")),
            "BIR Form 2551Q · Q1 2026"
        );
        assert_eq!(
            preview_title("BIR Form 2551Q — Juana Santos", None),
            "BIR Form 2551Q — Juana Santos"
        );
        assert_eq!(
            preview_title("BIR Form 2551Q — Juana Santos", Some("")),
            "BIR Form 2551Q — Juana Santos"
        );
    }

    #[test]
    fn t_page_summary() {
        assert_eq!(
            page_summary("A4", 4, "Folio", 0.8782),
            "A4 · 4 pages · Folio sheet at 88%"
        );
        assert_eq!(page_summary("A4", 1, "A4", 1.0), "A4 · 1 page");
    }

    #[test]
    fn t_form_identity() {
        let target = json!({"formCode":"2551Q","tin":"00012345300000","year":2026,"period":3,"mode":"return"});
        assert_eq!(
            form_identity("form", Some(&target)).as_deref(),
            Some("2551Q|00012345300000|2026|3|return")
        );
        let amend = json!({"formCode":"2551Q","tin":"00012345300000","year":2026,"period":3,"mode":"amend"});
        let saved = json!({"formCode":"2551Q","tin":"00012345300000","year":2026,"period":3,"mode":"return","key":5});
        assert_eq!(
            form_identity("form", Some(&amend)),
            form_identity("form", Some(&saved))
        );
        let sample = json!({"formCode":"2551Q","tin":"00012345300000","year":2026,"period":3,"mode":"sample"});
        assert_ne!(
            form_identity("form", Some(&sample)),
            form_identity("form", Some(&target))
        );
        let other = json!({"formCode":"2551Q","tin":"00012345300000","year":2026,"period":2,"mode":"return"});
        assert_ne!(
            form_identity("form", Some(&other)),
            form_identity("form", Some(&target))
        );
        assert_eq!(form_identity("home", Some(&json!({}))), None);
        assert_eq!(form_identity("form", None), None);
        assert_eq!(
            form_identity("form", Some(&json!({}))),
            Some("||||return".to_owned())
        );
    }

    #[test]
    fn t_chrome_script() {
        let fit = chrome_script(ZoomMode::Fit, "#e2e2e7", false);
        assert!(fit.contains("\"fit\""));
        assert!(fit.contains("html{background:#e2e2e7 !important}"));
        assert!(!fit.contains(".sheet{background"));
        assert!(fit.contains("window.innerWidth-48"));
        let full = chrome_script(ZoomMode::Full, "#0e0e10", true);
        assert!(fit.starts_with("(function(m,css){"));
        assert!(fit.ends_with(");"));
        assert!(full.contains("\"100\""));
        assert!(full.contains("rgba(0,0,0,.6)"));
    }

    #[test]
    fn t_base_name() {
        assert_eq!(
            base_name("/Users/x/Documents/BIR Form 2551Q.pdf"),
            "BIR Form 2551Q.pdf"
        );
        assert_eq!(base_name("C:\\Users\\x\\a.pdf"), "a.pdf");
        assert_eq!(base_name("a.pdf"), "a.pdf");
    }
}
