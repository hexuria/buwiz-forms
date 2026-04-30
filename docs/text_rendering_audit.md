# PDF Text Rendering Audit: Multi-Box Fields

## Executive Summary
This document consolidates the findings of multiple architectural audits regarding the implementation of multi-box text rendering in the `bir-print` engine. Currently, the system uses a 1:1 mapping between `FormField` and PDF text generation via Typst and AcroForm. Allowing duplicate keys for multi-box layout breaks two core assumptions:
1. `layout_keys_are_unique` test will fail.
2. The current `generate_typst` loops over `FormField` and fetches `fields.get(&field.key)`. If multiple boxes share a key, the exact same full text will be rendered overlapping in every box, rather than flowing sequentially.

## The Critics' Audit

### 1. Typst Layout Critic
**Observation:** Typst's absolute positioning macro (`label()` or `cells()`) isolates each string. Typst does not natively support "linked text frames" (like Adobe InDesign) where text auto-flows from one absolute coordinate to another.
**Verdict:** Text splitting *must* occur in the Rust layer (`bir-print`) before generating the Typst markup.

### 2. AcroForm (Editable PDF) Critic
**Observation:** In an editable AcroForm PDF, if you create multiple text widgets with the *same name* (e.g. `txtAddress`), Adobe Acrobat treats them as mirrors of each other (typing in one mirrors the text to the other). 
**Verdict:** For AcroForms, we cannot use the same field name for split text unless we want mirroring. If we want text flowing, they must be distinct fields (e.g., `txtAddress_1`, `txtAddress_2`) or AcroForm generation must be disabled for multi-box text flow, relying purely on the flat PDF generation. *Recommendation:* AcroForms rarely support auto-flowing text fields well. We should split the text in Rust and assign them to `txtAddress_1`, `txtAddress_2` in the AcroForm dict, OR just restrict multi-box flowing strictly to the Flat PDF rendering mode.

### 3. Font & Capacity Critic
**Observation:** To split text accurately in Rust, the engine needs to know *exactly* how many characters fit in Box 1 before spilling to Box 2.
**Verdict:**
- For `FieldKind::Cells`: Capacity is exactly `floor(field.cell_w) / cell_width` or determined directly by the visual bounding box width divided by `cell_w`.
- For `FieldKind::Text`: Because we use a monospace font (Courier/Helvetica equivalent), the width of a character is deterministic based on `field.size`. `capacity = floor(box_width / (font_size * approx_0.6))`.

---

## The Consolidated Implementation Plan

### Phase 1: Removing Uniqueness Constraints
1. **Update Unit Tests:** Modify `layout_keys_are_unique` in `crates/bir-print/src/lib.rs` to allow duplicates. Instead of asserting strict uniqueness, we assert that duplicates are valid and ordered.

### Phase 2: The Text Spanning Engine (Rust)
1. **Grouping Logic:** In `generate_typst`, do not iterate `formtype.fields` flatly. Instead, group `formtype.fields` by `key` into a `BTreeMap<String, Vec<&FormField>>`. Ensure the array order is preserved.
2. **Text Chunking Algorithm:**
   For a given `key` and its user-provided string `value`:
   - Iterate through the ordered `Vec<&FormField>` for that key.
   - For each box, calculate its capacity:
     - *Cells:* `capacity = (box_width / cell_w).floor() as usize`
     - *Text:* `capacity = (box_width / (font_size * 0.6)).floor() as usize` (Tweak 0.6 based on Typst font metrics).
   - Slice the `value` string up to `capacity`.
   - Take the chunk, pass it to `render_field` (or a modified version of it), and advance the string pointer.
   - If the string runs out before the boxes do, render the remaining boxes as empty.
   - If the boxes run out before the string does, truncate or log a warning (text overflow).

### Phase 3: AcroForm Adjustments (`crates/bir-print/src/editable.rs`)
1. During AcroForm widget injection, if a key has multiple boxes, append a suffix to the internal AcroForm name (e.g., `T (txtAddress_0)` and `T (txtAddress_1)`).
2. Populate the default `/V` (value) of these widgets using the same Chunking Algorithm developed in Phase 2, so the editable PDF matches the flat PDF.

## Next Steps for Execution
1. Implement the `Text Spanning Engine` function in `crates/bir-print/src/lib.rs`.
2. Refactor `generate_typst` to use this chunker.
3. Fix the `layout_keys_are_unique` test.
4. Test with a long string on a multi-box field in the Visual Editor.
