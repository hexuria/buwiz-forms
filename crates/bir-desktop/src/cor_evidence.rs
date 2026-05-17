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

    std::fs::create_dir_all(data_dir)
        .and_then(|_| std::fs::copy(source_path, &stored_path).map(|_| ()))
        .map_err(|error| format!("Failed to store COR document: {error}"))?;

    Ok(CorDocumentRef {
        id: document_id.to_string(),
        file_name: source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&file_name)
            .to_string(),
        stored_path: stored_path.to_string_lossy().to_string(),
        ocr_text: None,
        ocr_confidence: None,
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
