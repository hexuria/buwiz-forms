# BIR Confirmation and Receipt Tracking

## Overview
This document outlines the architecture and implementation of the post-submission workflow for eBIR forms, specifically focusing on how the system handles BIR email confirmations and user-uploaded payment receipts.

## 1. Email Confirmation Viewer

Once a form has been submitted and the background cron jobs successfully poll and parse the official BIR confirmation email, the system persists this email.

### Storage
- The raw text of the email is saved directly into the database within the `submission_receipts` table under the `raw_text` column.
- The `FilingStatus` of the form automatically transitions to `Confirmed`.

### Viewing Confirmation
- In the form toolbar, a **"View Confirmation Email"** button becomes available.
- Clicking this triggers logic in `Form2551QView` which looks up the `SubmissionReceipt` in the database by matching its filename against `draft.submission_filename` (or falling back to the newly updated `default_submission_filename()` which appends `#email#`).
- The receipt is then passed into the new `EmailConfirmationView` (`crates/bir-desktop/src/views/email_confirmation_view.rs`), which displays the raw email text inside a scrollable, formatted UI.

### Exporting and Printing
The `EmailConfirmationView` includes action buttons to:
- **Export PDF**: Generates a clean PDF containing the email metadata, form summary (including Tax Due and Penalties), and saves it to the user's selected path using `rfd::FileDialog`.
- **Print**: Generates the same PDF to a temporary directory and pipes it to the system printer using the `lp` command (macOS/Linux compatible).
- This PDF generation utilizes the existing `bir_print::build_simple_confirmation_pdf` utility.

---

## 2. Payment Receipt Tracking

After a form is successfully confirmed, users have the ability to attach proof of payment (images or PDFs).

### Data Schema
- The `Form2551QDraft` schema includes a `payment_receipt_path: Option<String>` field.
- Because drafts are stored as JSON blobs (`data_json` column) in the `form_drafts` table, this was added as a fully backward-compatible schema update using `#[serde(default)]`. No SQL migrations were required.

### Upload Workflow
- In the `Confirmed` or `Paid` state, if `payment_receipt_path` is `None`, the toolbar displays an **"Upload Receipt"** button.
- Clicking this opens a file picker (via `rfd`).
- The selected file (PNG, JPG, JPEG, or PDF) is copied into the application's persistent data directory under `bir_data/receipts/` with a sanitized filename format: `receipt-{TIN}-{YEAR}-{QUARTER}.{EXT}`.
- The path is saved to the draft, and the database is updated.

### Viewing and Re-uploading Receipts
- Once a receipt is linked, the toolbar button switches to **"View Receipt"**.
- This opens the `ReceiptViewerView` (`crates/bir-desktop/src/views/receipt_viewer.rs`).
- **Images (PNG, JPG, JPEG)**: Rendered directly within the GPUI window using the `img()` component.
- **PDFs**: GPUI cannot natively render PDFs yet, so the view provides a stylized placeholder and an "Open System Viewer" button that triggers the OS-level `open` command.
- The viewer includes a **"Change / Re-upload"** button, allowing users to correct mistakes. Re-uploading triggers an event that updates the main `Form2551QView` state and persists the new file path to the database.

---

## 3. UI Aesthetics
- Action buttons in the document viewers (such as "Export PDF", "Print", "Change / Re-upload") use the `.outline()` variant instead of `.ghost()`. This ensures high visibility against the dark viewer backgrounds (`gpui::rgb(0x0b0b0b)`).
- The `ReceiptViewerView` utilizes a clean `📄` emoji as the visual placeholder for PDFs rather than a standard SVG, avoiding compilation issues with unavailable `gpui_component::IconName` variants while remaining perfectly scaled.
