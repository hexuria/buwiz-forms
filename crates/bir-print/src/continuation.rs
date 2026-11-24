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
        self.schedules.iter().map(OverflowSchedule::warning_line).collect()
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
    pub fn warning_lines(&self) -> Vec<String> {
        self.overflow.warning_lines()
    }

    pub fn warning_summary(&self) -> Option<String> {
        self.overflow.summary()
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
        let base_idx = spec
            .columns
            .iter()
            .map(|c| c.start_idx)
            .min()
            .unwrap_or(0);

        let mut rows = Vec::new();
        let mut col_sums: Vec<f64> = vec![0.0; spec.columns.len()];
        let mut col_has_numeric: Vec<bool> = vec![false; spec.columns.len()];

        for idx in overflow_indices {
            let mut values = Vec::with_capacity(spec.columns.len());
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
            rows.push(OverflowRow { row_number, values });
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
                    Some(format_amount(col_sums[ci]))
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
    if report.is_empty() {
        return String::new();
    }

    let meta = extract_header_meta(slug, fields);
    // Split schedules into page chunks of at most MAX_ROWS_PER_PAGE rows.
    let chunks = paginate_schedules(&report.schedules, MAX_ROWS_PER_PAGE);
    let total_pages = chunks.len();

    let mut out = String::new();
    for (page_idx, page_chunks) in chunks.iter().enumerate() {
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

        out.push_str(&format!(
            "<div class=\"page page-continuation\" id=\"{id_attr}\" style=\"width:{width};min-height:{height};height:auto;overflow:visible;padding:24pt 28pt;box-sizing:border-box;font-family:'eBIRForms Arimo',Arial,Helvetica,sans-serif;font-size:8.5pt;line-height:1.3;color:#000;background:#fff\">"
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
        out.push_str(&format!(
            "<td style=\"width:26%;padding:2pt 0;vertical-align:top;text-align:right\"><span style=\"color:#444;font-weight:700\">Return Period:</span> {}</td>",
            escape_html(&meta.return_period),
        ));
        out.push_str("</tr></tbody></table></div>");

        // Schedule tables on this page
        for chunk in page_chunks {
            let sched = chunk.schedule;
            let rows = chunk.rows;
            let first_row = rows.first().map(|r| r.row_number).unwrap_or(0);
            let last_row = rows.last().map(|r| r.row_number).unwrap_or(0);

            out.push_str("<div style=\"margin-bottom:14pt\">");
            out.push_str(&format!(
                "<div style=\"display:flex;justify-content:space-between;align-items:baseline;margin-bottom:4pt\">\
                 <div style=\"font-size:9.5pt;font-weight:700\">{} (continued)</div>\
                 <div style=\"font-size:7.5pt;color:#444\">Rows {}–{} (continuing after {} printed rows)</div>\
                 </div>",
                escape_html(&sched.heading),
                first_row,
                last_row,
                sched.printed_capacity,
            ));

            out.push_str(
                "<table style=\"width:100%;border-collapse:collapse;table-layout:auto;font-size:7.5pt\"><thead><tr>",
            );
            out.push_str(
                "<th style=\"width:22pt;border:0.75pt solid #000;background:#f2f2f2;padding:3pt 3pt;text-align:center;font-weight:700;vertical-align:bottom\">#</th>",
            );
            for col in &sched.columns {
                let align = if col.is_amount { "right" } else { "left" };
                out.push_str(&format!(
                    "<th style=\"border:0.75pt solid #000;background:#f2f2f2;padding:3pt 4pt;text-align:{align};font-weight:700;vertical-align:bottom;overflow-wrap:break-word\">{}</th>",
                    escape_html(&col.heading),
                ));
            }
            out.push_str("</tr></thead><tbody>");

            for row in rows {
                out.push_str("<tr>");
                out.push_str(&format!(
                    "<td style=\"border:0.75pt solid #000;padding:3pt 3pt;text-align:center;vertical-align:top;color:#333\">{}</td>",
                    row.row_number,
                ));
                for (col, val) in sched.columns.iter().zip(row.values.iter()) {
                    let display = if col.is_amount {
                        parse_amount(val)
                            .map(format_amount)
                            .unwrap_or_else(|| val.clone())
                    } else {
                        val.clone()
                    };
                    let style = if col.is_amount {
                        "border:0.75pt solid #000;padding:3pt 4pt;text-align:right;vertical-align:top;white-space:nowrap;font-variant-numeric:tabular-nums"
                    } else {
                        "border:0.75pt solid #000;padding:3pt 4pt;text-align:left;vertical-align:top;overflow-wrap:break-word"
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

        out.push_str("</div>");
    }

    out
}

struct ScheduleChunk<'a> {
    schedule: &'a OverflowSchedule,
    rows: &'a [OverflowRow],
    is_last_chunk_of_schedule: bool,
}

fn paginate_schedules<'a>(
    schedules: &'a [OverflowSchedule],
    max_rows_per_page: usize,
) -> Vec<Vec<ScheduleChunk<'a>>> {
    let mut pages: Vec<Vec<ScheduleChunk<'a>>> = Vec::new();
    let mut current_page: Vec<ScheduleChunk<'a>> = Vec::new();
    let mut used_rows = 0usize;

    for sched in schedules {
        let mut offset = 0usize;
        while offset < sched.rows.len() {
            // Reserve 2 rows of vertical budget for each schedule header block
            let header_cost = 2usize;
            if !current_page.is_empty() && used_rows + header_cost + 1 > max_rows_per_page {
                pages.push(current_page);
                current_page = Vec::new();
                used_rows = 0;
            }
            let avail = max_rows_per_page
                .saturating_sub(used_rows + header_cost)
                .max(1);
            let end = (offset + avail).min(sched.rows.len());
            let is_last = end == sched.rows.len();
            current_page.push(ScheduleChunk {
                schedule: sched,
                rows: &sched.rows[offset..end],
                is_last_chunk_of_schedule: is_last,
            });
            used_rows += header_cost + (end - offset);
            offset = end;
        }
    }

    if !current_page.is_empty() {
        pages.push(current_page);
    }
    pages
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
        let is_amount =
            group.is_money_section || infer_is_amount(&group.col_heading, &stem);
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
    let digits_end = after
        .bytes()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digits_end == 0 {
        return None;
    }
    let row_num = after[..digits_end].parse::<usize>().ok()?;
    let rest = after[digits_end..].trim_start();
    // Normalize trailing "continuation" so Schedule 1 and Schedule 1 continuation merge
    let sched = strip_trailing_word_ci(before, "continuation").trim().to_string();
    if sched.is_empty() {
        return None;
    }
    Some((sched, row_num, rest))
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

fn format_amount(value: f64) -> String {
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
        format!("-{whole_grouped}.{frac}")
    } else {
        format!("{whole_grouped}.{frac}")
    }
}

struct HeaderMeta {
    form_number: String,
    form_title: String,
    tin: String,
    taxpayer_name: String,
    return_period: String,
}

fn extract_header_meta(slug: &str, fields: &BTreeMap<String, String>) -> HeaderMeta {
    let raw_code = slug_to_form_code(slug);
    let form_number = format!("BIR Form No. {raw_code}");
    let form_title = bir_core::forms::registry::find_form(&raw_code)
        .map(|f| f.title.to_string())
        .unwrap_or_default();

    let tin = extract_tin(fields);
    let taxpayer_name = first_matching_field(
        fields,
        &[
            "txtTaxpayerName",
            "registeredName",
            "txtDonorName",
            "txtAgentName",
            "txtWithholdingAgentName",
        ],
    )
    .unwrap_or_else(|| "—".to_string());

    let return_period = extract_return_period(fields);

    HeaderMeta {
        form_number,
        form_title,
        tin,
        taxpayer_name,
        return_period,
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
    if let Some(d) = fields.get("derived:tin").filter(|v| !v.trim().is_empty()) {
        return format_tin_digits(d);
    }
    let t1 = first_matching_field(fields, &["txtTIN1", "txtTin1"]).unwrap_or_default();
    let t2 = first_matching_field(fields, &["txtTIN2", "txtTin2"]).unwrap_or_default();
    let t3 = first_matching_field(fields, &["txtTIN3", "txtTin3"]).unwrap_or_default();
    let br = first_matching_field(fields, &["txtBranchCode", "txtTIN4", "txtBranch"])
        .unwrap_or_default();
    if !t1.is_empty() && !t2.is_empty() && !t3.is_empty() {
        if br.is_empty() {
            return format!("{t1}-{t2}-{t3}");
        }
        return format!("{t1}-{t2}-{t3}-{br}");
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

fn extract_return_period(fields: &BTreeMap<String, String>) -> String {
    // 1. Explicit date (transaction date / date of donation / date of withholding)
    if let Some(d) = fields
        .get("derived:from_date")
        .filter(|v| !v.trim().is_empty())
    {
        return format_mmddyyyy(d);
    }
    let m = first_matching_field(
        fields,
        &[
            "txtDateMonth",
            "DateWithholdingMonth",
            "txtMonth",
            "rtnMonth",
            "txtMonth1",
        ],
    );
    let d = first_matching_field(fields, &["txtDateDay", "DateWithholdingDay", "txtDay"]);
    let y = first_matching_field(
        fields,
        &[
            "txtDateYear",
            "DateWithholdingYear",
            "txtYear",
            "rtnYear",
            "txtYearEnded",
        ],
    );

    if let (Some(mm), Some(dd), Some(yyyy)) = (&m, &d, &y) {
        if !mm.is_empty() && !dd.is_empty() && !yyyy.is_empty() && mm != "00" && dd != "00" {
            return format!("{mm}/{dd}/{yyyy}");
        }
    }
    if let (Some(mm), Some(yyyy)) = (&m, &y) {
        if !mm.is_empty() && !yyyy.is_empty() && mm != "00" {
            return format!("{mm}/{yyyy}");
        }
    }
    if let Some(yyyy) = y {
        if !yyyy.is_empty() {
            return yyyy;
        }
    }
    "—".to_string()
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
            f.insert(format!("frm2000OT:sched1A:txtOCTNo{i}"), format!("TCT-2026-{n:03}"));
            f.insert(format!("frm2000OT:sched1A:txtTaxDecNo{i}"), format!("TD-2026-{n:03}"));
            f.insert(
                format!("frm2000OT:sched1A:txtLoc{i}"),
                format!("MAKATI CITY PARCEL {n}"),
            );
            f.insert(
                format!("frm2000OT:sched1A:txtLot{i}"),
                format!("LOT {n} BLK 12 / COMMERCIAL BLDG"),
            );
            f.insert(format!("frm2000OT:sched1A:txtClassification{i}"), "CR".into());
            f.insert(format!("frm2000OT:sched1A:txtArea{i}"), format!("{}.00", 200 + n * 10));
            f.insert(
                format!("frm2000OT:sched1A:txtFMVCol1{i}"),
                format!("{:.2}", fmv - 500_000.0),
            );
            f.insert(format!("frm2000OT:sched1A:txtFMVCol2{i}"), format!("{fmv:.2}"));
            f.insert(format!("frm2000OT:sched1A:txtFMVSubTot{i}"), format!("{fmv:.2}"));
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
            f.insert(format!("frm1600WP:dtSched:txtTin{i}"), format!("123-456-{:03}-00000", 780 + i));
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
            f.insert(format!("frm1600WP:dtSched:taxWithheld{i}"), format!("{tax:.2}"));
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
        assert_eq!(sched.columns[8].heading, "Sub-total (between Columns 1 & 2)");
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
            fields.insert(format!("frm2000OT:sched1A:txtFMVSubTot{i}"), "1000000.00".into());
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
            fields_5.insert(format!("frm2000OT:sched1A:txtOCTNo{i}"), format!("TCT-2026-{n:03}"));
            fields_5.insert(format!("frm2000OT:sched1A:txtTaxDecNo{i}"), format!("TD-2026-{n:03}"));
            fields_5.insert(
                format!("frm2000OT:sched1A:txtLoc{i}"),
                format!("MAKATI CITY PARCEL {n}"),
            );
            fields_5.insert(
                format!("frm2000OT:sched1A:txtLot{i}"),
                format!("LOT {n} BLK 12 / COMMERCIAL BLDG"),
            );
            fields_5.insert(format!("frm2000OT:sched1A:txtClassification{i}"), "CR".into());
            fields_5.insert(format!("frm2000OT:sched1A:txtArea{i}"), format!("{}.00", 200 + n * 10));
            fields_5.insert(
                format!("frm2000OT:sched1A:txtFMVCol1{i}"),
                format!("{:.2}", fmv - 500_000.0),
            );
            fields_5.insert(format!("frm2000OT:sched1A:txtFMVCol2{i}"), format!("{fmv:.2}"));
            fields_5.insert(format!("frm2000OT:sched1A:txtFMVSubTot{i}"), format!("{fmv:.2}"));
        }
        let doc_no_overflow = frozen_html::filled_document("2000-ot-2018", &fields_5).unwrap();
        assert!(!doc_no_overflow.contains("page-continuation"));

        let fields_15 = sample_2000ot_15_rows();
        let filled = frozen_html::filled_document_with_report("2000-ot-2018", &fields_15, None)
            .unwrap();
        assert!(filled.html.contains("page-continuation"));
        assert_eq!(
            filled.warning_lines(),
            vec!["Schedule 1.A: 10 rows are on an additional sheet"]
        );

        // The official portion before `<div class="page page-continuation"` must
        // be byte-for-byte identical to the document with no overflow rows!
        let split_at = filled
            .html
            .find("<div class=\"page page-continuation\"")
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
        f2000.insert("frm2000OT:txtRDOCode".into(), "044".into());
        f2000.insert("frm2000OT:txtAddress".into(), "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into());
        f2000.insert("frm2000OT:txtZipCode".into(), "1203".into());
        f2000.insert("frm2000OT:ATC_10".into(), "true".into());
        f2000.insert("frm2000OT:rbTransNature_2".into(), "true".into());
        f2000.insert("frm2000OT:sched1A:txtFMVTotal".into(), "270000000.00".into());
        let doc_2000_15 = frozen_html::filled_document_with_report("2000-ot-2018", &f2000, None).unwrap();
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
        let doc_2000_5 = frozen_html::filled_document_with_report("2000-ot-2018", &f2000_5, None).unwrap();
        assert!(doc_2000_5.overflow.is_empty());
        std::fs::write(out_dir.join("2000ot-no-overflow.html"), &doc_2000_5.html).unwrap();

        // 3. 1800 with 18 rows in Schedule A
        let mut f1800 = sample_1800_18_rows();
        f1800.insert("frm1800v2018:txtRDOCode".into(), "044".into());
        f1800.insert("frm1800v2018:txtAddress".into(), "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into());
        f1800.insert("frm1800v2018:schedA:txtTotalPayment1".into(), "17100000.00".into());
        let doc_1800 = frozen_html::filled_document_with_report("1800-2018", &f1800, None).unwrap();
        std::fs::write(out_dir.join("1800-18-rows.html"), &doc_1800.html).unwrap();
        std::fs::write(
            out_dir.join("1800-18-rows-report.json"),
            serde_json::to_string_pretty(&doc_1800.overflow).unwrap(),
        )
        .unwrap();

        // 4. 1600WP (growables: []) with 10 payees in Schedule 1.1
        let mut f1600wp = sample_1600wp_10_rows();
        f1600wp.insert("frm1600WP:txtRDOCode".into(), "044".into());
        f1600wp.insert("frm1600WP:txtAddress".into(), "123 RIZAL AVE, BRGY. SAN ANTONIO, MAKATI CITY".into());
        f1600wp.insert("derived:WB191_base".into(), "2750000.00".into());
        f1600wp.insert("derived:WB191_withheld".into(), "275000.00".into());
        let doc_1600wp = frozen_html::filled_document_with_report("1600wp-2010", &f1600wp, None).unwrap();
        std::fs::write(out_dir.join("1600wp-10-payees.html"), &doc_1600wp.html).unwrap();
        std::fs::write(
            out_dir.join("1600wp-10-payees-report.json"),
            serde_json::to_string_pretty(&doc_1600wp.overflow).unwrap(),
        )
        .unwrap();
    }
}
