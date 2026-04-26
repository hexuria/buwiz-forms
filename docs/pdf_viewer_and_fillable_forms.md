# PDF Viewer and Fillable Form Architecture

## Overview
The eBIRForms integration uses a dual-mode engine to produce both **Flat** (archival, official-looking) and **Editable** (fillable, interactive) PDFs. This allows the system to generate documents that are pixel-perfect for official printing, while also providing interactive AcroForms for external use.

## Core Architecture

The architecture is built on three main pillars:
1. **Canonical Schema**: Externalized form definitions (`formtypes/`).
2. **Flat Renderer**: A Typst-based compiler for pixel-perfect SVGs and text overlays.
3. **Editable Injector**: A low-level PDF manipulation layer (`lopdf`) that injects AcroForm widgets.

### 1. The Canonical Source of Truth (`formtypes/`)
Form layouts and assets are decoupled from Rust code. They live in the `formtypes/` directory. For example, `formtypes/2551Qv2018/` contains:
- `formtype.json`: The layout schema defining X/Y coordinates, pagination, and interactive widget metadata for each form field.
- `metadata.json`: Provenance data, including the official BIR source URL and SHA-256 hash to ensure transparency.
- `pages/page*.svg`: The official BIR PDF pages converted into SVG backgrounds.
- `template.typ`: The Typst template used to render the flat version.

### 2. Dual-Mode Rendering Pipeline

#### Mode A: Flat PDF (`render_flat_pdf`)
- **Purpose**: Official archival and physical printing.
- **How it works**:
  1. Takes a mapped key-value dictionary of form data (`BTreeMap<String, String>`).
  2. Overlays the data onto the `page*.svg` backgrounds using precise coordinates from `formtype.json`.
  3. Compiles via Typst to produce a `.pdf`.
- **Result**: A flattened, immutable PDF.

#### Mode B: Editable PDF (`render_2551q_editable`)
- **Purpose**: Interactive digital forms for macOS Preview or Adobe Acrobat.
- **How it works**:
  1. Generates the Flat PDF first to serve as the visual base.
  2. Uses the `lopdf` crate to parse the binary PDF.
  3. Injects a global `/AcroForm` catalog dictionary.
  4. For every field in `formtype.json` that has a `widget` object, it injects a `/Widget` annotation into the corresponding page's `/Annots` array.
  5. Translates coordinates dynamically: Typst uses a top-left origin, but PDF specifications require a bottom-left origin (`y_pdf = page_height - y_typst - widget_height`).
- **Result**: A fillable PDF with standard interactive text fields (`/FT /Tx`) and checkboxes (`/FT /Btn`).

### 3. The `formtype.json` Schema
The schema gracefully handles both flat and editable forms. If a field lacks a `widget` block, it will only render as flat text.

```json
{
  "key": "frm2551Qv2018:txtTIN1",
  "kind": "cells",
  "page": 1,
  "x": 220.0,
  "y": 164.0,
  "cell_w": 14.1,
  "widget": {
    "type": "text",
    "width": 45.0,
    "height": 14.0,
    "max_length": 3,
    "comb": true,
    "font_size": 8.5
  }
}
```

### 4. Developer Calibration Tooling
New form definitions are accelerated by `scripts/generate_formtype.py`.
- **Workflow**: Provide an official BIR PDF URL. The script downloads it, extracts SVG page shells, and uses heuristic vector detection to find input boxes and checkboxes.
- **Output**: It generates a draft `formtype.json` (with dummy field keys like `_field_001`). Developers then calibrate this file manually by mapping the keys to the actual BIR XML identifiers.
- *Note: This is strictly a dev-time tool. The runtime application never invokes Python or OCR.*

## UI Integration (PDF Viewer)
The GPUI-based `PdfViewerView` was streamlined to focus on utility:
- **Icon-Only Toolbar**: Clean UI featuring Lucide-style SVG icons with tooltips.
- **Reveal**: Opens the generated assets directory directly in macOS Finder.
- **Export**: Triggers a save dialog. Currently defaults to exporting the Flat PDF for safety and compliance.
- **Print**: Uses the native macOS `lp` command to send the flat PDF directly to the system's default printer, bypassing the need for an external preview application.

---

## 🚀 Potential Improvements & Next Steps

If we want to continue improving this feature, here are the highest-impact tasks:

### 1. Export Choice UI (User Experience)
Currently, the "Export" button defaults to saving the Flat PDF. We should add a dropdown or context menu to the export button:
- **Export as Official Document (Flat)**
- **Export as Interactive Form (Fillable)**
This exposes the powerful dual-mode engine directly to the user.

### 2. Explicit Appearance Streams (Compatibility)
Currently, the AcroForm injector relies on setting `/NeedAppearances true` in the PDF catalog. This tells the PDF viewer (like macOS Preview or Acrobat) to automatically draw the text in the fields.
- **Improvement**: Generate explicit `/AP` (Appearance) streams for every widget using `lopdf`. This embeds the exact visual representation of the text inside the binary, ensuring 100% compatibility even in primitive PDF viewers (e.g., in-browser viewers like Chrome/Firefox which sometimes ignore `/NeedAppearances`).

### 3. AI-Assisted Bounding Box Mapping (Tooling)
The `generate_formtype.py` script currently detects boxes. We could improve it by using optical layout analysis to map the *text* next to the box to the box itself.
- **Improvement**: Have the script auto-guess the BIR field name (e.g., seeing "TIN" next to a box and suggesting `txtTIN1`). This would reduce human calibration time from hours to minutes for new tax forms.

### 4. Cross-Platform Printing (Core)
The native print button uses `lp`, which is macOS/Linux specific. 
- **Improvement**: If Windows support becomes a priority, we need to abstract the print command to support `Print-Item` (PowerShell) or `SumatraPDF -print-to-default`.
