use bir_core::forms::{FormCapabilities, FormSupportLevel, FORM_CAPABILITY_REGISTRY};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct CapabilityManifest {
    schema_version: u8,
    forms: Vec<CapabilityRecord>,
}

#[derive(Debug, Serialize)]
struct CapabilityRecord {
    code: &'static str,
    revision: &'static str,
    form_id: &'static str,
    support_level: FormSupportLevel,
    capabilities: FormCapabilities,
    release_ready: bool,
}

fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: 1,
        forms: FORM_CAPABILITY_REGISTRY
            .iter()
            .copied()
            .map(|record| CapabilityRecord {
                code: record.code,
                revision: record.revision,
                form_id: record.form_id,
                support_level: record.support_level(),
                capabilities: record.capabilities,
                release_ready: record.release_ready,
            })
            .collect(),
    }
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/form-specs/generated/form-capabilities.json")
}

fn output_from_args() -> Result<PathBuf, String> {
    let mut arguments = env::args_os().skip(1);
    match arguments.next() {
        None => Ok(default_output()),
        Some(flag) if flag == "--output" => {
            let output = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "--output requires a path".to_string())?;
            if arguments.next().is_some() {
                return Err("unexpected arguments after --output <path>".to_string());
            }
            Ok(output)
        }
        Some(argument) => Err(format!(
            "unknown argument {}; expected --output <path>",
            argument.to_string_lossy()
        )),
    }
}

fn write_manifest(
    path: &Path,
    value: &CapabilityManifest,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("capability output has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    use std::io::Write as _;
    temporary.write_all(b"\n")?;
    temporary.persist(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        output_from_args().map_err(|message| -> Box<dyn std::error::Error> { message.into() })?;
    write_manifest(&output, &manifest())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_manifest_is_registry_complete_and_derived() {
        let generated = manifest();
        assert_eq!(generated.schema_version, 1);
        assert_eq!(generated.forms.len(), FORM_CAPABILITY_REGISTRY.len());
        for (source, output) in FORM_CAPABILITY_REGISTRY.iter().zip(generated.forms) {
            assert_eq!(output.code, source.code);
            assert_eq!(output.revision, source.revision);
            assert_eq!(output.form_id, source.form_id);
            assert_eq!(output.support_level, source.support_level());
            assert_eq!(output.capabilities, source.capabilities);
            assert_eq!(output.release_ready, source.release_ready);
        }
    }

    #[test]
    fn generated_manifest_writes_atomically_as_valid_json() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("nested/capabilities.json");
        write_manifest(&output, &manifest()).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert!(parsed["forms"].as_array().unwrap().len() >= 10);
    }
}
