# Multi-Form Rendering Pipeline

> **Status:** Data-driven. Any form can be added without code changes to `bir-print`.

## How to Add a New Form

1. **Create the form directory:**
   ```
   formtypes/{form_id}/
   ├── formtype.json    # Field layout (positions, dimensions, widget specs)
   ├── template.typ     # Typst macros (label, cells, mark, amount)
   └── pages/
       ├── page1.svg    # Background SVG for page 1
       ├── page2.svg    # Background SVG for page 2 (if applicable)
       └── ...
   ```

2. **Create a Draft struct** in `bir-core/src/forms/`:
   ```rust
   pub struct Form1601CDraft { ... }
   
   impl Form1601CDraft {
       pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
           // Map struct fields → formtype.json keys
       }
   }
   ```

3. **Render using `PrintRequest`:**
   ```rust
   use bir_print::{PrintRequest, render_flat_pdf, render_editable_pdf};
   
   let request = PrintRequest::new("1601Cv2018", draft.to_bir_field_map(), output_dir)
       .with_formtypes_dir("/path/to/formtypes");
   
   // Flat PDF (official-looking)
   let result = render_flat_pdf(request.clone())?;
   
   // Editable PDF (AcroForm fillable)
   let result = render_editable_pdf(request)?;
   ```

## Architecture

The rendering pipeline resolves form assets in this order:

1. **Filesystem** (`formtypes_dir/{form_id}/`) — checked first
2. **Embedded constants** — fallback for 2551Q only (compiled into the binary)

This means:
- **Development:** All forms are loaded from the `formtypes/` directory
- **Production:** 2551Q works even if the directory is missing (embedded fallback)
- **New forms:** Just add files to `formtypes/`, no code changes in `bir-print`

## File Reference

| File | Role |
|------|------|
| `crates/bir-print/src/lib.rs` | Rendering pipeline (flat, editable, preview) |
| `crates/bir-print/src/editable.rs` | AcroForm injection (fillable PDF) |
| `crates/bir-print/src/formtype.rs` | Schema types + display/capacity helpers |
| `formtypes/{form_id}/formtype.json` | Field layout definition |
| `formtypes/{form_id}/template.typ` | Typst rendering macros |
| `formtypes/{form_id}/pages/*.svg` | Background page images |

## Multi-Box Text Spanning

Fields with duplicate keys in `formtype.json` are treated as multi-box spans:
- **Text fields:** Word-aware splitting (breaks at last space before capacity)
- **Cells fields:** Strict character slicing (exact count)
- **Checkboxes:** Independent rendering (no spanning)
- **Amounts:** Full render in first box (no spanning)

Character capacity is computed from `widget.width` / font metrics, never from `cell_w` alone.
