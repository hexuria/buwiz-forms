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
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;

pub(crate) struct FrozenHtmlPreviewView {
    webview: Option<Entity<WebView>>,
    /// Only failures are worth a line in the toolbar.
    status: Option<String>,
}

/// The one paper the preview prints on, in points. The forms are drawn on
/// 8.5 × 13 in long bond, which nobody keeps in a printer tray; every sheet
/// is scaled to fit A4 instead, and what is on screen is what comes out.
pub(crate) const PAPER_WIDTH_PT: f32 = 595.28;
pub(crate) const PAPER_HEIGHT_PT: f32 = 841.89;
/// Kept clear at the top and bottom of every sheet for the printer's
/// unprintable edge.
const SAFETY_PT: f32 = 10.0;

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

/// Scale factor that fits a native sheet onto the paper, inside the safety
/// edge. Never enlarges.
fn fit_scale(native: (f32, f32)) -> f32 {
    let sx = (PAPER_WIDTH_PT - 2.0 * SAFETY_PT) / native.0;
    let sy = (PAPER_HEIGHT_PT - 2.0 * SAFETY_PT) / native.1;
    sx.min(sy).min(1.0)
}

/// Stylesheet that turns the document into paper-sized sheets, each holding
/// one scaled form page, for screen and print alike.
///
/// Two WebKit facts shape this. Its print layout width follows the widest
/// content unless `body` has an explicit width, and the whole document is
/// then scaled to the paper width — so a 612pt page zoomed to 537pt was
/// scaled back up, overflowed A4, and printed as two pages (the "6 of 6").
/// Pinning `html,body` to the paper width stops that. And a block exactly
/// as tall as the page still spills a fraction of a point onto a blank
/// page, so the sheet is a point shorter than the paper.
///
/// Anything outside the sheets (a bundle's "Guidelines and Instructions"
/// link, in any form) is hidden: it would not print, so showing it only
/// confuses.
fn fit_css(native: (f32, f32)) -> String {
    let scale = fit_scale(native);
    format!(
        "@page{{size:{w:.2}pt {h:.2}pt;margin:0}}\
html,body{{width:{w:.2}pt;margin:0 auto;padding:0;background:#fff}}\
body>:not(.sheet){{display:none !important}}\
.sheet{{position:relative;width:{w:.2}pt;height:{sheet_h:.2}pt;overflow:hidden;background:#fff;padding-top:{safety:.0}pt;break-after:page;page-break-after:always}}\
.sheet:last-of-type{{break-after:auto;page-break-after:auto}}\
.sheet-flow{{height:auto;min-height:{sheet_h:.2}pt;overflow:visible}}\
.sheet>.page{{zoom:{scale:.4};margin:0 auto;break-after:auto;page-break-after:auto}}",
        w = PAPER_WIDTH_PT,
        h = PAPER_HEIGHT_PT,
        sheet_h = PAPER_HEIGHT_PT - 1.0,
        safety = SAFETY_PT,
    )
}

fn with_fit_style(html: &str, css: &str) -> String {
    let tag = format!("<style id=\"preview-paper\">{css}</style>");
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], tag, &html[at..]),
        None => format!("{tag}{html}"),
    }
}

/// Wrap every top-level `.page` in a `.sheet` before first paint. The
/// receipt page is the one page that may run long (a mail body), so its
/// sheet is allowed to grow and paginate normally.
fn with_sheets(html: &str) -> String {
    const SCRIPT: &str = "<script>(function(){\
var pages=document.querySelectorAll('body>.page');\
for(var i=0;i<pages.length;i++){var p=pages[i];var s=document.createElement('div');\
s.className='sheet'+(p.classList.contains('page-receipt')?' sheet-flow':'');\
p.parentNode.insertBefore(s,p);s.appendChild(p);}})();</script>";
    match html.rfind("</body>") {
        Some(at) => format!("{}{}{}", &html[..at], SCRIPT, &html[at..]),
        None => format!("{html}{SCRIPT}"),
    }
}

impl FrozenHtmlPreviewView {
    pub(crate) fn new(html: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let native_sheet = document_sheet_size(&html).unwrap_or((612.0, 936.0));
        let html = with_screen_canvas(&html, cx.theme().secondary);
        let html = with_fit_style(&html, &fit_css(native_sheet));
        let html = with_sheets(&html);
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
            // The sheets are laid out for A4; a different default paper in
            // the shared print info would re-paginate them. wry prints from
            // that same shared object, so set it first.
            #[cfg(target_os = "macos")]
            {
                let info = objc2_app_kit_modern::NSPrintInfo::sharedPrintInfo();
                info.setPaperSize(objc2_foundation_modern::NSSize::new(
                    f64::from(PAPER_WIDTH_PT),
                    f64::from(PAPER_HEIGHT_PT),
                ));
                info.setOrientation(objc2_app_kit_modern::NSPaperOrientation::Portrait);
            }
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
html,body{{background:{canvas} !important;width:auto}}\
body{{padding:24pt 0}}\
.sheet{{margin:0 auto 24pt auto;box-shadow:0 1pt 6pt rgba(0,0,0,.28)}}\
.sheet:last-of-type{{margin-bottom:0}}\
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
                        <div text_color={cx.theme().muted_foreground}>{"Fits A4"}</div>
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
        PAPER_HEIGHT_PT, PAPER_WIDTH_PT, css_color, document_sheet_size, fit_css, fit_scale,
        with_fit_style, with_screen_canvas, with_sheets,
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
    fn long_sheet_fits_a4_by_height_and_nothing_is_enlarged() {
        let k = fit_scale((612.0, 936.0));
        assert!((k - (PAPER_HEIGHT_PT - 20.0) / 936.0).abs() < 1e-4, "{k}");
        assert_eq!(
            fit_scale((400.0, 500.0)),
            1.0,
            "small sheets keep their size"
        );
        let k_wide = fit_scale((1000.0, 500.0));
        assert!(
            (k_wide - (PAPER_WIDTH_PT - 20.0) / 1000.0).abs() < 1e-4,
            "{k_wide}"
        );
    }

    #[test]
    fn fit_stylesheet_pins_the_body_to_a4_and_scales_pages_inside_sheets() {
        let css = fit_css((612.0, 936.0));
        assert!(
            css.contains("@page{size:595.28pt 841.89pt;margin:0}"),
            "{css}"
        );
        assert!(css.contains("html,body{width:595.28pt;"), "{css}");
        assert!(
            css.contains("body>:not(.sheet){display:none !important}"),
            "{css}"
        );
        assert!(
            css.contains(".sheet{position:relative;width:595.28pt;height:840.89pt;"),
            "a point shorter than the paper: {css}"
        );
        assert!(
            css.contains(".sheet-flow{height:auto;min-height:840.89pt;"),
            "{css}"
        );
        let zoom = format!(
            ".sheet>.page{{zoom:{:.4};margin:0 auto;",
            fit_scale((612.0, 936.0))
        );
        assert!(css.contains(&zoom), "{css}");
        let doc = with_fit_style("<html><head></head><body></body></html>", &css);
        assert!(doc.find("<style id=\"preview-paper\">").unwrap() < doc.find("</head>").unwrap());
    }

    #[test]
    fn sheet_wrapper_script_runs_before_the_body_closes() {
        let doc = with_sheets("<html><body><div class=\"page\"></div></body></html>");
        let script = doc.find("<script>").expect("script");
        assert!(script < doc.find("</body>").unwrap());
        assert!(script > doc.find("class=\"page\"").unwrap());
        assert!(doc.contains("'sheet'+(p.classList.contains('page-receipt')?' sheet-flow':'')"));
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
        assert!(doc.contains(".sheet{margin:0 auto 24pt auto"));
        assert_eq!(css_color(grey), "hsla(0.0deg,0.0%,92.0%,1.000)");
    }
}
