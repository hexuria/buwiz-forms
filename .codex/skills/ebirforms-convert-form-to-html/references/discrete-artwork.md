# Discrete Official Artwork

Machine-readable symbols and government identity artwork are part of the exact
form revision. Treat them as verified source data, not decoration.

## Source rule

Use only the embedded image/XObject or vector content from the pinned official
BIR PDF for the exact revision. A native raster XObject is valid source artwork;
a crop from a rendered full-page reference is not. Preserve the object's native
dimensions, decoded pixels or vector geometry, color space, aspect ratio, and
placement CTM.

Do not download a replacement, substitute a generic or newer logo, or derive
runtime artwork from a screenshot/reference-page crop. Do not threshold,
resample, resize, recolor, sharpen, or otherwise enhance an embedded raster. A
lossless container conversion is allowed only when decoded pixels, dimensions,
and color space remain identical and both source and output hashes are recorded.
If an exact embedded object cannot be proven, keep the form experimental and
record the missing evidence.

## Barcode and QR workflow

Inventory every physical page first. For every page containing a barcode,
PDF417 symbol, or QR code:

1. Extract the embedded source object losslessly and record the PDF object ID,
   source page, pixel dimensions, source hash, and placement CTM in points.
2. Decode the symbol. Prefer two independent decoders when the symbology permits
   it. Record the exact payload and symbology.
3. Recover and hash the logical module matrix. Prove that the checked-in matrix
   has zero module differences from the official source.
4. Render the matrix as inline SVG or an equivalent deterministic vector with
   integer logical dimensions, `shape-rendering: crispEdges`, and no smoothing.
   Use the official page-specific physical width, height, and position; do not
   stretch one page's geometry across all pages.
5. Render the human-readable caption and adjacent static text separately as live
   text using a bundled, offline font whose measured metrics match the official
   font. Do not bake caption/static text into a bitmap.
6. Add tests for payload, symbology, matrix hash, module dimensions, physical
   geometry, caption text/font/alignment, and a scan/decode of the rendered
   symbol at the reference DPI.

Never regenerate a symbol from an assumed caption alone. The decoded official
payload is authoritative, including spaces, punctuation, revision tokens, and
page numbers.

### Explicit no-symbol result

Some official forms, including `0605:1999`, contain no machine-readable symbol.
Record that result explicitly in the form's reference evidence:

```json
{
  "machine_readable_artwork": {
    "status": "absent_in_official_pdf",
    "official_source_sha256": "<same pinned PDF hash>",
    "audited_pages": [1, 2],
    "inventory_method": "PDF object and page-content inventory",
    "object_inventory_sha256": "<deterministic inventory hash>"
  }
}
```

Add a test that the renderer emits no barcode, PDF417, or QR element. Never
fabricate a symbol or caption from the form code, revision, filename, or nearby
text. An empty asset list without the audited absence record is incomplete
evidence, not proof that the form has no code.

## Seal and logo workflow

1. Extract the exact embedded seal/wordmark image/XObject or vector object from
   the official PDF. Record the object ID, page, native dimensions, source stream
   and decoded-content hashes, color space, and CTM.
2. Preserve the official monochrome or grayscale appearance exactly. Do not
   threshold, recolor, sharpen, resample, resize, or replace it with a clearer
   generic/downloaded logo.
3. Preserve aspect ratio and transparency. Use the native lossless raster or the
   exact embedded vector geometry; never use a crop from a rendered page.
4. Add a critical-region test for physical geometry and an asset-provenance test
   binding the renderer import to the recorded hashes.

## Promotion gate

A form cannot claim visual parity while any page uses a rendered-page artwork
crop, an undecoded machine-readable payload, a nonzero/unproven module matrix,
a bitmap caption, a transformed seal/logo, a generic/downloaded substitution, or
an implicit no-symbol assumption. Keep the form experimental and record the
missing artwork evidence explicitly.
