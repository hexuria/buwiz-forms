# Print Preview And Official Form PDF Plan

Status: superseded by the working prototype in
`scripts/pdf_to_typst_form_prototype.py`.

The better tested direction is:

1. Convert the official PDF page to SVG paths/images with PyMuPDF.
2. Use each SVG as the Typst page background.
3. Render only dynamic form values as Typst foreground content using typed helpers
   for checkboxes, character cells, and amount cells.

Do not rebuild all static labels, table lines, logos, and barcodes as native
Typst text/rectangles. A direct native Typst replay was tested and failed the
quality bar because text metrics drifted from the original PDF renderer.

Date: 2026-04-25

## Goal

Generate print previews and PDFs that visually match the official BIR form, while keeping the implementation scalable for many forms. The app should not generate a plain text summary PDF. It should render the actual BIR sheet and fill every supported field in the right place.

## Findings

- Current code path:
  - `crates/bir-desktop/src/views/form_2551q_view.rs` calls `write_2551q_pdf(&self.draft, PaperSize::A4, &path)`.
  - `crates/bir-print/src/lib.rs` builds a raw PDF from text lines in `render_2551q_pdf()`.
  - The raw generator has no official form background, no field geometry, no second page, and uses A4. This is why Preview shows a plain text document.
- The official 2551Q January 2018 ENCS PDF from BIR is a static two-page PDF.
  - Source: https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf
  - `MediaBox` is `[0 0 612 936]`, which is 8.5 x 13 inches at 72 points per inch. It is not A4.
  - It has no obvious AcroForm/XFA fields, so "fill PDF fields" is not the path. We need to stamp values on top of the static PDF.
- RMC 26-2018 confirms the revised 2551Q January 2018 ENCS form and says manual filers download the PDF format, print it, and fill applicable fields.
  - Source: https://bir-cdn.bir.gov.ph/local/pdf/RMC%2026-2018.pdf
- Official form surface is larger than the current draft model. Current `Form2551QDraft` covers basic identity, quarter, tax relief boolean, Schedule 1 rows, items 14/15/16/24, and filing status. Missing printable fields include sheets attached, income tax rate option, tax relief specify text, other credits/payment and specify text, penalties, overpayment choice, signatory/tax agent fields, and payment details.
- GPUI can display images through `img(source)`, and the current checked-out GPUI version accepts local paths, `ImageSource::Render`, and `Arc<Image>`. A GPUI print preview should display rendered PDF page images.
- Repo rule: this workspace is stable Rust. Do not switch to nightly.

## Decision

Use an official-template stamping architecture:

1. Store the official BIR PDF template as a versioned print asset.
2. Store field coordinates in a declarative layout manifest.
3. Convert each typed form draft into a canonical printable field map.
4. Use Rust PDF stamping to append text/checkmark content streams to the official PDF pages.
5. Render the final PDF to page images for GPUI preview.

Do not recreate the entire form with Typst, GPUI layout, `genpdf`, or custom line drawing. That path will drift from the official form and will become unmaintainable across many forms.

## Proposed Crate Boundaries

### `bir-core`

Owns tax data, validation, calculations, and conversion to canonical field values.

Add:

```rust
pub trait PrintableForm {
    fn form_id(&self) -> &'static str;
    fn printable_fields(&self) -> PrintFieldSet;
}

pub type PrintFieldSet = BTreeMap<String, PrintValue>;

pub enum PrintValue {
    Text(String),
    Bool(bool),
    MoneyCents(i64),
    PercentBasisPoints(i32),
}
```

For the first pass, `Form2551QDraft::printable_fields()` can format from existing `f64` values after `recompute()`. Long term, money should move to cents or a decimal type to remove float rounding risk.

### `bir-print`

Owns template loading, layout manifests, PDF stamping, and preview rasterization.

Suggested modules:

```text
crates/bir-print/src/
  lib.rs
  error.rs
  template.rs        # load template PDF and layout manifest
  layout.rs          # deserialize layout JSON, units, page coordinate conversion
  values.rs          # formatting text, money, percent, checkbox marks
  stamp.rs           # lopdf-based PDF overlay implementation
  preview.rs         # pdfium-render or platform fallback for PNG/RenderImage pages
  forms/
    mod.rs
    form_2551q.rs    # form-specific adapter only when generic config is not enough
```

Suggested assets:

```text
crates/bir-print/templates/
  2551Qv2018/
    source.txt
    base.pdf
    layout.json
    samples/
      zero.json
      filled.json
```

`source.txt` should contain the BIR URL, download date, SHA-256, page count, and media box.

### `bir-desktop`

Owns UI only.

Replace direct `write_2551q_pdf()` usage with:

```rust
let request = PrintRequest::from_form(&self.draft)?;
let document = bir_print::render_pdf(request)?;
let preview = bir_print::render_preview_pages(&document, PreviewDpi::Screen)?;
```

Add a `PrintPreviewView` GPUI entity that displays rendered pages through `img(path)` or `img(ImageSource::Render(...))`.

## Layout Manifest Shape

Use top-left coordinates in JSON because they are easier to calibrate against screenshots. Convert to PDF bottom-left coordinates inside `bir-print`.

Example:

```json
{
  "form_id": "2551Qv2018",
  "version": "Jan 2018 ENCS final rev 3",
  "page_size": { "width_pt": 612.0, "height_pt": 936.0 },
  "template_pdf": "base.pdf",
  "pages": [
    {
      "index": 0,
      "fields": [
        {
          "key": "frm2551Qv2018:txtTIN1",
          "kind": "cells",
          "x": 286.0,
          "y": 162.0,
          "cell_width": 17.0,
          "cell_count": 3,
          "font": "F2",
          "font_size": 10.0,
          "align": "center"
        },
        {
          "key": "frm2551Qv2018:amendedRtn_1",
          "kind": "checkbox",
          "x": 425.0,
          "y": 113.0,
          "font": "F3",
          "font_size": 13.0,
          "mark": "X"
        },
        {
          "key": "frm2551Qv2018:txt14",
          "kind": "amount_boxes",
          "x": 392.0,
          "y": 352.0,
          "integer_cell_count": 11,
          "decimal_cell_count": 2,
          "cell_width": 14.0,
          "decimal_x": 546.0,
          "font": "F2",
          "font_size": 9.5,
          "align": "center"
        }
      ]
    }
  ]
}
```

Field kinds to support for 2551Q and most other BIR forms:

- `text`: draw a single string inside a rectangle.
- `cells`: draw one character per box.
- `checkbox`: draw `X` only when the bool is true.
- `amount_boxes`: right-align money into integer and decimal boxes.
- `percent`: draw percent rate into rate boxes.
- `date_cells`: MM/DD/YYYY cells.
- `multiline_text`: clipped/wrapped text for address and validation areas.
- `static_overlay`: rare escape hatch for form-specific labels or generated values.

The manifest should also define masks for visual tests: rectangles where dynamic values are allowed to differ from the blank template.

## PDF Stamping Approach

Use `lopdf` for generation.

Reasoning:

- The official PDF is already the exact form. We only need to append content streams.
- `lopdf` loads and manipulates existing PDFs and exposes low-level content stream operations.
- The template already contains font resources such as Arial/Arial Bold on each page. Try to reuse the page font resource names (`F2`, `F3`, etc.) so filled text visually matches the form. If a future template lacks usable fonts, inject a standard Helvetica fallback.

Implementation sketch:

```rust
pub fn render_form_pdf(request: PrintRequest) -> Result<Vec<u8>, PrintError> {
    let template = TemplateRegistry::load(request.form_id)?;
    let mut pdf = lopdf::Document::load_mem(template.pdf_bytes)?;
    let pages = pdf.get_pages();

    for page_layout in &template.layout.pages {
        let page_id = pages[&(page_layout.index + 1)];
        let overlay = build_overlay_stream(&page_layout, &request.fields)?;
        append_page_content_stream(&mut pdf, page_id, overlay)?;
    }

    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(out)
}
```

Key details:

- Append streams instead of editing the original page content.
- Preserve the original template page size. For 2551Q this is 612 x 936.
- Keep values uppercase where the form requires uppercase.
- For cells, draw each character at calibrated cell centers. Do not draw a full string over boxed character fields.
- For amounts, format with exactly two decimals and place the decimal portion in the official decimal boxes.
- For checkboxes, draw `X`, not a checkmark.
- If a value overflows its field, return a `PrintError::FieldOverflow` with form id, field key, actual length, and max layout capacity.

## GPUI Print Preview

Add a dedicated preview view instead of opening Preview immediately.

View behavior:

- Generate the PDF from the current synced draft.
- Render each page to a PNG or `RenderImage`.
- Show pages in a scrollable GPUI view, centered on a neutral background.
- Toolbar: back, regenerate, zoom out, zoom in, fit width, open in Preview, save PDF, print.
- The existing `Print PDF` button can become `Preview / Print`.

Rendering options:

- Preferred: `pdfium-render` for page rasterization in Rust. It can render PDF pages to bitmaps, but it does not bundle Pdfium itself, so the app must package `libpdfium` or bind to a known local library.
- Short-term macOS fallback: write the PDF and open it through `open` as the current code does, while the GPUI preview is built behind a feature flag.
- Long-term: package Pdfium with the app and render page images directly into GPUI using `img(path)` or `ImageSource::Render`.

## 2551Q Data Model Changes

Add these fields before claiming the print output is complete:

```rust
pub enum CalendarBasis {
    Calendar,
    Fiscal,
}

pub enum IncomeTaxRateElection {
    Graduated,
    EightPercent,
}

pub enum OverpaymentDisposition {
    Refund,
    TaxCreditCertificate,
}

pub struct PaymentDetail {
    pub kind: PaymentKind,
    pub drawee_bank_agency: String,
    pub number: String,
    pub date: Option<NaiveDate>,
    pub amount_cents: i64,
}
```

Fields to add to `Form2551QDraft`:

- `calendar_basis`
- `year_ended_month`
- `sheets_attached`
- `tax_relief_specify`
- `income_tax_rate_election`
- `other_credit_specify`
- `other_credit_amount_cents`
- `surcharge_cents`
- `interest_cents`
- `compromise_cents`
- `overpayment_disposition`
- `payment_details: Vec<PaymentDetail>`
- optional signatory/tax agent fields

Computation changes:

- Item 18 = item 15 + item 16 + item 17.
- Item 19 = item 14 - item 18 and can represent overpayment before final display.
- Item 23 = item 20 + item 21 + item 22.
- Item 24 = item 19 + item 23.
- Keep current clamped payable for dashboard summaries if desired, but printable item 19/24 should preserve the form meaning.

## Multi-Form Scaling

Adding a form should be data-first:

1. Add official template PDF under `crates/bir-print/templates/<form_id>/base.pdf`.
2. Record source URL, download date, SHA-256, page count, and media box in `source.txt`.
3. Complete or add `schemas/<form_id>.json` with XML field names and logical field definitions.
4. Add a typed draft model only if calculations or UI workflows require it. Otherwise use generic `FormData`.
5. Implement `PrintableForm` adapter to produce canonical field values.
6. Add `layout.json` with page coordinates.
7. Add sample data JSON and golden render tests.
8. Register the form in a `PrintTemplateRegistry`.

This avoids hardcoding a new renderer per form. Form-specific Rust code should be limited to calculations and non-trivial mapping.

## Verification Plan

Automated checks:

- `cargo test -p bir-print`
- `cargo test -p bir-core forms::form_2551q`
- New `bir-print` tests:
  - template loads and SHA-256 matches `source.txt`.
  - output PDF has expected page count and media boxes.
  - all layout fields reference known canonical fields.
  - every required printable field is covered by layout or explicitly marked `not_printed`.
  - overflow checks reject long values.
  - visual diff compares rendered output to the official template with dynamic-value masks.

Manual QA for 2551Q:

- Use a fully filled sample profile, not all zero values.
- Render PDF.
- Open in macOS Preview and verify two pages.
- Page 1 should match the official form, with TIN/name/address/contact/email, options, tax computation, signatures, and payment details positioned correctly.
- Page 2 should show TIN/name, up to six Schedule 1 rows, rates, tax due, and total tax due.
- Print to PDF from Preview and verify scaling is not changed.

What I already verified in this repo:

- `cargo test -p bir-print` passes with the current implementation.
- `cargo test -p bir-core forms::form_2551q` passes with zero selected tests and no compile failure.
- Official 2551Q template downloaded locally and Quick Look rendered successfully for inspection.

## Implementation Order For Another Agent

1. Replace the current raw `build_simple_pdf` path with a new `render_form_pdf()` API in `bir-print`, keeping the old function temporarily behind a test or deleting it once the new renderer passes.
2. Add `lopdf` and template asset loading.
3. Add `2551Qv2018/base.pdf`, `source.txt`, and a first calibrated `layout.json` for headers, checkboxes, TIN, taxpayer name, RDO, address, ZIP, contact, email, items 14-24, and Schedule 1.
4. Add `PrintableForm` and `Form2551QDraft::printable_fields()`.
5. Extend `Form2551QDraft` for missing official fields and update UI sections only where those fields need user entry.
6. Add `PrintPreviewView` in `bir-desktop`.
7. Swap `Print PDF` to open the GPUI preview and offer save/print actions there.
8. Add visual regression tests and sample-filled PDFs.
9. Repeat the template/layout process for 1701Q, 1702Q, 1702-RT, 2550M, and 2550Q.

## Non-Goals For The First Pass

- Do not build an editable PDF AcroForm.
- Do not recreate the BIR template with GPUI drawing.
- Do not solve all future forms before 2551Q is correct.
- Do not rework filing/submission XML except where the same canonical field map can remove duplication.
