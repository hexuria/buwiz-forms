//! Fill/print host for frozen 2551Q HTML.
//!
//! This is a thin WebView around `bir_print::frozen_html::filled_2551q_document`.
//! It does not use the semantic form-renderer, so visual-contract Chromium
//! refs stay on the existing Rust provider.

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
    status: String,
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
            Ok(webview) => (
                Some(cx.new(|cx| WebView::new(webview, window, cx))),
                "Frozen 2551Q HTML ready to fill/print.".to_string(),
            ),
            Err(error) => (None, format!("Frozen 2551Q HTML preview failed: {error}")),
        };

        Self { webview, status }
    }

    fn print(&mut self, cx: &mut Context<Self>) {
        let Some(webview) = self.webview.clone() else {
            self.status = "Frozen 2551Q HTML preview is not available to print.".to_string();
            cx.notify();
            return;
        };
        let outcome = webview.update(cx, |webview, _| {
            webview.raw().evaluate_script("window.print();")
        });
        match outcome {
            Ok(()) => {
                self.status = "Print dialog opened for the frozen 2551Q HTML.".to_string();
            }
            Err(error) => {
                self.status = format!("Frozen 2551Q HTML could not print: {error}");
            }
        }
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
                    {self.status.clone()}
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
                        this.p_6().child(self.status.clone())
                    })}
                />
            </div>
        }
    }
}
