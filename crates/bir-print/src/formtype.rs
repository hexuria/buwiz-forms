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
    /// Highest page number referenced by any field (at least 1).
    pub fn page_count(&self) -> usize {
        self.fields.iter().map(|f| f.page).max().unwrap_or(1).max(1)
    }

    /// Save the FormType back to a file.
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
}

/// Text direction for character-boxed rendering.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

impl Direction {
    pub fn is_default(&self) -> bool {
        *self == Direction::Ltr
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Direction::is_default")]
    pub direction: Direction,
    #[serde(default)]
    pub optional: bool,
    /// Widget specification for the editable PDF mode.
    /// Fields without a `widget` appear only in the flat (Typst) PDF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<WidgetSpec>,
}

impl FormField {
    /// The visual bounding-box width for this field.
    ///
    /// Priority: `widget.width` → `cell_w × int_cells` → reasonable default.
    /// This must **not** return `cell_w` directly — `cell_w` is the
    /// per-character spacing for `Cells`/`Amount` fields, not the total width.
    pub fn display_width(&self) -> f64 {
        if let Some(w) = &self.widget {
            return w.width;
        }
        if let Some(cw) = self.cell_w {
            // Cells / Amount: total width ≈ cell_w × cell count
            let count = self.int_cells.unwrap_or(1) as f64;
            return cw * count;
        }
        // Text fields with no widget — use a sensible fallback
        100.0
    }

    /// The visual bounding-box height for this field.
    ///
    /// Priority: `widget.height` → `size + 4` padding → reasonable default.
    pub fn display_height(&self) -> f64 {
        if let Some(w) = &self.widget {
            return w.height;
        }
        self.size.unwrap_or(10.0) + 4.0
    }

    /// Maximum number of characters that fit in this box.
    ///
    /// For `Cells` fields, capacity is exact (`max_length` or `width / cell_w`).
    /// For `Text` fields, we use a conservative estimate based on the font.
    /// Returns `None` for Checkbox (spanning is not applicable).
    /// For `Decimal`/`Amount`, returns `char_count` if set explicitly.
    pub fn char_capacity(&self) -> Option<usize> {
        if let Some(c) = self.char_count {
            return Some(c);
        }
        match self.kind {
            FieldKind::Char => {
                let font_size = self
                    .widget
                    .as_ref()
                    .and_then(|w| w.font_size)
                    .unwrap_or(self.size.unwrap_or(8.5));
                // Arial average glyph width ≈ 0.52 × font_size.
                // Use 0.7 as a conservative multiplier to prevent overflow.
                let char_w = font_size * 0.7;

                // If it's cell-based (cell_w is present), use cell width
                if let Some(cw) = self.cell_w {
                    Some((self.display_width() / cw).floor() as usize)
                } else {
                    Some((self.display_width() / char_w).floor() as usize)
                }
            }
            FieldKind::Bool => None,
            FieldKind::Dec => self.char_count,
            FieldKind::Int => self.char_count,
        }
    }
}

/// How a field is rendered in the flat (Typst) PDF.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Bool,
    Char,
    Int,
    Dec,
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
