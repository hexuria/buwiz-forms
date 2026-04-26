//! Editable PDF renderer — injects AcroForm widget annotations into a
//! flat (Typst-rendered) PDF to produce a real fillable form.
//!
//! Uses `lopdf` to load the base PDF, add `/AcroForm` and per-page
//! `/Annots` with `/Widget` annotations for text and checkbox fields.

use crate::formtype::{FormField, FormType, WidgetSpec, WidgetType};
use crate::PrintError;
use lopdf::dictionary;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::BTreeMap;
use std::path::Path;

/// Inject AcroForm widget annotations into an existing PDF.
///
/// - `base_pdf_path`: path to the flat PDF produced by the Typst renderer.
/// - `formtype`: the parsed [`FormType`] describing field locations and widgets.
/// - `fields`: the app-data field map (key → value).
/// - `output_path`: where to write the editable PDF.
pub fn inject_acroform(
    base_pdf_path: &Path,
    formtype: &FormType,
    fields: &BTreeMap<String, String>,
    output_path: &Path,
) -> Result<(), PrintError> {
    let mut doc = Document::load(base_pdf_path).map_err(|e| {
        PrintError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("lopdf load failed: {e}"),
        ))
    })?;

    // Collect page object IDs in page order.
    let page_ids = doc.get_pages();
    let mut sorted_pages: Vec<(u32, ObjectId)> = page_ids.into_iter().collect();
    sorted_pages.sort_by_key(|(num, _)| *num);

    let page_height = formtype.page_height;

    // Build widgets and track which page they belong to.
    let mut all_field_refs: Vec<Object> = Vec::new();
    let mut page_annots: BTreeMap<u32, Vec<Object>> = BTreeMap::new();

    for field_def in &formtype.fields {
        let Some(widget_spec) = &field_def.widget else {
            continue;
        };
        let value = fields.get(&field_def.key).cloned().unwrap_or_default();

        // Skip empty optional fields
        if value.is_empty() && field_def.optional {
            continue;
        }

        let widget_dict = match widget_spec.widget_type {
            WidgetType::Text => build_text_widget(field_def, widget_spec, &value, page_height),
            WidgetType::Checkbox => {
                build_checkbox_widget(field_def, widget_spec, &value, page_height)
            }
        };

        let obj_id = doc.add_object(Object::Dictionary(widget_dict));
        all_field_refs.push(Object::Reference(obj_id));
        page_annots
            .entry(field_def.page as u32)
            .or_default()
            .push(Object::Reference(obj_id));
    }

    if all_field_refs.is_empty() {
        // Nothing to inject — just copy the file as-is.
        std::fs::copy(base_pdf_path, output_path)?;
        return Ok(());
    }

    // Add /AcroForm to the document catalog.
    let acroform_dict = dictionary! {
        "Fields" => Object::Array(all_field_refs),
        "NeedAppearances" => Object::Boolean(true)
    };
    let acroform_id = doc.add_object(Object::Dictionary(acroform_dict));

    // Patch the catalog with /AcroForm reference.
    if let Some(Object::Reference(root_id)) = doc.trailer.get(b"Root").ok() {
        let root_id = *root_id;
        if let Ok(root_obj) = doc.get_object_mut(root_id) {
            if let Object::Dictionary(ref mut d) = root_obj {
                d.set("AcroForm", Object::Reference(acroform_id));
            }
        }
    }

    // Add /Annots to each page that has widgets.
    for (page_num, annot_refs) in &page_annots {
        if let Some((_, page_oid)) = sorted_pages.iter().find(|(n, _)| *n == *page_num) {
            if let Ok(page_obj) = doc.get_object_mut(*page_oid) {
                if let Object::Dictionary(ref mut page_dict) = page_obj {
                    // Merge with any existing /Annots.
                    let mut existing = match page_dict.get(b"Annots") {
                        Ok(Object::Array(arr)) => arr.clone(),
                        _ => Vec::new(),
                    };
                    existing.extend(annot_refs.iter().cloned());
                    page_dict.set("Annots", Object::Array(existing));
                }
            }
        }
    }

    doc.save(output_path).map_err(|e| {
        PrintError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("lopdf save failed: {e}"),
        ))
    })?;

    Ok(())
}

/// Build a text widget annotation dictionary.
fn build_text_widget(
    field: &FormField,
    spec: &WidgetSpec,
    value: &str,
    page_height: f64,
) -> Dictionary {
    // Convert Typst top-left coordinates to PDF bottom-left coordinates.
    let x1 = field.x;
    let y2 = page_height - field.y; // top of field in PDF coords
    let y1 = y2 - spec.height; // bottom of field in PDF coords
    let x2 = x1 + spec.width;

    let rect = Object::Array(vec![
        Object::Real(x1 as f32),
        Object::Real(y1 as f32),
        Object::Real(x2 as f32),
        Object::Real(y2 as f32),
    ]);

    let font_size = spec.font_size.unwrap_or(8.5);
    let da = format!("/Helv {font_size} Tf 0 g");

    let mut ff: u32 = 0;
    // Comb bit: bit 25 (1 << 24, zero-indexed from bit 1)
    if spec.comb.unwrap_or(false) {
        ff |= 1 << 24; // Comb
    }

    let mut dict = dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Widget".to_vec()),
        "FT" => Object::Name(b"Tx".to_vec()),
        "T" => Object::String(field.key.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "V" => Object::String(value.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "DA" => Object::String(da.into_bytes(), lopdf::StringFormat::Literal),
        "Rect" => rect,
        "F" => Object::Integer(4) // Print flag
    };

    if ff != 0 {
        dict.set("Ff", Object::Integer(ff as i64));
    }
    if let Some(max_len) = spec.max_length {
        dict.set("MaxLen", Object::Integer(max_len as i64));
    }

    dict
}

/// Build a checkbox widget annotation dictionary.
fn build_checkbox_widget(
    field: &FormField,
    spec: &WidgetSpec,
    value: &str,
    page_height: f64,
) -> Dictionary {
    let x1 = field.x;
    let y2 = page_height - field.y;
    let y1 = y2 - spec.height;
    let x2 = x1 + spec.width;

    let rect = Object::Array(vec![
        Object::Real(x1 as f32),
        Object::Real(y1 as f32),
        Object::Real(x2 as f32),
        Object::Real(y2 as f32),
    ]);

    let is_checked = matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "x"
    );

    let (v_name, as_name) = if is_checked {
        (b"Yes".to_vec(), b"Yes".to_vec())
    } else {
        (b"Off".to_vec(), b"Off".to_vec())
    };

    dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Widget".to_vec()),
        "FT" => Object::Name(b"Btn".to_vec()),
        "T" => Object::String(field.key.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "V" => Object::Name(v_name),
        "AS" => Object::Name(as_name),
        "Rect" => rect,
        "F" => Object::Integer(4) // Print flag
    }
}
