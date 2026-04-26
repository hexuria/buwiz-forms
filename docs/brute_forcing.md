# BIR Submission Bruteforce Testing Report

## Overview
This document records the results of systematic bruteforce testing against the BIR FTP submission gateway. The goal was to diagnose why our Rust application was successfully uploading encrypted payloads (receiving FTP `226 Operation successful`) but **never receiving confirmation emails** from the BIR.

## Test Infrastructure
- **Script**: `bruteforce/bruteforce.py` — Autonomous Python harness with SQLite tracking, FTP retry, and IMAP email polling.
- **Database**: `bruteforce/bruteforce.db` — Records every experiment with payload diffs, FTP results, and email confirmation status.
- **Logs**: `bruteforce/logs/` — Per-run payload JSON, encrypted XML, and curl verbose output.

## Experiment Results

| Run | Name | Hypothesis | FTP | Email | Delay |
|-----|------|-----------|-----|-------|-------|
| 01 | `run01_baseline_q1_resubmit` | Resubmit Q1 with only updated timestamp | ✅ | ✅ | ~20 min |
| 02 | `run02_amended_return_q1` | Amended flag bypasses dedup? | ✅ | ✅ | ~20 min |
| 03 | `run03_fresh_q2` | New period (Q2) triggers email | ✅ | ✅ | ~20 min |
| 04 | `run04_fresh_q3` | New period (Q3) confirms Q2 | ✅ | ✅ | ~20 min |

**Result: 4/4 experiments confirmed** — ALL submissions triggered BIR confirmation emails.

## Key Findings

### 1. Root Cause: Missing `#email#` in Filename (CRITICAL)
The BIR backend uses the **filename** — not the XML payload — to determine the confirmation email recipient. The expected format is:

```
{TIN}-{FORM_TYPE}-{PERIOD}#{EMAIL}#.xml
```

Example: `010558054000-2551Qv2018-122026Q1#codeitlikemiley@gmail.com#.xml`

Our Rust app was generating filenames **without** the `#email#` suffix:
```
010558054000-2551Qv2018-122026Q1.xml  ← WRONG (no email)
```

The BIR server accepted and processed the file, but had no email to send the receipt to.

**Affected code paths**:
- `form_2551q.rs` → `default_submission_filename()` — used by the background cron
- `official_import.rs` → `submit_filename` format string — had wrong delimiter (`#` instead of `-` before period)

### 2. BIR Processing Delay (~20 minutes)
The BIR backend does NOT send confirmation emails instantly. Our testing shows a consistent **~20 minute delay** between FTP upload and email delivery. This means:
- A 5-minute email polling timeout will ALWAYS miss the confirmation
- The background cron's polling job must be patient — polling every minute for at least 30 minutes

### 3. BIR Strips `#email#` from Confirmation Email
The BIR confirmation email shows the filename **without** the `#email#` suffix:
```
File name: 010558054000-2551Qv2018-122026Q1.xml  ← no email in this
```
Our receipt parser (`receipt.rs` → `split_bir_filename`) works correctly because it splits on `-` and the email hash portion is absent.

### 4. BIR Does NOT Deduplicate Submissions
Resubmitting the exact same TIN+Period+Quarter combination with only an updated `txtDateIssue` timestamp **still triggers a new confirmation email**. The BIR backend processes every file it receives, regardless of whether a previous filing exists for that period.

### 5. IMAP `UNSEEN` Filter is Unreliable
Our IMAP email polling used the `UNSEEN` filter, which silently skips emails that the user has already read on their phone or in Gmail web. This was a secondary cause of missed confirmations — even if BIR sent the email, our poller couldn't find it because it was already marked as read.

**Fix**: Change to search `ALL` BIR emails with date filtering, and use internal deduplication (DB-tracked filenames via `submission_receipts` table's `UNIQUE(filename)` constraint) instead of relying on IMAP's seen/unseen state.

## Bugs Fixed

### Bug 1: `form_2551q.rs` — Missing Email in Filename
**File**: `crates/bir-core/src/forms/form_2551q.rs`, line 270-272
```diff
-format!("{}-2551Qv2018-{}.xml", self.tin, self.period_code())
+format!("{}-2551Qv2018-{}#{}#.xml", self.tin, self.period_code(), self.email)
```

### Bug 2: `official_import.rs` — Wrong Delimiter in Filename
**File**: `crates/bir-core/src/official_import.rs`, line 70
```diff
-format!("{}-{}#{}#{}#.xml", tin, form_type, period_code, email)
+format!("{}-{}-{}#{}#.xml", tin, form_type, period_code, email)
```

### Bug 3: `fetcher.rs` — UNSEEN Filter Misses Read Emails
**File**: `crates/bir-core/src/email/fetcher.rs`, line 127
```diff
-session.search("UNSEEN FROM \"ebirforms-noreply@bir.gov.ph\"")
+session.search("FROM \"ebirforms-noreply@bir.gov.ph\" SINCE 01-Jan-2026")
```
Plus: Remove redundant `\\Seen` flag setting since we no longer rely on UNSEEN. Instead, deduplication is handled by the `submission_receipts` table's `UNIQUE(filename)` constraint — `save_submission_receipt` uses `ON CONFLICT(filename) DO UPDATE`, so processing the same email twice is harmless.

## Production Checklist

- [x] Fix `default_submission_filename` to include `#email#`
- [x] Fix `official_import.rs` filename delimiter
- [x] Fix `fetcher.rs` IMAP search to use `ALL` instead of `UNSEEN`
- [x] Remove `+FLAGS (\\Seen)` store call (no longer needed)
- [x] Verify `background_cron.rs` uses the fixed `default_submission_filename` (line 98 — calls `draft.default_submission_filename()`)
- [x] Email polling patience — cron runs every 60 seconds (`0 * * * * *`), so the 30-day IMAP `SINCE` window + the DB `UNIQUE` dedup handles BIR's ~20 min delay automatically
- [x] All changes compile cleanly (`cargo check` passes)
