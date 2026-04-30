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
    let mut doc = Document::load(base_pdf_path)
        .map_err(|e| PrintError::Io(std::io::Error::other(format!("lopdf load failed: {e}"))))?;

    // Collect page object IDs in page order.
    let page_ids = doc.get_pages();
    let mut sorted_pages: Vec<(u32, ObjectId)> = page_ids.into_iter().collect();
    sorted_pages.sort_by_key(|(num, _)| *num);

    let page_height = formtype.page_height;

    // Build widgets and track which page they belong to.
    let mut all_field_refs: Vec<Object> = Vec::new();
    let mut page_annots: BTreeMap<u32, Vec<Object>> = BTreeMap::new();

    // Track key occurrences for suffix generation and remaining text for spanning.
    let mut key_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut remaining_text: BTreeMap<String, String> = BTreeMap::new();

    for field_def in &formtype.fields {
        let Some(widget_spec) = &field_def.widget else {
            continue;
        };

        // Initialize remaining text for this key on first encounter
        if !remaining_text.contains_key(&field_def.key) {
            let full_value = fields.get(&field_def.key).cloned().unwrap_or_default();
            remaining_text.insert(field_def.key.clone(), full_value);
        }

        let current_value = remaining_text
            .get(&field_def.key)
            .cloned()
            .unwrap_or_default();

        // Skip empty optional fields
        if current_value.is_empty() && field_def.optional {
            // Still increment the count so suffix numbering stays consistent
            *key_count.entry(field_def.key.clone()).or_insert(0) += 1;
            continue;
        }

        // Determine the chunk of text for this box
        let chunk = if let Some(capacity) = field_def.char_capacity() {
            if capacity >= current_value.len() {
                current_value.clone()
            } else {
                current_value.chars().take(capacity).collect::<String>()
            }
        } else {
            current_value.clone()
        };

        // Generate a unique AcroForm name: append suffix if key is reused
        let occurrence = key_count.entry(field_def.key.clone()).or_insert(0);
        let acroform_name = if *occurrence == 0 {
            // Check if this key appears again later — if so, suffix even the first
            let total = formtype
                .fields
                .iter()
                .filter(|f| f.key == field_def.key)
                .count();
            if total > 1 {
                format!("{}_{}", field_def.key, *occurrence)
            } else {
                field_def.key.clone()
            }
        } else {
            format!("{}_{}", field_def.key, *occurrence)
        };
        *occurrence += 1;

        // Consume text for spanning
        let consumed = chunk.len();
        if consumed > 0 {
            let rest = if consumed >= current_value.len() {
                String::new()
            } else {
                current_value[consumed..].to_string()
            };
            remaining_text.insert(field_def.key.clone(), rest);
        }

        let widget_dict = match widget_spec.widget_type {
            WidgetType::Text => {
                build_text_widget_named(field_def, widget_spec, &chunk, page_height, &acroform_name)
            }
            WidgetType::Checkbox => {
                build_checkbox_widget(field_def, widget_spec, &chunk, page_height)
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
    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        let root_id = *root_id;
        if let Ok(Object::Dictionary(ref mut d)) = doc.get_object_mut(root_id) {
            d.set("AcroForm", Object::Reference(acroform_id));
        }
    }

    // Add /Annots to each page that has widgets.
    for (page_num, annot_refs) in &page_annots {
        if let Some((_, page_oid)) = sorted_pages.iter().find(|(n, _)| *n == *page_num) {
            if let Ok(Object::Dictionary(ref mut page_dict)) = doc.get_object_mut(*page_oid) {
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

    doc.save(output_path)
        .map_err(|e| PrintError::Io(std::io::Error::other(format!("lopdf save failed: {e}"))))?;

    Ok(())
}

/// Build a text widget annotation dictionary.
#[allow(dead_code)]
fn build_text_widget(
    field: &FormField,
    spec: &WidgetSpec,
    value: &str,
    page_height: f64,
) -> Dictionary {
    build_text_widget_named(field, spec, value, page_height, &field.key)
}

/// Build a text widget annotation dictionary with a custom AcroForm name.
///
/// This variant is used when multiple boxes share the same field key.
/// Each gets a unique `/T` name (e.g., `txtAddress_0`, `txtAddress_1`)
/// to prevent AcroForm field mirroring in PDF viewers.
fn build_text_widget_named(
    field: &FormField,
    spec: &WidgetSpec,
    value: &str,
    page_height: f64,
    acroform_name: &str,
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
    if spec.comb.unwrap_or(false) {
        ff |= 1 << 24; // Comb
    }

    let mut dict = dictionary! {
        "Type" => Object::Name(b"Annot".to_vec()),
        "Subtype" => Object::Name(b"Widget".to_vec()),
        "FT" => Object::Name(b"Tx".to_vec()),
        "T" => Object::String(acroform_name.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "V" => Object::String(value.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        "DA" => Object::String(da.into_bytes(), lopdf::StringFormat::Literal),
        "Rect" => rect,
        "F" => Object::Integer(4)
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
