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
