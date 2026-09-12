//! Fill/print host for frozen HTML documents.
//!
//! This is a thin WebView around `bir_print::frozen_html::filled_document`.

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_rsx::rsx;
use gpui_wry::WebView;
use raw_window_handle::HasWindowHandle;
#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;

pub(crate) struct FrozenHtmlPreviewView {
    webview: Option<Entity<WebView>>,
    /// Only failures are worth a line in the toolbar.
    status: Option<String>,
}

impl FrozenHtmlPreviewView {
    pub(crate) fn new(html: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let html = with_screen_canvas(&html, cx.theme().secondary);
        let result = window
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|window_handle| {
                let builder = wry::WebViewBuilder::new();
                #[cfg(target_os = "windows")]
                let builder = builder
                    .with_browser_accelerator_keys(false)
                    .with_default_context_menus(false);
                builder
                    .with_incognito(true)
                    .with_html(html)
                    .build_as_child(&window_handle)
                    .map_err(|error| error.to_string())
            });

        let (webview, status) = match result {
            Ok(webview) => (Some(cx.new(|cx| WebView::new(webview, window, cx))), None),
            Err(error) => (None, Some(format!("Print preview failed: {error}"))),
        };

        Self { webview, status }
    }

    /// The platform's print dialog through wry's native `print()`
    /// (`NSPrintOperation` on macOS, WebView2's print UI on Windows,
    /// `WebKitPrintOperation` on Linux). `window.print()` from script was
    /// the previous route; WKWebView ignores it, so the button did nothing on
    /// macOS. Script stays as the fallback if the native call errors.
    fn print(&mut self, cx: &mut Context<Self>) {
        let Some(webview) = self.webview.clone() else {
            self.status = Some("Nothing to print: the preview did not load.".to_string());
            cx.notify();
            return;
        };
        let outcome = webview.update(cx, |webview, _| {
            webview
                .raw()
                .print()
                .or_else(|_| webview.raw().evaluate_script("window.print();"))
        });
        self.status = outcome
            .err()
            .map(|error| format!("Could not open the print dialog: {error}"));
        cx.notify();
    }
}

/// On screen, show the sheets as sheets: centred white pages on the same
/// grey as the toolbar, with a gap so page breaks are visible. The bundle's
/// own CSS paints `html, body` white and stacks pages at the left edge,
/// which reads as one endless white surface. Print is untouched — the
/// `@media screen` block does not apply there.
fn with_screen_canvas(html: &str, canvas: gpui::Hsla) -> String {
    let css = format!(
        "<style id=\"preview-canvas\">@media screen{{\
html,body{{background:{canvas} !important}}\
body{{padding:24pt 0}}\
.page{{margin:0 auto 24pt auto;box-shadow:0 1pt 6pt rgba(0,0,0,.28)}}\
.page:last-of-type{{margin-bottom:0}}\
}}</style>",
        canvas = css_color(canvas)
    );
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], css, &html[at..]),
        None => format!("{css}{html}"),
    }
}

fn css_color(color: gpui::Hsla) -> String {
    format!(
        "hsla({:.1}deg,{:.1}%,{:.1}%,{:.3})",
        color.h * 360.0,
        color.s * 100.0,
        color.l * 100.0,
        color.a
    )
}

impl Render for FrozenHtmlPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_print = self.webview.is_some();
        rsx! {
            <div
                size_full
                flex
                flex_col
                bg={cx.theme().background}
                text_color={cx.theme().foreground}
            >
                <div
                    h={px(48.)}
                    px_3
                    flex
                    items_center
                    justify_between
                    bg={cx.theme().secondary}
                    text_sm
                    text_color={cx.theme().foreground}
                    border_b_1
                    border_color={cx.theme().border}
                >
                    <div text_color={cx.theme().danger}>{self.status.clone().unwrap_or_default()}</div>
                    <div flex items_center gap_2>
                        {Button::new("frozen-html-print")
                            .label("Print")
                            .primary()
                            .disabled(!can_print)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.print(cx);
                            }))}
                    </div>
                </div>
                <div
                    flex_1
                    min_h_0
                    whenSome={(self.webview.clone(), |this, webview| this.child(webview))}
                    when={(self.webview.is_none(), |this| {
                        this.p_6().child(self.status.clone().unwrap_or_default())
                    })}
                />
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{css_color, with_screen_canvas};

    #[test]
    fn screen_canvas_is_injected_into_head_and_scoped_to_screen() {
        let grey = gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.92,
            a: 1.0,
        };
        let doc = with_screen_canvas(
            "<html><head><title>x</title></head><body><div class=\"page\"></div></body></html>",
            grey,
        );
        let style = doc.find("<style id=\"preview-canvas\">").expect("style");
        let head_end = doc.find("</head>").expect("head");
        assert!(style < head_end, "injected inside <head>");
        assert!(doc.contains("@media screen{"));
        assert!(doc.contains("hsla(0.0deg,0.0%,92.0%,1.000)"));
        assert!(doc.contains(".page{margin:0 auto 24pt auto"));
        assert_eq!(css_color(grey), "hsla(0.0deg,0.0%,92.0%,1.000)");
    }
}
