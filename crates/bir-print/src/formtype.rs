//! FormType schema — typed Rust structs for `formtype.json`.
//!
//! These structs define the canonical layout and widget specification for a
//! BIR form type.  The flat PDF renderer uses the layout fields (kind, page,
//! x, y, cell_w, …) while the editable PDF renderer additionally reads
//! the `widget` sub-object.

use serde::{Deserialize, Serialize};

/// Root of a `formtype.json` file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FormType {
    pub form_id: String,
    pub page_width: f64,
    pub page_height: f64,
    pub fields: Vec<FormField>,
}

impl FormType {
    /// Highest page number referenced by any field (at least 2 for 2551Q).
    pub fn page_count(&self) -> usize {
        self.fields.iter().map(|f| f.page).max().unwrap_or(0).max(2)
    }

    /// Save the FormType back to a file.
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
}

/// A single field in the form layout.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FormField {
    pub key: String,
    pub kind: FieldKind,
    pub page: usize,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub int_cells: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dec_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default)]
    pub optional: bool,
    /// Widget specification for the editable PDF mode.
    /// Fields without a `widget` appear only in the flat (Typst) PDF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<WidgetSpec>,
}

/// How a field is rendered in the flat (Typst) PDF.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Checkbox,
    Text,
    Cells,
    Amount,
}

/// Widget specification for the editable PDF — drives AcroForm injection.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WidgetSpec {
    #[serde(rename = "type")]
    pub widget_type: WidgetType,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comb: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// The PDF widget annotation type.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    Text,
    Checkbox,
}
