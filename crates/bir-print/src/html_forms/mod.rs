//! Registry of Rust-owned adapters and deterministic fixture providers for HTML forms.

mod form_0605;
mod form_0619e;
mod form_0619f;
mod form_1601c;
mod form_1701;
mod form_1701q;
mod form_1702mx;
mod form_1702rt;
mod form_2550q;
mod form_2551q;

use std::collections::HashSet;

use crate::html::RenderEnvelopeV1;

/// Exact paper dimensions owned by an HTML form provider.
///
/// The BIR source set uses more than one 8.5-inch-wide paper height. Keep the
/// dimensions on the exact-revision provider instead of teaching native hosts
/// that every form is 2551Q-sized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderPageGeometry {
    pub width_points: f64,
    pub height_points: f64,
}

impl RenderPageGeometry {
    /// 8.5 x 11 inch US Letter paper.
    pub const LETTER: Self = Self::new(612.0, 792.0);
    /// 8.5 x 13 inch paper used by the existing BIR Legal references.
    pub const LEGAL: Self = Self::new(612.0, 936.0);
    /// 8.5 x 14 inch paper. Kept distinct from the BIR 13-inch Legal size.
    pub const FOURTEEN_INCH: Self = Self::new(612.0, 1_008.0);

    pub const fn new(width_points: f64, height_points: f64) -> Self {
        Self {
            width_points,
            height_points,
        }
    }

    pub fn validate(self) -> Result<(), RenderLayoutError> {
        if !self.width_points.is_finite()
            || !self.height_points.is_finite()
            || self.width_points <= 0.0
            || self.height_points <= 0.0
        {
            return Err(RenderLayoutError::InvalidPageGeometry {
                width_points: self.width_points,
                height_points: self.height_points,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFixtureKind {
    Minimum,
    Normal,
    LongValues,
    ValidationEdge,
    ScheduleCapacity,
    Variant,
}

#[derive(Debug, Clone)]
pub struct RenderContractFixture {
    pub file_name: &'static str,
    pub kind: RenderFixtureKind,
    pub expected_form_valid: bool,
    pub envelope: RenderEnvelopeV1,
}

#[derive(Debug, Clone)]
pub struct GeneratedContractArtifact {
    pub relative_path: &'static str,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSchedulePolicy {
    pub id: &'static str,
    pub minimum_rows: usize,
    pub first_page_rows: usize,
    pub continuation_page_rows: usize,
    pub repeat_header: bool,
    pub final_totals_on_last_page: bool,
}

impl RenderSchedulePolicy {
    pub fn continuation_page_count(self, row_count: usize) -> Result<usize, RenderLayoutError> {
        if self.continuation_page_rows == 0 {
            return Err(RenderLayoutError::InvalidSchedulePolicy {
                schedule_id: self.id.to_string(),
                reason: "continuation_page_rows must be greater than zero".to_string(),
            });
        }

        Ok(row_count
            .saturating_sub(self.first_page_rows)
            .div_ceil(self.continuation_page_rows))
    }
}

/// Provider-owned deterministic pagination metadata.
///
/// Each declared schedule owns its first-page and continuation capacity. Extra
/// schedules are additive because each policy describes a distinct sequence of
/// continuation pages. Forms with a more complex policy can introduce a new
/// exact-revision provider policy without changing the native host API.
#[derive(Debug, Clone, Copy)]
pub struct RenderPaginationPolicy {
    pub base_page_count: usize,
    pub schedules: &'static [RenderSchedulePolicy],
}

impl RenderPaginationPolicy {
    pub fn expected_page_count(
        self,
        envelope: &RenderEnvelopeV1,
    ) -> Result<usize, RenderLayoutError> {
        if self.base_page_count == 0 {
            return Err(RenderLayoutError::InvalidPaginationPolicy(
                "base_page_count must be greater than zero".to_string(),
            ));
        }

        let mut policy_ids = HashSet::with_capacity(self.schedules.len());
        for policy in self.schedules {
            if policy.id.trim().is_empty() {
                return Err(RenderLayoutError::InvalidPaginationPolicy(
                    "schedule ids must not be empty".to_string(),
                ));
            }
            if !policy_ids.insert(policy.id) {
                return Err(RenderLayoutError::InvalidPaginationPolicy(format!(
                    "duplicate schedule policy {}",
                    policy.id
                )));
            }
        }

        let mut envelope_ids = HashSet::with_capacity(envelope.schedules.len());
        for schedule in &envelope.schedules {
            if !envelope_ids.insert(schedule.id.as_str()) {
                return Err(RenderLayoutError::DuplicateSchedule(schedule.id.clone()));
            }
            if !policy_ids.contains(schedule.id.as_str()) {
                return Err(RenderLayoutError::UnexpectedSchedule(schedule.id.clone()));
            }
        }

        let mut expected = self.base_page_count;
        for policy in self.schedules {
            let schedule = envelope
                .schedules
                .iter()
                .find(|schedule| schedule.id == policy.id)
                .ok_or_else(|| RenderLayoutError::MissingSchedule(policy.id.to_string()))?;
            expected = expected
                .checked_add(policy.continuation_page_count(schedule.rows.len())?)
                .ok_or(RenderLayoutError::PageCountOverflow)?;
        }
        Ok(expected)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualReferencePage {
    pub page: usize,
    pub file_name: &'static str,
    pub sha256: &'static str,
}

/// One calibration-only official page rasterized through the same
/// Chromium/Playwright environment that captures the parity screenshots.
///
/// The pinned noise floor is the pixelmatch difference between the Poppler
/// raster and this Chromium raster of the same official page. It quantifies
/// pure cross-rasterizer noise; it is diagnostic provenance, never a gate
/// relaxation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromiumVisualReference {
    pub page: usize,
    pub file_name: &'static str,
    pub sha256: &'static str,
    pub vector_svg_sha256: &'static str,
    pub noise_floor_changed_pixels: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromiumReferenceGenerator {
    pub pdftocairo_version: &'static str,
    pub playwright_version: &'static str,
    pub chromium_version: &'static str,
}

/// Chromium-raster references for every official page of one exact revision.
/// A form pins either all of its pages or none; partial sets fail manifest
/// generation so the visual gate can never silently mix reference kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromiumReferenceSet {
    pub generator: ChromiumReferenceGenerator,
    pub pages: &'static [ChromiumVisualReference],
}

/// Reviewed machine-readable artwork evidence for one exact form revision.
///
/// `Present` is intentionally explicit even though the provider also exposes
/// the reviewed PDF417/QR asset records. `Absent` must point at a committed,
/// hashed inventory of every physical page; an empty runtime asset list is
/// never treated as proof that the official PDF contains no symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineReadableArtworkEvidence {
    Present,
    Absent {
        audited_pages: &'static [usize],
        inventory_method: &'static str,
        object_inventory_path: &'static str,
        object_inventory_sha256: &'static str,
    },
}

#[derive(Debug)]
pub struct RenderFormProvider {
    pub code: &'static str,
    pub revision: &'static str,
    pub form_id: &'static str,
    pub title: &'static str,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub expected_base_page_count: usize,
    pub schedules: &'static [RenderSchedulePolicy],
    pub visual_fixture_file_name: &'static str,
    pub visual_fixture_sha256: &'static str,
    pub official_source: &'static str,
    pub official_source_sha256: &'static str,
    pub reference_dpi: u32,
    pub reference_width_px: u32,
    pub reference_height_px: u32,
    pub visual_reference_pages: &'static [VisualReferencePage],
    pub chromium_references: Option<&'static ChromiumReferenceSet>,
    pub machine_readable_artwork: MachineReadableArtworkEvidence,
    pub runtime_discrete_assets: fn() -> Vec<serde_json::Value>,
    pub fixtures: fn() -> Result<Vec<RenderContractFixture>, RenderProviderError>,
    pub generated_artifacts: fn() -> Result<Vec<GeneratedContractArtifact>, RenderProviderError>,
}

impl RenderFormProvider {
    pub fn key(&self) -> String {
        format!("{}:{}", self.code, self.revision)
    }

    pub fn page_geometry(&self) -> Result<RenderPageGeometry, RenderLayoutError> {
        let geometry = RenderPageGeometry::new(self.page_width_pt, self.page_height_pt);
        geometry.validate()?;
        Ok(geometry)
    }

    pub const fn pagination_policy(&self) -> RenderPaginationPolicy {
        RenderPaginationPolicy {
            base_page_count: self.expected_base_page_count,
            schedules: self.schedules,
        }
    }

    pub fn expected_page_count(
        &self,
        envelope: &RenderEnvelopeV1,
    ) -> Result<usize, RenderLayoutError> {
        if envelope.form.code != self.code || envelope.form.version != self.revision {
            return Err(RenderLayoutError::ProviderIdentityMismatch {
                provider: self.key(),
                envelope: format!("{}:{}", envelope.form.code, envelope.form.version),
            });
        }
        self.pagination_policy().expected_page_count(envelope)
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RenderLayoutError {
    #[error("no HTML render provider is registered for {code}:{revision}")]
    UnknownProvider { code: String, revision: String },
    #[error("render provider {provider} cannot accept envelope {envelope}")]
    ProviderIdentityMismatch { provider: String, envelope: String },
    #[error("invalid provider page geometry {width_points}x{height_points}pt")]
    InvalidPageGeometry {
        width_points: f64,
        height_points: f64,
    },
    #[error("invalid provider pagination policy: {0}")]
    InvalidPaginationPolicy(String),
    #[error("invalid pagination policy for schedule {schedule_id}: {reason}")]
    InvalidSchedulePolicy { schedule_id: String, reason: String },
    #[error("render envelope is missing provider-owned schedule {0}")]
    MissingSchedule(String),
    #[error("render envelope contains duplicate schedule {0}")]
    DuplicateSchedule(String),
    #[error("render envelope contains undeclared schedule {0}")]
    UnexpectedSchedule(String),
    #[error("expected renderer page count overflowed usize")]
    PageCountOverflow,
}

/// Fully resolved native-host layout for one immutable render envelope.
#[derive(Debug, Clone, Copy)]
pub struct RenderLayoutPlan {
    pub provider: &'static RenderFormProvider,
    pub page_geometry: RenderPageGeometry,
    pub expected_page_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderProviderError {
    #[error("failed to build renderer fixture: {0}")]
    Fixture(String),
    #[error("failed to serialize renderer provider artifact: {0}")]
    Serialization(#[from] serde_json::Error),
}

static PROVIDERS: &[RenderFormProvider] = &[
    form_0605::PROVIDER,
    form_0619e::PROVIDER,
    form_0619f::PROVIDER,
    form_1601c::PROVIDER,
    form_1701::PROVIDER,
    form_1701q::PROVIDER,
    form_1702mx::PROVIDER,
    form_1702rt::PROVIDER,
    form_2550q::PROVIDER,
    form_2551q::PROVIDER,
];

pub fn render_form_providers() -> &'static [RenderFormProvider] {
    PROVIDERS
}

pub fn render_form_provider(code: &str, revision: &str) -> Option<&'static RenderFormProvider> {
    PROVIDERS
        .iter()
        .find(|provider| provider.code == code && provider.revision == revision)
}

/// Resolve exact provider geometry and pagination for an immutable envelope.
/// Unknown or malformed identities fail closed before the native host starts.
pub fn render_layout_plan(
    envelope: &RenderEnvelopeV1,
) -> Result<RenderLayoutPlan, RenderLayoutError> {
    let provider =
        render_form_provider(&envelope.form.code, &envelope.form.version).ok_or_else(|| {
            RenderLayoutError::UnknownProvider {
                code: envelope.form.code.clone(),
                revision: envelope.form.version.clone(),
            }
        })?;
    Ok(RenderLayoutPlan {
        provider,
        page_geometry: provider.page_geometry()?,
        expected_page_count: provider.expected_page_count(envelope)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_exposes_every_current_html_form() {
        let form_0605 = render_form_provider("0605", "1999").expect("0605 provider");
        let form_0619e = render_form_provider("0619E", "2018").expect("0619E provider");
        let form_0619f = render_form_provider("0619F", "2018").expect("0619F provider");
        let provider = render_form_provider("2551Q", "2018").expect("2551Q provider");
        let form_1601c = render_form_provider("1601C", "2018").expect("1601C provider");
        let form_1701 = render_form_provider("1701", "2018").expect("1701 provider");
        let form_1701q = render_form_provider("1701Q", "2018").expect("1701Q provider");
        let form_1702mx = render_form_provider("1702MX", "2018C").expect("1702MX provider");
        let form_1702rt = render_form_provider("1702RT", "2018C").expect("1702RT provider");
        let form_2550q = render_form_provider("2550Q", "2024").expect("2550Q provider");

        assert_eq!(form_0605.form_id, "0605v1999");
        assert_eq!(form_0605.expected_base_page_count, 2);
        assert_eq!(
            form_0605.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_0605.visual_reference_pages.len(), 2);
        assert_eq!(form_0619e.form_id, "0619Ev2018");
        assert_eq!(form_0619e.expected_base_page_count, 1);
        assert_eq!(
            form_0619e.page_geometry().unwrap(),
            RenderPageGeometry::LETTER
        );
        assert_eq!(form_0619e.visual_reference_pages.len(), 1);
        assert_eq!(form_0619f.form_id, "0619Fv2018");
        assert_eq!(form_0619f.expected_base_page_count, 1);
        assert_eq!(
            form_0619f.page_geometry().unwrap(),
            RenderPageGeometry::LETTER
        );
        assert_eq!(form_0619f.visual_reference_pages.len(), 1);
        assert_eq!(provider.form_id, "2551Qv2018");
        assert_eq!(provider.expected_base_page_count, 2);
        assert_eq!(provider.page_geometry().unwrap(), RenderPageGeometry::LEGAL);
        assert_eq!(provider.visual_reference_pages.len(), 2);
        assert_eq!(form_1601c.form_id, "1601Cv2018");
        assert_eq!(form_1601c.expected_base_page_count, 2);
        assert_eq!(
            form_1601c.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_1601c.visual_reference_pages.len(), 2);
        assert_eq!(form_1701.form_id, "1701v2018");
        assert_eq!(form_1701.expected_base_page_count, 4);
        assert_eq!(
            form_1701.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_1701.visual_reference_pages.len(), 4);
        assert_eq!(form_1701q.form_id, "1701Qv2018");
        assert_eq!(form_1701q.expected_base_page_count, 2);
        assert_eq!(
            form_1701q.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_1701q.visual_reference_pages.len(), 2);
        assert_eq!(form_1702mx.form_id, "1702MXv2018C");
        assert_eq!(form_1702mx.expected_base_page_count, 4);
        assert_eq!(
            form_1702mx.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_1702mx.visual_reference_pages.len(), 4);
        assert_eq!(form_1702rt.form_id, "1702RTv2018C");
        assert_eq!(form_1702rt.expected_base_page_count, 4);
        assert_eq!(
            form_1702rt.page_geometry().unwrap(),
            RenderPageGeometry::LEGAL
        );
        assert_eq!(form_1702rt.visual_reference_pages.len(), 4);
        assert_eq!(form_2550q.form_id, "2550Qv2024");
        assert_eq!(form_2550q.expected_base_page_count, 2);
        assert_eq!(
            form_2550q.page_geometry().unwrap(),
            RenderPageGeometry::FOURTEEN_INCH
        );
        assert_eq!(form_2550q.visual_reference_pages.len(), 2);
        assert_eq!(render_form_providers().len(), 10);
    }

    #[test]
    fn named_page_geometries_cover_the_official_source_pack_sizes() {
        assert_eq!(
            RenderPageGeometry::LETTER,
            RenderPageGeometry::new(612.0, 792.0)
        );
        assert_eq!(
            RenderPageGeometry::LEGAL,
            RenderPageGeometry::new(612.0, 936.0)
        );
        assert_eq!(
            RenderPageGeometry::FOURTEEN_INCH,
            RenderPageGeometry::new(612.0, 1_008.0)
        );
        for geometry in [
            RenderPageGeometry::LETTER,
            RenderPageGeometry::LEGAL,
            RenderPageGeometry::FOURTEEN_INCH,
        ] {
            geometry.validate().expect("official geometry");
        }
    }

    #[test]
    fn provider_pagination_keeps_2551q_boundaries_exact() {
        let provider = render_form_provider("2551Q", "2018").unwrap();
        let mut envelope = (provider.fixtures)()
            .unwrap()
            .into_iter()
            .find(|fixture| fixture.kind == RenderFixtureKind::ScheduleCapacity)
            .unwrap()
            .envelope;
        let prototype = envelope.schedules[0]
            .rows
            .first()
            .cloned()
            .expect("capacity fixture row");

        for (rows, expected_pages) in [(0, 2), (6, 2), (7, 3), (18, 3), (19, 4), (30, 4), (31, 5)] {
            envelope.schedules[0].rows = std::iter::repeat_n(prototype.clone(), rows).collect();
            assert_eq!(
                provider.expected_page_count(&envelope).unwrap(),
                expected_pages
            );
        }
    }

    #[test]
    fn layout_plan_fails_closed_for_unknown_and_unowned_schedules() {
        let provider = render_form_provider("2551Q", "2018").unwrap();
        let mut envelope = (provider.fixtures)().unwrap().remove(0).envelope;
        envelope.form.code = "UNKNOWN".to_string();
        assert!(matches!(
            render_layout_plan(&envelope),
            Err(RenderLayoutError::UnknownProvider { .. })
        ));

        envelope.form.code = "2551Q".to_string();
        envelope.schedules[0].id = "undeclared".to_string();
        assert_eq!(
            render_layout_plan(&envelope).unwrap_err(),
            RenderLayoutError::UnexpectedSchedule("undeclared".to_string())
        );
    }

    #[test]
    fn every_provider_has_the_required_fixture_kinds() {
        for provider in render_form_providers() {
            let fixtures = (provider.fixtures)().expect("provider fixtures");
            for required in [
                RenderFixtureKind::Minimum,
                RenderFixtureKind::Normal,
                RenderFixtureKind::LongValues,
                RenderFixtureKind::ValidationEdge,
                RenderFixtureKind::ScheduleCapacity,
            ] {
                assert!(fixtures.iter().any(|fixture| fixture.kind == required));
            }
        }
    }
}
