//! Disabled-by-default, non-promotional macOS runtime-observation sink.
//!
//! This module accepts no fixture or fault input. It records only closed,
//! path-free observations emitted after the existing native output succeeds.

use bir_print::certification_observation::{
    CertificationDestinationSnapshotV1, CertificationDestinationUnavailableReasonV1,
    MacosCandidateRuntimeObservationV1,
};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const EVIDENCE_DIR_ENV: &str = "EBIR_CERTIFICATION_EVIDENCE_DIR";
const CHALLENGE_ENV: &str = "EBIR_CERTIFICATION_EVIDENCE_CHALLENGE";

static SINK: OnceLock<Result<Option<CertificationEvidenceSink>, String>> = OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) struct CertificationOutputContext {
    pub(crate) challenge_sha256: String,
    pub(crate) started_at_unix_ms: u64,
    pub(crate) destination_before: CertificationDestinationSnapshotV1,
}

#[derive(Debug)]
struct CertificationEvidenceSink {
    root_directory: File,
    challenge_sha256: String,
}

pub(crate) fn begin_certification_output(
    destination: Option<&Path>,
) -> Result<Option<CertificationOutputContext>, String> {
    let Some(sink) = configured_sink()? else {
        return Ok(None);
    };
    Ok(Some(CertificationOutputContext {
        challenge_sha256: sink.challenge_sha256.clone(),
        started_at_unix_ms: unix_ms_now(),
        destination_before: destination
            .map(destination_snapshot)
            .unwrap_or(CertificationDestinationSnapshotV1::Absent),
    }))
}

pub(crate) fn write_certification_observation(
    observation: &MacosCandidateRuntimeObservationV1,
) -> Result<Option<String>, String> {
    let Some(sink) = configured_sink()? else {
        return Ok(None);
    };
    if observation.collector_challenge_sha256 != sink.challenge_sha256 {
        return Err("certification observation challenge does not match the process sink".into());
    }
    sink.write(observation).map(Some)
}

pub(crate) fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn destination_snapshot(path: &Path) -> CertificationDestinationSnapshotV1 {
    match hash_regular_file(path) {
        Ok(Some(sha256)) => CertificationDestinationSnapshotV1::File { sha256 },
        Ok(None) => CertificationDestinationSnapshotV1::Absent,
        Err(reason_code) => CertificationDestinationSnapshotV1::Unavailable { reason_code },
    }
}

fn configured_sink() -> Result<Option<&'static CertificationEvidenceSink>, String> {
    SINK.get_or_init(CertificationEvidenceSink::from_environment)
        .as_ref()
        .map(|sink| sink.as_ref())
        .map_err(Clone::clone)
}

impl CertificationEvidenceSink {
    fn from_environment() -> Result<Option<Self>, String> {
        Self::from_settings(
            std::env::var_os(EVIDENCE_DIR_ENV),
            std::env::var_os(CHALLENGE_ENV),
            &protected_runtime_roots(),
        )
    }

    fn from_settings(
        root: Option<std::ffi::OsString>,
        challenge: Option<std::ffi::OsString>,
        protected: &[PathBuf],
    ) -> Result<Option<Self>, String> {
        match (root, challenge) {
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => {
                Err("certification evidence requires both directory and challenge settings".into())
            }
            (Some(root), Some(challenge)) => {
                let challenge = challenge
                    .into_string()
                    .map_err(|_| "certification challenge must be UTF-8".to_string())?;
                Self::from_parts(Path::new(&root), &challenge, protected).map(Some)
            }
        }
    }

    fn from_parts(root: &Path, challenge: &str, protected: &[PathBuf]) -> Result<Self, String> {
        if challenge.len() != 64
            || !challenge
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("certification challenge must be 64 lowercase hexadecimal bytes".into());
        }
        if !root.is_absolute() {
            return Err("certification evidence directory must be absolute".into());
        }
        reject_symlink_components(root)?;
        let canonical = root
            .canonicalize()
            .map_err(|_| "certification evidence directory could not be resolved".to_string())?;
        if canonical != root {
            return Err("certification evidence directory must use its canonical path".into());
        }
        let metadata = std::fs::metadata(&canonical)
            .map_err(|_| "certification evidence directory metadata is unavailable".to_string())?;
        if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(
                "certification evidence directory must be owned by the current user".into(),
            );
        }
        if metadata.permissions().mode() & 0o777 != 0o700 {
            return Err("certification evidence directory mode must be 0700".into());
        }
        if std::fs::read_dir(&canonical)
            .map_err(|_| "certification evidence directory could not be inspected".to_string())?
            .next()
            .is_some()
        {
            return Err("certification evidence directory must be empty at process start".into());
        }
        if protected
            .iter()
            .any(|protected| canonical.starts_with(protected))
        {
            return Err(
                "certification evidence directory is inside a protected app resource".into(),
            );
        }
        let root = CString::new(canonical.as_os_str().as_bytes())
            .map_err(|_| "certification evidence directory contains an embedded NUL".to_string())?;
        let descriptor = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err("certification evidence directory could not be securely opened".into());
        }
        let root_directory = unsafe { File::from_raw_fd(descriptor) };
        let opened = root_directory
            .metadata()
            .map_err(|_| "certification evidence directory descriptor is invalid".to_string())?;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err("certification evidence directory changed during setup".into());
        }
        Ok(Self {
            root_directory,
            challenge_sha256: sha256_hex(challenge.as_bytes()),
        })
    }

    fn write(&self, observation: &MacosCandidateRuntimeObservationV1) -> Result<String, String> {
        observation.validate_non_promotional()?;
        let bytes = serde_json::to_vec_pretty(observation)
            .map_err(|_| "certification observation could not be encoded".to_string())?;
        let kind = match &observation.output {
            bir_print::certification_observation::MacosCandidateOutputObservationV1::PdfExportSucceeded { .. } => "pdf",
            bir_print::certification_observation::MacosCandidateOutputObservationV1::SystemPrintCompleted { .. } => "print",
        };
        let name = format!(
            "runtime-{}-{}-{}.json",
            &observation.document_run_id_sha256[..16],
            observation.issued_nonce,
            kind
        );
        let temporary_name = format!(".{name}.{}.tmp", uuid::Uuid::new_v4());
        let destination = CString::new(name.as_bytes())
            .map_err(|_| "certification artifact name is invalid".to_string())?;
        let temporary = CString::new(temporary_name.as_bytes())
            .map_err(|_| "certification temporary name is invalid".to_string())?;
        let root_fd = self.root_directory.as_raw_fd();
        let result = (|| {
            let descriptor = unsafe {
                libc::openat(
                    root_fd,
                    temporary.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if descriptor < 0 {
                return Err("certification temporary artifact could not be created".to_string());
            }
            let mut file = unsafe { File::from_raw_fd(descriptor) };
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| {
                    "certification temporary artifact could not be persisted".to_string()
                })?;
            if unsafe {
                libc::linkat(
                    root_fd,
                    temporary.as_ptr(),
                    root_fd,
                    destination.as_ptr(),
                    0,
                )
            } != 0
            {
                return Err(
                    "certification artifact already exists or could not be installed".to_string(),
                );
            }
            if unsafe { libc::unlinkat(root_fd, temporary.as_ptr(), 0) } != 0 {
                return Err("certification temporary artifact could not be removed".to_string());
            }
            self.root_directory.sync_all().map_err(|_| {
                "certification evidence directory could not be synchronized".to_string()
            })?;
            Ok(name)
        })();
        if result.is_err() {
            unsafe {
                libc::unlinkat(root_fd, temporary.as_ptr(), 0);
            }
        }
        result
    }
}

fn hash_regular_file(
    path: &Path,
) -> Result<Option<String>, CertificationDestinationUnavailableReasonV1> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CertificationDestinationUnavailableReasonV1::MetadataReadFailed),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CertificationDestinationUnavailableReasonV1::NotRegularFile);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| CertificationDestinationUnavailableReasonV1::FileReadFailed)?;
    if !file
        .metadata()
        .map_err(|_| CertificationDestinationUnavailableReasonV1::MetadataReadFailed)?
        .is_file()
    {
        return Err(CertificationDestinationUnavailableReasonV1::NotRegularFile);
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| CertificationDestinationUnavailableReasonV1::FileReadFailed)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(Some(format!("{:x}", hash.finalize())))
}

fn reject_symlink_components(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|_| {
            "certification evidence directory has an unreadable ancestor".to_string()
        })?;
        if metadata.file_type().is_symlink() {
            return Err("certification evidence directory may not traverse symlinks".into());
        }
    }
    Ok(())
}

fn protected_runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        for ancestor in executable.ancestors() {
            if ancestor
                .extension()
                .is_some_and(|extension| extension == "app")
            {
                if let Ok(root) = ancestor.canonicalize() {
                    roots.push(root);
                }
                break;
            }
        }
    }
    let renderer = crate::platform::find_resource_dir("assets").join("form-renderer");
    if let Ok(renderer) = renderer.canonicalize() {
        roots.push(renderer);
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_print::certification_observation::{
        CertificationGeometryPageV1, CertificationGeometryReportV1, CertificationVerifierGapV1,
        MACOS_CANDIDATE_RUNTIME_OBSERVATION_SCHEMA_VERSION,
        MACOS_CANDIDATE_RUNTIME_OBSERVATION_SCOPE, MacosCandidateOutputObservationV1,
    };
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn private_directory() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("private evidence directory");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private permissions");
        root
    }

    fn observation(challenge_sha256: String) -> MacosCandidateRuntimeObservationV1 {
        let report = CertificationGeometryReportV1 {
            page_count: 2,
            page_width_pt: 612.0,
            page_height_pt: 936.0,
            pages: (0..2)
                .map(|page| CertificationGeometryPageV1 {
                    x: 0.0,
                    y: page as f64 * 1248.0,
                    width: 816.0,
                    height: 1248.0,
                    client_width: 816.0,
                    client_height: 1248.0,
                    scroll_width: 816.0,
                    scroll_height: 1248.0,
                    descendant_overflow_x: 0,
                    descendant_overflow_y: 0,
                    descendant_clipped_x: 0,
                    descendant_clipped_y: 0,
                })
                .collect(),
        };
        MacosCandidateRuntimeObservationV1 {
            schema_version: MACOS_CANDIDATE_RUNTIME_OBSERVATION_SCHEMA_VERSION,
            scope: MACOS_CANDIDATE_RUNTIME_OBSERVATION_SCOPE.to_string(),
            promotion_eligible: false,
            trusted_producer: false,
            collector_challenge_sha256: challenge_sha256,
            form_code: "2551Q".to_string(),
            form_revision: "2018".to_string(),
            document_run_id_sha256: "b".repeat(64),
            envelope_sha256: "c".repeat(64),
            render_epoch: 1,
            readiness_revision: 1,
            issued_nonce: 1,
            preflight_consumptions: vec![1],
            backend_completion_nonce: 1,
            started_at_unix_ms: 1,
            completed_at_unix_ms: 2,
            geometry_reports: [report.clone(), report],
            output: MacosCandidateOutputObservationV1::SystemPrintCompleted {
                appkit_completion_succeeded: true,
            },
            strict_verifier_gaps: [
                CertificationVerifierGapV1::RuntimeSelfAuthored,
                CertificationVerifierGapV1::ExternalUiAndPrintRequired,
                CertificationVerifierGapV1::ExternalCandidateBindingRequired,
            ],
        }
    }

    #[test]
    fn settings_are_disabled_only_when_both_values_are_absent() {
        assert!(
            CertificationEvidenceSink::from_settings(None, None, &[])
                .expect("disabled settings")
                .is_none()
        );
        assert!(CertificationEvidenceSink::from_settings(Some("/tmp".into()), None, &[],).is_err());
    }

    #[test]
    fn rejects_noncanonical_challenge_and_open_permissions() {
        let root = tempfile::tempdir().expect("temp dir");
        let root = root.path().canonicalize().expect("canonical temp dir");
        assert!(CertificationEvidenceSink::from_parts(&root, "abc", &[]).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
            .expect("open permissions");
        assert!(CertificationEvidenceSink::from_parts(&root, &"a".repeat(64), &[]).is_err());
    }

    #[test]
    fn writes_mode_0600_without_overwrite_or_identity_leaks() {
        let root = private_directory();
        let root_path = root.path().canonicalize().expect("canonical evidence dir");
        let challenge = "a".repeat(64);
        let sink =
            CertificationEvidenceSink::from_parts(&root_path, &challenge, &[]).expect("valid sink");
        let observation = observation(sink.challenge_sha256.clone());
        let name = sink.write(&observation).expect("first write");
        let artifact = root_path.join(&name);
        let first = std::fs::read(&artifact).expect("artifact bytes");
        assert_eq!(
            std::fs::metadata(&artifact)
                .expect("artifact metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let encoded = String::from_utf8(first.clone()).expect("JSON text");
        let root_text = root_path.to_string_lossy().into_owned();
        for forbidden in [
            root_text.as_str(),
            "destination_path",
            "envelope_json",
            "taxpayer",
            "address",
            "email",
            "phone",
        ] {
            assert!(!encoded.contains(forbidden), "leaked {forbidden}");
        }
        assert!(sink.write(&observation).is_err());
        assert_eq!(std::fs::read(&artifact).expect("preserved artifact"), first);
    }

    #[test]
    fn rejects_symlink_and_protected_roots() {
        let parent = private_directory();
        let parent_path = parent.path().canonicalize().expect("canonical parent");
        let target = parent_path.join("target");
        std::fs::create_dir(&target).expect("target directory");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))
            .expect("target permissions");
        let linked = parent_path.join("linked");
        symlink(&target, &linked).expect("directory symlink");
        let challenge = "a".repeat(64);
        assert!(CertificationEvidenceSink::from_parts(&linked, &challenge, &[]).is_err());
        assert!(
            CertificationEvidenceSink::from_parts(&target, &challenge, &[parent_path],).is_err()
        );
    }
}
