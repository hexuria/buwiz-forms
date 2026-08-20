//! Shared launcher for immutable HTML form-preview envelopes.
//!
//! Form views own draft editing and envelope construction. This module owns
//! the platform-specific preview host so adding another HTML form does not
//! copy the macOS, Windows, and Linux window orchestration.

use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::html::RenderEnvelopeV1;
use gpui::Context;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use gpui::{AppContext, TitlebarOptions, WindowBounds, WindowOptions, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HtmlPreviewLaunchKind {
    NativeWindow,
    LinuxEmbedded,
    LinuxGtkTopLevel,
    FrozenHtmlWindow,
    FrozenHtmlDocument,
}

impl HtmlPreviewLaunchKind {
    pub(crate) const fn status_message(self) -> &'static str {
        match self {
            Self::NativeWindow => "HTML print preview opened.",
            Self::LinuxEmbedded => "HTML preview opened in the X11 child WebView.",
            Self::LinuxGtkTopLevel => {
                "HTML preview opened in the application-owned GTK/WebKit window."
            }
            Self::FrozenHtmlWindow => "Frozen 2551Q HTML opened for fill/print.",
            Self::FrozenHtmlDocument => {
                "Frozen 2551Q HTML opened as a local document for fill/print."
            }
        }
    }
}

pub(crate) fn launch_html_form_preview<T: 'static>(
    envelope: &RenderEnvelopeV1,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let prepared = super::html_form_preview::prepare_html_form_preview(envelope)
            .map_err(|error| error.to_string())?;
        let options = preview_window_options(envelope, cx);
        cx.open_window(options, move |window, cx| {
            cx.new(|cx| super::html_form_preview::HtmlFormPreviewView::new(prepared, window, cx))
        })
        .map_err(|error| format!("the preview window could not be opened: {error}"))?;
        Ok(HtmlPreviewLaunchKind::NativeWindow)
    }

    #[cfg(target_os = "linux")]
    {
        match super::linux_html_preview::launch_linux_html_preview(envelope)
            .map_err(|error| error.to_string())?
        {
            super::linux_html_preview::LinuxHtmlPreviewLaunch::GtkTopLevel => {
                Ok(HtmlPreviewLaunchKind::LinuxGtkTopLevel)
            }
            super::linux_html_preview::LinuxHtmlPreviewLaunch::Embedded(prepared) => {
                let options = preview_window_options(envelope, cx);
                cx.open_window(options, move |window, cx| {
                    cx.new(|cx| {
                        super::linux_html_preview::LinuxEmbeddedHtmlPreviewView::new(
                            *prepared, window, cx,
                        )
                    })
                })
                .map_err(|error| {
                    format!("the Linux preview window could not be opened: {error}")
                })?;
                Ok(HtmlPreviewLaunchKind::LinuxEmbedded)
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (envelope, cx);
        Err("the native HTML host is unavailable on this platform".to_string())
    }
}

pub(crate) fn launch_frozen_2551q_preview<T: 'static>(
    draft: &Form2551QDraft,
    cx: &mut Context<T>,
) -> Result<HtmlPreviewLaunchKind, String> {
    let html = bir_print::frozen_html::filled_2551q_document(draft);

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let options = frozen_preview_window_options(cx);
        let opened = cx
            .open_window(options, {
                let html = html.clone();
                move |window, cx| {
                    cx.new(|cx| {
                        super::frozen_html_preview::FrozenHtmlPreviewView::new(html, window, cx)
                    })
                }
            })
            .map_err(|error| format!("the frozen 2551Q window could not be opened: {error}"));
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
        let _ = cx;
        let path = write_frozen_html_document(&html)?;
        open::that(&path)
            .map_err(|error| format!("frozen 2551Q HTML could not be opened: {error}"))?;
        Ok(HtmlPreviewLaunchKind::FrozenHtmlDocument)
    }
}

fn write_frozen_html_document(html: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("buwiz-2551q-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create frozen 2551Q temp dir: {error}"))?;
    let path = dir.join("index.html");
    std::fs::write(&path, html)
        .map_err(|error| format!("could not write frozen 2551Q HTML: {error}"))?;
    Ok(path)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn frozen_preview_window_options<T: 'static>(cx: &mut Context<T>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1200.), px(900.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some("2551Q Frozen HTML".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn preview_window_options<T: 'static>(
    envelope: &RenderEnvelopeV1,
    cx: &mut Context<T>,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1200.), px(900.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(format!("{} HTML Form Preview", envelope.form.code).into()),
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
            HtmlPreviewLaunchKind::NativeWindow.status_message(),
            "HTML print preview opened."
        );
        assert!(
            HtmlPreviewLaunchKind::LinuxEmbedded
                .status_message()
                .contains("X11 child")
        );
        assert!(
            HtmlPreviewLaunchKind::LinuxGtkTopLevel
                .status_message()
                .contains("application-owned GTK/WebKit")
        );
        assert_eq!(
            HtmlPreviewLaunchKind::FrozenHtmlWindow.status_message(),
            "Frozen 2551Q HTML opened for fill/print."
        );
        assert!(
            HtmlPreviewLaunchKind::FrozenHtmlDocument
                .status_message()
                .contains("local document")
        );
    }
}
