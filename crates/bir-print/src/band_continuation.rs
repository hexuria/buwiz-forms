//! Band-cloning continuation pages for frozen BIR print forms.
//!
//! When an overflowing schedule belongs to a form whose frozen HTML carries
//! `<script type="application/json" id="formgen-bands">` band metadata (23 of
//! 43 forms), this module builds continuation pages that repeat the official
//! paper grid at the exact same pitch, margins, rules, combs, and typography,
//! and fills the cloned cells through `frozen_html`'s own fill path.
//!
//! Forms without band data (`growables: []` in `provenance.json`, e.g. 1600WP,
//! 1801, 1707, 2550Q, 2200A/AN/C/M/P/S/T) fall back to the plain continuation
//! table in [`crate::continuation`].
//!
//! # Band Metadata Format (`hexuria/buwiz-forms-html`)
//!
//! Emitted by `tools/formgen/lattice.py` (`detect_growables`) and serialized
//! into `html-frozen/<slug>/index.html` by `tools/formgen/emit.py`:
//!
//! 1. **`provenance.json`**:
//!    - `paper`: paper size of the official sheet (matches `@page { size: Wpt Hpt }`
//!      and `<div class="page ... " style="width:Wpt;height:Hpt">`).
//!    - `growables`: array of `{ page, rows, capacity, pitch_pt }` summaries.
//!      Empty (`[]`) when the generator did not emit bands for the form.
//!
//! 2. **`<script type="application/json" id="formgen-bands">`** in `index.html`:
//!    JSON array of band objects, one per repeating-row table region:
//!    - `id`: band identifier, e.g. `"p2g0"`.
//!    - `page`: 1-based page number on which the band sits.
//!    - `kind`: `"repeating_rows"`.
//!    - `capacity` / `rendered_rows`: number of pre-rendered rows in the band.
//!    - `row_pitch_pt`: vertical pitch in points between consecutive rows.
//!    - `row_y`: array of `rendered_rows + 1` top/bottom y-coordinates (pt).
//!    - `template_row`: index into `rows[]` cloned for overflow rows (usually `0`).
//!    - `template_boundary`: index into `boundaries[]` cloned for horizontal
//!      row separator rules (usually `0`).
//!    - `ordinal_column`: optional `[x_left, x_right]` (pt) of the row-number column.
//!    - `boundaries`: array of horizontal SVG rule lists (`{x, y, w, h, fill, id}`),
//!      one list per boundary `0..=rendered_rows`.
//!    - `spanning`: array of vertical rules spanning the table:
//!      `{ rect: {x, y, w, h, fill, id}, d_top, d_bottom }`, where the rule runs
//!      from `row_y[0] + d_top` to `row_y[last] + d_bottom`.
//!    - `rows`: array of pre-rendered row objects:
//!      - `index`, `y`: row index and top y (pt).
//!      - `rules`: internal SVG rects inside the row (comb ticks, white masks,
//!        internal column dividers).
//!      - `cells`: array of `{ id, kind ("label"|"field"), row, col, x, y, w, h, field? }`.
//!      - `texts`: array of `{ id, text, x, baseline_y, role?, ... }`, where
//!        `role == "enumerated"` marks the row's printed ordinal (`"1"`, `"2"`, …).
//!
//! 3. **`<template id="band-template-<band.id>">`** in `index.html`:
//!    Anonymous prototype DOM for one row at `top:0pt` (used by the browser JS
//!    runtime; in Rust we clone the concrete `<div id="<cell.id>" ...>` nodes
//!    from the source page so all `<input>` comb slots and metrics match).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::continuation::{self, OverflowReport, OverflowSchedule};
use crate::frozen_html;

/// Bottom margin reserved on cloned continuation pages (points).
const PAGE_BOTTOM_MARGIN_PT: f64 = 36.0;
/// Vertical gap between two stacked half-tables (or schedules) on one page.
const SECTION_GAP_PT: f64 = 10.0;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct FormgenBand {
    pub id: String,
    pub page: usize,
    pub kind: String,
    pub capacity: usize,
    pub row_pitch_pt: f64,
    pub row_y: Vec<f64>,
    pub template_row: usize,
    pub template_boundary: usize,
    #[serde(default)]
    pub ordinal_column: Option<[f64; 2]>,
    #[serde(default)]
    pub boundaries: Vec<Vec<BandRect>>,
    #[serde(default)]
    pub spanning: Vec<BandSpanning>,
    #[serde(default)]
    pub rows: Vec<BandRow>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BandRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub fill: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BandSpanning {
    pub rect: BandRect,
    pub d_top: f64,
    pub d_bottom: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BandRow {
    pub index: usize,
    pub y: f64,
    #[serde(default)]
    pub rules: Vec<BandRect>,
    #[serde(default)]
    pub cells: Vec<BandCell>,
    #[serde(default)]
    pub texts: Vec<BandText>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BandCell {
    pub id: String,
    pub kind: String,
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BandText {
    pub id: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub baseline_y: f64,
}

/// Output of building continuation pages for a filled return.
pub(crate) struct ContinuationOutput {
    /// Cloned-band continuation page fragments (`<div class="page page-continuation-band" ...>`).
    pub pages_html: String,
    /// Schedules without matching band metadata that must be rendered on plain
    /// `.page-continuation` sheets (unified with any truncated-text entries).
    pub fallback_schedules: Vec<OverflowSchedule>,
}

/// Parse `<script type="application/json" id="formgen-bands">...</script>` from
/// a frozen sheet's HTML. Returns an empty `Vec` if absent or empty.
pub(crate) fn parse_formgen_bands(html: &str) -> Vec<FormgenBand> {
    let Some(id_pos) = html.find("id=\"formgen-bands\"") else {
        return Vec::new();
    };
    let Some(gt_rel) = html[id_pos..].find('>') else {
        return Vec::new();
    };
    let content_start = id_pos + gt_rel + 1;
    let Some(end_rel) = html[content_start..].find("</script>") else {
        return Vec::new();
    };
    let json_str = html[content_start..content_start + end_rel].trim();
    if json_str.is_empty() || json_str == "[]" {
        return Vec::new();
    }
    serde_json::from_str::<Vec<FormgenBand>>(json_str).unwrap_or_default()
}

/// Build continuation pages for `report`:
/// - Schedules whose form has matching `formgen-bands` render as cloned-band
///   official-grid continuation pages filled via `frozen_html`'s own fill path.
/// - Schedules without matching band data fall back to the plain continuation
///   table in [`crate::continuation`].
pub(crate) fn build_continuation_pages(
    slug: &str,
    raw_html: &str,
    fields: &BTreeMap<String, String>,
    report: &OverflowReport,
    width: &str,
    height: &str,
) -> Result<ContinuationOutput, String> {
    if report.is_empty() {
        return Ok(ContinuationOutput {
            pages_html: String::new(),
            fallback_schedules: Vec::new(),
        });
    }

    let bands = parse_formgen_bands(raw_html);
    let wc_parsed: Option<Value> =
        frozen_html::writer_cells_json(slug).and_then(|s| serde_json::from_str::<Value>(s).ok());

    let mut pages_html = String::new();
    let mut fallback_schedules: Vec<OverflowSchedule> = Vec::new();

    for (sched_idx, sched) in report.schedules.iter().enumerate() {
        let matched = wc_parsed
            .as_ref()
            .and_then(|wc| match_schedule_to_bands(sched, &bands, wc));
        match matched {
            Some(plan) => {
                let html_frag = render_band_continuation_for_schedule(
                    slug, raw_html, fields, sched, sched_idx, &plan,
                )?;
                pages_html.push_str(&html_frag);
            }
            None => {
                fallback_schedules.push(sched.clone());
            }
        }
    }

    let _ = (width, height);
    Ok(ContinuationOutput {
        pages_html,
        fallback_schedules,
    })
}

type DatePartIds = (String, Option<String>, Option<String>, Option<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatePart {
    Month,
    Day,
    Year,
}

/// How one cell in a band's template row maps to an `OverflowSchedule` column.
#[derive(Debug, Clone)]
enum CellJoinKind {
    Text {
        col_idx: usize,
        orig_writer_key: String,
    },
    Money {
        col_idx: usize,
        orig_writer_key: String,
        is_peso: bool,
    },
    Peso {
        col_idx: usize,
        orig_writer_key: String,
    },
    /// One part of a writer date split across the sheet's MM, DD and YYYY
    /// combs (`writer-cells.json`'s `date_joins`).
    Date {
        col_idx: usize,
        orig_writer_key: String,
        part: DatePart,
    },
    /// Print-only sequence/ordinal cell (e.g. `1600WP`'s `derived:sched_seq*`),
    /// filled with the continuing row number (`7`, `8`, `9`, …) through the
    /// normal text fill path.
    Ordinal { orig_writer_key: String },
}

#[derive(Debug, Clone)]
struct MatchedBand {
    band: FormgenBand,
    /// `template_cell_id -> CellJoinKind`
    cell_map: BTreeMap<String, CellJoinKind>,
    /// Earliest `y` of row 1 of the columns in this band (may be above `band.row_y[0]`
    /// when e.g. rows 1–3 sit above a band starting at row 4, as on 1800 Schedule A).
    first_printed_row_cell_ids: Vec<String>,
}

struct ScheduleBandPlan {
    page_num: usize,
    bands: Vec<MatchedBand>,
    /// Official total writer key and its html_id (or peso+cents ids) if present.
    total_join: Option<TotalJoinInfo>,
}

#[derive(Debug, Clone)]
struct TotalJoinInfo {
    writer_key: String,
    html_id: String,
    cents_html_id: Option<String>,
    is_money: bool,
    is_peso: bool,
}

fn match_schedule_to_bands(
    sched: &OverflowSchedule,
    bands: &[FormgenBand],
    wc: &Value,
) -> Option<ScheduleBandPlan> {
    if bands.is_empty() {
        return None;
    }

    // Build html_id -> (col_idx, join_kind) across ALL printed rows of this schedule.
    // We also track row-1 html_ids per column to locate the schedule's heading strip.
    let mut html_to_col: BTreeMap<String, CellJoinKind> = BTreeMap::new();
    let mut col_row1_ids: Vec<Vec<String>> = vec![Vec::new(); sched.columns.len()];
    let mut ord_row1_ids: Vec<String> = Vec::new();

    for (section, kind_tag) in [
        ("joins", "text"),
        ("money_joins", "money"),
        ("peso_joins", "peso"),
    ] {
        let Some(arr) = wc.get(section).and_then(Value::as_array) else {
            continue;
        };
        for item in arr {
            let Some(printed) = item.get("printed").and_then(Value::as_str) else {
                continue;
            };
            let orig_writer_key = item
                .get("writer_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let parsed_row = continuation::parse_schedule_row_and_column_pub(printed)
                .or_else(|| parse_non_schedule_band_caption(printed, &orig_writer_key));
            let Some((sched_name, row_num, col_heading)) = parsed_row else {
                continue;
            };
            if !sched_name.eq_ignore_ascii_case(&sched.heading) {
                continue;
            }

            // Print-only sequence/ordinal column (e.g. 1600WP `derived:sched_seq*`).
            if kind_tag == "text"
                && orig_writer_key.starts_with("derived:")
                && (orig_writer_key.to_ascii_lowercase().contains("seq")
                    || col_heading.to_ascii_lowercase().contains("seq"))
            {
                if let Some(hid) = item.get("html_id").and_then(Value::as_str) {
                    html_to_col.insert(
                        hid.to_string(),
                        CellJoinKind::Ordinal {
                            orig_writer_key: orig_writer_key.clone(),
                        },
                    );
                    if row_num == 1 {
                        ord_row1_ids.push(hid.to_string());
                    }
                }
                continue;
            }

            // Match column by heading (exact or prefix match for row-1 fuller captions)
            let Some(col_idx) = sched.columns.iter().position(|c| {
                c.heading.eq_ignore_ascii_case(&col_heading)
                    || c.heading
                        .to_ascii_lowercase()
                        .starts_with(&col_heading.to_ascii_lowercase())
                    || col_heading
                        .to_ascii_lowercase()
                        .starts_with(&c.heading.to_ascii_lowercase())
            }) else {
                continue;
            };

            match kind_tag {
                "text" => {
                    if let Some(hid) = item.get("html_id").and_then(Value::as_str) {
                        html_to_col.insert(
                            hid.to_string(),
                            CellJoinKind::Text {
                                col_idx,
                                orig_writer_key: orig_writer_key.clone(),
                            },
                        );
                        if row_num == 1 {
                            col_row1_ids[col_idx].push(hid.to_string());
                        }
                    }
                }
                "money" => {
                    if let Some(pid) = item.get("peso_html_id").and_then(Value::as_str) {
                        html_to_col.insert(
                            pid.to_string(),
                            CellJoinKind::Money {
                                col_idx,
                                orig_writer_key: orig_writer_key.clone(),
                                is_peso: true,
                            },
                        );
                        if row_num == 1 {
                            col_row1_ids[col_idx].push(pid.to_string());
                        }
                    }
                    if let Some(cid) = item
                        .get("cent_html_id")
                        .or_else(|| item.get("cents_html_id"))
                        .and_then(Value::as_str)
                    {
                        html_to_col.insert(
                            cid.to_string(),
                            CellJoinKind::Money {
                                col_idx,
                                orig_writer_key: orig_writer_key.clone(),
                                is_peso: false,
                            },
                        );
                    }
                }
                "peso" => {
                    if let Some(hid) = item.get("html_id").and_then(Value::as_str) {
                        html_to_col.insert(
                            hid.to_string(),
                            CellJoinKind::Peso {
                                col_idx,
                                orig_writer_key: orig_writer_key.clone(),
                            },
                        );
                        if row_num == 1 {
                            col_row1_ids[col_idx].push(hid.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(arr) = wc.get("date_joins").and_then(Value::as_array) {
        for item in arr {
            let Some(printed) = item.get("printed").and_then(Value::as_str) else {
                continue;
            };
            let orig_writer_key = item
                .get("writer_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let Some((sched_name, row_num, col_heading)) =
                continuation::parse_schedule_row_and_column_pub(printed)
            else {
                continue;
            };
            if !sched_name.eq_ignore_ascii_case(&sched.heading) {
                continue;
            }
            let Some(col_idx) = sched.columns.iter().position(|c| {
                c.heading.eq_ignore_ascii_case(&col_heading)
                    || c.heading
                        .to_ascii_lowercase()
                        .starts_with(&col_heading.to_ascii_lowercase())
                    || col_heading
                        .to_ascii_lowercase()
                        .starts_with(&c.heading.to_ascii_lowercase())
            }) else {
                continue;
            };
            for (field, part) in [
                ("mm_html_id", DatePart::Month),
                ("dd_html_id", DatePart::Day),
                ("yyyy_html_id", DatePart::Year),
            ] {
                if let Some(hid) = item.get(field).and_then(Value::as_str) {
                    html_to_col.insert(
                        hid.to_string(),
                        CellJoinKind::Date {
                            col_idx,
                            orig_writer_key: orig_writer_key.clone(),
                            part,
                        },
                    );
                    if row_num == 1 {
                        col_row1_ids[col_idx].push(hid.to_string());
                    }
                }
            }
        }
    }

    if html_to_col.is_empty() {
        return None;
    }

    // Match bands whose template_row cells overlap `html_to_col`.
    let mut matched_bands: Vec<MatchedBand> = Vec::new();
    let mut covered_cols: BTreeSet<usize> = BTreeSet::new();
    for band in bands {
        let Some(trow) = band.rows.get(band.template_row) else {
            continue;
        };
        let mut cell_map = BTreeMap::new();
        let mut row1_ids = Vec::new();
        let mut has_data_col = false;
        for cell in &trow.cells {
            if let Some(kind) = html_to_col.get(&cell.id) {
                match kind {
                    CellJoinKind::Text { col_idx, .. }
                    | CellJoinKind::Money { col_idx, .. }
                    | CellJoinKind::Peso { col_idx, .. }
                    | CellJoinKind::Date { col_idx, .. } => {
                        has_data_col = true;
                        covered_cols.insert(*col_idx);
                        row1_ids.extend(col_row1_ids[*col_idx].iter().cloned());
                    }
                    CellJoinKind::Ordinal { .. } => {
                        row1_ids.extend(ord_row1_ids.iter().cloned());
                    }
                }
                cell_map.insert(cell.id.clone(), kind.clone());
            }
        }
        if has_data_col {
            matched_bands.push(MatchedBand {
                band: band.clone(),
                cell_map,
                first_printed_row_cell_ids: row1_ids,
            });
        }
    }

    if matched_bands.is_empty() {
        return None;
    }

    // Never silently drop a column: if the official formgen-bands metadata
    // omits one or more columns of this schedule (e.g. 1707 Schedule 2's
    // upper Name of Corporate Stock rows), fall back to the plain table.
    if covered_cols.len() < sched.columns.len() {
        return None;
    }

    // All bands for one schedule live on the same official page.
    let page_num = matched_bands[0].band.page;
    matched_bands.retain(|b| b.band.page == page_num);
    matched_bands.sort_by(|a, b| {
        a.band
            .row_y
            .first()
            .unwrap_or(&0.0)
            .partial_cmp(b.band.row_y.first().unwrap_or(&0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Find the official total join for this schedule (if any).
    let total_join = find_schedule_total_join(sched, wc, &matched_bands);

    Some(ScheduleBandPlan {
        page_num,
        bands: matched_bands,
        total_join,
    })
}

/// Handle repeating-band captions that don't use `"Schedule X row N"` wording
/// (currently `1701A` Part IV-A items 41–42 and Part IV-B items 50–51).
fn parse_non_schedule_band_caption(
    printed: &str,
    writer_key: &str,
) -> Option<(String, usize, String)> {
    let trimmed = printed.trim();
    if writer_key.starts_with("frm1701A:txt41")
        || writer_key.starts_with("frm1701A:txt42")
        || writer_key.starts_with("derived:amount_41")
        || writer_key.starts_with("derived:amount_42")
    {
        let row_num = if writer_key.contains("41") { 1 } else { 2 };
        let col = trimmed
            .strip_prefix("41 ")
            .or_else(|| trimmed.strip_prefix("42 "))
            .unwrap_or(trimmed);
        let col = col
            .strip_prefix("Other Non-Operating Income ")
            .unwrap_or(col)
            .trim();
        let col = if col == "(specify)" {
            "Other Non-Operating Income (specify)"
        } else {
            col
        };
        return Some((
            "Part IV-A Other Non-Operating Income".to_string(),
            row_num,
            col.to_string(),
        ));
    }
    if writer_key.starts_with("frm1701A:txt50")
        || writer_key.starts_with("frm1701A:txt51")
        || writer_key.starts_with("derived:amount_50")
        || writer_key.starts_with("derived:amount_51")
    {
        let row_num = if writer_key.contains("50") { 1 } else { 2 };
        let col = trimmed
            .strip_prefix("50 ")
            .or_else(|| trimmed.strip_prefix("51 "))
            .unwrap_or(trimmed);
        let col = col
            .strip_prefix("Other Non-Operating Income ")
            .unwrap_or(col)
            .trim();
        let col = if col == "(specify)" {
            "Other Non-Operating Income (specify)"
        } else {
            col
        };
        return Some((
            "Part IV-B Other Non-Operating Income".to_string(),
            row_num,
            col.to_string(),
        ));
    }
    None
}

fn find_schedule_total_join(
    sched: &OverflowSchedule,
    wc: &Value,
    matched_bands: &[MatchedBand],
) -> Option<TotalJoinInfo> {
    // Derive namespace prefix from any writer_key that targets one of our band cells.
    let band_cell_ids: BTreeSet<&str> = matched_bands
        .iter()
        .flat_map(|b| b.cell_map.keys().map(String::as_str))
        .collect();

    let mut ns_prefixes: BTreeSet<String> = BTreeSet::new();
    for sec in ["joins", "money_joins", "peso_joins"] {
        if let Some(arr) = wc.get(sec).and_then(Value::as_array) {
            for item in arr {
                let hid = item
                    .get("html_id")
                    .or_else(|| item.get("peso_html_id"))
                    .and_then(Value::as_str);
                if let Some(h) = hid {
                    if band_cell_ids.contains(h) {
                        if let Some(wk) = item.get("writer_key").and_then(Value::as_str) {
                            if let Some(colon) = wk.rfind(':') {
                                ns_prefixes.insert(wk[..=colon].to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let sched_lower = sched.heading.to_ascii_lowercase();
    let mut best_same_ns: Option<TotalJoinInfo> = None;
    let mut best_mentions: Option<TotalJoinInfo> = None;
    for (sec, is_money, is_peso) in [
        ("joins", false, false),
        ("money_joins", true, false),
        ("peso_joins", false, true),
    ] {
        let Some(arr) = wc.get(sec).and_then(Value::as_array) else {
            continue;
        };
        for item in arr {
            let Some(wk) = item.get("writer_key").and_then(Value::as_str) else {
                continue;
            };
            let pr = item.get("printed").and_then(Value::as_str).unwrap_or("");
            let wk_lower = wk.to_ascii_lowercase();
            let pr_lower = pr.to_ascii_lowercase();
            let is_total = (wk_lower.contains("total") || wk_lower.contains("tot"))
                && !wk_lower.contains("subtot")
                && pr_lower.contains("total")
                && !pr_lower.contains("sub-total");
            if !is_total {
                continue;
            }
            let same_ns = ns_prefixes.iter().any(|p| wk.starts_with(p));
            let mentions_sched = pr_lower.contains(&sched_lower);
            if !same_ns && !mentions_sched {
                continue;
            }
            let Some(html_id) = item
                .get("html_id")
                .or_else(|| item.get("peso_html_id"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let cents_html_id = item
                .get("cent_html_id")
                .or_else(|| item.get("cents_html_id"))
                .and_then(Value::as_str)
                .map(String::from);
            let info = TotalJoinInfo {
                writer_key: wk.to_string(),
                html_id: html_id.to_string(),
                cents_html_id,
                is_money,
                is_peso,
            };
            if same_ns && mentions_sched {
                return Some(info);
            }
            if same_ns && best_same_ns.is_none() {
                best_same_ns = Some(info);
            } else if mentions_sched && best_mentions.is_none() {
                best_mentions = Some(info);
            }
        }
    }
    best_same_ns.or(best_mentions)
}

// ---------------------------------------------------------------------------
// Source page parser (SVG images/rects, .t text divs, .c cell divs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SvgRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: String,
    layer: String,
}

#[derive(Debug, Clone)]
struct TextNode {
    left: f64,
    top: f64,
    raw_html: String,
    plain_text: String,
}

#[derive(Debug, Clone)]
struct CellNode {
    #[allow(dead_code)]
    id: String,
    top: f64,
    height: f64,
    raw_html: String,
}

struct ParsedPage {
    width_pt: f64,
    height_pt: f64,
    svg_images: Vec<String>,
    svg_rects: Vec<SvgRect>,
    texts: Vec<TextNode>,
    cells: BTreeMap<String, CellNode>,
}

fn parse_source_page(raw_html: &str, page_num: usize) -> Result<ParsedPage, String> {
    let marker = format!("class=\"page page-{page_num}\"");
    let start = raw_html
        .find(&marker)
        .ok_or_else(|| format!("page {page_num} not found in frozen HTML"))?;
    let div_start = raw_html[..start]
        .rfind("<div")
        .ok_or_else(|| format!("malformed page {page_num} div"))?;

    // Find end of this page (either next `<div class="page ` or `<script` or `</body>`)
    let search_from = start + marker.len();
    let end = raw_html[search_from..]
        .find("<div class=\"page ")
        .or_else(|| raw_html[search_from..].find("<script"))
        .or_else(|| raw_html[search_from..].find("</body>"))
        .map(|rel| search_from + rel)
        .unwrap_or(raw_html.len());
    let page_html = &raw_html[div_start..end];

    let tag_end = page_html.find('>').unwrap_or(0);
    let open_tag = &page_html[..tag_end];
    let width_pt = extract_style_pt(open_tag, "width:").unwrap_or(612.0);
    let height_pt = extract_style_pt(open_tag, "height:").unwrap_or(936.0);

    // 1. Parse SVG <image .../> and <rect .../> inside <svg class="rl" ...>...</svg>
    let mut svg_images = Vec::new();
    let mut svg_rects = Vec::new();
    if let Some(svg_start) = page_html.find("<svg ") {
        if let Some(svg_end_rel) = page_html[svg_start..].find("</svg>") {
            let svg = &page_html[svg_start..svg_start + svg_end_rel];
            // Collect <image .../>
            let mut pos = 0;
            while let Some(rel) = svg[pos..].find("<image ") {
                let a = pos + rel;
                if let Some(close) = svg[a..].find("/>") {
                    svg_images.push(svg[a..a + close + 2].to_string());
                    pos = a + close + 2;
                } else {
                    break;
                }
            }
            // Collect <rect .../> with enclosing <g class="..."> layer
            let mut cur_layer = "layer-structural".to_string();
            let mut scan = 0;
            while scan < svg.len() {
                let next_g = svg[scan..].find("<g ");
                let next_r = svg[scan..].find("<rect ");
                match (next_g, next_r) {
                    (Some(g), Some(r)) if g < r => {
                        let a = scan + g;
                        let end_g = svg[a..].find('>').map(|e| a + e + 1).unwrap_or(svg.len());
                        if let Some(cls) = extract_attr(&svg[a..end_g], "class") {
                            cur_layer = cls.to_string();
                        }
                        scan = end_g;
                    }
                    (_, Some(r)) => {
                        let a = scan + r;
                        let end_r = svg[a..].find("/>").map(|e| a + e + 2).unwrap_or(svg.len());
                        let tag = &svg[a..end_r];
                        if let (Some(x), Some(y), Some(w), Some(h)) = (
                            extract_attr(tag, "x").and_then(|v| v.parse::<f64>().ok()),
                            extract_attr(tag, "y").and_then(|v| v.parse::<f64>().ok()),
                            extract_attr(tag, "width").and_then(|v| v.parse::<f64>().ok()),
                            extract_attr(tag, "height").and_then(|v| v.parse::<f64>().ok()),
                        ) {
                            let fill = extract_attr(tag, "fill").unwrap_or("#000000").to_string();
                            svg_rects.push(SvgRect {
                                x,
                                y,
                                w,
                                h,
                                fill,
                                layer: cur_layer.clone(),
                            });
                        }
                        scan = end_r;
                    }
                    _ => break,
                }
            }
        }
    }

    // 2. Parse `.t` text elements (outside `<template>`). Elements carry
    //    `class="t"` or `class="t ..."`.
    let html_no_tpl = strip_templates(page_html);
    let mut texts = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html_no_tpl[pos..].find("class=\"t") {
        let hit = pos + rel;
        let after_t = html_no_tpl.as_bytes().get(hit + 8).copied();
        if after_t != Some(b'"') && after_t != Some(b' ') {
            pos = hit + 8;
            continue;
        }
        let Some(tag_open) = html_no_tpl[..hit].rfind('<') else {
            pos = hit + 8;
            continue;
        };
        let Some(tag_close_rel) = html_no_tpl[tag_open..].find('>') else {
            break;
        };
        let tag_close = tag_open + tag_close_rel;
        let open_slice = &html_no_tpl[tag_open..=tag_close];
        let tag_name = open_slice[1..]
            .split(|c: char| c.is_ascii_whitespace() || c == '>')
            .next()
            .unwrap_or("div");
        let close_marker = format!("</{tag_name}>");
        let Some(end_rel) = html_no_tpl[tag_close + 1..].find(&close_marker) else {
            pos = tag_close + 1;
            continue;
        };
        let elem_end = tag_close + 1 + end_rel + close_marker.len();
        let raw_elem = html_no_tpl[tag_open..elem_end].to_string();
        let inner = &html_no_tpl[tag_close + 1..tag_close + 1 + end_rel];
        let left = extract_style_pt(open_slice, "left:").unwrap_or(0.0);
        let top = extract_style_pt(open_slice, "top:").unwrap_or(0.0);
        texts.push(TextNode {
            left,
            top,
            raw_html: raw_elem,
            plain_text: strip_html_tags(inner).trim().to_string(),
        });
        pos = elem_end;
    }

    // 3. Parse `.c` cell divs (`<div id="p2c..." class="c ..." ...>...</div>`)
    let mut cells = BTreeMap::new();
    let mut cpos = 0;
    while let Some(rel) = html_no_tpl[cpos..].find("<div id=\"") {
        let a = cpos + rel;
        let Some(gt_rel) = html_no_tpl[a..].find('>') else {
            break;
        };
        let open_tag = &html_no_tpl[a..=a + gt_rel];
        let cls = extract_attr(open_tag, "class").unwrap_or("");
        let id = extract_attr(open_tag, "id").unwrap_or("");
        if (cls == "c" || cls.starts_with("c ")) && !id.is_empty() {
            if let Some(elem_end) = find_matching_div_end(&html_no_tpl, a) {
                let top = extract_style_pt(open_tag, "top:").unwrap_or(0.0);
                let height = extract_style_pt(open_tag, "height:").unwrap_or(0.0);
                cells.insert(
                    id.to_string(),
                    CellNode {
                        id: id.to_string(),
                        top,
                        height,
                        raw_html: html_no_tpl[a..elem_end].to_string(),
                    },
                );
                cpos = elem_end;
                continue;
            }
        }
        cpos = a + gt_rel + 1;
    }

    Ok(ParsedPage {
        width_pt,
        height_pt,
        svg_images,
        svg_rects,
        texts,
        cells,
    })
}

fn render_band_continuation_for_schedule(
    slug: &str,
    raw_html: &str,
    fields: &BTreeMap<String, String>,
    sched: &OverflowSchedule,
    sched_idx: usize,
    plan: &ScheduleBandPlan,
) -> Result<String, String> {
    let page = parse_source_page(raw_html, plan.page_num)?;

    // 1. Locate the official Page N header bottom (`header_bottom_y`):
    //    the bottom of the TIN/Taxpayer's Name strip above the first band.
    let first_band_y = plan.bands[0].band.row_y.first().copied().unwrap_or(160.0);
    let header_bottom_y = detect_page_header_bottom(&page, first_band_y);

    // 2. For each matched band, compute its heading strip `[hdr_top, table_row0_y]`
    //    on the official page so we can clone the exact table title + column headers.
    let mut band_hdr_ranges: Vec<(f64, f64)> = Vec::with_capacity(plan.bands.len());
    let mut prev_bottom = header_bottom_y;
    for mb in &plan.bands {
        let band_y0 = mb.band.row_y.first().copied().unwrap_or(0.0);
        let row1_min_y = mb
            .first_printed_row_cell_ids
            .iter()
            .filter_map(|id| page.cells.get(id).map(|c| c.top))
            .fold(band_y0, f64::min);
        let table_top_y = row1_min_y.min(band_y0);
        let hdr_top = find_band_header_top(&page, prev_bottom, table_top_y, &mb.band);
        band_hdr_ranges.push((hdr_top, table_top_y));
        // Next band's header starts right after this band's printed rows.
        let band_y_end = mb.band.row_y.last().copied().unwrap_or(band_y0);
        prev_bottom = band_y_end + 0.5;
    }

    // 3. Locate the official TOTAL row strip if the schedule has a total join.
    let total_strip: Option<(f64, f64, &TotalJoinInfo)> = plan.total_join.as_ref().and_then(|tj| {
        let cell = page.cells.get(&tj.html_id)?;
        Some((cell.top - 1.0, cell.top + cell.height + 1.0, tj))
    });
    let total_height = total_strip
        .map(|(t0, t1, _)| (t1 - t0) + 4.0)
        .unwrap_or(0.0);

    // 4. Paginate `sched.rows` across as many official-sized pages as needed.
    //    On each page, all half-tables (`plan.bands`) for the same chunk of rows
    //    appear together (e.g. on 2000-OT, rows 6–15 top half + rows 6–15 bottom half).
    let usable_height = page.height_pt - PAGE_BOTTOM_MARGIN_PT - (header_bottom_y + 6.0);
    let fixed_headers_height: f64 = band_hdr_ranges
        .iter()
        .map(|(h0, h1)| (h1 - h0).max(0.0) + SECTION_GAP_PT)
        .sum();
    let per_row_height: f64 = plan.bands.iter().map(|b| b.band.row_pitch_pt).sum();
    let max_rows_per_page = if per_row_height > 0.1 {
        ((usable_height - fixed_headers_height - total_height) / per_row_height)
            .floor()
            .max(1.0) as usize
    } else {
        15
    };

    let mut all_pages_html = String::new();
    let mut row_offset = 0usize;
    let mut cont_page_idx = 0usize;

    while row_offset < sched.rows.len() {
        let row_end = (row_offset + max_rows_per_page).min(sched.rows.len());
        let is_last_page = row_end == sched.rows.len();
        let row_slice = &sched.rows[row_offset..row_end];

        let page_html = build_single_band_continuation_page(
            slug,
            &page,
            plan.page_num,
            fields,
            sched,
            sched_idx,
            cont_page_idx,
            header_bottom_y,
            &plan.bands,
            &band_hdr_ranges,
            row_slice,
            if is_last_page { total_strip } else { None },
        )?;
        all_pages_html.push_str(&page_html);

        row_offset = row_end;
        cont_page_idx += 1;
    }

    Ok(all_pages_html)
}

#[allow(clippy::too_many_arguments)]
fn build_single_band_continuation_page(
    slug: &str,
    page: &ParsedPage,
    official_page_num: usize,
    fields: &BTreeMap<String, String>,
    sched: &OverflowSchedule,
    sched_idx: usize,
    cont_page_idx: usize,
    header_bottom_y: f64,
    matched_bands: &[MatchedBand],
    band_hdr_ranges: &[(f64, f64)],
    rows: &[continuation::OverflowRow],
    total_strip: Option<(f64, f64, &TotalJoinInfo)>,
) -> Result<String, String> {
    let mut svg_structural = String::new();
    let mut svg_decorative = String::new();
    let mut body_texts = String::new();
    let mut body_cells = String::new();

    // Synthetic joins + fields + caption inheritance (`syn_id -> orig_id`)
    // for filling cloned cells via frozen_html's own fill path
    let mut syn_fields = fields.clone();
    let mut extra_joins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut extra_money_joins: Vec<(String, String, Option<String>)> = Vec::new();
    let mut extra_peso_joins: Vec<(String, String)> = Vec::new();
    let mut extra_date_joins: Vec<(String, String, Option<String>, String)> = Vec::new();
    let mut caption_aliases: BTreeMap<String, String> = BTreeMap::new();

    // --- 1. Clone Page Header (y <= header_bottom_y) ---
    let mut header_has_tin = false;
    for r in &page.svg_rects {
        if r.y < header_bottom_y && r.y + r.h <= header_bottom_y + 2.0 {
            push_svg_rect(&mut svg_structural, &mut svg_decorative, r, 0.0);
        }
    }
    for t in &page.texts {
        if t.top < header_bottom_y - 1.0 {
            let mut html = t.raw_html.clone();
            if t.plain_text.trim().eq_ignore_ascii_case("TIN")
                || t.plain_text.contains("Taxpayer")
                || t.plain_text.contains("Donor")
                || t.plain_text.contains("Decedent")
            {
                header_has_tin = true;
            }
            if t.plain_text.starts_with("Page ") {
                html = replace_inner_text(&html, "Add'l Sheet");
            }
            body_texts.push_str(&html);
        }
    }
    for c in page.cells.values() {
        if c.top < header_bottom_y - 1.0 {
            body_cells.push_str(&c.raw_html);
        }
    }

    // --- 2. Stamp each matched band (heading strip + cloned rows) ---
    let mut cursor_y = header_bottom_y + 4.0;
    // If the official later page has no TIN/Taxpayer's Name strip (e.g. Form 2552
    // Page 2), stamp a compact paper-styled identity strip so every continuation
    // sheet still carries TIN, Taxpayer's Name, and Return Period.
    if !header_has_tin {
        let meta = continuation::extract_header_meta_pub(slug, fields);
        let strip_y = cursor_y;
        let strip_h = 15.0;
        let strip_x = 22.0;
        let strip_w = page.width_pt - 44.0;
        svg_structural.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"#e6e6e6\"/>\
             <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"0.75\" fill=\"#000000\"/>\
             <rect x=\"{x}\" y=\"{y2}\" width=\"{w}\" height=\"0.75\" fill=\"#000000\"/>\
             <rect x=\"{x}\" y=\"{y}\" width=\"0.75\" height=\"{h}\" fill=\"#000000\"/>\
             <rect x=\"{x2}\" y=\"{y}\" width=\"0.75\" height=\"{h}\" fill=\"#000000\"/>",
            x = fmt_pt(strip_x),
            y = fmt_pt(strip_y),
            w = fmt_pt(strip_w),
            h = fmt_pt(strip_h),
            y2 = fmt_pt(strip_y + strip_h - 0.75),
            x2 = fmt_pt(strip_x + strip_w - 0.75),
        ));
        let mut parts = Vec::new();
        if !meta.tin.is_empty() {
            parts.push(format!("TIN: {}", meta.tin));
        }
        if !meta.taxpayer_name.is_empty() {
            parts.push(format!("Taxpayer: {}", meta.taxpayer_name));
        }
        if !meta.period.is_empty() {
            parts.push(format!("Period: {}", meta.period));
        }
        let info = parts.join("   |   ");
        body_texts.push_str(&format!(
            "<div class=\"t\" style=\"left:{}pt;top:{}pt;font-family:Arial,Helvetica,sans-serif;font-size:7.5pt;font-weight:700;color:#000000\">{}</div>",
            fmt_pt(strip_x + 4.0),
            fmt_pt(strip_y + 3.0),
            escape_html(&info),
        ));
        cursor_y = strip_y + strip_h + 6.0;
    }

    for (bi, (mb, &(hdr_top, hdr_bottom))) in
        matched_bands.iter().zip(band_hdr_ranges.iter()).enumerate()
    {
        let hdr_height = (hdr_bottom - hdr_top).max(0.0);
        let dy_hdr = cursor_y - hdr_top;

        // 2a. Heading strip SVG rects (clamp vertical spanning lines to [hdr_top, hdr_bottom])
        for r in &page.svg_rects {
            let r_bottom = r.y + r.h;
            if r_bottom <= hdr_top + 0.2 || r.y >= hdr_bottom - 0.2 {
                continue;
            }
            let cy0 = r.y.max(hdr_top);
            let cy1 = r_bottom.min(hdr_bottom);
            if cy1 - cy0 <= 0.05 {
                continue;
            }
            let clamped = SvgRect {
                x: r.x,
                y: cy0,
                w: r.w,
                h: cy1 - cy0,
                fill: r.fill.clone(),
                layer: r.layer.clone(),
            };
            push_svg_rect(&mut svg_structural, &mut svg_decorative, &clamped, dy_hdr);
        }

        // 2b. Heading strip `.t` labels (mark schedule title with "(continued)"
        //     without colliding with any adjacent "(Attach additional sheet/s…)" span
        //     or duplicating a heading that already says "continuation"/"continued")
        let hdr_texts: Vec<&TextNode> = page
            .texts
            .iter()
            .filter(|t| t.top >= hdr_top - 0.5 && t.top < hdr_bottom - 0.5)
            .collect();
        let already_continued = hdr_texts.iter().any(|t| {
            let lower = t.plain_text.to_ascii_lowercase();
            lower.contains("continuation") || lower.contains("continued")
        });
        let mut marked_continued = already_continued;
        let sched_lower = sched.heading.to_ascii_lowercase();
        let has_attach_note = hdr_texts.iter().any(|t| {
            t.plain_text
                .to_ascii_lowercase()
                .contains("attach additional")
        });
        for t in hdr_texts {
            let mut shifted = shift_top_in_tag(&t.raw_html, dy_hdr);
            if !marked_continued
                && has_attach_note
                && t.plain_text
                    .to_ascii_lowercase()
                    .contains("attach additional")
            {
                shifted = replace_inner_text(&shifted, "(continued)");
                marked_continued = true;
            } else if !marked_continued
                && !has_attach_note
                && !t.plain_text.is_empty()
                && (t.plain_text.to_ascii_lowercase().contains(&sched_lower)
                    || (bi == 0
                        && (t.plain_text.starts_with("1.A")
                            || t.plain_text.starts_with("Schedule"))))
            {
                let new_txt = format!("{} (continued)", t.plain_text);
                shifted = replace_inner_text(&shifted, &new_txt);
                marked_continued = true;
            }
            body_texts.push_str(&shifted);
        }

        // 2c. Band rows
        let band = &mb.band;
        let band_top_y = cursor_y + hdr_height;
        let trow = &band.rows[band.template_row];
        let template_y = band.row_y[band.template_row];
        let pitch = band.row_pitch_pt;
        let n_rows = rows.len();
        let band_bottom_y = band_top_y + (n_rows as f64) * pitch;

        // Horizontal boundaries (0..=n_rows)
        let tpl_bnd = band
            .boundaries
            .get(band.template_boundary)
            .cloned()
            .unwrap_or_default();
        let bnd_base_y = band
            .row_y
            .get(band.template_boundary)
            .copied()
            .unwrap_or(template_y);
        for k in 0..=n_rows {
            let dy = (band_top_y + (k as f64) * pitch) - bnd_base_y;
            for br in &tpl_bnd {
                push_band_rect(&mut svg_structural, br, dy);
            }
        }

        // Vertical spanning rules clamped to [band_top_y, band_bottom_y]
        for sp in &band.spanning {
            let sr = SvgRect {
                x: sp.rect.x,
                y: band_top_y - 0.24,
                w: sp.rect.w,
                h: (band_bottom_y - band_top_y) + 0.48,
                fill: sp.rect.fill.clone(),
                layer: "layer-structural".to_string(),
            };
            push_svg_rect(&mut svg_structural, &mut svg_decorative, &sr, 0.0);
        }

        // Locate template ordinal `.t` element (if any) on the source page
        let template_ord_text = band.ordinal_column.and_then(|[ox0, ox1]| {
            page.texts.iter().find(|t| {
                t.top >= template_y - 1.0
                    && t.top < template_y + pitch
                    && t.left >= ox0 - 1.0
                    && t.left <= ox1 + 1.0
            })
        });

        // Stamp each overflowing row k = 0..n_rows
        for (k, row) in rows.iter().enumerate() {
            let row_y = band_top_y + (k as f64) * pitch;
            let dy = row_y - template_y;

            // Internal row rules (comb ticks, white mask bands, dividers)
            for rr in &trow.rules {
                push_band_rect(&mut svg_structural, rr, dy);
            }

            // Continuing row number (ordinal)
            if let Some(ord_t) = template_ord_text {
                let shifted = shift_top_in_tag(&ord_t.raw_html, dy);
                let is_letter_ord = ord_t.plain_text.len() == 1
                    && ord_t.plain_text.as_bytes()[0].is_ascii_uppercase();
                let ord_label = if is_letter_ord && row.row_number >= 1 && row.row_number <= 26 {
                    ((b'A' + (row.row_number as u8) - 1) as char).to_string()
                } else {
                    row.row_number.to_string()
                };
                body_texts.push_str(&replace_inner_text(&shifted, &ord_label));
            }

            // Collect peso/cents and date-part synthetic ids per column in this row.
            let mut row_money_peso: BTreeMap<usize, (String, String)> = BTreeMap::new();
            let mut row_money_cents: BTreeMap<usize, String> = BTreeMap::new();
            let mut row_date_parts: BTreeMap<usize, DatePartIds> = BTreeMap::new();

            for cell in &trow.cells {
                let Some(src_cell) = page.cells.get(&cell.id) else {
                    continue;
                };
                let syn_id = format!("cont_s{sched_idx}_p{cont_page_idx}_b{bi}_r{k}_{}", cell.id);
                caption_aliases.insert(syn_id.clone(), cell.id.clone());
                let cloned = clone_cell_div(&src_cell.raw_html, &cell.id, &syn_id, dy);
                // The upload control the cloned cell prints (the row's own
                // key, not the template row's), for readers of the sheet.
                let upload_key = match mb.cell_map.get(&cell.id) {
                    Some(
                        CellJoinKind::Text { col_idx, .. }
                        | CellJoinKind::Peso { col_idx, .. }
                        | CellJoinKind::Money {
                            col_idx,
                            is_peso: true,
                            ..
                        }
                        | CellJoinKind::Date {
                            col_idx,
                            part: DatePart::Month,
                            ..
                        },
                    ) => row.keys.get(*col_idx),
                    _ => None,
                };
                body_cells.push_str(&match upload_key {
                    Some(key) => with_upload_key(&cloned, key),
                    None => cloned,
                });

                if let Some(join_kind) = mb.cell_map.get(&cell.id) {
                    match join_kind {
                        CellJoinKind::Text {
                            col_idx,
                            orig_writer_key,
                        } => {
                            let val = row.values.get(*col_idx).cloned().unwrap_or_default();
                            if !val.is_empty() {
                                let syn_key = format!("{orig_writer_key}__cont_{syn_id}");
                                // Whole-peso amount combs cloned onto a band
                                // continuation page (e.g. 1701A Part IV-A/IV-B
                                // p2c31/p2c32/p2c64/p2c65) receive raw modal
                                // amounts like "50000.00" and "0.00". Leave
                                // zero amounts blank (matching the official
                                // sheet and RULES 7 — avoiding stray "0 . 0 0"
                                // boxes) and route non-zero amounts through
                                // `extra_peso_joins` so they print as right-
                                // aligned whole pesos (or RULES 2 overflow).
                                if src_cell.raw_html.contains("data-comb-slots=")
                                    && is_whole_peso_comb_key(orig_writer_key)
                                {
                                    if !is_numeric_zero(&val) {
                                        syn_fields.insert(syn_key.clone(), val);
                                        extra_peso_joins.push((syn_key, syn_id));
                                    }
                                } else {
                                    // Preserve the original writer_key prefix so
                                    // frozen_html's amount/quantity right-alignment
                                    // rules match identically to the official page.
                                    syn_fields.insert(syn_key.clone(), val);
                                    extra_joins.entry(syn_key).or_default().push(syn_id);
                                }
                            }
                        }
                        CellJoinKind::Money {
                            col_idx,
                            orig_writer_key,
                            is_peso,
                        } => {
                            if *is_peso {
                                row_money_peso.insert(*col_idx, (syn_id, orig_writer_key.clone()));
                            } else {
                                row_money_cents.insert(*col_idx, syn_id);
                            }
                        }
                        CellJoinKind::Peso {
                            col_idx,
                            orig_writer_key,
                        } => {
                            let val = row.values.get(*col_idx).cloned().unwrap_or_default();
                            if !val.is_empty() {
                                let syn_key = format!("{orig_writer_key}__cont_{syn_id}");
                                syn_fields.insert(syn_key.clone(), val);
                                extra_peso_joins.push((syn_key, syn_id));
                            }
                        }
                        CellJoinKind::Date {
                            col_idx,
                            orig_writer_key,
                            part,
                        } => {
                            let parts = row_date_parts
                                .entry(*col_idx)
                                .or_insert_with(|| (orig_writer_key.clone(), None, None, None));
                            match part {
                                DatePart::Month => parts.1 = Some(syn_id.clone()),
                                DatePart::Day => parts.2 = Some(syn_id.clone()),
                                DatePart::Year => parts.3 = Some(syn_id.clone()),
                            }
                        }
                        CellJoinKind::Ordinal { orig_writer_key } => {
                            let syn_key = format!("{orig_writer_key}__cont_{syn_id}");
                            syn_fields.insert(syn_key.clone(), row.row_number.to_string());
                            extra_joins.entry(syn_key).or_default().push(syn_id);
                        }
                    }
                }
            }

            for (col_idx, (orig_writer_key, mm_id, dd_id, yyyy_id)) in row_date_parts {
                let val = row.values.get(col_idx).cloned().unwrap_or_default();
                if val.is_empty() {
                    continue;
                }
                let (Some(mm_id), Some(yyyy_id)) = (mm_id, yyyy_id) else {
                    continue;
                };
                let syn_key = format!("{orig_writer_key}__cont_{mm_id}");
                syn_fields.insert(syn_key.clone(), val);
                extra_date_joins.push((syn_key, mm_id, dd_id, yyyy_id));
            }

            for (col_idx, (peso_id, orig_writer_key)) in row_money_peso {
                let val = row.values.get(col_idx).cloned().unwrap_or_default();
                if !val.is_empty() {
                    let cents_id = row_money_cents.remove(&col_idx);
                    let syn_key = format!("{orig_writer_key}__cont_{peso_id}");
                    syn_fields.insert(syn_key.clone(), val);
                    extra_money_joins.push((syn_key, peso_id, cents_id));
                }
            }
        }

        cursor_y = band_bottom_y + SECTION_GAP_PT;
    }

    // --- 3. Clone official TOTAL row on the last continuation page ---
    if let Some((t_top, t_bottom, tj)) = total_strip {
        let total_y = cursor_y - SECTION_GAP_PT;
        let dy_tot = total_y - t_top;

        for r in &page.svg_rects {
            let r_bottom = r.y + r.h;
            if r_bottom <= t_top + 0.1 || r.y >= t_bottom - 0.1 {
                continue;
            }
            let cy0 = r.y.max(t_top);
            let cy1 = r_bottom.min(t_bottom);
            if cy1 - cy0 <= 0.05 {
                continue;
            }
            let clamped = SvgRect {
                x: r.x,
                y: cy0,
                w: r.w,
                h: cy1 - cy0,
                fill: r.fill.clone(),
                layer: r.layer.clone(),
            };
            push_svg_rect(&mut svg_structural, &mut svg_decorative, &clamped, dy_tot);
        }

        // Mark the rightmost label in the cloned TOTAL row with "(as on page N)"
        // so the reader sees it repeats the official-page grand total rather
        // than a second figure to add.
        let mut tot_texts: Vec<&TextNode> = page
            .texts
            .iter()
            .filter(|t| t.top >= t_top - 0.5 && t.top < t_bottom - 2.0)
            .collect();
        tot_texts.sort_by(|a, b| {
            a.left
                .partial_cmp(&b.left)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let last_idx = tot_texts.len().saturating_sub(1);
        for (idx, t) in tot_texts.into_iter().enumerate() {
            let mut shifted = shift_top_in_tag(&t.raw_html, dy_tot);
            if idx == last_idx && !t.plain_text.is_empty() {
                let marked = format!("{} (as on page {official_page_num})", t.plain_text);
                shifted = replace_inner_text(&shifted, &marked);
            }
            body_texts.push_str(&shifted);
        }

        // Clone the official total cell(s) and wire the schedule's grand total
        let syn_tot_id = format!("cont_tot_s{sched_idx}_{}", tj.html_id);
        caption_aliases.insert(syn_tot_id.clone(), tj.html_id.clone());
        if let Some(src_tot) = page.cells.get(&tj.html_id) {
            body_cells.push_str(&with_upload_key(
                &clone_cell_div(&src_tot.raw_html, &tj.html_id, &syn_tot_id, dy_tot),
                &tj.writer_key,
            ));
        }
        let syn_cents_id = tj.cents_html_id.as_ref().and_then(|cid| {
            let src = page.cells.get(cid)?;
            let sid = format!("cont_tot_s{sched_idx}_{cid}");
            body_cells.push_str(&clone_cell_div(&src.raw_html, cid, &sid, dy_tot));
            Some(sid)
        });

        if let Some(tot_val) = fields.get(&tj.writer_key).filter(|v| !v.trim().is_empty()) {
            let syn_key = format!("{}__cont_{syn_tot_id}", tj.writer_key);
            syn_fields.insert(syn_key.clone(), tot_val.clone());
            if tj.is_money {
                extra_money_joins.push((syn_key, syn_tot_id, syn_cents_id));
            } else if tj.is_peso {
                extra_peso_joins.push((syn_key, syn_tot_id));
            } else {
                extra_joins.entry(syn_key).or_default().push(syn_tot_id);
            }
        }
    }

    let page_id = format!("page-continuation-s{sched_idx}-p{cont_page_idx}");
    let mut raw_page = format!(
        "<div class=\"page page-continuation-band\" id=\"{page_id}\" style=\"width:{w}pt;height:{h}pt\">\
         <svg class=\"rl\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" style=\"width:{w}pt;height:{h}pt;z-index:1\">",
        w = fmt_pt(page.width_pt),
        h = fmt_pt(page.height_pt),
    );
    for img in &page.svg_images {
        raw_page.push_str(img);
    }
    raw_page.push_str("<g class=\"layer-decorative\">");
    raw_page.push_str(&svg_decorative);
    raw_page.push_str("</g><g class=\"layer-structural\">");
    raw_page.push_str(&svg_structural);
    raw_page.push_str("</g></svg>");
    raw_page.push_str(&body_texts);
    raw_page.push_str(&body_cells);
    raw_page.push_str("</div>");

    // Fill the entire continuation page (header combs + cloned row cells +
    // total cell) through frozen_html's own fill engine!
    frozen_html::fill_continuation_fragment(
        &raw_page,
        &syn_fields,
        slug,
        &extra_joins,
        &extra_money_joins,
        &extra_peso_joins,
        &extra_date_joins,
        &caption_aliases,
    )
}

// ---------------------------------------------------------------------------
// Geometry & HTML helpers
// ---------------------------------------------------------------------------

fn detect_page_header_bottom(page: &ParsedPage, first_band_y: f64) -> f64 {
    // Never let the page header cross into "Part IV / Part V / Schedule …"
    let first_section_y = page
        .texts
        .iter()
        .filter(|t| {
            let low = t.plain_text.trim().to_ascii_lowercase();
            t.top > 70.0
                && (low.starts_with("part ")
                    || low.starts_with("schedule ")
                    || low.starts_with("1.a"))
        })
        .map(|t| t.top)
        .fold(first_band_y, f64::min);

    // Check whether this official page carries a TIN / Taxpayer's Name strip
    // below the BIR Form No. / barcode box (e.g. 2000-OT, 1800). Forms without
    // a Page 2 TIN strip (e.g. 2552) go straight from the title box into Part IV.
    let has_header_tin = page.texts.iter().any(|t| {
        t.top < first_section_y
            && (t.plain_text.trim().eq_ignore_ascii_case("TIN")
                || t.plain_text.contains("Taxpayer")
                || t.plain_text.contains("Donor")
                || t.plain_text.contains("Decedent"))
    });

    if has_header_tin {
        let mut max_cell_bottom = 0.0_f64;
        for c in page.cells.values() {
            if c.top < first_section_y - 2.0 && c.raw_html.contains("data-cell-kind=\"field\"") {
                let b = c.top + c.height;
                if b > max_cell_bottom && b <= first_section_y + 1.0 {
                    max_cell_bottom = b;
                }
            }
        }
        if max_cell_bottom > 40.0 {
            return max_cell_bottom + 0.5;
        }
    }

    // Stop right below the BIR Form No. / barcode title box (strictly above `first_section_y`).
    let title_box_bottom = page
        .svg_rects
        .iter()
        .filter(|r| r.w > 400.0 && r.h >= 0.5 && r.y > 60.0 && r.y + r.h <= first_section_y + 0.5)
        .map(|r| r.y + r.h)
        .fold(0.0_f64, f64::max);
    if title_box_bottom > 60.0 {
        return title_box_bottom + 0.5;
    }

    (first_section_y - 1.5).max(85.0)
}

fn find_band_header_top(
    page: &ParsedPage,
    prev_bottom: f64,
    table_top_y: f64,
    band: &FormgenBand,
) -> f64 {
    // Clamp the search window to at most 48pt above `table_top_y` so a lower
    // schedule on the same page (e.g. Form 2552 Schedule 2) never reaches up
    // into an earlier schedule above it.
    let min_top = prev_bottom.max(table_top_y - 48.0);
    let heavy_rule_above = page
        .svg_rects
        .iter()
        .filter(|r| r.w > 400.0 && r.h >= 0.9 && r.y >= min_top - 2.0 && r.y <= table_top_y - 10.0)
        .map(|r| r.y)
        .fold(None::<f64>, |acc, y| {
            Some(acc.map_or(y, |prev| prev.max(y)))
        });

    if let Some(hr_y) = heavy_rule_above {
        let band_y0 = band.row_y.first().copied().unwrap_or(table_top_y);
        let mut top = hr_y;
        for sp in &band.spanning {
            let sp_top = band_y0 + sp.d_top;
            if sp_top >= min_top - 2.0 && sp_top < top && top - sp_top <= 36.0 {
                top = sp_top;
            }
        }
        // Include a schedule title sitting directly above `top` (within 16pt)
        for t in &page.texts {
            if t.top >= min_top && t.top < top && top - t.top <= 16.0 {
                let low = t.plain_text.to_ascii_lowercase();
                if low.starts_with("schedule ") || low.starts_with("part ") {
                    top = t.top - 1.5;
                }
            }
        }
        return top.max(min_top);
    }

    min_top
}

fn push_svg_rect(structural: &mut String, decorative: &mut String, r: &SvgRect, dy: f64) {
    let target = if r.layer.contains("decorative") {
        decorative
    } else {
        structural
    };
    target.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
        fmt_pt(r.x),
        fmt_pt(r.y + dy),
        fmt_pt(r.w),
        fmt_pt(r.h),
        r.fill,
    ));
}

fn push_band_rect(target: &mut String, r: &BandRect, dy: f64) {
    target.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
        fmt_pt(r.x),
        fmt_pt(r.y + dy),
        fmt_pt(r.w),
        fmt_pt(r.h),
        r.fill,
    ));
}

fn fmt_pt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `cell_div` with `data-upload-key="{key}"` on its opening tag: the upload
/// control a cloned continuation cell prints.
fn with_upload_key(cell_div: &str, key: &str) -> String {
    match cell_div.find('>') {
        Some(at) => format!(
            "{} data-upload-key=\"{}\"{}",
            &cell_div[..at],
            escape_html(key),
            &cell_div[at..]
        ),
        None => cell_div.to_string(),
    }
}

fn clone_cell_div(raw_html: &str, orig_id: &str, new_id: &str, dy: f64) -> String {
    let shifted = shift_top_in_tag(raw_html, dy);
    // Replace the outer div id="orig_id", inner input name="orig_id", and input id="orig_id-..."
    shifted
        .replace(&format!("id=\"{orig_id}\""), &format!("id=\"{new_id}\""))
        .replace(
            &format!("name=\"{orig_id}\""),
            &format!("name=\"{new_id}\""),
        )
        .replace(&format!("id=\"{orig_id}-"), &format!("id=\"{new_id}-"))
}

fn shift_top_in_tag(elem: &str, dy: f64) -> String {
    let Some(gt) = elem.find('>') else {
        return elem.to_string();
    };
    let open = &elem[..=gt];
    let rest = &elem[gt + 1..];
    let Some(top_pos) = open.find("top:") else {
        return elem.to_string();
    };
    let val_start = top_pos + 4;
    let Some(pt_rel) = open[val_start..].find("pt") else {
        return elem.to_string();
    };
    let val_str = open[val_start..val_start + pt_rel].trim();
    let Ok(old_top) = val_str.parse::<f64>() else {
        return elem.to_string();
    };
    let new_top = fmt_pt(old_top + dy);
    format!(
        "{}top:{}pt{}{}",
        &open[..top_pos],
        new_top,
        &open[val_start + pt_rel + 2..],
        rest
    )
}

fn replace_inner_text(elem: &str, new_text: &str) -> String {
    let Some(gt) = elem.find('>') else {
        return elem.to_string();
    };
    let Some(close) = elem.rfind("</") else {
        return elem.to_string();
    };
    format!(
        "{}{}{}",
        &elem[..=gt],
        escape_html(new_text),
        &elem[close..]
    )
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

fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let pos = tag.find(&needle)?;
    let start = pos + needle.len();
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

fn extract_style_pt(tag: &str, prop: &str) -> Option<f64> {
    let pos = tag.find(prop)?;
    let start = pos + prop.len();
    let rest = &tag[start..];
    let pt = rest.find("pt")?;
    rest[..pt].trim().parse::<f64>().ok()
}

fn strip_templates(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cur = 0;
    while let Some(rel) = html[cur..].find("<template") {
        out.push_str(&html[cur..cur + rel]);
        if let Some(end_rel) = html[cur + rel..].find("</template>") {
            cur = cur + rel + end_rel + "</template>".len();
        } else {
            return out;
        }
    }
    out.push_str(&html[cur..]);
    out
}

fn is_whole_peso_comb_key(key: &str) -> bool {
    key.starts_with("derived:p2_")
        || key.starts_with("derived:p3_")
        || key.starts_with("derived:tp_p")
        || key.starts_with("derived:sp_p")
        || key.starts_with("derived:amount_")
}

fn is_numeric_zero(val: &str) -> bool {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return true;
    }
    let bare: String = trimmed
        .chars()
        .filter(|c| *c != ',' && *c != '(' && *c != ')')
        .collect();
    bare.parse::<f64>()
        .map(|n| n.is_finite() && n.abs() < 0.005)
        .unwrap_or(false)
}

fn strip_html_tags(s: &str) -> String {
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
}

fn find_matching_div_end(html: &str, open_at: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut cur = open_at;
    while cur < html.len() {
        let next_open = html[cur..].find("<div");
        let next_close = html[cur..].find("</div>");
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                cur += o + 4;
            }
            (_, Some(c)) => {
                depth = depth.saturating_sub(1);
                cur += c + 6;
                if depth == 0 {
                    return Some(cur);
                }
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formgen_bands_for_band_forms_and_empty_for_fallback() {
        let b2000 =
            parse_formgen_bands(include_str!("../../../html-frozen/2000-ot-2018/index.html"));
        assert_eq!(b2000.len(), 2, "2000-OT has 2 bands (p2g0, p2g1)");
        assert_eq!(b2000[0].id, "p2g0");
        assert_eq!(b2000[1].id, "p2g1");

        let b1800 = parse_formgen_bands(include_str!("../../../html-frozen/1800-2018/index.html"));
        assert_eq!(b1800.len(), 4, "1800 has 4 bands (p2g0..p2g3)");

        let b2552 = parse_formgen_bands(include_str!("../../../html-frozen/2552-2018/index.html"));
        assert_eq!(b2552.len(), 7, "2552 has 7 bands (p2g0..p2g6)");

        let b1600wp =
            parse_formgen_bands(include_str!("../../../html-frozen/1600wp-2010/index.html"));
        assert!(
            !b1600wp.is_empty(),
            "1600WP now carries metadata-only formgen-bands"
        );

        let b2200m =
            parse_formgen_bands(include_str!("../../../html-frozen/2200m-2018/index.html"));
        assert!(
            b2200m.is_empty(),
            "2200M stays on plain fallback (growables: [])"
        );
    }

    #[test]
    fn band_continuation_preserves_official_pages_byte_for_byte_including_total() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("derived:pg2_tin".into(), "00012345300000".into());
        fields.insert("frm2000OT:txtTaxpayerName".into(), "JUANA SANTOS".into());
        fields.insert("frm2000OT:txtPg2TaxpayerName".into(), "JUANA SANTOS".into());
        for i in 0..15 {
            let n = i + 1;
            fields.insert(
                format!("frm2000OT:sched1A:txtOCT{i}"),
                format!("TCT-{n:03}"),
            );
            fields.insert(
                format!("frm2000OT:sched1A:txtFMVCol1{i}"),
                format!("{}.00", 1_000_000 * n),
            );
            fields.insert(
                format!("frm2000OT:sched1A:txtSubTotal{i}"),
                format!("{}.00", 1_000_000 * n),
            );
        }
        fields.insert(
            "frm2000OT:sched1A:txtFMVTotal".into(),
            "120000000.00".into(),
        );

        let doc = frozen_html::filled_document_with_report("2000-ot-2018", &fields, None).unwrap();
        assert_eq!(doc.overflow.schedules.len(), 1);
        assert!(doc.html.contains("page-continuation-band"));
        assert!(!doc.html.contains("class=\"page page-continuation\""));
        assert!(
            doc.html.contains("(as on page 2)"),
            "cloned TOTAL row should mark `(as on page 2)`"
        );

        // Official pages (everything before the continuation page) must retain
        // the official grand total AND match a 5-row-only fill with the same total.
        let cont_pos = doc
            .html
            .find("<div class=\"page page-continuation-band\"")
            .unwrap();
        let official_prefix = &doc.html[..cont_pos];
        assert!(
            official_prefix.contains("120000000.00"),
            "official page 2 must keep the grand total untouched"
        );
        let cont_slice = &doc.html[cont_pos..];
        assert!(
            cont_slice.contains("120000000.00"),
            "cloned TOTAL row on continuation page must show the same grand total"
        );
        assert!(
            cont_slice.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "cloned numeric cells must use frozen_html right-alignment"
        );
    }

    #[test]
    fn band_continuation_handles_2552_two_schedules_simultaneously() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("frm2552:txtTaxpayerName".into(), "JUANA SANTOS".into());
        fields.insert("frm2552:txtYearEnded".into(), "2026".into());
        for row_no in 1..=8 {
            fields.insert(
                format!("frm2552:txtPg2Pt5Sch1Seller{row_no}"),
                format!("S1-{row_no}"),
            );
            fields.insert(
                format!("frm2552:txtPg2Pt5Sch1TaxDue{row_no}"),
                format!("{}.00", row_no * 100),
            );
            fields.insert(
                format!("frm2552:txtPg2Pt5Sch2Seller{row_no}"),
                format!("S2-{row_no}"),
            );
            fields.insert(
                format!("frm2552:txtPg2Pt5Sch2TaxDue{row_no}"),
                format!("{}.00", row_no * 500),
            );
        }
        fields.insert("frm2552:txtPg2Pt5Sch1TaxDueTot".into(), "3600.00".into());
        fields.insert("frm2552:txtPg2Pt5Sch2TaxDueTot".into(), "18000.00".into());

        let doc = frozen_html::filled_document_with_report("2552-2018", &fields, None).unwrap();
        assert_eq!(doc.overflow.schedules.len(), 2);
        assert!(doc.html.contains("page-continuation-s0-p0"));
        assert!(doc.html.contains("page-continuation-s1-p0"));
        assert!(doc.html.contains("3600.00"));
        assert!(doc.html.contains("18000.00"));
        assert!(doc.html.contains("(as on page 2)"));
    }

    #[test]
    fn band_continuation_1600wp_continues_sequence_numbers_7_to_10() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("frm1600WP:txtTaxpayerName".into(), "JUANA SANTOS".into());
        for i in 1..=10 {
            if i <= 6 {
                fields.insert(format!("derived:sched_seq{i}"), i.to_string());
            }
            fields.insert(
                format!("frm1600WP:dtSched:txtFullname{i}"),
                format!("PAYEE {i}"),
            );
            fields.insert(
                format!("frm1600WP:dtSched:amount{i}"),
                format!("{}.00", i * 1000),
            );
            fields.insert(
                format!("frm1600WP:dtSched:taxWithheld{i}"),
                format!("{}.00", i * 100),
            );
        }
        let doc = frozen_html::filled_document_with_report("1600wp-2010", &fields, None).unwrap();
        assert_eq!(doc.overflow.schedules.len(), 1);
        let cont_pos = doc
            .html
            .find("<div class=\"page page-continuation-band\"")
            .unwrap();
        let cont = &doc.html[cont_pos..];
        for n in 7..=10 {
            assert!(
                cont.contains(&format!("value=\"{n}\"")),
                "1600WP continuation must populate sequence number {n}"
            );
            assert!(cont.contains(&format!("PAYEE {n}")));
        }
    }

    #[test]
    fn band_continuation_1801_does_not_duplicate_continued_on_continuation_heading() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:tin".into(), "00012345300000".into());
        fields.insert("frm1801v2018:txtTaxpayerName".into(), "JUANA SANTOS".into());
        for i in 1..=4 {
            fields.insert(format!("frm1801v2018:sched1Oct_{i}"), format!("TCT-{i:03}"));
            fields.insert(format!("frm1801v2018:sched1Class_{i}'"), "RES".into());
            fields.insert(
                format!("frm1801v2018:sched1Fmv_{i}"),
                format!("{}.00", i * 100_000),
            );
        }
        let doc = frozen_html::filled_document_with_report("1801-2018", &fields, None).unwrap();
        assert_eq!(doc.overflow.schedules.len(), 1);
        let cont_pos = doc
            .html
            .find("<div class=\"page page-continuation-band\"")
            .unwrap();
        let cont = &doc.html[cont_pos..];
        assert!(cont.contains("TCT-003"));
        assert!(cont.contains("TCT-004"));
        assert!(
            !cont.contains("continuation) (continued)")
                && !cont.contains("Continuation) (continued)"),
            "1801 Schedule 1 continuation heading must not duplicate (continued)"
        );
    }

    #[test]
    fn popup_overflow_works_for_1707_1701a_and_other_popup_forms() {
        // 1707: Schedule 2 (falls back to plain table because p2g0 omits col 1) + Schedule 3 (cloned band p2g1)
        let mut f1707 = BTreeMap::new();
        f1707.insert("derived:tin".into(), "00012345300000".into());
        f1707.insert("frm1707:txtSellerName".into(), "JUANA SANTOS".into());
        for k in 1..=3 {
            f1707.insert(format!("frm1707:txtPg2Pt4S2_{k}Col1"), format!("CORP {k}"));
            f1707.insert(format!("frm1707:txtPg2Pt4S2_{k}Col2"), format!("{}00", k));
            f1707.insert(format!("frm1707:txtPg2Pt4S2_{k}Col3"), format!("CERT-{k}"));
            f1707.insert(
                format!("frm1707:txtPg2Pt4S2_{k}Col4"),
                format!("{k}0000.00"),
            );
            f1707.insert(
                format!("frm1707:txtPg2Pt4S3_{k}Col1"),
                format!("EXPENSE {k}"),
            );
            f1707.insert(format!("frm1707:txtPg2Pt4S3_{k}Col2"), format!("{k}500.25"));
        }
        let doc1707 = frozen_html::filled_document_with_report("1707-2021", &f1707, None).unwrap();
        assert_eq!(doc1707.overflow.schedules.len(), 2);
        assert!(doc1707.html.contains("CORP 1") && doc1707.html.contains("CORP 3"));
        assert!(
            doc1707.html.contains("page-continuation-band"),
            "1707 Schedule 3 uses cloned band p2g1"
        );
        assert!(doc1707.html.contains("EXPENSE 1") || doc1707.html.contains("value=\"E\""));

        // 1701A: Part IV-A & Part IV-B popup rows -> cloned bands p2g0 & p2g1
        let mut f1701a = BTreeMap::new();
        f1701a.insert("derived:tin".into(), "00012345300000".into());
        f1701a.insert("frm1701A:txtTaxpayerName".into(), "JUANA SANTOS".into());
        for n in 1..=3 {
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVADesc_{n}C1"),
                format!("OTHER INC A {n}"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVA_{n}C2"),
                format!("{n}0000.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVA_{n}C3"),
                format!("{n}500.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVBDesc_{n}C1"),
                format!("OTHER INC B {n}"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C2"),
                format!("{n}2000.00"),
            );
            f1701a.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C3"),
                format!("{n}300.00"),
            );
        }
        let doc1701a =
            frozen_html::filled_document_with_report("1701a-2018", &f1701a, None).unwrap();
        assert_eq!(doc1701a.overflow.schedules.len(), 2);
        assert!(
            doc1701a.html.contains("page-continuation-band"),
            "1701A uses cloned bands"
        );
        assert!(!doc1701a.html.contains("class=\"page page-continuation\""));
        assert!(doc1701a.html.contains("OTHER INC A 1") && doc1701a.html.contains("OTHER INC B 3"));

        // 1707A, 1700, 1702EX, 1702MX, 1702RT popup detection
        let mut f1707a = BTreeMap::new();
        f1707a.insert(
            "frm1707Av2021:txtS1C2NameOfCorporateStock_1".into(),
            "ALPHA CORP".into(),
        );
        f1707a.insert(
            "frm1707Av2021:txtS1C3SellingPrice_1".into(),
            "50000.00".into(),
        );
        assert_eq!(
            continuation::detect_overflow("1707a-2021", &f1707a)
                .unwrap()
                .schedules
                .len(),
            1
        );

        let mut f1700 = BTreeMap::new();
        f1700.insert(
            "frm1700:txtPg2I4_1PartVIEmployerName1".into(),
            "ACME INC".into(),
        );
        f1700.insert("frm1700:txtPg2ISched1c_4_1REG".into(), "120000.00".into());
        assert_eq!(
            continuation::detect_overflow("1700-2018", &f1700)
                .unwrap()
                .schedules
                .len(),
            1
        );

        let mut f1702ex = BTreeMap::new();
        f1702ex.insert("frm1702EX:txtPg3Pt6S2I4_1Col1".into(), "SPECIAL DED".into());
        f1702ex.insert("frm1702EX:txtPg3Pt6S2I4_1Col3".into(), "9000.00".into());
        assert_eq!(
            continuation::detect_overflow("1702ex-2018", &f1702ex)
                .unwrap()
                .schedules
                .len(),
            1
        );

        let mut f1702mx = BTreeMap::new();
        f1702mx.insert("frm1702MX:txtPg6Sc6_1Col1".into(), "MX DED".into());
        f1702mx.insert("frm1702MX:txtPg6Sc6_1Col3".into(), "8000.00".into());
        assert_eq!(
            continuation::detect_overflow("1702mx-2018c", &f1702mx)
                .unwrap()
                .schedules
                .len(),
            1
        );

        let mut f1702rt = BTreeMap::new();
        f1702rt.insert("frm1702RT:txtPg3Sc2I4.1C1".into(), "RT DED".into());
        f1702rt.insert("frm1702RT:txtPg3Sc2I4.1C3".into(), "7000.00".into());
        assert_eq!(
            continuation::detect_overflow("1702rt-2018c", &f1702rt)
                .unwrap()
                .schedules
                .len(),
            1
        );
    }

    #[test]
    fn cloned_band_cells_receive_white_knockout_band_on_overflow() {
        // 1707 Schedule 3 uses comb cells (p2c89 peso + p2c90 cents) and p2c88 particulars.
        // Feed an over-wide amount (15 digits) on a continuation row and verify the cloned
        // cell gets `data-overflow-of="cont-..."` and the document carries the white-band CSS.
        let mut f1707 = BTreeMap::new();
        f1707.insert("derived:tin".into(), "00012345300000".into());
        f1707.insert("frm1707:txtSellerName".into(), "JUANA SANTOS".into());
        f1707.insert(
            "frm1707:txtPg2Pt4S3_1Col1".into(),
            "EXTREMELY LONG DOCUMENTARY STAMP TAX AND BROKERAGE EXPENSE DESCRIPTION FOR WHITE BAND TEST".into(),
        );
        f1707.insert(
            "frm1707:txtPg2Pt4S3_1Col2".into(),
            "999999999999999.99".into(),
        );

        let doc = frozen_html::filled_document_with_report("1707-2021", &f1707, None).unwrap();
        let cont_pos = doc
            .html
            .find("<div class=\"page page-continuation-band\"")
            .unwrap();
        let cont = &doc.html[cont_pos..];
        assert!(
            cont.contains("data-overflow-of=\"cont_"),
            "cloned comb/text cell on continuation page must get data-overflow-of white band"
        );
        assert!(
            cont.contains("data-overflow-warning=\"amount-comb\""),
            "over-wide cloned money comb should carry amount-comb overflow warning"
        );
        let cell_warnings = doc.cell_overflow_warnings();
        assert!(
            cell_warnings
                .iter()
                .any(|w| w.cell.starts_with("cont_") && w.kind == "amount-comb"),
            "overflow_warnings() must report the cloned continuation cell: {cell_warnings:?}"
        );
        let summary = doc
            .warning_summary()
            .expect("warning_summary() must be Some when rows and amounts overflow");
        assert!(
            summary.contains("additional sheet") && summary.contains("fitted line"),
            "unified warning_summary() must include both schedule rows and fitted-line amount: {summary}"
        );
    }
}
