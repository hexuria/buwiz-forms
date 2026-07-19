//! Development-only evidence transcript validation for native HTML PDF output.
//!
//! This module deliberately does not drive a WebView, create a PDF, update a
//! release manifest, or register a trusted evidence producer. It verifies a
//! transcript against separately supplied artifacts claimed by a future native
//! collector. It cannot prove that a particular bundle and envelope causally
//! produced the WKPDF bytes; that requires an attested runtime collector. The
//! default `bir-print` build excludes the module; opt in with the
//! `native-output-evidence` feature when wiring development evidence tooling.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::html::{RenderEnvelopeV1, RENDER_CONTRACT_VERSION};
use crate::html_forms::{render_layout_plan, RenderLayoutPlan};
use crate::html_output::{
    normalize_wkpdf_page_from_css_pixels, validate_pdf_bytes, HtmlOutputError, PdfExpectation,
    PdfValidationReport,
};
use crate::html_support::{validate_renderer_geometry, RendererGeometryReport, RendererPageRect};

pub const DEVELOPMENT_NATIVE_OUTPUT_TRANSCRIPT_SCHEMA_VERSION: u8 = 1;
pub const DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION: u8 = 1;
pub const OFFLINE_RENDERER_BUILD_IDENTITY_SCHEMA_VERSION: u8 = 1;
pub const OFFLINE_RENDERER_BUILD_IDENTITY_FILE_NAME: &str = "form-renderer-build-identity.json";
pub const OFFLINE_RENDERER_BUILD_IDENTITY_RELATIVE_PATH: &str = "form-renderer";
pub const MACOS_RENDERER_BUNDLE_RELATIVE_PATH: &str = "Contents/Resources/assets/form-renderer";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentEvidenceScope {
    DevelopmentDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeEvidencePlatform {
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePdfBackend {
    WkWebViewCreatePdf,
}

/// A non-promotional transcript claiming observations around one native export.
///
/// Every digest is recomputed by [`verify_development_native_output_transcript`].
/// Boolean fields are never sufficient on their own to prove a release gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentNativeOutputTranscriptV1 {
    pub schema_version: u8,
    pub scope: DevelopmentEvidenceScope,
    pub promotion_eligible: bool,
    pub platform: NativeEvidencePlatform,
    pub architecture: String,
    pub backend: NativePdfBackend,
    pub form_code: String,
    pub form_revision: String,
    pub source_revision: String,
    pub package_sha256: String,
    pub renderer_bundle_sha256: String,
    pub envelope_sha256: String,
    pub output_pdf_sha256: String,
    pub same_webview: SameWebViewEvidenceV1,
    pub geometry_reports: [GeometryReportEvidenceV1; 2],
    pub clipping_totals: ClippingCountersV1,
    pub nonce: NonceEvidenceV1,
    pub wkpdf_completion: WkPdfCompletionEvidenceV1,
    pub pdf_validation: PdfValidationEvidenceV1,
    pub destination_preservation: DestinationPreservationEvidenceV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SameWebViewEvidenceV1 {
    /// Opaque development-run identifier assigned when the prepared WebView is created.
    pub prepared_webview_id: String,
    /// Identifier observed when the nonce-bound print-mode preflight completes.
    pub preflight_webview_id: String,
    /// Identifier observed by the WKPDF completion callback.
    pub wkpdf_webview_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryReportEvidenceV1 {
    pub page_count: usize,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub pages: Vec<GeometryPageEvidenceV1>,
}

impl From<&RendererGeometryReport> for GeometryReportEvidenceV1 {
    fn from(report: &RendererGeometryReport) -> Self {
        Self {
            page_count: report.page_count,
            page_width_pt: report.page_width_pt,
            page_height_pt: report.page_height_pt,
            pages: report
                .pages
                .iter()
                .map(GeometryPageEvidenceV1::from)
                .collect(),
        }
    }
}

impl GeometryReportEvidenceV1 {
    fn to_renderer_report(&self) -> RendererGeometryReport {
        RendererGeometryReport {
            page_count: self.page_count,
            page_width_pt: self.page_width_pt,
            page_height_pt: self.page_height_pt,
            pages: self
                .pages
                .iter()
                .map(GeometryPageEvidenceV1::to_renderer_page)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryPageEvidenceV1 {
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

impl From<&RendererPageRect> for GeometryPageEvidenceV1 {
    fn from(page: &RendererPageRect) -> Self {
        Self {
            x: page.x,
            y: page.y,
            width: page.width,
            height: page.height,
            client_width: page.client_width,
            client_height: page.client_height,
            scroll_width: page.scroll_width,
            scroll_height: page.scroll_height,
            descendant_overflow_x: page.descendant_overflow_x,
            descendant_overflow_y: page.descendant_overflow_y,
            descendant_clipped_x: page.descendant_clipped_x,
            descendant_clipped_y: page.descendant_clipped_y,
        }
    }
}

impl GeometryPageEvidenceV1 {
    fn to_renderer_page(&self) -> RendererPageRect {
        RendererPageRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            client_width: self.client_width,
            client_height: self.client_height,
            scroll_width: self.scroll_width,
            scroll_height: self.scroll_height,
            descendant_overflow_x: self.descendant_overflow_x,
            descendant_overflow_y: self.descendant_overflow_y,
            descendant_clipped_x: self.descendant_clipped_x,
            descendant_clipped_y: self.descendant_clipped_y,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClippingCountersV1 {
    pub descendant_overflow_x: usize,
    pub descendant_overflow_y: usize,
    pub descendant_clipped_x: usize,
    pub descendant_clipped_y: usize,
}

impl ClippingCountersV1 {
    pub fn from_geometry(report: &GeometryReportEvidenceV1) -> Self {
        report
            .pages
            .iter()
            .fold(Self::default(), |mut totals, page| {
                totals.descendant_overflow_x = totals
                    .descendant_overflow_x
                    .saturating_add(page.descendant_overflow_x);
                totals.descendant_overflow_y = totals
                    .descendant_overflow_y
                    .saturating_add(page.descendant_overflow_y);
                totals.descendant_clipped_x = totals
                    .descendant_clipped_x
                    .saturating_add(page.descendant_clipped_x);
                totals.descendant_clipped_y = totals
                    .descendant_clipped_y
                    .saturating_add(page.descendant_clipped_y);
                totals
            })
    }

    fn is_zero(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonceEvidenceV1 {
    pub issued_nonce: u64,
    /// Must contain exactly one occurrence of `issued_nonce`.
    pub preflight_consumptions: Vec<u64>,
    pub wkpdf_completion_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WkPdfCompletionEvidenceV1 {
    pub callback_completed: bool,
    pub pages: Vec<WkPdfPageEvidenceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WkPdfPageEvidenceV1 {
    pub page_number: usize,
    pub succeeded: bool,
    pub byte_count: usize,
    pub sha256: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfValidationEvidenceV1 {
    pub lopdf_validated: bool,
    pub page_count: usize,
    pub width_points: f64,
    pub height_points: f64,
}

impl From<&PdfValidationReport> for PdfValidationEvidenceV1 {
    fn from(report: &PdfValidationReport) -> Self {
        Self {
            lopdf_validated: true,
            page_count: report.page_count,
            width_points: report.width_points,
            height_points: report.height_points,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationPreservationEvidenceV1 {
    /// Collector assertion. The verifier checks the accompanying before/after
    /// bytes and rejects nonzero cleanup counts, but it does not drive a native
    /// export failure itself.
    pub failure_observed: bool,
    pub failure_message: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub temporary_files_remaining: usize,
}

/// A runtime observation that is useful for diagnostics but is deliberately
/// incapable of satisfying [`verify_development_native_output_transcript`].
///
/// The native host writes this shape while collector provenance is incomplete.
/// Missing artifacts remain explicit `unavailable` values; they must never be
/// replaced with guessed hashes or duplicated observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentNativeOutputObservationV1 {
    pub schema_version: u8,
    pub scope: DevelopmentEvidenceScope,
    pub promotion_eligible: bool,
    pub platform: DevelopmentNativeOutputPlatformV1,
    pub backend: DevelopmentNativeOutputBackendV1,
    pub form_code: String,
    pub form_revision: String,
    pub document_run_id: String,
    pub envelope_sha256: String,
    pub source_revision: DevelopmentEvidenceAvailability<String>,
    pub package_sha256: DevelopmentEvidenceAvailability<String>,
    pub renderer_bundle_sha256: DevelopmentEvidenceAvailability<String>,
    pub independently_expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability<String>,
    pub geometry_reports: [GeometryReportEvidenceV1; 2],
    pub geometry_page_rect_sha256: Vec<String>,
    pub clipping_totals: ClippingCountersV1,
    pub nonce: NativeNonceObservationV1,
    pub render_epoch: u64,
    pub readiness_revision: u64,
    pub backend_completion: DevelopmentEvidenceAvailability<NativeBackendCompletionObservationV1>,
    pub native_page_payloads:
        DevelopmentEvidenceAvailability<Vec<NativePdfPagePayloadObservationV1>>,
    pub output_pdf_sha256: DevelopmentEvidenceAvailability<String>,
    pub pdf_validation: DevelopmentEvidenceAvailability<PdfValidationEvidenceV1>,
    pub destination_outcome: DevelopmentDestinationOutcomeV1,
    /// Concrete reasons this observation cannot be promoted or passed to the
    /// strict transcript verifier. At least one gap is always required.
    pub strict_verifier_gaps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentNativeOutputPlatformV1 {
    Macos,
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentNativeOutputBackendV1 {
    WkWebViewCreatePdf,
    WebView2PrintToPdf,
    WebKitGtkPrintOperationPdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DevelopmentEvidenceAvailability<T> {
    Observed { value: T },
    Unavailable { reason: String },
}

impl<T> DevelopmentEvidenceAvailability<T> {
    pub fn observed(value: T) -> Self {
        Self::Observed { value }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineRendererBuildIdentityScope {
    BuildTimeNonPromotionalIdentity,
}

/// Deterministic identity emitted by the offline verifier beside the renderer.
///
/// This is deliberately not a package attestation. The runtime must hash the
/// renderer independently and compare that observation with this expected
/// build-time value; package signing and an external collector remain separate
/// release-evidence requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfflineRendererBuildIdentityV1 {
    pub schema_version: u8,
    pub scope: OfflineRendererBuildIdentityScope,
    pub promotion_eligible: bool,
    pub offline_verification_passed: bool,
    pub renderer_bundle_relative_path: String,
    pub renderer_bundle_sha256: String,
    pub source_revision: DevelopmentEvidenceAvailability<String>,
}

impl OfflineRendererBuildIdentityV1 {
    pub fn validate_non_promotional(&self) -> Result<(), DevelopmentNativeOutputEvidenceError> {
        if self.schema_version != OFFLINE_RENDERER_BUILD_IDENTITY_SCHEMA_VERSION {
            return invalid(format!(
                "renderer build identity schema_version must be {}",
                OFFLINE_RENDERER_BUILD_IDENTITY_SCHEMA_VERSION
            ));
        }
        if self.scope != OfflineRendererBuildIdentityScope::BuildTimeNonPromotionalIdentity {
            return invalid(
                "renderer build identity scope must remain build_time_non_promotional_identity",
            );
        }
        if self.promotion_eligible {
            return invalid("renderer build identity must never be promotion eligible");
        }
        if !self.offline_verification_passed {
            return invalid("renderer build identity requires a passed offline verification");
        }
        if self.renderer_bundle_relative_path != OFFLINE_RENDERER_BUILD_IDENTITY_RELATIVE_PATH {
            return invalid("renderer build identity has an unexpected bundle-relative path");
        }
        verify_digest(
            "renderer build identity bundle sha256",
            &self.renderer_bundle_sha256,
        )?;
        match &self.source_revision {
            DevelopmentEvidenceAvailability::Observed { value }
                if value.len() == 40
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) => {}
            DevelopmentEvidenceAvailability::Observed { .. } => {
                return invalid(
                    "renderer build identity source revision must be a canonical Git commit",
                );
            }
            DevelopmentEvidenceAvailability::Unavailable { reason } if reason.trim().is_empty() => {
                return invalid(
                    "renderer build identity source-revision unavailability requires a reason",
                );
            }
            DevelopmentEvidenceAvailability::Unavailable { .. } => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeNonceObservationV1 {
    pub issued_nonce: u64,
    pub preflight_consumptions: Vec<u64>,
    pub backend_completion_nonce: DevelopmentEvidenceAvailability<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBackendCompletionObservationV1 {
    pub nonce: u64,
    pub document_run_id: String,
    pub envelope_sha256: String,
    pub render_epoch: u64,
    pub succeeded: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePdfPagePayloadObservationV1 {
    pub page_number: usize,
    pub succeeded: bool,
    pub byte_count: usize,
    pub sha256: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DevelopmentDestinationSnapshotV1 {
    Absent,
    File { sha256: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum DevelopmentDestinationOutcomeV1 {
    ExportSucceeded {
        before: DevelopmentDestinationSnapshotV1,
        after: DevelopmentDestinationSnapshotV1,
        temporary_file_remaining: bool,
        preservation_failure_case_exercised: bool,
    },
    ExportFailed {
        message: String,
        before: DevelopmentDestinationSnapshotV1,
        after: DevelopmentDestinationSnapshotV1,
        temporary_file_remaining: bool,
        destination_preserved: DevelopmentEvidenceAvailability<bool>,
    },
    NotApplicable {
        reason: String,
    },
}

impl DevelopmentNativeOutputObservationV1 {
    /// Validate only the internal consistency of a diagnostic observation.
    /// This intentionally does not turn it into strict or promotional evidence.
    pub fn validate_non_promotional(&self) -> Result<(), DevelopmentNativeOutputEvidenceError> {
        if self.schema_version != DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION {
            return invalid(format!(
                "observation schema_version must be {}",
                DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION
            ));
        }
        if self.scope != DevelopmentEvidenceScope::DevelopmentDiagnostic {
            return invalid("observation scope must remain development_diagnostic");
        }
        if self.promotion_eligible {
            return invalid("runtime observations must never be promotion eligible");
        }
        if self.document_run_id.trim().is_empty() {
            return invalid("runtime observation document_run_id is required");
        }
        if self.form_code.trim().is_empty() || self.form_revision.trim().is_empty() {
            return invalid("runtime observation form code and revision are required");
        }
        if !matches!(
            (self.platform, self.backend),
            (
                DevelopmentNativeOutputPlatformV1::Macos,
                DevelopmentNativeOutputBackendV1::WkWebViewCreatePdf
            ) | (
                DevelopmentNativeOutputPlatformV1::Windows,
                DevelopmentNativeOutputBackendV1::WebView2PrintToPdf
            ) | (
                DevelopmentNativeOutputPlatformV1::Linux,
                DevelopmentNativeOutputBackendV1::WebKitGtkPrintOperationPdf
            )
        ) {
            return invalid("runtime observation platform and backend do not match");
        }
        if self.render_epoch == 0 || self.readiness_revision == 0 {
            return invalid("runtime observation requires a nonzero epoch and readiness revision");
        }
        verify_digest("runtime observation envelope sha256", &self.envelope_sha256)?;
        verify_available_digest("source revision", &self.source_revision, true)?;
        verify_available_digest("package sha256", &self.package_sha256, false)?;
        verify_available_digest(
            "renderer bundle sha256",
            &self.renderer_bundle_sha256,
            false,
        )?;
        verify_available_digest(
            "independently expected renderer bundle sha256",
            &self.independently_expected_renderer_bundle_sha256,
            false,
        )?;

        let [first, second] = &self.geometry_reports;
        if first != second {
            return invalid("runtime observation geometry reports are not identical");
        }
        if first.page_count == 0 || first.pages.len() != first.page_count {
            return invalid("runtime observation geometry report has an invalid page count");
        }
        let expected_rect_hashes = geometry_page_rect_sha256(first)?;
        if self.geometry_page_rect_sha256 != expected_rect_hashes {
            return invalid("runtime observation page-rectangle hashes do not match the reports");
        }
        let clipping_totals = ClippingCountersV1::from_geometry(first);
        if self.clipping_totals != clipping_totals || !clipping_totals.is_zero() {
            return invalid("runtime observation contains clipping or inconsistent counters");
        }
        if self.nonce.issued_nonce == 0
            || self.nonce.preflight_consumptions.as_slice() != [self.nonce.issued_nonce]
        {
            return invalid("runtime observation nonce was not consumed exactly once");
        }
        validate_observation_completion(self)?;
        validate_observation_page_payloads(&self.native_page_payloads, first.page_count)?;
        verify_available_digest("output PDF sha256", &self.output_pdf_sha256, false)?;
        validate_observation_pdf_validation(&self.pdf_validation, first)?;
        validate_destination_outcome(&self.destination_outcome, &self.output_pdf_sha256)?;
        if self.strict_verifier_gaps.is_empty()
            || self
                .strict_verifier_gaps
                .iter()
                .any(|gap| gap.trim().is_empty())
        {
            return invalid("runtime observation must record concrete strict-verifier gaps");
        }
        Ok(())
    }
}

/// Serialize an internally consistent diagnostic observation. The resulting
/// JSON is not accepted by the strict transcript verifier.
pub fn encode_development_native_output_observation(
    observation: &DevelopmentNativeOutputObservationV1,
) -> Result<Vec<u8>, DevelopmentNativeOutputEvidenceError> {
    observation.validate_non_promotional()?;
    let mut encoded = serde_json::to_vec_pretty(observation)?;
    encoded.push(b'\n');
    Ok(encoded)
}

/// Decode and validate an app-written native-output observation without
/// treating it as a release transcript or promotion evidence.
///
/// Keeping this decoder beside the encoder gives operator tooling the exact
/// same fail-closed validation used by the native host. A valid observation
/// remains development-only and must retain at least one explicit strict
/// verifier gap.
pub fn decode_development_native_output_observation(
    bytes: &[u8],
) -> Result<DevelopmentNativeOutputObservationV1, DevelopmentNativeOutputEvidenceError> {
    let observation: DevelopmentNativeOutputObservationV1 = serde_json::from_slice(bytes)?;
    observation.validate_non_promotional()?;
    Ok(observation)
}

/// Decode and validate the build-time identity without treating it as a
/// package signature or release transcript.
pub fn decode_offline_renderer_build_identity(
    bytes: &[u8],
) -> Result<OfflineRendererBuildIdentityV1, DevelopmentNativeOutputEvidenceError> {
    let identity: OfflineRendererBuildIdentityV1 = serde_json::from_slice(bytes)?;
    identity.validate_non_promotional()?;
    Ok(identity)
}

pub fn geometry_page_rect_sha256(
    report: &GeometryReportEvidenceV1,
) -> Result<Vec<String>, DevelopmentNativeOutputEvidenceError> {
    report
        .pages
        .iter()
        .map(|page| {
            serde_json::to_vec(page)
                .map(|bytes| sha256_bytes(&bytes))
                .map_err(Into::into)
        })
        .collect()
}

fn verify_available_digest<T: AsRef<str>>(
    field: &str,
    value: &DevelopmentEvidenceAvailability<T>,
    allow_git_object_id: bool,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match value {
        DevelopmentEvidenceAvailability::Observed { value } => {
            let value = value.as_ref();
            if allow_git_object_id && value.len() == 40 {
                if value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                {
                    return Ok(());
                }
                return invalid(format!("{field} must be canonical lowercase hex"));
            }
            verify_digest(field, value)
        }
        DevelopmentEvidenceAvailability::Unavailable { reason } => {
            if reason.trim().is_empty() {
                return invalid(format!("{field} unavailability requires a reason"));
            }
            Ok(())
        }
    }
}

fn validate_observation_completion(
    observation: &DevelopmentNativeOutputObservationV1,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match &observation.backend_completion {
        DevelopmentEvidenceAvailability::Observed { value } => {
            if value.nonce != observation.nonce.issued_nonce
                || value.document_run_id != observation.document_run_id
                || value.envelope_sha256 != observation.envelope_sha256
                || value.render_epoch != observation.render_epoch
            {
                return invalid(
                    "runtime backend completion is not bound to document, envelope, nonce, and epoch",
                );
            }
            if value.succeeded == value.error.is_some() {
                return invalid("runtime backend completion success/error fields are inconsistent");
            }
            match observation.nonce.backend_completion_nonce {
                DevelopmentEvidenceAvailability::Observed { value: nonce }
                    if nonce == value.nonce => {}
                _ => return invalid("runtime completion nonce is missing or inconsistent"),
            }
        }
        DevelopmentEvidenceAvailability::Unavailable { reason } if reason.trim().is_empty() => {
            return invalid("backend completion unavailability requires a reason");
        }
        DevelopmentEvidenceAvailability::Unavailable { .. } => {}
    }
    Ok(())
}

fn validate_observation_page_payloads(
    payloads: &DevelopmentEvidenceAvailability<Vec<NativePdfPagePayloadObservationV1>>,
    expected_page_count: usize,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match payloads {
        DevelopmentEvidenceAvailability::Observed { value } => {
            if value.len() != expected_page_count {
                return invalid("observed native page payload count differs from geometry");
            }
            for (index, page) in value.iter().enumerate() {
                if page.page_number != index + 1 {
                    return invalid("native page payload observations are out of order");
                }
                match (
                    page.succeeded,
                    page.sha256.as_deref(),
                    page.error.as_deref(),
                ) {
                    (true, Some(hash), None) if page.byte_count > 0 => {
                        verify_digest("native page payload sha256", hash)?;
                    }
                    (false, None, Some(error)) if !error.trim().is_empty() => {}
                    _ => return invalid("native page payload observation is inconsistent"),
                }
            }
        }
        DevelopmentEvidenceAvailability::Unavailable { reason } if reason.trim().is_empty() => {
            return invalid("native page payload unavailability requires a reason");
        }
        DevelopmentEvidenceAvailability::Unavailable { .. } => {}
    }
    Ok(())
}

fn validate_observation_pdf_validation(
    validation: &DevelopmentEvidenceAvailability<PdfValidationEvidenceV1>,
    geometry: &GeometryReportEvidenceV1,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match validation {
        DevelopmentEvidenceAvailability::Observed { value } => {
            if !value.lopdf_validated
                || value.page_count != geometry.page_count
                || value.width_points != geometry.page_width_pt
                || value.height_points != geometry.page_height_pt
            {
                return invalid("runtime PDF validation differs from renderer geometry");
            }
        }
        DevelopmentEvidenceAvailability::Unavailable { reason } if reason.trim().is_empty() => {
            return invalid("PDF validation unavailability requires a reason");
        }
        DevelopmentEvidenceAvailability::Unavailable { .. } => {}
    }
    Ok(())
}

fn validate_destination_snapshot(
    snapshot: &DevelopmentDestinationSnapshotV1,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match snapshot {
        DevelopmentDestinationSnapshotV1::File { sha256 } => {
            verify_digest("destination snapshot sha256", sha256)
        }
        DevelopmentDestinationSnapshotV1::Unavailable { reason } if reason.trim().is_empty() => {
            invalid("destination snapshot unavailability requires a reason")
        }
        DevelopmentDestinationSnapshotV1::Absent
        | DevelopmentDestinationSnapshotV1::Unavailable { .. } => Ok(()),
    }
}

fn validate_destination_outcome(
    outcome: &DevelopmentDestinationOutcomeV1,
    output_pdf_sha256: &DevelopmentEvidenceAvailability<String>,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    match outcome {
        DevelopmentDestinationOutcomeV1::ExportSucceeded {
            before,
            after,
            temporary_file_remaining,
            preservation_failure_case_exercised,
        } => {
            validate_destination_snapshot(before)?;
            validate_destination_snapshot(after)?;
            if *temporary_file_remaining || *preservation_failure_case_exercised {
                return invalid(
                    "successful export cannot claim a destination-preservation failure exercise",
                );
            }
            if !matches!(after, DevelopmentDestinationSnapshotV1::File { .. }) {
                return invalid("successful export must retain the final destination hash");
            }
            match (after, output_pdf_sha256) {
                (
                    DevelopmentDestinationSnapshotV1::File {
                        sha256: destination,
                    },
                    DevelopmentEvidenceAvailability::Observed { value: output },
                ) if destination == output => {}
                _ => {
                    return invalid(
                        "successful export destination and output PDF hashes are inconsistent",
                    );
                }
            }
        }
        DevelopmentDestinationOutcomeV1::ExportFailed {
            message,
            before,
            after,
            temporary_file_remaining: _,
            destination_preserved,
        } => {
            if message.trim().is_empty() {
                return invalid("failed export observation requires an error message");
            }
            validate_destination_snapshot(before)?;
            validate_destination_snapshot(after)?;
            if let DevelopmentEvidenceAvailability::Unavailable { reason } = destination_preserved {
                if reason.trim().is_empty() {
                    return invalid("destination-preservation unavailability requires a reason");
                }
            }
            if let DevelopmentEvidenceAvailability::Observed { value } = destination_preserved {
                if *value != (before == after) {
                    return invalid(
                        "destination-preservation result differs from the before/after snapshots",
                    );
                }
            }
        }
        DevelopmentDestinationOutcomeV1::NotApplicable { reason } if reason.trim().is_empty() => {
            return invalid("destination outcome not-applicable reason is required");
        }
        DevelopmentDestinationOutcomeV1::NotApplicable { .. } => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum EvidenceArtifactSource<'a> {
    File(&'a Path),
    Directory(&'a Path),
}

/// Actual artifacts used to independently verify a transcript.
///
/// `wkpdf_page_payloads` are the bytes returned by WKWebView callbacks before
/// the existing merger/finalizer runs. `output_pdf` is the finalized, stamped
/// destination validated by `lopdf`.
pub struct DevelopmentNativeOutputArtifacts<'a> {
    /// Source revision independently resolved by the future runtime collector.
    /// This pure verifier checks canonical syntax and transcript equality only.
    pub source_revision: &'a str,
    pub package: EvidenceArtifactSource<'a>,
    /// Expected tree hash supplied independently by the offline build gate.
    /// There is no authoritative runtime bundle manifest in the package yet,
    /// so this remains diagnostic input rather than promotion evidence.
    pub expected_renderer_bundle_sha256: &'a str,
    pub envelope_json: &'a [u8],
    pub wkpdf_page_payloads: Vec<&'a [u8]>,
    pub output_pdf: &'a Path,
    pub destination_before_failure: &'a [u8],
    pub destination_after_failure: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub struct DevelopmentNativeOutputVerification {
    pub transcript_sha256: String,
    pub output_pdf_sha256: String,
    pub pdf_validation: PdfValidationReport,
}

#[derive(Debug, thiserror::Error)]
pub enum DevelopmentNativeOutputEvidenceError {
    #[error("invalid development native-output transcript: {0}")]
    Invalid(String),
    #[error("could not read evidence artifact {path}: {source}")]
    ArtifactIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("native output PDF validation failed: {0}")]
    Pdf(#[from] HtmlOutputError),
    #[error("could not encode or decode native-output evidence JSON: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Verify a development transcript against real artifacts without producing
/// or modifying any PDF bytes.
pub fn verify_development_native_output_transcript(
    transcript: &DevelopmentNativeOutputTranscriptV1,
    artifacts: &DevelopmentNativeOutputArtifacts<'_>,
    expectation: &PdfExpectation,
    layout_plan: &RenderLayoutPlan,
) -> Result<DevelopmentNativeOutputVerification, DevelopmentNativeOutputEvidenceError> {
    if transcript.schema_version != DEVELOPMENT_NATIVE_OUTPUT_TRANSCRIPT_SCHEMA_VERSION {
        return invalid(format!(
            "schema_version must be {}",
            DEVELOPMENT_NATIVE_OUTPUT_TRANSCRIPT_SCHEMA_VERSION
        ));
    }
    if transcript.scope != DevelopmentEvidenceScope::DevelopmentDiagnostic {
        return invalid("scope must remain development_diagnostic");
    }
    if transcript.promotion_eligible {
        return invalid("development transcripts must never be promotion eligible");
    }
    if transcript.platform != NativeEvidencePlatform::Macos
        || transcript.backend != NativePdfBackend::WkWebViewCreatePdf
    {
        return invalid("this foundation accepts only the macOS WKWebView PDF backend");
    }
    if transcript.architecture.trim().is_empty() {
        return invalid("architecture is required");
    }
    let envelope: RenderEnvelopeV1 = serde_json::from_slice(artifacts.envelope_json)?;
    if envelope.schema_version != RENDER_CONTRACT_VERSION {
        return invalid(format!(
            "render envelope schema_version must be {RENDER_CONTRACT_VERSION}"
        ));
    }
    let derived_layout_plan = render_layout_plan(&envelope).map_err(|error| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "immutable render envelope has no valid layout plan: {error}"
        ))
    })?;
    verify_layout_binding(transcript, expectation, layout_plan, &derived_layout_plan)?;

    verify_source_revision(&transcript.source_revision, artifacts.source_revision)?;
    verify_digest("package_sha256", &transcript.package_sha256)?;
    verify_digest("renderer_bundle_sha256", &transcript.renderer_bundle_sha256)?;
    verify_digest("envelope_sha256", &transcript.envelope_sha256)?;
    verify_digest("output_pdf_sha256", &transcript.output_pdf_sha256)?;

    verify_digest(
        "expected renderer bundle sha256",
        artifacts.expected_renderer_bundle_sha256,
    )?;
    let (package_hash, renderer_bundle_hash) = hash_macos_package_and_renderer(artifacts.package)?;
    compare_hash("package", &transcript.package_sha256, &package_hash)?;
    compare_hash(
        "renderer bundle",
        &transcript.renderer_bundle_sha256,
        &renderer_bundle_hash,
    )?;
    compare_hash(
        "independently expected renderer bundle",
        artifacts.expected_renderer_bundle_sha256,
        &renderer_bundle_hash,
    )?;
    let envelope_hash = sha256_bytes(artifacts.envelope_json);
    compare_hash("envelope", &transcript.envelope_sha256, &envelope_hash)?;
    compare_hash(
        "PDF expectation envelope",
        &expectation.envelope_hash,
        &envelope_hash,
    )?;

    verify_same_webview(&transcript.same_webview)?;
    verify_geometry(transcript, &derived_layout_plan)?;
    verify_nonce(&transcript.nonce)?;
    let captured_page_contents = verify_wkpdf_completion(
        &transcript.wkpdf_completion,
        &transcript.nonce,
        &artifacts.wkpdf_page_payloads,
        expectation,
    )?;

    let output_pdf_bytes = read_regular_file(artifacts.output_pdf)?;
    let output_pdf_sha256 = sha256_bytes(&output_pdf_bytes);
    compare_hash(
        "final output PDF",
        &transcript.output_pdf_sha256,
        &output_pdf_sha256,
    )?;
    let pdf_validation = validate_pdf_bytes(&output_pdf_bytes, expectation)?;
    verify_pdf_validation(&transcript.pdf_validation, &pdf_validation)?;
    let output_document = Document::load_mem(&output_pdf_bytes).map_err(|error| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "final output PDF could not be parsed for WKPDF binding: {error}"
        ))
    })?;
    reject_unsupported_pdf_visual_state(&output_document, "final output PDF")?;
    verify_final_pdf_matches_wkpdf(&output_document, &captured_page_contents)?;
    verify_destination_preservation(
        &transcript.destination_preservation,
        artifacts.destination_before_failure,
        artifacts.destination_after_failure,
    )?;

    let transcript_sha256 = sha256_bytes(&serde_json::to_vec(transcript)?);
    Ok(DevelopmentNativeOutputVerification {
        transcript_sha256,
        output_pdf_sha256,
        pdf_validation,
    })
}

fn verify_layout_binding(
    transcript: &DevelopmentNativeOutputTranscriptV1,
    expectation: &PdfExpectation,
    supplied: &RenderLayoutPlan,
    derived: &RenderLayoutPlan,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if transcript.form_code != expectation.form_code
        || transcript.form_revision != expectation.revision
        || transcript.form_code != derived.provider.code
        || transcript.form_revision != derived.provider.revision
    {
        return invalid("form identity differs from the envelope-derived layout provider");
    }
    if supplied.provider.code != derived.provider.code
        || supplied.provider.revision != derived.provider.revision
        || supplied.page_geometry != derived.page_geometry
        || supplied.expected_page_count != derived.expected_page_count
    {
        return invalid("caller-supplied layout plan differs from the immutable envelope");
    }
    if expectation.expected_page_count != derived.expected_page_count
        || expectation.width_points != derived.page_geometry.width_points
        || expectation.height_points != derived.page_geometry.height_points
    {
        return invalid("PDF expectation differs from the immutable envelope layout");
    }
    Ok(())
}

fn verify_source_revision(
    transcript_revision: &str,
    observed_revision: &str,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    let canonical = |value: &str| {
        matches!(value.len(), 40 | 64)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    };
    if !canonical(transcript_revision) {
        return invalid("source_revision must be a canonical lowercase Git object ID");
    }
    if transcript_revision != observed_revision {
        return invalid("source_revision differs from the observed source revision");
    }
    Ok(())
}

fn verify_same_webview(
    evidence: &SameWebViewEvidenceV1,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if evidence.prepared_webview_id.trim().is_empty() {
        return invalid("prepared WebView identifier is required");
    }
    if evidence.prepared_webview_id != evidence.preflight_webview_id
        || evidence.prepared_webview_id != evidence.wkpdf_webview_id
    {
        return invalid("prepare, preflight, and WKPDF completion must use the same WebView");
    }
    Ok(())
}

fn verify_geometry(
    transcript: &DevelopmentNativeOutputTranscriptV1,
    layout_plan: &RenderLayoutPlan,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    let [first, second] = &transcript.geometry_reports;
    if first != second {
        return invalid("the two renderer geometry reports are not identical");
    }
    for (index, geometry) in transcript.geometry_reports.iter().enumerate() {
        validate_renderer_geometry(&geometry.to_renderer_report(), layout_plan).map_err(
            |error| {
                DevelopmentNativeOutputEvidenceError::Invalid(format!(
                    "geometry report {} failed host validation: {error}",
                    index + 1
                ))
            },
        )?;
    }
    let clipping_totals = ClippingCountersV1::from_geometry(first);
    if transcript.clipping_totals != clipping_totals {
        return invalid("recorded clipping totals do not match renderer geometry");
    }
    if !clipping_totals.is_zero() {
        return invalid("renderer geometry contains clipping or descendant overflow");
    }
    Ok(())
}

fn verify_nonce(evidence: &NonceEvidenceV1) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if evidence.issued_nonce == 0 {
        return invalid("native output nonce must be nonzero");
    }
    if evidence.preflight_consumptions.as_slice() != [evidence.issued_nonce] {
        return invalid("native output nonce must be consumed exactly once by preflight");
    }
    if evidence.wkpdf_completion_nonce != evidence.issued_nonce {
        return invalid("WKPDF completion nonce differs from the issued nonce");
    }
    Ok(())
}

fn verify_wkpdf_completion(
    completion: &WkPdfCompletionEvidenceV1,
    nonce: &NonceEvidenceV1,
    payloads: &[&[u8]],
    expectation: &PdfExpectation,
) -> Result<Vec<String>, DevelopmentNativeOutputEvidenceError> {
    if !completion.callback_completed {
        return invalid("WKPDF completion callback did not complete");
    }
    if completion.pages.len() != expectation.expected_page_count
        || payloads.len() != expectation.expected_page_count
    {
        return invalid("WKPDF page evidence does not match the expected page count");
    }
    if nonce.wkpdf_completion_nonce != nonce.issued_nonce {
        return invalid("WKPDF completion is not bound to the issued nonce");
    }
    let mut captured_contents = Vec::with_capacity(payloads.len());
    for (index, (page, payload)) in completion.pages.iter().zip(payloads).enumerate() {
        let page_number = index + 1;
        if page.page_number != page_number {
            return invalid(format!("WKPDF page {page_number} is out of order"));
        }
        if !page.succeeded || page.error.is_some() {
            return invalid(format!("WKPDF page {page_number} did not succeed"));
        }
        if page.byte_count != payload.len() || payload.is_empty() {
            return invalid(format!(
                "WKPDF page {page_number} byte count is inconsistent"
            ));
        }
        verify_digest("WKPDF page sha256", &page.sha256)?;
        compare_hash(
            &format!("WKPDF page {page_number}"),
            &page.sha256,
            &sha256_bytes(payload),
        )?;
        captured_contents.push(verify_single_page_wkpdf(payload, page_number, expectation)?);
    }
    Ok(captured_contents)
}

fn verify_single_page_wkpdf(
    payload: &[u8],
    page_number: usize,
    expectation: &PdfExpectation,
) -> Result<String, DevelopmentNativeOutputEvidenceError> {
    let normalized =
        normalize_wkpdf_page_from_css_pixels(payload, expectation).map_err(|error| {
            DevelopmentNativeOutputEvidenceError::Invalid(format!(
                "WKPDF page {page_number} could not be normalized from CSS pixels: {error}"
            ))
        })?;
    let document = Document::load_mem(&normalized).map_err(|error| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "normalized WKPDF page {page_number} is not a valid PDF: {error}"
        ))
    })?;
    reject_unsupported_pdf_visual_state(&document, &format!("WKPDF page {page_number}"))?;
    let pages = document.get_pages();
    if pages.len() != 1 {
        return invalid(format!(
            "WKPDF page {page_number} capture must contain exactly one PDF page"
        ));
    }
    let page_id = *pages.values().next().expect("one page was checked above");
    canonical_page_render_hash(&document, page_id).map_err(|error| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "WKPDF page {page_number} render graph could not be hashed: {error}"
        ))
    })
}

fn reject_unsupported_pdf_visual_state(
    document: &Document,
    label: &str,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    let catalog = document.catalog().map_err(|error| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "{label} has no readable PDF catalog: {error}"
        ))
    })?;
    for (key, description) in [
        (b"AcroForm".as_slice(), "catalog AcroForm state"),
        (b"OCProperties".as_slice(), "catalog optional-content state"),
        (b"OutputIntents".as_slice(), "catalog output-intent state"),
        (b"OpenAction".as_slice(), "catalog open action"),
        (b"AA".as_slice(), "catalog additional actions"),
        (
            b"ViewerPreferences".as_slice(),
            "catalog viewer/print preferences",
        ),
    ] {
        if catalog.has(key) {
            return invalid(format!(
                "{label} contains unsupported {description} outside the page render graph"
            ));
        }
    }
    for object in document.objects.values() {
        if contains_external_stream(object) {
            return invalid(format!(
                "{label} contains an unsupported stream backed by external file data"
            ));
        }
    }
    for (_, object) in document.trailer.iter() {
        if contains_external_stream(object) {
            return invalid(format!(
                "{label} contains an unsupported stream backed by external file data"
            ));
        }
    }
    Ok(())
}

fn contains_external_stream(root: &Object) -> bool {
    let mut pending = vec![root];
    while let Some(object) = pending.pop() {
        match object {
            Object::Array(values) => pending.extend(values),
            Object::Dictionary(dictionary) => {
                pending.extend(dictionary.iter().map(|(_, value)| value))
            }
            Object::Stream(stream) => {
                if stream.dict.has(b"F")
                    || stream.dict.has(b"FFilter")
                    || stream.dict.has(b"FDecodeParms")
                {
                    return true;
                }
                pending.extend(stream.dict.iter().map(|(_, value)| value));
            }
            Object::Null
            | Object::Boolean(_)
            | Object::Integer(_)
            | Object::Real(_)
            | Object::Name(_)
            | Object::String(_, _)
            | Object::Reference(_) => {}
        }
    }
    false
}

fn verify_final_pdf_matches_wkpdf(
    document: &Document,
    captured_page_hashes: &[String],
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    let pages = document.get_pages();
    if pages.len() != captured_page_hashes.len() {
        return invalid("final output page count differs from WKPDF captures");
    }
    for ((page_number, page_id), captured_hash) in pages.into_iter().zip(captured_page_hashes) {
        let output_hash = canonical_page_render_hash(document, page_id).map_err(|error| {
            DevelopmentNativeOutputEvidenceError::Invalid(format!(
                "final output page {page_number} render graph could not be hashed: {error}"
            ))
        })?;
        if output_hash != *captured_hash {
            return invalid(format!(
                "final output page {page_number} is not bound to its WKPDF capture"
            ));
        }
    }
    Ok(())
}

/// Hash the page dictionary and its reachable object graph without depending
/// on PDF object numbers. Catalog-level state is rejected separately because
/// it is outside this page-reachable comparison. WKWebView captures are merged
/// into a new object namespace, so identifiers cannot be part of the hash.
fn canonical_page_render_hash(
    document: &Document,
    page_id: ObjectId,
) -> Result<String, HtmlOutputError> {
    let mut page = document
        .get_dictionary(page_id)
        .map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?
        .clone();
    for key in [b"MediaBox".as_slice(), b"CropBox", b"Resources", b"Rotate"] {
        if !page.has(key) {
            if let Some(value) = inherited_page_value(document, page_id, key) {
                page.set(key, value);
            }
        }
    }
    page.remove(b"Parent");

    let mut state = CanonicalPageHashState {
        document,
        hasher: Sha256::new(),
        references: BTreeMap::new(),
        next_reference: 0,
    };
    state.hasher.update(b"ebirforms-wkpdf-page-v1\0");
    state.hash_dictionary(&page, &[])?;
    Ok(format!("{:x}", state.hasher.finalize()))
}

fn inherited_page_value(document: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page_id;
    let mut visited = std::collections::BTreeSet::new();
    while visited.insert(current) {
        let dictionary = document.get_dictionary(current).ok()?;
        if let Ok(value) = dictionary.get(key) {
            return Some(value.clone());
        }
        current = dictionary.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

struct CanonicalPageHashState<'a> {
    document: &'a Document,
    hasher: Sha256,
    references: BTreeMap<ObjectId, u64>,
    next_reference: u64,
}

impl CanonicalPageHashState<'_> {
    fn hash_object(&mut self, object: &Object) -> Result<(), HtmlOutputError> {
        match object {
            Object::Null => self.hasher.update(b"null\0"),
            Object::Boolean(value) => {
                self.hasher.update(b"bool\0");
                self.hasher.update([u8::from(*value)]);
            }
            Object::Integer(value) => {
                self.hasher.update(b"integer\0");
                self.hasher.update(value.to_be_bytes());
            }
            Object::Real(value) => {
                self.hasher.update(b"real\0");
                self.hasher.update(value.to_bits().to_be_bytes());
            }
            Object::Name(value) => {
                self.hasher.update(b"name\0");
                self.hash_bytes(value);
            }
            Object::String(value, format) => {
                self.hasher.update(b"string\0");
                self.hasher.update([match format {
                    lopdf::StringFormat::Literal => 0,
                    lopdf::StringFormat::Hexadecimal => 1,
                }]);
                self.hash_bytes(value);
            }
            Object::Array(values) => {
                self.hasher.update(b"array\0");
                self.hash_length(values.len());
                for value in values {
                    self.hash_object(value)?;
                }
            }
            Object::Dictionary(dictionary) => {
                self.hasher.update(b"dictionary\0");
                self.hash_dictionary(dictionary, &[])?;
            }
            Object::Stream(stream) => {
                self.hasher.update(b"stream\0");
                // Length is derived from the stream bytes and may be rewritten
                // by lopdf serialization without changing the page graph.
                self.hash_dictionary(&stream.dict, &[b"Length"])?;
                self.hash_bytes(&stream.content);
            }
            Object::Reference(object_id) => {
                self.hasher.update(b"reference\0");
                if let Some(reference) = self.references.get(object_id).copied() {
                    self.hasher.update(b"seen\0");
                    self.hasher.update(reference.to_be_bytes());
                } else {
                    let reference = self.next_reference;
                    self.next_reference = self.next_reference.checked_add(1).ok_or_else(|| {
                        HtmlOutputError::InvalidPdf(
                            "canonical page reference count overflowed".to_string(),
                        )
                    })?;
                    self.references.insert(*object_id, reference);
                    self.hasher.update(b"new\0");
                    self.hasher.update(reference.to_be_bytes());
                    let resolved = self
                        .document
                        .get_object(*object_id)
                        .map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?;
                    self.hash_object(resolved)?;
                }
            }
        }
        Ok(())
    }

    fn hash_dictionary(
        &mut self,
        dictionary: &Dictionary,
        ignored_keys: &[&[u8]],
    ) -> Result<(), HtmlOutputError> {
        let mut entries = dictionary
            .iter()
            .filter(|(key, _)| !ignored_keys.contains(&key.as_slice()))
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|(key, _)| *key);
        self.hash_length(entries.len());
        for (key, value) in entries {
            self.hash_bytes(key);
            self.hash_object(value)?;
        }
        Ok(())
    }

    fn hash_bytes(&mut self, bytes: &[u8]) {
        self.hash_length(bytes.len());
        self.hasher.update(bytes);
    }

    fn hash_length(&mut self, length: usize) {
        self.hasher.update((length as u64).to_be_bytes());
    }
}

fn verify_pdf_validation(
    evidence: &PdfValidationEvidenceV1,
    observed: &PdfValidationReport,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if !evidence.lopdf_validated
        || evidence.page_count != observed.page_count
        || evidence.width_points != observed.width_points
        || evidence.height_points != observed.height_points
    {
        return invalid("recorded lopdf validation differs from independent validation");
    }
    Ok(())
}

fn verify_destination_preservation(
    evidence: &DestinationPreservationEvidenceV1,
    before: &[u8],
    after: &[u8],
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if !evidence.failure_observed || evidence.failure_message.trim().is_empty() {
        return invalid("destination-preservation case must record an observed failure");
    }
    if evidence.temporary_files_remaining != 0 {
        return invalid("destination-preservation case left temporary files behind");
    }
    verify_digest("destination before sha256", &evidence.before_sha256)?;
    verify_digest("destination after sha256", &evidence.after_sha256)?;
    let before_hash = sha256_bytes(before);
    let after_hash = sha256_bytes(after);
    compare_hash(
        "destination before failure",
        &evidence.before_sha256,
        &before_hash,
    )?;
    compare_hash(
        "destination after failure",
        &evidence.after_sha256,
        &after_hash,
    )?;
    if before_hash != after_hash {
        return invalid("failed export changed the existing destination");
    }
    Ok(())
}

fn verify_digest(field: &str, value: &str) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return invalid(format!("{field} must be a lowercase SHA-256 digest"));
    }
    Ok(())
}

fn compare_hash(
    label: &str,
    expected: &str,
    observed: &str,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    if expected != observed {
        return invalid(format!("{label} hash does not match the observed artifact"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, DevelopmentNativeOutputEvidenceError> {
    Err(DevelopmentNativeOutputEvidenceError::Invalid(
        message.into(),
    ))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, DevelopmentNativeOutputEvidenceError> {
    let bytes = read_regular_file(path)?;
    Ok(sha256_bytes(&bytes))
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, DevelopmentNativeOutputEvidenceError> {
    read_regular_file_with_identity(path).map(|(bytes, _)| bytes)
}

fn read_regular_file_with_identity(
    path: &Path,
) -> Result<(Vec<u8>, ArtifactIdentity), DevelopmentNativeOutputEvidenceError> {
    let before = fs::symlink_metadata(path).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return invalid(format!(
            "evidence artifact is not a regular, non-symlink file: {}",
            path.display()
        ));
    }
    let mut file =
        File::open(path).map_err(|source| DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        })?;
    let before_identity = ArtifactIdentity::from_metadata(&before);
    let opened_before =
        file.metadata()
            .map_err(|source| DevelopmentNativeOutputEvidenceError::ArtifactIo {
                path: path.to_path_buf(),
                source,
            })?;
    let path_after_open = fs::symlink_metadata(path).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if path_after_open.file_type().is_symlink() || !path_after_open.is_file() {
        return invalid(format!(
            "evidence artifact changed type while it was opened: {}",
            path.display()
        ));
    }
    let opened_before = ArtifactIdentity::from_metadata(&opened_before);
    let path_after_open = ArtifactIdentity::from_metadata(&path_after_open);
    if before_identity != opened_before || opened_before != path_after_open {
        return invalid(format!(
            "evidence artifact changed identity while it was opened: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let opened_after =
        file.metadata()
            .map_err(|source| DevelopmentNativeOutputEvidenceError::ArtifactIo {
                path: path.to_path_buf(),
                source,
            })?;
    let path_after_read = fs::symlink_metadata(path).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if path_after_read.file_type().is_symlink() || !path_after_read.is_file() {
        return invalid(format!(
            "evidence artifact changed type while it was read: {}",
            path.display()
        ));
    }
    let opened_after = ArtifactIdentity::from_metadata(&opened_after);
    let path_after_read = ArtifactIdentity::from_metadata(&path_after_read);
    if opened_before != opened_after || opened_after != path_after_read {
        return invalid(format!(
            "evidence artifact changed identity while it was read: {}",
            path.display()
        ));
    }
    if opened_after.len != bytes.len() as u64 {
        return invalid(format!(
            "evidence artifact length changed while it was read: {}",
            path.display()
        ));
    }
    Ok((bytes, opened_after))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactIdentity {
    len: u64,
    modified_nanos: Option<u128>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl ArtifactIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                len: metadata.len(),
                modified_nanos,
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        Self {
            len: metadata.len(),
            modified_nanos,
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update(self.len.to_be_bytes());
        match self.modified_nanos {
            Some(value) => {
                hasher.update([1]);
                hasher.update(value.to_be_bytes());
            }
            None => hasher.update([0]),
        }
        #[cfg(unix)]
        {
            hasher.update(self.device.to_be_bytes());
            hasher.update(self.inode.to_be_bytes());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectorySnapshot {
    files: Vec<(String, String)>,
    identities: Vec<(u8, String, ArtifactIdentity)>,
}

impl DirectorySnapshot {
    fn tree_hash(
        &self,
        relative_directory: Option<&str>,
    ) -> Result<String, DevelopmentNativeOutputEvidenceError> {
        let prefix = relative_directory.map(|value| format!("{value}/"));
        if let Some(directory) = relative_directory {
            if !self
                .identities
                .iter()
                .any(|(kind, path, _)| *kind == b'd' && path == directory)
            {
                return invalid(format!(
                    "required evidence directory is missing: {directory}"
                ));
            }
        }

        let mut included = 0usize;
        let mut hasher = Sha256::new();
        for (relative, digest) in &self.files {
            let relative = match &prefix {
                Some(prefix) => match relative.strip_prefix(prefix) {
                    Some(relative) if !relative.is_empty() => relative,
                    _ => continue,
                },
                None => relative.as_str(),
            };
            included = included.saturating_add(1);
            hasher.update(relative.as_bytes());
            hasher.update(b"\0file\0");
            hasher.update(digest.as_bytes());
            hasher.update(b"\n");
        }
        if included == 0 {
            return invalid("evidence artifact directory contains no files");
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn stability_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for (kind, relative, identity) in &self.identities {
            hasher.update([*kind]);
            hasher.update((relative.len() as u64).to_be_bytes());
            hasher.update(relative.as_bytes());
            identity.hash_into(&mut hasher);
        }
        for (relative, digest) in &self.files {
            hasher.update(b"content\0");
            hasher.update((relative.len() as u64).to_be_bytes());
            hasher.update(relative.as_bytes());
            hasher.update(digest.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

fn hash_macos_package_and_renderer(
    package: EvidenceArtifactSource<'_>,
) -> Result<(String, String), DevelopmentNativeOutputEvidenceError> {
    let EvidenceArtifactSource::Directory(package_root) = package else {
        return invalid("macOS package evidence must be a directory");
    };
    let snapshot = stable_directory_snapshot(package_root)?;
    let package_hash = snapshot.tree_hash(None)?;
    let renderer_hash = snapshot.tree_hash(Some(MACOS_RENDERER_BUNDLE_RELATIVE_PATH))?;
    Ok((package_hash, renderer_hash))
}

/// Hash a file directly or a directory as a sorted path/type/content manifest.
///
/// The directory algorithm matches the offline renderer verifier: each sorted
/// file contributes `relative-path NUL file NUL sha256 LF`. Symlinks and
/// non-UTF-8 paths fail closed.
pub fn hash_evidence_artifact(
    source: EvidenceArtifactSource<'_>,
) -> Result<String, DevelopmentNativeOutputEvidenceError> {
    match source {
        EvidenceArtifactSource::File(path) => sha256_file(path),
        EvidenceArtifactSource::Directory(root) => stable_directory_snapshot(root)?.tree_hash(None),
    }
}

fn stable_directory_snapshot(
    root: &Path,
) -> Result<DirectorySnapshot, DevelopmentNativeOutputEvidenceError> {
    stable_directory_snapshot_with_between(root, || {})
}

fn stable_directory_snapshot_with_between(
    root: &Path,
    between: impl FnOnce(),
) -> Result<DirectorySnapshot, DevelopmentNativeOutputEvidenceError> {
    let first = capture_directory_snapshot(root)?;
    between();
    let second = capture_directory_snapshot(root)?;
    if first.stability_hash() != second.stability_hash() || first != second {
        return invalid(format!(
            "evidence artifact directory changed while it was being hashed: {}",
            root.display()
        ));
    }
    Ok(second)
}

fn capture_directory_snapshot(
    root: &Path,
) -> Result<DirectorySnapshot, DevelopmentNativeOutputEvidenceError> {
    let mut snapshot = DirectorySnapshot {
        files: Vec::new(),
        identities: Vec::new(),
    };
    collect_directory_snapshot(root, root, &mut snapshot)?;
    snapshot.files.sort_by(|left, right| left.0.cmp(&right.0));
    snapshot
        .identities
        .sort_by(|left, right| (left.1.as_str(), left.0).cmp(&(right.1.as_str(), right.0)));
    Ok(snapshot)
}

fn collect_directory_snapshot(
    root: &Path,
    current: &Path,
    snapshot: &mut DirectorySnapshot,
) -> Result<(), DevelopmentNativeOutputEvidenceError> {
    let metadata = fs::symlink_metadata(current).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: current.to_path_buf(),
            source,
        }
    })?;
    if metadata.file_type().is_symlink() {
        return invalid(format!(
            "evidence artifact contains a symlink: {}",
            current.display()
        ));
    }
    let relative = current.strip_prefix(root).map_err(|_| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "evidence artifact escaped its root: {}",
            current.display()
        ))
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        DevelopmentNativeOutputEvidenceError::Invalid(format!(
            "evidence artifact path is not UTF-8: {}",
            relative.display()
        ))
    })?;
    let relative = relative.replace(std::path::MAIN_SEPARATOR, "/");
    if metadata.is_file() {
        let (bytes, identity) = read_regular_file_with_identity(current)?;
        snapshot.identities.push((b'f', relative.clone(), identity));
        snapshot.files.push((relative, sha256_bytes(&bytes)));
        return Ok(());
    }
    if !metadata.is_dir() {
        return invalid(format!(
            "unsupported evidence artifact type: {}",
            current.display()
        ));
    }
    snapshot
        .identities
        .push((b'd', relative, ArtifactIdentity::from_metadata(&metadata)));
    let entries = fs::read_dir(current).map_err(|source| {
        DevelopmentNativeOutputEvidenceError::ArtifactIo {
            path: current.to_path_buf(),
            source,
        }
    })?;
    let mut entries = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|source| {
                DevelopmentNativeOutputEvidenceError::ArtifactIo {
                    path: current.to_path_buf(),
                    source,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for entry in entries {
        collect_directory_snapshot(root, &entry, snapshot)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lopdf::{dictionary, Document, Object, Stream};

    use super::*;
    use crate::html_forms::{render_form_provider, render_layout_plan, RenderFixtureKind};
    use crate::html_output::{
        create_pdf_export_temp, finalize_pdf_export, merge_single_page_pdfs,
        normalize_wkpdf_page_from_css_pixels,
    };

    struct Fixture {
        _directory: tempfile::TempDir,
        package: PathBuf,
        renderer: PathBuf,
        expected_renderer_bundle_sha256: String,
        envelope_json: Vec<u8>,
        page_payloads: Vec<Vec<u8>>,
        output_pdf: PathBuf,
        destination_snapshot: Vec<u8>,
        expectation: PdfExpectation,
        layout_plan: RenderLayoutPlan,
        transcript: DevelopmentNativeOutputTranscriptV1,
    }

    impl Fixture {
        fn artifacts(&self) -> DevelopmentNativeOutputArtifacts<'_> {
            DevelopmentNativeOutputArtifacts {
                source_revision: &self.transcript.source_revision,
                package: EvidenceArtifactSource::Directory(&self.package),
                expected_renderer_bundle_sha256: &self.expected_renderer_bundle_sha256,
                envelope_json: &self.envelope_json,
                wkpdf_page_payloads: self.page_payloads.iter().map(Vec::as_slice).collect(),
                output_pdf: &self.output_pdf,
                destination_before_failure: &self.destination_snapshot,
                destination_after_failure: &self.destination_snapshot,
            }
        }
    }

    fn make_fixture() -> Fixture {
        let directory = tempfile::tempdir().expect("temp directory");
        let package = directory.path().join("eBIRForms.app");
        let renderer = package.join(MACOS_RENDERER_BUNDLE_RELATIVE_PATH);
        fs::create_dir_all(package.join("Contents/MacOS")).expect("package directory");
        fs::create_dir_all(&renderer).expect("renderer directory");
        fs::write(package.join("Contents/MacOS/bir"), b"native-binary").expect("package file");
        fs::write(renderer.join("index.html"), b"<main>renderer</main>").expect("renderer file");

        let provider = render_form_provider("2551Q", "2018").expect("2551Q provider");
        let envelope = (provider.fixtures)()
            .expect("fixtures")
            .into_iter()
            .find(|fixture| fixture.kind == RenderFixtureKind::Normal)
            .expect("normal fixture")
            .envelope;
        let envelope_json = serde_json::to_vec(&envelope).expect("envelope JSON");
        let envelope_sha256 = sha256_bytes(&envelope_json);
        let layout_plan = render_layout_plan(&envelope).expect("layout plan");
        let expectation = PdfExpectation {
            form_code: envelope.form.code.clone(),
            revision: envelope.form.version.clone(),
            envelope_hash: envelope_sha256.clone(),
            expected_page_count: layout_plan.expected_page_count,
            width_points: layout_plan.page_geometry.width_points,
            height_points: layout_plan.page_geometry.height_points,
        };

        let page_payloads = (0..layout_plan.expected_page_count)
            .map(|_| one_page_pdf(816, 1248, b"q 1 0 0 1 0 0 cm Q"))
            .collect::<Vec<_>>();
        let normalized_page_payloads = page_payloads
            .iter()
            .map(|page| {
                normalize_wkpdf_page_from_css_pixels(page, &expectation)
                    .expect("normalize synthetic WKPDF capture")
            })
            .collect::<Vec<_>>();
        let output_pdf = directory.path().join("selected.pdf");
        let raw_temp = create_pdf_export_temp(&output_pdf).expect("export temp");
        let merged =
            merge_single_page_pdfs(&normalized_page_payloads).expect("merge WKPDF captures");
        fs::write(&raw_temp, merged).expect("write merged platform PDF");
        let validation =
            finalize_pdf_export(&raw_temp, &output_pdf, &expectation).expect("finalized PDF");

        let failed_destination = directory.path().join("existing.pdf");
        let destination_snapshot = b"preserved-existing-destination".to_vec();
        fs::write(&failed_destination, &destination_snapshot).expect("existing destination");
        let invalid_temp = create_pdf_export_temp(&failed_destination).expect("failure temp");
        fs::write(&invalid_temp, b"not a PDF").expect("invalid platform output");
        finalize_pdf_export(&invalid_temp, &failed_destination, &expectation)
            .expect_err("invalid platform output must fail");
        assert!(
            !invalid_temp.exists(),
            "failed export must remove its temp file"
        );
        assert_eq!(
            fs::read(&failed_destination).expect("preserved destination"),
            destination_snapshot
        );
        let geometry = geometry_report(layout_plan);
        let geometry = GeometryReportEvidenceV1::from(&geometry);
        let destination_hash = sha256_bytes(&destination_snapshot);
        let (package_sha256, renderer_bundle_sha256) =
            hash_macos_package_and_renderer(EvidenceArtifactSource::Directory(&package))
                .expect("package and renderer hashes");

        let transcript = DevelopmentNativeOutputTranscriptV1 {
            schema_version: DEVELOPMENT_NATIVE_OUTPUT_TRANSCRIPT_SCHEMA_VERSION,
            scope: DevelopmentEvidenceScope::DevelopmentDiagnostic,
            promotion_eligible: false,
            platform: NativeEvidencePlatform::Macos,
            architecture: "aarch64".to_string(),
            backend: NativePdfBackend::WkWebViewCreatePdf,
            form_code: expectation.form_code.clone(),
            form_revision: expectation.revision.clone(),
            source_revision: "1".repeat(40),
            package_sha256,
            renderer_bundle_sha256: renderer_bundle_sha256.clone(),
            envelope_sha256,
            output_pdf_sha256: sha256_file(&output_pdf).expect("output hash"),
            same_webview: SameWebViewEvidenceV1 {
                prepared_webview_id: "run-webview-1".to_string(),
                preflight_webview_id: "run-webview-1".to_string(),
                wkpdf_webview_id: "run-webview-1".to_string(),
            },
            geometry_reports: [geometry.clone(), geometry],
            clipping_totals: ClippingCountersV1::default(),
            nonce: NonceEvidenceV1 {
                issued_nonce: 7,
                preflight_consumptions: vec![7],
                wkpdf_completion_nonce: 7,
            },
            wkpdf_completion: WkPdfCompletionEvidenceV1 {
                callback_completed: true,
                pages: page_payloads
                    .iter()
                    .enumerate()
                    .map(|(index, bytes)| WkPdfPageEvidenceV1 {
                        page_number: index + 1,
                        succeeded: true,
                        byte_count: bytes.len(),
                        sha256: sha256_bytes(bytes),
                        error: None,
                    })
                    .collect(),
            },
            pdf_validation: PdfValidationEvidenceV1::from(&validation),
            destination_preservation: DestinationPreservationEvidenceV1 {
                failure_observed: true,
                failure_message: "invalid platform PDF was rejected".to_string(),
                before_sha256: destination_hash.clone(),
                after_sha256: destination_hash,
                temporary_files_remaining: 0,
            },
        };

        Fixture {
            _directory: directory,
            package,
            renderer,
            expected_renderer_bundle_sha256: renderer_bundle_sha256,
            envelope_json,
            page_payloads,
            output_pdf,
            destination_snapshot,
            expectation,
            layout_plan,
            transcript,
        }
    }

    fn geometry_report(layout_plan: RenderLayoutPlan) -> RendererGeometryReport {
        let width = layout_plan.page_geometry.width_points * 96.0 / 72.0;
        let height = layout_plan.page_geometry.height_points * 96.0 / 72.0;
        RendererGeometryReport {
            page_count: layout_plan.expected_page_count,
            page_width_pt: layout_plan.page_geometry.width_points,
            page_height_pt: layout_plan.page_geometry.height_points,
            pages: (0..layout_plan.expected_page_count)
                .map(|index| RendererPageRect {
                    x: 24.0,
                    y: index as f64 * (height + 12.0),
                    width,
                    height,
                    client_width: width,
                    client_height: height,
                    scroll_width: width,
                    scroll_height: height,
                    descendant_overflow_x: 0,
                    descendant_overflow_y: 0,
                    descendant_clipped_x: 0,
                    descendant_clipped_y: 0,
                })
                .collect(),
        }
    }

    fn one_page_pdf(width: i64, height: i64, content: &[u8]) -> Vec<u8> {
        let directory = tempfile::tempdir().expect("page PDF directory");
        let path = directory.path().join("page.pdf");
        write_pdf(&path, 1, width, height, content);
        fs::read(path).expect("page PDF bytes")
    }

    fn write_pdf(path: &Path, page_count: usize, width: i64, height: i64, content: &[u8]) {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let mut page_ids = Vec::new();
        for _ in 0..page_count {
            let content_id = document.add_object(Stream::new(dictionary! {}, content.to_vec()));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                "CropBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                "Resources" => dictionary! {},
                "Contents" => content_id,
            });
            page_ids.push(page_id);
        }
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
                "Count" => page_count as i64,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        document.save(path).expect("test PDF should save");
    }

    fn development_observation(fixture: &Fixture) -> DevelopmentNativeOutputObservationV1 {
        let geometry_reports = fixture.transcript.geometry_reports.clone();
        let geometry_page_rect_sha256 =
            geometry_page_rect_sha256(&geometry_reports[0]).expect("rectangle hashes");
        DevelopmentNativeOutputObservationV1 {
            schema_version: DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION,
            scope: DevelopmentEvidenceScope::DevelopmentDiagnostic,
            promotion_eligible: false,
            platform: DevelopmentNativeOutputPlatformV1::Macos,
            backend: DevelopmentNativeOutputBackendV1::WkWebViewCreatePdf,
            form_code: fixture.transcript.form_code.clone(),
            form_revision: fixture.transcript.form_revision.clone(),
            document_run_id: "runtime-document-1".to_string(),
            envelope_sha256: fixture.transcript.envelope_sha256.clone(),
            source_revision: DevelopmentEvidenceAvailability::unavailable(
                "source revision is not embedded",
            ),
            package_sha256: DevelopmentEvidenceAvailability::unavailable(
                "cargo-run process has no package root",
            ),
            renderer_bundle_sha256: DevelopmentEvidenceAvailability::observed(
                fixture.transcript.renderer_bundle_sha256.clone(),
            ),
            independently_expected_renderer_bundle_sha256:
                DevelopmentEvidenceAvailability::unavailable("offline evidence is not injected"),
            geometry_reports,
            geometry_page_rect_sha256,
            clipping_totals: ClippingCountersV1::default(),
            nonce: NativeNonceObservationV1 {
                issued_nonce: 7,
                preflight_consumptions: vec![7],
                backend_completion_nonce: DevelopmentEvidenceAvailability::observed(7),
            },
            render_epoch: 4,
            readiness_revision: 9,
            backend_completion: DevelopmentEvidenceAvailability::observed(
                NativeBackendCompletionObservationV1 {
                    nonce: 7,
                    document_run_id: "runtime-document-1".to_string(),
                    envelope_sha256: fixture.transcript.envelope_sha256.clone(),
                    render_epoch: 4,
                    succeeded: true,
                    error: None,
                },
            ),
            native_page_payloads: DevelopmentEvidenceAvailability::observed(
                fixture
                    .page_payloads
                    .iter()
                    .enumerate()
                    .map(|(index, payload)| NativePdfPagePayloadObservationV1 {
                        page_number: index + 1,
                        succeeded: true,
                        byte_count: payload.len(),
                        sha256: Some(sha256_bytes(payload)),
                        error: None,
                    })
                    .collect(),
            ),
            output_pdf_sha256: DevelopmentEvidenceAvailability::observed(
                fixture.transcript.output_pdf_sha256.clone(),
            ),
            pdf_validation: DevelopmentEvidenceAvailability::observed(
                fixture.transcript.pdf_validation.clone(),
            ),
            destination_outcome: DevelopmentDestinationOutcomeV1::ExportSucceeded {
                before: DevelopmentDestinationSnapshotV1::Absent,
                after: DevelopmentDestinationSnapshotV1::File {
                    sha256: fixture.transcript.output_pdf_sha256.clone(),
                },
                temporary_file_remaining: false,
                preservation_failure_case_exercised: false,
            },
            strict_verifier_gaps: vec![
                "collector is not attested".to_string(),
                "failed destination-preservation case was not exercised".to_string(),
            ],
        }
    }

    #[test]
    fn runtime_observation_serializes_real_values_and_explicit_gaps_without_promotion() {
        let fixture = make_fixture();
        let observation = development_observation(&fixture);
        let encoded = encode_development_native_output_observation(&observation)
            .expect("non-promotional observation");
        assert_eq!(encoded.last(), Some(&b'\n'));
        let decoded = decode_development_native_output_observation(&encoded)
            .expect("validated observation JSON");
        assert_eq!(decoded, observation);
        assert!(!decoded.promotion_eligible);
        assert!(matches!(
            decoded.source_revision,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ));
        assert!(
            serde_json::from_slice::<DevelopmentNativeOutputTranscriptV1>(&encoded).is_err(),
            "an incomplete runtime observation must not parse as the strict transcript"
        );
    }

    #[test]
    fn runtime_observation_decoder_rejects_promotion_claims() {
        let fixture = make_fixture();
        let observation = development_observation(&fixture);
        let mut encoded = serde_json::to_value(observation).expect("observation JSON value");
        encoded["promotion_eligible"] = serde_json::Value::Bool(true);

        let error = decode_development_native_output_observation(
            &serde_json::to_vec(&encoded).expect("mutated observation JSON"),
        )
        .expect_err("development observation must never claim promotion eligibility");

        assert!(error
            .to_string()
            .contains("runtime observations must never be promotion eligible"));
    }

    #[test]
    fn linux_webkitgtk_runtime_observation_remains_explicitly_nonpromotional() {
        let fixture = make_fixture();
        let mut observation = development_observation(&fixture);
        observation.platform = DevelopmentNativeOutputPlatformV1::Linux;
        observation.backend = DevelopmentNativeOutputBackendV1::WebKitGtkPrintOperationPdf;
        observation.native_page_payloads = DevelopmentEvidenceAvailability::unavailable(
            "WebKitGTK PrintOperation exposes the completed PDF file, not one callback payload per page",
        );
        observation.strict_verifier_gaps.push(
            "Linux X11 and Wayland packaged runtime attestation is outside this observation"
                .to_string(),
        );

        let encoded = encode_development_native_output_observation(&observation)
            .expect("Linux WebKitGTK diagnostic observation");
        let decoded = decode_development_native_output_observation(&encoded)
            .expect("validated Linux diagnostic observation");
        assert_eq!(decoded.platform, DevelopmentNativeOutputPlatformV1::Linux);
        assert_eq!(
            decoded.backend,
            DevelopmentNativeOutputBackendV1::WebKitGtkPrintOperationPdf
        );
        assert!(!decoded.promotion_eligible);
        assert!(matches!(
            decoded.native_page_payloads,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ));

        let json: serde_json::Value =
            serde_json::from_slice(&encoded).expect("Linux observation JSON");
        assert_eq!(json["platform"], "linux");
        assert_eq!(json["backend"], "web_kit_gtk_print_operation_pdf");
        assert_eq!(json["promotion_eligible"], false);
    }

    #[test]
    fn runtime_observation_rejects_cross_platform_native_backends() {
        let fixture = make_fixture();
        let mut observation = development_observation(&fixture);
        observation.platform = DevelopmentNativeOutputPlatformV1::Linux;
        observation.backend = DevelopmentNativeOutputBackendV1::WebView2PrintToPdf;
        assert!(observation.validate_non_promotional().is_err());

        observation.platform = DevelopmentNativeOutputPlatformV1::Windows;
        observation.backend = DevelopmentNativeOutputBackendV1::WebKitGtkPrintOperationPdf;
        assert!(observation.validate_non_promotional().is_err());
    }

    #[test]
    fn offline_renderer_build_identity_is_strictly_nonpromotional() {
        let identity = OfflineRendererBuildIdentityV1 {
            schema_version: OFFLINE_RENDERER_BUILD_IDENTITY_SCHEMA_VERSION,
            scope: OfflineRendererBuildIdentityScope::BuildTimeNonPromotionalIdentity,
            promotion_eligible: false,
            offline_verification_passed: true,
            renderer_bundle_relative_path: OFFLINE_RENDERER_BUILD_IDENTITY_RELATIVE_PATH
                .to_string(),
            renderer_bundle_sha256: "a".repeat(64),
            source_revision: DevelopmentEvidenceAvailability::observed("b".repeat(40)),
        };
        let encoded = serde_json::to_vec(&identity).expect("build identity JSON");
        assert_eq!(
            decode_offline_renderer_build_identity(&encoded).expect("valid build identity"),
            identity
        );

        let mut promoting = identity.clone();
        promoting.promotion_eligible = true;
        assert!(promoting.validate_non_promotional().is_err());

        let mut wrong_path = identity.clone();
        wrong_path.renderer_bundle_relative_path = "../different-renderer".to_string();
        assert!(wrong_path.validate_non_promotional().is_err());

        let mut dirty_source = identity;
        dirty_source.source_revision = DevelopmentEvidenceAvailability::unavailable(
            "curated renderer source was dirty during the local build",
        );
        assert!(dirty_source.validate_non_promotional().is_ok());
    }

    #[test]
    fn runtime_observation_rejects_duplicated_claims_and_missing_gap_reasons() {
        let fixture = make_fixture();
        let mut observation = development_observation(&fixture);
        observation.geometry_reports[1].pages[0].x += 1.0;
        assert!(observation.validate_non_promotional().is_err());

        let mut observation = development_observation(&fixture);
        observation.source_revision = DevelopmentEvidenceAvailability::unavailable(" ");
        assert!(observation.validate_non_promotional().is_err());

        let mut observation = development_observation(&fixture);
        observation.promotion_eligible = true;
        assert!(observation.validate_non_promotional().is_err());

        let mut observation = development_observation(&fixture);
        observation.form_code = " ".to_string();
        assert!(observation.validate_non_promotional().is_err());
    }

    #[test]
    fn internally_consistent_synthetic_transcript_remains_nonpromotional() {
        // The fixture's blank synthetic PDFs prove verifier mechanics only. They
        // are not evidence that the renderer bundle and envelope caused WKPDF
        // to emit these pages.
        let fixture = make_fixture();
        let verification = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect("development evidence should verify");

        assert_eq!(verification.pdf_validation.page_count, 2);
        assert_eq!(
            verification.output_pdf_sha256,
            fixture.transcript.output_pdf_sha256
        );
        assert!(!fixture.transcript.promotion_eligible);
    }

    #[test]
    fn rejects_promotion_claim_and_unstable_geometry() {
        let mut fixture = make_fixture();
        fixture.transcript.promotion_eligible = true;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("promotion claim must fail");
        assert!(error.to_string().contains("never be promotion eligible"));

        fixture.transcript.promotion_eligible = false;
        fixture.transcript.geometry_reports[1].pages[0].x += 1.0;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("unstable geometry must fail");
        assert!(error.to_string().contains("not identical"));
    }

    #[test]
    fn rejects_clipping_and_nonce_reuse() {
        let mut fixture = make_fixture();
        fixture.transcript.geometry_reports[0].pages[0].descendant_clipped_x = 1;
        fixture.transcript.geometry_reports[1].pages[0].descendant_clipped_x = 1;
        fixture.transcript.clipping_totals.descendant_clipped_x = 1;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("clipping must fail");
        assert!(error
            .to_string()
            .contains("geometry report 1 failed host validation"));

        let mut fixture = make_fixture();
        fixture.transcript.nonce.preflight_consumptions.push(7);
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("reused nonce must fail");
        assert!(error.to_string().contains("consumed exactly once"));
    }

    #[test]
    fn rejects_wkpdf_tampering_and_changed_destination() {
        let mut fixture = make_fixture();
        fixture.transcript.wkpdf_completion.pages[0].byte_count += 1;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("WKPDF byte mismatch must fail");
        assert!(error.to_string().contains("byte count is inconsistent"));

        let fixture = make_fixture();
        let mut changed = fixture.destination_snapshot.clone();
        changed.push(b'!');
        let payloads = fixture
            .page_payloads
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let artifacts = DevelopmentNativeOutputArtifacts {
            source_revision: &fixture.transcript.source_revision,
            package: EvidenceArtifactSource::Directory(&fixture.package),
            expected_renderer_bundle_sha256: &fixture.expected_renderer_bundle_sha256,
            envelope_json: &fixture.envelope_json,
            wkpdf_page_payloads: payloads,
            output_pdf: &fixture.output_pdf,
            destination_before_failure: &fixture.destination_snapshot,
            destination_after_failure: &changed,
        };
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &artifacts,
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("changed destination must fail");
        assert!(error.to_string().contains("destination after failure hash"));
    }

    #[test]
    fn rejects_capture_with_same_content_but_different_page_semantics() {
        let mut fixture = make_fixture();
        let mut capture = Document::load_mem(&fixture.page_payloads[0]).expect("capture PDF");
        let page_id = *capture.get_pages().values().next().expect("capture page");
        capture
            .get_dictionary_mut(page_id)
            .expect("capture page dictionary")
            .set("Rotate", 90);
        let mut bytes = Vec::new();
        capture.save_to(&mut bytes).expect("rotated capture bytes");
        fixture.page_payloads[0] = bytes;
        fixture.transcript.wkpdf_completion.pages[0].byte_count = fixture.page_payloads[0].len();
        fixture.transcript.wkpdf_completion.pages[0].sha256 =
            sha256_bytes(&fixture.page_payloads[0]);

        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("page semantics must remain bound to the WKPDF capture");
        assert!(error.to_string().contains("unexpected rotation"));
    }

    fn replace_first_capture(fixture: &mut Fixture, mutate: impl FnOnce(&mut Document)) {
        let mut capture = Document::load_mem(&fixture.page_payloads[0]).expect("capture PDF");
        mutate(&mut capture);
        let mut bytes = Vec::new();
        capture.save_to(&mut bytes).expect("modified capture bytes");
        fixture.page_payloads[0] = bytes;
        fixture.transcript.wkpdf_completion.pages[0].byte_count = fixture.page_payloads[0].len();
        fixture.transcript.wkpdf_completion.pages[0].sha256 =
            sha256_bytes(&fixture.page_payloads[0]);
    }

    #[test]
    fn rejects_raw_wkpdf_content_not_bound_to_final_normalized_page() {
        let mut fixture = make_fixture();
        replace_first_capture(&mut fixture, |capture| {
            let page_id = *capture.get_pages().values().next().expect("capture page");
            let content_id = capture
                .get_dictionary(page_id)
                .expect("capture page dictionary")
                .get(b"Contents")
                .expect("capture page content")
                .as_reference()
                .expect("capture content reference");
            capture
                .get_object_mut(content_id)
                .expect("capture content object")
                .as_stream_mut()
                .expect("capture content stream")
                .set_plain_content(b"q 1 0 0 1 42 0 cm Q".to_vec());
        });

        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("normalized raw callback content must remain bound to final output");
        assert!(error.to_string().contains("not bound to its WKPDF capture"));
    }

    #[test]
    fn rejects_catalog_visual_state_and_external_streams() {
        for key in [
            "AcroForm",
            "OCProperties",
            "OutputIntents",
            "OpenAction",
            "AA",
            "ViewerPreferences",
        ] {
            let mut fixture = make_fixture();
            replace_first_capture(&mut fixture, |capture| {
                capture
                    .catalog_mut()
                    .expect("capture catalog")
                    .set(key, dictionary! {});
            });
            let error = verify_development_native_output_transcript(
                &fixture.transcript,
                &fixture.artifacts(),
                &fixture.expectation,
                &fixture.layout_plan,
            )
            .expect_err("catalog-level visual state must fail closed");
            assert!(error.to_string().contains("unsupported catalog"));
        }

        let mut fixture = make_fixture();
        replace_first_capture(&mut fixture, |capture| {
            capture.add_object(Stream::new(
                dictionary! { "F" => Object::string_literal(b"external.bin") },
                Vec::new(),
            ));
        });
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("external stream data must fail closed");
        assert!(error.to_string().contains("external file data"));
    }

    #[test]
    fn rejects_wrong_render_contract_version() {
        let mut fixture = make_fixture();
        let mut envelope: RenderEnvelopeV1 =
            serde_json::from_slice(&fixture.envelope_json).expect("fixture envelope");
        envelope.schema_version = "future-contract".to_string();
        fixture.envelope_json = serde_json::to_vec(&envelope).expect("modified envelope");
        fixture.transcript.envelope_sha256 = sha256_bytes(&fixture.envelope_json);
        fixture.expectation.envelope_hash = fixture.transcript.envelope_sha256.clone();

        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("wrong render contract version must fail closed");
        assert!(error
            .to_string()
            .contains("render envelope schema_version must be"));
    }

    #[test]
    fn rejects_wrong_transcript_schema_and_unknown_v1_fields() {
        let mut fixture = make_fixture();
        fixture.transcript.schema_version = DEVELOPMENT_NATIVE_OUTPUT_TRANSCRIPT_SCHEMA_VERSION + 1;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("wrong transcript schema version must fail closed");
        assert!(error.to_string().contains("schema_version must be"));

        let fixture = make_fixture();
        let mut transcript_json =
            serde_json::to_value(&fixture.transcript).expect("serialized transcript");
        transcript_json
            .as_object_mut()
            .expect("transcript object")
            .insert("unexpected_v1_field".to_string(), true.into());
        let error = serde_json::from_value::<DevelopmentNativeOutputTranscriptV1>(transcript_json)
            .expect_err("unknown top-level v1 field must fail closed");
        assert!(error.to_string().contains("unknown field"));

        let fixture = make_fixture();
        let mut transcript_json =
            serde_json::to_value(&fixture.transcript).expect("serialized transcript");
        transcript_json["nonce"]
            .as_object_mut()
            .expect("nonce object")
            .insert("unexpected_nonce_field".to_string(), true.into());
        let error = serde_json::from_value::<DevelopmentNativeOutputTranscriptV1>(transcript_json)
            .expect_err("unknown nested v1 field must fail closed");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_non_json_envelope_and_wrong_renderer_identity() {
        let mut fixture = make_fixture();
        fixture.envelope_json = b"not JSON".to_vec();
        fixture.transcript.envelope_sha256 = sha256_bytes(&fixture.envelope_json);
        fixture.expectation.envelope_hash = fixture.transcript.envelope_sha256.clone();
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("arbitrary envelope bytes must fail");
        assert!(error.to_string().contains("evidence JSON"));

        let fixture = make_fixture();
        let wrong_expected = "0".repeat(64);
        let payloads = fixture
            .page_payloads
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let artifacts = DevelopmentNativeOutputArtifacts {
            source_revision: &fixture.transcript.source_revision,
            package: EvidenceArtifactSource::Directory(&fixture.package),
            expected_renderer_bundle_sha256: &wrong_expected,
            envelope_json: &fixture.envelope_json,
            wkpdf_page_payloads: payloads,
            output_pdf: &fixture.output_pdf,
            destination_before_failure: &fixture.destination_snapshot,
            destination_after_failure: &fixture.destination_snapshot,
        };
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &artifacts,
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("wrong independent renderer identity must fail");
        assert!(error
            .to_string()
            .contains("independently expected renderer bundle"));

        let fixture = make_fixture();
        let wrong_path = fixture.package.join("Contents/Resources/form-renderer");
        fs::rename(&fixture.renderer, &wrong_path).expect("move renderer to legacy path");
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("renderer at any noncanonical package path must fail");
        assert!(error
            .to_string()
            .contains("required evidence directory is missing"));
    }

    #[test]
    fn artifact_tree_hash_is_stable_and_detects_changes() {
        let directory = tempfile::tempdir().expect("artifact directory");
        fs::create_dir_all(directory.path().join("nested")).expect("nested directory");
        fs::write(directory.path().join("b"), b"two").expect("file b");
        fs::write(directory.path().join("nested/a"), b"one").expect("file a");
        let first = hash_evidence_artifact(EvidenceArtifactSource::Directory(directory.path()))
            .expect("first hash");
        let second = hash_evidence_artifact(EvidenceArtifactSource::Directory(directory.path()))
            .expect("second hash");
        assert_eq!(first, second);

        fs::write(directory.path().join("nested/a"), b"changed").expect("changed file");
        let changed = hash_evidence_artifact(EvidenceArtifactSource::Directory(directory.path()))
            .expect("changed hash");
        assert_ne!(first, changed);
    }

    #[test]
    fn renderer_subtree_hash_matches_independent_known_vector() {
        let directory = tempfile::tempdir().expect("package directory");
        let package = directory.path().join("eBIRForms.app");
        let renderer = package.join(MACOS_RENDERER_BUNDLE_RELATIVE_PATH);
        fs::create_dir_all(renderer.join("assets")).expect("renderer assets directory");
        fs::write(
            renderer.join("assets/app.js"),
            b"console.log(\"renderer\");",
        )
        .expect("renderer JavaScript");
        fs::write(renderer.join("index.html"), b"<main>renderer</main>").expect("renderer HTML");

        let (_, renderer_hash) =
            hash_macos_package_and_renderer(EvidenceArtifactSource::Directory(&package))
                .expect("renderer subtree hash");

        // Independently computed from the documented sorted
        // `relative-path NUL file NUL sha256 LF` algorithm, rather than from
        // this implementation's output.
        assert_eq!(
            renderer_hash,
            "bd382bbc2ff3f6a32ebf12d68b1bbd59bcfe9c934e952abda65a4f24a500a02c"
        );
    }

    #[test]
    fn rejects_directory_mutation_between_snapshot_passes() {
        let directory = tempfile::tempdir().expect("artifact directory");
        let file = directory.path().join("asset.js");
        fs::write(&file, b"before").expect("initial asset");
        let error = stable_directory_snapshot_with_between(directory.path(), || {
            fs::write(&file, b"after").expect("mutated asset");
        })
        .expect_err("directory mutation must fail closed");
        assert!(error
            .to_string()
            .contains("changed while it was being hashed"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_snapshot() {
        use std::os::unix::fs::symlink;

        let mut fixture = make_fixture();
        let link = fixture._directory.path().join("linked-output.pdf");
        symlink(&fixture.output_pdf, &link).expect("output symlink");
        fixture.output_pdf = link;
        let error = verify_development_native_output_transcript(
            &fixture.transcript,
            &fixture.artifacts(),
            &fixture.expectation,
            &fixture.layout_plan,
        )
        .expect_err("symlinked output must fail closed");
        assert!(error.to_string().contains("non-symlink file"));
    }
}
