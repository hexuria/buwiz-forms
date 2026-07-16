//! Shared state and fail-closed validation for HTML print and PDF output.
//!
//! Platform WebViews create the PDF bytes, but every platform hands the result
//! through this module before a user-selected destination can be replaced.

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const GEOMETRY_TOLERANCE_POINTS: f64 = 0.25;
const FORM_CODE_INFO_KEY: &[u8] = b"BirFormCode";
const REVISION_INFO_KEY: &[u8] = b"BirFormRevision";
const ENVELOPE_HASH_INFO_KEY: &[u8] = b"BirEnvelopeSha256";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HtmlOutputKind {
    SystemPrint,
    PdfExport,
}

/// A bounded phase in the native HTML output lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlOutputTimeoutStage {
    Preflight,
    PdfExportBackend,
}

/// Classifies elapsed native-output work without timing out an active system print dialog.
///
/// System print is completion-callback-bound after its platform backend starts because the user
/// may legitimately keep the native dialog open. PDF export remains bounded because it has no
/// interactive native-dialog phase.
pub fn html_output_timeout_stage(
    kind: HtmlOutputKind,
    backend_started: bool,
    elapsed: Duration,
    readiness_timeout: Duration,
    pdf_export_timeout: Duration,
) -> Option<HtmlOutputTimeoutStage> {
    if !backend_started && elapsed >= readiness_timeout {
        return Some(HtmlOutputTimeoutStage::Preflight);
    }
    if kind == HtmlOutputKind::PdfExport && elapsed >= pdf_export_timeout {
        return Some(HtmlOutputTimeoutStage::PdfExportBackend);
    }
    None
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HtmlOutputState {
    #[default]
    Idle,
    Validating {
        kind: HtmlOutputKind,
        nonce: String,
        destination: Option<PathBuf>,
    },
    Running {
        kind: HtmlOutputKind,
        temp_path: Option<PathBuf>,
    },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfExpectation {
    pub form_code: String,
    pub revision: String,
    pub envelope_hash: String,
    pub expected_page_count: usize,
    pub width_points: f64,
    pub height_points: f64,
}

impl PdfExpectation {
    pub fn validate(&self) -> Result<(), HtmlOutputError> {
        if self.form_code.trim().is_empty() {
            return Err(HtmlOutputError::InvalidExpectation(
                "form code must not be empty".to_string(),
            ));
        }
        if self.revision.trim().is_empty() {
            return Err(HtmlOutputError::InvalidExpectation(
                "revision must not be empty".to_string(),
            ));
        }
        if self.envelope_hash.len() != 64
            || !self
                .envelope_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(HtmlOutputError::InvalidExpectation(
                "envelope hash must be a 64-character SHA-256 hex digest".to_string(),
            ));
        }
        if self.expected_page_count == 0 {
            return Err(HtmlOutputError::InvalidExpectation(
                "expected page count must be greater than zero".to_string(),
            ));
        }
        if !self.width_points.is_finite()
            || !self.height_points.is_finite()
            || self.width_points <= 0.0
            || self.height_points <= 0.0
        {
            return Err(HtmlOutputError::InvalidExpectation(
                "paper geometry must be finite and positive".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfValidationReport {
    pub page_count: usize,
    pub width_points: f64,
    pub height_points: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlOutputError {
    #[error("invalid PDF expectation: {0}")]
    InvalidExpectation(String),
    #[error("invalid PDF bytes: {0}")]
    InvalidPdf(String),
    #[error("PDF page count mismatch: expected {expected}, found {actual}")]
    PageCount { expected: usize, actual: usize },
    #[error("PDF page {page} has invalid {box_name}: {reason}")]
    InvalidPageBox {
        page: u32,
        box_name: &'static str,
        reason: String,
    },
    #[error(
        "PDF page {page} {box_name} mismatch: expected [0, 0, {expected_width}, {expected_height}], found {actual:?}"
    )]
    PageGeometry {
        page: u32,
        box_name: &'static str,
        expected_width: f64,
        expected_height: f64,
        actual: [f64; 4],
    },
    #[error("PDF page {page} has unexpected rotation {rotation}")]
    UnexpectedRotation { page: u32, rotation: f64 },
    #[error("PDF page {page} has no drawing content")]
    EmptyPage { page: u32 },
    #[error("PDF evidence {field} mismatch: expected {expected:?}, found {actual:?}")]
    EvidenceMismatch {
        field: &'static str,
        expected: String,
        actual: Option<String>,
    },
    #[error("PDF output path is invalid: {0}")]
    InvalidPath(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Create a unique, hidden sibling file for a platform PDF backend.
///
/// The backend may overwrite this empty file. Call [`finalize_pdf_export`] only
/// after the platform reports success. Failed callers should use
/// [`discard_pdf_export_temp`].
pub fn create_pdf_export_temp(destination: &Path) -> Result<PathBuf, HtmlOutputError> {
    let parent = destination.parent().ok_or_else(|| {
        HtmlOutputError::InvalidPath(format!("{} has no parent directory", destination.display()))
    })?;
    if !parent.is_dir() {
        return Err(HtmlOutputError::InvalidPath(format!(
            "destination directory does not exist: {}",
            parent.display()
        )));
    }
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            HtmlOutputError::InvalidPath(format!(
                "destination has no valid file name: {}",
                destination.display()
            ))
        })?;
    let temporary = tempfile::Builder::new()
        .prefix(&format!(".{file_name}."))
        .suffix(".partial.pdf")
        .tempfile_in(parent)?;
    let (file, path) = temporary
        .keep()
        .map_err(|error| HtmlOutputError::Io(error.error))?;
    drop(file);
    Ok(path)
}

pub fn discard_pdf_export_temp(path: &Path) -> Result<(), HtmlOutputError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Add immutable form/envelope evidence to a platform-generated PDF.
pub fn stamp_pdf_evidence(
    path: &Path,
    expectation: &PdfExpectation,
) -> Result<(), HtmlOutputError> {
    expectation.validate()?;
    let mut document =
        Document::load(path).map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?;

    let mut info = document_info_dictionary(&document).unwrap_or_default();
    info.set(
        FORM_CODE_INFO_KEY,
        Object::string_literal(expectation.form_code.as_bytes()),
    );
    info.set(
        REVISION_INFO_KEY,
        Object::string_literal(expectation.revision.as_bytes()),
    );
    info.set(
        ENVELOPE_HASH_INFO_KEY,
        Object::string_literal(expectation.envelope_hash.as_bytes()),
    );
    let info_id = document.add_object(info);
    document.trailer.set("Info", info_id);
    document
        .save(path)
        .map_err(|error| HtmlOutputError::Io(std::io::Error::other(error.to_string())))?;
    Ok(())
}

pub fn validate_pdf_file(
    path: &Path,
    expectation: &PdfExpectation,
) -> Result<PdfValidationReport, HtmlOutputError> {
    expectation.validate()?;
    let document =
        Document::load(path).map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?;
    validate_pdf_document(&document, expectation)
}

pub fn validate_pdf_bytes(
    bytes: &[u8],
    expectation: &PdfExpectation,
) -> Result<PdfValidationReport, HtmlOutputError> {
    expectation.validate()?;
    let document = Document::load_mem(bytes)
        .map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?;
    validate_pdf_document(&document, expectation)
}

/// Merge platform captures that each contain exactly one page.
///
/// WKWebView is intentionally captured one validated DOM rectangle at a time;
/// this keeps CSS page geometry from becoming one tall PDF page. Inherited
/// page attributes are materialized before the source page tree is discarded.
pub fn merge_single_page_pdfs(page_pdfs: &[Vec<u8>]) -> Result<Vec<u8>, HtmlOutputError> {
    if page_pdfs.is_empty() {
        return Err(HtmlOutputError::InvalidPdf(
            "cannot merge an empty page list".to_string(),
        ));
    }

    let mut merged = Document::with_version("1.7");
    let pages_root_id = merged.new_object_id();
    let mut next_object_id = pages_root_id.0.saturating_add(1);
    let mut merged_pages = BTreeMap::new();

    for (index, page_bytes) in page_pdfs.iter().enumerate() {
        let mut source = Document::load_mem(page_bytes).map_err(|error| {
            HtmlOutputError::InvalidPdf(format!("captured page {}: {error}", index + 1))
        })?;
        if source.get_pages().len() != 1 {
            return Err(HtmlOutputError::InvalidPdf(format!(
                "captured page {} must contain exactly one PDF page",
                index + 1
            )));
        }

        source.renumber_objects_with(next_object_id);
        next_object_id = source.max_id.saturating_add(1);
        let (_, page_id) =
            source.get_pages().into_iter().next().ok_or_else(|| {
                HtmlOutputError::InvalidPdf("captured page disappeared".to_string())
            })?;
        let mut page = source
            .get_dictionary(page_id)
            .map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?
            .clone();

        for key in [b"MediaBox".as_slice(), b"CropBox", b"Resources", b"Rotate"] {
            if !page.has(key) {
                if let Some(value) = inherited_page_value(&source, page_id, key) {
                    page.set(key, value.clone());
                }
            }
        }
        page.set("Parent", pages_root_id);
        merged_pages.insert(page_id, Object::Dictionary(page));

        for (object_id, object) in source.objects {
            if object_id == page_id {
                continue;
            }
            match object.type_name().unwrap_or("") {
                "Catalog" | "Pages" | "Page" | "Outlines" | "Outline" => {}
                _ => {
                    merged.objects.insert(object_id, object);
                }
            }
        }
    }

    let page_ids = merged_pages.keys().copied().collect::<Vec<_>>();
    merged.objects.extend(merged_pages);
    merged.objects.insert(
        pages_root_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => page_ids.len() as i64,
        }),
    );
    let catalog_id = merged.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_root_id,
    });
    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged
        .objects
        .keys()
        .map(|(object_id, _)| *object_id)
        .max()
        .unwrap_or_default();
    merged.renumber_objects();

    let mut bytes = Vec::new();
    merged
        .save_to(&mut bytes)
        .map_err(|error| HtmlOutputError::Io(std::io::Error::other(error.to_string())))?;
    Ok(bytes)
}

/// Stamp, validate, flush, and atomically replace the chosen destination.
///
/// Any failure removes the sibling temporary file and leaves a pre-existing
/// destination untouched.
pub fn finalize_pdf_export(
    temp_path: &Path,
    destination: &Path,
    expectation: &PdfExpectation,
) -> Result<PdfValidationReport, HtmlOutputError> {
    if temp_path == destination {
        return Err(HtmlOutputError::InvalidPath(
            "temporary and destination paths must differ".to_string(),
        ));
    }

    let result = (|| {
        stamp_pdf_evidence(temp_path, expectation)?;
        let report = validate_pdf_file(temp_path, expectation)?;
        fs::OpenOptions::new()
            .read(true)
            .open(temp_path)?
            .sync_all()?;
        atomic_replace(temp_path, destination)?;
        Ok(report)
    })();

    if result.is_err() {
        let _ = discard_pdf_export_temp(temp_path);
    }
    result
}

fn validate_pdf_document(
    document: &Document,
    expectation: &PdfExpectation,
) -> Result<PdfValidationReport, HtmlOutputError> {
    validate_pdf_evidence(document, expectation)?;
    let pages = document.get_pages();
    if pages.len() != expectation.expected_page_count {
        return Err(HtmlOutputError::PageCount {
            expected: expectation.expected_page_count,
            actual: pages.len(),
        });
    }

    for (page_number, page_id) in pages {
        let media_box = inherited_page_box(document, page_id, b"MediaBox", page_number)?;
        validate_page_geometry(
            page_number,
            "MediaBox",
            media_box,
            expectation.width_points,
            expectation.height_points,
        )?;

        let crop_box = match inherited_page_value(document, page_id, b"CropBox") {
            Some(value) => parse_page_box(document, value, page_number, "CropBox")?,
            None => media_box,
        };
        validate_page_geometry(
            page_number,
            "CropBox",
            crop_box,
            expectation.width_points,
            expectation.height_points,
        )?;

        if let Some(rotation) = inherited_page_value(document, page_id, b"Rotate") {
            let rotation = object_number(document, rotation).ok_or_else(|| {
                HtmlOutputError::InvalidPageBox {
                    page: page_number,
                    box_name: "Rotate",
                    reason: "rotation must be a finite number".to_string(),
                }
            })?;
            if rotation.abs() > f64::EPSILON {
                return Err(HtmlOutputError::UnexpectedRotation {
                    page: page_number,
                    rotation,
                });
            }
        }

        let content = document
            .get_page_content(page_id)
            .map_err(|error| HtmlOutputError::InvalidPdf(error.to_string()))?;
        if content.iter().all(u8::is_ascii_whitespace) {
            return Err(HtmlOutputError::EmptyPage { page: page_number });
        }
    }

    Ok(PdfValidationReport {
        page_count: expectation.expected_page_count,
        width_points: expectation.width_points,
        height_points: expectation.height_points,
    })
}

fn validate_pdf_evidence(
    document: &Document,
    expectation: &PdfExpectation,
) -> Result<(), HtmlOutputError> {
    let info = document_info_dictionary(document);
    for (key, field, expected) in [
        (
            FORM_CODE_INFO_KEY,
            "form code",
            expectation.form_code.as_str(),
        ),
        (REVISION_INFO_KEY, "revision", expectation.revision.as_str()),
        (
            ENVELOPE_HASH_INFO_KEY,
            "envelope hash",
            expectation.envelope_hash.as_str(),
        ),
    ] {
        let actual = info
            .as_ref()
            .and_then(|dictionary| dictionary.get(key).ok())
            .and_then(|value| resolved_object(document, value))
            .and_then(|value| value.as_string().ok())
            .map(|value| value.into_owned());
        if actual.as_deref() != Some(expected) {
            return Err(HtmlOutputError::EvidenceMismatch {
                field,
                expected: expected.to_string(),
                actual,
            });
        }
    }
    Ok(())
}

fn document_info_dictionary(document: &Document) -> Option<Dictionary> {
    let info = document.trailer.get(b"Info").ok()?;
    resolved_object(document, info)?.as_dict().ok().cloned()
}

fn inherited_page_box(
    document: &Document,
    page_id: ObjectId,
    key: &[u8],
    page_number: u32,
) -> Result<[f64; 4], HtmlOutputError> {
    let value = inherited_page_value(document, page_id, key).ok_or_else(|| {
        HtmlOutputError::InvalidPageBox {
            page: page_number,
            box_name: "MediaBox",
            reason: "missing inherited page box".to_string(),
        }
    })?;
    parse_page_box(document, value, page_number, "MediaBox")
}

fn inherited_page_value<'a>(
    document: &'a Document,
    mut object_id: ObjectId,
    key: &[u8],
) -> Option<&'a Object> {
    for _ in 0..64 {
        let dictionary = document.get_dictionary(object_id).ok()?;
        if let Ok(value) = dictionary.get(key) {
            return Some(value);
        }
        object_id = dictionary.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn parse_page_box(
    document: &Document,
    value: &Object,
    page_number: u32,
    box_name: &'static str,
) -> Result<[f64; 4], HtmlOutputError> {
    let value =
        resolved_object(document, value).ok_or_else(|| HtmlOutputError::InvalidPageBox {
            page: page_number,
            box_name,
            reason: "indirect page box could not be resolved".to_string(),
        })?;
    let values = value
        .as_array()
        .map_err(|_| HtmlOutputError::InvalidPageBox {
            page: page_number,
            box_name,
            reason: "page box must be an array".to_string(),
        })?;
    if values.len() != 4 {
        return Err(HtmlOutputError::InvalidPageBox {
            page: page_number,
            box_name,
            reason: format!(
                "page box must have four coordinates, found {}",
                values.len()
            ),
        });
    }
    let mut parsed = [0.0; 4];
    for (index, coordinate) in values.iter().enumerate() {
        parsed[index] =
            object_number(document, coordinate).ok_or_else(|| HtmlOutputError::InvalidPageBox {
                page: page_number,
                box_name,
                reason: format!("coordinate {index} is not a finite number"),
            })?;
    }
    if parsed[2] <= parsed[0] || parsed[3] <= parsed[1] {
        return Err(HtmlOutputError::InvalidPageBox {
            page: page_number,
            box_name,
            reason: "upper-right coordinates must exceed lower-left coordinates".to_string(),
        });
    }
    Ok(parsed)
}

fn validate_page_geometry(
    page: u32,
    box_name: &'static str,
    actual: [f64; 4],
    expected_width: f64,
    expected_height: f64,
) -> Result<(), HtmlOutputError> {
    let expected = [0.0, 0.0, expected_width, expected_height];
    if actual
        .iter()
        .zip(expected)
        .any(|(actual, expected)| (actual - expected).abs() > GEOMETRY_TOLERANCE_POINTS)
    {
        return Err(HtmlOutputError::PageGeometry {
            page,
            box_name,
            expected_width,
            expected_height,
            actual,
        });
    }
    Ok(())
}

fn object_number(document: &Document, value: &Object) -> Option<f64> {
    let value = resolved_object(document, value)?;
    let number = match value {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => return None,
    };
    number.is_finite().then_some(number)
}

fn resolved_object<'a>(document: &'a Document, mut value: &'a Object) -> Option<&'a Object> {
    for _ in 0..64 {
        match value {
            Object::Reference(object_id) => value = document.get_object(*object_id).ok()?,
            _ => return Some(value),
        }
    }
    None
}

#[cfg(not(windows))]
fn atomic_replace(temp_path: &Path, destination: &Path) -> Result<(), HtmlOutputError> {
    fs::rename(temp_path, destination)?;
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(temp_path: &Path, destination: &Path) -> Result<(), HtmlOutputError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let target: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: both arguments are owned, NUL-terminated UTF-16 buffers that
    // remain alive for the duration of this synchronous Win32 call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    const TEST_READINESS_TIMEOUT: Duration = Duration::from_secs(5);
    const TEST_PDF_EXPORT_TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn active_system_print_has_no_backend_deadline() {
        let stage = html_output_timeout_stage(
            HtmlOutputKind::SystemPrint,
            true,
            Duration::from_secs(3_600),
            TEST_READINESS_TIMEOUT,
            TEST_PDF_EXPORT_TIMEOUT,
        );
        assert_eq!(stage, None);
    }

    #[test]
    fn active_pdf_export_expires_at_its_backend_deadline() {
        let stage = html_output_timeout_stage(
            HtmlOutputKind::PdfExport,
            true,
            TEST_PDF_EXPORT_TIMEOUT,
            TEST_READINESS_TIMEOUT,
            TEST_PDF_EXPORT_TIMEOUT,
        );
        assert_eq!(stage, Some(HtmlOutputTimeoutStage::PdfExportBackend));
    }

    #[test]
    fn system_print_preflight_expires_before_backend_start() {
        let stage = html_output_timeout_stage(
            HtmlOutputKind::SystemPrint,
            false,
            TEST_READINESS_TIMEOUT,
            TEST_READINESS_TIMEOUT,
            TEST_PDF_EXPORT_TIMEOUT,
        );
        assert_eq!(stage, Some(HtmlOutputTimeoutStage::Preflight));
    }

    fn expectation(page_count: usize) -> PdfExpectation {
        PdfExpectation {
            form_code: "2551Q".to_string(),
            revision: "2018".to_string(),
            envelope_hash: "a".repeat(64),
            expected_page_count: page_count,
            width_points: 612.0,
            height_points: 936.0,
        }
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

    fn stamped_pdf(page_count: usize) -> (tempfile::TempDir, PathBuf, PdfExpectation) {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("form.pdf");
        let expectation = expectation(page_count);
        write_pdf(&path, page_count, 612, 936, b"q 1 0 0 1 0 0 cm Q");
        stamp_pdf_evidence(&path, &expectation).expect("evidence should be stamped");
        (directory, path, expectation)
    }

    #[test]
    fn validates_expected_page_geometry_and_evidence() {
        let (_directory, path, expectation) = stamped_pdf(2);
        let report = validate_pdf_file(&path, &expectation).expect("PDF should validate");
        assert_eq!(report.page_count, 2);
        assert_eq!(report.width_points, 612.0);
        assert_eq!(report.height_points, 936.0);
    }

    #[test]
    fn rejects_incorrect_page_count() {
        let (_directory, path, mut expectation) = stamped_pdf(1);
        expectation.expected_page_count = 2;
        assert!(matches!(
            validate_pdf_file(&path, &expectation),
            Err(HtmlOutputError::PageCount {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn rejects_incorrect_page_geometry() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("form.pdf");
        let expectation = expectation(1);
        write_pdf(&path, 1, 612, 935, b"q Q");
        stamp_pdf_evidence(&path, &expectation).expect("evidence should be stamped");
        assert!(matches!(
            validate_pdf_file(&path, &expectation),
            Err(HtmlOutputError::PageGeometry { .. })
        ));
    }

    #[test]
    fn rejects_empty_page_content() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("form.pdf");
        let expectation = expectation(1);
        write_pdf(&path, 1, 612, 936, b" \n\t");
        stamp_pdf_evidence(&path, &expectation).expect("evidence should be stamped");
        assert!(matches!(
            validate_pdf_file(&path, &expectation),
            Err(HtmlOutputError::EmptyPage { page: 1 })
        ));
    }

    #[test]
    fn rejects_mismatched_envelope_evidence() {
        let (_directory, path, mut expectation) = stamped_pdf(1);
        expectation.envelope_hash = "b".repeat(64);
        assert!(matches!(
            validate_pdf_file(&path, &expectation),
            Err(HtmlOutputError::EvidenceMismatch {
                field: "envelope hash",
                ..
            })
        ));
    }

    #[test]
    fn failed_export_preserves_existing_destination() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let destination = directory.path().join("selected.pdf");
        fs::write(&destination, b"existing destination").expect("destination should be written");
        let temp_path = create_pdf_export_temp(&destination).expect("temporary path should exist");
        fs::write(&temp_path, b"not a PDF").expect("invalid PDF should be written");

        assert!(finalize_pdf_export(&temp_path, &destination, &expectation(1)).is_err());
        assert_eq!(
            fs::read(&destination).expect("destination should remain readable"),
            b"existing destination"
        );
        assert!(!temp_path.exists());
    }

    #[test]
    fn valid_export_atomically_replaces_destination() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let destination = directory.path().join("selected.pdf");
        fs::write(&destination, b"old").expect("destination should be written");
        let temp_path = create_pdf_export_temp(&destination).expect("temporary path should exist");
        write_pdf(&temp_path, 1, 612, 936, b"q Q");

        let report = finalize_pdf_export(&temp_path, &destination, &expectation(1))
            .expect("valid export should replace destination");
        assert_eq!(report.page_count, 1);
        assert!(!temp_path.exists());
        validate_pdf_file(&destination, &expectation(1))
            .expect("final destination should remain valid");
    }

    #[test]
    fn merges_one_capture_per_page_in_renderer_order() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let first = directory.path().join("first.pdf");
        let second = directory.path().join("second.pdf");
        write_pdf(&first, 1, 612, 936, b"q 1 0 0 1 1 1 cm Q");
        write_pdf(&second, 1, 612, 936, b"q 1 0 0 1 2 2 cm Q");
        let bytes = merge_single_page_pdfs(&[
            fs::read(first).expect("first capture should be readable"),
            fs::read(second).expect("second capture should be readable"),
        ])
        .expect("captures should merge");
        let merged = Document::load_mem(&bytes).expect("merged PDF should load");
        assert_eq!(merged.get_pages().len(), 2);
    }
}
