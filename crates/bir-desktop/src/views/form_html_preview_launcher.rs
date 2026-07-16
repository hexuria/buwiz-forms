//! Shared launcher for immutable HTML form-preview envelopes.
//!
//! Form views own draft editing and envelope construction. This module owns
//! the platform-specific preview host so adding another HTML form does not
//! copy the macOS, Windows, and Linux window orchestration.

use bir_print::html::RenderEnvelopeV1;
use gpui::Context;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use gpui::{AppContext, TitlebarOptions, WindowBounds, WindowOptions, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HtmlPreviewLaunchKind {
    NativeWindow,
    LinuxEmbedded,
    LinuxGtkTopLevel,
}

impl HtmlPreviewLaunchKind {
    pub(crate) const fn status_message(self) -> &'static str {
        match self {
            Self::NativeWindow => "HTML print preview opened.",
            Self::LinuxEmbedded => "HTML preview opened in the X11 child WebView.",
            Self::LinuxGtkTopLevel => {
                "HTML preview opened in the application-owned GTK/WebKit window."
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
    }
}
