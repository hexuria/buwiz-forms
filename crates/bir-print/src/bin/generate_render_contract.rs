#![recursion_limit = "256"]

use bir_print::html::{RenderEnvelopeV1, RENDER_CONTRACT_VERSION};
use bir_print::html_forms::{render_form_provider, render_form_providers, RenderFormProvider};
use schemars::schema_for;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
const CONTINUATION_FIXTURE_SHA256: &str =
    "1d5c560fa7a87325e69a1092f283cf32d839b6954dc900ab7588b35a88aa0e4d";

#[derive(Debug, Clone, PartialEq, Eq)]
enum FormSelection {
    All,
    One { code: String, revision: String },
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    output_root: PathBuf,
    selection: FormSelection,
    visual_references: bool,
    check_visual_references: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let options = options_from_args(&root, env::args_os().skip(1))?;
    let providers = selected_providers(&options.selection)?;
    if options.check_visual_references {
        let all_providers = render_form_providers().iter().collect::<Vec<_>>();
        verify_committed_visual_references(&root, &all_providers)?;
        return Ok(());
    }
    generate_contracts(&options.output_root, &providers)?;
    if options.visual_references {
        refresh_visual_reference_manifest(&root, render_form_providers())?;
    }
    Ok(())
}

fn options_from_args(
    root: &Path,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Options, Box<dyn std::error::Error>> {
    let mut output_root = root.join("packages/form-contracts");
    let mut output_dir_seen = false;
    let mut selection: Option<FormSelection> = None;
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
        } else if argument == "--all" {
            if selection.is_some() {
                return Err("--all cannot be combined with another form selection".into());
            }
            selection = Some(FormSelection::All);
        } else if argument == "--form" {
            if selection.is_some() {
                return Err("--form cannot be combined with another form selection".into());
            }
            let value = arguments.next().ok_or("--form requires CODE:REVISION")?;
            let value = value.to_string_lossy();
            let (code, revision) = value
                .split_once(':')
                .filter(|(code, revision)| !code.is_empty() && !revision.is_empty())
                .ok_or("--form requires CODE:REVISION")?;
            selection = Some(FormSelection::One {
                code: code.to_string(),
                revision: revision.to_string(),
            });
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
                "unknown argument {}; expected --all, --form CODE:REVISION, --output-dir <path>, \
                 --visual-references, or --check-visual-references",
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
        selection: selection.unwrap_or(FormSelection::All),
        visual_references,
        check_visual_references,
    })
}

fn selected_providers(
    selection: &FormSelection,
) -> Result<Vec<&'static RenderFormProvider>, Box<dyn std::error::Error>> {
    match selection {
        FormSelection::All => Ok(render_form_providers().iter().collect()),
        FormSelection::One { code, revision } => render_form_provider(code, revision)
            .map(|provider| vec![provider])
            .ok_or_else(|| {
                format!("no HTML render provider is registered for {code}:{revision}").into()
            }),
    }
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("bir-print is not inside the workspace")?
        .to_path_buf())
}

fn generate_contracts(
    output_root: &Path,
    providers: &[&RenderFormProvider],
) -> Result<(), Box<dyn std::error::Error>> {
    let schema_dir = output_root.join("schema");
    let fixture_dir = output_root.join("fixtures");
    fs::create_dir_all(&schema_dir)?;
    fs::create_dir_all(&fixture_dir)?;

    let mut schema = serde_json::to_value(schema_for!(RenderEnvelopeV1))?;
    schema["$id"] = json!("https://goldcoders.dev/schemas/render-envelope-v1.json");
    schema["title"] = json!("RenderEnvelopeV1");
    schema["description"] = json!(format!(
        "Canonical eBIRForms renderer contract version {RENDER_CONTRACT_VERSION}"
    ));
    write_json(&schema_dir.join("render-envelope-v1.schema.json"), &schema)?;
    for provider in providers {
        for fixture in (provider.fixtures)()? {
            write_json(
                &fixture_dir.join(fixture.file_name),
                &serde_json::to_value(fixture.envelope)?,
            )?;
        }
        for artifact in (provider.generated_artifacts)()? {
            let path = output_root.join(artifact.relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            write_json(&path, &artifact.value)?;
        }
    }
    Ok(())
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

fn build_visual_reference_manifest(
    root: &Path,
    providers: &[&RenderFormProvider],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let references_dir = root.join("packages/form-renderer/references");
    let reference_dpi = providers
        .first()
        .map_or(144, |provider| provider.reference_dpi);
    let mut forms = Vec::with_capacity(providers.len());
    for provider in providers {
        if provider.reference_dpi != reference_dpi {
            return Err(format!(
                "{} uses {} DPI; the shared reference manifest uses {reference_dpi} DPI",
                provider.key(),
                provider.reference_dpi
            )
            .into());
        }
        if provider.visual_reference_pages.len() != provider.expected_base_page_count {
            return Err(format!(
                "{} defines {} visual pages; expected {}",
                provider.key(),
                provider.visual_reference_pages.len(),
                provider.expected_base_page_count
            )
            .into());
        }
        let fixture_path = root
            .join("packages/form-contracts/fixtures")
            .join(provider.visual_fixture_file_name);
        validate_fixture(&fixture_path, provider)?;

        let mut pages = Vec::with_capacity(provider.visual_reference_pages.len());
        for page in provider.visual_reference_pages {
            let reference_path = references_dir.join(page.file_name);
            validate_reference_png(&reference_path, provider, page.page, page.sha256)?;
            pages.push(json!({
                "page": page.page,
                "reference_png": format!("packages/form-renderer/references/{}", page.file_name),
                "reference_png_sha256": sha256_file(&reference_path)?,
                "reference_width_px": provider.reference_width_px,
                "reference_height_px": provider.reference_height_px
            }));
        }

        forms.push(json!({
            "code": provider.code,
            "revision": provider.revision,
            "form_id": provider.form_id,
            "fixture": format!("packages/form-contracts/fixtures/{}", provider.visual_fixture_file_name),
            "fixture_sha256": sha256_file(&fixture_path)?,
            "official_source": provider.official_source,
            "official_source_sha256": provider.official_source_sha256,
            "page_width_pt": provider.page_width_pt,
            "page_height_pt": provider.page_height_pt,
            "page_count": provider.expected_base_page_count,
            "reference_provenance": {
                "kind": "official_pdf_raster",
                "runtime_eligible": false,
                "replacement_required": false,
                "note": "Rendered directly from the pinned official BIR PDF with Poppler at the manifest DPI; calibration-only and never runtime-loaded."
            },
            "runtime_discrete_assets": (provider.runtime_discrete_assets)(),
            "pages": pages
        }));
    }

    Ok(json!({
        "schema_version": 2,
        "dpi": reference_dpi,
        "generator": "cargo run -p bir-print --bin generate_render_contract -- --visual-references",
        "calibration_only": true,
        "runtime_background_allowed": false,
        "forms": forms
    }))
}

fn refresh_visual_reference_manifest(
    root: &Path,
    providers: &[RenderFormProvider],
) -> Result<(), Box<dyn std::error::Error>> {
    let providers = providers.iter().collect::<Vec<_>>();
    let manifest = canonical_pretty_json(&build_visual_reference_manifest(root, &providers)?)?;
    replace_bytes(
        &manifest,
        &root.join("packages/form-renderer/references/manifest.json"),
    )?;
    verify_committed_visual_references(root, &providers)
}

fn verify_committed_visual_references(
    root: &Path,
    providers: &[&RenderFormProvider],
) -> Result<(), Box<dyn std::error::Error>> {
    let references_dir = root.join("packages/form-renderer/references");
    let expected = canonical_pretty_json(&build_visual_reference_manifest(root, providers)?)?;
    let actual = fs::read_to_string(references_dir.join("manifest.json"))?;
    if actual != expected {
        return Err("visual reference manifest is stale or non-deterministic".into());
    }
    for provider in providers {
        println!(
            "verified {} visual references: {} pages, {}x{}pt, {}x{}px at {} DPI",
            provider.key(),
            provider.visual_reference_pages.len(),
            provider.page_width_pt,
            provider.page_height_pt,
            provider.reference_width_px,
            provider.reference_height_px,
            provider.reference_dpi,
        );
    }
    Ok(())
}

fn validate_fixture(
    path: &Path,
    provider: &RenderFormProvider,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = (provider.fixtures)()?
        .into_iter()
        .find(|fixture| fixture.file_name == provider.visual_fixture_file_name)
        .ok_or_else(|| {
            format!(
                "{} does not define visual fixture {}",
                provider.key(),
                provider.visual_fixture_file_name
            )
        })?;
    let expected = canonical_pretty_json(&serde_json::to_value(fixture.envelope)?)?;
    let actual = fs::read_to_string(path)?;
    if actual != expected {
        return Err(format!(
            "{} does not match the canonical {} Rust fixture",
            path.display(),
            provider.key()
        )
        .into());
    }
    validate_pinned_hash(
        path,
        provider.visual_fixture_sha256,
        "visual contract fixture",
    )
}

fn validate_reference_png(
    path: &Path,
    provider: &RenderFormProvider,
    page: usize,
    expected_hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let dimensions = png_dimensions(&bytes)
        .ok_or_else(|| format!("{} is not a valid PNG with an IHDR chunk", path.display()))?;
    if dimensions != (provider.reference_width_px, provider.reference_height_px) {
        return Err(format!(
            "{} is {} x {} pixels, expected {} x {} at {} DPI",
            path.display(),
            dimensions.0,
            dimensions.1,
            provider.reference_width_px,
            provider.reference_height_px,
            provider.reference_dpi,
        )
        .into());
    }
    validate_pinned_hash(
        path,
        expected_hash,
        &format!("{} page {page} reference PNG", provider.key()),
    )?;
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

    fn provider() -> &'static RenderFormProvider {
        render_form_provider("2551Q", "2018").expect("2551Q provider")
    }

    fn provider_1601c() -> &'static RenderFormProvider {
        render_form_provider("1601C", "2018").expect("1601C provider")
    }

    fn providers() -> Vec<&'static RenderFormProvider> {
        vec![provider()]
    }

    #[test]
    fn generates_deterministic_schema_and_fixture_matrix() {
        let output = tempfile::tempdir().expect("temporary output should be available");
        generate_contracts(output.path(), &providers()).expect("contracts should generate");

        let schema = output.path().join("schema/render-envelope-v1.schema.json");
        let artifacts = (provider().generated_artifacts)().expect("provider artifacts");
        let atc_artifact = &artifacts[0];
        let first_atc_reference = fs::read(output.path().join(atc_artifact.relative_path))
            .expect("generated ATC reference should exist");
        let fixtures = (provider().fixtures)().expect("provider fixtures");
        let first = fixtures
            .iter()
            .map(|fixture| {
                (
                    fixture.file_name,
                    fs::read(output.path().join("fixtures").join(fixture.file_name))
                        .expect("generated fixture should exist"),
                )
            })
            .collect::<BTreeMap<_, _>>();

        generate_contracts(output.path(), &providers()).expect("contracts should regenerate");

        assert!(schema.is_file());
        assert_eq!(
            first_atc_reference,
            fs::read(output.path().join(atc_artifact.relative_path))
                .expect("regenerated ATC reference should exist"),
            "2551Q ATC reference must be byte deterministic"
        );
        for fixture in fixtures {
            let name = fixture.file_name;
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
            provider().visual_fixture_sha256
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
        let expected_artifact = (provider().generated_artifacts)()
            .expect("provider artifacts")
            .remove(0);
        let artifact_path = root
            .join("packages/form-contracts")
            .join(expected_artifact.relative_path);
        let actual: serde_json::Value = serde_json::from_slice(
            &fs::read(&artifact_path).expect("committed generated ATC reference should exist"),
        )
        .expect("committed generated ATC reference should be valid JSON");
        let expected = expected_artifact.value;

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
        generate_contracts(output.path(), &providers()).expect("contracts should generate");

        for fixture in (provider().fixtures)().expect("provider fixtures") {
            let envelope = read_fixture(output.path(), fixture.file_name);
            assert_eq!(
                envelope.validation.is_empty(),
                fixture.expected_form_valid,
                "{} validity expectation disagrees with Rust validation: {:?}",
                fixture.file_name,
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
        assert_eq!(visual.selection, FormSelection::All);
        assert_eq!(
            visual.output_root,
            Path::new("/workspace/packages/form-contracts")
        );
        assert!(!output.visual_references);
        assert_eq!(output.output_root, Path::new("/tmp/contracts"));

        let one = options_from_args(
            root,
            [OsString::from("--form"), OsString::from("2551Q:2018")],
        )
        .expect("single-form option should parse");
        assert_eq!(
            one.selection,
            FormSelection::One {
                code: "2551Q".to_string(),
                revision: "2018".to_string()
            }
        );
        assert_eq!(selected_providers(&one.selection).unwrap().len(), 1);

        let one_1601c = options_from_args(
            root,
            [OsString::from("--form"), OsString::from("1601C:2018")],
        )
        .expect("1601C single-form option should parse");
        let selected_1601c = selected_providers(&one_1601c.selection).unwrap();
        assert_eq!(selected_1601c.len(), 1);
        assert_eq!(selected_1601c[0].key(), provider_1601c().key());
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
        assert!(options_from_args(
            root,
            [
                OsString::from("--all"),
                OsString::from("--form"),
                OsString::from("2551Q:2018")
            ]
        )
        .is_err());
        assert!(selected_providers(&FormSelection::One {
            code: "9999".to_string(),
            revision: "2018".to_string()
        })
        .is_err());
    }

    #[test]
    fn reads_pinned_png_dimensions_from_ihdr() {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&provider().reference_width_px.to_be_bytes());
        png.extend_from_slice(&provider().reference_height_px.to_be_bytes());
        assert_eq!(
            png_dimensions(&png),
            Some((
                provider().reference_width_px,
                provider().reference_height_px
            ))
        );
        assert_eq!(png_dimensions(b"not a png"), None);
    }

    #[test]
    fn committed_reference_manifest_is_reproducible_from_pinned_files() {
        let root = workspace_root().expect("workspace root should resolve");
        let all_providers = render_form_providers().iter().collect::<Vec<_>>();
        verify_committed_visual_references(&root, &all_providers)
            .expect("committed HTML form references and manifest should match pinned evidence");
        let manifest =
            fs::read_to_string(root.join("packages/form-renderer/references/manifest.json"))
                .expect("reference manifest");
        for forbidden in ["formtypes/", ".typ\"", "source_svg"] {
            assert!(
                !manifest.contains(forbidden),
                "manifest contains {forbidden}"
            );
        }
    }
}
