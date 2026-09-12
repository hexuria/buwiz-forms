//! Shared launcher for frozen HTML fill/print previews.
//!
//! Form views own draft editing and writer maps. This module owns the
//! platform-specific WebView host.

use bir_core::db::Database;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::frozen_html::ReceiptPage;
use gpui::Context;
use gpui::WindowHandle;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use gpui::{AppContext, TitlebarOptions, WindowBounds, WindowOptions, px, size};

#[derive(Debug, Clone)]
pub(crate) enum HtmlPreviewLaunchKind {
    /// An owned preview window; the handle lets the form view follow its
    /// lifetime (the status line goes away when the window does).
    FrozenHtmlWindow(WindowHandle<super::frozen_html_preview::FrozenHtmlPreviewView>),
    /// Written to a temp file and handed to the system browser.
    FrozenHtmlDocument,
}

impl HtmlPreviewLaunchKind {
    pub(crate) const fn status_message(&self) -> &'static str {
        match self {
            Self::FrozenHtmlWindow(_) => "Print preview opened.",
            Self::FrozenHtmlDocument => "Print preview opened as a local document.",
        }
    }

    /// Run `on_close` on the launching view once the preview window has been
    /// closed. A document opened externally cannot be followed; nothing runs.
    pub(crate) fn observe_close<T: 'static>(
        &self,
        cx: &mut Context<T>,
        on_close: impl FnOnce(&mut T, &mut Context<T>) + 'static,
    ) {
        let Self::FrozenHtmlWindow(handle) = self else {
            return;
        };
        let Ok(view) = handle.update(cx, |_, _, cx| cx.entity()) else {
            return;
        };
        cx.observe_release(&view, move |this, _, cx| on_close(this, cx))
            .detach();
    }
}

pub(crate) fn launch_frozen_form_preview<T: 'static>(
    slug: &str,
    fields: &BTreeMap<String, String>,
    title: &str,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    launch_frozen_form_preview_with_receipt(slug, fields, None, title, cx)
}

/// Form pages plus, for a confirmed return, BIR's receipt as the last page.
pub(crate) fn launch_frozen_form_preview_with_receipt<T: 'static>(
    slug: &str,
    fields: &BTreeMap<String, String>,
    receipt: Option<ReceiptPage>,
    title: &str,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    let html =
        bir_print::frozen_html::filled_document_with_receipt(slug, fields, receipt.as_ref())?;
    launch_frozen_html(html, title, cx)
}

pub(crate) fn launch_frozen_2551q_preview<T: 'static>(
    draft: &Form2551QDraft,
    receipt: Option<ReceiptPage>,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    launch_frozen_form_preview_with_receipt(
        "2551q-2018",
        &draft.to_bir_field_map(),
        receipt,
        "2551Q — Print Preview",
        cx,
    )
}

/// The receipt row a confirmed draft points at, shaped for the print page.
/// `None` for anything not confirmed or whose row is gone.
pub(crate) fn receipt_page_for(
    db: &Arc<Mutex<Database>>,
    receipt_id: Option<i64>,
    mailbox: &str,
) -> Option<ReceiptPage> {
    let receipt_id = receipt_id?;
    let receipt = db
        .lock()
        .ok()?
        .get_submission_receipt_by_id(receipt_id)
        .ok()
        .flatten()?;
    Some(ReceiptPage {
        filename: receipt.filename.clone(),
        subject: "Tax Return Receipt Confirmation".to_string(),
        from: receipt
            .source_from
            .clone()
            .unwrap_or_else(|| "ebirforms-noreply@bir.gov.ph".to_string()),
        to: mailbox.to_string(),
        received_at: bir_core::background_cron::display_received_at(
            &receipt.received_date,
            &receipt.received_time,
        ),
        email_received_at: receipt
            .email_received_at
            .as_deref()
            .and_then(bir_core::receipt::display_email_received_at),
        body_text: receipt.raw_text.clone(),
        body_html: receipt
            .raw_html
            .as_deref()
            .map(bir_core::receipt::sanitized_receipt_html),
    })
}

fn launch_frozen_html<T: 'static>(
    html: String,
    title: &str,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let options = frozen_preview_window_options(title, cx);
        let opened = cx
            .open_window(options, {
                let html = html.clone();
                move |window, cx| {
                    cx.new(|cx| {
                        super::frozen_html_preview::FrozenHtmlPreviewView::new(html, window, cx)
                    })
                }
            })
            .map_err(|error| format!("the frozen HTML window could not be opened: {error}"));
        match opened {
            Ok(handle) => {
                let generation = handle
                    .update(cx, |view, _, _| view.generation())
                    .unwrap_or(0);
                super::frozen_html_preview::register_live_preview(handle, generation);
                Ok(HtmlPreviewLaunchKind::FrozenHtmlWindow(handle))
            }
            Err(error) => {
                let path = write_frozen_html_document(&html)?;
                open::that(&path).map_err(|open_error| {
                    format!("{error}; also failed to open {path:?}: {open_error}")
                })?;
                Ok(HtmlPreviewLaunchKind::FrozenHtmlDocument)
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (title, cx);
        let path = write_frozen_html_document(&html)?;
        open::that(&path).map_err(|error| format!("frozen HTML could not be opened: {error}"))?;
        Ok(HtmlPreviewLaunchKind::FrozenHtmlDocument)
    }
}

fn write_frozen_html_document(html: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("buwiz-frozen-html-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create frozen HTML temp dir: {error}"))?;
    let path = dir.join("index.html");
    std::fs::write(&path, html).map_err(|error| format!("could not write frozen HTML: {error}"))?;
    Ok(path)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn frozen_preview_window_options<T: 'static>(title: &str, cx: &mut Context<T>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1200.), px(900.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_kinds_report_the_owned_host_that_opened() {
        assert!(
            HtmlPreviewLaunchKind::FrozenHtmlDocument
                .status_message()
                .contains("local document")
        );
    }

    #[gpui::test]
    fn a_window_launch_reports_the_owned_host(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::theme::init);
        let handle = cx.add_window(|window, cx| {
            super::super::frozen_html_preview::FrozenHtmlPreviewView::new(
                "<html><body><div class=\"page\"></div></body></html>".to_string(),
                window,
                cx,
            )
        });
        assert_eq!(
            HtmlPreviewLaunchKind::FrozenHtmlWindow(handle).status_message(),
            "Print preview opened."
        );
    }
}
