# 🛠️ Developer Scripts

This folder contains utilities for verifying the eBIRForms reverse-engineering logic and maintaining form type definitions.

---

## 📄 generate_formtype.py

Generates a `formtypes/<form_id>/` directory from an official BIR PDF — the canonical structure used by the Rust runtime for both flat (Typst) and editable (AcroForm) PDF rendering.

### Prerequisites

```bash
python3 -m pip install --user pymupdf
```

### Usage

```bash
# From a URL (downloads automatically):
python3 scripts/generate_formtype.py \
  --input "https://bir-cdn.bir.gov.ph/..." \
  --form-id 2551Qv2018 \
  --title "Quarterly Percentage Tax Return"

# From a local PDF:
python3 scripts/generate_formtype.py \
  --input official.pdf \
  --form-id 2551Qv2018

# With automatic field rectangle detection:
python3 scripts/generate_formtype.py \
  --input official.pdf \
  --form-id 2551Qv2018 \
  --detect-fields

# Overwrite an existing formtype directory:
python3 scripts/generate_formtype.py \
  --input official.pdf \
  --form-id 2551Qv2018 \
  --overwrite
```

### Output

```
formtypes/<form_id>/
├── formtype.json    ← Field layout + widget specs (DRAFT — review before committing)
├── metadata.json    ← Source URL, SHA-256, page dimensions
└── pages/
    ├── page1.svg    ← Official page shell (SVG)
    └── page2.svg
```

### Workflow

1. **Generate** — Run the script against an official BIR PDF
2. **Calibrate** — Review `formtype.json`, rename placeholder field keys to BIR identifiers
3. **Add widgets** — Add `"widget"` specifications for fields that should be editable
4. **Commit** — Check in the calibrated `formtype.json`
5. **Verify** — `cargo test -p bir-print` validates loading and rendering

> ⚠️ This script is **dev-only**. The Rust runtime loads committed `formtype.json` files via `include_str!()` and never invokes Python.

---

## 📄 pdf_to_typst_form_prototype.py

The original prototype script that proved the SVG-background + Typst-foreground rendering pipeline. It converts an official BIR PDF into a Typst document with hardcoded field coordinates.

### Prerequisites

```bash
python3 -m pip install --user pymupdf
cargo install typst-cli --version 0.13.1 --locked
```

### Usage

```bash
python3 scripts/pdf_to_typst_form_prototype.py \
  --pdf "https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf" \
  --xml ../bir-analyze/fixed.xml \
  --out /tmp/bir-typst-2551q \
  --compile
```

---

## 📄 verify_submission.sh

The **Gold Standard** verification script. If this script works and you receive an email, it proves that the following are correctly implemented:
1.  **Parser:** Reading the non-standard XML format.
2.  **Encryption:** The replicated AES-128 / DCPcrypt2 logic.
3.  **Transport:** The FTP connection and directory structure requirements.

### Usage
```bash
chmod +x verify_submission.sh
./verify_submission.sh
```

### How it works
1.  Reads a raw `savefile/` XML.
2.  Converts it to structured JSON.
3.  Updates the `txtDateIssue` field to the current second (to avoid duplicate data rejection).
4.  Encrypts and Compresses the modified data into a new `.xml` payload.
5.  Uploads it to the BIR FTP using a unique filename containing a `+timestamp` suffix.
