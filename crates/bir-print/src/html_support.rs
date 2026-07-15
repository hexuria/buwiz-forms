//! Native routing gates for the owned HTML form renderer.
//!
//! `html_enabled` permits an explicitly labelled development preview.
//! `release_ready` is a separate, stricter gate for any future production
//! routing. Keeping both decisions in Rust prevents the desktop host from
//! treating the presence of a React component as release evidence.

use serde::Deserialize;

const BUNDLED_MIGRATION_STATUS: &str =
    include_str!("../../../packages/form-specs/form-migration-status.json");

pub const LEGACY_2551Q_SCHEDULE_CAPACITY: usize = 6;
pub const CONTINUATION_2551Q_SCHEDULE_CAPACITY: usize = 12;
pub const PAGE_2551Q_WIDTH_PT: f64 = 612.0;
pub const PAGE_2551Q_HEIGHT_PT: f64 = 936.0;

const CSS_PIXELS_PER_POINT: f64 = 96.0 / 72.0;
const POINT_TOLERANCE: f64 = 0.25;
const CSS_PIXEL_TOLERANCE: f64 = 0.75;
const CSS_CLIENT_PIXEL_TOLERANCE: f64 = 2.25;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HtmlRendererSupport {
    pub html_enabled: bool,
    pub release_ready: bool,
}

impl HtmlRendererSupport {
    /// Explicit development access only requires the renderer to be enabled.
    pub fn permits_experimental_preview(self) -> bool {
        self.html_enabled
    }

    /// Production routing requires both the enablement and evidence gates.
    pub fn permits_release_routing(self) -> bool {
        self.html_enabled && self.release_ready
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyPreviewDecision {
    Render,
    BlockScheduleOverflow {
        row_count: usize,
        supported_rows: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererReadinessDecision {
    Pending,
    Ready { page_count: usize },
    Fallback(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererPageRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub client_width: f64,
    pub client_height: f64,
    pub scroll_width: f64,
    pub scroll_height: f64,
    pub descendant_overflow_x: usize,
    pub descendant_overflow_y: usize,
    pub descendant_clipped_x: usize,
    pub descendant_clipped_y: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererGeometryReport {
    pub page_count: usize,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub pages: Vec<RendererPageRect>,
}

/// Look up the renderer flags embedded from the single source-of-truth
/// migration manifest. Malformed or missing entries fail closed.
pub fn bundled_html_renderer_support(code: &str, revision: &str) -> HtmlRendererSupport {
    html_renderer_support_from_manifest(BUNDLED_MIGRATION_STATUS, code, revision)
        .unwrap_or_default()
}

pub fn legacy_2551q_preview_decision(row_count: usize) -> LegacyPreviewDecision {
    if row_count <= LEGACY_2551Q_SCHEDULE_CAPACITY {
        LegacyPreviewDecision::Render
    } else {
        LegacyPreviewDecision::BlockScheduleOverflow {
            row_count,
            supported_rows: LEGACY_2551Q_SCHEDULE_CAPACITY,
        }
    }
}

pub fn expected_2551q_page_count(row_count: usize) -> usize {
    let continuation_rows = row_count.saturating_sub(LEGACY_2551Q_SCHEDULE_CAPACITY);
    let continuation_pages = continuation_rows.div_ceil(CONTINUATION_2551Q_SCHEDULE_CAPACITY);
    2 + continuation_pages
}

/// Validate renderer measurements against host-owned 2551Q geometry. The
/// renderer cannot become ready merely by reporting a nonzero page count.
pub fn validate_2551q_renderer_geometry(
    report: &RendererGeometryReport,
    expected_page_count: usize,
) -> Result<(), String> {
    if report.page_count != expected_page_count {
        return Err(format!(
            "HTML renderer reported {} pages; host expected {expected_page_count}",
            report.page_count
        ));
    }
    if report.pages.len() != report.page_count {
        return Err(format!(
            "HTML renderer supplied {} page rectangles for {} pages",
            report.pages.len(),
            report.page_count
        ));
    }
    if !approximately_equal(report.page_width_pt, PAGE_2551Q_WIDTH_PT, POINT_TOLERANCE)
        || !approximately_equal(report.page_height_pt, PAGE_2551Q_HEIGHT_PT, POINT_TOLERANCE)
    {
        return Err(format!(
            "HTML renderer reported {:.3}x{:.3}pt pages; host expected {}x{}pt",
            report.page_width_pt, report.page_height_pt, PAGE_2551Q_WIDTH_PT, PAGE_2551Q_HEIGHT_PT
        ));
    }

    let expected_width_px = PAGE_2551Q_WIDTH_PT * CSS_PIXELS_PER_POINT;
    let expected_height_px = PAGE_2551Q_HEIGHT_PT * CSS_PIXELS_PER_POINT;
    let mut expected_x = None;
    let mut previous_bottom = None;
    for (index, page) in report.pages.iter().enumerate() {
        if ![
            page.x,
            page.y,
            page.width,
            page.height,
            page.client_width,
            page.client_height,
            page.scroll_width,
            page.scroll_height,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(format!(
                "HTML renderer page {} contains non-finite geometry",
                index + 1
            ));
        }
        if !approximately_equal(page.width, expected_width_px, CSS_PIXEL_TOLERANCE)
            || !approximately_equal(page.height, expected_height_px, CSS_PIXEL_TOLERANCE)
        {
            return Err(format!(
                "HTML renderer page {} measured {:.3}x{:.3}px; host expected {:.3}x{:.3}px",
                index + 1,
                page.width,
                page.height,
                expected_width_px,
                expected_height_px
            ));
        }
        if !approximately_equal(
            page.client_width,
            expected_width_px,
            CSS_CLIENT_PIXEL_TOLERANCE,
        ) || !approximately_equal(
            page.client_height,
            expected_height_px,
            CSS_CLIENT_PIXEL_TOLERANCE,
        ) {
            return Err(format!(
                "HTML renderer page {} has client dimensions {:.3}x{:.3}px; host expected {:.3}x{:.3}px",
                index + 1,
                page.client_width,
                page.client_height,
                expected_width_px,
                expected_height_px,
            ));
        }
        if let Some(first_x) = expected_x {
            if !approximately_equal(page.x, first_x, CSS_PIXEL_TOLERANCE) {
                return Err(format!(
                    "HTML renderer page {} is not horizontally aligned with page 1",
                    index + 1
                ));
            }
        } else {
            expected_x = Some(page.x);
        }
        if previous_bottom.is_some_and(|bottom| page.y + CSS_PIXEL_TOLERANCE < bottom) {
            return Err(format!(
                "HTML renderer page {} overlaps the preceding page",
                index + 1
            ));
        }
        if page.client_width <= 0.0
            || page.client_height <= 0.0
            || page.scroll_width > page.client_width + CSS_PIXEL_TOLERANCE
            || page.scroll_height > page.client_height + CSS_PIXEL_TOLERANCE
            || page.descendant_overflow_x > 0
            || page.descendant_overflow_y > 0
            || page.descendant_clipped_x > 0
            || page.descendant_clipped_y > 0
        {
            return Err(format!(
                "HTML renderer page {} contains clipped or overflowing content \
                 (descendant overflow x/y: {}/{}, clipped x/y: {}/{})",
                index + 1,
                page.descendant_overflow_x,
                page.descendant_overflow_y,
                page.descendant_clipped_x,
                page.descendant_clipped_y,
            ));
        }
        previous_bottom = Some(page.y + page.height);
    }

    Ok(())
}

fn approximately_equal(actual: f64, expected: f64, tolerance: f64) -> bool {
    actual.is_finite() && (actual - expected).abs() <= tolerance
}

/// Convert asynchronous renderer signals into one terminal native-host
/// decision. A ready message without measurable pages is not sufficient.
pub fn renderer_readiness_decision(
    ready: bool,
    page_count: Option<usize>,
    error: Option<&str>,
    timed_out: bool,
) -> RendererReadinessDecision {
    if let Some(error) = error {
        return RendererReadinessDecision::Fallback(format!("HTML renderer failed: {error}"));
    }
    if ready && page_count.is_some_and(|count| count > 0) {
        return RendererReadinessDecision::Ready {
            page_count: page_count.unwrap_or_default(),
        };
    }
    if ready && page_count == Some(0) {
        return RendererReadinessDecision::Fallback(
            "HTML renderer reported zero printable pages".to_string(),
        );
    }
    if timed_out {
        return RendererReadinessDecision::Fallback(
            "HTML renderer readiness timed out after five seconds".to_string(),
        );
    }
    RendererReadinessDecision::Pending
}

fn html_renderer_support_from_manifest(
    manifest: &str,
    code: &str,
    revision: &str,
) -> Option<HtmlRendererSupport> {
    let manifest: MigrationManifest = serde_json::from_str(manifest).ok()?;
    manifest
        .forms
        .into_iter()
        .find(|form| form.code == code && form.revision == revision)
        .map(|form| HtmlRendererSupport {
            html_enabled: form.html_enabled,
            release_ready: form.release_ready,
        })
}

#[derive(Debug, Deserialize)]
struct MigrationManifest {
    forms: Vec<MigrationForm>,
}

#[derive(Debug, Deserialize)]
struct MigrationForm {
    code: String,
    revision: String,
    #[serde(default)]
    html_enabled: bool,
    #[serde(default)]
    release_ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_2551q_is_experimental_but_not_release_routable() {
        let support = bundled_html_renderer_support("2551Q", "2018");

        assert!(support.permits_experimental_preview());
        assert!(!support.permits_release_routing());
    }

    #[test]
    fn unknown_or_invalid_manifest_entries_fail_closed() {
        assert_eq!(
            bundled_html_renderer_support("UNKNOWN", "2018"),
            HtmlRendererSupport::default()
        );
        assert_eq!(
            html_renderer_support_from_manifest("not-json", "2551Q", "2018"),
            None
        );
    }

    #[test]
    fn release_ready_never_bypasses_html_enablement() {
        let support = html_renderer_support_from_manifest(
            r#"{"forms":[{"code":"2551Q","revision":"2018","html_enabled":false,"release_ready":true}]}"#,
            "2551Q",
            "2018",
        )
        .expect("matching support entry");

        assert!(!support.permits_experimental_preview());
        assert!(!support.permits_release_routing());
    }

    #[test]
    fn legacy_preview_blocks_the_first_unrepresentable_row() {
        assert_eq!(
            legacy_2551q_preview_decision(LEGACY_2551Q_SCHEDULE_CAPACITY),
            LegacyPreviewDecision::Render
        );
        assert_eq!(
            legacy_2551q_preview_decision(LEGACY_2551Q_SCHEDULE_CAPACITY + 1),
            LegacyPreviewDecision::BlockScheduleOverflow {
                row_count: 7,
                supported_rows: 6,
            }
        );
    }

    #[test]
    fn readiness_requires_both_renderer_signal_and_measurable_pages() {
        assert_eq!(
            renderer_readiness_decision(true, None, None, false),
            RendererReadinessDecision::Pending
        );
        assert_eq!(
            renderer_readiness_decision(true, Some(2), None, false),
            RendererReadinessDecision::Ready { page_count: 2 }
        );
        assert!(matches!(
            renderer_readiness_decision(false, None, Some("boom"), false),
            RendererReadinessDecision::Fallback(reason) if reason.contains("boom")
        ));
        assert!(matches!(
            renderer_readiness_decision(false, None, None, true),
            RendererReadinessDecision::Fallback(reason) if reason.contains("timed out")
        ));
        assert!(matches!(
            renderer_readiness_decision(true, Some(2), Some("late failure"), false),
            RendererReadinessDecision::Fallback(reason) if reason.contains("late failure")
        ));
    }

    fn geometry_report(page_count: usize) -> RendererGeometryReport {
        RendererGeometryReport {
            page_count,
            page_width_pt: PAGE_2551Q_WIDTH_PT,
            page_height_pt: PAGE_2551Q_HEIGHT_PT,
            pages: (0..page_count)
                .map(|index| RendererPageRect {
                    x: 24.0,
                    y: index as f64 * 1_260.0,
                    width: PAGE_2551Q_WIDTH_PT * CSS_PIXELS_PER_POINT,
                    height: PAGE_2551Q_HEIGHT_PT * CSS_PIXELS_PER_POINT,
                    client_width: 814.0,
                    client_height: 1_246.0,
                    scroll_width: 814.0,
                    scroll_height: 1_246.0,
                    descendant_overflow_x: 0,
                    descendant_overflow_y: 0,
                    descendant_clipped_x: 0,
                    descendant_clipped_y: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn host_owns_2551q_page_count_boundaries() {
        for (rows, expected_pages) in [(0, 2), (6, 2), (7, 3), (18, 3), (19, 4), (30, 4), (31, 5)] {
            assert_eq!(expected_2551q_page_count(rows), expected_pages);
        }
    }

    #[test]
    fn geometry_validation_rejects_self_reported_drift() {
        assert!(validate_2551q_renderer_geometry(&geometry_report(2), 2).is_ok());

        let mut wrong_count = geometry_report(2);
        wrong_count.page_count = 3;
        assert!(validate_2551q_renderer_geometry(&wrong_count, 2)
            .expect_err("wrong count must fail")
            .contains("host expected"));

        let mut wrong_points = geometry_report(2);
        wrong_points.page_height_pt = 792.0;
        assert!(validate_2551q_renderer_geometry(&wrong_points, 2)
            .expect_err("wrong paper size must fail")
            .contains("612x936pt"));

        let mut wrong_rect = geometry_report(2);
        wrong_rect.pages[1].width = 800.0;
        assert!(validate_2551q_renderer_geometry(&wrong_rect, 2)
            .expect_err("wrong measured rectangle must fail")
            .contains("page 2"));

        let mut overflowing = geometry_report(2);
        overflowing.pages[0].scroll_height = 1_300.0;
        assert!(validate_2551q_renderer_geometry(&overflowing, 2)
            .expect_err("overflow must fail")
            .contains("overflowing"));

        let mut hidden_descendant_overflow = geometry_report(2);
        hidden_descendant_overflow.pages[0].descendant_overflow_x = 1;
        assert!(
            validate_2551q_renderer_geometry(&hidden_descendant_overflow, 2)
                .expect_err("hidden descendant overflow must fail")
                .contains("descendant overflow x/y: 1/0")
        );

        let mut clipped_descendant = geometry_report(2);
        clipped_descendant.pages[1].descendant_clipped_y = 2;
        assert!(validate_2551q_renderer_geometry(&clipped_descendant, 2)
            .expect_err("descendant clipping must fail")
            .contains("clipped x/y: 0/2"));

        let mut overlapping = geometry_report(2);
        overlapping.pages[1].y = overlapping.pages[0].y + 1.0;
        assert!(validate_2551q_renderer_geometry(&overlapping, 2)
            .expect_err("overlapping pages must fail")
            .contains("overlaps the preceding page"));

        let mut misaligned = geometry_report(2);
        misaligned.pages[1].x += 2.0;
        assert!(validate_2551q_renderer_geometry(&misaligned, 2)
            .expect_err("horizontal page drift must fail")
            .contains("not horizontally aligned"));

        let mut transformed_client_box = geometry_report(2);
        transformed_client_box.pages[0].client_width /= 2.0;
        assert!(validate_2551q_renderer_geometry(&transformed_client_box, 2)
            .expect_err("transformed client dimensions must fail")
            .contains("client dimensions"));
    }
}
