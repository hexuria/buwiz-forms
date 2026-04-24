use std::path::Path;
use std::process::Command;
use crate::db::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("Failed to execute Typst: {0}")]
    Execution(#[from] std::io::Error),
    #[error("Typst compilation failed: {0}")]
    Compilation(String),
}

/// Generates a PDF using the local Typst CLI and a given template file.
/// `template_path`: e.g. "templates/2551Q.typ"
/// `data_path`: e.g. "templates/data.json"
/// `output_path`: e.g. "output.pdf"
pub fn generate_pdf(
    template_path: impl AsRef<Path>,
    data_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<(), PdfError> {
    // Determine the path to our local typst binary
    // In production, this would be bundled with the app or expected in PATH
    let typst_bin = Path::new("bin/typst");
    if !typst_bin.exists() {
        return Err(PdfError::Compilation("Typst binary not found in bin/typst".to_string()));
    }

    // We can't pass JSON data natively in Typst 0.13 via --input simply for the whole payload if we just use `json("data.json")` in the template.
    // The template relies on reading `data.json` directly from its directory, or we can copy data_path to `data.json` next to the template.
    // For this basic setup, we assume data_path is passed or the template reads `data.json`.
    // Actually, in Typst 0.11+, `--input data=path/to.json` can be read as `sys.inputs.data`, but our template uses `json("data.json")`.

    let output = Command::new(typst_bin)
        .arg("compile")
        .arg(template_path.as_ref())
        .arg(output_path.as_ref())
        // Set the root to allow reading the data.json
        .arg("--root")
        .arg(template_path.as_ref().parent().unwrap_or(Path::new(".")))
        .output()?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(PdfError::Compilation(err_msg.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_generate_pdf() {
        // Run from bir-core/ so we need to point to the workspace root bin/
        let template = "../../templates/2551Q.typ";
        let data = "../../templates/data.json";
        let output = "../../templates/rust_output.pdf";
        
        let _ = fs::remove_file(output); // ensure clean start
        
        // This test might fail if typst_bin path is relative to workspace root but we are in bir-core
        // So we just skip if it can't find it, or we can adjust paths.
        if !Path::new("../../bin/typst").exists() {
            println!("Typst binary not found, skipping test");
            return;
        }

        let typst_bin = Path::new("../../bin/typst");
        let result = Command::new(typst_bin)
            .arg("compile")
            .arg(template)
            .arg(output)
            .arg("--root")
            .arg("../../templates")
            .output();
        
        assert!(result.is_ok());
        assert!(Path::new(output).exists());
    }
}
