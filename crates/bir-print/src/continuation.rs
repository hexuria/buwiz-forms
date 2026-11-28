//! Overflow detection and plain continuation sheets for frozen BIR print forms.
//!
//! When a filled return holds schedule rows past the fixed number of lines on
//! the official frozen sheet, `detect_overflow` identifies those rows and
//! `continuation_pages_html` renders one or more plain `.page-continuation`
//! pages to append after the official pages without modifying a single byte of
//! the official pages.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::frozen_html;

/// Maximum data rows placed on a single continuation page before splitting onto
/// another `.page-continuation` sheet.
const MAX_ROWS_PER_PAGE: usize = 20;

/// One column in an overflowing schedule table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverflowColumn {
    pub heading: String,
    pub is_amount: bool,
    pub subtotal: Option<String>,
}

/// One overflowing schedule row with its continuing 1-based row number and
/// cell values in official column order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverflowRow {
    pub row_number: usize,
    pub values: Vec<String>,
    /// The writer key of each value, in the same order: the upload control
    /// the row's cell holds.
    pub keys: Vec<String>,
}

/// All overflowing rows of a single schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverflowSchedule {
    pub heading: String,
    pub printed_capacity: usize,
    pub columns: Vec<OverflowColumn>,
    pub rows: Vec<OverflowRow>,
    pub has_total: bool,
}

impl OverflowSchedule {
    /// Human-readable warning line for print preview banners.
    pub fn warning_line(&self) -> String {
        let count = self.rows.len();
        if count == 1 {
            format!("{}: 1 row is on an additional sheet", self.heading)
        } else {
            format!("{}: {count} rows are on an additional sheet", self.heading)
        }
    }
}

/// One text cell whose rendered text was shortened with `"..."` at the 6.5 pt
/// floor, carried onto the plain additional sheet with its full value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TruncatedEntry {
    pub page_number: usize,
    pub label: String,
    pub full_text: String,
}

/// Structured overflow report for one filled return.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OverflowReport {
    pub schedules: Vec<OverflowSchedule>,
}

impl OverflowReport {
    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }

    pub fn total_overflow_rows(&self) -> usize {
        self.schedules.iter().map(|s| s.rows.len()).sum()
    }

    /// One warning line per overflowing schedule, e.g.
    /// `"Schedule A: 8 rows are on an additional sheet"`.
    pub fn warning_lines(&self) -> Vec<String> {
        self.schedules
            .iter()
            .map(OverflowSchedule::warning_line)
            .collect()
    }

    /// Combined single-string warning summary, or `None` when nothing overflows.
    pub fn summary(&self) -> Option<String> {
        if self.schedules.is_empty() {
            None
        } else {
            Some(self.warning_lines().join("; "))
        }
    }
}

/// Return value of `filled_document_with_report`: the rendered HTML plus the
/// structured overflow report (and its warning lines) for preview callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilledDocument {
    pub html: String,
    pub overflow: OverflowReport,
}

impl FilledDocument {
    /// Structured cell-level overflow warnings from the rendered document
    /// (official pages and cloned continuation pages alike), such as amounts
    /// wider than their comb slots printed as one fitted line.
    pub fn cell_overflow_warnings(&self) -> Vec<frozen_html::OverflowWarning> {
        frozen_html::overflow_warnings(&self.html)
    }

    /// Unified warning lines for the preview: schedule rows moved to additional
    /// pages plus any over-wide amounts printed as a fitted line instead of in boxes.
    pub fn warning_lines(&self) -> Vec<String> {
        let mut lines = self.overflow.warning_lines();
        let cell_warnings = self.cell_overflow_warnings();
        if !cell_warnings.is_empty() {
            // Blank cells carry no value: an empty amount-small mark is no
            // overflow of the filer's, so it neither reaches the additional
            // sheet nor counts here.
            let non_blank = |w: &&frozen_html::OverflowWarning| !w.value.trim().is_empty();
            let amount_count = cell_warnings
                .iter()
                .filter(|w| w.kind == "amount-comb" && non_blank(w))
                .count();
            let small_count = cell_warnings
                .iter()
                .filter(|w| w.kind == "amount-small" && non_blank(w))
                .count();
            if small_count == 1 {
                lines.push(
                    "1 amount printed very small is also printed in full on an additional sheet"
                        .to_string(),
                );
            } else if small_count > 1 {
                lines.push(format!(
                    "{small_count} amounts printed very small are also printed in full on an additional sheet"
                ));
            }
            if amount_count == 1 {
                lines.push("1 amount printed as a fitted line instead of in boxes".to_string());
            } else if amount_count > 1 {
                lines.push(format!(
                    "{amount_count} amounts printed as fitted lines instead of in boxes"
                ));
            }
            let truncated_count = cell_warnings
                .iter()
                .filter(|w| w.kind == "text-truncated")
                .count();
            if truncated_count == 1 {
                lines.push("1 long entry is printed in full on an additional sheet".to_string());
            } else if truncated_count > 1 {
                lines.push(format!(
                    "{truncated_count} long entries are printed in full on an additional sheet"
                ));
            }
            let identifier_count = cell_warnings
                .iter()
                .filter(|w| w.kind == "identifier-small")
                .count();
            if identifier_count == 1 {
                lines.push(
                    "1 TIN or ATC printed very small is also printed in full on an additional sheet"
                        .to_string(),
                );
            } else if identifier_count > 1 {
                lines.push(format!(
                    "{identifier_count} TINs or ATCs printed very small are also printed in full on an additional sheet"
                ));
            }
            let other_count = cell_warnings.len()
                - amount_count
                - small_count
                - identifier_count
                - truncated_count;
            if other_count == 1 {
                lines.push("1 value overflowed its official box".to_string());
            } else if other_count > 1 {
                lines.push(format!(
                    "{other_count} values overflowed their official boxes"
                ));
            }
        }
        lines
    }

    pub fn warning_summary(&self) -> Option<String> {
        let lines = self.warning_lines();
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("; "))
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnSpec {
    stem: String,
    heading: String,
    is_amount: bool,
    start_idx: usize,
    end_idx: usize,
}

#[derive(Debug, Clone)]
struct ScheduleSpec {
    heading: String,
    columns: Vec<ColumnSpec>,
    has_total: bool,
}

/// Detect schedule rows in `fields` whose values no cell on `slug`'s frozen
/// sheet prints.
///
/// Builds on `frozen_html::unprinted_keys` and also inspects `not_joined` keys
/// whose writer-key stem belongs to a repeating schedule past its printed row
/// capacity (such as 2000-OT Schedule 1.A rows 6–15 or 1600WP Schedule 1.1
/// rows 7–10, which `writer-cells.json` records as `not_joined` overflow rows).
pub fn detect_overflow(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> Result<OverflowReport, String> {
    let Some(wc_json) = frozen_html::writer_cells_json(slug) else {
        return Ok(OverflowReport::default());
    };
    let parsed: Value = serde_json::from_str(wc_json)
        .map_err(|e| format!("failed to parse writer-cells.json for {slug}: {e}"))?;

    // Collect every non-empty key that is NOT printed on the frozen sheet:
    // 1) keys returned by frozen_html::unprinted_keys (completely unmapped)
    // 2) keys listed in writer-cells.json `not_joined` (many forms document
    //    their overflow schedule rows there so the coverage gate passes).
    let mut unprinted: BTreeSet<String> = frozen_html::unprinted_keys(slug, fields)?
        .into_iter()
        .collect();
    let not_joined = collect_not_joined_keys(&parsed);
    for (key, value) in fields {
        if !value.trim().is_empty() && not_joined.contains(key.as_str()) {
            unprinted.insert(key.clone());
        }
    }

    if unprinted.is_empty() {
        return Ok(OverflowReport::default());
    }

    let specs = discover_schedules(&parsed);
    let mut schedules = Vec::new();

    for spec in specs {
        let mut overflow_indices: BTreeSet<usize> = BTreeSet::new();
        for col in &spec.columns {
            for key in &unprinted {
                if let Some(idx) = match_stem_index(key, &col.stem) {
                    if idx > col.end_idx {
                        overflow_indices.insert(idx);
                    }
                }
            }
        }
        if overflow_indices.is_empty() {
            continue;
        }

        let printed_capacity = spec
            .columns
            .iter()
            .map(|c| c.end_idx.saturating_sub(c.start_idx) + 1)
            .max()
            .unwrap_or(0);
        let base_idx = spec.columns.iter().map(|c| c.start_idx).min().unwrap_or(0);

        let mut rows = Vec::new();
        let mut col_sums: Vec<f64> = vec![0.0; spec.columns.len()];
        let mut col_has_numeric: Vec<bool> = vec![false; spec.columns.len()];

        for idx in overflow_indices {
            let mut values = Vec::with_capacity(spec.columns.len());
            let mut keys = Vec::with_capacity(spec.columns.len());
            let mut has_meaningful_value = false;
            for col in &spec.columns {
                let key = format!("{}{}", col.stem, idx);
                let val = fields
                    .get(&key)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if is_meaningful_cell(&val) {
                    has_meaningful_value = true;
                }
                values.push(val);
                keys.push(key);
            }
            if !has_meaningful_value {
                continue;
            }
            for (ci, (col, val)) in spec.columns.iter().zip(values.iter()).enumerate() {
                if col.is_amount {
                    if let Some(num) = parse_amount(val) {
                        col_sums[ci] += num;
                        col_has_numeric[ci] = true;
                    }
                }
            }
            let row_number = idx.saturating_sub(base_idx) + 1;
            rows.push(OverflowRow {
                row_number,
                values,
                keys,
            });
        }

        if rows.is_empty() {
            continue;
        }

        let columns = spec
            .columns
            .iter()
            .enumerate()
            .map(|(ci, col)| {
                let subtotal = if spec.has_total && col.is_amount && col_has_numeric[ci] {
                    Some(format_amount(col_sums[ci], slug))
                } else {
                    None
                };
                OverflowColumn {
                    heading: col.heading.clone(),
                    is_amount: col.is_amount,
                    subtotal,
                }
            })
            .collect();

        schedules.push(OverflowSchedule {
            heading: spec.heading,
            printed_capacity,
            columns,
            rows,
            has_total: spec.has_total,
        });
    }

    // Also detect popup/modal overflow rows whose writer keys use a separate
    // popup naming scheme from the printed rows (1700, 1701A, 1702EX, 1702MX,
    // 1702RT, 1707, 1707A).
    for popup_sched in detect_popup_overflow(slug, fields, &unprinted) {
        if !schedules
            .iter()
            .any(|s| s.heading.eq_ignore_ascii_case(&popup_sched.heading))
        {
            schedules.push(popup_sched);
        }
    }

    Ok(OverflowReport { schedules })
}

/// Render one or more plain `.page-continuation` sheets to append after the
/// official frozen pages. Returns an empty string if `report` has no overflow.
pub fn continuation_pages_html(
    slug: &str,
    fields: &BTreeMap<String, String>,
    report: &OverflowReport,
    width: &str,
    height: &str,
) -> String {
    continuation_pages_html_with_truncated(slug, fields, report, &[], width, height)
}

/// Collect every `text-truncated` cell from `document_html` (official pages
/// and cloned band continuation pages alike), resolving each cell's 1-based
/// page number, official printed label (falling back to the nearest printed
/// caption on that page — never a cell id or file name), and full value from
/// `data-writer-value`.
pub fn collect_truncated_entries(
    slug: &str,
    document_html: &str,
    overflow: &OverflowReport,
) -> Vec<TruncatedEntry> {
    let cell_labels = load_cell_printed_labels(slug);
    let mut entries = Vec::new();
    let mut page_number = 0usize;

    for page_slice in split_page_divs(document_html) {
        let open_tag_end = page_slice.find('>').unwrap_or(page_slice.len());
        let open_tag = &page_slice[..open_tag_end];
        if open_tag.contains("page-receipt")
            || (open_tag.contains("page-continuation")
                && !open_tag.contains("page-continuation-band"))
        {
            continue;
        }
        page_number += 1;

        for warning in frozen_html::overflow_warnings(page_slice) {
            // A long text shortened with "...", or an amount or a TIN/ATC
            // shrunk below `SMALL_WHOLE_BELOW_PT`: each prints whole here.
            if !matches!(
                warning.kind.as_str(),
                "text-truncated" | "amount-small" | "identifier-small"
            ) {
                continue;
            }
            // A blank row of a converted column is blanked to nothing: its
            // empty amount-small mark carries no value, and an empty entry
            // on the additional sheet would read as a missing figure.
            if warning.value.is_empty() {
                continue;
            }
            let cell_id = warning.cell.strip_suffix("-i").unwrap_or(&warning.cell);
            let label = resolve_truncated_label(cell_id, page_slice, &cell_labels, overflow);
            entries.push(TruncatedEntry {
                page_number,
                label,
                full_text: warning.value,
            });
        }
    }

    entries
}

/// Render one coherent set of plain `.page-continuation` sheets containing
/// any fallback overflowing schedule tables plus any `"Entries continued from
/// the form"` truncated-text entries. Returns an empty string when both are empty.
pub fn continuation_pages_html_with_truncated(
    slug: &str,
    fields: &BTreeMap<String, String>,
    report: &OverflowReport,
    truncated_entries: &[TruncatedEntry],
    width: &str,
    height: &str,
) -> String {
    if report.is_empty() && truncated_entries.is_empty() {
        return String::new();
    }

    let meta = extract_header_meta(slug, fields);
    let pages = paginate_plain_pages(&report.schedules, truncated_entries, MAX_ROWS_PER_PAGE);
    let total_pages = pages.len();

    let mut out = String::new();
    for (page_idx, page) in pages.iter().enumerate() {
        let page_num = page_idx + 1;
        let id_attr = if page_idx == 0 {
            "page-continuation".to_string()
        } else {
            format!("page-continuation-{page_num}")
        };
        let sheet_label = if total_pages == 1 {
            "Additional sheet".to_string()
        } else {
            format!("Additional sheet ({page_num} of {total_pages})")
        };

        let _ = height;
        out.push_str(&format!(
            "<div class=\"page page-continuation\" id=\"{id_attr}\" style=\"width:{width};height:auto;overflow:visible;padding:24pt 28pt;box-sizing:border-box;font-family:'eBIRForms Arimo',Arial,Helvetica,sans-serif;font-size:8.5pt;line-height:1.3;color:#000;background:#fff\">"
        ));

        // Header block
        out.push_str(
            "<div style=\"border-bottom:1.5pt solid #000;padding-bottom:8pt;margin-bottom:12pt\">",
        );
        out.push_str(&format!(
            "<div style=\"display:flex;justify-content:space-between;align-items:baseline;gap:12pt\">\
             <div style=\"font-size:11pt;font-weight:700\">{}</div>\
             <div style=\"font-size:9pt;font-weight:700;text-transform:uppercase;letter-spacing:0.4pt\">{}</div>\
             </div>",
            escape_html(&meta.form_number),
            escape_html(&sheet_label),
        ));
        if !meta.form_title.is_empty() {
            out.push_str(&format!(
                "<div style=\"font-size:8.5pt;color:#222;margin-top:2pt\">{}</div>",
                escape_html(&meta.form_title),
            ));
        }
        if !page.truncated_entries.is_empty() {
            out.push_str(
                "<div style=\"font-size:7.5pt;color:#444;margin-top:3pt\">Entries shortened with &quot;...&quot; on the form are printed in full below.</div>",
            );
        }
        out.push_str(
            "<table style=\"width:100%;border-collapse:collapse;margin-top:8pt;font-size:8pt\"><tbody><tr>",
        );
        out.push_str(&format!(
            "<td style=\"width:28%;padding:2pt 8pt 2pt 0;vertical-align:top\"><span style=\"color:#444;font-weight:700\">TIN:</span> {}</td>",
            escape_html(&meta.tin),
        ));
        out.push_str(&format!(
            "<td style=\"width:46%;padding:2pt 8pt 2pt 0;vertical-align:top;overflow-wrap:anywhere;word-break:break-word\"><span style=\"color:#444;font-weight:700\">Taxpayer's Name:</span> {}</td>",
            escape_html(&meta.taxpayer_name),
        ));
        if let Some((label, value)) = &meta.period {
            out.push_str(&format!(
                "<td style=\"width:26%;padding:2pt 0;vertical-align:top;text-align:right\"><span style=\"color:#444;font-weight:700\">{}:</span> {}</td>",
                escape_html(label),
                escape_html(value),
            ));
        } else {
            out.push_str(
                "<td style=\"width:26%;padding:2pt 0;vertical-align:top;text-align:right\"></td>",
            );
        }
        out.push_str("</tr></tbody></table></div>");

        // Schedule tables on this page
        for chunk in &page.schedule_chunks {
            let sched = chunk.schedule;
            let rows = chunk.rows;
            let first_row = rows.first().map(|r| r.row_number).unwrap_or(0);
            let last_row = rows.last().map(|r| r.row_number).unwrap_or(0);

            out.push_str("<div style=\"margin-bottom:14pt\">");
            out.push_str(&format!(
                "<div style=\"display:flex;justify-content:space-between;align-items:baseline;margin-bottom:4pt\">\
                 <div style=\"font-size:9.5pt;font-weight:700\">{} (continued)</div>\
                 <div style=\"font-size:8.6pt;color:#444\">Rows {}–{} (continuing after {} printed rows)</div>\
                 </div>",
                escape_html(&sched.heading),
                first_row,
                last_row,
                sched.printed_capacity,
            ));

            // Very wide plain continuation tables (notably 2200M Schedule 1)
            // otherwise size to their unwrapped amount columns and run past the
            // paper edge. Fixed layout plus a compact cell size keeps every
            // wide-table column inside the additional sheet; narrower tables
            // keep the roomier default typography.
            let wide_table = sched.columns.len() >= 10;
            let (table_layout, font_size, cell_padding) = if wide_table {
                ("fixed", "6.8pt", "2pt 2pt")
            } else {
                ("auto", "8.6pt", "3pt 4pt")
            };
            out.push_str(&format!(
                "<table style=\"width:100%;border-collapse:collapse;table-layout:{table_layout};font-size:{font_size}\"><thead><tr>",
            ));
            out.push_str(&format!(
                "<th style=\"width:22pt;border:0.75pt solid #000;background:#f2f2f2;padding:{cell_padding};text-align:center;font-weight:700;vertical-align:bottom\">#</th>",
            ));
            for col in &sched.columns {
                let align = if col.is_amount { "right" } else { "left" };
                out.push_str(&format!(
                    "<th style=\"border:0.75pt solid #000;background:#f2f2f2;padding:{cell_padding};text-align:{align};font-weight:700;vertical-align:bottom;overflow-wrap:break-word\">{}</th>",
                    escape_html(&col.heading),
                ));
            }
            out.push_str("</tr></thead><tbody>");
            let amount_cell_style = format!(
                "border:0.75pt solid #000;padding:{cell_padding};text-align:right;vertical-align:top;white-space:nowrap;font-variant-numeric:tabular-nums"
            );
            let text_cell_style = format!(
                "border:0.75pt solid #000;padding:{cell_padding};text-align:left;vertical-align:top;overflow-wrap:break-word"
            );

            for row in rows {
                out.push_str("<tr>");
                out.push_str(&format!(
                    "<td style=\"border:0.75pt solid #000;padding:{cell_padding};text-align:center;vertical-align:top;color:#333\">{}</td>",
                    row.row_number,
                ));
                for (col, val) in sched.columns.iter().zip(row.values.iter()) {
                    let display = if col.is_amount {
                        parse_amount(val)
                            .map(|amount| format_amount(amount, slug))
                            .unwrap_or_else(|| val.clone())
                    } else {
                        val.clone()
                    };
                    let style = if col.is_amount {
                        &amount_cell_style
                    } else {
                        &text_cell_style
                    };
                    out.push_str(&format!(
                        "<td style=\"{style}\">{}</td>",
                        escape_html(&display),
                    ));
                }
                out.push_str("</tr>");
            }

            if chunk.is_last_chunk_of_schedule
                && sched.has_total
                && sched.columns.iter().any(|c| c.subtotal.is_some())
            {
                out.push_str("<tr style=\"background:#f7f7f7;font-weight:700\">");
                out.push_str(
                    "<td style=\"border:0.75pt solid #000;padding:3.5pt 3pt;text-align:center\"></td>",
                );
                let mut labeled = false;
                for col in &sched.columns {
                    if let Some(sub) = &col.subtotal {
                        out.push_str(&format!(
                            "<td style=\"border:0.75pt solid #000;padding:3.5pt 4pt;text-align:right;white-space:nowrap;font-variant-numeric:tabular-nums\">{}</td>",
                            escape_html(sub),
                        ));
                    } else if !labeled {
                        labeled = true;
                        out.push_str(
                            "<td style=\"border:0.75pt solid #000;padding:3.5pt 4pt;text-align:left\">Subtotal (additional rows)</td>",
                        );
                    } else {
                        out.push_str(
                            "<td style=\"border:0.75pt solid #000;padding:3.5pt 4pt\"></td>",
                        );
                    }
                }
                out.push_str("</tr>");
            }

            out.push_str("</tbody></table></div>");
        }

        // Truncated text entries on this page
        if !page.truncated_entries.is_empty() {
            let section_heading = if page.truncated_is_continuation {
                "Entries continued from the form (continued)"
            } else {
                "Entries continued from the form"
            };
            out.push_str("<div style=\"margin-bottom:14pt\">");
            out.push_str(&format!(
                "<div style=\"display:flex;justify-content:space-between;align-items:baseline;margin-bottom:4pt\">\
                 <div style=\"font-size:9.5pt;font-weight:700\">{section_heading}</div>\
                 <div style=\"font-size:8.6pt;color:#444\">Full text of entries shortened with &quot;...&quot; on the form</div>\
                 </div>",
            ));
            out.push_str(
                "<table style=\"width:100%;border-collapse:collapse;table-layout:fixed;font-size:8.6pt\"><thead><tr>\
                 <th style=\"width:10%;border:0.75pt solid #000;background:#f2f2f2;padding:3pt 4pt;text-align:center;font-weight:700;vertical-align:bottom\">Page</th>\
                 <th style=\"width:32%;border:0.75pt solid #000;background:#f2f2f2;padding:3pt 4pt;text-align:left;font-weight:700;vertical-align:bottom\">Item / Field</th>\
                 <th style=\"width:58%;border:0.75pt solid #000;background:#f2f2f2;padding:3pt 4pt;text-align:left;font-weight:700;vertical-align:bottom\">Full Entry</th>\
                 </tr></thead><tbody>",
            );
            for entry in &page.truncated_entries {
                out.push_str(&format!(
                    "<tr>\
                     <td style=\"border:0.75pt solid #000;padding:3pt 4pt;text-align:center;vertical-align:top;color:#333\">Page {}</td>\
                     <td style=\"border:0.75pt solid #000;padding:3pt 4pt;text-align:left;vertical-align:top;overflow-wrap:break-word;font-weight:600\">{}</td>\
                     <td style=\"border:0.75pt solid #000;padding:3pt 4pt;text-align:left;vertical-align:top;overflow-wrap:anywhere;word-break:break-word;white-space:pre-wrap\">{}</td>\
                     </tr>",
                    entry.page_number,
                    escape_html(&entry.label),
                    escape_html(&entry.full_text),
                ));
            }
            out.push_str("</tbody></table></div>");
        }

        out.push_str("</div>");
    }

    out
}

struct PlainPagePlan<'a> {
    schedule_chunks: Vec<ScheduleChunk<'a>>,
    truncated_entries: Vec<&'a TruncatedEntry>,
    truncated_is_continuation: bool,
}

fn paginate_plain_pages<'a>(
    schedules: &'a [OverflowSchedule],
    truncated_entries: &'a [TruncatedEntry],
    max_rows_per_page: usize,
) -> Vec<PlainPagePlan<'a>> {
    let mut pages: Vec<PlainPagePlan<'a>> = Vec::new();
    let mut current_scheds: Vec<ScheduleChunk<'a>> = Vec::new();
    let mut used_rows = 0usize;

    for sched in schedules {
        let mut offset = 0usize;
        while offset < sched.rows.len() {
            let header_cost = 2usize;
            if !current_scheds.is_empty() && used_rows + header_cost + 1 > max_rows_per_page {
                pages.push(PlainPagePlan {
                    schedule_chunks: current_scheds,
                    truncated_entries: Vec::new(),
                    truncated_is_continuation: false,
                });
                current_scheds = Vec::new();
                used_rows = 0;
            }
            let avail = max_rows_per_page
                .saturating_sub(used_rows + header_cost)
                .max(1);
            let end = (offset + avail).min(sched.rows.len());
            let is_last = end == sched.rows.len();
            current_scheds.push(ScheduleChunk {
                schedule: sched,
                rows: &sched.rows[offset..end],
                is_last_chunk_of_schedule: is_last,
            });
            used_rows += header_cost + (end - offset);
            offset = end;
        }
    }

    if truncated_entries.is_empty() {
        if !current_scheds.is_empty() {
            pages.push(PlainPagePlan {
                schedule_chunks: current_scheds,
                truncated_entries: Vec::new(),
                truncated_is_continuation: false,
            });
        }
        return pages;
    }

    let mut current_truncated: Vec<&'a TruncatedEntry> = Vec::new();
    let mut started_truncated = false;
    let mut current_is_cont = false;

    for entry in truncated_entries {
        let entry_cost = ((entry.full_text.chars().count() / 70) + 1).clamp(1, 6);
        let header_cost = if current_truncated.is_empty() { 2 } else { 0 };
        let has_content = !current_scheds.is_empty() || !current_truncated.is_empty();
        if has_content && used_rows + header_cost + entry_cost > max_rows_per_page {
            pages.push(PlainPagePlan {
                schedule_chunks: current_scheds,
                truncated_entries: current_truncated,
                truncated_is_continuation: current_is_cont,
            });
            current_scheds = Vec::new();
            current_truncated = Vec::new();
            used_rows = 0;
            current_is_cont = started_truncated;
        }
        if current_truncated.is_empty() {
            used_rows += 2;
        }
        current_truncated.push(entry);
        started_truncated = true;
        used_rows += entry_cost;
    }

    if !current_scheds.is_empty() || !current_truncated.is_empty() {
        pages.push(PlainPagePlan {
            schedule_chunks: current_scheds,
            truncated_entries: current_truncated,
            truncated_is_continuation: current_is_cont,
        });
    }

    pages
}

fn split_page_divs(html: &str) -> Vec<&str> {
    let marker = "<div class=\"page ";
    let mut starts = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = html[pos..].find(marker) {
        let abs = pos + rel;
        starts.push(abs);
        pos = abs + marker.len();
    }
    let mut slices = Vec::with_capacity(starts.len());
    for i in 0..starts.len() {
        let start = starts[i];
        let end = starts.get(i + 1).copied().unwrap_or(html.len());
        slices.push(&html[start..end]);
    }
    slices
}

fn load_cell_printed_labels(slug: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(raw) = frozen_html::writer_cells_json(slug) else {
        return map;
    };
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return map;
    };
    for section in ["joins", "money_joins", "peso_joins", "xbox_joins", "boxes"] {
        let Some(arr) = parsed.get(section).and_then(Value::as_array) else {
            continue;
        };
        for item in arr {
            let printed = item
                .get("printed")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            if printed.is_empty() {
                continue;
            }
            if let Some(id) = item.get("html_id").and_then(Value::as_str) {
                map.entry(id.to_string())
                    .or_insert_with(|| printed.to_string());
            }
            if let Some(ids) = item.get("html_ids").and_then(Value::as_array) {
                for id in ids.iter().filter_map(Value::as_str) {
                    map.entry(id.to_string())
                        .or_insert_with(|| printed.to_string());
                }
            }
            for key in ["peso_id", "cents_id"] {
                if let Some(id) = item.get(key).and_then(Value::as_str) {
                    map.entry(id.to_string())
                        .or_insert_with(|| printed.to_string());
                }
            }
        }
    }
    map
}

fn resolve_truncated_label(
    cell_id: &str,
    page_html: &str,
    cell_labels: &BTreeMap<String, String>,
    overflow: &OverflowReport,
) -> String {
    // Cloned band continuation cell: `cont_s{sched}_p{page}_b{band}_r{k}_{orig_cell_id}`
    if let Some(rest) = cell_id.strip_prefix("cont_s") {
        let parts: Vec<&str> = rest.splitn(5, '_').collect();
        if parts.len() == 5 {
            let sched_idx = parts[0].parse::<usize>().ok();
            let orig_cell_id = parts[4];
            if let Some(raw_printed) = cell_labels.get(orig_cell_id) {
                let mut formatted = format_printed_label(raw_printed);
                if let Some(row_num) =
                    find_cloned_row_number(cell_id, page_html, sched_idx, overflow)
                {
                    formatted = replace_row_number_in_label(&formatted, row_num);
                }
                return formatted;
            }
        }
    }

    if let Some(raw_printed) = cell_labels.get(cell_id) {
        return format_printed_label(raw_printed);
    }

    if let Some(caption) = nearest_printed_caption(page_html, cell_id) {
        return caption;
    }

    "Form entry".to_string()
}

fn find_cloned_row_number(
    cell_id: &str,
    _page_html: &str,
    sched_idx: Option<usize>,
    overflow: &OverflowReport,
) -> Option<usize> {
    let sched = overflow.schedules.get(sched_idx?)?;
    // Parse `cont_s{sched}_p{page}_b{band}_r{k}_{orig}` -> row index `k` within the page;
    // when all rows are on page 0 (common case), `sched.rows.get(k)` gives the exact row number.
    let rest = cell_id.strip_prefix("cont_s")?;
    let parts: Vec<&str> = rest.splitn(5, '_').collect();
    if parts.len() != 5 {
        return None;
    }
    let k = parts[3].strip_prefix('r')?.parse::<usize>().ok()?;
    sched.rows.get(k).map(|r| r.row_number)
}

fn replace_row_number_in_label(label: &str, row_num: usize) -> String {
    if let Some(pos) = label.find("row ") {
        let after = &label[pos + 4..];
        let digits_len = after.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits_len > 0 {
            return format!("{}row {row_num}{}", &label[..pos], &after[digits_len..]);
        }
    }
    label.to_string()
}

/// Turn a `writer-cells.json` `"printed"` string into a clean official-form
/// item label (e.g. `"Schedule 1.B, row 3, Name of Corporate Stock (a)"`,
/// `"Part V, Item 7 — Foreign Tax Credits (specify)"`, or
/// `"Item 17 — Other Tax Credit/Payment (specify)"`).
fn format_printed_label(raw: &str) -> String {
    let s = raw.trim();
    // Pattern: "<column> (<Schedule ... row N>)" -> "<Schedule ...>, row N, <column>"
    if let Some(stripped) = s.strip_suffix(')') {
        if let Some(open) = stripped.rfind(" (") {
            let col = stripped[..open].trim();
            let ctx = stripped[open + 2..].trim();
            if let Some(row_pos) = ctx.find(" row ") {
                let sched = ctx[..row_pos].trim();
                let row_part = ctx[row_pos + 1..].trim();
                if !sched.is_empty() && !col.is_empty() {
                    return format!("{sched}, {row_part}, {col}");
                }
            }
        }
    }
    // Pattern: "Schedule <X> row <N> <col>" -> "Schedule <X>, row <N>, <col>"
    if s.starts_with("Schedule ") {
        if let Some(row_pos) = s.find(" row ") {
            let sched = s[..row_pos].trim();
            let after_row = &s[row_pos + 5..];
            let digits_len = after_row.bytes().take_while(|b| b.is_ascii_digit()).count();
            if digits_len > 0 {
                let row_num = &after_row[..digits_len];
                let col = after_row[digits_len..].trim();
                if !col.is_empty() {
                    return format!("{sched}, row {row_num}, {col}");
                }
            }
        }
    }
    // Pattern: "Part <ROMAN> <ITEM> <rest>" -> "Part <ROMAN>, Item <ITEM> — <rest>"
    if let Some(after_part) = s.strip_prefix("Part ") {
        let mut tokens = after_part.splitn(3, ' ');
        if let (Some(roman), Some(item_tok), Some(rest)) =
            (tokens.next(), tokens.next(), tokens.next())
        {
            if item_tok.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
                return format!("Part {roman}, Item {item_tok} — {}", rest.trim());
            }
        }
    }
    // Pattern: "<ITEM> <rest>" where ITEM starts with a digit (e.g. "17 Other ...")
    if let Some((first, rest)) = s.split_once(' ') {
        if first.bytes().next().is_some_and(|b| b.is_ascii_digit())
            && first.len() <= 4
            && !rest.trim().is_empty()
        {
            return format!("Item {first} — {}", rest.trim());
        }
    }
    s.to_string()
}

/// Fallback when a cell has no `writer-cells.json` `"printed"` entry: find the
/// nearest printed `<div class="t ...">` caption on the same page by geometry.
fn nearest_printed_caption(page_html: &str, cell_id: &str) -> Option<String> {
    let id_needle = format!("id=\"{cell_id}\"");
    let cell_pos = page_html.find(&id_needle)?;
    let div_start = page_html[..cell_pos].rfind('<')?;
    let div_end = cell_pos + page_html[cell_pos..].find('>')?;
    let cell_tag = &page_html[div_start..=div_end];
    let (cell_left, cell_top) = parse_left_top_pt(cell_tag)?;

    let mut best: Option<(f64, String)> = None;
    let mut fallback_any: Option<(f64, String)> = None;
    let mut pos = 0usize;
    while let Some(rel) = page_html[pos..].find("<div class=\"t") {
        let start = pos + rel;
        let next_byte = page_html.as_bytes().get(start + 13).copied();
        if !matches!(next_byte, Some(b' ' | b'"')) {
            pos = start + 13;
            continue;
        }
        let Some(gt_rel) = page_html[start..].find('>') else {
            break;
        };
        let open_end = start + gt_rel;
        let Some(close_rel) = page_html[open_end + 1..].find("</div>") else {
            break;
        };
        let open_tag = &page_html[start..=open_end];
        let inner = &page_html[open_end + 1..open_end + 1 + close_rel];
        pos = open_end + 1 + close_rel + 6;

        let text = strip_tags_and_unescape(inner);
        let text = text.trim();
        if text.len() < 2 || !text.chars().any(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        let Some((t_left, t_top)) = parse_left_top_pt(open_tag) else {
            continue;
        };
        let dx = cell_left - t_left;
        let dy = cell_top - t_top;
        let dist = dx.abs() * 0.5 + dy.abs();
        if fallback_any.as_ref().is_none_or(|(d, _)| dist < *d) {
            fallback_any = Some((dist, text.to_string()));
        }
        // Prefer captions above or to the left of the cell
        if dy >= -4.0 && dx >= -12.0 && best.as_ref().is_none_or(|(d, _)| dist < *d) {
            best = Some((dist, text.to_string()));
        }
    }

    best.or(fallback_any).map(|(_, text)| text)
}

fn parse_left_top_pt(tag: &str) -> Option<(f64, f64)> {
    let style_pos = tag.find("style=\"")?;
    let rest = &tag[style_pos + 7..];
    let style_end = rest.find('"')?;
    let style = &rest[..style_end];
    let mut left = None;
    let mut top = None;
    for decl in style.split(';') {
        let Some((k, v)) = decl.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim().strip_suffix("pt").unwrap_or(v.trim());
        if key == "left" {
            left = val.parse::<f64>().ok();
        } else if key == "top" {
            top = val.parse::<f64>().ok();
        }
    }
    Some((left?, top?))
}

fn strip_tags_and_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

struct ScheduleChunk<'a> {
    schedule: &'a OverflowSchedule,
    rows: &'a [OverflowRow],
    is_last_chunk_of_schedule: bool,
}

fn collect_not_joined_keys(parsed: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(arr) = parsed.get("not_joined").and_then(Value::as_array) {
        for entry in arr {
            if let Some(k) = entry.get("writer_key").and_then(Value::as_str) {
                keys.insert(k.to_string());
            }
            if let Some(ks) = entry.get("writer_keys").and_then(Value::as_array) {
                for k in ks.iter().filter_map(Value::as_str) {
                    keys.insert(k.to_string());
                }
            }
        }
    }
    keys
}

struct RawJoin {
    writer_key: String,
    printed: String,
    is_money_section: bool,
}

fn discover_schedules(parsed: &Value) -> Vec<ScheduleSpec> {
    let mut raw_joins = Vec::new();
    for (section, is_money) in [
        ("joins", false),
        ("money_joins", true),
        ("peso_joins", true),
    ] {
        if let Some(arr) = parsed.get(section).and_then(Value::as_array) {
            for item in arr {
                let Some(wk) = item.get("writer_key").and_then(Value::as_str) else {
                    continue;
                };
                let printed = item
                    .get("printed")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                raw_joins.push(RawJoin {
                    writer_key: wk.to_string(),
                    printed,
                    is_money_section: is_money,
                });
            }
        }
    }

    // Group row-like joins by (schedule_heading, inferred_key_group) so:
    // - stems ending in a fixed column digit (`txtFMVCol1` + `0..4`,
    //   `txtFMVCol2` + `0..4`) never collide, AND
    // - a column whose row-1 caption is fuller than row-2..N (e.g.
    //   `Sub-total (between Columns 1 & 2)` vs `Sub-total`) stays one column
    //   with row 1's full official heading.
    struct ColGroup {
        first_order: usize,
        schedule_heading: String,
        col_heading: String,
        group_key: String,
        is_money_section: bool,
        /// (1-based printed row number from caption, writer_key)
        entries: Vec<(usize, String)>,
    }

    let mut col_groups: Vec<ColGroup> = Vec::new();
    for (order, join) in raw_joins.iter().enumerate() {
        if join.writer_key.starts_with("derived:") {
            continue;
        }
        if !looks_like_schedule_row_caption(&join.printed) {
            continue;
        }
        let Some((schedule_heading, row_num, col_heading)) =
            parse_schedule_row_and_column(&join.printed)
        else {
            continue;
        };
        if schedule_heading.is_empty() || col_heading.is_empty() {
            continue;
        }
        let group_key = infer_column_group_key(&join.writer_key, row_num, &col_heading);
        if let Some(existing) = col_groups.iter_mut().find(|g| {
            g.schedule_heading.eq_ignore_ascii_case(&schedule_heading)
                && g.group_key.eq_ignore_ascii_case(&group_key)
        }) {
            existing.entries.push((row_num, join.writer_key.clone()));
            existing.is_money_section |= join.is_money_section;
            if row_num == 1 && col_heading.len() > existing.col_heading.len() {
                existing.col_heading = col_heading;
            }
        } else {
            col_groups.push(ColGroup {
                first_order: order,
                schedule_heading,
                col_heading,
                group_key,
                is_money_section: join.is_money_section,
                entries: vec![(row_num, join.writer_key.clone())],
            });
        }
    }

    // Derive exact (stem, start_idx, end_idx) from the longest common prefix
    // of each column's writer keys.
    struct DiscoveredCol {
        first_order: usize,
        schedule_heading: String,
        col: ColumnSpec,
    }

    let mut discovered_cols = Vec::new();
    for mut group in col_groups {
        if group.entries.len() < 2 {
            continue;
        }
        group.entries.sort_by_key(|(r, _)| *r);
        let keys: Vec<&str> = group.entries.iter().map(|(_, k)| k.as_str()).collect();
        let Some((stem, start_idx, end_idx)) = derive_stem_and_range(&keys) else {
            continue;
        };
        let is_amount = group.is_money_section || infer_is_amount(&group.col_heading, &stem);
        discovered_cols.push(DiscoveredCol {
            first_order: group.first_order,
            schedule_heading: group.schedule_heading,
            col: ColumnSpec {
                stem,
                heading: group.col_heading,
                is_amount,
                start_idx,
                end_idx,
            },
        });
    }

    discovered_cols.sort_by_key(|c| c.first_order);

    // Group columns into schedules in order of first appearance.
    let mut schedules: Vec<ScheduleSpec> = Vec::new();
    for dc in discovered_cols {
        if let Some(existing) = schedules
            .iter_mut()
            .find(|s| s.heading.eq_ignore_ascii_case(&dc.schedule_heading))
        {
            existing.columns.push(dc.col);
        } else {
            let has_total = schedule_has_total(&raw_joins, &dc.schedule_heading, &dc.col.stem);
            schedules.push(ScheduleSpec {
                heading: dc.schedule_heading,
                columns: vec![dc.col],
                has_total,
            });
        }
    }

    schedules
}

fn schedule_has_total(raw_joins: &[RawJoin], schedule_heading: &str, sample_stem: &str) -> bool {
    // Check if any join in the same schedule namespace or with a matching
    // schedule caption mentions "total" (excluding per-row "sub-total").
    let ns_prefix = sample_stem
        .rfind(':')
        .map(|i| &sample_stem[..=i])
        .unwrap_or("");
    let sched_lower = schedule_heading.to_ascii_lowercase();

    for j in raw_joins {
        let key_lower = j.writer_key.to_ascii_lowercase();
        let pr_lower = j.printed.to_ascii_lowercase();
        let in_same_ns = !ns_prefix.is_empty() && j.writer_key.starts_with(ns_prefix);
        let mentions_sched = pr_lower.contains(&sched_lower);
        let is_total_key = (key_lower.contains("total") && !key_lower.contains("subtot"))
            || (pr_lower.contains("total") && !pr_lower.contains("sub-total"));
        if is_total_key && (in_same_ns || mentions_sched) {
            return true;
        }
    }
    // Also default to true if the schedule has at least one amount column and
    // a matching `not_joined` total exists (e.g. 1600WP Schedule 1.1 carries
    // its total in Part II); wait — for 1600WP the official Schedule 1.1 has no
    // total box, so returning false here accurately matches the official sheet!
    false
}

fn infer_column_group_key(writer_key: &str, row_num: usize, col_heading: &str) -> String {
    if row_num >= 1 {
        let zero_based = (row_num - 1).to_string();
        if let Some(stem) = writer_key.strip_suffix(&zero_based) {
            return format!("stem0:{stem}");
        }
        let one_based = row_num.to_string();
        if let Some(stem) = writer_key.strip_suffix(&one_based) {
            return format!("stem1:{stem}");
        }
    }
    format!("col:{col_heading}")
}

fn derive_stem_and_range(keys: &[&str]) -> Option<(String, usize, usize)> {
    if keys.len() < 2 {
        return None;
    }
    // Longest common prefix of all keys, then step back if needed so that
    // every remaining suffix is a non-empty ASCII integer (e.g. for
    // `txtDatePurchase10`, `txtDatePurchase11`, `txtDatePurchase12`, LCP is
    // `txtDatePurchase1`, which leaves `0, 1, 2` — still valid; if keys span
    // `9` and `10`, LCP stops before `9`/`1`, leaving `9, 10`).
    let first = keys[0].as_bytes();
    let mut prefix_len = first.len();
    for k in &keys[1..] {
        let kb = k.as_bytes();
        prefix_len = prefix_len.min(kb.len());
        let mut i = 0;
        while i < prefix_len && first[i] == kb[i] {
            i += 1;
        }
        prefix_len = i;
    }
    // If the LCP splits a multi-digit row index (e.g. `txtDatePurchase10..13`
    // has LCP `txtDatePurchase1`), check whether stepping back 0 or 1 digits
    // yields consecutive indices. Prefer the longest prefix where all suffixes
    // parse as consecutive non-negative integers (except step back when a
    // suffix would be empty).
    loop {
        let stem = &keys[0][..prefix_len];
        let mut indices: Vec<usize> = Vec::with_capacity(keys.len());
        let mut all_ok = true;
        for k in keys {
            let suffix = &k[prefix_len..];
            if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
                all_ok = false;
                break;
            }
            match suffix.parse::<usize>() {
                Ok(n) => indices.push(n),
                Err(_) => {
                    all_ok = false;
                    break;
                }
            }
        }
        if all_ok {
            indices.sort_unstable();
            indices.dedup();
            let start = indices[0];
            let end = *indices.last().unwrap();
            if indices.len() >= 2 && end - start + 1 == indices.len() {
                return Some((stem.to_string(), start, end));
            }
        }
        if prefix_len == 0 || !first[prefix_len - 1].is_ascii_digit() {
            return None;
        }
        prefix_len -= 1;
    }
}

fn match_stem_index(key: &str, stem: &str) -> Option<usize> {
    let rest = key.strip_prefix(stem)?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse::<usize>().ok()
}

fn looks_like_schedule_row_caption(printed: &str) -> bool {
    let lower = printed.to_ascii_lowercase();
    lower.contains(" row ") && (lower.contains("schedule ") || lower.contains("part "))
}

/// Extract `(schedule_heading, row_number, column_heading)` from a
/// `writer-cells.json` `printed` caption. Handles both common shapes:
/// - `"OCT/TCT/CCT No. (Schedule 1.A row 1)"` -> `("Schedule 1.A", 1, "OCT/TCT/CCT No.")`
/// - `"Schedule A row 1 Particulars"` -> `("Schedule A", 1, "Particulars")`
/// - `"Schedule 1.1 row 1 Individual/Corporation"` -> `("Schedule 1.1", 1, "Individual/Corporation")`
/// - `"Schedule 1 continuation row 1 FMV per TD"` -> `("Schedule 1", 1, "FMV per TD")`
pub(crate) fn parse_schedule_row_and_column_pub(printed: &str) -> Option<(String, usize, String)> {
    parse_schedule_row_and_column(printed)
}

fn parse_schedule_row_and_column(printed: &str) -> Option<(String, usize, String)> {
    let trimmed = printed.trim();

    // Shape 1: "<column> (Schedule <X> [continuation] row <N>)"
    if let Some(open) = trimmed.rfind('(') {
        if trimmed.ends_with(')') {
            let inside = trimmed[open + 1..trimmed.len() - 1].trim();
            if let Some((sched, row_num, _)) = extract_schedule_row_and_after(inside) {
                let col = trimmed[..open].trim().to_string();
                return Some((sched, row_num, col));
            }
        }
    }

    // Shape 2: "Schedule <X> [continuation] row <N> <column>"
    if let Some((sched, row_num, after_row)) = extract_schedule_row_and_after(trimmed) {
        let col = after_row
            .trim_start_matches(|c: char| c == ':' || c == '-' || c == ',' || c.is_whitespace())
            .to_string();
        return Some((sched, row_num, col));
    }

    None
}

fn extract_schedule_row_and_after(s: &str) -> Option<(String, usize, &str)> {
    let lower = s.to_ascii_lowercase();
    let row_pos = lower.find(" row ")?;
    let before = s[..row_pos].trim();
    let after = s[row_pos + 5..].trim_start();
    let digits_end = after.bytes().take_while(|b| b.is_ascii_digit()).count();
    let (row_num, rest) = if digits_end > 0 {
        let n = after[..digits_end].parse::<usize>().ok()?;
        (n, after[digits_end..].trim_start())
    } else if after
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_uppercase())
        && after
            .as_bytes()
            .get(1)
            .is_none_or(|b| !b.is_ascii_alphanumeric())
    {
        // Lettered rows such as "Schedule 2 row A Name of Corporate Stock" (1707).
        let n = (after.as_bytes()[0] - b'A') as usize + 1;
        (n, after[1..].trim_start())
    } else {
        return None;
    };
    // Normalize trailing "continuation" so Schedule 1 and Schedule 1 continuation merge
    let sched = strip_trailing_word_ci(before, "continuation")
        .trim()
        .to_string();
    if sched.is_empty() {
        return None;
    }
    Some((sched, row_num, rest))
}

struct PopupColDef {
    /// Key template with `{n}` replaced by the 1-based popup row index.
    key_template: &'static str,
    heading: &'static str,
    is_amount: bool,
}

struct PopupScheduleDef {
    slug: &'static str,
    heading: &'static str,
    printed_capacity: usize,
    /// Continuing row number for popup index `n = 1` is `row_num_base + n`.
    row_num_base: usize,
    has_total: bool,
    columns: &'static [PopupColDef],
}

/// Complete registry of forms in `crates/bir-core/src/forms/` whose overflow
/// ("Add more..." / popup / modal) rows are emitted under keys distinct from
/// the printed row stems (`extend_layout` and friends):
/// - `1700-2018` (`form_1700.rs`): Schedule 1 (`4_{n}` employer & comp keys)
/// - `1701a-2018` (`form_1701a.rs`): Part IV-A (`txtmodalPartIVA*_{n}C1..3`) &
///   Part IV-B (`txtmodalPartIVB*_{n}C1..3`)
/// - `1702ex-2018` (`form_1702ex.rs`): 5 popups (`txtPg3Pt6{S1I17,S2I4,S3I3,S3I6,S3I8}_{n}Col*`)
/// - `1702mx-2018c` (`form_1702mx_official.rs`): Schedule 6 (`txtPg6Sc6_{n}Col1..6`)
/// - `1702rt-2018c` (`form_1702rt_official.rs`): 6 modals (`<rowName>.{n}C1..3`)
/// - `1707-2021` (`form_1707.rs`): Schedule 2 (`txtPg2Pt4S2_{k}Col1..4`) &
///   Schedule 3 (`txtPg2Pt4S3_{k}Col1..2`)
/// - `1707a-2021` (`form_1707a.rs`): Schedule 1 (`txtS1C*_{k}`) & Schedule 2 (`txtS2C*_{k}`)
const POPUP_SCHEDULES: &[PopupScheduleDef] = &[
    // 1707
    PopupScheduleDef {
        slug: "1707-2021",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col1",
                heading: "Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col2",
                heading: "No. of Shares",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col3",
                heading: "Stock Certificate Number",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col4",
                heading: "Taxable Base/Selling Price",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1707-2021",
        heading: "Schedule 3",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S3_{n}Col1",
                heading: "Particulars",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S3_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    // 1701A
    PopupScheduleDef {
        slug: "1701a-2018",
        heading: "Part IV-A Other Non-Operating Income",
        printed_capacity: 2,
        row_num_base: 41,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVADesc_{n}C1",
                heading: "Other Non-Operating Income (specify)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVA_{n}C2",
                heading: "A) Taxpayer/Filer",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVA_{n}C3",
                heading: "B) Spouse",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1701a-2018",
        heading: "Part IV-B Other Non-Operating Income",
        printed_capacity: 2,
        row_num_base: 50,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVBDesc_{n}C1",
                heading: "Other Non-Operating Income (specify)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVB_{n}C2",
                heading: "A) Taxpayer/Filer",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVB_{n}C3",
                heading: "B) Spouse",
                is_amount: true,
            },
        ],
    },
    // 1707A
    PopupScheduleDef {
        slug: "1707a-2021",
        heading: "Schedule 1",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C1DateOfTransaction_{n}",
                heading: "(A) Date of Transaction (MM/DD/YYYY)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C2NameOfCorporateStock_{n}",
                heading: "(B) Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C3SellingPrice_{n}",
                heading: "(C) Selling Price",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C4Cost_{n}",
                heading: "(D) Cost",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C5CapitalGains_{n}",
                heading: "(E = C-D) Capital Gains",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C6TaxPaid_{n}",
                heading: "(F) Tax Paid",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1707a-2021",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C1DateOfTransaction_{n}",
                heading: "(A) Date of Transaction (MM/DD/YYYY)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C2NameOfCorporateStock_{n}",
                heading: "(B) Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C3SellingPrice_{n}",
                heading: "(C) Selling Price",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C4Cost_{n}",
                heading: "(D) Cost",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C5CapitalLoss_{n}",
                heading: "(E = C-D) Capital Loss",
                is_amount: true,
            },
        ],
    },
    // 1700
    PopupScheduleDef {
        slug: "1700-2018",
        heading: "Schedule 1",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerName1",
                heading: "Name of Employer",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerName2",
                heading: "Name of Employer (line 2)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN1",
                heading: "TIN (1)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN2",
                heading: "TIN (2)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN3",
                heading: "TIN (3)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerBranchCode",
                heading: "Branch Code",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1c_4_{n}REG",
                heading: "Gross Compensation Income (Regular)",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1d_4_{n}CIFR",
                heading: "Compensation Income (Subject to Final/Other)",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1e_4_{n}TW",
                heading: "Tax Withheld",
                is_amount: true,
            },
        ],
    },
    // 1702EX
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 1 Item 17",
        printed_capacity: 6,
        row_num_base: 5,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S1I17_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S1I17_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col3",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 3",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I3_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I3_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 6",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I6_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I6_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 8",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I8_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I8_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    // 1702MX
    PopupScheduleDef {
        slug: "1702mx-2018c",
        heading: "Schedule 6",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col3",
                heading: "Column A",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col4",
                heading: "Column B",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col5",
                heading: "Column C",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col6",
                heading: "Column D",
                is_amount: true,
            },
        ],
    },
    // 1702RT
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Part IV Item 54",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg2Pt4I54.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg2Pt4I54.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule I Item 17i",
        printed_capacity: 6,
        row_num_base: 5,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc1I17i.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc1I17i.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule II",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C3",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 3",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I3.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I3.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 6",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I6.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I6.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 8",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I8.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I8.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
];

fn match_template_index(key: &str, template: &str) -> Option<usize> {
    let (prefix, suffix) = template.split_once("{n}")?;
    let rest = key.strip_prefix(prefix)?.strip_suffix(suffix)?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse::<usize>().ok()
}

fn detect_popup_overflow(
    slug: &str,
    fields: &BTreeMap<String, String>,
    unprinted: &BTreeSet<String>,
) -> Vec<OverflowSchedule> {
    let mut out = Vec::new();
    for def in POPUP_SCHEDULES {
        if def.slug != slug {
            continue;
        }
        let mut indices: BTreeSet<usize> = BTreeSet::new();
        for col in def.columns {
            for key in unprinted {
                if let Some(n) = match_template_index(key, col.key_template) {
                    indices.insert(n);
                }
            }
        }
        if indices.is_empty() {
            continue;
        }
        let mut rows = Vec::new();
        let mut col_sums: Vec<f64> = vec![0.0; def.columns.len()];
        let mut col_has_numeric: Vec<bool> = vec![false; def.columns.len()];
        for n in indices {
            let mut values = Vec::with_capacity(def.columns.len());
            let mut keys = Vec::with_capacity(def.columns.len());
            let mut has_meaningful = false;
            for col in def.columns {
                let key = col.key_template.replace("{n}", &n.to_string());
                let val = fields
                    .get(&key)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if is_meaningful_cell(&val) {
                    has_meaningful = true;
                }
                values.push(val);
                keys.push(key);
            }
            if !has_meaningful {
                continue;
            }
            for (ci, (col, val)) in def.columns.iter().zip(values.iter()).enumerate() {
                if col.is_amount {
                    if let Some(num) = parse_amount(val) {
                        col_sums[ci] += num;
                        col_has_numeric[ci] = true;
                    }
                }
            }
            rows.push(OverflowRow {
                row_number: def.row_num_base + n,
                values,
                keys,
            });
        }
        if rows.is_empty() {
            continue;
        }
        let columns = def
            .columns
            .iter()
            .enumerate()
            .map(|(ci, col)| {
                let subtotal = if def.has_total && col.is_amount && col_has_numeric[ci] {
                    Some(format_amount(col_sums[ci], slug))
                } else {
                    None
                };
                OverflowColumn {
                    heading: col.heading.to_string(),
                    is_amount: col.is_amount,
                    subtotal,
                }
            })
            .collect();
        out.push(OverflowSchedule {
            heading: def.heading.to_string(),
            printed_capacity: def.printed_capacity,
            columns,
            rows,
            has_total: def.has_total,
        });
    }
    out
}

fn strip_trailing_word_ci<'a>(s: &'a str, word: &str) -> &'a str {
    let lower = s.to_ascii_lowercase();
    if lower.ends_with(word) {
        let cut = s.len() - word.len();
        if cut == 0 || s.as_bytes()[cut - 1].is_ascii_whitespace() {
            return s[..cut].trim_end();
        }
    }
    s
}

fn infer_is_amount(heading: &str, stem: &str) -> bool {
    let h = heading.to_ascii_lowercase();
    let s = stem.to_ascii_lowercase();

    // Non-amount columns even if they contain words like "tax" or "no."
    if h.contains("declaration")
        || h.contains("tax rate")
        || h.ends_with("rate")
        || h.contains("area")
        || h.contains("tin")
        || h.contains("atc")
        || h.contains("seq")
        || h.contains("oct/")
        || h.contains("tct/")
        || h.contains("cert")
        || h.contains("no. of shares")
        || h.contains("life")
        || s.contains("rate")
        || s.contains("area")
        || s.contains("taxdec")
        || s.contains("tin")
        || s.contains("atc")
    {
        return false;
    }

    h.contains("amount")
        || h.contains("fair market")
        || h.contains("fmv")
        || h.contains("value")
        || h.contains("sub-total")
        || h.contains("subtotal")
        || h.contains("total")
        || h.contains("withheld")
        || h.contains("tax due")
        || h.contains("tax paid")
        || h.contains("dst paid")
        || h.contains("tax base")
        || h.contains("input tax")
        || h.contains("exclusive")
        || h.contains("conjugal")
        || h.contains("deduction")
        || h.contains("price")
        || h.contains("cost")
        || h.contains("sales")
        || h.contains("purchases")
        || s.contains("fmv")
        || s.contains("amount")
        || s.contains("withheld")
        || s.contains("taxdue")
        || s.contains("dstpaid")
        || s.contains("parval")
}

fn is_meaningful_cell(val: &str) -> bool {
    let t = val.trim();
    if t.is_empty() || t == "-" || t == "false" {
        return false;
    }
    if let Some(n) = parse_amount(t) {
        return n.abs() > 0.000_001;
    }
    true
}

fn parse_amount(val: &str) -> Option<f64> {
    let cleaned: String = val
        .trim()
        .chars()
        .filter(|c| *c != ',' && *c != ' ' && *c != '₱')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// A column total, grouped, with a negative in the form's official
/// [`frozen_html::NegativeStyle`] (brackets on 1702MX, 1702RT and 1707A,
/// a minus elsewhere).
fn format_amount(value: f64, slug: &str) -> String {
    let negative = value < -0.005;
    let abs = value.abs();
    let formatted = format!("{abs:.2}");
    let (whole, frac) = formatted.split_once('.').unwrap_or((&formatted, "00"));
    let mut with_commas = String::with_capacity(whole.len() + whole.len() / 3);
    for (i, ch) in whole.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(ch);
    }
    let whole_grouped: String = with_commas.chars().rev().collect();
    if negative {
        frozen_html::NegativeStyle::of_sheet(slug).sign(&format!("{whole_grouped}.{frac}"))
    } else {
        format!("{whole_grouped}.{frac}")
    }
}

struct HeaderMeta {
    form_number: String,
    form_title: String,
    tin: String,
    taxpayer_name: String,
    period: Option<(&'static str, String)>,
}

pub(crate) struct HeaderMetaPub {
    pub tin: String,
    pub taxpayer_name: String,
    pub period: String,
}

pub(crate) fn extract_header_meta_pub(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> HeaderMetaPub {
    let m = extract_header_meta(slug, fields);
    HeaderMetaPub {
        tin: m.tin,
        taxpayer_name: m.taxpayer_name,
        period: m.period.map(|(_, v)| v).unwrap_or_default(),
    }
}

fn extract_header_meta(slug: &str, fields: &BTreeMap<String, String>) -> HeaderMeta {
    let raw_code = slug_to_form_code(slug);
    let form_number = format!("BIR Form No. {raw_code}");
    let form_title = bir_core::forms::registry::find_form(&raw_code)
        .map(|f| f.title.to_string())
        .unwrap_or_default();

    let tin = extract_tin(fields);
    let taxpayer_name = extract_taxpayer_name(fields);

    let period = extract_return_period(slug, fields);

    HeaderMeta {
        form_number,
        form_title,
        tin,
        taxpayer_name,
        period,
    }
}

fn slug_to_form_code(slug: &str) -> String {
    // Strip trailing -YYYY or -YYYYc revision suffix
    let base = match slug.rfind('-') {
        Some(i) if slug[i + 1..].chars().take(4).all(|c| c.is_ascii_digit()) => &slug[..i],
        _ => slug,
    };
    let upper = base.to_ascii_uppercase();
    // Normalize known slug variants like "2000-DST" -> "2000"
    upper.strip_suffix("-DST").unwrap_or(&upper).to_string()
}

fn extract_tin(fields: &BTreeMap<String, String>) -> String {
    let br = first_top_level_digits_ci(
        fields,
        &[
            "txtBranchCode",
            "branchCode",
            "branchCode_1",
            "txtTIN4",
            "txtPg1TIN4",
            "txtBranch",
            "txtPg1I4BranchCode",
            "txtPg1Pt1I6BranchCode",
            "txtPg1Pt1I6TINC4",
            "BranchMaskP1",
            "txtI5BranchCode",
            "txtBranchCode1",
        ],
        5,
    )
    .unwrap_or_default();

    let mut nine_digit_fallback: Option<String> = None;
    for derived_key in [
        "derived:tin",
        "derived:tin_digits",
        "derived:tp_tin",
        "derived:seller_tin",
        "derived:buyer_tin",
    ] {
        if let Some(d) = fields.get(derived_key).filter(|v| !v.trim().is_empty()) {
            let digits: String = d.chars().filter(char::is_ascii_digit).collect();
            if digits.len() >= 12 {
                return format_tin_digits(&digits);
            }
            if digits.len() == 9 {
                let base = format!("{}-{}-{}", &digits[0..3], &digits[3..6], &digits[6..9]);
                if !br.is_empty() {
                    return format!("{base}-{br}");
                }
                if nine_digit_fallback.is_none() {
                    nine_digit_fallback = Some(base);
                }
            }
        }
    }
    let t1 = first_top_level_digits_ci(
        fields,
        &[
            "txtTIN1",
            "txtPg1TIN1",
            "tinA",
            "txtPg1I4TIN1",
            "txtPg1Pt1I6TINC1",
            "txtPg1I6SellerTIN0",
            "txtI5TIN1",
        ],
        3,
    )
    .unwrap_or_default();
    let t2 = first_top_level_digits_ci(
        fields,
        &[
            "txtTIN2",
            "txtPg1TIN2",
            "tinB",
            "txtPg1I4TIN2",
            "txtPg1Pt1I6TINC2",
            "txtPg1I6SellerTIN1",
            "txtI5TIN2",
        ],
        3,
    )
    .unwrap_or_default();
    let t3 = first_top_level_digits_ci(
        fields,
        &[
            "txtTIN3",
            "txtPg1TIN3",
            "tinC",
            "txtPg1I4TIN3",
            "txtPg1Pt1I6TINC3",
            "txtPg1I6SellerTIN2",
            "txtI5TIN3",
        ],
        3,
    )
    .unwrap_or_default();
    if !t1.is_empty() && !t2.is_empty() && !t3.is_empty() {
        if br.is_empty() {
            return format!("{t1}-{t2}-{t3}");
        }
        return format!("{t1}-{t2}-{t3}-{br}");
    }
    nine_digit_fallback.unwrap_or_else(|| "—".to_string())
}

/// Match a top-level filer TIN or branch-code key (`key` has at most one `:`
/// namespace separator, excluding schedule/row keys like `frm1600WP:dtSched:txtTin1`
/// or `frmDummy:sched1:txtTIN1`) whose trimmed value is `1..=max_digits` ASCII digits.
fn first_top_level_digits_ci(
    fields: &BTreeMap<String, String>,
    suffixes: &[&str],
    max_digits: usize,
) -> Option<String> {
    for suffix in suffixes {
        for (key, value) in fields {
            if key.matches(':').count() > 1 {
                continue;
            }
            let tail = key.rsplit(':').next().unwrap_or(key);
            if tail.eq_ignore_ascii_case(suffix) {
                let trimmed = value.trim();
                if !trimmed.is_empty()
                    && trimmed.len() <= max_digits
                    && trimmed.chars().all(|c| c.is_ascii_digit())
                {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn first_matching_field_ci(fields: &BTreeMap<String, String>, suffixes: &[&str]) -> Option<String> {
    for suffix in suffixes {
        for (key, value) in fields {
            let tail = key.rsplit(':').next().unwrap_or(key);
            if tail.eq_ignore_ascii_case(suffix) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn join_matching_fields_ci(fields: &BTreeMap<String, String>, suffixes: &[&str]) -> Option<String> {
    let mut parts = Vec::new();
    for suffix in suffixes {
        if let Some(val) = first_matching_field_ci(fields, &[suffix]) {
            parts.push(val);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn extract_taxpayer_name(fields: &BTreeMap<String, String>) -> String {
    // Single-field taxpayer / seller / withholding-agent / donor / estate name keys across all 43 forms.
    if let Some(name) = first_matching_field_ci(
        fields,
        &[
            "txtTaxpayerName",
            "registeredName",
            "taxpayerName",
            "txtDonorName",
            "txtAgentName",
            "txtWithholdingAgentName",
            "txtPg1Pt1I7WithholdingAgentsName",
            "txtWthhldngAgntsNme",
            "txtSellerName",
            "txtPg1I7TaxpayerName",
            "txtPg1I8TaxpayerName",
            "txtTaxpayerNo8a",
            "txtI8RegisteredName",
            "registeredName_1",
            "txtPg1Pt1I7RegisteredName",
            "txtPg1Pt1I6RegisteredName",
            "txtPg1Pt1I6Name",
            "txtTPName",
        ],
    ) {
        return name;
    }
    // Multi-line / split name keys (1702MX, 1702RT, 1707, 1702EX, 1702Q).
    for group in [
        &[
            "txtPg1Pt1I9RegisteredName",
            "txtPg1Pt1I9RegisteredName2",
            "txtPg1Pt1I9RegisteredName3",
        ][..],
        &["txtPg1Pt1I8Name1", "txtPg1Pt1I8Name2", "txtPg1Pt1I8Name3"][..],
        &[
            "txtPg1I6SellerName0",
            "txtPg1I6SellerName1",
            "txtPg1I6SellerName2",
        ][..],
        &["name_line1", "name_line2", "name_line3"][..],
    ] {
        if let Some(joined) = join_matching_fields_ci(fields, group) {
            return joined;
        }
    }
    "—".to_string()
}

fn format_tin_digits(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 12 {
        format!(
            "{}-{}-{}-{}",
            &digits[0..3],
            &digits[3..6],
            &digits[6..9],
            &digits[9..]
        )
    } else if digits.len() == 9 {
        format!("{}-{}-{}", &digits[0..3], &digits[3..6], &digits[6..9])
    } else {
        raw.trim().to_string()
    }
}

fn period_header_label(slug: &str) -> &'static str {
    let s = slug.to_ascii_lowercase();
    if s.starts_with("2000-ot")
        || s.starts_with("1606")
        || s.starts_with("1706")
        || s.starts_with("1707-")
        || s.starts_with("2552")
    {
        "Date of Transaction"
    } else if s.starts_with("1800") {
        "Date of Donation"
    } else if s.starts_with("1801") {
        "Date of Death"
    } else if s.starts_with("1600wp") {
        "Date of Withholding"
    } else if s.starts_with("1707a") {
        "Year Ended"
    } else if s.starts_with("2200") {
        "Date"
    } else {
        "Return Period"
    }
}

fn is_valid_date_part(val: &str) -> bool {
    let t = val.trim();
    !t.is_empty() && !t.chars().all(|c| c == '0')
}

fn extract_return_period(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> Option<(&'static str, String)> {
    let label = period_header_label(slug);
    // 1. Explicit date (transaction date / date of donation / date of withholding)
    if let Some(d) = fields
        .get("derived:from_date")
        .filter(|v| is_valid_date_part(v))
    {
        return Some((label, format_mmddyyyy(d)));
    }
    let m = first_matching_field(
        fields,
        &[
            "txtDateMonth",
            "DateWithholdingMonth",
            "txtPg1I1Month",
            "ddlPg1I2Date",
            "txtMonth1",
            "txtMonth",
            "rtnMonth",
        ],
    )
    .filter(|v| is_valid_date_part(v));
    let d = first_matching_field(
        fields,
        &[
            "txtDateDay",
            "DateWithholdingDay",
            "txtPg1I1Day",
            "txtDay",
            "txtDate",
        ],
    )
    .filter(|v| is_valid_date_part(v));
    let y = first_matching_field(
        fields,
        &[
            "txtDateYear",
            "DateWithholdingYear",
            "txtPg1I1Year",
            "txtPg1I2YearEnd",
            "txtYear",
            "txtForYr",
            "rtnYear",
            "txtYearEnded",
        ],
    )
    .filter(|v| is_valid_date_part(v));

    if let (Some(mm), Some(dd), Some(yyyy)) = (&m, &d, &y) {
        return Some((label, format!("{mm}/{dd}/{yyyy}")));
    }
    if let (Some(mm), Some(yyyy)) = (&m, &y) {
        return Some((label, format!("{mm}/{yyyy}")));
    }
    if let Some(yyyy) = y {
        return Some((label, yyyy));
    }
    None
}

fn format_mmddyyyy(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 8 {
        format!("{}/{}/{}", &digits[0..2], &digits[2..4], &digits[4..8])
    } else {
        raw.trim().to_string()
    }
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn first_matching_field(fields: &BTreeMap<String, String>, suffixes: &[&str]) -> Option<String> {
    for suffix in suffixes {
        for (k, v) in fields {
            if k.ends_with(suffix) && !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_2000ot_15_rows() -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        f.insert("derived:tin".into(), "00012345300000".into());
        f.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f.insert("frm2000OT:txtDateMonth".into(), "01".into());
        f.insert("frm2000OT:txtDateDay".into(), "15".into());
        f.insert("frm2000OT:txtDateYear".into(), "2026".into());

        for i in 0..15 {
            let n = i + 1;
            let fmv = 10_000_000.0 + (n as f64) * 1_000_000.0;
            f.insert(
                format!("frm2000OT:sched1A:txtOCTNo{i}"),
                format!("TCT-2026-{n:03}"),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtTaxDecNo{i}"),
                format!("TD-2026-{n:03}"),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtLoc{i}"),
                format!("MAKATI CITY PARCEL {n}"),
            );
            f.insert(format!("frm2000OT:sched1A:txtLot{i}"), "LOT".into());
            f.insert(
                format!("frm2000OT:sched1A:txtClassification{i}"),
                "CR".into(),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtArea{i}"),
                format!("{}.00", 200 + n * 10),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtFMVCol1{i}"),
                format!("{:.2}", fmv - 500_000.0),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtFMVCol2{i}"),
                format!("{fmv:.2}"),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtFMVSubTot{i}"),
                format!("{fmv:.2}"),
            );
        }
        f
    }

    fn sample_1800_18_rows() -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        f.insert("frm1800v2018:txtTIN1".into(), "000".into());
        f.insert("frm1800v2018:txtTIN2".into(), "123".into());
        f.insert("frm1800v2018:txtTIN3".into(), "453".into());
        f.insert("frm1800v2018:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f.insert("frm1800v2018:txtDateMonth".into(), "02".into());
        f.insert("frm1800v2018:txtDateDay".into(), "10".into());
        f.insert("frm1800v2018:txtDateYear".into(), "2026".into());

        for i in 0..18 {
            let n = i + 1;
            f.insert(
                format!("frm1800v2018:schedA:txtParticulars{i}"),
                format!("Cash gift to donee #{n} — bank transfer reference REF-2026-{n:03}"),
            );
            f.insert(
                format!("frm1800v2018:schedA:txtFairMarketValue{i}"),
                format!("{:.2}", 100_000.0 * n as f64),
            );
        }
        f
    }

    fn sample_1600wp_10_rows() -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        f.insert("frm1600WP:txtTIN1".into(), "000".into());
        f.insert("frm1600WP:txtTIN2".into(), "123".into());
        f.insert("frm1600WP:txtTIN3".into(), "453".into());
        f.insert("frm1600WP:txtBranchCode".into(), "00000".into());
        f.insert("frm1600WP:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f.insert("derived:from_date".into(), "03152026".into());

        for i in 1..=10 {
            let amt = 50_000.0 * i as f64;
            let tax = amt * 0.10;
            f.insert(
                format!("frm1600WP:dtSched:txtTin{i}"),
                format!("123-456-{:03}-00000", 780 + i),
            );
            f.insert(
                format!("frm1600WP:dtSched:txtFullname{i}"),
                format!("PAYEE WINNER NUMBER {i}"),
            );
            f.insert(format!("frm1600WP:dtSched:drpAtcCode{i}"), "WB191".into());
            f.insert(
                format!("frm1600WP:dtSched:naturePayment{i}"),
                "Winnings or prizes in horse races".into(),
            );
            f.insert(format!("frm1600WP:dtSched:amount{i}"), format!("{amt:.2}"));
            f.insert(format!("frm1600WP:dtSched:txtRatePercent{i}"), "10%".into());
            f.insert(
                format!("frm1600WP:dtSched:taxWithheld{i}"),
                format!("{tax:.2}"),
            );
        }
        f
    }

    #[test]
    fn detects_2000ot_15_rows_overflow() {
        let fields = sample_2000ot_15_rows();
        let report = detect_overflow("2000-ot-2018", &fields).unwrap();
        assert_eq!(report.schedules.len(), 1);
        let sched = &report.schedules[0];
        assert_eq!(sched.heading, "Schedule 1.A");
        assert_eq!(sched.printed_capacity, 5);
        assert_eq!(sched.rows.len(), 10);
        assert_eq!(sched.rows[0].row_number, 6);
        assert_eq!(sched.rows[9].row_number, 15);
        assert_eq!(sched.columns.len(), 9);
        assert_eq!(sched.columns[0].heading, "OCT/TCT/CCT No.");
        assert_eq!(
            sched.columns[8].heading,
            "Sub-total (between Columns 1 & 2)"
        );
        assert!(sched.has_total);
        // Sum of rows 6..=15: 16M + 17M + ... + 25M = 205,000,000.00
        assert_eq!(sched.columns[8].subtotal.as_deref(), Some("205,000,000.00"));
        assert_eq!(
            report.warning_lines(),
            vec!["Schedule 1.A: 10 rows are on an additional sheet"]
        );
    }

    #[test]
    fn detects_1800_18_rows_overflow() {
        let fields = sample_1800_18_rows();
        let report = detect_overflow("1800-2018", &fields).unwrap();
        assert_eq!(report.schedules.len(), 1);
        let sched = &report.schedules[0];
        assert_eq!(sched.heading, "Schedule A");
        assert_eq!(sched.printed_capacity, 5);
        assert_eq!(sched.rows.len(), 13);
        assert_eq!(sched.rows[0].row_number, 6);
        assert_eq!(sched.rows[12].row_number, 18);
        assert_eq!(sched.columns.len(), 2);
        assert_eq!(sched.columns[0].heading, "Particulars");
        assert_eq!(sched.columns[1].heading, "Fair Market Value");
        assert!(sched.has_total);
        // Sum of 600k + 700k + ... + 1.8M = 15,600,000.00
        assert_eq!(sched.columns[1].subtotal.as_deref(), Some("15,600,000.00"));
        assert_eq!(
            report.warning_lines(),
            vec!["Schedule A: 13 rows are on an additional sheet"]
        );
    }

    #[test]
    fn detects_1600wp_non_growable_overflow() {
        let fields = sample_1600wp_10_rows();
        let report = detect_overflow("1600wp-2010", &fields).unwrap();
        assert_eq!(report.schedules.len(), 1);
        let sched = &report.schedules[0];
        assert_eq!(sched.heading, "Schedule 1.1");
        assert_eq!(sched.printed_capacity, 6);
        assert_eq!(sched.rows.len(), 4);
        assert_eq!(sched.rows[0].row_number, 7);
        assert_eq!(sched.rows[3].row_number, 10);
        assert_eq!(sched.columns.len(), 7);
        assert_eq!(sched.columns[0].heading, "TIN");
        assert_eq!(sched.columns[1].heading, "Individual/Corporation");
        assert_eq!(
            report.warning_lines(),
            vec!["Schedule 1.1: 4 rows are on an additional sheet"]
        );
    }

    #[test]
    fn no_overflow_produces_empty_report_and_no_continuation_html() {
        let mut fields = BTreeMap::new();
        for i in 0..5 {
            fields.insert(format!("frm2000OT:sched1A:txtOCTNo{i}"), format!("TCT-{i}"));
            fields.insert(
                format!("frm2000OT:sched1A:txtFMVSubTot{i}"),
                "1000000.00".into(),
            );
        }
        let report = detect_overflow("2000-ot-2018", &fields).unwrap();
        assert!(report.is_empty());
        assert_eq!(report.summary(), None);
        assert_eq!(
            continuation_pages_html("2000-ot-2018", &fields, &report, "612pt", "936pt"),
            ""
        );
    }

    #[test]
    fn continuation_page_html_includes_header_columns_rows_and_subtotal() {
        let fields = sample_2000ot_15_rows();
        let report = detect_overflow("2000-ot-2018", &fields).unwrap();
        let html = continuation_pages_html("2000-ot-2018", &fields, &report, "612pt", "936pt");
        assert!(html.contains("page-continuation"));
        assert!(html.contains("BIR Form No. 2000-OT"));
        assert!(html.contains("Documentary Stamp Tax Declaration/Return (One-Time Transactions)"));
        assert!(html.contains("Additional sheet"));
        assert!(html.contains("000-123-453-00000"));
        assert!(html.contains("JUANA SANTOS"));
        assert!(html.contains("01/15/2026"));
        assert!(html.contains("Schedule 1.A (continued)"));
        assert!(html.contains("TCT-2026-006"));
        assert!(html.contains("TCT-2026-015"));
        assert!(html.contains("Subtotal (additional rows)"));
        assert!(html.contains("205,000,000.00"));
        assert!(html.contains("overflow-wrap:break-word"));
        assert!(html.contains("text-align:right"));
    }

    #[test]
    fn filled_document_appends_continuation_without_altering_official_pages() {
        let mut fields_5 = BTreeMap::new();
        fields_5.insert("derived:tin".into(), "00012345300000".into());
        fields_5.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        fields_5.insert("frm2000OT:txtDateMonth".into(), "01".into());
        fields_5.insert("frm2000OT:txtDateDay".into(), "15".into());
        fields_5.insert("frm2000OT:txtDateYear".into(), "2026".into());
        for i in 0..5 {
            let n = i + 1;
            let fmv = 10_000_000.0 + (n as f64) * 1_000_000.0;
            fields_5.insert(
                format!("frm2000OT:sched1A:txtOCTNo{i}"),
                format!("TCT-2026-{n:03}"),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtTaxDecNo{i}"),
                format!("TD-2026-{n:03}"),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtLoc{i}"),
                format!("MAKATI CITY PARCEL {n}"),
            );
            fields_5.insert(format!("frm2000OT:sched1A:txtLot{i}"), "LOT".into());
            fields_5.insert(
                format!("frm2000OT:sched1A:txtClassification{i}"),
                "CR".into(),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtArea{i}"),
                format!("{}.00", 200 + n * 10),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtFMVCol1{i}"),
                format!("{:.2}", fmv - 500_000.0),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtFMVCol2{i}"),
                format!("{fmv:.2}"),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtFMVSubTot{i}"),
                format!("{fmv:.2}"),
            );
        }
        let doc_no_overflow = frozen_html::filled_document("2000-ot-2018", &fields_5).unwrap();
        assert!(!doc_no_overflow.contains("page-continuation"));

        let fields_15 = sample_2000ot_15_rows();
        let filled =
            frozen_html::filled_document_with_report("2000-ot-2018", &fields_15, None).unwrap();
        assert!(filled.html.contains("page-continuation"));
        assert_eq!(
            filled.warning_lines(),
            vec!["Schedule 1.A: 10 rows are on an additional sheet"]
        );

        // The official portion before `<div class="page page-continuation"` must
        // be byte-for-byte identical to the document with no overflow rows!
        let split_at = filled
            .html
            .find("<div class=\"page page-continuation")
            .expect("continuation page inserted");
        let official_with_overflow = &filled.html[..split_at];
        let body_end = doc_no_overflow.rfind("</body>").unwrap();
        let official_without_overflow = &doc_no_overflow[..body_end];
        assert_eq!(official_with_overflow, official_without_overflow);
    }

    #[test]
    #[ignore]
    fn write_continuation_proof_htmls() {
        let out_dir = std::path::Path::new("/tmp/continuation-proof");
        std::fs::create_dir_all(out_dir).unwrap();

        // 1. 2000OT with 15 rows in Schedule 1.A
        let mut f2000 = sample_2000ot_15_rows();
        f2000.insert("derived:pg2_tin".into(), "00012345300000".into());
        f2000.insert("frm2000OT:txtPg2TaxpayerName".into(), "JUANA SANTOS".into());
        f2000.insert("frm2000OT:txtRDOCode".into(), "044".into());
        f2000.insert(
            "frm2000OT:txtAddress".into(),
            "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into(),
        );
        f2000.insert("frm2000OT:txtZipCode".into(), "1203".into());
        f2000.insert("frm2000OT:ATC_10".into(), "true".into());
        f2000.insert("frm2000OT:rbTransNature_2".into(), "true".into());
        f2000.insert(
            "frm2000OT:sched1A:txtFMVTotal".into(),
            "270000000.00".into(),
        );
        let doc_2000_15 =
            frozen_html::filled_document_with_report("2000-ot-2018", &f2000, None).unwrap();
        std::fs::write(out_dir.join("2000ot-15-rows.html"), &doc_2000_15.html).unwrap();
        std::fs::write(
            out_dir.join("2000ot-15-rows-report.json"),
            serde_json::to_string_pretty(&doc_2000_15.overflow).unwrap(),
        )
        .unwrap();

        // 2. 2000OT with 5 rows (no overflow)
        let mut f2000_5 = BTreeMap::new();
        for (k, v) in &f2000 {
            let is_overflow = k.starts_with("frm2000OT:sched1A:")
                && !k.contains("Total")
                && !(k.ends_with("0") && !k.ends_with("10"))
                && !(k.ends_with("1") && !k.ends_with("11"))
                && !(k.ends_with("2") && !k.ends_with("12"))
                && !(k.ends_with("3") && !k.ends_with("13"))
                && !(k.ends_with("4") && !k.ends_with("14"));
            if !is_overflow {
                f2000_5.insert(k.clone(), v.clone());
            }
        }
        f2000_5.insert("frm2000OT:sched1A:txtFMVTotal".into(), "65000000.00".into());
        let doc_2000_5 =
            frozen_html::filled_document_with_report("2000-ot-2018", &f2000_5, None).unwrap();
        assert!(doc_2000_5.overflow.is_empty());
        std::fs::write(out_dir.join("2000ot-no-overflow.html"), &doc_2000_5.html).unwrap();

        // 3. 1800 with 18 rows in Schedule A AND 8 rows in Schedule B simultaneously
        let mut f1800 = sample_1800_18_rows();
        f1800.insert("derived:pg2_tin".into(), "00012345300000".into());
        f1800.insert(
            "frm1800v2018:txtPg2TaxpayerName".into(),
            "JUANA SANTOS".into(),
        );
        f1800.insert("frm1800v2018:txtRDOCode".into(), "044".into());
        f1800.insert(
            "frm1800v2018:txtAddress".into(),
            "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into(),
        );
        f1800.insert(
            "frm1800v2018:schedA:txtTotalPayment1".into(),
            "17100000.00".into(),
        );
        let mut sched_b_total = 0.0_f64;
        for i in 0..8 {
            let row_no = i + 1;
            let ftd = 2_000_000.0 + (i as f64) * 250_000.0;
            let fbir = ftd + 200_000.0;
            sched_b_total += fbir;
            f1800.insert(
                format!("frm1800v2018:schedB1:txtOCT{i}"),
                format!("TCT-B-{row_no:03}"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB1:txtTaxDecNo{i}"),
                format!("TD-B-{row_no:03}"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB1:txtLocation{i}"),
                format!("QUEZON CITY LOT {row_no}"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB1:txtLotImprovement{i}"),
                format!("LOT {row_no} BLK 4 RESIDENTIAL"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB1:txtClassification{i}"),
                "RR".into(),
            );
            f1800.insert(
                format!("frm1800v2018:schedB2:txtArea{i}"),
                format!("{:.2}", 150.0 + (i as f64) * 15.0),
            );
            f1800.insert(
                format!("frm1800v2018:schedB2:txtFTD{i}"),
                format!("{ftd:.2}"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB2:txtFMVperTDperBIRZonalValue{i}"),
                format!("{fbir:.2}"),
            );
            f1800.insert(
                format!("frm1800v2018:schedB2:txtFairMarketValue{i}"),
                format!("{fbir:.2}"),
            );
        }
        f1800.insert(
            "frm1800v2018:schedB2:txtTotal".into(),
            format!("{sched_b_total:.2}"),
        );
        let doc_1800 = frozen_html::filled_document_with_report("1800-2018", &f1800, None).unwrap();
        std::fs::write(out_dir.join("1800-18-rows.html"), &doc_1800.html).unwrap();
        std::fs::write(
            out_dir.join("1800-18-rows-report.json"),
            serde_json::to_string_pretty(&doc_1800.overflow).unwrap(),
        )
        .unwrap();

        // 4. 2552 with overflow in TWO schedules at once (Schedule 1: 8 rows, Schedule 2: 8 rows)
        let mut f2552 = BTreeMap::new();
        f2552.insert("derived:tin".into(), "00012345300000".into());
        f2552.insert("derived:pg2_tin".into(), "00012345300000".into());
        f2552.insert("frm2552:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f2552.insert("frm2552:txtPg2TaxpayerName".into(), "JUANA SANTOS".into());
        f2552.insert("frm2552:txtYearEnded".into(), "2026".into());
        let mut s1_tot = 0.0_f64;
        let mut s2_tot = 0.0_f64;
        for row_no in 1..=8 {
            let base1 = 100_000.0 * (row_no as f64);
            let due1 = base1 * 0.006;
            s1_tot += due1;
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1Date{row_no}"),
                format!("01/{row_no:02}/2026"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1Seller{row_no}"),
                format!("SELLER {row_no} INC."),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1Buyer{row_no}"),
                format!("BUYER {row_no} CORP."),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1Corp{row_no}"),
                format!("ISSUER ALPHA {row_no}"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1NumShares{row_no}"),
                format!("{}", 10_000 * row_no),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1TaxBase{row_no}"),
                format!("{base1:.2}"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1TaxRate{row_no}"),
                "0.6%".into(),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch1TaxDue{row_no}"),
                format!("{due1:.2}"),
            );

            let base2 = 250_000.0 * (row_no as f64);
            let due2 = base2 * 0.04;
            s2_tot += due2;
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2Date{row_no}"),
                format!("02/{row_no:02}/2026"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2Seller{row_no}"),
                format!("IPO SELLER {row_no}"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2Buyer{row_no}"),
                format!("IPO BUYER {row_no}"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2Corp{row_no}"),
                format!("IPO CORP {row_no}"),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2NumShares{row_no}"),
                format!("{}", 5_000 * row_no),
            );
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2TaxBase{row_no}"),
                format!("{base2:.2}"),
            );
            f2552.insert(format!("frm2552:txtPg2Pt5Sch2TaxRate{row_no}"), "4%".into());
            f2552.insert(
                format!("frm2552:txtPg2Pt5Sch2TaxDue{row_no}"),
                format!("{due2:.2}"),
            );
        }
        f2552.insert(
            "frm2552:txtPg2Pt5Sch1TaxDueTot".into(),
            format!("{s1_tot:.2}"),
        );
        f2552.insert(
            "frm2552:txtPg2Pt5Sch2TaxDueTot".into(),
            format!("{s2_tot:.2}"),
        );
        let doc_2552 = frozen_html::filled_document_with_report("2552-2018", &f2552, None).unwrap();
        std::fs::write(out_dir.join("2552-two-schedules.html"), &doc_2552.html).unwrap();
        std::fs::write(
            out_dir.join("2552-two-schedules-report.json"),
            serde_json::to_string_pretty(&doc_2552.overflow).unwrap(),
        )
        .unwrap();

        // 5. 1600WP (now with metadata-only formgen-bands + derived:sched_seq* ordinal continuation)
        let mut f1600wp = sample_1600wp_10_rows();
        for seq in 1..=6 {
            f1600wp.insert(format!("derived:sched_seq{seq}"), seq.to_string());
        }
        f1600wp.insert("frm1600WP:txtRDOCode".into(), "044".into());
        f1600wp.insert(
            "frm1600WP:txtAddress".into(),
            "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into(),
        );
        f1600wp.insert("derived:WB191_base".into(), "2750000.00".into());
        f1600wp.insert("derived:WB191_withheld".into(), "275000.00".into());
        let doc_1600wp =
            frozen_html::filled_document_with_report("1600wp-2010", &f1600wp, None).unwrap();
        std::fs::write(out_dir.join("1600wp-10-payees.html"), &doc_1600wp.html).unwrap();
        std::fs::write(
            out_dir.join("1600wp-10-payees-report.json"),
            serde_json::to_string_pretty(&doc_1600wp.overflow).unwrap(),
        )
        .unwrap();

        // 6. 1707 (Schedule 2 + Schedule 3 popup rows; Schedule 3 cloned band + white knockout band)
        let mut f1707 = BTreeMap::new();
        f1707.insert("derived:tin".into(), "00012345300000".into());
        f1707.insert("frm1707:txtSellerName".into(), "JUANA SANTOS".into());
        f1707.insert("frm1707:txtBuyerName".into(), "PEDRO REYES".into());
        f1707.insert("frm1707:txtRDO".into(), "044".into());
        for (idx, letter) in [(1, "A"), (2, "B"), (3, "C"), (4, "D")] {
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2Col1{letter}"),
                format!("OFFICIAL CORP {idx}"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2Col2{letter}"),
                format!("{idx}00"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2Col3{letter}"),
                format!("CERT-00{idx}"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2Col4{letter}"),
                format!("{idx}00000.00"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S3Col1{letter}"),
                format!("OFFICIAL EXPENSE {idx}"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S3Col2{letter}"),
                format!("{idx}500.00"),
            );
        }
        for k in 1..=4 {
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2_{k}Col1"),
                format!("OVERFLOW CORP {k}"),
            );
            f1707.insert(format!("frm1707:txtPg2Pt4S2_{k}Col2"), format!("{}50", k));
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2_{k}Col3"),
                format!("CERT-10{k}"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2_{k}Col4"),
                format!("{k}25000.00"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S3_{k}Col1"),
                format!("OVERFLOW EXPENSE {k}"),
            );
            f1707.insert(format!("frm1707:txtPg2Pt4S3_{k}Col2"), format!("{k}750.50"));
        }
        f1707.insert("frm1707:txtPg2Pt4S3Total".into(), "29002.00".into());
        let doc_1707 = frozen_html::filled_document_with_report("1707-2021", &f1707, None).unwrap();
        std::fs::write(out_dir.join("1707-popup-rows.html"), &doc_1707.html).unwrap();
        std::fs::write(
            out_dir.join("1707-popup-rows-report.json"),
            serde_json::to_string_pretty(&doc_1707.overflow).unwrap(),
        )
        .unwrap();

        // 7. 1801 (Schedule 1 + Schedule 2 cloned bands; no duplicate "(continued)" on Schedule 1)
        let mut f1801 = BTreeMap::new();
        f1801.insert("derived:tin".into(), "00012345300000".into());
        f1801.insert("frm1801v2018:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f1801.insert("frm1801v2018:txtRDO".into(), "044".into());
        for i in 1..=5 {
            f1801.insert(format!("frm1801v2018:sched1Oct_{i}"), format!("TCT-{i:04}"));
            f1801.insert(format!("frm1801v2018:sched1Td_{i}"), format!("TD-{i:04}"));
            f1801.insert(
                format!("frm1801v2018:sched1Loc_{i}"),
                format!("MAKATI CITY LOT {i}"),
            );
            f1801.insert(
                format!("frm1801v2018:sched1Lot_{i}"),
                format!("LOT {i} BLK 2"),
            );
            f1801.insert(
                format!("frm1801v2018:sched1Area_{i}"),
                format!("{}.00", 120 * i),
            );
            f1801.insert(format!("frm1801v2018:sched1Class_{i}'"), "RES".into());
            f1801.insert(
                format!("frm1801v2018:sched1Fmv_{i}"),
                format!("{}.00", 1_000_000 * i),
            );
            f1801.insert(
                format!("frm1801v2018:sched2Oct_{i}"),
                format!("FAM-TCT-{i:03}"),
            );
            f1801.insert(
                format!("frm1801v2018:sched2Td_{i}"),
                format!("FAM-TD-{i:03}"),
            );
            f1801.insert(
                format!("frm1801v2018:sched2Loc_{i}"),
                format!("TAGUIG HOME {i}"),
            );
            f1801.insert(
                format!("frm1801v2018:sched2Fmv_{i}"),
                format!("{}.00", 500_000 * i),
            );
        }
        let doc_1801 = frozen_html::filled_document_with_report("1801-2018", &f1801, None).unwrap();
        std::fs::write(out_dir.join("1801-two-schedules.html"), &doc_1801.html).unwrap();
        std::fs::write(
            out_dir.join("1801-two-schedules-report.json"),
            serde_json::to_string_pretty(&doc_1801.overflow).unwrap(),
        )
        .unwrap();

        // 8. 2550Q (Schedule 1 cloned band)
        let mut f2550q = BTreeMap::new();
        f2550q.insert("derived:tin".into(), "00012345300000".into());
        f2550q.insert("derived:pg2_tin".into(), "00012345300000".into());
        f2550q.insert("frm2550q:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f2550q.insert("frm2550q:txtPg2TaxpayerName".into(), "JUANA SANTOS".into());
        f2550q.insert("frm2550q:txtRDOCode".into(), "044".into());
        for i in 0..6 {
            let n = i + 1;
            f2550q.insert(format!("txtDatePurchase1{i}"), format!("01/1{n}/2026"));
            f2550q.insert(format!("txtSourceCode1{i}"), "D".into());
            f2550q.insert(
                format!("txtDescription1{i}"),
                format!("CAPITAL EQUIPMENT {n}"),
            );
            f2550q.insert(
                format!("txtAmountPurchase1{i}"),
                format!("{}.00", 1_500_000 * n),
            );
            f2550q.insert(format!("txtInputTax1{i}"), format!("{}.00", 180_000 * n));
            f2550q.insert(format!("txtEstimatedLife1{i}"), "60".into());
            f2550q.insert(format!("txtRecognizedLife1{i}"), "48".into());
            f2550q.insert(
                format!("txtAllowedInputTax1{i}"),
                format!("{}.00", 9_000 * n),
            );
            f2550q.insert(
                format!("txtBalanceInputTax1{i}"),
                format!("{}.00", 171_000 * n),
            );
        }
        let doc_2550q =
            frozen_html::filled_document_with_report("2550q-2024", &f2550q, None).unwrap();
        std::fs::write(out_dir.join("2550q-sched1-overflow.html"), &doc_2550q.html).unwrap();
        std::fs::write(
            out_dir.join("2550q-sched1-overflow-report.json"),
            serde_json::to_string_pretty(&doc_2550q.overflow).unwrap(),
        )
        .unwrap();

        // 9. 1701A (Part IV-A & Part IV-B popup rows on cloned bands p2g0 & p2g1)
        let mut f1701a = BTreeMap::new();
        f1701a.insert("derived:tin".into(), "00012345300000".into());
        f1701a.insert("frm1701A:txtTaxpayerName".into(), "JUANA SANTOS".into());
        f1701a.insert("frm1701A:txtRDO".into(), "044".into());
        f1701a.insert("frm1701A:txt41OtherDesc".into(), "RENTAL INCOME".into());
        f1701a.insert("frm1701A:txt41A".into(), "120000.00".into());
        f1701a.insert("frm1701A:txt42OtherDesc".into(), "ROYALTY INCOME".into());
        f1701a.insert("frm1701A:txt42A".into(), "80000.00".into());
        for n in 1..=4 {
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVADesc_{n}C1"),
                format!("OTHER NON-OP A {n}"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVA_{n}C2"),
                format!("{n}5000.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVA_{n}C3"),
                format!("{n}200.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVBDesc_{n}C1"),
                format!("OTHER NON-OP B {n}"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C2"),
                format!("{n}8000.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C3"),
                format!("{n}400.00"),
            );
        }
        let doc_1701a =
            frozen_html::filled_document_with_report("1701a-2018", &f1701a, None).unwrap();
        std::fs::write(out_dir.join("1701a-popup-rows.html"), &doc_1701a.html).unwrap();
        std::fs::write(
            out_dir.join("1701a-popup-rows-report.json"),
            serde_json::to_string_pretty(&doc_1701a.overflow).unwrap(),
        )
        .unwrap();
    }

    /// Extract all printed text from a filled HTML document: both plain/overflow
    /// `<input ... value="...">`, comb-split single-character `<input name="..." value="X">`
    /// concatenated in order, and plain continuation table `<td>...</td>` text.
    fn extract_printed_corpus(html: &str) -> String {
        let mut out = String::new();
        // 1. All <input ... value="..."> concatenated (comb chars join seamlessly!)
        //    plus space-separated copies so full tokens also match cleanly.
        let mut comb_buf = String::new();
        let mut last_name = String::new();
        let mut pos = 0;
        while let Some(rel) = html[pos..].find("<input ") {
            let start = pos + rel;
            let Some(end_rel) = html[start..].find('>') else {
                break;
            };
            let tag = &html[start..=start + end_rel];
            pos = start + end_rel + 1;
            let val = extract_attr(tag, "value").unwrap_or_default();
            if val.is_empty() {
                continue;
            }
            let name = extract_attr(tag, "name").unwrap_or_default();
            if !name.is_empty() && name == last_name {
                comb_buf.push_str(&val);
            } else {
                if !comb_buf.is_empty() {
                    out.push(' ');
                    out.push_str(&comb_buf);
                    comb_buf.clear();
                }
                last_name = name;
                comb_buf.push_str(&val);
            }
            out.push(' ');
            out.push_str(&val);
            if val.contains("&#10;") {
                out.push(' ');
                out.push_str(&val.replace("&#10;", ""));
                out.push(' ');
                out.push_str(&val.replace("&#10;", " "));
            }
        }
        if !comb_buf.is_empty() {
            out.push(' ');
            out.push_str(&comb_buf);
        }
        // 2. Plain continuation table cells <td>...</td>
        pos = 0;
        while let Some(rel) = html[pos..].find("<td") {
            let start = pos + rel;
            let Some(gt) = html[start..].find('>') else {
                break;
            };
            let content_start = start + gt + 1;
            let Some(close) = html[content_start..].find("</td>") else {
                break;
            };
            out.push(' ');
            out.push_str(&html[content_start..content_start + close]);
            pos = content_start + close + 5;
        }
        out.to_ascii_uppercase()
    }

    fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
        let needle = format!("{attr_name}=\"");
        let at = tag.find(&needle)? + needle.len();
        let end = at + tag[at..].find('"')?;
        Some(tag[at..end].to_string())
    }

    fn corpus_contains_value(
        corpus: &str,
        corpus_no_commas: &str,
        val: &str,
        is_amount: bool,
    ) -> bool {
        let up = val.trim().to_ascii_uppercase();
        if up.is_empty() {
            return true;
        }
        if corpus.contains(&up) {
            return true;
        }
        if is_amount {
            let stripped = up.replace(',', "");
            let whole = stripped.split('.').next().unwrap_or(&stripped);
            if !whole.is_empty() && corpus_no_commas.contains(whole) {
                return true;
            }
        }
        false
    }

    /// Safety net across all 43 forms: for every form, fills every repeatable
    /// schedule with more rows than print (`printed_capacity + 3`) and asserts
    /// that every filled row's values appear somewhere in the printed document
    /// (official pages or continuation pages). No allow-list: any failing form
    /// is reported in the test output.
    #[test]
    fn every_form_repeatable_schedule_prints_all_filled_rows() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen"));
        let mut slugs: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("writer-cells.json").exists())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        slugs.sort();
        assert_eq!(slugs.len(), 43, "expected all 43 forms in html-frozen/");

        let mut failures: Vec<String> = Vec::new();
        let mut checked_schedules = 0usize;

        for slug in &slugs {
            let Some(wc_json) = frozen_html::writer_cells_json(slug) else {
                failures.push(format!("{slug}: missing writer-cells.json"));
                continue;
            };
            let parsed: Value = serde_json::from_str(wc_json).unwrap();
            let stem_specs = discover_schedules(&parsed);
            let popup_defs: Vec<&PopupScheduleDef> =
                POPUP_SCHEDULES.iter().filter(|d| d.slug == slug).collect();

            if stem_specs.is_empty() && popup_defs.is_empty() {
                continue;
            }

            let mut fields: BTreeMap<String, String> = BTreeMap::new();
            // Expected (schedule_heading, row_num, col_heading, value, is_amount)
            let mut expected: Vec<(String, usize, String, String, bool)> = Vec::new();

            // 1. Fill all stem-based repeatable schedules with printed_capacity + 3 rows.
            for (si, spec) in stem_specs.iter().enumerate() {
                // Skip stem specs whose heading is handled by a popup def for this slug
                // (popup forms put extra rows in popup keys, not higher stem indices).
                if popup_defs
                    .iter()
                    .any(|p| p.heading.eq_ignore_ascii_case(&spec.heading))
                {
                    continue;
                }
                checked_schedules += 1;
                let min_idx = spec.columns.iter().map(|c| c.start_idx).min().unwrap_or(1);
                let max_idx = spec.columns.iter().map(|c| c.end_idx).max().unwrap_or(1);
                let extra_start = max_idx + 1;
                let extra_end = max_idx + 3;
                for raw_idx in min_idx..=extra_end {
                    let row_num = if min_idx == 0 { raw_idx + 1 } else { raw_idx };
                    for (ci, col) in spec.columns.iter().enumerate() {
                        // On official rows, only fill columns that actually exist on this row
                        // (e.g. 2200M Schedule 1 has different column subsets per mineral row).
                        // On overflowing rows (raw_idx >= extra_start), fill every column.
                        if raw_idx < extra_start
                            && (raw_idx < col.start_idx || raw_idx > col.end_idx)
                        {
                            continue;
                        }
                        let key = format!("{}{}", col.stem, raw_idx);
                        let is_date = col.heading.to_ascii_lowercase().contains("date");
                        let val = if col.is_amount {
                            format!("{}.00", 10_000 + si * 1000 + row_num * 17 + ci)
                        } else if is_date {
                            format!("01/{:02}/2026", ((row_num + ci) % 28) + 1)
                        } else {
                            format!("S{si}R{row_num}C{ci}")
                        };
                        fields.insert(key, val.clone());
                        // Always verify every overflowing row (and official rows for non-date).
                        if raw_idx >= extra_start || !is_date {
                            expected.push((
                                spec.heading.clone(),
                                row_num,
                                col.heading.clone(),
                                val,
                                col.is_amount,
                            ));
                        }
                    }
                }
            }

            // 2. Fill all popup schedules with 3 popup overflow rows.
            for (pi, def) in popup_defs.iter().enumerate() {
                checked_schedules += 1;
                for n in 1..=3 {
                    let row_num = def.row_num_base + n;
                    for (ci, col) in def.columns.iter().enumerate() {
                        let key = col.key_template.replace("{n}", &n.to_string());
                        let is_date = col.heading.to_ascii_lowercase().contains("date");
                        let val = if col.is_amount {
                            format!("{}.00", 50_000 + pi * 1000 + n * 19 + ci)
                        } else if is_date {
                            format!("02/{:02}/2026", (n % 28) + 1)
                        } else {
                            format!("P{pi}R{row_num}C{ci}")
                        };
                        fields.insert(key, val.clone());
                        expected.push((
                            def.heading.to_string(),
                            row_num,
                            col.heading.to_string(),
                            val,
                            col.is_amount,
                        ));
                    }
                }
            }

            let doc = match frozen_html::filled_document_with_report(slug, &fields, None) {
                Ok(d) => d,
                Err(e) => {
                    failures.push(format!("{slug}: filled_document_with_report error: {e}"));
                    continue;
                }
            };

            let corpus = extract_printed_corpus(&doc.html);
            let corpus_no_commas = corpus.replace(',', "");

            // Group missing values per (schedule, row) so the failure list is readable.
            let mut missing_per_sched: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (sched, row_num, col_heading, val, is_amount) in &expected {
                if !corpus_contains_value(&corpus, &corpus_no_commas, val, *is_amount) {
                    missing_per_sched
                        .entry(sched.clone())
                        .or_default()
                        .push(format!("row {row_num} [{col_heading}]={val}"));
                }
            }
            for (sched, missing) in missing_per_sched {
                let sample: Vec<&str> = missing.iter().take(4).map(String::as_str).collect();
                failures.push(format!(
                    "{slug} / {sched}: {} missing value(s) (e.g. {})",
                    missing.len(),
                    sample.join(", ")
                ));
            }
        }

        assert!(
            checked_schedules > 0,
            "safety-net test must exercise repeatable schedules across the 43 forms"
        );
        assert!(
            failures.is_empty(),
            "43-form continuation safety-net failures ({} total):\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        );
    }

    #[test]
    fn form_2200m_wide_schedule_table_stays_inside_the_paper_width() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert(
            "frm2200M:txtPg1Pt1I6RegisteredName".into(),
            "MINERAL PRODUCTS FILING CORPORATION".into(),
        );
        for (key, value) in [
            ("frm2200M:txtSched1_description11", "OTHER MINERAL ROW 11"),
            ("frm2200M:txtSched1_PlaceOfRemoval11", "PROVINCE 11"),
            ("frm2200M:txtSched1_VOMRITaxableA11", "10000.00"),
            ("frm2200M:txtSched1_VOMRIExemptB11", "1000.00"),
            ("frm2200M:txtSched1_LocallyExtractedRateC11", "2.00"),
            ("frm2200M:txtSched1_LocallyExtractedRateD11", "3.00"),
            ("frm2200M:txtSched1_LocallyExtractedTaxRateE11", "4.00"),
            ("frm2200M:txtSched1_LocallyExtractedTaxDueF11", "500.00"),
        ] {
            fields.insert(key.into(), value.into());
        }

        let report = detect_overflow("2200m-2018", &fields).unwrap();
        assert_eq!(report.schedules.len(), 1);
        assert!(
            report.schedules[0].columns.len() >= 10,
            "2200M Schedule 1 is the wide plain continuation table"
        );
        let html = continuation_pages_html("2200m-2018", &fields, &report, "612pt", "936pt");
        assert!(
            html.contains("table-layout:fixed;font-size:6.8pt"),
            "the wide table must use the compact fixed layout that keeps it inside the paper: {html}"
        );
        assert!(html.contains("OTHER MINERAL ROW 11"));
    }

    #[test]
    fn truncated_cell_produces_one_additional_sheet_with_full_text() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        fields.insert("frm2000OT:txtDateMonth".into(), "01".into());
        fields.insert("frm2000OT:txtDateDay".into(), "15".into());
        fields.insert("frm2000OT:txtDateYear".into(), "2026".into());
        let long_stock = "METROPOLITAN PHILIPPINE INFRASTRUCTURE HOLDINGS AND INDUSTRIAL DEVELOPMENT CORPORATION CUMULATIVE PARTICIPATING REDEEMABLE PREFERRED SHARES SERIES 2026-A";
        fields.insert(
            "frm2000OT:sched1B:txtNameOfCorpStock2".into(),
            long_stock.into(),
        );

        let doc = frozen_html::filled_document_with_report("2000-ot-2018", &fields, None).unwrap();
        assert_eq!(
            doc.html.matches("class=\"page page-continuation\"").count(),
            1,
            "one truncated cell must produce exactly one additional sheet"
        );
        assert!(doc.html.contains("Entries continued from the form"));
        assert!(doc.html.contains("Page 2"));
        assert!(doc
            .html
            .contains("Schedule 1.B, row 3, Name of Corporate Stock (a)"));
        assert!(doc.html.contains(long_stock));
        // Never leak internal cell ids or file names in the printed table.
        let cont_pos = doc.html.find("class=\"page page-continuation\"").unwrap();
        let cont_html = &doc.html[cont_pos..];
        assert!(!cont_html.contains("p2c"));
        assert!(!cont_html.contains("writer-cells.json"));
        // The official cell ends with "..." followed by nothing else.
        let official_html = &doc.html[..cont_pos];
        assert!(official_html.contains("data-overflow-warning=\"text-truncated\""));
        assert!(official_html.contains("...\""));
    }

    #[test]
    fn no_truncated_cell_produces_no_additional_sheet() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        fields.insert("frm2000OT:txtDateMonth".into(), "01".into());
        fields.insert("frm2000OT:txtDateDay".into(), "15".into());
        fields.insert("frm2000OT:txtDateYear".into(), "2026".into());
        fields.insert(
            "frm2000OT:sched1B:txtNameOfCorpStock2".into(),
            "AYALA CORP".into(),
        );

        let doc = frozen_html::filled_document_with_report("2000-ot-2018", &fields, None).unwrap();
        assert!(!doc.html.contains("page-continuation"));
        assert!(!doc.html.contains("Entries continued from the form"));
    }

    #[test]
    fn extra_schedule_rows_and_truncated_cell_produce_one_coherent_set_of_additional_sheets() {
        // Case 1: plain fallback schedule rows + truncated cell on the SAME plain sheet sequence
        let mut plain_fields = BTreeMap::new();
        plain_fields.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        let sched = OverflowSchedule {
            heading: "Schedule 1".into(),
            printed_capacity: 5,
            columns: vec![OverflowColumn {
                heading: "Description".into(),
                is_amount: false,
                subtotal: None,
            }],
            rows: vec![OverflowRow {
                row_number: 6,
                values: vec!["Overflow row 6".into()],
                keys: vec!["frm:txtDesc6".into()],
            }],
            has_total: false,
        };
        let truncated = vec![TruncatedEntry {
            page_number: 2,
            label: "Part V, Item 7 — Foreign Tax Credits (specify)".into(),
            full_text: "VERY LONG TAX CREDIT SPECIFICATION THAT DID NOT FIT ON THE OFFICIAL LINE"
                .into(),
        }];
        let combined = continuation_pages_html_with_truncated(
            "1701ms-2024",
            &plain_fields,
            &OverflowReport {
                schedules: vec![sched],
            },
            &truncated,
            "612pt",
            "936pt",
        );
        assert_eq!(
            combined.matches("id=\"page-continuation\"").count(),
            1,
            "must produce one coherent additional sheet, not two competing ones"
        );
        assert!(combined.contains("Schedule 1 (continued)"));
        assert!(combined.contains("Entries continued from the form"));
        assert!(combined.contains("Part V, Item 7 — Foreign Tax Credits (specify)"));

        // Case 2: end-to-end 2000-OT with BOTH 15 Schedule 1.A rows (cloned band)
        // AND a truncated Schedule 1.B stock name -> band page + 1 plain additional sheet
        let mut fields = sample_2000ot_15_rows();
        let long_stock = "METROPOLITAN PHILIPPINE INFRASTRUCTURE HOLDINGS AND INDUSTRIAL DEVELOPMENT CORPORATION CUMULATIVE PARTICIPATING REDEEMABLE PREFERRED SHARES SERIES 2026-A";
        fields.insert(
            "frm2000OT:sched1B:txtNameOfCorpStock2".into(),
            long_stock.into(),
        );
        let doc = frozen_html::filled_document_with_report("2000-ot-2018", &fields, None).unwrap();
        assert!(doc.html.contains("page-continuation-band"));
        assert_eq!(
            doc.html.matches("id=\"page-continuation\"").count(),
            1,
            "only one plain additional sheet id"
        );
        assert!(doc.html.contains("Entries continued from the form"));
        assert!(doc.html.contains(long_stock));
    }

    #[test]
    fn truncated_cell_without_writer_label_falls_back_to_nearest_printed_caption() {
        let synthetic = "<div class=\"page page-1\" style=\"width:612pt;height:936pt\">\
            <div class=\"t\" style=\"left:40pt;top:100pt\">9 Other Remarks</div>\
            <div class=\"f\" id=\"p1c999\" style=\"left:40pt;top:114pt;width:80pt;height:14pt\">\
            <input id=\"p1c999-i\" data-overflow-warning=\"text-truncated\" data-writer-value=\"FULL UNLABELED TEXT\" value=\"FULL UN...\">\
            </div></div>";
        let entries =
            collect_truncated_entries("2000-ot-2018", synthetic, &OverflowReport::default());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].page_number, 1);
        assert_eq!(entries[0].label, "9 Other Remarks");
        assert_eq!(entries[0].full_text, "FULL UNLABELED TEXT");
    }

    #[test]
    fn additional_sheet_header_uses_form_date_label_or_omits_period_never_dash() {
        let truncated = vec![TruncatedEntry {
            page_number: 2,
            label: "Schedule 1.B, row 3, Name of Corporate Stock (a)".to_string(),
            full_text: "LONG STOCK NAME".to_string(),
        }];
        let mut fields = BTreeMap::new();
        fields.insert("frm2000OT:txtTIN1".into(), "000".into());
        fields.insert("frm2000OT:txtTIN2".into(), "123".into());
        fields.insert("frm2000OT:txtTIN3".into(), "453".into());
        fields.insert("frm2000OT:txtBranchCode".into(), "00000".into());
        fields.insert("frm2000OT:txtTaxpayerName".into(), "SANTOS, JUANA".into());
        // No transaction date -> omit period label completely, never print a dash.
        let html_no_date = continuation_pages_html_with_truncated(
            "2000-ot-2018",
            &fields,
            &OverflowReport::default(),
            &truncated,
            "612pt",
            "936pt",
        );
        assert!(!html_no_date.contains("Return Period:"));
        assert!(!html_no_date.contains("Date of Transaction:"));
        assert!(!html_no_date.contains("—"));

        // With transaction date -> print the form's own Item 1 label ("Date of Transaction").
        fields.insert("frm2000OT:txtDateMonth".into(), "06".into());
        fields.insert("frm2000OT:txtDateDay".into(), "15".into());
        fields.insert("frm2000OT:txtDateYear".into(), "2026".into());
        let html_with_date = continuation_pages_html_with_truncated(
            "2000-ot-2018",
            &fields,
            &OverflowReport::default(),
            &truncated,
            "612pt",
            "936pt",
        );
        assert!(html_with_date.contains("Date of Transaction:</span> 06/15/2026"));
        assert!(!html_with_date.contains("Return Period:"));
    }

    #[test]
    fn additional_sheet_header_resolves_taxpayer_name_across_form_key_variants() {
        let cases = [
            ("1706-2018", "frm1706:txtSellerName"),
            ("2550m-2007", "frm2550m:txtTaxPayerName"),
            ("0605-1999", "frm0605:txtTaxPayerName"),
            ("1600-pt-2018", "frm1600PT:txtPg1Pt1I7WithholdingAgentsName"),
            ("1604e-2018", "frm1604e:txtWthhldngAgntsNme"),
            ("1606-2018", "frm1606:txtSellerName"),
            ("1700-2018", "frm1700:txtPg1I7TaxpayerName"),
            ("1701-2018", "frm1701:txtPg1I8TaxpayerName"),
            ("1701ms-2024", "frm1701MS:txtTaxpayerNo8a"),
            ("1702ex-2018", "derived:name_line1"),
            ("1702mx-2018c", "frm1702MX:txtPg1Pt1I9RegisteredName"),
            ("1702q-2018", "derived:name_line1"),
            ("1702rt-2018c", "frm1702RT:txtPg1Pt1I8Name1"),
            ("1707-2021", "frm1707:txtPg1I6SellerName0"),
            ("1707a-2021", "frm1707Av2021:txtI8RegisteredName"),
            ("1801-2018", "frm1801v2018:registeredName_1"),
            ("2200c-2018", "frm2200C:txtPg1Pt1I7RegisteredName"),
            ("2200m-2018", "frm2200M:txtPg1Pt1I6RegisteredName"),
            ("2200p-2020", "frm2200P:txtPg1Pt1I6RegisteredName"),
            ("2200s-2018", "frm2200S:taxpayerName"),
            ("2550q-2024", "frm2550qv2024:taxpayerName"),
            ("2552-2018", "frm2552:txtPg1Pt1I6Name"),
            ("2553-1999", "frm2553:txtTPName"),
        ];
        let truncated = vec![TruncatedEntry {
            page_number: 1,
            label: "Address".into(),
            full_text: "VERY LONG REGISTERED ADDRESS FOR TEST".into(),
        }];
        for (slug, key) in cases {
            let mut fields = BTreeMap::new();
            fields.insert(
                key.to_string(),
                "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA".to_string(),
            );
            let html = continuation_pages_html_with_truncated(
                slug,
                &fields,
                &OverflowReport::default(),
                &truncated,
                "612pt",
                "936pt",
            );
            assert!(
                html.contains("Taxpayer's Name:</span> SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA"),
                "{slug} ({key}) printed dash instead of taxpayer name"
            );
            assert!(
                !html.contains("min-height:936pt"),
                "{slug} continuation sheet must not force unscaled min-height that emits a blank trailing page"
            );
        }
    }

    #[test]
    fn all_43_forms_additional_sheet_header_extracts_tin() {
        use bir_core::forms::*;
        use bir_core::profile::TaxpayerProfile;

        let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA",
            "tin": bir_core::Tin::parse_digits("00012345300000").unwrap(),
            "rdo_code": "039",
            "line_of_business": "CONSULTING SERVICES",
            "registered_address": "UNIT 27F CORPORATE TOWER MAKATI",
            "zip_code": "1223",
            "phone": "09171234567",
            "email": "juana.santos.demo@example.com",
            "default_form_type": "1701"
        }))
        .expect("demo profile");

        let forms: Vec<(&str, BTreeMap<String, String>)> = vec![
            (
                "0605-1999",
                form_0605::Form0605Draft::new_from_profile(&profile, 2025, 1).to_bir_field_map(),
            ),
            (
                "0619e-2018",
                form_0619e::Form0619EDraft::new_from_profile(&profile, 2025, 1).to_bir_field_map(),
            ),
            (
                "0619f-2018",
                form_0619f::Form0619FDraft::new_from_profile(&profile, 2025, 1).to_bir_field_map(),
            ),
            (
                "1600-pt-2018",
                form_1600pt::Form1600PtDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1600-vt-2018",
                form_1600vt::Form1600VtDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1600wp-2010",
                form_1600wp::Form1600WpDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1601-fq-2020",
                form_1601fq::Form1601FqDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1601c-2018",
                form_1601c::Form1601CDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1601eq-2019",
                form_1601eq::Form1601EqDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1602q-2019",
                form_1602q::Form1602QDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1603q-2018",
                form_1603q::Form1603QDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1604c-2018",
                form_1604c::Form1604cDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1604e-2018",
                form_1604e::Form1604eDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1604f-2018",
                form_1604f::Form1604fDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1606-2018",
                form_1606::Form1606Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "1700-2018",
                form_1700::Form1700Draft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1701-2018",
                form_1701::Form1701Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "1701a-2018",
                form_1701a::Form1701ADraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1701ms-2024",
                form_1701ms::Form1701MsDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1701q-2018",
                form_1701q::Form1701QDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1702ex-2018",
                form_1702ex::Form1702ExDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1702mx-2018c",
                form_1702mx::Form1702MXDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "1702q-2018",
                form_1702q::Form1702qDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1702rt-2018c",
                form_1702rt::Form1702RTDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1706-2018",
                form_1706::Form1706Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "1707-2021",
                form_1707::Form1707Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "1707a-2021",
                form_1707a::Form1707ADraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "1800-2018",
                form_1800::Form1800Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "1801-2018",
                form_1801::Form1801Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "2000-dst-2018",
                form_2000::Form2000Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
            (
                "2000-ot-2018",
                form_2000ot::Form2000OTDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "2200a-2020",
                form_2200a::Form2200ADraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2200an-2018",
                form_2200an::Form2200ANDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2200c-2018",
                form_2200c::Form2200CDraft::new_from_profile(&profile, 2025).to_print_field_map(),
            ),
            (
                "2200m-2018",
                form_2200m::Form2200MDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2200p-2020",
                form_2200p::Form2200PDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2200s-2018",
                form_2200s::Form2200SDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2200t-2022",
                form_2200t::Form2200TDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2550m-2007",
                form_2550m::Form2550MDraft::new_from_profile(&profile, 2025, 1).to_bir_field_map(),
            ),
            (
                "2550q-2024",
                form_2550q::Form2550QDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2551q-2018",
                form_2551q::Form2551QDraft::new_from_profile(&profile, 2025, 1)
                    .to_print_field_map(),
            ),
            (
                "2552-2018",
                form_2552::Form2552Draft::new_from_profile(&profile, 2025, 1, 15)
                    .to_print_field_map(),
            ),
            (
                "2553-1999",
                form_2553::Form2553Draft::new_from_profile(&profile, 2025, 1).to_print_field_map(),
            ),
        ];
        assert_eq!(forms.len(), 43);

        let truncated = vec![TruncatedEntry {
            page_number: 1,
            label: "Address".into(),
            full_text: "VERY LONG REGISTERED ADDRESS FOR TEST".into(),
        }];
        let mut missing = Vec::new();
        for (slug, base_fields) in &forms {
            let mut fields = base_fields.clone();
            // Seed schedule/payee row TIN decoys (matching 1600WP's `frm1600WP:dtSched:txtTin1..2`)
            // so the header must always pick the filer's own TIN + branch code, never a row's.
            fields.insert("frm1600WP:dtSched:txtTin1".into(), "12345678900001".into());
            fields.insert("frm1600WP:dtSched:txtTin2".into(), "12345678900002".into());
            fields.insert("frmDummy:sched1:txtTIN1".into(), "999".into());
            fields.insert("frmDummy:sched1:txtTIN2".into(), "888".into());
            fields.insert("frmDummy:sched1:txtTIN3".into(), "777".into());
            fields.insert("frmDummy:sched1:txtBranchCode".into(), "99999".into());
            let tin = extract_tin(&fields);
            let html = continuation_pages_html_with_truncated(
                slug,
                &fields,
                &OverflowReport::default(),
                &truncated,
                "612pt",
                "936pt",
            );
            if tin != "000-123-453-00000" || !html.contains("TIN:</span> 000-123-453-00000") {
                missing.push(format!("{slug} (got {tin})"));
            }
        }
        assert!(
            missing.is_empty(),
            "forms with missing/wrong Additional Sheet TIN ({}/43): {}",
            missing.len(),
            missing.join(", ")
        );
    }

    #[test]
    fn additional_sheet_header_includes_return_period_for_1702mx_and_1702ex() {
        use bir_core::forms::{form_1702ex, form_1702mx};
        use bir_core::profile::TaxpayerProfile;

        let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "ACME CORP.",
            "tin": bir_core::Tin::parse_digits("00012345300000").unwrap(),
            "rdo_code": "044",
            "line_of_business": "TRADING",
            "registered_address": "123 AYALA AVE",
            "zip_code": "1226",
            "phone": "0281234567",
            "email": "tax@acme.ph",
            "default_form_type": "1702MX"
        }))
        .expect("demo profile");
        let truncated = vec![TruncatedEntry {
            page_number: 1,
            label: "Registered Name".into(),
            full_text: "VERY LONG REGISTERED NAME FOR TEST".into(),
        }];

        let mx_fields =
            form_1702mx::Form1702MXDraft::new_from_profile(&profile, 2025).to_print_field_map();
        let mx_html = continuation_pages_html_with_truncated(
            "1702mx-2018c",
            &mx_fields,
            &OverflowReport::default(),
            &truncated,
            "612pt",
            "936pt",
        );
        assert!(
            mx_html.contains("Return Period:</span> 12/25"),
            "1702MX additional sheet must include Return Period, got: {mx_html}"
        );

        let ex_fields =
            form_1702ex::Form1702ExDraft::new_from_profile(&profile, 2025).to_print_field_map();
        let ex_html = continuation_pages_html_with_truncated(
            "1702ex-2018",
            &ex_fields,
            &OverflowReport::default(),
            &truncated,
            "612pt",
            "936pt",
        );
        assert!(
            ex_html.contains("Return Period:</span> 12/25"),
            "1702EX additional sheet must include Return Period, got: {ex_html}"
        );
    }
}
