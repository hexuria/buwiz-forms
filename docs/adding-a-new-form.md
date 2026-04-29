# Building BIR Tax Forms — Developer Guide

> Comprehensive reference for implementing, validating, rendering, and submitting tax forms in the eBIRForms Rust workspace.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Phase 1 — Core Data Model (`bir-core`)](#phase-1--core-data-model-bir-core)
3. [Phase 2 — Database Persistence (`bir-core/src/db`)](#phase-2--database-persistence-bir-coresrcdb)
4. [Phase 3 — Desktop UI View (`bir-desktop`)](#phase-3--desktop-ui-view-bir-desktop)
5. [Phase 4 — XML Export & PDF Printing](#phase-4--xml-export--pdf-printing)
6. [Component Reference — `form_parts.rs`](#component-reference--form_partsrs)
7. [Component Reference — `form_engine.rs`](#component-reference--form_enginers)
8. [Walkthrough — How `Form2551QView` Uses the Engine](#walkthrough--how-form2551qview-uses-the-engine)
9. [Checklist — Adding a New Form from Scratch](#checklist--adding-a-new-form-from-scratch)

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        bir-desktop (UI)                          │
│                                                                  │
│  ┌────────────────────┐   ┌──────────────────────────────────┐  │
│  │  form_engine.rs    │   │  form_parts.rs                   │  │
│  │  ─────────────────  │   │  ─────────────────────────────── │  │
│  │  FormViewTrait      │   │  field_label()                   │  │
│  │  render_header()    │   │  readonly_field()                │  │
│  │  render_status_pipe │   │  currency_display()              │  │
│  └────────────────────┘   │  taxpayer_info_section()          │  │
│           ▲                │  form_accordion()                 │  │
│           │                │  computation_row_readonly()       │  │
│  ┌────────┴───────────┐   │  computation_row_input()          │  │
│  │ form_2551q_view.rs │   │  atc_schedule_table()             │  │
│  │ form_1701q_view.rs │   │  penalty_summary_section()        │  │
│  │ form_XXXX_view.rs  │   └──────────────────────────────────┘  │
│  └────────────────────┘                                          │
├──────────────────────────────────────────────────────────────────┤
│                        bir-core (Domain)                         │
│                                                                  │
│  forms/                                                          │
│  ├── mod.rs            FormValidator trait, re-exports           │
│  ├── form_2551q.rs     Form2551QDraft, Schedule1Row, recompute() │
│  ├── form_1701q.rs     Form1701QDraft, FormValidator impl       │
│  ├── form_2551q_xml.rs XML field mapping for submission         │
│  ├── atc.rs            ATC tax code lookup tables               │
│  └── registry.rs       FORM_REGISTRY, FormDefinition            │
│                                                                  │
│  db/                                                             │
│  ├── mod.rs            Database struct, open/close, schema       │
│  ├── drafts.rs         save_2551q_draft(), get_2551q_draft()     │
│  ├── profiles.rs       save_profile(), list_profiles()           │
│  └── ...                                                         │
└──────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Configuration over implementation** — New forms are primarily data (field names, ATC codes, tax rates). The UI rendering engine (`form_parts`) handles presentation.
2. **FormValidator as the single validation interface** — All business rules live in `bir-core`, never in the UI layer.
3. **FormViewTrait for shared chrome** — The status pipeline, header, and action bar are rendered identically for every form via trait default methods.
4. **Accordion-based sections** — Every form uses `form_accordion()` to wrap collapsible sections with validation indicators.

---

## Phase 1 — Core Data Model (`bir-core`)

### Step 1.1 — Create the Draft Struct

Create `crates/bir-core/src/forms/form_<number>.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::forms::FormValidator;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Form2550MDraft {
    // ── Identity (pre-filled from TaxpayerProfile) ──
    pub id: Option<i64>,
    pub tin: String,
    pub rdo_code: String,
    pub taxpayer_name: String,
    pub registered_address: String,
    pub zip_code: String,
    pub contact_number: String,
    pub email: String,

    // ── Filing Period ──
    pub taxable_year: u16,
    pub month: u8,          // form-specific: month instead of quarter

    // ── Schedules (form-specific) ──
    pub schedule_1: Vec<Schedule1Row>,

    // ── Computed Summaries ──
    pub total_tax_due: f64,
    pub total_amount_payable: f64,

    // ── Penalties (auto-computed) ──
    pub surcharge: f64,
    pub interest: f64,
    pub compromise: f64,
    pub total_penalties: f64,

    // ── Lifecycle ──
    pub status: super::FilingStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub submitted_at: Option<String>,
    pub confirmed_at: Option<String>,
}
```

> **Convention:** The `status` field MUST use `super::FilingStatus` (the shared enum from `forms/mod.rs`). This is required for `FormViewTrait` interoperability.

### Step 1.2 — Add a Profile Constructor

```rust
impl Form2550MDraft {
    pub fn new_from_profile(
        profile: &crate::profile::TaxpayerProfile,
        year: u16,
        month: u8,
    ) -> Self {
        Self {
            tin: profile.tin.full(),
            rdo_code: profile.rdo_code.clone(),
            taxpayer_name: profile.full_name.clone(),
            registered_address: profile.registered_address.clone(),
            zip_code: profile.zip_code.clone(),
            contact_number: profile.phone.clone(),
            email: profile.email.clone(),
            taxable_year: year,
            month,
            ..Default::default()
        }
    }
}
```

### Step 1.3 — Implement `FormValidator`

```rust
impl FormValidator for Form2550MDraft {
    fn validate(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();

        if self.tin.trim().is_empty() {
            errors.push(("tin".into(), "TIN is required".into()));
        }
        if self.rdo_code.trim().is_empty() {
            errors.push(("rdo_code".into(), "RDO Code is required".into()));
        }
        if self.taxpayer_name.trim().is_empty() {
            errors.push(("taxpayer_name".into(), "Name is required".into()));
        }
        // ... form-specific validations ...

        errors
    }
}
```

> **The (field_id, message) convention:** The `field_id` string maps directly to the accordion section error detection in the UI via `has_section_error()`. Use consistent naming.

### Step 1.4 — Add a `recompute()` Method (if applicable)

For forms with auto-calculated fields (tax due = amount × rate):

```rust
impl Form2550MDraft {
    pub fn recompute(&mut self) {
        for row in &mut self.schedule_1 {
            row.tax_due = row.taxable_amount * row.tax_rate;
        }
        self.total_tax_due = self.schedule_1.iter().map(|r| r.tax_due).sum();
        self.total_amount_payable = self.total_tax_due + self.total_penalties;
    }
}
```

### Step 1.5 — Add State Transition Methods

**IMPORTANT:** Never assign `self.status = FilingStatus::X` directly. Use transition methods with precondition checks:

```rust
impl Form2550MDraft {
    pub fn is_editable(&self) -> bool {
        matches!(self.status, FilingStatus::Draft)
    }

    /// Draft → Queued (validates first)
    pub fn transition_to_queued(&mut self) -> Result<(), Vec<(String, String)>> {
        assert!(matches!(self.status, FilingStatus::Draft));
        let errors = <Self as FormValidator>::validate(self);
        if !errors.is_empty() { return Err(errors); }
        self.status = FilingStatus::Queued;
        self.submission_attempts = 0;
        self.next_retry_at = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    /// Queued → Submitted (after FTP upload)
    pub fn transition_to_submitted(&mut self, filename: String) { /* ... */ }

    /// Submitted → Confirmed (when email receipt matched)
    pub fn transition_to_confirmed(&mut self, confirmed_at: String, receipt_id: Option<i64>, filename: Option<String>) { /* ... */ }

    /// Confirmed → Paid (user action)
    pub fn transition_to_paid(&mut self) { /* ... */ }

    /// Any non-Paid → Draft (revert, clears metadata)
    pub fn revert_to_draft(&mut self) { /* ... */ }

    /// Track failed submission with exponential backoff (auto-reverts after 5 failures)
    pub fn record_submission_failure(&mut self, error_msg: String) { /* ... */ }
}
```

> **Reference:** See `form_2551q.rs` lines 293–408 for the complete implementation of all transition methods.

### Step 1.6 — Register in `forms/mod.rs`

```rust
pub mod form_2550m;
pub use form_2550m::Form2550MDraft;
```

### Step 1.7 — Register in `forms/registry.rs`

Add your form to `FORM_REGISTRY` so it appears in the Dashboard's form selection dropdown:

```rust
FormDefinition {
    form_code: "2550M",
    form_title: "Monthly VAT Declaration",
    frequency: FilingFrequency::Monthly,
    // ...
},
```

---

## Phase 2 — Database Persistence (`bir-core/src/db`)

### Step 2.1 — Add a Schema Migration

In `crates/bir-core/src/db/migrations.rs`, add a new versioned migration:

```rust
(N, "CREATE TABLE IF NOT EXISTS form_2550m_drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tin TEXT NOT NULL,
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    json_data TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(tin, year, month)
)"),
```

### Step 2.2 — Add CRUD Methods

In `crates/bir-core/src/db/drafts.rs` (or a new file):

```rust
impl Database {
    pub fn save_2550m_draft(&self, draft: &Form2550MDraft) -> Result<i64> {
        let json = serde_json::to_string(draft)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO form_2550m_drafts (tin, year, month, json_data, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![draft.tin, draft.taxable_year, draft.month, json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_2550m_draft(&self, tin: &str, year: u16, month: u8) -> Result<Option<Form2550MDraft>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT json_data FROM form_2550m_drafts WHERE tin = ?1 AND year = ?2 AND month = ?3"
        )?;
        // ... deserialize from JSON ...
    }
}
```

---

## Phase 3 — Desktop UI View (`bir-desktop`)

### Step 3.1 — Create the View Struct

Create `crates/bir-desktop/src/views/form_2550m_view.rs`:

```rust
use bir_core::forms::Form2550MDraft;
use bir_core::forms::FilingStatus;
use gpui::*;
use gpui_component::*;
use crate::components::form_engine::FormViewTrait;
use crate::components::form_parts::{taxpayer_info_section, TaxpayerInfoProps};

pub enum Form2550MEvent {
    BackToDashboard,
}

pub struct Form2550MView {
    pub draft: Form2550MDraft,
    pub show_filing_period: bool,
    pub show_background_info: bool,
    pub show_schedule_1: bool,
    pub show_tax_computation: bool,
}

impl EventEmitter<Form2550MEvent> for Form2550MView {}

impl Form2550MView {
    pub fn new(draft: Form2550MDraft) -> Self {
        Self {
            draft,
            show_filing_period: true,
            show_background_info: true,
            show_schedule_1: true,
            show_tax_computation: true,
        }
    }
}
```

### Step 3.2 — Implement `FormViewTrait`

This gives your form the standardized header and status pipeline for free:

```rust
impl FormViewTrait for Form2550MView {
    fn form_title(&self) -> &'static str { "BIR Form No. 2550M" }
    fn form_subtitle(&self) -> &'static str { "Monthly VAT Declaration" }
    fn form_version(&self) -> &'static str { "January 2018 (ENCS)" }
    fn current_status(&self) -> FilingStatus { self.draft.status.clone() }
    fn submitted_at(&self) -> Option<&str> { self.draft.submitted_at.as_deref() }
    fn confirmed_at(&self) -> Option<&str> { self.draft.confirmed_at.as_deref() }

    fn save_draft(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // Persist to database
    }
    fn mark_submitted(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { /* ... */ }
    fn mark_paid(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { /* ... */ }
    fn revert_to_draft(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { /* ... */ }
    fn preview_pdf(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { /* ... */ }
    fn print_confirmation(&mut self, _window: &mut Window, _cx: &mut Context<Self>) { /* ... */ }
}
```

### Step 3.3 — Implement `Render` Using Form Parts

This is the core of the form-building workflow. Use the reusable components:

```rust
impl Render for Form2550MView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── Header (from FormViewTrait) ──
        let title_block = <Self as FormViewTrait>::render_header(self, cx);
        let status_pipeline = <Self as FormViewTrait>::render_status_pipeline(self, cx);

        // ── Section: Background Info (reusable) ──
        let background_info = taxpayer_info_section(
            TaxpayerInfoProps {
                tin: &self.draft.tin,       tin_err: None,
                rdo: &self.draft.rdo_code,  rdo_err: None,
                name: &self.draft.taxpayer_name, name_err: None,
                address: &self.draft.registered_address, address_err: None,
                zip: &self.draft.zip_code,  zip_err: None,
                contact: &self.draft.contact_number, contact_err: None,
                email: &self.draft.email,   email_err: None,
            },
            cx,
        );

        // ── Section: Tax Computation (reusable) ──
        let computation = div()
            .flex().flex_col().gap_4()
            .child(crate::components::form_parts::computation_row_readonly(
                "Total Tax Due", self.draft.total_tax_due, false, cx,
            ))
            .child(
                div().pt_4().border_t_1().border_color(cx.theme().border)
                    .child(crate::components::form_parts::computation_row_readonly(
                        "Total Amount Payable", self.draft.total_amount_payable, true, cx,
                    ))
            );

        // ── Assemble with Accordions ──
        div().size_full().flex().flex_col().bg(cx.theme().background)
            .child(title_block)
            .child(/* status banner wrapper */)
            .child(
                div().id("scroll").flex_1().overflow_y_scroll().p_8()
                    .child(
                        div().w_full().max_w(px(900.)).mx_auto()
                            .flex().flex_col().gap_8()
                            .child(crate::components::form_parts::form_accordion(
                                "acc_background",
                                "PART I — BACKGROUND INFORMATION",
                                self.show_background_info,
                                true,  // is_valid
                                false, // has_error
                                cx.listener(|this: &mut Self, _, _, cx| {
                                    this.show_background_info = !this.show_background_info;
                                    cx.notify();
                                }),
                                background_info.into_any_element(),
                                cx,
                            ))
                            .child(crate::components::form_parts::form_accordion(
                                "acc_computation",
                                "PART II — COMPUTATION OF TAX",
                                self.show_tax_computation,
                                true,
                                false,
                                cx.listener(|this: &mut Self, _, _, cx| {
                                    this.show_tax_computation = !this.show_tax_computation;
                                    cx.notify();
                                }),
                                computation.into_any_element(),
                                cx,
                            ))
                    )
            )
    }
}
```

### Step 3.4 — Register the View

1. Add `pub mod form_2550m_view;` to `views/mod.rs`
2. Wire the view instantiation in `app.rs` routing logic

---

## Phase 4 — XML Export & PDF Printing

### Step 4.1 — XML Field Mapping

Create `crates/bir-core/src/forms/form_2550m_xml.rs`:

```rust
pub fn draft_to_field_map(draft: &Form2550MDraft) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("return_period".into(), format!("{:02}/{}", draft.month, draft.taxable_year));
    m.insert("tin".into(), draft.tin.clone());
    // ... map every XML field to your draft struct ...
    m
}
```

### Step 4.2 — PDF Generation

Update `crates/bir-print` with a rendering function for the new form. The pattern follows `render_2551q_print()` — serialize the draft to JSON, feed it to the Typst template.

---

## Component Reference — `form_parts.rs`

All components in `crates/bir-desktop/src/components/form_parts.rs` are generic over `V: 'static` so they work with any GPUI view.

### Atomic Components

| Function | Purpose | Signature |
|---|---|---|
| `field_label(label, cx)` | Uppercase muted label | `→ Div` |
| `readonly_field(label, value, error, cx)` | Label + value + optional error | `→ Div` |
| `currency_display(amount, cx)` | `₱ 1,234.56` with primary color | `→ Div` |

### Compound Components

| Function | Purpose | Key Props |
|---|---|---|
| `taxpayer_info_section(props, cx)` | 7-field profile info block | `TaxpayerInfoProps` with 7 field/error pairs |
| `penalty_summary_section(...)` | Surcharge + Interest + Compromise + Total | 5 `f64` amounts |
| `form_accordion(id, label, is_expanded, is_valid, has_error, on_click, content, cx)` | Collapsible section card | ✓/✗ indicator, expand/collapse, error border |
| `computation_row_readonly(label, amount, is_total, cx)` | Non-editable computed field | `is_total` controls emphasis styling |
| `computation_row_input(props, cx)` | Editable field with lock support | `ComputationRowInputProps` — label, input, error, locked_message, is_mobile |
| `atc_schedule_table(props, cx)` | Full ATC schedule grid | `AtcScheduleTableProps` — title, columns, rows with inputs |

### `form_accordion` — Detailed API

```rust
pub fn form_accordion<V: 'static, F>(
    id: &str,           // Unique element ID for GPUI (e.g., "acc_filing_period")
    label: &str,        // Section heading text
    is_expanded: bool,  // Current expand state (from view struct field)
    is_valid: bool,     // Shows green ✓ when true
    has_error: bool,    // Red border when true
    on_click: F,        // Toggle callback — use cx.listener(|this, _, _, cx| { ... })
    content: AnyElement, // The section body — call .into_any_element()
    cx: &Context<V>,
) -> gpui::Div
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static
```

**Important:** The `on_click` callback must use `cx.listener()` to capture `&mut Self`. Because the closure signature is `Fn(&ClickEvent, &mut Window, &mut App)` (not `Context<V>`), it works through GPUI's entity listener system.

### `atc_schedule_table` — Row Data

```rust
pub struct ScheduleRowProps<'a> {
    pub atc: String,            // ATC code (e.g., "PT010")
    pub description: String,    // Tax type description
    pub amount_label: String,   // Column header for the amount column
    pub rate: String,           // Display string (e.g., "3.0%")
    pub tax_due: f64,           // Computed tax due (readonly)
    pub error_message: Option<&'a String>,  // Validation error
    pub input_component: AnyElement,         // The editable input widget
}
```

Each row receives an `AnyElement` for the input — this lets the calling view maintain full control over its `InputState` entities while delegating the layout to the library.

---

## Component Reference — `form_engine.rs`

### `FormViewTrait`

Located at `crates/bir-desktop/src/components/form_engine.rs`.

```rust
pub trait FormViewTrait: 'static + Sized {
    // ── Required: Metadata ──
    fn form_title(&self) -> &'static str;
    fn form_subtitle(&self) -> &'static str;
    fn form_version(&self) -> &'static str;

    // ── Required: State ──
    fn current_status(&self) -> FilingStatus;
    fn submitted_at(&self) -> Option<&str>;
    fn confirmed_at(&self) -> Option<&str>;

    // ── Required: Actions ──
    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn print_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>);

    // ── Provided: Shared UI ──
    fn render_status_pipeline(&self, cx: &Context<Self>) -> gpui::Div { ... }
    fn render_header(&self, cx: &Context<Self>) -> gpui::Div { ... }
}
```

**What `render_status_pipeline` gives you:**
- A 5-step horizontal pipeline: Draft → Queued → Submitted → Confirmed → Paid
- Completed steps show green ✓, current step is highlighted with status-specific color
- Submitted/Confirmed steps show date tooltips on hover

**What `render_header` gives you:**
- Centered form title, subtitle, and version
- "PDF Viewer" button (always visible)
- "View Confirmation" button (visible when Confirmed or Paid)

---

## Walkthrough — How `Form2551QView` Uses the Engine

The production `form_2551q_view.rs` (1,473 LOC) demonstrates the full integration:

1. **Struct fields** — `show_filing_period`, `show_background_info`, `show_schedule_1`, `show_tax_computation` (accordion state booleans).
2. **Helper wrappers** — `field_label()`, `readonly_field()`, `currency_display()` delegate directly to `form_parts`.
3. **`render()` method:**
   - Calls `<Self as FormViewTrait>::render_header()` and `::render_status_pipeline()`
   - Builds section content using `taxpayer_info_section()`, `atc_schedule_table()`, `computation_row_*()`, `penalty_summary_section()`
   - Wraps each section in `form_accordion()` with `cx.listener()` toggles
   - Assembles the full-page layout: Back button + Toolbar → Status Banner → Scrollable Form Content
4. **Toolbar** — Status-aware buttons (Save/Submit for Draft, Cancel for Queued, Check Confirmation for Submitted, Mark Paid for Confirmed) — this is the one section still inline because it's deeply form-specific with validation logic.

---

## Checklist — Adding a New Form from Scratch

- [ ] **bir-core**: Create `forms/form_<code>.rs` with Draft struct + `Default`
- [ ] **bir-core**: Add `new_from_profile()` constructor
- [ ] **bir-core**: Implement `FormValidator` (shared trait from `forms/mod.rs`) with field-level validations
- [ ] **bir-core**: Add `recompute()` if the form has auto-calculated fields
- [ ] **bir-core**: Add state transition methods (`transition_to_queued()`, `transition_to_paid()`, `revert_to_draft()`, etc.)
- [ ] **bir-core**: Register in `forms/mod.rs` (`pub mod` + `pub use`)
- [ ] **bir-core**: Register in `forms/registry.rs` (`FORM_REGISTRY`)
- [ ] **bir-core/db**: Add migration for the new draft table
- [ ] **bir-core/db**: Add `save_*_draft()` and `get_*_draft()` methods
- [ ] **bir-desktop**: Create `views/form_<code>_view.rs`
- [ ] **bir-desktop**: Define view struct with accordion state bools
- [ ] **bir-desktop**: Implement `FormViewTrait` (6 required methods)
- [ ] **bir-desktop**: Implement `Render` using `form_parts` components
- [ ] **bir-desktop**: Register in `views/mod.rs`
- [ ] **bir-desktop**: Wire routing in `app.rs`
- [ ] **bir-core**: Create `forms/form_<code>_xml.rs` for BIR submission
- [ ] **bir-print**: Add PDF rendering function
- [ ] **Tests**: Add lifecycle integration test in `bir-core/tests/`

---

## Appendix — PDF Layout Editor

The **PDF Layout Editor** is an internal tool to visually align form fields on top of the SVG backgrounds generated from official BIR PDFs.

### How it works
- **Single Combobox** — Click the combobox to see the list of available forms. Type to filter. Select one and it auto-loads instantly.
- **Auto-loads first form** — If `formtypes/2551Qv2018/` exists, it's loaded on startup to avoid a blank canvas.
- **Pagination** — Only shows when a form is loaded, displays `1 / 2` format, and respects the max page count.

### How to add new form layouts

Each form needs a directory under `formtypes/` with this structure:

```
formtypes/
└── <FormID>/            # e.g. "1701Qv2023"
    ├── formtype.json    # REQUIRED — field positions, coordinates, sizes
    ├── metadata.json    # Optional — title, source URL, SHA
    ├── template.typ     # Optional — Typst template for PDF generation
    └── pages/
        ├── page1.svg    # SVG background for page 1
        └── page2.svg    # SVG background for page 2
```

The minimum requirement is the `formtype.json` file. The editor will auto-discover it.

**To test the layout editor:**
Run the application with the feature flag:
```bash
cargo run --features layout-editor
```
The combobox will automatically list your new form.
