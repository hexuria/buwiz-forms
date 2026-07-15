#![recursion_limit = "256"]

use bir_core::forms::{
    form_2551q::{
        Form2551QDraft, Item13Election, OverpaymentDisposition, Schedule1Row, TaxPeriodBasis,
    },
    ATC_TABLE_2551Q,
};
use bir_print::html::{RenderEnvelopeV1, RENDER_CONTRACT_VERSION};
use bir_print::render_2551q_print;
use schemars::schema_for;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const FORM_CODE: &str = "2551Q";
const FORM_REVISION: &str = "2018";
const FORM_ID: &str = "2551Qv2018";
const FORM_FIXTURE: &str = "packages/form-contracts/fixtures/2551q-6-rows.json";
const ATC_REFERENCE_ARTIFACT: &str = "src/generated/2551q-atc-reference.json";
const FIXTURE_ATC_CODES: [&str; 10] = [
    "PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160", "PT170",
];
const VISUAL_FIXTURE_SHA256: &str =
    "f3d49ddab5cdd7c1d889a7b2cbd519babf7556c186702f0232b9f18257f7a5b7";
#[cfg(test)]
const CONTINUATION_FIXTURE_SHA256: &str =
    "1d5c560fa7a87325e69a1092f283cf32d839b6954dc900ab7588b35a88aa0e4d";
const OFFICIAL_SOURCE: &str =
    "https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf";
const OFFICIAL_SOURCE_SHA256: &str =
    "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24";
const PAGE_WIDTH_PT: f64 = 612.0;
const PAGE_HEIGHT_PT: f64 = 936.0;
const PAGE_COUNT: usize = 2;
const REFERENCE_DPI: u32 = 144;
const REFERENCE_WIDTH_PX: u32 = 1_224;
const REFERENCE_HEIGHT_PX: u32 = 1_872;
const SOURCE_SVG_SHA256: [&str; PAGE_COUNT] = [
    "e62c392a3962ba4c2c31ffcb4b77a7798140473a2af99abf95173680536db599",
    "377ec4cee07cbff674686926aa0d402ec068b9a70fe3e8dbfc9802e90902f47a",
];
const REFERENCE_PNG_SHA256: [&str; PAGE_COUNT] = [
    "c78f0724e2f320f1b306408008e9085ed36397c4e1add66bf5e77c322a3485ea",
    "d6ab5afbf6b3f4cbac7c69a01df231eaf6dcf7fde587e78c02ee20e3f2508d1a",
];

#[derive(Debug, Serialize)]
struct AtcReferenceArtifact {
    schema_version: u8,
    form_code: &'static str,
    revision: &'static str,
    entries: Vec<AtcReferenceArtifactEntry>,
}

#[derive(Debug, Serialize)]
struct AtcReferenceArtifactEntry {
    code: &'static str,
    description: &'static str,
    rate: f64,
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    output_root: PathBuf,
    visual_references: bool,
    check_visual_references: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let options = options_from_args(&root, env::args_os().skip(1))?;
    if options.check_visual_references {
        verify_committed_visual_references(&root)?;
        println!(
            "verified {FORM_CODE}:{FORM_REVISION} visual references: {PAGE_COUNT} pages, \
             {PAGE_WIDTH_PT}x{PAGE_HEIGHT_PT}pt, {REFERENCE_WIDTH_PX}x{REFERENCE_HEIGHT_PX}px \
             at {REFERENCE_DPI} DPI"
        );
        return Ok(());
    }
    generate_contracts(&options.output_root)?;
    if options.visual_references {
        generate_visual_references(&root, &fixture_2551q(6))?;
    }
    Ok(())
}

fn options_from_args(
    root: &Path,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Options, Box<dyn std::error::Error>> {
    let mut output_root = root.join("packages/form-contracts");
    let mut output_dir_seen = false;
    let mut visual_references = false;
    let mut check_visual_references = false;
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        if argument == "--output-dir" {
            if output_dir_seen {
                return Err("--output-dir may be specified only once".into());
            }
            output_root = arguments
                .next()
                .map(PathBuf::from)
                .ok_or("--output-dir requires a path")?;
            output_dir_seen = true;
        } else if argument == "--visual-references" {
            if visual_references {
                return Err("--visual-references may be specified only once".into());
            }
            visual_references = true;
        } else if argument == "--check-visual-references" {
            if check_visual_references {
                return Err("--check-visual-references may be specified only once".into());
            }
            check_visual_references = true;
        } else {
            return Err(format!(
                "unknown argument {}; expected --output-dir <path>, --visual-references, or \
                 --check-visual-references",
                PathBuf::from(argument).display()
            )
            .into());
        }
    }

    if visual_references && output_dir_seen {
        return Err("--visual-references cannot be combined with a custom --output-dir".into());
    }
    if check_visual_references && (visual_references || output_dir_seen) {
        return Err("--check-visual-references cannot be combined with generation options".into());
    }

    Ok(Options {
        output_root,
        visual_references,
        check_visual_references,
    })
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("bir-print is not inside the workspace")?
        .to_path_buf())
}

fn generate_contracts(output_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let schema_dir = output_root.join("schema");
    let fixture_dir = output_root.join("fixtures");
    let generated_source_dir = output_root.join("src/generated");
    fs::create_dir_all(&schema_dir)?;
    fs::create_dir_all(&fixture_dir)?;
    fs::create_dir_all(&generated_source_dir)?;

    let mut schema = serde_json::to_value(schema_for!(RenderEnvelopeV1))?;
    schema["$id"] = json!("https://goldcoders.dev/schemas/render-envelope-v1.json");
    schema["title"] = json!("RenderEnvelopeV1");
    schema["description"] = json!(format!(
        "Canonical eBIRForms renderer contract version {RENDER_CONTRACT_VERSION}"
    ));
    write_json(&schema_dir.join("render-envelope-v1.schema.json"), &schema)?;
    write_serializable_json(
        &output_root.join(ATC_REFERENCE_ARTIFACT),
        &atc_reference_artifact(),
    )?;

    write_fixture(&fixture_dir, "2551q-6-rows.json", 6)?;
    write_fixture(&fixture_dir, "2551q-10-rows.json", 10)?;
    write_draft_fixture(&fixture_dir, "2551q-minimum.json", &minimum_fixture_2551q())?;
    write_draft_fixture(
        &fixture_dir,
        "2551q-fiscal-period.json",
        &fiscal_period_fixture_2551q(),
    )?;
    write_draft_fixture(
        &fixture_dir,
        "2551q-tax-relief.json",
        &tax_relief_fixture_2551q(),
    )?;
    write_draft_fixture(
        &fixture_dir,
        "2551q-item13-eight-percent.json",
        &eight_percent_fixture_2551q(),
    )?;
    write_draft_fixture(
        &fixture_dir,
        "2551q-overpayment-refund.json",
        &overpayment_fixture_2551q(OverpaymentDisposition::Refund),
    )?;
    write_draft_fixture(
        &fixture_dir,
        "2551q-overpayment-tcc.json",
        &overpayment_fixture_2551q(OverpaymentDisposition::TaxCreditCertificate),
    )?;
    Ok(())
}

fn write_fixture(
    fixture_dir: &Path,
    file_name: &str,
    row_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    write_draft_fixture(fixture_dir, file_name, &fixture_2551q(row_count))
}

fn write_draft_fixture(
    fixture_dir: &Path,
    file_name: &str,
    draft: &Form2551QDraft,
) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = RenderEnvelopeV1::from(draft);
    write_json(
        &fixture_dir.join(file_name),
        &serde_json::to_value(envelope)?,
    )
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, canonical_pretty_json(value)?)?;
    Ok(())
}

fn canonical_pretty_json(value: &serde_json::Value) -> Result<String, serde_json::Error> {
    // `serde_json/preserve_order` is enabled transitively by the desktop
    // workspace. Canonicalize object keys so contract bytes do not depend on
    // whether this generator is built alone or as part of the workspace.
    let mut canonical = value.clone();
    canonical.sort_all_objects();
    Ok(format!("{}\n", serde_json::to_string_pretty(&canonical)?))
}

fn write_serializable_json(
    path: &Path,
    value: &impl Serialize,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn atc_reference_artifact() -> AtcReferenceArtifact {
    AtcReferenceArtifact {
        schema_version: 1,
        form_code: FORM_CODE,
        revision: FORM_REVISION,
        entries: ATC_TABLE_2551Q
            .iter()
            .map(|entry| AtcReferenceArtifactEntry {
                code: entry.code,
                description: entry.description,
                rate: entry.rate,
            })
            .collect(),
    }
}

fn fixture_2551q(row_count: usize) -> Form2551QDraft {
    let mut draft: Form2551QDraft = serde_json::from_value(json!({
        "id": null,
        "tin": "12345678900000",
        "taxpayer_type": "Individual",
        "business_start_date": "2010-01-01",
        "taxable_year": 2026,
        "quarter": 1,
        "tax_period_basis": "calendar",
        "year_end_month": 12,
        "eopt_tier": null,
        "is_amended": true,
        "original_return_filed_and_paid_on_time": true,
        "number_of_attached_sheets": 0,
        "tax_relief": false,
        "tax_relief_specification": "",
        "item_13_election": "graduated",
        "annual_income_tax_election": "unrecorded",
        "rdo_code": "018",
        "taxpayer_name": "Renderer Fixture Corporation",
        "registered_address": "53 Santol Extension, New Cabalan, Olongapo City",
        "zip_code": "2200",
        "contact_number": "09123456789",
        "email": "renderer@example.com",
        "schedule_1": [],
        "total_tax_due": 0.0,
        "creditable_tax_withheld": 125.0,
        "tax_paid_previous": 50.0,
        "other_tax_credit": 25.0,
        "other_tax_credit_description": "Validated prior payment",
        "total_tax_credits": 200.0,
        "tax_payable": 0.0,
        "auto_compute_penalties": false,
        "surcharge": 10.0,
        "interest": 5.0,
        "compromise": 1000.0,
        "total_penalties": 1015.0,
        "total_amount_payable": 0.0,
        "overpayment_disposition": "none",
        "status": "Draft",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "submitted_at": null,
        "confirmed_at": null,
        "submission_filename": null,
        "receipt_id": null,
        "queued_submission_fingerprint": null,
        "submission_attempts": 0,
        "next_retry_at": null,
        "last_error": null,
        "carried_forward_from": null,
        "payment_receipt_path": null
    }))
    .expect("canonical fixture must deserialize");
    assert!(
        row_count <= FIXTURE_ATC_CODES.len(),
        "canonical fixture only defines {} real ATC rows",
        FIXTURE_ATC_CODES.len()
    );
    draft.schedule_1 = FIXTURE_ATC_CODES
        .iter()
        .take(row_count)
        .enumerate()
        .map(|(index, atc)| {
            let mut row = Schedule1Row::new(atc).expect("fixture ATC must exist in Rust registry");
            // Preserve the established 300, 600, ... tax-due progression and
            // aggregate fixture totals while exercising real official rates.
            let intended_tax_due = (index + 1) as f64 * 300.0;
            row.taxable_amount = ((intended_tax_due / row.tax_rate) * 100.0).round() / 100.0;
            row.recompute();
            assert_eq!(row.tax_due, intended_tax_due);
            row
        })
        .collect();
    draft.recompute(None);
    draft
}

/// A valid draft with only the required identity, period, and one zero-valued
/// Schedule 1 row populated. This is the renderer's blank/minimum state: Rust
/// still supplies required taxpayer data instead of asking layout code to
/// invent it.
fn minimum_fixture_2551q() -> Form2551QDraft {
    let mut draft = fixture_2551q(1);
    draft.is_amended = false;
    draft.original_return_filed_and_paid_on_time = false;
    draft.number_of_attached_sheets = 0;
    draft.tax_relief = false;
    draft.tax_relief_specification.clear();
    draft.item_13_election = Item13Election::Graduated;
    draft.schedule_1 = vec![Schedule1Row::default_pt010()];
    draft.creditable_tax_withheld = 0.0;
    draft.tax_paid_previous = 0.0;
    draft.other_tax_credit = 0.0;
    draft.other_tax_credit_description.clear();
    draft.surcharge = 0.0;
    draft.interest = 0.0;
    draft.compromise = 0.0;
    draft.overpayment_disposition = OverpaymentDisposition::None;
    draft.recompute(None);
    draft
}

fn fiscal_period_fixture_2551q() -> Form2551QDraft {
    let mut draft = minimum_fixture_2551q();
    draft.tax_period_basis = TaxPeriodBasis::Fiscal;
    draft.year_end_month = 6;
    draft.quarter = 3;
    draft.item_13_election = Item13Election::NotApplicable;
    draft
}

fn tax_relief_fixture_2551q() -> Form2551QDraft {
    let mut draft = minimum_fixture_2551q();
    draft.tax_relief = true;
    draft.tax_relief_specification = "Special Law 123".to_string();
    draft
}

fn eight_percent_fixture_2551q() -> Form2551QDraft {
    let mut draft = minimum_fixture_2551q();
    draft.item_13_election = Item13Election::EightPercent;
    draft
}

fn overpayment_fixture_2551q(disposition: OverpaymentDisposition) -> Form2551QDraft {
    let mut draft = minimum_fixture_2551q();
    draft.schedule_1[0].taxable_amount = 100_000.0;
    draft.creditable_tax_withheld = 5_000.0;
    draft.recompute(None);
    debug_assert!(draft.total_amount_payable < 0.0);
    draft.overpayment_disposition = disposition;
    draft
}

fn generate_visual_references(
    root: &Path,
    form: &Form2551QDraft,
) -> Result<(), Box<dyn std::error::Error>> {
    let render_output = tempfile::tempdir()?;
    let result = render_2551q_print(
        form,
        render_output.path().join("2551q"),
        Some(root.join("formtypes")),
    )?;
    if result.preview_png_paths.len() != PAGE_COUNT {
        return Err(format!(
            "{FORM_ID} generated {} preview pages, expected {PAGE_COUNT}",
            result.preview_png_paths.len()
        )
        .into());
    }

    let references_dir = root.join("packages/form-renderer/references");
    fs::create_dir_all(&references_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(".2551q-reference-staging-")
        .tempdir_in(&references_dir)?;

    let mut staged_references = Vec::with_capacity(PAGE_COUNT);
    for (index, generated_path) in result.preview_png_paths.iter().enumerate() {
        let page = index + 1;
        let staged_path = staging.path().join(format!("2551q-2018-page-{page}.png"));
        fs::copy(generated_path, &staged_path)?;
        validate_reference_png(&staged_path, page, false)?;
        staged_references.push(staged_path);
    }

    if let Err(error) = validate_reference_hashes(&staged_references) {
        let candidates = root.join(".scratch/visual-reference-drift/2551q-2018");
        fs::create_dir_all(&candidates)?;
        for (index, staged_path) in staged_references.iter().enumerate() {
            fs::copy(
                staged_path,
                candidates.join(format!("page-{}.png", index + 1)),
            )?;
        }
        return Err(format!(
            "{error}; regenerated candidates were preserved under {} for review",
            candidates.display()
        )
        .into());
    }

    let manifest = build_visual_reference_manifest(root, &staged_references)?;
    let manifest_bytes = canonical_pretty_json(&manifest)?;

    for (index, staged_path) in staged_references.iter().enumerate() {
        let page = index + 1;
        let destination = references_dir.join(format!("2551q-2018-page-{page}.png"));
        replace_file(staged_path, &destination)?;
    }
    replace_bytes(&manifest_bytes, &references_dir.join("manifest.json"))?;

    verify_committed_visual_references(root)?;
    Ok(())
}

fn validate_reference_hashes(paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    for (index, path) in paths.iter().enumerate() {
        validate_pinned_hash(path, REFERENCE_PNG_SHA256[index], "reference PNG")?;
    }
    Ok(())
}

fn build_visual_reference_manifest(
    root: &Path,
    reference_paths: &[PathBuf],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if reference_paths.len() != PAGE_COUNT {
        return Err(format!(
            "{FORM_ID} has {} reference pages, expected {PAGE_COUNT}",
            reference_paths.len()
        )
        .into());
    }

    let form_dir = root.join("formtypes").join(FORM_ID);
    let metadata_path = form_dir.join("metadata.json");
    let formtype_path = form_dir.join("formtype.json");
    let template_path = form_dir.join("template.typ");
    let fixture_path = root.join(FORM_FIXTURE);
    validate_metadata(&metadata_path)?;
    validate_fixture(&fixture_path)?;

    let mut pages = Vec::with_capacity(PAGE_COUNT);
    for (index, reference_path) in reference_paths.iter().enumerate() {
        let page = index + 1;
        let source_svg = form_dir.join("pages").join(format!("page{page}.svg"));
        validate_source_svg(&source_svg, page)?;
        validate_reference_png(reference_path, page, true)?;
        pages.push(json!({
            "page": page,
            "source_svg": repo_relative(root, &source_svg)?,
            "source_svg_sha256": sha256_file(&source_svg)?,
            "reference_png": format!("packages/form-renderer/references/2551q-2018-page-{page}.png"),
            "reference_png_sha256": sha256_file(reference_path)?,
            "reference_width_px": REFERENCE_WIDTH_PX,
            "reference_height_px": REFERENCE_HEIGHT_PX
        }));
    }

    Ok(json!({
        "schema_version": 1,
        "dpi": REFERENCE_DPI,
        "generator": "cargo run -p bir-print --bin generate_render_contract -- --visual-references",
        "calibration_only": true,
        "runtime_background_allowed": false,
        "forms": [{
            "code": FORM_CODE,
            "revision": FORM_REVISION,
            "form_id": FORM_ID,
            "fixture": FORM_FIXTURE,
            "fixture_sha256": sha256_file(&fixture_path)?,
            "official_source": OFFICIAL_SOURCE,
            "official_source_sha256": OFFICIAL_SOURCE_SHA256,
            "metadata": repo_relative(root, &metadata_path)?,
            "metadata_sha256": sha256_file(&metadata_path)?,
            "formtype": repo_relative(root, &formtype_path)?,
            "formtype_sha256": sha256_file(&formtype_path)?,
            "template": repo_relative(root, &template_path)?,
            "template_sha256": sha256_file(&template_path)?,
            "page_width_pt": PAGE_WIDTH_PT,
            "page_height_pt": PAGE_HEIGHT_PT,
            "page_count": PAGE_COUNT,
            "pages": pages
        }]
    }))
}

fn verify_committed_visual_references(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let references_dir = root.join("packages/form-renderer/references");
    let reference_paths = (1..=PAGE_COUNT)
        .map(|page| references_dir.join(format!("2551q-2018-page-{page}.png")))
        .collect::<Vec<_>>();
    let expected =
        canonical_pretty_json(&build_visual_reference_manifest(root, &reference_paths)?)?;
    let actual = fs::read_to_string(references_dir.join("manifest.json"))?;
    if actual != expected {
        return Err("2551Q visual reference manifest is stale or non-deterministic".into());
    }
    Ok(())
}

fn validate_metadata(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let expected = [
        ("form_id", json!(FORM_ID)),
        ("official_source", json!(OFFICIAL_SOURCE)),
        ("sha256", json!(OFFICIAL_SOURCE_SHA256)),
        ("page_width_pt", json!(PAGE_WIDTH_PT)),
        ("page_height_pt", json!(PAGE_HEIGHT_PT)),
        ("page_count", json!(PAGE_COUNT)),
    ];
    for (key, value) in expected {
        if metadata.get(key) != Some(&value) {
            return Err(format!(
                "{} has unexpected {key}; expected {value}, found {}",
                path.display(),
                metadata.get(key).unwrap_or(&serde_json::Value::Null)
            )
            .into());
        }
    }
    Ok(())
}

fn validate_fixture(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let expected = canonical_pretty_json(&serde_json::to_value(RenderEnvelopeV1::from(
        &fixture_2551q(6),
    ))?)?;
    let actual = fs::read_to_string(path)?;
    if actual != expected {
        return Err(format!(
            "{} does not match the canonical six-row Rust fixture",
            path.display()
        )
        .into());
    }
    validate_pinned_hash(path, VISUAL_FIXTURE_SHA256, "visual contract fixture")
}

fn validate_source_svg(path: &Path, page: usize) -> Result<(), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let root_element = contents
        .split_once('>')
        .map(|(header, _)| header)
        .ok_or_else(|| format!("{} has no SVG root element", path.display()))?;
    for expected in ["width=\"612\"", "height=\"936\"", "viewBox=\"0 0 612 936\""] {
        if !root_element.contains(expected) {
            return Err(format!(
                "{} does not declare the pinned 612 x 936 point geometry",
                path.display()
            )
            .into());
        }
    }
    validate_pinned_hash(path, SOURCE_SVG_SHA256[page - 1], "official SVG source")
}

fn validate_reference_png(
    path: &Path,
    page: usize,
    verify_pinned_hash: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let dimensions = png_dimensions(&bytes)
        .ok_or_else(|| format!("{} is not a valid PNG with an IHDR chunk", path.display()))?;
    if dimensions != (REFERENCE_WIDTH_PX, REFERENCE_HEIGHT_PX) {
        return Err(format!(
            "{} is {} x {} pixels, expected {} x {} at {REFERENCE_DPI} DPI",
            path.display(),
            dimensions.0,
            dimensions.1,
            REFERENCE_WIDTH_PX,
            REFERENCE_HEIGHT_PX
        )
        .into());
    }
    if verify_pinned_hash {
        validate_pinned_hash(path, REFERENCE_PNG_SHA256[page - 1], "reference PNG")?;
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE_AND_IHDR: &[u8; 16] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    if bytes.len() < 24 || &bytes[..16] != PNG_SIGNATURE_AND_IHDR {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn validate_pinned_hash(
    path: &Path,
    expected: &str,
    role: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err(format!(
            "{} {role} SHA-256 drifted: expected {expected}, found {actual}",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let digest = Sha256::digest(fs::read(path)?);
    Ok(format!("{digest:x}"))
}

fn repo_relative(root: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    let temporary = destination.with_extension("png.tmp");
    fs::copy(source, &temporary)?;
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temporary, destination)
}

fn replace_bytes(contents: &str, destination: &Path) -> Result<(), std::io::Error> {
    let temporary = destination.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temporary, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::forms::find_atc;
    use bir_print::html::RenderValue;
    use std::collections::BTreeMap;

    const FIXTURE_NAMES: [&str; 8] = [
        "2551q-6-rows.json",
        "2551q-10-rows.json",
        "2551q-minimum.json",
        "2551q-fiscal-period.json",
        "2551q-tax-relief.json",
        "2551q-item13-eight-percent.json",
        "2551q-overpayment-refund.json",
        "2551q-overpayment-tcc.json",
    ];

    const VALID_FIXTURE_NAMES: [&str; 7] = [
        "2551q-6-rows.json",
        "2551q-minimum.json",
        "2551q-fiscal-period.json",
        "2551q-tax-relief.json",
        "2551q-item13-eight-percent.json",
        "2551q-overpayment-refund.json",
        "2551q-overpayment-tcc.json",
    ];

    #[test]
    fn generates_deterministic_schema_and_fixture_matrix() {
        let output = tempfile::tempdir().expect("temporary output should be available");
        generate_contracts(output.path()).expect("contracts should generate");

        let schema = output.path().join("schema/render-envelope-v1.schema.json");
        let first_atc_reference = fs::read(output.path().join(ATC_REFERENCE_ARTIFACT))
            .expect("generated ATC reference should exist");
        let first = FIXTURE_NAMES
            .iter()
            .map(|name| {
                (
                    *name,
                    fs::read(output.path().join("fixtures").join(name))
                        .expect("generated fixture should exist"),
                )
            })
            .collect::<BTreeMap<_, _>>();

        generate_contracts(output.path()).expect("contracts should regenerate");

        assert!(schema.is_file());
        assert_eq!(
            first_atc_reference,
            fs::read(output.path().join(ATC_REFERENCE_ARTIFACT))
                .expect("regenerated ATC reference should exist"),
            "2551Q ATC reference must be byte deterministic"
        );
        for name in FIXTURE_NAMES {
            let regenerated = fs::read(output.path().join("fixtures").join(name))
                .expect("regenerated fixture should exist");
            assert_eq!(
                first[name], regenerated,
                "{name} must be byte deterministic"
            );
        }

        assert_eq!(
            sha256_file(&output.path().join("fixtures/2551q-6-rows.json"))
                .expect("visual fixture hash should be readable"),
            VISUAL_FIXTURE_SHA256
        );
        assert_eq!(
            sha256_file(&output.path().join("fixtures/2551q-10-rows.json"))
                .expect("continuation fixture hash should be readable"),
            CONTINUATION_FIXTURE_SHA256
        );
    }

    #[test]
    fn committed_cross_language_atc_reference_exactly_matches_rust_registry() {
        let root = workspace_root().expect("workspace root should resolve");
        let artifact_path = root
            .join("packages/form-contracts")
            .join(ATC_REFERENCE_ARTIFACT);
        let actual: serde_json::Value = serde_json::from_slice(
            &fs::read(&artifact_path).expect("committed generated ATC reference should exist"),
        )
        .expect("committed generated ATC reference should be valid JSON");
        let expected = serde_json::to_value(atc_reference_artifact())
            .expect("Rust ATC registry should serialize");

        assert_eq!(
            actual,
            expected,
            "{} must match every Rust ATC code, description, rate, and its official order",
            artifact_path.display()
        );
    }

    #[test]
    fn fixture_matrix_covers_2551q_print_scenarios_without_unintended_errors() {
        let output = tempfile::tempdir().expect("temporary output should be available");
        generate_contracts(output.path()).expect("contracts should generate");

        for name in VALID_FIXTURE_NAMES {
            let envelope = read_fixture(output.path(), name);
            assert!(
                envelope.validation.is_empty(),
                "{name} should be valid, found {:?}",
                envelope.validation
            );
        }

        // The pinned visual fixture deliberately covers five requested cases so
        // separate JSON files would only duplicate the visual-reference input.
        let visual = read_fixture(output.path(), "2551q-6-rows.json");
        assert_eq!(visual.period.month, Some(12));
        assert_eq!(visual.period.quarter, Some(1));
        assert_eq!(
            visual.fields["tax_period_basis"],
            RenderValue::Text("calendar".to_string())
        );
        assert_eq!(visual.fields["is_amended"], RenderValue::Boolean(true));
        assert_eq!(
            visual.fields["item_13_election"],
            RenderValue::Text("graduated".to_string())
        );
        assert!(matches!(
            visual.fields["total_amount_payable"],
            RenderValue::Decimal(value) if value > 0.0
        ));
        assert_eq!(visual.schedules[0].rows.len(), 6);
        assert_eq!(
            visual.schedules[0].rows[0].cells["atc"],
            RenderValue::Text("PT010".to_string())
        );
        assert_eq!(
            visual.schedules[0].rows[5].cells["atc"],
            RenderValue::Text("PT090".to_string())
        );
        for row in &visual.schedules[0].rows {
            let RenderValue::Text(code) = &row.cells["atc"] else {
                panic!("fixture ATC must be text");
            };
            assert!(find_atc(code).is_some(), "fixture uses unknown ATC {code}");
        }
        assert!(!visual.fields.contains_key("schedule_1_page_2_subtotal"));

        let continuation = read_fixture(output.path(), "2551q-10-rows.json");
        assert_eq!(continuation.schedules[0].rows.len(), 10);
        assert_eq!(
            continuation.schedules[0].rows[9].cells["atc"],
            RenderValue::Text("PT170".to_string())
        );
        assert_eq!(
            continuation.fields["schedule_1_page_2_subtotal"],
            RenderValue::Decimal(6_300.0)
        );
        assert_eq!(
            continuation.fields["total_tax_due"],
            RenderValue::Decimal(16_500.0)
        );
        assert_eq!(continuation.validation.len(), 1);
        assert_eq!(continuation.validation[0].field_path, "schedule_1");
        assert!(continuation.validation[0].message.contains("at most six"));

        let minimum = read_fixture(output.path(), "2551q-minimum.json");
        assert_eq!(minimum.schedules[0].rows.len(), 1);
        assert_eq!(minimum.fields["is_amended"], RenderValue::Boolean(false));
        assert_eq!(
            minimum.fields["item_13_election"],
            RenderValue::Text("graduated".to_string())
        );
        assert!(matches!(
            minimum.fields["total_amount_payable"],
            RenderValue::Decimal(value) if value == 0.0
        ));

        let fiscal = read_fixture(output.path(), "2551q-fiscal-period.json");
        assert_eq!(fiscal.period.month, Some(6));
        assert_eq!(fiscal.period.quarter, Some(3));
        assert_eq!(
            fiscal.fields["tax_period_basis"],
            RenderValue::Text("fiscal".to_string())
        );
        assert_eq!(
            fiscal.fields["item_13_election"],
            RenderValue::Text("not_applicable".to_string())
        );

        let tax_relief = read_fixture(output.path(), "2551q-tax-relief.json");
        assert_eq!(tax_relief.fields["tax_relief"], RenderValue::Boolean(true));
        assert!(matches!(
            &tax_relief.fields["tax_relief_specification"],
            RenderValue::Text(value) if !value.trim().is_empty()
        ));

        let eight_percent = read_fixture(output.path(), "2551q-item13-eight-percent.json");
        assert_eq!(
            eight_percent.fields["item_13_election"],
            RenderValue::Text("eight_percent".to_string())
        );

        for (name, expected) in [
            ("2551q-overpayment-refund.json", "refund"),
            ("2551q-overpayment-tcc.json", "tax_credit_certificate"),
        ] {
            let overpayment = read_fixture(output.path(), name);
            assert!(matches!(
                overpayment.fields["total_amount_payable"],
                RenderValue::Decimal(value) if value < 0.0
            ));
            assert_eq!(
                overpayment.fields["overpayment_disposition"],
                RenderValue::Text(expected.to_string())
            );
        }
    }

    fn read_fixture(root: &Path, name: &str) -> RenderEnvelopeV1 {
        serde_json::from_slice(
            &fs::read(root.join("fixtures").join(name))
                .expect("canonical fixture should be readable"),
        )
        .expect("canonical fixture should match the schema model")
    }

    #[test]
    fn parses_visual_reference_and_output_options() {
        let root = Path::new("/workspace");
        let visual = options_from_args(root, [OsString::from("--visual-references")])
            .expect("visual reference option should parse");
        let output = options_from_args(
            root,
            [
                OsString::from("--output-dir"),
                OsString::from("/tmp/contracts"),
            ],
        )
        .expect("output option should parse");

        assert!(visual.visual_references);
        assert!(!visual.check_visual_references);
        assert_eq!(
            visual.output_root,
            Path::new("/workspace/packages/form-contracts")
        );
        assert!(!output.visual_references);
        assert_eq!(output.output_root, Path::new("/tmp/contracts"));
    }

    #[test]
    fn rejects_duplicate_or_unknown_options() {
        let root = Path::new("/workspace");
        assert!(options_from_args(
            root,
            [
                OsString::from("--visual-references"),
                OsString::from("--visual-references"),
            ]
        )
        .is_err());
        assert!(options_from_args(
            root,
            [
                OsString::from("--visual-references"),
                OsString::from("--output-dir"),
                OsString::from("/tmp/contracts"),
            ]
        )
        .is_err());
        assert!(options_from_args(
            root,
            [
                OsString::from("--visual-references"),
                OsString::from("--check-visual-references"),
            ]
        )
        .is_err());
        assert!(options_from_args(root, [OsString::from("--other")]).is_err());
    }

    #[test]
    fn reads_pinned_png_dimensions_from_ihdr() {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&REFERENCE_WIDTH_PX.to_be_bytes());
        png.extend_from_slice(&REFERENCE_HEIGHT_PX.to_be_bytes());
        assert_eq!(
            png_dimensions(&png),
            Some((REFERENCE_WIDTH_PX, REFERENCE_HEIGHT_PX))
        );
        assert_eq!(png_dimensions(b"not a png"), None);
    }

    #[test]
    fn committed_reference_manifest_is_reproducible_from_pinned_files() {
        let root = workspace_root().expect("workspace root should resolve");
        verify_committed_visual_references(&root)
            .expect("committed 2551Q references and manifest should match pinned evidence");
    }
}
