# Discrete Official Artwork

Machine-readable symbols and government identity artwork are part of the exact
form revision. Treat them as verified source data, not decoration.

## Source hierarchy

1. Use the embedded image/XObject or vector content from the pinned official BIR
   PDF for the exact revision.
2. If the PDF does not expose a reusable discrete object, derive only the smallest
   reviewed crop needed from the official page raster and record the crop and
   derivation algorithm.
3. Download replacement artwork only when the exact official PDF lacks it. The
   source must be an official BIR/Philippine government origin, its hash must be
   pinned, and a reviewer must confirm that it is the same artwork used by that
   form revision.

Do not substitute a generic colored BIR logo for the monochrome government seal
printed on a form. Do not use a newer logo merely because it looks clearer.

## Barcode and QR workflow

For every physical page containing a barcode, PDF417 symbol, or QR code:

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
5. Render the human-readable caption separately as live text using a bundled,
   offline font whose measured metrics match the official font. Do not bake the
   caption into a low-resolution bitmap.
6. Add tests for payload, symbology, matrix hash, module dimensions, physical
   geometry, caption text/font/alignment, and a scan/decode of the rendered
   symbol at the reference DPI.

Never regenerate a symbol from an assumed caption alone. The decoded official
payload is authoritative, including spaces, punctuation, revision tokens, and
page numbers.

## Seal and logo workflow

1. Extract the exact embedded seal/wordmark object from the official PDF when
   possible. Record the object ID, page, native dimensions, source hash, and CTM.
2. Preserve the official monochrome appearance. A deterministic grayscale or
   black-and-white derivation is allowed only when it matches the printed source;
   record the algorithm and derived hash.
3. Preserve aspect ratio and transparency. Use a lossless native-resolution image
   or a reviewed vector reconstruction that passes the critical-region visual
   comparison. Never enlarge a tiny screenshot crop and call it high resolution.
4. Add a critical-region test for physical geometry and an asset-provenance test
   binding the renderer import to the recorded hashes.

## Promotion gate

A form cannot claim visual parity while any page uses an unverified barcode/QR
crop, an undecoded machine-readable payload, a bitmap caption, or an unproven
generic seal/logo substitution. Keep the form experimental and record the missing
artwork evidence explicitly.
