//! Shared launcher for frozen HTML fill/print previews.
//!
//! Form views own draft editing and writer maps. This module owns the
//! platform-specific WebView host.

use bir_core::db::Database;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::frozen_html::ReceiptPage;
use gpui::Context;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use gpui::{AppContext, TitlebarOptions, WindowBounds, WindowOptions, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HtmlPreviewLaunchKind {
    FrozenHtmlWindow,
    FrozenHtmlDocument,
}

impl HtmlPreviewLaunchKind {
    pub(crate) const fn status_message(self) -> &'static str {
        match self {
            Self::FrozenHtmlWindow => "Print preview opened.",
            Self::FrozenHtmlDocument => "Print preview opened as a local document.",
        }
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
            Ok(_) => Ok(HtmlPreviewLaunchKind::FrozenHtmlWindow),
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
        assert_eq!(
            HtmlPreviewLaunchKind::FrozenHtmlWindow.status_message(),
            "Print preview opened."
        );
        assert!(
            HtmlPreviewLaunchKind::FrozenHtmlDocument
                .status_message()
                .contains("local document")
        );
    }
}
