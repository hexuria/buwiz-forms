use std::path::{Path, PathBuf};

use bir_core::profile::CorDocumentRef;

const ALLOWED_COR_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "pdf"];

pub(crate) fn store_cor_document(source_path: &Path, tin: &str) -> Result<CorDocumentRef, String> {
    let data_dir = bir_core::db::default_database_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cor_documents");
    let document_id = uuid::Uuid::new_v4().to_string();
    store_cor_document_in_dir(source_path, tin, &data_dir, &document_id)
}

fn store_cor_document_in_dir(
    source_path: &Path,
    tin: &str,
    data_dir: &Path,
    document_id: &str,
) -> Result<CorDocumentRef, String> {
    let ext = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .ok_or_else(|| "COR upload requires png, jpg, jpeg, or pdf.".to_string())?;

    if !ALLOWED_COR_EXTENSIONS.contains(&ext.as_str()) {
        return Err("COR upload requires png, jpg, jpeg, or pdf.".to_string());
    }

    let tin_part = sanitize_tin_for_filename(tin);
    let file_name = format!("cor-{tin_part}-{document_id}.{ext}");
    let stored_path = data_dir.join(&file_name);

    tracing::info!(
        "[COR Evidence] Storing document: {} → {}",
        source_path.display(),
        stored_path.display()
    );

    std::fs::create_dir_all(data_dir)
        .and_then(|_| std::fs::copy(source_path, &stored_path).map(|_| ()))
        .map_err(|error| format!("Failed to store COR document: {error}"))?;

    tracing::info!("[COR Evidence] File copied successfully. ext={ext}");

    // Convert PDF to PNG for in-app rendering (gpui::img() only supports raster images)
    let viewer_path = if ext == "pdf" && !cfg!(test) {
        let png_path = stored_path.with_extension("png");
        tracing::info!("[COR Evidence] PDF detected — converting to PNG via qlmanage");

        // qlmanage generates thumbnails in a temporary directory.
        // Use -t (thumbnail) -s 1600 (size) -o (output dir)
        let output_dir = data_dir.join("ql_tmp");
        let _ = std::fs::create_dir_all(&output_dir);

        let status = std::process::Command::new("qlmanage")
            .args([
                "-t",
                "-s",
                "1600",
                "-o",
                &output_dir.to_string_lossy(),
                &stored_path.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() => {
                // qlmanage creates the file as "<original_name>.png" in the output dir
                let ql_generated = output_dir.join(format!("{}.png", file_name));
                if ql_generated.exists() {
                    if let Err(e) = std::fs::rename(&ql_generated, &png_path) {
                        tracing::info!("[COR Evidence] rename failed: {e}, trying copy");
                        let _ = std::fs::copy(&ql_generated, &png_path);
                    }
                    let _ = std::fs::remove_dir_all(&output_dir);
                    if png_path.exists() {
                        tracing::info!(
                            "[COR Evidence] PDF→PNG conversion succeeded: {}",
                            png_path.display()
                        );
                        png_path
                    } else {
                        tracing::info!(
                            "[COR Evidence] PNG file not created after rename/copy, falling back to PDF path"
                        );
                        stored_path.clone()
                    }
                } else {
                    // qlmanage might name it differently; scan the output dir
                    tracing::info!(
                        "[COR Evidence] Expected ql output not found at {:?}, scanning dir",
                        ql_generated
                    );
                    let found_png = std::fs::read_dir(&output_dir).ok().and_then(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .find(|e| {
                                e.path()
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .map(|ext| ext == "png")
                                    .unwrap_or(false)
                            })
                            .map(|e| e.path())
                    });
                    if let Some(found) = found_png {
                        let _ = std::fs::copy(&found, &png_path);
                        let _ = std::fs::remove_dir_all(&output_dir);
                        tracing::info!(
                            "[COR Evidence] PDF→PNG conversion succeeded (alt name): {}",
                            png_path.display()
                        );
                        png_path
                    } else {
                        let _ = std::fs::remove_dir_all(&output_dir);
                        tracing::info!(
                            "[COR Evidence] qlmanage produced no PNG output, falling back to PDF path"
                        );
                        stored_path.clone()
                    }
                }
            }
            Ok(s) => {
                tracing::info!("[COR Evidence] qlmanage exited with status: {s}");
                let _ = std::fs::remove_dir_all(&output_dir);
                stored_path.clone()
            }
            Err(e) => {
                tracing::info!("[COR Evidence] qlmanage failed to execute: {e}");
                let _ = std::fs::remove_dir_all(&output_dir);
                stored_path.clone()
            }
        }
    } else {
        tracing::info!("[COR Evidence] Raster image, no conversion needed");
        stored_path.clone()
    };

    tracing::info!(
        "[COR Evidence] Final viewer_path: {}",
        viewer_path.display()
    );

    Ok(CorDocumentRef {
        id: document_id.to_string(),
        file_name: source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&file_name)
            .to_string(),
        stored_path: viewer_path.to_string_lossy().to_string(),
        uploaded_at: Some(chrono::Local::now().naive_local()),
        provider: None,
        model: None,
        document_type: None,
        extracted_form_codes: vec![],
        ocr_text: None,
        ocr_confidence: None,
        field_bboxes: std::collections::HashMap::new(),
    })
}

fn sanitize_tin_for_filename(tin: &str) -> String {
    let sanitized = tin
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown-tin".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cor_dir() -> PathBuf {
        std::env::temp_dir().join(format!("bir-cor-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn stores_cor_document_with_sanitized_name() {
        let dir = temp_cor_dir();
        let source = dir.join("source file.PDF");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"test-cor").unwrap();

        let evidence = store_cor_document_in_dir(&source, "000-111-222-00000", &dir, "doc-id")
            .expect("store cor document");

        let stored_path = PathBuf::from(&evidence.stored_path);
        assert_eq!(evidence.id, "doc-id");
        assert_eq!(evidence.file_name, "source file.PDF");
        assert_eq!(
            stored_path.file_name().and_then(|name| name.to_str()),
            Some("cor-00011122200000-doc-id.pdf")
        );
        assert_eq!(std::fs::read(stored_path).unwrap(), b"test-cor");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_unsupported_cor_document_extension() {
        let dir = temp_cor_dir();
        let source = dir.join("source.txt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"test-cor").unwrap();

        let error = store_cor_document_in_dir(&source, "000", &dir, "doc-id").unwrap_err();

        assert!(error.contains("png, jpg, jpeg, or pdf"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
