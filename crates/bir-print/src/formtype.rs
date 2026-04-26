//! FormType schema — typed Rust structs for `formtype.json`.
//!
//! These structs define the canonical layout and widget specification for a
//! BIR form type.  The flat PDF renderer uses the layout fields (kind, page,
//! x, y, cell_w, …) while the editable PDF renderer additionally reads
//! the `widget` sub-object.

use serde::Deserialize;

/// Root of a `formtype.json` file.
#[derive(Debug, Deserialize)]
pub struct FormType {
    pub form_id: String,
    pub page_width: f64,
    pub page_height: f64,
    pub fields: Vec<FormField>,
}

impl FormType {
    /// Highest page number referenced by any field (at least 2 for 2551Q).
    pub fn page_count(&self) -> usize {
        self.fields
            .iter()
            .map(|f| f.page)
            .max()
            .unwrap_or(0)
            .max(2)
    }
}

/// A single field in the form layout.
#[derive(Debug, Deserialize)]
pub struct FormField {
    pub key: String,
    pub kind: FieldKind,
    pub page: usize,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub cell_w: Option<f64>,
    #[serde(default)]
    pub int_cells: Option<usize>,
    #[serde(default)]
    pub dec_x: Option<f64>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub optional: bool,
    /// Widget specification for the editable PDF mode.
    /// Fields without a `widget` appear only in the flat (Typst) PDF.
    #[serde(default)]
    pub widget: Option<WidgetSpec>,
}

/// How a field is rendered in the flat (Typst) PDF.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Checkbox,
    Text,
    Cells,
    Amount,
}

/// Widget specification for the editable PDF — drives AcroForm injection.
#[derive(Debug, Deserialize, Clone)]
pub struct WidgetSpec {
    #[serde(rename = "type")]
    pub widget_type: WidgetType,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub comb: Option<bool>,
    #[serde(default)]
    pub font_size: Option<f64>,
}

/// The PDF widget annotation type.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    Text,
    Checkbox,
}
