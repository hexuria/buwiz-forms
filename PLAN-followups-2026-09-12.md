# Follow-ups after PR #51 — execution plan

Base: `public/main` at `15cc5371` (PR #51 merged, CI green). Work on a new
branch `gol/followups-2026-09-12`; push to `public` (the only remote with CI
minutes); one PR per workstream so each can be verified and merged on its own.

Every item below was read from the code on this branch; the "why" lines are
the actual root causes, not guesses.

---

## Workstream A — Print preview: receipt page, label, Print button

**PR title:** `fix(print): confirmation page after the form, working Print, no jargon label`

### A1. The BIR confirmation no longer prints after the last form page (regression)

**Why.** Before `83fe4122` ("migrate print pipeline to HTML-only rendering",
2026-07-16) the PDF preview looked up `draft.receipt_id` →
`db.get_submission_receipt_by_id`, built a `pdf_viewer::ConfirmationInfo`
(subject, from, to, received date/time, body) and appended it as text pages
with `bir_print::append_text_pages_to_pdf`. The HTML migration replaced the
viewer with `frozen_html::filled_document(slug, fields)`, which knows only
the form's field map — the receipt lookup and the appended page were deleted
with the old viewer and never re-created. `receipt_id` is still set on the
draft by `confirm_1601c_from_receipt` / `confirm_2551q_from_receipt`, and
`append_text_pages_to_pdf` still exists unused in `bir-print`.

**Change.**
- `bir-print/src/frozen_html.rs`: add `ReceiptPage { filename, from, to,
  received_at, subject, body_text }` (plain struct — `bir-print` must not
  depend on `bir-core`) and `filled_document_with_receipt(slug, fields,
  Option<&ReceiptPage>)`. When present, insert one more
  `<section class="page page-receipt">` before `</body>`: BIR seal-free
  typographic page titled "BIR Tax Return Receipt Confirmation" with the
  fields above and the email body in a monospace block. `base.css` already
  gives `.page { break-after: page }` and `.page:last-of-type { break-after:
  auto }`, so it lands on its own printed sheet after page 2 with no extra
  CSS. `filled_document` stays as a thin wrapper.
- `form_html_preview_launcher.rs`: `launch_frozen_form_preview` takes
  `Option<ReceiptPage>`; form views build it from `self.draft.receipt_id` via
  `db.get_submission_receipt_by_id` when the status is Confirmed (1601-C and
  2551Q both; the other forms pass `None`).
- Sanitisation: body from `receipt.raw_text` (already plain text; HTML-escape
  it). Do **not** embed `raw_html` — it is third-party email HTML inside a
  `with_html` WebView.

**Tests.** `frozen_html`: document with a receipt has exactly one
`.page-receipt` after `.page-2`, escaped body, and no receipt section when
`None`. Launcher: a Confirmed 1601-C draft with `receipt_id` produces a
document containing the receipt; a Draft does not.

### A2. Remove the "Frozen HTML ready to fill/print." label

**Why.** `FrozenHtmlPreviewView.status` is set to that string on success and
painted in the toolbar. It is build jargon, not a user message.

**Change.** Status is `Option<String>`; only failures ("could not print: …",
"preview failed: …") are shown. Rename the window title from
"`{FORM} Frozen HTML`" to "`{FORM} — Print Preview`" in the six launch sites
(same jargon; one string each).

### A3. Print button does nothing on macOS (and is unreliable elsewhere)

**Why.** `FrozenHtmlPreviewView::print` runs
`webview.raw().evaluate_script("window.print();")`. WKWebView does not
implement `window.print()` — it is a silent no-op — so on macOS nothing
happens. WebView2 and WebKitGTK do honour it, but only if page focus/scripts
allow. The pinned `lb-wry 0.53.3` has a **native** `WebView::print()` on all
three backends (macOS `NSPrintOperation` via `printOperationWithPrintInfo:`,
Windows WebView2 `ShowPrintUI`, Linux `WebKitPrintOperation`).

**Change.** Call `webview.raw().print()` and surface its `Err` in the
toolbar. Keep `window.print()` only as a fallback if `print()` returns an
error. Verify per platform: macOS opens the system print sheet with the
form pages + receipt page; Windows opens the WebView2 print UI; Linux the
GTK print dialog (CI builds all three; the dialogs need a manual check on
each).

**Estimate.** ~3 h including tests. Files: `bir-print/src/frozen_html.rs`,
`views/frozen_html_preview.rs`, `views/form_html_preview_launcher.rs`,
`views/form_1601c_view.rs`, `views/form_2551q_view.rs` (+ 4 other form views
for the title string), `docs/AGENT.md` (`form.pdf` note).

---

## Workstream B — Preview window keyboard: Cmd+W / Cmd+M / Cmd+Q

**PR title:** `fix(preview): standard window shortcuts in the print preview window`

### B1. Cmd+W and Cmd+M do nothing in the preview window

**Why.** `CloseWindow` / `MinimizeWindow` are global key bindings
(`bind_global_keys`) and App-menu items, but their **handlers** are
registered only on `AppState`'s root element (`app.rs:2400`), i.e. the main
window. `FrozenHtmlPreviewView` registers none, so in that window the action
has no target and is dropped. (`EmailConfirmationView` already does this
right: `key_context` + `track_focus` + `on_action(CloseWindow →
window.remove_window())`.)

**Change.** In `FrozenHtmlPreviewView::render`: a `FocusHandle`,
`.key_context("FrozenHtmlPreview")`, `.track_focus(...)`, and handlers
`CloseWindow → window.remove_window()`, `MinimizeWindow →
window.minimize_window()`, `ZoomWindow → window.zoom_window()`. Focus the
view on open so the bindings resolve even while the native WebView child has
first-responder (menu key equivalents route through the App menu → action →
key window, so the menu path works once handlers exist).

### B2. Cmd+Q from the preview quits the whole app

**Why — and a correction.** Cmd+Q quits from the **main** window too:
`handle_quit_application` → `request_application_quit` →
`ApplicationQuitDecision::Quit` → `cx.quit()` unless there are unsaved
compliance edits. What hides to the tray is the red **close** button
(`on_window_should_close` → `hide_from_dock`) and **Cmd+W**
(`handle_close_window` → `hide_from_dock`). So the preview is consistent with
the main window; nothing intercepts Cmd+Q anywhere.

**Decision needed (see bottom).** Either keep Cmd+Q = quit everywhere (then
B2 is no change), or make Cmd+Q hide-to-tray like Cmd+W, with Quit only from
the tray menu / agent `shutdown`. The second matches what you described
wanting; it is a one-line change in `handle_quit_application` plus the
`MacosQuitRouter` path, but it changes long-standing behaviour and the
AGENT.md "Hide is not quit" contract wording.

**Estimate.** ~1 h. Files: `views/frozen_html_preview.rs`
(+ `actions.rs`/`app.rs` if B2 changes).

---

## Workstream C — Background Tasks: filters, log viewer, log retention

**PR title:** `fix(cron-tasks): honest filters, readable log levels, bounded log storage`

### C1. Job filter "Done" is dead; "Archived" is misnamed

**Why.** "Done" matches `Done | Submitted | Confirmed | Paid`. A job becomes
`Done` only when its cron expression has no next run
(`background_cron.rs:1559`) — never for the every-minute poll jobs, which go
Queued → **Archived** on completion. Submission cards come from the in-flight
query (Queued, briefly Submitted). So "Done" is empty in practice.

**Change.** Drop "Done" from the combobox; rename "Archived" → "Completed"
(label only; DB status string unchanged); "Purge Archives" → "Purge
Completed". Default filter stays "Queued".

### C2. Log level colours

**Why.** ERROR/FATAL → `danger`; WARN → `primary` (a literal "fallback"
comment; near-black in the light theme — the black WARN you saw); INFO →
`info` (that teal wall); else muted. The theme has `warning` (used by the
deadlines list) and it was simply not used.

**Change.** ERROR/FATAL → `danger`, WARN → `warning`, INFO → `foreground`
(plain), DEBUG/TRACE → `muted_foreground`. Theme colours already switch with
light/dark. Add the level filter options "Error", "Warn", "Info" as today plus
nothing new.

### C3. Log order, count, and where the newest line is

**Why.** Refresh keeps the last 5,000 lines of the file, then paints the last
500 that match the filter, oldest → newest top → bottom. The note says
"of 5000 matching" — that is the retained window, not the file; and nothing
scrolls to the newest line.

**Change.** Count real file lines for the note ("Showing the newest 500 of
12,340 lines; Export for the full file"), scroll to the bottom after
Refresh / tab switch, and add a one-line hint "newest at the bottom". Keep
oldest→newest (tail convention).

### C4. Log storage is unbounded

**Why.** `gui.rs:98` uses `tracing_appender::rolling::never` — one
`ebirforms.log` that only "Clear Logs" ever truncates (1.7 MB after three
days here; order of 100–200 MB/year on a busy machine).

**Change.** `RollingFileAppender::builder().rotation(Rotation::DAILY)
.filename_prefix("ebirforms").filename_suffix("log").max_log_files(14)`
(tracing-appender 0.2.5 supports it) — daily files, automatic pruning to 14
days. The Logs tab, Export Error Logs, Clear Logs and Email Support read the
**current** day's file (`ebirforms.YYYY-MM-DD.log`); Export offers the kept
set as one concatenated file. `bir-headless` uses the same appender
(`headless.rs`), so it gets the same bound.

**Tests.** filter mapping unit test (status → visible), colour mapping by
level, `tail_lines` count/scroll helpers, appender path helper for "today's
file".

**Estimate.** ~2.5 h. Files: `views/cron_tasks.rs`, `gui.rs`,
`agent/headless.rs`, `NOTES-UI-HANG.md` (log section).

---

## Order and verification

1. **C** first (smallest, self-contained, no behaviour decisions).
2. **A** (biggest user-visible fix; needs a Confirmed return to verify —
   Juan's Aug/Sep/Oct/Nov/Dec 2026 all qualify on the repro DB).
3. **B** after the Cmd+Q decision.

Per PR: `cargo fmt --check`, `cargo clippy --all-targets`, full
`cargo test` for `bir-core`/`bir-desktop`/`bir-print`, `--locked` build of
`bir` + `bir-headless`, push to `public`, wait for the 9 CI jobs, then a
manual check on the Mac with `scripts/dev_bundle_macos.sh`.

Manual checks: A — open the Nov 2026 1601-C (Confirmed) → Print preview →
page 3 is the BIR confirmation; Print opens the system sheet; toolbar shows
no label. B — Cmd+W closes the preview only, Cmd+M minimises it, the main
window is untouched. C — filter list has no "Done"; WARN is amber, INFO is
plain; Logs tab lands at the newest line; `~/Library/Group
Containers/group.dev.goldcoders.bir/logs/` holds `ebirforms.<date>.log`
files and never more than 14.

## Decisions (taken 2026-09-12)

1. **Cmd+Q** → hide to the tray; real quit only from the tray menu or agent
   `shutdown`. Menu item reads "Close to Menu Bar".
2. **Log retention** → daily files, **7 days default, user-set** in Settings
   ("Keep application logs for": 3 / 7 / 14 / 30 / 90).
3. **Receipt page** → text header **plus** BIR's HTML email (sanitised).
4. **Titles** → "1601-C — Print Preview" (and the other forms alike).
5. **Filters** → "Done" removed, "Archived" shown as "Completed".

## Status

| workstream | PR | branch | state |
| --- | --- | --- | --- |
| C — filters, log levels, retention | #52 | `gol/followups-2026-09-12` | done; verified on disk (legacy log folded into `ebirforms.2026-09-12.log`) |
| A — receipt page, Print, label, titles, preview canvas, A4 fit, status banner | #53 (base #52) | `gol/followups-print-preview` | done; verified on the Confirmed Nov 2026 return. Receipt page = mail-client print (mailbox, title, sender + arrival time, To, email as sent). `email_received_at` column added at ingest; older rows fall back to BIR's stamp and heal on the next poll. Preview: A4 sheets on the toolbar grey, each form page zoomed to fit (WebKit print-to-PDF harness: 3 pages, was 6); no paper selector; macOS print forces A4 portrait. Form status banner: content-column width, tone colours, close button, clears when the preview window closes. |
| B — window shortcuts, Cmd+Q | #54 (base #53) | `gol/followups-window-keys` | done; Cmd+W/M/zoom in every secondary window, Cmd+Q hides to the tray, menu item "Close to Menu Bar". Keys need the manual check. |

Merge order: #52, then #53 (GitHub retargets it to `main`), then #54.
