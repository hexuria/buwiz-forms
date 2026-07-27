use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use same_file::Handle;

use crate::error::{CodegenError, Result};
use crate::hash::sha256_hex;
use crate::json::{CANONICALIZATION_ID, canonical_bytes, parse_strict};
use crate::path::{is_same_path, is_symlink_or_reparse_point};
use crate::verified_file::open_verified_external_regular_file;
#[cfg(windows)]
use crate::verified_file::stable_windows_link_count;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalizeJsonReport {
    pub canonicalization: &'static str,
    pub input_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub size_bytes: u64,
    pub sha256: String,
    pub written: bool,
}

struct LoadedInput {
    path: PathBuf,
    bytes: Vec<u8>,
}

struct PreparedOutput {
    path: PathBuf,
    parent: PathBuf,
    parent_handle: Handle,
}

/// Verify that a JSON file already consists of the exact canonical bytes.
pub fn check_canonical_json(path: &Path) -> Result<CanonicalizeJsonReport> {
    let input = load_input(path)?;
    let value = parse_strict(&input.bytes, &input.path)?;
    let canonical = canonical_bytes(&value);
    if input.bytes != canonical {
        return Err(CodegenError::new(format!(
            "JSON input `{}` is not exact `{CANONICALIZATION_ID}` bytes",
            input.path.display()
        )));
    }
    validate_canonical_bytes(&input.bytes, &input.path)?;
    Ok(report(&input, None, &input.bytes, false))
}

/// Canonicalize one strict JSON input into a fresh, distinct output file.
pub fn canonicalize_json(input_path: &Path, output_path: &Path) -> Result<CanonicalizeJsonReport> {
    let input = load_input(input_path)?;
    let value = parse_strict(&input.bytes, &input.path)?;
    let bytes = canonical_bytes(&value);
    validate_canonical_bytes(&bytes, Path::new("generated canonical JSON"))?;

    let output = prepare_output(output_path)?;
    if is_same_path(&input.path, &output.path) {
        return Err(CodegenError::new(format!(
            "canonical JSON input and output resolve to the same path `{}`",
            input.path.display()
        )));
    }
    reject_existing_output(&output.path)?;
    write_fresh_verified(&output, &bytes)?;

    Ok(report(&input, Some(output.path), &bytes, true))
}

/// Minimal parser for the standalone canonicalization command.
pub fn run_canonicalize_json_command(arguments: impl IntoIterator<Item = String>) -> Result<()> {
    let mut check = None;
    let mut input = None;
    let mut output = None;
    let mut help = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--check" => set_once(
                &mut check,
                PathBuf::from(next_value(&mut arguments, "--check")?),
                "--check",
            )?,
            "--input" => set_once(
                &mut input,
                PathBuf::from(next_value(&mut arguments, "--input")?),
                "--input",
            )?,
            "--output" => set_once(
                &mut output,
                PathBuf::from(next_value(&mut arguments, "--output")?),
                "--output",
            )?,
            "--help" | "-h" => help = true,
            other => {
                return Err(CodegenError::new(format!(
                    "unknown argument `{other}` for `canonicalize-json`\n\n{}",
                    canonicalize_json_usage()
                )));
            }
        }
    }
    if help {
        println!("{}", canonicalize_json_usage());
        return Ok(());
    }

    match (check, input, output) {
        (Some(path), None, None) => {
            let report = check_canonical_json(&path)?;
            println!(
                "verified {}: {} byte(s), SHA-256 {}, canonicalization {}",
                report.input_path.display(),
                report.size_bytes,
                report.sha256,
                report.canonicalization
            );
            Ok(())
        }
        (None, Some(input), Some(output)) => {
            let report = canonicalize_json(&input, &output)?;
            let output = report
                .output_path
                .as_deref()
                .expect("write report always carries an output path");
            println!(
                "wrote {}: {} byte(s), SHA-256 {}, canonicalization {}",
                output.display(),
                report.size_bytes,
                report.sha256,
                report.canonicalization
            );
            Ok(())
        }
        _ => Err(CodegenError::new(format!(
            "`canonicalize-json` requires exactly `--check FILE` or \
             `--input FILE --output FRESH-FILE`\n\n{}",
            canonicalize_json_usage()
        ))),
    }
}

pub fn canonicalize_json_usage() -> String {
    [
        "Usage:",
        "  bir-rules-codegen canonicalize-json --check FILE",
        "  bir-rules-codegen canonicalize-json --input FILE --output FRESH-FILE",
        "",
        "Strict parsing rejects duplicate keys and trailing content. Conversion",
        "never mutates the input or overwrites an existing output.",
    ]
    .join("\n")
}

fn report(
    input: &LoadedInput,
    output_path: Option<PathBuf>,
    canonical: &[u8],
    written: bool,
) -> CanonicalizeJsonReport {
    CanonicalizeJsonReport {
        canonicalization: CANONICALIZATION_ID,
        input_path: input.path.clone(),
        output_path,
        size_bytes: canonical.len() as u64,
        sha256: sha256_hex(canonical),
        written,
    }
}

fn load_input(path: &Path) -> Result<LoadedInput> {
    let path = absolute_normalized_path(path, "canonical JSON input")?;
    let mut file = open_verified_external_regular_file(&path, "canonical JSON input", |_| Ok(()))?;
    let canonical_path = file.canonical_path().to_path_buf();
    let mut bytes = Vec::new();
    file.file_mut()
        .read_to_end(&mut bytes)
        .map_err(|source| CodegenError::io("read canonical JSON input", &canonical_path, source))?;
    Ok(LoadedInput {
        path: canonical_path,
        bytes,
    })
}

fn prepare_output(path: &Path) -> Result<PreparedOutput> {
    let path = absolute_normalized_path(path, "canonical JSON output")?;
    let file_name = path.file_name().map(ToOwned::to_owned).ok_or_else(|| {
        CodegenError::new(format!(
            "canonical JSON output `{}` has no final file name",
            path.display()
        ))
    })?;
    validate_portable_file_name(&file_name, &path)?;
    let parent = path.parent().ok_or_else(|| {
        CodegenError::new(format!(
            "canonical JSON output `{}` has no parent",
            path.display()
        ))
    })?;
    reject_symlink_ancestors(parent, "canonical JSON output parent")?;
    let metadata = fs::symlink_metadata(parent).map_err(|source| {
        CodegenError::io("inspect canonical JSON output parent", parent, source)
    })?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "canonical JSON output parent `{}` must be a real directory",
            parent.display()
        )));
    }
    let parent = fs::canonicalize(parent).map_err(|source| {
        CodegenError::io("canonicalize canonical JSON output parent", parent, source)
    })?;
    reject_symlink_ancestors(&parent, "canonical JSON output parent")?;
    let path = parent.join(file_name);
    let parent_handle = Handle::from_path(&parent).map_err(|source| {
        CodegenError::io(
            "identify canonical JSON output parent before create",
            &parent,
            source,
        )
    })?;
    Ok(PreparedOutput {
        path,
        parent,
        parent_handle,
    })
}

fn write_fresh_verified(output: &PreparedOutput, bytes: &[u8]) -> Result<()> {
    reject_existing_output(&output.path)?;
    verify_parent_identity(output)?;
    reject_symlink_ancestors(&output.path, "canonical JSON output")?;

    let mut file = match OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&output.path)
    {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(CodegenError::new(format!(
                "canonical JSON output `{}` appeared before create; refusing to overwrite",
                output.path.display()
            )));
        }
        Err(source) => {
            return Err(CodegenError::io(
                "create fresh canonical JSON output",
                &output.path,
                source,
            ));
        }
    };
    let opened_handle = file
        .try_clone()
        .and_then(Handle::from_file)
        .map_err(|source| {
            incomplete_output_error(
                &output.path,
                CodegenError::io(
                    "identify fresh canonical JSON output handle",
                    &output.path,
                    source,
                ),
            )
        })?;
    let expected_sha256 = sha256_hex(bytes);

    let operation = (|| {
        verify_output_identity(output, &opened_handle, &file)?;
        file.write_all(bytes).map_err(|source| {
            CodegenError::io("write fresh canonical JSON output", &output.path, source)
        })?;
        file.sync_all().map_err(|source| {
            CodegenError::io("sync fresh canonical JSON output", &output.path, source)
        })?;
        verify_written_bytes(&mut file, &output.path, bytes, &expected_sha256)?;
        verify_output_identity(output, &opened_handle, &file)?;
        sync_directory(&output.parent)?;
        verify_output_identity(output, &opened_handle, &file)
    })();
    operation.map_err(|source| incomplete_output_error(&output.path, source))
}

fn verify_written_bytes(
    file: &mut File,
    path: &Path,
    expected: &[u8],
    expected_sha256: &str,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| CodegenError::io("rewind fresh canonical JSON output", path, source))?;
    let mut actual = Vec::new();
    file.read_to_end(&mut actual)
        .map_err(|source| CodegenError::io("reread fresh canonical JSON output", path, source))?;
    let actual_sha256 = sha256_hex(&actual);
    if actual != expected || actual_sha256 != expected_sha256 {
        return Err(CodegenError::new(format!(
            "fresh canonical JSON output `{}` failed byte/hash verification \
             (expected {expected_sha256}, observed {actual_sha256})",
            path.display()
        )));
    }
    validate_canonical_bytes(&actual, path)
}

fn verify_parent_identity(output: &PreparedOutput) -> Result<()> {
    reject_symlink_ancestors(&output.parent, "canonical JSON output parent")?;
    let metadata = fs::symlink_metadata(&output.parent).map_err(|source| {
        CodegenError::io(
            "reinspect canonical JSON output parent",
            &output.parent,
            source,
        )
    })?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(CodegenError::new(format!(
            "canonical JSON output parent `{}` is no longer a real directory",
            output.parent.display()
        )));
    }
    let current = Handle::from_path(&output.parent).map_err(|source| {
        CodegenError::io(
            "reidentify canonical JSON output parent",
            &output.parent,
            source,
        )
    })?;
    if current != output.parent_handle {
        return Err(CodegenError::new(format!(
            "canonical JSON output parent `{}` was replaced before publication",
            output.parent.display()
        )));
    }
    Ok(())
}

fn verify_output_identity(
    output: &PreparedOutput,
    opened_handle: &Handle,
    opened_file: &File,
) -> Result<()> {
    verify_parent_identity(output)?;
    reject_symlink_ancestors(&output.path, "canonical JSON output")?;
    let path_metadata = fs::symlink_metadata(&output.path).map_err(|source| {
        CodegenError::io("inspect fresh canonical JSON output", &output.path, source)
    })?;
    if is_symlink_or_reparse_point(&path_metadata) || !path_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "canonical JSON output `{}` changed to a non-regular or symlink/reparse entry",
            output.path.display()
        )));
    }
    let current_handle = Handle::from_path(&output.path).map_err(|source| {
        CodegenError::io(
            "reidentify fresh canonical JSON output",
            &output.path,
            source,
        )
    })?;
    if &current_handle != opened_handle {
        return Err(CodegenError::new(format!(
            "canonical JSON output `{}` was substituted after create_new",
            output.path.display()
        )));
    }
    let opened_metadata = opened_file.metadata().map_err(|source| {
        CodegenError::io(
            "inspect opened canonical JSON output handle",
            &output.path,
            source,
        )
    })?;
    if is_symlink_or_reparse_point(&opened_metadata) || !opened_metadata.is_file() {
        return Err(CodegenError::new(format!(
            "opened canonical JSON output handle for `{}` is not a real regular file",
            output.path.display()
        )));
    }
    reject_hard_link_alias(opened_file, &opened_metadata, &output.path)?;

    let canonical = fs::canonicalize(&output.path).map_err(|source| {
        CodegenError::io(
            "canonicalize fresh canonical JSON output",
            &output.path,
            source,
        )
    })?;
    if !is_same_path(&canonical, &output.path) {
        return Err(CodegenError::new(format!(
            "canonical JSON output `{}` escaped its verified parent as `{}`",
            output.path.display(),
            canonical.display()
        )));
    }
    Ok(())
}

fn validate_canonical_bytes(bytes: &[u8], path: &Path) -> Result<()> {
    if bytes.starts_with(UTF8_BOM) {
        return Err(CodegenError::new(format!(
            "canonical JSON `{}` contains a UTF-8 BOM",
            path.display()
        )));
    }
    if bytes.contains(&b'\r') {
        return Err(CodegenError::new(format!(
            "canonical JSON `{}` contains a CR byte",
            path.display()
        )));
    }
    if bytes.last() == Some(&b'\n') {
        return Err(CodegenError::new(format!(
            "canonical JSON `{}` contains a trailing LF",
            path.display()
        )));
    }
    let value = parse_strict(bytes, path)?;
    if canonical_bytes(&value) != bytes {
        return Err(CodegenError::new(format!(
            "JSON `{}` is not exact `{CANONICALIZATION_ID}` bytes",
            path.display()
        )));
    }
    Ok(())
}

fn reject_existing_output(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_symlink_or_reparse_point(&metadata) => Err(CodegenError::new(format!(
            "canonical JSON output `{}` is a symlink/reparse point; refusing it",
            path.display()
        ))),
        Ok(_) => Err(CodegenError::new(format!(
            "canonical JSON output `{}` already exists; refusing to overwrite",
            path.display()
        ))),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(CodegenError::io(
            "inspect canonical JSON output",
            path,
            source,
        )),
    }
}

fn absolute_normalized_path(path: &Path, label: &str) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || contains_dot_component(path) {
        return Err(CodegenError::new(format!(
            "{label} `{}` must be a non-empty, lexically normalized path without `.` or `..`",
            path.display()
        )));
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(CodegenError::new(format!(
            "{label} `{}` is neither a portable relative path nor an absolute path",
            path.display()
        )));
    }
    let current = std::env::current_dir()
        .map_err(|source| CodegenError::with_source("resolve current directory", source))?;
    let current = fs::canonicalize(&current)
        .map_err(|source| CodegenError::io("canonicalize current directory", &current, source))?;
    Ok(current.join(path))
}

fn contains_dot_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path
            .as_os_str()
            .to_string_lossy()
            .split(|character| character == '/' || character == '\\')
            .any(|component| component == "." || component == "..")
}

fn validate_portable_file_name(name: &std::ffi::OsStr, path: &Path) -> Result<()> {
    let name = name.to_str().ok_or_else(|| {
        CodegenError::new(format!(
            "canonical JSON output file name `{}` is not valid UTF-8",
            path.display()
        ))
    })?;
    if name.is_empty()
        || name.chars().any(char::is_control)
        || name.contains(':')
        || name.ends_with([' ', '.'])
    {
        return Err(CodegenError::new(format!(
            "canonical JSON output file name `{name}` is not portable"
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

#[cfg(unix)]
fn reject_hard_link_alias(_file: &File, metadata: &Metadata, path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(CodegenError::new(format!(
            "canonical JSON output `{}` has {} hard links; aliased outputs are forbidden",
            path.display(),
            metadata.nlink()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn reject_hard_link_alias(file: &File, _metadata: &Metadata, path: &Path) -> Result<()> {
    let count = stable_windows_link_count(file, path, "canonical JSON output")?;
    if count != 1 {
        return Err(CodegenError::new(format!(
            "canonical JSON output `{}` has {count} hard links; aliased outputs are forbidden",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn reject_hard_link_alias(_file: &File, _metadata: &Metadata, path: &Path) -> Result<()> {
    let _ = path;
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
            "sync canonical JSON output directory",
            path,
            source,
        )),
    }
}

fn incomplete_output_error(path: &Path, source: CodegenError) -> CodegenError {
    CodegenError::with_source(
        format!(
            "fresh canonical JSON output `{}` may be incomplete and was deliberately left in \
             place; no pathname cleanup was attempted: {source}",
            path.display()
        ),
        source,
    )
}

fn next_value(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String> {
    arguments
        .next()
        .ok_or_else(|| CodegenError::new(format!("missing value for `{option}`")))
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(CodegenError::new(format!(
            "{option} may be provided only once"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{canonicalize_json, check_canonical_json, run_canonicalize_json_command};
    use crate::hash::sha256_hex;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "bir-rules-canonical-json-{label}-{}-{}",
                std::process::id(),
                TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test root");
            Self(fs::canonicalize(path).expect("canonicalize test root"))
        }

        fn join(&self, value: impl AsRef<Path>) -> PathBuf {
            self.0.join(value)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test root");
        }
    }

    #[test]
    fn check_requires_exact_canonical_bytes() {
        let root = TestRoot::new("check");
        let canonical = root.join("canonical.json");
        fs::write(&canonical, br#"{"a":1,"b":[2,3]}"#).expect("write canonical input");
        let report = check_canonical_json(&canonical).expect("canonical input passes");
        assert_eq!(report.size_bytes, 17);
        assert_eq!(report.sha256, sha256_hex(br#"{"a":1,"b":[2,3]}"#));
        assert!(!report.written);

        let formatted = root.join("formatted.json");
        fs::write(&formatted, b"{\"a\":1}\n").expect("write formatted input");
        let error = check_canonical_json(&formatted)
            .expect_err("trailing LF must make exact-byte check fail");
        assert!(error.to_string().contains("bir-json-c14n-v1"));
        assert_eq!(
            fs::read(&formatted).expect("input remains readable"),
            b"{\"a\":1}\n"
        );
    }

    #[test]
    fn conversion_writes_only_exact_canonical_bytes_and_hash() {
        let root = TestRoot::new("write");
        let input = root.join("input.json");
        let output = root.join("output.json");
        let original = b"{\r\n  \"z\": 2,\r\n  \"a\": [3, 1]\r\n}";
        fs::write(&input, original).expect("write input");

        let report = canonicalize_json(&input, &output).expect("canonicalize input");
        let expected = br#"{"a":[3,1],"z":2}"#;
        assert_eq!(fs::read(&output).expect("read output"), expected);
        assert_eq!(fs::read(&input).expect("reread input"), original);
        assert_eq!(report.size_bytes, expected.len() as u64);
        assert_eq!(report.sha256, sha256_hex(expected));
        assert_eq!(report.output_path.as_deref(), Some(output.as_path()));
        assert!(report.written);
        assert!(!expected.starts_with(b"\xef\xbb\xbf"));
        assert!(!expected.contains(&b'\r'));
        assert_ne!(expected.last(), Some(&b'\n'));
    }

    #[test]
    fn conversion_rejects_duplicate_keys_and_trailing_content_without_output() {
        let root = TestRoot::new("strict");
        for (index, bytes) in [
            br#"{"same":1,"same":2}"#.as_slice(),
            br#"{"valid":true} false"#.as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            let input = root.join(format!("input-{index}.json"));
            let output = root.join(format!("output-{index}.json"));
            fs::write(&input, bytes).expect("write invalid input");
            canonicalize_json(&input, &output).expect_err("strict parse must fail");
            assert!(!output.exists(), "strict failure must not create output");
            assert_eq!(fs::read(&input).expect("input survives"), bytes);
        }
    }

    #[test]
    fn conversion_refuses_same_path_and_existing_output_without_mutation() {
        let root = TestRoot::new("fresh");
        let input = root.join("input.json");
        fs::write(&input, br#"{"a":1}"#).expect("write input");
        let same_error =
            canonicalize_json(&input, &input).expect_err("same input/output must fail");
        assert!(same_error.to_string().contains("same path"));
        assert_eq!(
            fs::read(&input).expect("same-path input survives"),
            br#"{"a":1}"#
        );

        let output = root.join("existing.json");
        fs::write(&output, b"sentinel").expect("write existing output");
        let error =
            canonicalize_json(&input, &output).expect_err("existing output must fail closed");
        assert!(error.to_string().contains("already exists"));
        assert_eq!(
            fs::read(&output).expect("existing output survives"),
            b"sentinel"
        );
    }

    #[test]
    fn conversion_rejects_lexical_parent_escape() {
        let root = TestRoot::new("escape");
        let input = root.join("input.json");
        fs::write(&input, br#"{"a":1}"#).expect("write input");
        let separator = std::path::MAIN_SEPARATOR;
        let output = PathBuf::from(format!(
            "{}{separator}nested{separator}..{separator}escaped.json",
            root.0.display()
        ));
        assert!(super::contains_dot_component(&output));
        let error = canonicalize_json(&input, &output).expect_err("parent component must fail");
        assert!(error.to_string().contains("lexically normalized"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_input_and_output_parent_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new("symlink");
        let input = root.join("input.json");
        let linked_input = root.join("linked-input.json");
        fs::write(&input, br#"{"a":1}"#).expect("write input");
        symlink(&input, &linked_input).expect("create input symlink");
        let error = check_canonical_json(&linked_input).expect_err("symlink input must fail");
        assert!(error.to_string().contains("symlink/reparse"));

        let real_parent = root.join("real-parent");
        let linked_parent = root.join("linked-parent");
        fs::create_dir(&real_parent).expect("create real output parent");
        symlink(&real_parent, &linked_parent).expect("create output-parent symlink");
        let error = canonicalize_json(&input, &linked_parent.join("output.json"))
            .expect_err("symlink parent must fail");
        assert!(error.to_string().contains("symlink/reparse"));
        assert!(!real_parent.join("output.json").exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_reparse_input_and_output_parent_are_rejected_when_links_are_available() {
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let root = TestRoot::new("reparse");
        let input = root.join("input.json");
        let linked_input = root.join("linked-input.json");
        fs::write(&input, br#"{"a":1}"#).expect("write input");
        if symlink_file(&input, &linked_input).is_err() {
            return;
        }
        let error = check_canonical_json(&linked_input).expect_err("reparse input must fail");
        assert!(error.to_string().contains("symlink/reparse"));

        let real_parent = root.join("real-parent");
        let linked_parent = root.join("linked-parent");
        fs::create_dir(&real_parent).expect("create real output parent");
        if symlink_dir(&real_parent, &linked_parent).is_err() {
            return;
        }
        let error = canonicalize_json(&input, &linked_parent.join("output.json"))
            .expect_err("reparse parent must fail");
        assert!(error.to_string().contains("symlink/reparse"));
        assert!(!real_parent.join("output.json").exists());
    }

    #[test]
    fn command_parser_requires_one_exact_mode() {
        let error = run_canonicalize_json_command(
            ["--check", "one.json", "--output", "two.json"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("mixed modes must fail");
        assert!(error.to_string().contains("requires exactly"));

        let duplicate = run_canonicalize_json_command(
            ["--check", "one.json", "--check", "two.json"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("duplicate flag must fail");
        assert!(duplicate.to_string().contains("only once"));
    }
}
