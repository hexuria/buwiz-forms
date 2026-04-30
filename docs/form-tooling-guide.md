# Form Tooling Guide — Skills vs Scripts

> When to use the **form-generator skill**, when to use the **manual scripts**, and how they fit together.

---

## At a Glance

| | form-generator skill | generate_formtype.py | pdf_to_typst_form_prototype.py |
|---|---|---|---|
| **Location** | `.agent/skills/form-generator/` | `.scripts/generate_formtype.py` | `.scripts/pdf_to_typst_form_prototype.py` |
| **Input** | Savefile XML (filled form payload) | Official blank BIR PDF | PDF + savefile XML |
| **Output** | Rust code (`form_*.rs`, `_xml.rs`, `_view.rs`) | Layout data (`formtype.json`, SVGs) | Typst prototype document (standalone) |
| **Purpose** | Scaffold the data model + business logic | Scaffold the visual layout + coordinates | End-to-end proof-of-concept (legacy) |
| **Run by** | AI agent (reads SKILL.md) | Human via terminal | Human via terminal |
| **justfile recipe** | — | `just generate-form` | — |
| **Status** | ✅ Active | ✅ Active | 📦 Archived (prototype only) |

---

## Tool 1: form-generator skill

### What It Is

An **agent skill** — a structured set of instructions the AI coding assistant reads and follows. It automates the most tedious part of adding a new form: parsing all the BIR field IDs from a savefile and scaffolding the Rust code.

### When to Use

- You have a **savefile XML** from eBIRForms (the pseudo-XML payload with `<div>key=value</div>` entries)
- You need the **Rust code** for a new form: draft struct, XML field mapping, UI view

### What It Produces

| File | Contents |
|------|----------|
| `form_<id>.rs` | Draft struct, `Default`, `new_from_profile()`, `recompute()` skeleton, `FormValidator` impl, state transition methods |
| `form_<id>_xml.rs` | `to_bir_field_map()` — maps every Rust struct field → BIR field ID string |
| `form_<id>_view.rs` | GPUI desktop view scaffold with `FormViewTrait` implementation |

### How to Use

1. Place a savefile at a known path (e.g., `/savefile/779025068000-1601Cv2018-112024.xml`)
2. Ask the AI agent: _"Generate the form code for 1601C using the savefile at /savefile/..."_
3. The agent reads the skill instructions, runs the parser, and generates the Rust files
4. **You still need to manually**: add computation logic, wire routing, add DB migration

### What It Does NOT Do

- Does not generate visual layouts (no formtype.json, no SVGs)
- Does not produce a printable PDF
- Does not calibrate field positions

---

## Tool 2: generate_formtype.py

### What It Is

A **Python script** that converts an official blank BIR PDF into the layout data structure consumed by the PDF renderer at runtime.

### When to Use

- You have an **official BIR PDF** (blank form — from the eBIRForms install directory or BIR website)
- You need the **visual layout** for PDF rendering: SVG page backgrounds + field positions

### What It Produces

| File | Contents |
|------|----------|
| `formtype.json` | Field positions (x, y), widget specs (width, height, type), page assignments |
| `metadata.json` | Source URL, SHA-256 hash, page dimensions, provenance |
| `pages/page*.svg` | SVG renders of each PDF page (used as background images) |

### How to Use

```bash
# Shortcut via justfile:
just generate-form ~/Downloads/1601C.pdf 1601Cv2018 "Monthly Remittance Return"

# Full command:
python3 .scripts/generate_formtype.py \
  --input ~/Downloads/1601C.pdf \
  --form-id 1601Cv2018 \
  --title "Monthly Remittance Return" \
  --detect-fields
```

### What It Does NOT Do

- Does not generate Rust code (no draft struct, no XML mapping, no UI view)
- Does not know what the fields mean — produces placeholder keys like `_field_000`
- Requires manual renaming of field keys to match the real BIR identifiers from the savefile

---

## Tool 3: pdf_to_typst_form_prototype.py (Legacy)

### What It Is

The **original prototype** that proved the rendering concept. It generates a standalone Typst document from a PDF + savefile, with hardcoded field coordinates baked into the Python source.

### When to Use

- Generally **don't** — it's the historical proof-of-concept
- Useful as a reference for how the Typst rendering primitives (`put`, `label`, `cells`, `mark`, `amount`) work
- Can be used for quick one-off visual tests if you want to compile a PDF without touching Rust

### Why It's Superseded

The prototype has field positions hardcoded in Python. The production system has them in `formtype.json` (data-driven). The prototype was the stepping stone that proved the approach; `generate_formtype.py` + `bir-print` replaced it.

---

## How They Work Together

```
                 SAVEFILE XML                         OFFICIAL PDF
                 (from eBIRForms)                     (from eBIRForms)
                      │                                     │
         ┌────────────┴────────────┐           ┌────────────┴────────────┐
         │   form-generator skill  │           │  generate_formtype.py   │
         │   (agent-driven)        │           │  (human-driven)         │
         └────────────┬────────────┘           └────────────┬────────────┘
                      │                                     │
                      ▼                                     ▼
         ┌─────────────────────┐               ┌─────────────────────┐
         │ form_1601c.rs       │               │ formtype.json       │
         │ form_1601c_xml.rs   │               │ pages/page*.svg     │
         │ form_1601c_view.rs  │               │ metadata.json       │
         └─────────┬───────────┘               └─────────┬───────────┘
                   │                                     │
                   │        SHARED KEY: BIR field ID     │
                   │  e.g. "frm1601c:txtTIN1"            │
                   │                                     │
                   └──────────────┬──────────────────────┘
                                  │
                                  ▼
                        ┌─────────────────┐
                        │   bir-print     │
                        │   (Typst)       │
                        │                 │
                        │ field map       │
                        │   + layout      │
                        │   = final PDF   │
                        └─────────────────┘
```

The **BIR field ID string** (e.g., `frm1601c:txtTIN1`) is the contract between the two systems:
- The **form-generator skill** produces code that maps Rust struct fields → BIR field IDs
- The **generate_formtype.py** produces layout data that maps BIR field IDs → page coordinates
- The **bir-print crate** joins them: looks up each field ID in the layout, renders the value at the coordinate

---

## Decision Matrix

| I need to... | Use |
|---|---|
| Add a brand new form to the app | **Both** — skill for code, script for layout |
| Generate Rust structs from a savefile | **form-generator skill** |
| Create SVG backgrounds from an official PDF | **generate_formtype.py** |
| Calibrate field positions visually | **Layout Editor** (`cargo run --features layout-editor`) |
| Quick-test a PDF rendering idea | **pdf_to_typst_form_prototype.py** |
| Understand what fields a savefile contains | **form-generator skill** (parse step) |
| Verify our PDF matches the official one | **Print from both** and compare side-by-side |

---

## Full Workflow: Adding a New Form End-to-End

### Prerequisites
- [ ] Savefile XML from eBIRForms (at least 1, ideally 2+ with different data)
- [ ] Official blank PDF from `C:\Program Files\eBIRForms\BIRForms7.9.5\PDF\`

### Step 1: Generate Layout (human)
```bash
just generate-form ~/Downloads/<form>.pdf <FormID> "<Title>"
```

### Step 2: Generate Rust Code (agent)
Ask the AI: _"Generate form code for `<FormID>` using the savefile at `/savefile/...`"_

### Step 3: Rename Field Keys (human)
Open `formtypes/<FormID>/formtype.json` and rename `_field_000` keys to match the real BIR field IDs from the savefile (e.g., `frm1601c:txtTIN1`).

### Step 4: Calibrate Layout (human)
```bash
cargo run --features layout-editor
```
Select the form, nudge field positions until they align with the SVG background.

### Step 5: Wire Integration (agent or human)
- Register in `forms/mod.rs` + `registry.rs`
- Add DB migration + CRUD
- Add `include_str!` in `bir-print` for the new form assets
- Wire routing in `app.rs`

### Step 6: Verify
Compare our PDF output against a printout from the official eBIRForms.

---

## Typst-Native Forms (New Approach)

### What It Is

A **Typst-authored form template** (`form.typ`) that draws the **entire form** — static headers, lines, boxes, labels, AND dynamic field values. This replaces the SVG-background approach with a fully version-controlled, programmable, and maintainable text file.

### When to Use

- You want **full control** over the form layout without depending on an official PDF
- You want to **version-control** form changes with meaningful git diffs
- You want to **programmatically modify** form structure (loops, conditionals, computed fields)

### How It Works

```
formtypes/{form_id}/
├── form.typ              ← Typst draws the ENTIRE form (static + dynamic)
├── template.typ          ← Shared macros (put, label, cells, mark, amount)
├── formtype.json         ← Field coordinates (still used for AcroForm + Layout Editor)
├── form_structure.json   ← AI-readable structure (dev-time only, from extract_form_structure.py)
└── pages/*.svg           ← SVG backgrounds (kept as reference for side-by-side comparison)
```

**Rendering priority in `bir-print`:**
1. If `form.typ` exists → Typst-native path (form.typ draws everything)
2. Else → SVG-background path (existing behavior, no changes)

### Workflow

1. **Extract**: `just extract-form ~/Downloads/2551Q.pdf 2551Qv2018`
2. **Generate**: Ask AI to generate `form.typ` from `form_structure.json`
3. **Compare**: Use Layout Editor side-by-side mode (SVG vs Typst preview)
4. **Nudge**: Adjust coordinates in `form.typ` until pixel-perfect

---

## File Reference

| Path | Type | Purpose |
|------|------|---------|
| `.agent/skills/form-generator/SKILL.md` | Skill | Agent instructions for code generation |
| `.agent/skills/form-generator/scripts/parse_savefile.py` | Script | Parse savefile XML → field analysis |
| `.agent/skills/form-generator/scripts/generate_form.rs` | Script | Generate Rust code from analysis |
| `.scripts/generate_formtype.py` | Script | Generate layout from official PDF |
| `.scripts/extract_form_structure.py` | Script | Extract PDF structure for Typst-native form generation |
| `.scripts/pdf_to_typst_form_prototype.py` | Script | Legacy prototype (reference only) |
| `formtypes/<FormID>/form.typ` | Template | Typst-native form (draws entire form) |
| `formtypes/<FormID>/formtype.json` | Data | Field positions for PDF rendering |
| `formtypes/<FormID>/form_structure.json` | Data | AI-readable PDF structure (dev-time) |
| `formtypes/<FormID>/pages/*.svg` | Asset | SVG page backgrounds |
| `formtypes/<FormID>/template.typ` | Template | Typst rendering helpers |
| `docs/adding-a-new-form.md` | Docs | Full developer guide with code samples |
