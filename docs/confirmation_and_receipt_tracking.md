# BIR Confirmation and Receipt Tracking

## Overview
This document outlines the architecture and implementation of the post-submission workflow for eBIR forms, specifically focusing on how the system handles BIR email confirmations and user-uploaded payment receipts.

## 1. Email Confirmation Viewer

Once a form has been submitted and the background cron jobs successfully poll and parse the official BIR confirmation email, the system persists this email.

### Email Matching and Parsing Logic

The system utilizes robust logic to parse and match BIR confirmation emails, ensuring accuracy and preventing duplication:

1. **Regex Parsing (`receipt.rs`)**:
   - The system parses the raw email content using precise regular expressions:
     - `Filename`: Matches `File name: <filename>` (e.g., `261708015000-2551Qv2018-122026Q1.xml`).
     - `Date`: Matches `Date received by BIR: <date>` (e.g., `26 April 2026`).
     - `Time`: Matches `Time received by BIR: <time>` (e.g., `03:51 PM`).
   - The parsed time (e.g., `03:51 PM`) is flawlessly converted from a 12-hour AM/PM format into a 24-hour internal database time (e.g., `15:51:00`) using `NaiveTime::parse_from_str`.

2. **Deduplication (`db.rs`)**:
   - When saving a receipt, the system queries the `submission_receipts` table by `filename`. If a receipt with the identical filename, received date, and received time (in 24-hour format) already exists, the system safely ignores the duplicate without generating errors.

3. **Status Confirmation (`db.rs` / `confirm_2551q_from_receipt`)**:
   - The extracted filename is split to retrieve the TIN, Form Type, and Period (e.g., `122026Q1`).
   - The system safely extracts the specific Taxable Year (e.g., `2026`) and Quarter (e.g., `1`) from the period string.
   - It queries the `form_drafts` table for the exact match (TIN, Year, and Quarter).
   - **Safety Check**: The receipt timestamp (`Date` + `Time`) is compared against the draft's `submitted_at` timestamp. A 5-minute buffer is applied to ensure that older confirmation receipts cannot erroneously override a newly submitted draft.
   - Once verified, the system transitions the form's status to `Confirmed`, updates the `confirmed_at` timestamp, and associates the receipt's filename.

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
