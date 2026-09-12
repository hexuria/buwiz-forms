//! Fill/print host for frozen HTML documents.
//!
//! This is a thin WebView around `bir_print::frozen_html::filled_document`.

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
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
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;

pub(crate) struct FrozenHtmlPreviewView {
    webview: Option<Entity<WebView>>,
    /// Only failures are worth a line in the toolbar.
    status: Option<String>,
    /// The document's own sheet in points (`@page { size }`), e.g. 612×936.
    native_sheet: (f32, f32),
    paper: Entity<ComboboxState>,
}

/// Paper sizes the preview can fit the sheets to, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Paper {
    pub label: &'static str,
    pub width: f32,
    pub height: f32,
}

pub(crate) const PAPERS: [Paper; 4] = [
    Paper {
        label: "Long 8.5 × 13 in",
        width: 612.0,
        height: 936.0,
    },
    Paper {
        label: "A4",
        width: 595.28,
        height: 841.89,
    },
    Paper {
        label: "Letter 8.5 × 11 in",
        width: 612.0,
        height: 792.0,
    },
    Paper {
        label: "Legal 8.5 × 14 in",
        width: 612.0,
        height: 1008.0,
    },
];

fn paper_by_label(label: &str) -> Option<Paper> {
    PAPERS.iter().copied().find(|paper| paper.label == label)
}

/// The paper whose size matches `(width, height)` within a couple of points.
fn paper_matching(width: f32, height: f32) -> Option<Paper> {
    PAPERS
        .iter()
        .copied()
        .find(|paper| (paper.width - width).abs() < 3.0 && (paper.height - height).abs() < 3.0)
}

/// The system's default paper on macOS (`NSPrintInfo.sharedPrintInfo`),
/// falling back to A4 where that is unavailable or unmatched.
fn default_paper() -> Paper {
    #[cfg(target_os = "macos")]
    {
        let size = objc2_app_kit_modern::NSPrintInfo::sharedPrintInfo().paperSize();
        if let Some(paper) = paper_matching(size.width as f32, size.height as f32) {
            return paper;
        }
    }
    PAPERS[1]
}

/// `@page { size:612pt 936pt }` from the document's inlined form CSS.
fn document_sheet_size(html: &str) -> Option<(f32, f32)> {
    let at = html.find("@page")?;
    let rest = &html[at..];
    let size_at = rest.find("size:")?;
    let rest = &rest[size_at + "size:".len()..];
    let end = rest.find([';', '}'])?;
    let mut parts = rest[..end].split_whitespace();
    let w = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    let h = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    Some((w, h))
}

/// Scale factor that fits a native sheet onto `paper` with a small safety
/// margin for the printer's unprintable edge. Never enlarges.
fn fit_scale(native: (f32, f32), paper: Paper) -> f32 {
    const SAFETY_PT: f32 = 10.0;
    let sx = (paper.width - 2.0 * SAFETY_PT) / native.0;
    let sy = (paper.height - 2.0 * SAFETY_PT) / native.1;
    sx.min(sy).min(1.0)
}

/// Stylesheet that fits every sheet to `paper`, on screen and in print, and
/// hides anything outside the sheets (a bundle's "Guidelines and
/// Instructions" link, in any form) — it would not print, so showing it
/// only confuses. Chromium hosts honour the `@page` size in their print dialog;
/// WebKit takes the paper from the print panel, so the scale is what matters.
fn paper_css(native: (f32, f32), paper: Paper) -> String {
    let scale = fit_scale(native, paper);
    format!(
        "@page{{size:{w:.2}pt {h:.2}pt;margin:0}}\
body>:not(.page){{display:none !important}}@media print{{html,body{{background:#fff !important}}body{{padding:0 !important}}.page{{margin:0 !important;box-shadow:none !important}}}}\
.page{{zoom:{scale:.4}}}",
        w = paper.width,
        h = paper.height,
    )
}

fn with_paper_style(html: &str, css: &str) -> String {
    let tag = format!("<style id=\"preview-paper\">{css}</style>");
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], tag, &html[at..]),
        None => format!("{tag}{html}"),
    }
}

impl FrozenHtmlPreviewView {
    pub(crate) fn new(html: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let native_sheet = document_sheet_size(&html).unwrap_or((612.0, 936.0));
        let paper_default = default_paper();
        let html = with_screen_canvas(&html, cx.theme().secondary);
        let html = with_paper_style(&html, &paper_css(native_sheet, paper_default));
        let paper = cx.new(|cx| {
            ComboboxState::new(
                PAPERS.iter().map(|paper| paper.label.to_string()).collect(),
                6,
                window,
                cx,
            )
        });
        paper.update(cx, |state, cx| {
            state.set_selected_value(paper_default.label, window, cx)
        });
        cx.subscribe(&paper, |this: &mut Self, _, _: &ComboboxEvent, cx| {
            this.apply_paper(cx);
        })
        .detach();
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

        Self {
            webview,
            status,
            native_sheet,
            paper,
        }
    }

    /// Re-fit the sheets to the paper picked in the toolbar, in place.
    fn apply_paper(&mut self, cx: &mut Context<Self>) {
        let label = self.paper.read(cx).selected_value(cx);
        let Some(paper) = paper_by_label(&label) else {
            return;
        };
        let Some(webview) = self.webview.clone() else {
            return;
        };
        let css = paper_css(self.native_sheet, paper);
        let script = format!(
            "(function(){{var s=document.getElementById('preview-paper');if(s){{s.textContent={};}}}})();",
            serde_json::to_string(&css).unwrap_or_else(|_| "\"\"".to_string())
        );
        let outcome = webview.update(cx, |webview, _| webview.raw().evaluate_script(&script));
        if let Err(error) = outcome {
            self.status = Some(format!("Could not change the paper size: {error}"));
        }
        cx.notify();
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
            // Zero margins: the sheets already carry the form's own margins,
            // and the paper fit above leaves a safety edge.
            #[cfg(target_os = "macos")]
            let native = webview.raw().print_with_options(&wry::PrintOptions {
                margins: wry::PrintMargin {
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 0.0,
                },
            });
            #[cfg(not(target_os = "macos"))]
            let native = webview.raw().print();
            native.or_else(|_| webview.raw().evaluate_script("window.print();"))
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
                    <div flex items_center gap_3>
                        <div text_color={cx.theme().muted_foreground}>{"Paper"}</div>
                        <div w_48>{Combobox::new(&self.paper)}</div>
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
    use super::{
        PAPERS, css_color, document_sheet_size, fit_scale, paper_css, paper_matching,
        with_paper_style, with_screen_canvas,
    };

    #[test]
    fn sheet_size_is_read_from_the_inlined_form_css() {
        assert_eq!(
            document_sheet_size("<style>@page { size:612pt 936pt;margin:0 }</style>"),
            Some((612.0, 936.0))
        );
        assert_eq!(document_sheet_size("<style>.page{}</style>"), None);
    }

    #[test]
    fn long_sheet_fits_a4_and_letter_by_height_and_never_grows() {
        let long = (612.0, 936.0);
        let k_a4 = fit_scale(long, PAPERS[1]);
        let k_letter = fit_scale(long, PAPERS[2]);
        assert!((k_a4 - (841.89 - 20.0) / 936.0).abs() < 1e-4, "{k_a4}");
        assert!(
            (k_letter - (792.0 - 20.0) / 936.0).abs() < 1e-4,
            "{k_letter}"
        );
        assert_eq!(
            fit_scale(long, PAPERS[3]),
            1.0,
            "larger paper does not enlarge"
        );
        assert!(paper_matching(595.0, 842.0).is_some());
        assert!(paper_matching(500.0, 700.0).is_none());
    }

    #[test]
    fn paper_stylesheet_scales_pages_and_hides_non_pages_in_print() {
        let css = paper_css((612.0, 936.0), PAPERS[1]);
        assert!(
            css.contains("@page{size:595.28pt 841.89pt;margin:0}"),
            "{css}"
        );
        assert!(
            css.contains("body>:not(.page){display:none !important}"),
            "{css}"
        );
        assert!(
            !css.contains("@media print{body>:not(.page)"),
            "non-page content is hidden on screen as well as in print"
        );
        assert!(css.contains(".page{zoom:0.8780}"), "{css}");
        let doc = with_paper_style("<html><head></head><body></body></html>", &css);
        assert!(doc.find("<style id=\"preview-paper\">").unwrap() < doc.find("</head>").unwrap());
    }

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
