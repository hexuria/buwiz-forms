//! Fail-closed construction of external evidence-vault capture metadata.
//!
//! Every capture fact is supplied by the caller. This module deliberately does
//! not read a clock, user name, host name, environment variable, or repository
//! state to invent provenance.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use same_file::Handle;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{CodegenError, Result};
use crate::evidence::EvidenceCaptureProvenance;
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::path::{canonical_repo_root, is_same_or_below, is_symlink_or_reparse_point};
use crate::vault_acquisition::{
    EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT, EvidenceVaultCaptureMetadata, validate_capture_metadata,
};
#[cfg(windows)]
use crate::verified_file::stable_windows_link_count;

/// Complete caller-supplied inputs for a no-write plan or fresh external emit.
#[derive(Clone, Debug)]
pub struct WriteEvidenceVaultCaptureMetadataOptions {
    pub repo_root: PathBuf,
    pub output_path: PathBuf,
    pub capture_session_id: String,
    pub source_map_sha256: String,
    pub source_verification_sha256: String,
    pub capture_provenance: EvidenceCaptureProvenance,
    pub dry_run: bool,
}

impl WriteEvidenceVaultCaptureMetadataOptions {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
        capture_session_id: impl Into<String>,
        source_map_sha256: impl Into<String>,
        source_verification_sha256: impl Into<String>,
        capture_provenance: EvidenceCaptureProvenance,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            output_path: output_path.into(),
            capture_session_id: capture_session_id.into(),
            source_map_sha256: source_map_sha256.into(),
            source_verification_sha256: source_verification_sha256.into(),
            capture_provenance,
            dry_run: true,
        }
    }
}

/// Exact canonical artifact and publication result.
#[derive(Clone, Debug, Serialize)]
pub struct WriteEvidenceVaultCaptureMetadataReport {
    pub capture_metadata: EvidenceVaultCaptureMetadata,
    pub capture_metadata_bytes: Vec<u8>,
    pub capture_metadata_sha256: String,
    pub output_path: PathBuf,
    pub written: bool,
}

/// Validate caller-supplied facts and optionally publish a fresh external file.
pub fn write_evidence_vault_capture_metadata(
    options: &WriteEvidenceVaultCaptureMetadataOptions,
) -> Result<WriteEvidenceVaultCaptureMetadataReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let output_path = validate_fresh_external_output(&repo_root, &options.output_path)?;
    let capture_metadata = EvidenceVaultCaptureMetadata {
        format: EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT.to_owned(),
        canonicalization: CANONICALIZATION_ID.to_owned(),
        capture_session_id: options.capture_session_id.clone(),
        source_map_sha256: options.source_map_sha256.clone(),
        source_verification_sha256: options.source_verification_sha256.clone(),
        capture_provenance: options.capture_provenance.clone(),
    };
    validate_capture_metadata(&capture_metadata)?;
    let capture_metadata_bytes =
        canonical_serialize(&capture_metadata, "evidence vault capture metadata")?;
    let capture_metadata_sha256 = sha256_hex(&capture_metadata_bytes);

    if !options.dry_run {
        write_fresh_capture_metadata_file(&output_path, &capture_metadata_bytes)?;
    }

    Ok(WriteEvidenceVaultCaptureMetadataReport {
        capture_metadata,
        capture_metadata_bytes,
        capture_metadata_sha256,
        output_path,
        written: !options.dry_run,
    })
}

fn validate_fresh_external_output(repo_root: &Path, target: &Path) -> Result<PathBuf> {
    require_absolute_normalized(target, "vault capture-metadata output")?;
    if is_same_or_below(repo_root, target) {
        return Err(CodegenError::new(format!(
            "vault capture-metadata output `{}` must remain outside repository `{}`",
            target.display(),
            repo_root.display()
        )));
    }
    reject_symlink_ancestors(target, "vault capture-metadata output")?;
    reject_sensitive_path(target, "vault capture-metadata output")?;
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(CodegenError::new(format!(
                "vault capture-metadata output `{}` already exists; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodegenError::io(
                "inspect vault capture-metadata output",
                target,
                source,
            ));
        }
    }

    let parent = target.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "vault capture-metadata output `{}` has no parent",
            target.display()
        ))
    })?;
    reject_symlink_ancestors(parent, "vault capture-metadata output parent")?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|source| {
        CodegenError::io(
            "inspect vault capture-metadata output parent",
            parent,
            source,
        )
    })?;
    if is_symlink_or_reparse_point(&parent_metadata) || !parent_metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "vault capture-metadata output parent `{}` must be a real directory",
            parent.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent).map_err(|source| {
        CodegenError::io(
            "canonicalize vault capture-metadata output parent",
            parent,
            source,
        )
    })?;
    if is_same_or_below(repo_root, &canonical_parent) {
        return Err(CodegenError::new(format!(
            "vault capture-metadata output parent `{}` resolves beneath repository `{}`",
            parent.display(),
            repo_root.display()
        )));
    }
    reject_sensitive_path(&canonical_parent, "vault capture-metadata output parent")?;

    let file_name = target.file_name().ok_or_else(|| {
        CodegenError::new("vault capture-metadata output must have a final file name")
    })?;
    let file_name_text = file_name.to_str().ok_or_else(|| {
        CodegenError::new("vault capture-metadata output file name must be valid UTF-8")
    })?;
    if file_name_text.is_empty()
        || file_name_text.chars().any(char::is_control)
        || file_name_text.contains(':')
        || file_name_text.ends_with([' ', '.'])
    {
        return Err(CodegenError::new(
            "vault capture-metadata output file name must be non-empty, control-free, and portable",
        ));
    }

    let canonical_target = canonical_parent.join(file_name);
    if is_same_or_below(repo_root, &canonical_target) {
        return Err(CodegenError::new(format!(
            "vault capture-metadata output `{}` resolves beneath repository `{}`",
            target.display(),
            repo_root.display()
        )));
    }
    reject_sensitive_path(&canonical_target, "vault capture-metadata output")?;
    Ok(canonical_target)
}

fn write_fresh_capture_metadata_file(target: &Path, bytes: &[u8]) -> Result<()> {
    write_fresh_capture_metadata_file_with_hook(target, bytes, |_| Ok(()))
}

fn write_fresh_capture_metadata_file_with_hook<F>(
    target: &Path,
    bytes: &[u8],
    after_create: F,
) -> Result<()>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(CodegenError::new(format!(
                "vault capture-metadata output `{}` already exists; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodegenError::io(
                "inspect vault capture-metadata output before install",
                target,
                source,
            ));
        }
    }
    reject_symlink_ancestors(target, "vault capture-metadata output")?;
    let parent = target
        .parent()
        .expect("validated capture-metadata output has a parent");
    let parent_handle = Handle::from_path(parent).map_err(|source| {
        CodegenError::io(
            "identify vault capture-metadata output parent before create",
            parent,
            source,
        )
    })?;
    let mut output_file = match OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(target)
    {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(CodegenError::new(format!(
                "vault capture-metadata output `{}` appeared before create; refusing to overwrite",
                target.display()
            )));
        }
        Err(source) => {
            return Err(CodegenError::io(
                "create fresh vault capture-metadata output",
                target,
                source,
            ));
        }
    };
    let opened_handle = (|| {
        let cloned = output_file.try_clone().map_err(|source| {
            CodegenError::io(
                "clone fresh vault capture-metadata output handle",
                target,
                source,
            )
        })?;
        Handle::from_file(cloned).map_err(|source| {
            CodegenError::io(
                "identify fresh vault capture-metadata output handle",
                target,
                source,
            )
        })
    })()
    .map_err(|source| incomplete_fresh_output_error(target, "vault capture-metadata", source))?;

    // Direct create_new deliberately exposes an incomplete final file while it
    // is written. Once creation succeeds, no error path removes by pathname:
    // a same-user actor could have substituted that path.
    let operation = (|| {
        after_create(target).map_err(|source| {
            CodegenError::io(
                "run vault capture-metadata post-create verification hook",
                target,
                source,
            )
        })?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "vault capture-metadata output",
        )?;
        output_file.write_all(bytes).map_err(|source| {
            CodegenError::io("write fresh vault capture-metadata output", target, source)
        })?;
        output_file.sync_all().map_err(|source| {
            CodegenError::io("sync fresh vault capture-metadata output", target, source)
        })?;
        verify_canonical_capture_metadata_file(&mut output_file, target, bytes)?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "vault capture-metadata output",
        )?;
        sync_directory(parent)?;
        verify_fresh_output_identity(
            target,
            parent,
            &parent_handle,
            &opened_handle,
            &output_file,
            "vault capture-metadata output",
        )?;
        Ok(())
    })();
    operation
        .map_err(|source| incomplete_fresh_output_error(target, "vault capture-metadata", source))
}

fn incomplete_fresh_output_error(target: &Path, label: &str, source: CodegenError) -> CodegenError {
    CodegenError::with_source(
        format!(
            "fresh {label} output `{}` may be incomplete and was deliberately left in place; no path cleanup was attempted: {source}",
            target.display(),
        ),
        source,
    )
}

fn verify_canonical_capture_metadata_file(
    file: &mut File,
    path: &Path,
    expected: &[u8],
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| CodegenError::io("rewind fresh vault capture metadata", path, source))?;
    let mut actual = Vec::new();
    file.read_to_end(&mut actual).map_err(|source| {
        CodegenError::io("read fresh vault capture metadata handle", path, source)
    })?;
    if actual != expected {
        return Err(CodegenError::new(
            "fresh vault capture-metadata bytes drifted after write",
        ));
    }
    validate_canonical_utf8_json(&actual, path)?;
    let parsed = parse_strict(&actual, path)?;
    let capture_metadata: EvidenceVaultCaptureMetadata =
        serde_json::from_value(parsed.into_serde()).map_err(|source| {
            CodegenError::with_source(
                "closed-structure load of fresh vault capture metadata failed",
                source,
            )
        })?;
    validate_capture_metadata(&capture_metadata)
}

// This repeated handle/path/parent comparison narrows same-user races but
// cannot make path lookup transactional: an actor with write access to the
// parent can still replace entries after the final check. The safety property
// here is fail-closed non-destruction, not immunity from that actor.
fn verify_fresh_output_identity(
    target: &Path,
    parent: &Path,
    parent_handle: &Handle,
    opened_handle: &Handle,
    opened_file: &File,
    label: &str,
) -> Result<()> {
    reject_symlink_ancestors(target, label)?;
    let current_parent = Handle::from_path(parent).map_err(|source| {
        CodegenError::io(&format!("reidentify {label} parent"), parent, source)
    })?;
    if &current_parent != parent_handle {
        return Err(CodegenError::new(format!(
            "{label} parent `{}` was replaced during fresh output construction",
            parent.display()
        )));
    }
    let path_metadata = fs::symlink_metadata(target)
        .map_err(|source| CodegenError::io(&format!("inspect current {label}"), target, source))?;
    if is_symlink_or_reparse_point(&path_metadata) || !path_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "{label} `{}` changed to a non-regular or symlink/reparse entry",
            target.display()
        )));
    }
    let current_handle = Handle::from_path(target).map_err(|source| {
        CodegenError::io(&format!("reidentify current {label}"), target, source)
    })?;
    if &current_handle != opened_handle {
        return Err(CodegenError::new(format!(
            "{label} `{}` was substituted after create_new",
            target.display()
        )));
    }
    let opened_metadata = opened_file.metadata().map_err(|source| {
        CodegenError::io(&format!("inspect opened {label} handle"), target, source)
    })?;
    if is_symlink_or_reparse_point(&opened_metadata) || !opened_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "opened {label} handle for `{}` is not a real regular file",
            target.display()
        )));
    }
    reject_hard_link_alias(opened_file, &opened_metadata, target, label)
}

#[cfg(unix)]
fn reject_hard_link_alias(
    _file: &File,
    metadata: &Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{}` has {} hard links; aliased fresh outputs are forbidden",
            path.display(),
            metadata.nlink()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn reject_hard_link_alias(
    file: &File,
    _metadata: &Metadata,
    path: &Path,
    label: &str,
) -> Result<()> {
    let link_count = stable_windows_link_count(file, path, label)?;
    if link_count != 1 {
        return Err(CodegenError::new(format!(
            "{label} `{path}` has {link_count} hard links; aliased fresh outputs are forbidden",
            path = path.display()
        )));
    }
    Ok(())
}

fn canonical_serialize(value: &impl Serialize, label: &str) -> Result<Vec<u8>> {
    let ordinary = serde_json::to_vec(value)
        .map_err(|source| CodegenError::with_source(format!("serialize {label}"), source))?;
    let parsed = parse_strict(&ordinary, Path::new(label))?;
    let bytes = canonical_bytes(&parsed);
    validate_canonical_utf8_json(&bytes, Path::new(label))?;
    Ok(bytes)
}

fn validate_canonical_utf8_json(bytes: &[u8], path: &Path) -> Result<()> {
    std::str::from_utf8(bytes).map_err(|source| {
        CodegenError::with_source(
            format!("canonical JSON `{}` is not UTF-8", path.display()),
            source,
        )
    })?;
    if bytes.contains(&b'\r') {
        return Err(CodegenError::new(format!(
            "canonical JSON `{}` contains a CR line ending",
            path.display()
        )));
    }
    let parsed = parse_strict(bytes, path)?;
    if canonical_bytes(&parsed) != bytes {
        return Err(CodegenError::new(format!(
            "JSON `{}` is not canonical `{CANONICALIZATION_ID}`",
            path.display()
        )));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    match File::open(path).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        Err(source) if cfg!(windows) => {
            let _ = source;
            Ok(())
        }
        Err(source) => Err(CodegenError::io(
            "sync vault capture-metadata output directory",
            path,
            source,
        )),
    }
}

fn require_absolute_normalized(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be an explicit absolute, lexically normalized OS path",
            path.display()
        )));
    }
    Ok(())
}

fn reject_sensitive_path(path: &Path, label: &str) -> Result<()> {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect();
    let has_pair = |left: &str, right: &str| {
        components
            .windows(2)
            .any(|pair| pair[0] == left && pair[1] == right)
    };
    let sensitive = has_pair("ebirforms", "savefile")
        || has_pair("ebirforms", "profile")
        || components
            .iter()
            .any(|component| component == "group.dev.goldcoders.bir")
        || components
            .last()
            .is_some_and(|component| component == "bir_data.db")
        || components.iter().any(|component| {
            matches!(
                component.as_str(),
                "taxpayer-data"
                    | "taxpayer_data"
                    | "live-taxpayer-data"
                    | ".ssh"
                    | ".aws"
                    | ".azure"
                    | ".gnupg"
                    | ".kube"
                    | ".docker"
                    | "keychain"
                    | "keychains"
                    | "credential"
                    | "credentials"
                    | "secrets"
            )
        });
    if sensitive {
        return Err(CodegenError::new(format!(
            "{label} `{}` is beneath a known taxpayer/save/live-database root",
            path.display()
        )));
    }
    Ok(())
}

fn reject_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if is_symlink_or_reparse_point(&metadata) => {
                return Err(CodegenError::new(format!(
                    "{label} `{}` traverses symlink/reparse point `{}`",
                    path.display(),
                    ancestor.display()
                )));
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CodegenError::io(
                    &format!("inspect {label} ancestor"),
                    ancestor,
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn encode_digest(digest: impl AsRef<[u8]>) -> String {
    let bytes = digest.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode_digest(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    use super::{
        WriteEvidenceVaultCaptureMetadataOptions, canonical_bytes, canonical_serialize,
        parse_strict, sha256_hex, write_evidence_vault_capture_metadata,
        write_fresh_capture_metadata_file, write_fresh_capture_metadata_file_with_hook,
    };
    use crate::evidence::{EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance};
    use crate::vault_acquisition::EvidenceVaultCaptureMetadata;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestRoot {
        path: PathBuf,
    }

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "bir-capture-metadata-{label}-{}-{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test root");
            Self { path }
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            if self.path.exists() {
                fs::remove_dir_all(&self.path).expect("remove owned test root");
            }
        }
    }

    struct Fixture {
        _root: TestRoot,
        repo_root: PathBuf,
        external_root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let root = TestRoot::new(label);
            let repo_root = root.path.join("repo");
            let external_root = root.path.join("external");
            fs::create_dir(&repo_root).expect("create repository fixture");
            fs::create_dir(&external_root).expect("create external fixture");
            Self {
                _root: root,
                repo_root,
                external_root,
            }
        }

        fn output(&self, name: &str) -> PathBuf {
            self.external_root.join(name)
        }

        fn options(&self, name: &str) -> WriteEvidenceVaultCaptureMetadataOptions {
            WriteEvidenceVaultCaptureMetadataOptions::new(
                &self.repo_root,
                self.output(name),
                "capture-session-2026-07-26",
                source_map_sha256(),
                source_verification_sha256(),
                provenance(),
            )
        }
    }

    fn source_map_sha256() -> String {
        "1".repeat(64)
    }

    fn source_verification_sha256() -> String {
        "2".repeat(64)
    }

    fn provenance() -> EvidenceCaptureProvenance {
        EvidenceCaptureProvenance {
            tool_commit: "a".repeat(40),
            command_argv: vec![
                "bir-rules-codegen".to_owned(),
                "verify-evidence-vault-source-map".to_owned(),
                "--source-map".to_owned(),
                "../evidence/source-map.json".to_owned(),
            ],
            capture_tool_version: "bir-rules-codegen 0.1.0".to_owned(),
            operating_system: EvidenceCaptureOperatingSystem::Windows,
            windows_version: "Windows 11 23H2".to_owned(),
            official_app_version: "7.9.6.0".to_owned(),
            started_at_utc: "2026-07-26T01:00:00Z".to_owned(),
            finished_at_utc: "2026-07-26T01:05:00Z".to_owned(),
        }
    }

    #[test]
    fn dry_run_is_default_canonical_and_preserves_exact_caller_facts() {
        let fixture = Fixture::new("dry-run");
        let options = fixture.options("capture-metadata.json");
        assert!(options.dry_run);

        let report =
            write_evidence_vault_capture_metadata(&options).expect("build no-write metadata");
        assert!(!report.written);
        assert!(!options.output_path.exists());
        assert_eq!(
            report.output_path,
            fs::canonicalize(&fixture.external_root)
                .expect("canonical external fixture root")
                .join("capture-metadata.json")
        );
        assert_eq!(
            report.capture_metadata.capture_session_id,
            options.capture_session_id
        );
        assert_eq!(
            report.capture_metadata.source_map_sha256,
            options.source_map_sha256
        );
        assert_eq!(
            report.capture_metadata.source_verification_sha256,
            options.source_verification_sha256
        );
        assert_eq!(
            report.capture_metadata.capture_provenance.tool_commit,
            options.capture_provenance.tool_commit
        );
        assert_eq!(
            report.capture_metadata.capture_provenance.command_argv,
            options.capture_provenance.command_argv
        );
        assert_eq!(
            report.capture_metadata.capture_provenance.started_at_utc,
            options.capture_provenance.started_at_utc
        );
        assert_eq!(
            report.capture_metadata.capture_provenance.finished_at_utc,
            options.capture_provenance.finished_at_utc
        );
        assert!(std::str::from_utf8(&report.capture_metadata_bytes).is_ok());
        assert!(!report.capture_metadata_bytes.contains(&b'\r'));
        let parsed = parse_strict(
            &report.capture_metadata_bytes,
            Path::new("capture-metadata.json"),
        )
        .expect("parse emitted metadata");
        assert_eq!(
            canonical_bytes(&parsed),
            report.capture_metadata_bytes,
            "emitted bytes must use the crate canonicalization"
        );
        assert_eq!(
            sha256_hex(&report.capture_metadata_bytes),
            report.capture_metadata_sha256
        );
    }

    #[test]
    fn fresh_write_preserves_exact_bytes_and_refuses_overwrite() {
        let fixture = Fixture::new("fresh-write");
        let mut options = fixture.options("capture-metadata.json");
        options.dry_run = false;
        let report =
            write_evidence_vault_capture_metadata(&options).expect("write capture metadata");
        assert!(report.written);
        assert_eq!(
            fs::read(&options.output_path).expect("read output"),
            report.capture_metadata_bytes
        );

        let original = fs::read(&options.output_path).expect("read original output");
        let error =
            write_evidence_vault_capture_metadata(&options).expect_err("existing output must fail");
        assert!(
            error.to_string().contains("refusing to overwrite"),
            "{error}"
        );
        assert_eq!(
            fs::read(&options.output_path).expect("re-read original output"),
            original
        );
    }

    #[test]
    fn output_rejects_repository_escape_sensitive_and_missing_parent_paths() {
        let fixture = Fixture::new("bad-output-paths");

        let internal = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            fixture.repo_root.join("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            provenance(),
        );
        let error = write_evidence_vault_capture_metadata(&internal)
            .expect_err("repository output must fail");
        assert!(error.to_string().contains("outside repository"), "{error}");

        let escaped = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            fixture
                .external_root
                .join("nested")
                .join("..")
                .join("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            provenance(),
        );
        let error =
            write_evidence_vault_capture_metadata(&escaped).expect_err("path escape must fail");
        assert!(
            error.to_string().contains("lexically normalized"),
            "{error}"
        );

        let sensitive_root = fixture._root.path.join("ebirforms").join("savefile");
        fs::create_dir_all(&sensitive_root).expect("create sensitive fixture root");
        let sensitive = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            sensitive_root.join("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            provenance(),
        );
        let error = write_evidence_vault_capture_metadata(&sensitive)
            .expect_err("sensitive output must fail");
        assert!(
            error.to_string().contains("taxpayer/save/live-database"),
            "{error}"
        );

        let missing_parent = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            fixture
                .external_root
                .join("missing")
                .join("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            provenance(),
        );
        let error = write_evidence_vault_capture_metadata(&missing_parent)
            .expect_err("missing parent must fail");
        assert!(
            error.to_string().contains("output parent") || error.to_string().contains("ancestor"),
            "{error}"
        );
    }

    #[test]
    fn output_rejects_symlink_or_reparse_ancestors() {
        let fixture = Fixture::new("symlink-parent");
        let real_parent = fixture._root.path.join("real-external");
        fs::create_dir(&real_parent).expect("create real parent");
        let linked_parent = fixture._root.path.join("linked-external");
        match create_directory_symlink(&real_parent, &linked_parent) {
            Ok(()) => {}
            Err(source) if symlink_is_unavailable(&source) => return,
            Err(source) => panic!("create fixture directory symlink: {source}"),
        }
        let options = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            linked_parent.join("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            provenance(),
        );
        let error = write_evidence_vault_capture_metadata(&options)
            .expect_err("symlink/reparse ancestor must fail");
        assert!(error.to_string().contains("symlink/reparse"), "{error}");
        assert!(!real_parent.join("capture-metadata.json").exists());
    }

    #[test]
    fn metadata_rejects_nonportable_identifiers_provenance_and_argv() {
        let fixture = Fixture::new("bad-metadata");

        for invalid_digest in ["", "a", &"A".repeat(64), &"0".repeat(63)] {
            let mut options = fixture.options("capture-metadata.json");
            options.source_map_sha256 = invalid_digest.to_owned();
            assert!(
                write_evidence_vault_capture_metadata(&options).is_err(),
                "{invalid_digest:?} must fail as a source-map digest"
            );
        }

        for capture_session_id in ["", "Uppercase", "../escape", "double..dot", "trailing-"] {
            let options = WriteEvidenceVaultCaptureMetadataOptions::new(
                &fixture.repo_root,
                fixture.output("capture-metadata.json"),
                capture_session_id,
                source_map_sha256(),
                source_verification_sha256(),
                provenance(),
            );
            assert!(
                write_evidence_vault_capture_metadata(&options).is_err(),
                "{capture_session_id:?} must fail"
            );
        }

        for argument in [
            r"C:\Users\analyst\capture.exe",
            r"\\host\share\capture.exe",
            "file:///C:/capture.exe",
            "/Users/analyst/capture",
            "/Volumes/evidence/capture",
            r"prefix\Users\analyst\capture.exe",
            "line\nbreak",
            "",
        ] {
            let mut bad_provenance = provenance();
            bad_provenance.command_argv.push(argument.to_owned());
            let options = WriteEvidenceVaultCaptureMetadataOptions::new(
                &fixture.repo_root,
                fixture.output("capture-metadata.json"),
                "capture-session",
                source_map_sha256(),
                source_verification_sha256(),
                bad_provenance,
            );
            assert!(
                write_evidence_vault_capture_metadata(&options).is_err(),
                "{argument:?} must fail"
            );
        }

        let mut bad_commit = provenance();
        bad_commit.tool_commit = "A".repeat(40);
        let options = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            fixture.output("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            bad_commit,
        );
        assert!(
            write_evidence_vault_capture_metadata(&options)
                .expect_err("non-lowercase commit must fail")
                .to_string()
                .contains("lowercase hexadecimal")
        );

        let mut backwards = provenance();
        backwards.finished_at_utc = "2026-07-26T00:59:59Z".to_owned();
        let options = WriteEvidenceVaultCaptureMetadataOptions::new(
            &fixture.repo_root,
            fixture.output("capture-metadata.json"),
            "capture-session",
            source_map_sha256(),
            source_verification_sha256(),
            backwards,
        );
        assert!(
            write_evidence_vault_capture_metadata(&options)
                .expect_err("backwards capture interval must fail")
                .to_string()
                .contains("must not precede")
        );

        assert!(!fixture.output("capture-metadata.json").exists());
    }

    #[test]
    fn racing_publishers_get_exactly_one_nonoverwrite_install() {
        let fixture = Fixture::new("publish-race");
        let target = fixture.output("capture-metadata.json");
        let mut left = fixture.options("left-unused");
        left.capture_session_id = "capture-left".to_owned();
        let mut right = fixture.options("right-unused");
        right.capture_session_id = "capture-right".to_owned();
        let left_metadata = EvidenceVaultCaptureMetadata {
            format: crate::vault_acquisition::EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT.to_owned(),
            canonicalization: crate::json::CANONICALIZATION_ID.to_owned(),
            capture_session_id: left.capture_session_id,
            source_map_sha256: left.source_map_sha256,
            source_verification_sha256: left.source_verification_sha256,
            capture_provenance: left.capture_provenance,
        };
        let right_metadata = EvidenceVaultCaptureMetadata {
            format: crate::vault_acquisition::EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT.to_owned(),
            canonicalization: crate::json::CANONICALIZATION_ID.to_owned(),
            capture_session_id: right.capture_session_id,
            source_map_sha256: right.source_map_sha256,
            source_verification_sha256: right.source_verification_sha256,
            capture_provenance: right.capture_provenance,
        };
        let left_bytes =
            canonical_serialize(&left_metadata, "left capture metadata").expect("serialize left");
        let right_bytes = canonical_serialize(&right_metadata, "right capture metadata")
            .expect("serialize right");

        let left_target = target.clone();
        let left_publish = left_bytes.clone();
        let left_thread =
            thread::spawn(move || write_fresh_capture_metadata_file(&left_target, &left_publish));
        let right_target = target.clone();
        let right_publish = right_bytes.clone();
        let right_thread =
            thread::spawn(move || write_fresh_capture_metadata_file(&right_target, &right_publish));
        let outcomes = [
            left_thread.join().expect("left publisher did not panic"),
            right_thread.join().expect("right publisher did not panic"),
        ];
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
            1,
            "{outcomes:?}"
        );
        let installed = fs::read(&target).expect("read winning output");
        assert!(installed == left_bytes || installed == right_bytes);
    }

    #[test]
    fn hard_link_alias_after_create_is_rejected_and_neither_name_is_deleted() {
        let fixture = Fixture::new("hard-link-output");
        let target = fixture.output("capture-metadata.json");
        let alias = fixture.output("attacker-alias.json");
        let bytes = br#"{"incomplete":true}"#;
        let error = write_fresh_capture_metadata_file_with_hook(&target, bytes, |created| {
            fs::hard_link(created, &alias)
        })
        .expect_err("hard-linked fresh output must fail closed");
        assert!(error.to_string().contains("deliberately left in place"));
        assert!(target.exists(), "owned incomplete output must remain");
        assert!(alias.exists(), "attacker alias must never be removed");
    }

    #[cfg(unix)]
    #[test]
    fn substituted_target_is_reported_and_attacker_file_is_not_deleted() {
        let fixture = Fixture::new("substituted-output");
        let target = fixture.output("capture-metadata.json");
        let attacker_bytes = b"attacker-substitute";
        let error =
            write_fresh_capture_metadata_file_with_hook(&target, br#"{"owned":true}"#, |created| {
                fs::remove_file(created)?;
                fs::write(created, attacker_bytes)
            })
            .expect_err("substituted target must fail identity verification");
        assert!(error.to_string().contains("deliberately left in place"));
        assert_eq!(
            fs::read(&target).expect("substitute must remain"),
            attacker_bytes
        );
    }

    fn symlink_is_unavailable(source: &io::Error) -> bool {
        source.kind() == io::ErrorKind::PermissionDenied
            || source.kind() == io::ErrorKind::Unsupported
            || source.raw_os_error() == Some(1314)
    }

    #[cfg(unix)]
    fn create_directory_symlink(source: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn create_directory_symlink(source: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(source, link)
    }
}
