//! Native routing gates for the owned HTML form renderer.
//!
//! The schema-v3 `route` permits an explicitly labelled development preview or
//! an exact-revision HTML-only candidate runtime. `release_ready` remains a
//! separate, stricter distribution gate. Keeping both decisions in Rust lets a
//! non-development candidate exercise the real native host before certification
//! without treating the presence of a React component as release evidence.

use serde::Deserialize;

use crate::html::RenderEnvelopeV1;
use crate::html_forms::{
    render_layout_plan, RenderLayoutError, RenderLayoutPlan, RenderPageGeometry,
};

const BUNDLED_MIGRATION_STATUS: &str =
    include_str!("../../../packages/form-specs/form-migration-status.json");

const CSS_PIXELS_PER_POINT: f64 = 96.0 / 72.0;
const POINT_TOLERANCE: f64 = 0.25;
const CSS_PIXEL_TOLERANCE: f64 = 0.75;
const CSS_CLIENT_PIXEL_TOLERANCE: f64 = 2.25;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HtmlRendererSupport {
    pub html_enabled: bool,
    pub html_only: bool,
    pub release_ready: bool,
}

impl HtmlRendererSupport {
    /// Preview access requires an enabled HTML route. Release certification is
    /// tracked separately so pre-release builds can exercise the only renderer
    /// without reintroducing a second output path.
    pub fn permits_preview(self) -> bool {
        self.html_enabled
    }

    /// A certification candidate may exercise only a manifest-owned
    /// `html_only` route. Experimental forms remain developer-build-only.
    /// Public distribution is still blocked independently by the migration
    /// audit until `release_ready` and every evidence gate pass.
    pub fn permits_certification_candidate(self) -> bool {
        self.html_enabled && self.html_only
    }

    /// Public release certification requires both the HTML-only route and the
    /// reviewed evidence gate. This is intentionally not the runtime candidate
    /// switch; tagged packaging workflows enforce it before publication.
    pub fn permits_release_routing(self) -> bool {
        self.permits_certification_candidate() && self.release_ready
    }
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

/// Resolve the exact-revision provider before constructing a native HTML host.
/// The returned plan is the single source for both DOM and PDF validation.
pub fn renderer_host_plan(
    envelope: &RenderEnvelopeV1,
) -> Result<RenderLayoutPlan, RenderLayoutError> {
    render_layout_plan(envelope)
}

/// Look up the renderer flags embedded from the single source-of-truth
/// migration manifest. Malformed or missing entries fail closed.
pub fn bundled_html_renderer_support(code: &str, revision: &str) -> HtmlRendererSupport {
    html_renderer_support_from_manifest(BUNDLED_MIGRATION_STATUS, code, revision)
        .unwrap_or_default()
}

/// Validate renderer measurements against an exact provider-owned host plan.
/// The renderer cannot become ready merely by reporting a nonzero page count
/// or geometry that it chose for itself.
pub fn validate_renderer_geometry(
    report: &RendererGeometryReport,
    plan: &RenderLayoutPlan,
) -> Result<(), String> {
    validate_renderer_geometry_against(
        report,
        plan.expected_page_count,
        plan.page_geometry,
        &plan.provider.key(),
    )
}

fn validate_renderer_geometry_against(
    report: &RendererGeometryReport,
    expected_page_count: usize,
    page_geometry: RenderPageGeometry,
    provider_key: &str,
) -> Result<(), String> {
    page_geometry
        .validate()
        .map_err(|error| error.to_string())?;
    if report.page_count != expected_page_count {
        return Err(format!(
            "HTML renderer for {provider_key} reported {} pages; host expected {expected_page_count}",
            report.page_count,
        ));
    }
    if report.pages.len() != report.page_count {
        return Err(format!(
            "HTML renderer supplied {} page rectangles for {} pages",
            report.pages.len(),
            report.page_count
        ));
    }
    if !approximately_equal(
        report.page_width_pt,
        page_geometry.width_points,
        POINT_TOLERANCE,
    ) || !approximately_equal(
        report.page_height_pt,
        page_geometry.height_points,
        POINT_TOLERANCE,
    ) {
        return Err(format!(
            "HTML renderer reported {:.3}x{:.3}pt pages; host expected {}x{}pt",
            report.page_width_pt,
            report.page_height_pt,
            page_geometry.width_points,
            page_geometry.height_points,
        ));
    }

    let expected_width_px = page_geometry.width_points * CSS_PIXELS_PER_POINT;
    let expected_height_px = page_geometry.height_points * CSS_PIXELS_PER_POINT;
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
    if manifest.schema_version != 3 {
        return None;
    }
    manifest
        .forms
        .into_iter()
        .find(|form| form.code == code && form.revision == revision)
        .map(|form| HtmlRendererSupport {
            html_enabled: form.route != MigrationRoute::Disabled,
            html_only: form.route == MigrationRoute::HtmlOnly,
            release_ready: form.release_ready && form.route == MigrationRoute::HtmlOnly,
        })
}

#[derive(Debug, Deserialize)]
struct MigrationManifest {
    schema_version: u8,
    forms: Vec<MigrationForm>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MigrationRoute {
    Disabled,
    Experimental,
    HtmlOnly,
}

#[derive(Debug, Deserialize)]
struct MigrationForm {
    code: String,
    revision: String,
    route: MigrationRoute,
    #[serde(default)]
    release_ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html_forms::{render_form_provider, RenderFixtureKind};

    #[test]
    fn bundled_2551q_is_html_only_but_not_release_certified() {
        let support = bundled_html_renderer_support("2551Q", "2018");

        assert!(support.permits_preview());
        assert!(support.permits_certification_candidate());
        assert!(!support.permits_release_routing());
    }

    #[test]
    fn bundled_1601c_is_experimental_but_not_release_certified() {
        let support = bundled_html_renderer_support("1601C", "2018");

        assert!(support.permits_preview());
        assert!(!support.permits_certification_candidate());
        assert!(!support.permits_release_routing());
    }

    #[test]
    fn bundled_0619e_is_experimental_but_not_release_certified() {
        let support = bundled_html_renderer_support("0619E", "2018");

        assert!(support.permits_preview());
        assert!(!support.permits_certification_candidate());
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
    fn release_ready_never_bypasses_experimental_route() {
        let support = html_renderer_support_from_manifest(
            r#"{"schema_version":3,"forms":[{"code":"2551Q","revision":"2018","route":"experimental","release_ready":true}]}"#,
            "2551Q",
            "2018",
        )
        .expect("matching support entry");

        assert!(support.permits_preview());
        assert!(!support.permits_certification_candidate());
        assert!(!support.permits_release_routing());
    }

    #[test]
    fn only_html_only_route_can_be_release_routable() {
        let support = html_renderer_support_from_manifest(
            r#"{"schema_version":3,"forms":[{"code":"2551Q","revision":"2018","route":"html_only","release_ready":true}]}"#,
            "2551Q",
            "2018",
        )
        .expect("matching support entry");

        assert!(support.permits_certification_candidate());
        assert!(support.permits_release_routing());
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

    fn geometry_report_for(
        page_count: usize,
        page_geometry: RenderPageGeometry,
    ) -> RendererGeometryReport {
        let width_px = page_geometry.width_points * CSS_PIXELS_PER_POINT;
        let height_px = page_geometry.height_points * CSS_PIXELS_PER_POINT;
        RendererGeometryReport {
            page_count,
            page_width_pt: page_geometry.width_points,
            page_height_pt: page_geometry.height_points,
            pages: (0..page_count)
                .map(|index| RendererPageRect {
                    x: 24.0,
                    y: index as f64 * (height_px + 12.0),
                    width: width_px,
                    height: height_px,
                    client_width: width_px,
                    client_height: height_px,
                    scroll_width: width_px,
                    scroll_height: height_px,
                    descendant_overflow_x: 0,
                    descendant_overflow_y: 0,
                    descendant_clipped_x: 0,
                    descendant_clipped_y: 0,
                })
                .collect(),
        }
    }

    fn geometry_report(page_count: usize) -> RendererGeometryReport {
        geometry_report_for(page_count, RenderPageGeometry::LEGAL)
    }

    #[test]
    fn host_owns_2551q_page_count_boundaries() {
        let provider = render_form_provider("2551Q", "2018").expect("2551Q provider");
        let mut envelope = (provider.fixtures)()
            .expect("provider fixtures")
            .into_iter()
            .find(|fixture| fixture.kind == RenderFixtureKind::ScheduleCapacity)
            .expect("schedule-capacity fixture")
            .envelope;
        let prototype = envelope.schedules[0]
            .rows
            .first()
            .cloned()
            .expect("schedule prototype");
        for (rows, expected_pages) in [(0, 2), (6, 2), (7, 3), (18, 3), (19, 4), (30, 4), (31, 5)] {
            envelope.schedules[0].rows = std::iter::repeat_n(prototype.clone(), rows).collect();
            assert_eq!(
                renderer_host_plan(&envelope)
                    .expect("provider-owned page count")
                    .expected_page_count,
                expected_pages
            );
        }
    }

    #[test]
    fn host_plan_resolves_exact_provider_geometry_and_rejects_unknown_forms() {
        let provider = render_form_provider("2551Q", "2018").unwrap();
        let mut envelope = (provider.fixtures)().unwrap().remove(0).envelope;
        let plan = renderer_host_plan(&envelope).expect("known provider plan");

        assert_eq!(plan.provider.key(), "2551Q:2018");
        assert_eq!(plan.page_geometry, RenderPageGeometry::LEGAL);
        assert_eq!(plan.expected_page_count, 2);

        envelope.form.version = "unknown".to_string();
        assert!(matches!(
            renderer_host_plan(&envelope),
            Err(RenderLayoutError::UnknownProvider { .. })
        ));
    }

    #[test]
    fn generic_geometry_validation_supports_every_source_pack_paper_height() {
        for geometry in [
            RenderPageGeometry::LETTER,
            RenderPageGeometry::LEGAL,
            RenderPageGeometry::FOURTEEN_INCH,
        ] {
            validate_renderer_geometry_against(
                &geometry_report_for(1, geometry),
                1,
                geometry,
                "test:revision",
            )
            .expect("provider geometry should validate");
        }

        let letter_report = geometry_report_for(1, RenderPageGeometry::LETTER);
        assert!(validate_renderer_geometry_against(
            &letter_report,
            1,
            RenderPageGeometry::FOURTEEN_INCH,
            "test:revision",
        )
        .expect_err("provider paper height mismatch must fail")
        .contains("612x1008pt"));
    }

    #[test]
    fn generic_validator_uses_the_resolved_provider_plan() {
        let provider = render_form_provider("2551Q", "2018").unwrap();
        let envelope = (provider.fixtures)().unwrap().remove(0).envelope;
        let plan = renderer_host_plan(&envelope).unwrap();

        validate_renderer_geometry(&geometry_report(2), &plan).expect("resolved provider geometry");
        assert!(validate_renderer_geometry(&geometry_report(3), &plan)
            .expect_err("self-reported page count cannot override provider")
            .contains("2551Q:2018"));
    }

    #[test]
    fn geometry_validation_rejects_self_reported_drift() {
        let provider = render_form_provider("2551Q", "2018").expect("2551Q provider");
        let envelope = (provider.fixtures)().expect("fixtures").remove(0).envelope;
        let plan = renderer_host_plan(&envelope).expect("2551Q host plan");

        assert!(validate_renderer_geometry(&geometry_report(2), &plan).is_ok());

        let mut wrong_count = geometry_report(2);
        wrong_count.page_count = 3;
        assert!(validate_renderer_geometry(&wrong_count, &plan)
            .expect_err("wrong count must fail")
            .contains("host expected"));

        let mut wrong_points = geometry_report(2);
        wrong_points.page_height_pt = 792.0;
        assert!(validate_renderer_geometry(&wrong_points, &plan)
            .expect_err("wrong paper size must fail")
            .contains("612x936pt"));

        let mut wrong_rect = geometry_report(2);
        wrong_rect.pages[1].width = 800.0;
        assert!(validate_renderer_geometry(&wrong_rect, &plan)
            .expect_err("wrong measured rectangle must fail")
            .contains("page 2"));

        let mut overflowing = geometry_report(2);
        overflowing.pages[0].scroll_height = 1_300.0;
        assert!(validate_renderer_geometry(&overflowing, &plan)
            .expect_err("overflow must fail")
            .contains("overflowing"));

        let mut hidden_descendant_overflow = geometry_report(2);
        hidden_descendant_overflow.pages[0].descendant_overflow_x = 1;
        assert!(
            validate_renderer_geometry(&hidden_descendant_overflow, &plan)
                .expect_err("hidden descendant overflow must fail")
                .contains("descendant overflow x/y: 1/0")
        );

        let mut clipped_descendant = geometry_report(2);
        clipped_descendant.pages[1].descendant_clipped_y = 2;
        assert!(validate_renderer_geometry(&clipped_descendant, &plan)
            .expect_err("descendant clipping must fail")
            .contains("clipped x/y: 0/2"));

        let mut overlapping = geometry_report(2);
        overlapping.pages[1].y = overlapping.pages[0].y + 1.0;
        assert!(validate_renderer_geometry(&overlapping, &plan)
            .expect_err("overlapping pages must fail")
            .contains("overlaps the preceding page"));

        let mut misaligned = geometry_report(2);
        misaligned.pages[1].x += 2.0;
        assert!(validate_renderer_geometry(&misaligned, &plan)
            .expect_err("horizontal page drift must fail")
            .contains("not horizontally aligned"));

        let mut transformed_client_box = geometry_report(2);
        transformed_client_box.pages[0].client_width /= 2.0;
        assert!(validate_renderer_geometry(&transformed_client_box, &plan)
            .expect_err("transformed client dimensions must fail")
            .contains("client dimensions"));
    }
}
