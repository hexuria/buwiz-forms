//! A4 print layout for the frozen BIR sheets.
//!
//! Ported from the retired painted preview's A4 fit implementation. Each
//! `.page` sits in a paper-sized `.sheet` (the wrappers are
//! written by [`crate::static_html`]), the page is scaled to fit A4, and
//! html/body are pinned to the paper width so WebKit doesn't re-scale the
//! print layout. Without that pin a 4-page form printed as 8 pages.
//!
//! The scale rule depends on the engine the host prints through. WebKit
//! (macOS WKWebView, iOS, Linux WebKitGTK) measures text at the zoomed size
//! (a 7pt run at zoom 0.88 quantizes wider than at zoom 1), so a page laid
//! out at sub-1 zoom does not have the widths the sheet was written with and
//! its small runs overflow into the next column: WebKit gets
//! `transform:scale`, which paints the natural layout scaled. Chromium
//! (Windows WebView2, Android WebView) scales text metrics linearly with
//! `zoom` and has no such fault — and its PDF writer loses the text layer
//! under a transform — so Chromium keeps `zoom`.

pub const PAPER_WIDTH_PT: f32 = 595.28;
pub const PAPER_HEIGHT_PT: f32 = 841.89;
const SAFETY_PT: f32 = 10.0;

/// The sheet's native `@page size` (e.g. 612pt × 936pt folio).
fn document_sheet_size(html: &str) -> Option<(f32, f32)> {
    let rest = &html[html.find("@page")?..];
    let rest = &rest[rest.find("size:")? + 5..];
    let end = rest.find([';', '}'])?;
    let mut parts = rest[..end].split_whitespace();
    let w = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    let h = parts.next()?.trim_end_matches("pt").parse::<f32>().ok()?;
    Some((w, h))
}

fn fit_scale(native: (f32, f32)) -> f32 {
    let sx = (PAPER_WIDTH_PT - 2.0 * SAFETY_PT) / native.0;
    let sy = (PAPER_HEIGHT_PT - 2.0 * SAFETY_PT) / native.1;
    sx.min(sy).min(1.0)
}

fn fit_css(native: (f32, f32)) -> String {
    let scale = fit_scale(native);
    let page_rule = page_scale_rule(native, scale);
    format!(
        "@page{{size:{w:.2}pt {h:.2}pt;margin:0}}\
html,body{{width:{w:.2}pt;margin:0 auto;padding:0;background:#fff}}\
body>:not(.sheet){{display:none !important}}\
.sheet{{position:relative;width:{w:.2}pt;height:{sheet_h:.2}pt;box-sizing:border-box;overflow:hidden;background:#fff;padding-top:{safety:.0}pt;break-after:page;page-break-after:always}}\
.sheet:last-of-type{{break-after:auto;page-break-after:auto}}\
.sheet-flow{{height:auto;min-height:{sheet_h:.2}pt;overflow:visible}}\
@media print{{.sheet-flow{{min-height:0}}}}\
{page_rule}",
        w = PAPER_WIDTH_PT,
        h = PAPER_HEIGHT_PT,
        sheet_h = PAPER_HEIGHT_PT - 1.0,
        safety = SAFETY_PT,
    )
}

/// The `.sheet>.page` scale rule for the engine this binary prints through
/// (the host is compiled per system). WebKit needs `transform:scale`; a
/// transform keeps the page's native box, so it is centred as `margin:0
/// auto` did, and its bottom edge is pulled up by what the scale shrinks so
/// a `.sheet-flow` wraps the painted size. Chromium keeps `zoom`.
fn page_scale_rule(native: (f32, f32), scale: f32) -> String {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        format!(
            ".sheet>.page{{zoom:{scale:.4};margin:0 auto;break-after:auto;page-break-after:auto}}"
        )
    } else {
        let left = ((PAPER_WIDTH_PT - native.0 * scale) / 2.0).max(0.0);
        let bottom = (1.0 - scale) * native.1;
        format!(
            ".sheet>.page{{transform:scale({scale:.4});transform-origin:0 0;margin:0 0 -{bottom:.4}pt {left:.4}pt;break-after:auto;page-break-after:auto}}"
        )
    }
}

/// Screen-only extras (grey desk, gap and shadow between sheets, page label).
/// Inside `@media screen` and limited to `.sheet` margins/shadow, never its
/// height, so the print layout is exactly `fit_css`.
const SCREEN_CSS: &str = "@media screen{\
html{width:auto !important;background:#5f6368 !important}\
body{background:transparent !important;padding:24px 0 8px !important}\
.sheet{margin:0 auto 32px auto;box-shadow:0 6px 22px rgba(0,0,0,.45)}\
.sheet::after{content:attr(data-preview-label);position:absolute;right:8px;bottom:4px;\
font:600 9px -apple-system,sans-serif;color:#888;pointer-events:none}}";

/// Add the A4 paper CSS so the filled sheet prints one A4 page per form page.
/// Run [`crate::static_html::staticize`] on the result to get the `.sheet`
/// wrappers and the script-free markup.
pub fn prepare_html(html: &str) -> String {
    let native = document_sheet_size(html).unwrap_or((612.0, 936.0));
    let style = format!(
        "<style id=\"preview-paper\">{}{}</style>",
        fit_css(native),
        SCREEN_CSS
    );
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], style, &html[at..]),
        None => format!("{style}{html}"),
    }
}

/// Per-form override layer: `print-overrides/<slug>.css`, read at render time
/// and injected after the frozen CSS for that slug only. The frozen HTML is
/// never edited.
///
/// Lookup order:
/// 1. `BIR_PRINT_OVERRIDES` — tests and overrides always win.
/// 2. The packaged layout beside the running executable
///    ([`crate::packaged`]): `Contents/Resources/print-overrides` in a macOS
///    bundle, `print-overrides/` beside the executable in a Windows/Linux
///    install or an unpacked archive, and `/usr/share/ebirforms/print-overrides`
///    for deb/rpm/pacman installs.
/// 3. Development checkout `<repo>/print-overrides` — debug builds only,
///    because a shipped binary must not resolve a path baked in at build time.
///
/// On Android there is no such folder: [`override_css`] reads the copy compiled
/// into the library ([`embedded`]) after `BIR_PRINT_OVERRIDES`.
pub fn overrides_dir() -> std::path::PathBuf {
    overrides_dir_from(
        std::env::var_os("BIR_PRINT_OVERRIDES"),
        std::env::current_exe().ok(),
    )
}

fn overrides_dir_from(
    env_override: Option<std::ffi::OsString>,
    executable: Option<std::path::PathBuf>,
) -> std::path::PathBuf {
    if let Some(dir) = env_override {
        return std::path::PathBuf::from(dir);
    }
    if let Some(dir) = crate::packaged::dir(
        "print-overrides",
        executable.as_deref(),
        crate::packaged::directory_present,
    ) {
        return dir;
    }
    dev_overrides_dir()
}

/// The checkout's `print-overrides/` for development runs and tests.
#[cfg(debug_assertions)]
fn dev_overrides_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../print-overrides")
}

/// No packaged `print-overrides/` was found: the location it would have
/// occupied beside the executable (which simply reads as "no override").
#[cfg(not(debug_assertions))]
fn dev_overrides_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("print-overrides")))
        .unwrap_or_else(|| std::path::PathBuf::from("print-overrides"))
}

/// The per-form stylesheets compiled into the library: Android's only source,
/// since an Android app has no folder beside its executable (build.rs).
#[cfg(any(test, target_os = "android"))]
pub(crate) mod embedded {
    include!(concat!(env!("OUT_DIR"), "/print_overrides.rs"));

    /// `slug`'s stylesheet, if `print-overrides/` had one when the library was built.
    pub(crate) fn css(slug: &str) -> Option<&'static str> {
        SHEETS
            .binary_search_by(|(name, _)| (*name).cmp(slug))
            .ok()
            .map(|at| SHEETS[at].1)
    }
}

/// `slug`'s override stylesheet: `BIR_PRINT_OVERRIDES` first; then, on Android,
/// the compiled-in copy, and elsewhere the file in [`overrides_dir`].
fn override_css(slug: &str) -> Option<String> {
    let file = format!("{slug}.css");
    if let Some(dir) = std::env::var_os("BIR_PRINT_OVERRIDES") {
        return std::fs::read_to_string(std::path::Path::new(&dir).join(file)).ok();
    }
    #[cfg(target_os = "android")]
    {
        embedded::css(slug).map(str::to_owned)
    }
    #[cfg(not(target_os = "android"))]
    {
        std::fs::read_to_string(overrides_dir().join(file)).ok()
    }
}

/// [`prepare_html`] plus `slug`'s override stylesheet (when `apply_override`
/// and there is one).
pub fn prepare_html_for(slug: &str, html: &str, apply_override: bool) -> String {
    let html = prepare_html(html);
    if !apply_override {
        return html;
    }
    let Some(css) = override_css(slug) else {
        return html;
    };
    inject_override(slug, &html, &css)
}

fn inject_override(slug: &str, html: &str, css: &str) -> String {
    let tag = format!("<style id=\"form-override\" data-slug=\"{slug}\">{css}</style>");
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], tag, &html[at..]),
        None => format!("{tag}{html}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn folio_sheets_scale_to_fit_a4() {
        let scale = super::fit_scale((612.0, 936.0));
        assert!((scale - (841.89 - 20.0) / 936.0).abs() < 1e-4);
        // Never upscale a sheet that already fits.
        assert_eq!(super::fit_scale((400.0, 500.0)), 1.0);
    }

    #[test]
    fn the_scale_rule_matches_the_host_engine() {
        // WebKit measures text at the zoomed size, so it gets a transform;
        // Chromium's text metrics scale with `zoom` and its PDF writer loses
        // the text layer under a transform, so it keeps `zoom`.
        let css = super::fit_css((612.0, 936.0));
        if cfg!(any(target_os = "windows", target_os = "android")) {
            assert!(css.contains(".sheet>.page{zoom:"), "{css}");
            assert!(!css.contains(".sheet>.page{transform:scale("), "{css}");
        } else {
            assert!(css.contains(".sheet>.page{transform:scale("), "{css}");
            assert!(!css.contains(".sheet>.page{zoom:"), "{css}");
        }
    }

    #[test]
    fn reads_the_native_page_size_and_injects_paper_css() {
        let html =
            "<html><head><style>@page { size: 612pt 936pt; }</style></head><body></body></html>";
        assert_eq!(super::document_sheet_size(html), Some((612.0, 936.0)));
        let out = super::prepare_html(html);
        assert!(out.contains("id=\"preview-paper\""));
        assert!(out.find("preview-paper").unwrap() < out.find("</head>").unwrap());
    }

    #[test]
    fn override_lands_after_the_paper_css_inside_head() {
        let html = super::prepare_html(
            "<html><head><style>@page{size:612pt 936pt}</style></head><body></body></html>",
        );
        let out = super::inject_override("bir-1701", &html, ".x{top:1pt}");
        let tag = out
            .find("<style id=\"form-override\" data-slug=\"bir-1701\">.x{top:1pt}</style>")
            .unwrap();
        assert!(out.find("id=\"preview-paper\"").unwrap() < tag);
        assert!(tag < out.find("</head>").unwrap());
    }

    #[test]
    fn every_print_override_is_compiled_in_unchanged() {
        // What Android reads must be the folder's files, all of them, byte for byte.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../print-overrides");
        let mut slugs = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("print-overrides/") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|ext| ext == "css") {
                let slug = path.file_stem().unwrap().to_str().unwrap().to_owned();
                let file = std::fs::read_to_string(&path).expect("stylesheet");
                assert_eq!(super::embedded::css(&slug), Some(file.as_str()), "{slug}");
                slugs.push(slug);
            }
        }
        assert!(!slugs.is_empty(), "no stylesheets in {}", dir.display());
        assert_eq!(super::embedded::SHEETS.len(), slugs.len());
        // Sorted, so the lookup's binary search finds every one.
        assert!(super::embedded::SHEETS.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(super::embedded::css("no-such-slug-xyz"), None);
    }

    #[test]
    fn missing_override_file_is_a_no_op() {
        let html = "<html><head></head><body></body></html>";
        assert_eq!(
            super::prepare_html_for("no-such-slug-xyz", html, true),
            super::prepare_html(html)
        );
    }

    #[test]
    fn overrides_env_override_wins_over_the_packaged_layout() {
        let tag = format!("bir-shell-overrides-{}", std::process::id());
        let root = std::env::temp_dir().join(tag);
        let _ = std::fs::remove_dir_all(&root);
        let bin = root.join("install-dir");
        std::fs::create_dir_all(bin.join("print-overrides")).expect("packaged overrides");
        let exe = bin.join("bir");
        std::fs::write(&exe, "").expect("exe");

        let chosen = std::path::PathBuf::from("/override/print-overrides");
        assert_eq!(
            super::overrides_dir_from(Some(chosen.clone().into_os_string()), Some(exe)),
            chosen
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sheet_flow_does_not_force_blank_trailing_print_page() {
        let css = super::fit_css((612.0, 936.0));
        assert!(
            css.contains("box-sizing:border-box"),
            "sheet must include padding-top inside its A4 height: {css}"
        );
        assert!(
            css.contains("@media print{.sheet-flow{min-height:0}}"),
            "flow sheets must not force a full-page min-height when printing: {css}"
        );
    }

    #[test]
    fn packaged_overrides_beside_the_executable_are_used() {
        let tag = format!("bir-shell-overrides-pkg-{}", std::process::id());
        let root = std::env::temp_dir().join(tag);
        let _ = std::fs::remove_dir_all(&root);
        let bin = root.join("install-dir");
        let overrides = bin.join("print-overrides");
        std::fs::create_dir_all(&overrides).expect("packaged overrides");
        let exe = bin.join("bir");
        std::fs::write(&exe, "").expect("exe");

        // The resolver canonicalizes the executable path, so compare against
        // the resolved root (/var -> /private/var on macOS).
        let expected = overrides.canonicalize().unwrap_or(overrides);
        assert_eq!(super::overrides_dir_from(None, Some(exe)), expected);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn assert_band_rows_stay_on_fixed_print_sheets(slug: &str, keys: &[&str]) {
        let fields: std::collections::BTreeMap<String, String> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| ((*key).to_string(), format!("ROW{}X", i + 1)))
            .collect();
        let doc = bir_print::frozen_html::filled_document_with_report(slug, &fields, None)
            .unwrap_or_else(|err| panic!("{slug}: fill failed: {err}"));
        assert!(
            doc.html.contains("page-continuation-band"),
            "{slug}: the overflow fill must produce a cloned-band continuation page"
        );

        // Exercise the same wrapper path the app's print preview and PDF print use.
        let html = crate::static_html::staticize(&super::prepare_html_for(slug, &doc.html, true));
        let marker = "class=\"page page-continuation-band\"";
        let mut from = 0usize;
        let mut count = 0usize;
        while let Some(rel) = html[from..].find(marker) {
            let at = from + rel;
            let sheet_at = html[..at]
                .rfind("<div class=\"sheet")
                .unwrap_or_else(|| panic!("{slug}: band page without a sheet wrapper"));
            let tag_end = html[sheet_at..]
                .find('>')
                .unwrap_or_else(|| panic!("{slug}: unterminated sheet tag"));
            let sheet_tag = &html[sheet_at..sheet_at + tag_end + 1];
            assert!(
                sheet_tag.starts_with("<div class=\"sheet\" "),
                "{slug}: cloned-band pages are fixed paper grids and must not use the flow wrapper: {sheet_tag}"
            );
            count += 1;
            from = at + marker.len();
        }
        assert!(count > 0, "{slug}: no band page in the static print html");
    }

    macro_rules! band_rows_stay_inside_page {
        ($name:ident, $slug:literal, $keys:expr) => {
            #[test]
            fn $name() {
                assert_band_rows_stay_on_fixed_print_sheets($slug, $keys);
            }
        };
    }

    band_rows_stay_inside_page!(
        form_1601eq_band_rows_stay_inside_page,
        "1601eq-2019",
        &["frm1601EQ:txtAtcCd7", "frm1601EQ:txtAtcCd8"]
    );
    band_rows_stay_inside_page!(
        form_1602q_band_rows_stay_inside_page,
        "1602q-2019",
        &["frm1602Q:txtSched2Interest3", "frm1602Q:txtSched2Interest4",]
    );
    band_rows_stay_inside_page!(
        form_1701a_band_rows_stay_inside_page,
        "1701a-2018",
        &[
            "frm1701A:txtmodalPartIVADesc_1C1",
            "frm1701A:txtmodalPartIVADesc_2C1",
        ]
    );
    band_rows_stay_inside_page!(
        form_1702rt_band_rows_stay_inside_page,
        "1702rt-2018c",
        &["frm1702RT:txtPg3Sc2I4.1C1", "frm1702RT:txtPg3Sc2I4.2C1",]
    );
    band_rows_stay_inside_page!(
        form_1707_band_rows_stay_inside_page,
        "1707-2021",
        &["frm1707:txtPg2Pt4S3_1Col1", "frm1707:txtPg2Pt4S3_2Col1"]
    );
    band_rows_stay_inside_page!(
        form_1800_band_rows_stay_inside_page,
        "1800-2018",
        &[
            "frm1800v2018:schedA:txtParticulars5",
            "frm1800v2018:schedA:txtParticulars6",
        ]
    );
    band_rows_stay_inside_page!(
        form_1801_band_rows_stay_inside_page,
        "1801-2018",
        &["frm1801v2018:sched1Oct_3", "frm1801v2018:sched1Oct_4"]
    );
    band_rows_stay_inside_page!(
        form_2000dst_band_rows_stay_inside_page,
        "2000-dst-2018",
        &["frm2000:sched2:txtReceipt3", "frm2000:sched2:txtReceipt4"]
    );
    band_rows_stay_inside_page!(
        form_2000ot_band_rows_stay_inside_page,
        "2000-ot-2018",
        &["frm2000OT:sched1A:txtOCTNo5", "frm2000OT:sched1A:txtOCTNo6"]
    );
    band_rows_stay_inside_page!(
        form_2551q_band_rows_stay_inside_page,
        "2551q-2018",
        &["txtATCAmt7", "txtATCAmt8"]
    );
    band_rows_stay_inside_page!(
        form_2552_band_rows_stay_inside_page,
        "2552-2018",
        &[
            "frm2552:txtPg2Pt5Sch1Seller6",
            "frm2552:txtPg2Pt5Sch1Seller7",
        ]
    );
}
