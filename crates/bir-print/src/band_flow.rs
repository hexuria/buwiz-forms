//! In-page band flow engine for frozen BIR forms (Rules 5 & 6 in
//! `form-review/RULES.md`, metadata spec in `form-review/BAND-FORMAT.md`).
//!
//! When a growable table has more rows than the paper form prints (or a Rule 6
//! "specify" field overflows its inline box), extra rows are drawn directly
//! beneath the last printed row in the table's own row design and every band
//! below slides down by `dy`.
//!
//! - Empty trailing space is consumed first, then any `elastic_box` below the
//!   growth shrinks down to its declared `min_height_pt`.
//! - When content exceeds `content_bottom_y` (which reserves at least 18pt for
//!   the shared page footer), the packer breaks cleanly between bands — never
//!   through a row, never between a table's head and its first row, and never
//!   between its last row and its total.
//! - The single true `total_row` prints once, immediately after the last row
//!   wherever it lands. A page that breaks mid-table omits `total_row` and
//!   prints `"Continued on the next page"` below its last row.
//! - Spawned continuation pages open with the page's own `page_header` (with
//!   TIN & taxpayer name filled and page label `"Page <p> (continued)"`), plus
//!   a repeated `table_head` (marked `"(continued)"` once) on a mid-table break.
//! - Pages without `formgen-page-layout` AND without usable `formgen-bands`
//!   fall back to [`crate::band_continuation`] / [`crate::continuation`].
//! - Pages with no overflow and no Rule 6 insertions are returned byte-for-byte
//!   unchanged.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::band_continuation::{
    clone_cell_div, escape_html, extract_attr, extract_style_pt, find_matching_div_end, fmt_pt,
    match_schedule_to_bands, parse_formgen_bands, push_band_rect, replace_inner_text,
    shift_top_in_tag, strip_html_tags, strip_templates, BandRect, CellJoinKind, FormgenBand,
    MatchedBand,
};
use crate::continuation::{OverflowReport, OverflowSchedule};
use crate::frozen_html;

/// Minimum space (in native page pt) reserved at the bottom of every flowed
/// page for the shared page footer (Rule 7).
pub const FOOTER_RESERVE_PT: f64 = 18.0;

/// Vertical space reserved below the last row on a mid-table breaking page
/// for the closing rule + `"Continued on the next page"` cue.
const BREAK_CUE_HEIGHT_PT: f64 = 14.0;

// ---------------------------------------------------------------------------
// Metadata schema (`form-review/BAND-FORMAT.md` v1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct PageLayoutDoc {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub pages: Vec<PageLayout>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageLayout {
    pub page: usize,
    pub width_pt: f64,
    pub height_pt: f64,
    pub content_bottom_y: f64,
    pub table_x0: f64,
    pub table_x1: f64,
    #[serde(default)]
    pub bands: Vec<LayoutBand>,
    #[serde(default)]
    pub rule6_fields: Vec<Rule6Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BandKind {
    PageHeader,
    SectionTitle,
    TableHead,
    RowTemplate,
    TotalRow,
    PlainBlock,
    ElasticBox,
    TrailingSpace,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutBand {
    pub id: String,
    pub kind: BandKind,
    pub y0: f64,
    pub y1: f64,
    #[serde(default)]
    pub keep_with_next: bool,
    #[serde(default)]
    pub keep_with_prev: bool,
    #[serde(default)]
    pub repeat_on_break: bool,
    #[serde(default)]
    pub min_height_pt: Option<f64>,
    #[serde(default)]
    pub page_label_text_id: Option<String>,
    #[serde(default)]
    pub growable_id: Option<String>,
    #[serde(default)]
    pub table_group: Option<String>,
    #[serde(default)]
    pub svg_ids: Vec<String>,
    #[serde(default)]
    pub svg_indices: Vec<usize>,
    #[serde(default)]
    pub text_ids: Vec<String>,
    #[serde(default)]
    pub cell_ids: Vec<String>,
    #[serde(default)]
    pub spanning_svg_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule6Field {
    pub writer_key: String,
    pub cell_id: String,
    pub band_id: String,
    pub insert_after_y: f64,
    pub x0: f64,
    pub x1: f64,
}

/// Parse `<script type="application/json" id="formgen-page-layout">...</script>`
/// from a frozen sheet's HTML. Returns `None` if absent.
pub fn parse_page_layout(html: &str) -> Option<PageLayoutDoc> {
    let id_pos = html.find("id=\"formgen-page-layout\"")?;
    let gt_rel = html[id_pos..].find('>')?;
    let content_start = id_pos + gt_rel + 1;
    let end_rel = html[content_start..].find("</script>")?;
    let json_str = html[content_start..content_start + end_rel].trim();
    if json_str.is_empty() {
        return None;
    }
    serde_json::from_str::<PageLayoutDoc>(json_str).ok()
}

// ---------------------------------------------------------------------------
// Parsed page DOM (from `official_filled`, preserving all 3 layers + overlays)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FlowSvgNode {
    index: usize,
    layer: String,
    rule_id: Option<String>,
    /// Present for `<rect .../>` elements; `None` for `<path>`, `<line>`, etc.
    rect: Option<(f64, f64, f64, f64, String)>, // (x, y, w, h, fill)
    raw_tag: String,
}

impl FlowSvgNode {
    fn y_top(&self) -> f64 {
        if let Some((_, y, _, _, _)) = self.rect {
            y
        } else {
            extract_attr(&self.raw_tag, "y")
                .or_else(|| extract_attr(&self.raw_tag, "y1"))
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
        }
    }

    fn y_bottom(&self) -> f64 {
        if let Some((_, y, _, h, _)) = self.rect {
            y + h
        } else {
            extract_attr(&self.raw_tag, "y2")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or_else(|| self.y_top())
        }
    }

    fn is_vertical_rule(&self) -> bool {
        if let Some((_, _, w, h, _)) = self.rect {
            w <= 3.0 && h > 12.0
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
struct FlowTextNode {
    id: Option<String>,
    left: f64,
    top: f64,
    raw_html: String,
    plain_text: String,
}

#[derive(Debug, Clone)]
struct FlowCellNode {
    id: String,
    top: f64,
    height: f64,
    raw_html: String,
    /// Fill-time sibling overlay elements keyed to this cell via
    /// `data-overflow-of="<id>"` (or `data-cell-id="<id>"`).
    overlays: Vec<String>,
}

#[derive(Debug, Clone)]
struct FlowPageDom {
    page_num: usize,
    /// Byte range `[div_start, div_end)` inside the full document HTML.
    doc_span: (usize, usize),
    width_pt: f64,
    height_pt: f64,
    svg_open_tag: String,
    svg_images: Vec<String>,
    svg_nodes: Vec<FlowSvgNode>,
    texts: Vec<FlowTextNode>,
    cells: Vec<FlowCellNode>,
    #[allow(dead_code)]
    orphan_overlays: Vec<(f64, String)>,
}

fn parse_flow_page_dom(doc_html: &str, page_num: usize) -> Result<FlowPageDom, String> {
    let marker = format!("class=\"page page-{page_num}\"");
    let start = doc_html
        .find(&marker)
        .ok_or_else(|| format!("page {page_num} not found in filled HTML"))?;
    let div_start = doc_html[..start]
        .rfind("<div")
        .ok_or_else(|| format!("malformed page {page_num} div"))?;
    let div_end = find_matching_div_end(doc_html, div_start)
        .ok_or_else(|| format!("unclosed page {page_num} div"))?;
    let page_html = &doc_html[div_start..div_end];

    let tag_end = page_html.find('>').unwrap_or(0);
    let open_tag = &page_html[..tag_end];
    let width_pt = extract_style_pt(open_tag, "width:").unwrap_or(612.0);
    let height_pt = extract_style_pt(open_tag, "height:").unwrap_or(936.0);

    // 1. Parse `<svg class="rl" ...>...</svg>`
    let mut svg_open_tag = format!(
        "<svg class=\"rl\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"width:{}pt;height:{}pt;z-index:1\">",
        fmt_pt(width_pt),
        fmt_pt(height_pt),
        fmt_pt(width_pt),
        fmt_pt(height_pt),
    );
    let mut svg_images = Vec::new();
    let mut svg_nodes = Vec::new();

    if let Some(svg_start) = page_html.find("<svg ") {
        if let Some(svg_gt) = page_html[svg_start..].find('>') {
            svg_open_tag = page_html[svg_start..svg_start + svg_gt + 1].to_string();
        }
        if let Some(svg_end_rel) = page_html[svg_start..].find("</svg>") {
            let svg_inner_start =
                svg_start + page_html[svg_start..].find('>').map(|i| i + 1).unwrap_or(0);
            let svg = &page_html[svg_inner_start..svg_start + svg_end_rel];

            let mut cur_layer = "layer-structural".to_string();
            let mut scan = 0;
            while scan < svg.len() {
                let Some(lt_rel) = svg[scan..].find('<') else {
                    break;
                };
                let a = scan + lt_rel;
                let rest = &svg[a..];
                if rest.starts_with("<g ") || rest.starts_with("<g>") {
                    let end_g = rest.find('>').map(|e| a + e + 1).unwrap_or(svg.len());
                    if let Some(cls) = extract_attr(&svg[a..end_g], "class") {
                        cur_layer = cls.to_string();
                    }
                    scan = end_g;
                } else if rest.starts_with("</g>") {
                    scan = a + 4;
                } else if rest.starts_with("<image ") {
                    let end_img = find_svg_elem_end(svg, a, "image");
                    svg_images.push(svg[a..end_img].to_string());
                    scan = end_img;
                } else if rest.starts_with("<rect ") {
                    let end_r = find_svg_elem_end(svg, a, "rect");
                    let tag = &svg[a..end_r];
                    let rule_id = extract_attr(tag, "data-rule-id")
                        .or_else(|| extract_attr(tag, "id"))
                        .map(|s| s.to_string());
                    let rect = match (
                        extract_attr(tag, "x").and_then(|v| v.parse::<f64>().ok()),
                        extract_attr(tag, "y").and_then(|v| v.parse::<f64>().ok()),
                        extract_attr(tag, "width").and_then(|v| v.parse::<f64>().ok()),
                        extract_attr(tag, "height").and_then(|v| v.parse::<f64>().ok()),
                    ) {
                        (Some(x), Some(y), Some(w), Some(h)) => {
                            let fill = extract_attr(tag, "fill").unwrap_or("#000000").to_string();
                            Some((x, y, w, h, fill))
                        }
                        _ => None,
                    };
                    let index = svg_nodes.len();
                    svg_nodes.push(FlowSvgNode {
                        index,
                        layer: cur_layer.clone(),
                        rule_id,
                        rect,
                        raw_tag: tag.to_string(),
                    });
                    scan = end_r;
                } else if let Some(tag_name) = drawable_svg_tag_name(rest) {
                    let end_el = find_svg_elem_end(svg, a, tag_name);
                    let tag = &svg[a..end_el];
                    let rule_id = extract_attr(tag, "data-rule-id")
                        .or_else(|| extract_attr(tag, "id"))
                        .map(|s| s.to_string());
                    let index = svg_nodes.len();
                    svg_nodes.push(FlowSvgNode {
                        index,
                        layer: cur_layer.clone(),
                        rule_id,
                        rect: None,
                        raw_tag: tag.to_string(),
                    });
                    scan = end_el;
                } else {
                    scan = a + 1;
                }
            }
        }
    }

    // 2. Parse `<div class="layer-text">...</div>`
    let html_no_tpl = strip_templates(page_html);
    let mut texts = Vec::new();
    if let Some(lt_pos) = html_no_tpl.find("<div class=\"layer-text\">") {
        let lt_end = find_matching_div_end(&html_no_tpl, lt_pos).unwrap_or(html_no_tpl.len());
        let lt_inner = &html_no_tpl[lt_pos + "<div class=\"layer-text\">".len()..lt_end - 6];
        let mut pos = 0;
        while pos < lt_inner.len() {
            let Some(rel) = lt_inner[pos..].find('<') else {
                break;
            };
            let tag_open = pos + rel;
            if lt_inner[tag_open..].starts_with("</") {
                pos = tag_open + 2;
                continue;
            }
            let Some(gt_rel) = lt_inner[tag_open..].find('>') else {
                break;
            };
            let open_slice = &lt_inner[tag_open..tag_open + gt_rel + 1];
            let is_div = open_slice.starts_with("<div");
            let elem_end = if is_div {
                find_matching_div_end(lt_inner, tag_open).unwrap_or(tag_open + gt_rel + 1)
            } else if open_slice.starts_with("<span") {
                lt_inner[tag_open + gt_rel + 1..]
                    .find("</span>")
                    .map(|e| tag_open + gt_rel + 1 + e + 7)
                    .unwrap_or(tag_open + gt_rel + 1)
            } else {
                pos = tag_open + gt_rel + 1;
                continue;
            };
            let raw = &lt_inner[tag_open..elem_end];
            let id = extract_attr(open_slice, "id").map(|s| s.to_string());
            let left = extract_style_pt(open_slice, "left:").unwrap_or(0.0);
            let top = extract_style_pt(open_slice, "top:").unwrap_or(0.0);
            let inner = &raw[gt_rel + 1..raw.rfind("</").unwrap_or(raw.len())];
            texts.push(FlowTextNode {
                id,
                left,
                top,
                raw_html: raw.to_string(),
                plain_text: strip_html_tags(inner).trim().to_string(),
            });
            pos = elem_end;
        }
    }

    // 3. Parse `<div class="layer-cells">...</div>` (cells + cell-keyed overlays)
    let mut cells: Vec<FlowCellNode> = Vec::new();
    let mut cell_idx_by_id: BTreeMap<String, usize> = BTreeMap::new();
    let mut pending_overlays: Vec<(Option<String>, f64, String)> = Vec::new();

    if let Some(lc_pos) = html_no_tpl.find("<div class=\"layer-cells\">") {
        let lc_end = find_matching_div_end(&html_no_tpl, lc_pos).unwrap_or(html_no_tpl.len());
        let lc_inner = &html_no_tpl[lc_pos + "<div class=\"layer-cells\">".len()..lc_end - 6];
        let mut pos = 0;
        while pos < lc_inner.len() {
            let Some(rel) = lc_inner[pos..].find('<') else {
                break;
            };
            let tag_open = pos + rel;
            if lc_inner[tag_open..].starts_with("</") {
                pos = tag_open + 2;
                continue;
            }
            let Some(gt_rel) = lc_inner[tag_open..].find('>') else {
                break;
            };
            let open_slice = &lc_inner[tag_open..tag_open + gt_rel + 1];
            let elem_end = if open_slice.starts_with("<div") {
                find_matching_div_end(lc_inner, tag_open).unwrap_or(tag_open + gt_rel + 1)
            } else if open_slice.starts_with("<input") || open_slice.ends_with("/>") {
                tag_open + gt_rel + 1
            } else if open_slice.starts_with("<span") {
                lc_inner[tag_open + gt_rel + 1..]
                    .find("</span>")
                    .map(|e| tag_open + gt_rel + 1 + e + 7)
                    .unwrap_or(tag_open + gt_rel + 1)
            } else {
                pos = tag_open + gt_rel + 1;
                continue;
            };
            let raw = &lc_inner[tag_open..elem_end];

            // Check if this is a fill-time overlay keyed to a cell
            if let Some(owner) = extract_attr(open_slice, "data-overflow-of")
                .or_else(|| extract_attr(open_slice, "data-cell-id"))
            {
                let top = extract_style_pt(open_slice, "top:").unwrap_or(0.0);
                pending_overlays.push((Some(owner.to_string()), top, raw.to_string()));
                pos = elem_end;
                continue;
            }

            if let Some(id) = extract_attr(open_slice, "id") {
                let top = extract_style_pt(open_slice, "top:").unwrap_or(0.0);
                let height = extract_style_pt(open_slice, "height:").unwrap_or(0.0);
                let idx = cells.len();
                cell_idx_by_id.insert(id.to_string(), idx);
                cells.push(FlowCellNode {
                    id: id.to_string(),
                    top,
                    height,
                    raw_html: raw.to_string(),
                    overlays: Vec::new(),
                });
            } else {
                let top = extract_style_pt(open_slice, "top:").unwrap_or(0.0);
                pending_overlays.push((None, top, raw.to_string()));
            }
            pos = elem_end;
        }
    }

    let mut orphan_overlays = Vec::new();
    for (owner_opt, top, raw) in pending_overlays {
        if let Some(owner) = owner_opt {
            if let Some(&idx) = cell_idx_by_id.get(&owner) {
                cells[idx].overlays.push(raw);
                continue;
            }
        }
        orphan_overlays.push((top, raw));
    }

    Ok(FlowPageDom {
        page_num,
        doc_span: (div_start, div_end),
        width_pt,
        height_pt,
        svg_open_tag,
        svg_images,
        svg_nodes,
        texts,
        cells,
        orphan_overlays,
    })
}

fn drawable_svg_tag_name(rest: &str) -> Option<&'static str> {
    for name in ["path", "line", "polygon", "polyline", "circle", "ellipse"] {
        if rest.starts_with(&format!("<{name} ")) || rest.starts_with(&format!("<{name}>")) {
            return Some(name);
        }
    }
    None
}

fn find_svg_elem_end(svg: &str, start: usize, tag_name: &str) -> usize {
    let rest = &svg[start..];
    if let Some(gt) = rest.find('>') {
        if rest[..gt].ends_with('/') {
            return start + gt + 1;
        }
        let close = format!("</{tag_name}>");
        if let Some(c) = rest[gt + 1..].find(&close) {
            return start + gt + 1 + c + close.len();
        }
        return start + gt + 1;
    }
    svg.len()
}

// ---------------------------------------------------------------------------
// Automatic PageLayout derivation when `formgen-page-layout` is not yet present
// ---------------------------------------------------------------------------

/// Derive a complete, 100%-partitioning [`PageLayout`] for `dom` from its
/// existing `formgen-bands` entries when `<script id="formgen-page-layout">`
/// is not yet embedded in the frozen HTML.
fn derive_page_layout(dom: &FlowPageDom, page_bands: &[&FormgenBand]) -> Option<PageLayout> {
    if page_bands.is_empty() {
        return None;
    }

    let mut sorted_bands: Vec<&FormgenBand> = page_bands.to_vec();
    sorted_bands.sort_by(|a, b| {
        let ya = a.row_y.first().copied().unwrap_or(0.0);
        let yb = b.row_y.first().copied().unwrap_or(0.0);
        ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let first_band_y = sorted_bands[0].row_y.first().copied().unwrap_or(140.0);
    let header_bottom = detect_header_bottom_from_dom(dom, first_band_y);
    let content_bottom_y = (dom.height_pt - FOOTER_RESERVE_PT).max(header_bottom + 40.0);

    // Determine main outer frame X bounds from full-width horizontal rules.
    let (mut table_x0, mut table_x1) = (17.64_f64, dom.width_pt - 17.64);
    for node in &dom.svg_nodes {
        if let Some((x, _, w, h, _)) = node.rect {
            if w > dom.width_pt * 0.75 && h < 5.0 {
                table_x0 = x;
                table_x1 = x + w;
                break;
            }
        }
    }

    // Locate page label text ID (e.g. "Page 2") inside the page header.
    let page_label_text_id = find_page_label_text_id(dom, header_bottom);

    // Build contiguous vertical band slices `[y0, y1]`.
    struct SliceSpec {
        kind: BandKind,
        y0: f64,
        y1: f64,
        keep_with_next: bool,
        keep_with_prev: bool,
        repeat_on_break: bool,
        page_label_text_id: Option<String>,
        growable_id: Option<String>,
        table_group: Option<String>,
    }

    let mut slices: Vec<SliceSpec> = Vec::new();
    slices.push(SliceSpec {
        kind: BandKind::PageHeader,
        y0: 0.0,
        y1: header_bottom,
        keep_with_next: false,
        keep_with_prev: false,
        repeat_on_break: true,
        page_label_text_id,
        growable_id: None,
        table_group: None,
    });

    let mut cursor_y = header_bottom;

    for (bi, fb) in sorted_bands.iter().enumerate() {
        let Some(&row_y0) = fb.row_y.first() else {
            continue;
        };
        let Some(&row_y_end) = fb.row_y.last() else {
            continue;
        };
        if row_y0 < cursor_y - 1.0 {
            continue;
        }

        let group_id = format!("p{}g{}", dom.page_num, bi);

        // Find table head start above `row_y0` (back to the Schedule/Part
        // heading banner or column captions above `row_y0`).
        let head_y0 = find_table_head_top(dom, row_y0, cursor_y);

        // Plain block between `cursor_y` and `head_y0` (if non-empty).
        if head_y0 > cursor_y + 2.0 {
            slices.push(SliceSpec {
                kind: BandKind::PlainBlock,
                y0: cursor_y,
                y1: head_y0,
                keep_with_next: false,
                keep_with_prev: false,
                repeat_on_break: false,
                page_label_text_id: None,
                growable_id: None,
                table_group: None,
            });
        }

        // Table head (`head_y0 .. row_y0`)
        if row_y0 > head_y0 + 1.0 {
            slices.push(SliceSpec {
                kind: BandKind::TableHead,
                y0: head_y0,
                y1: row_y0,
                keep_with_next: true,
                keep_with_prev: false,
                repeat_on_break: true,
                page_label_text_id: None,
                growable_id: None,
                table_group: Some(group_id.clone()),
            });
        }

        // Row template (`row_y0 .. row_y_end`)
        slices.push(SliceSpec {
            kind: BandKind::RowTemplate,
            y0: row_y0,
            y1: row_y_end,
            keep_with_next: false,
            keep_with_prev: false,
            repeat_on_break: false,
            page_label_text_id: None,
            growable_id: Some(fb.id.clone()),
            table_group: Some(group_id.clone()),
        });

        // Detect an immediately following `total_row` (within ~30pt below `row_y_end`).
        let next_band_y = sorted_bands
            .get(bi + 1)
            .and_then(|nb| nb.row_y.first().copied())
            .unwrap_or(content_bottom_y);
        if let Some(total_y1) = find_total_row_bottom(dom, row_y_end, next_band_y) {
            slices.push(SliceSpec {
                kind: BandKind::TotalRow,
                y0: row_y_end,
                y1: total_y1,
                keep_with_next: false,
                keep_with_prev: true,
                repeat_on_break: false,
                page_label_text_id: None,
                growable_id: None,
                table_group: Some(group_id),
            });
            cursor_y = total_y1;
        } else {
            cursor_y = row_y_end;
        }
    }

    // Remaining content below the last band up to the bottom of the last element.
    let last_content_y = find_last_content_bottom(dom, cursor_y);
    if last_content_y > cursor_y + 2.0 {
        slices.push(SliceSpec {
            kind: BandKind::PlainBlock,
            y0: cursor_y,
            y1: last_content_y,
            keep_with_next: false,
            keep_with_prev: false,
            repeat_on_break: false,
            page_label_text_id: None,
            growable_id: None,
            table_group: None,
        });
        cursor_y = last_content_y;
    }

    if content_bottom_y > cursor_y + 1.0 {
        slices.push(SliceSpec {
            kind: BandKind::TrailingSpace,
            y0: cursor_y,
            y1: content_bottom_y,
            keep_with_next: false,
            keep_with_prev: false,
            repeat_on_break: false,
            page_label_text_id: None,
            growable_id: None,
            table_group: None,
        });
    }

    // Build `LayoutBand`s and partition 100% of `dom`'s SVG nodes, texts, and cells.
    let mut bands: Vec<LayoutBand> = slices
        .into_iter()
        .enumerate()
        .map(|(i, s)| LayoutBand {
            id: format!("p{}b{i}", dom.page_num),
            kind: s.kind,
            y0: s.y0,
            y1: s.y1,
            keep_with_next: s.keep_with_next,
            keep_with_prev: s.keep_with_prev,
            repeat_on_break: s.repeat_on_break,
            min_height_pt: None,
            page_label_text_id: s.page_label_text_id,
            growable_id: s.growable_id,
            table_group: s.table_group,
            svg_ids: Vec::new(),
            svg_indices: Vec::new(),
            text_ids: Vec::new(),
            cell_ids: Vec::new(),
            spanning_svg_ids: Vec::new(),
        })
        .collect();

    // Assign every SVG node to exactly one band.
    // Tall vertical rules that cross a `RowTemplate` band go into that
    // `RowTemplate` band's `spanning_svg_ids` (or `svg_indices` if un-ID'd).
    let row_template_indices: Vec<usize> = bands
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BandKind::RowTemplate)
        .map(|(i, _)| i)
        .collect();

    for node in &dom.svg_nodes {
        let yt = node.y_top();
        let yb = node.y_bottom();
        let ym = 0.5 * (yt + yb);

        // Check if this is a vertical rule spanning across a RowTemplate band.
        if node.is_vertical_rule() {
            let mut assigned_spanning = false;
            for &rt_idx in &row_template_indices {
                let rt = &bands[rt_idx];
                if yt <= rt.y0 + 2.0 && yb >= rt.y1 - 2.0 {
                    if let Some(rid) = &node.rule_id {
                        bands[rt_idx].spanning_svg_ids.push(rid.clone());
                    } else {
                        bands[rt_idx].svg_indices.push(node.index);
                    }
                    assigned_spanning = true;
                    break;
                }
            }
            if assigned_spanning {
                continue;
            }
        }

        // Otherwise assign by vertical midpoint (or top edge for horizontal rules
        // sitting right on a band boundary).
        let target_y = if let Some((_, y, _, h, _)) = node.rect {
            if h <= 2.5 {
                y + 0.25
            } else {
                ym
            }
        } else {
            ym
        };
        let b_idx = find_owning_band_by_y(&bands, target_y);
        if let Some(rid) = &node.rule_id {
            bands[b_idx].svg_ids.push(rid.clone());
        } else {
            bands[b_idx].svg_indices.push(node.index);
        }
    }

    // Assign every text node to exactly one band.
    for t in &dom.texts {
        if let Some(tid) = &t.id {
            let b_idx = find_owning_band_by_y(&bands, t.top + 2.0);
            bands[b_idx].text_ids.push(tid.clone());
        }
    }

    // Assign every cell to exactly one band.
    for c in &dom.cells {
        let cy = c.top + (c.height * 0.5).max(1.0);
        let b_idx = find_owning_band_by_y(&bands, cy);
        bands[b_idx].cell_ids.push(c.id.clone());
    }

    Some(PageLayout {
        page: dom.page_num,
        width_pt: dom.width_pt,
        height_pt: dom.height_pt,
        content_bottom_y,
        table_x0,
        table_x1,
        bands,
        rule6_fields: Vec::new(),
    })
}

fn find_owning_band_by_y(bands: &[LayoutBand], y: f64) -> usize {
    // Prefer the first non-TrailingSpace band containing `y`.
    for (i, b) in bands.iter().enumerate() {
        if b.kind != BandKind::TrailingSpace && y >= b.y0 - 0.5 && y < b.y1 - 0.1 {
            return i;
        }
    }
    // Fall back to nearest non-TrailingSpace band by distance.
    let mut best = 0usize;
    let mut best_dist = f64::INFINITY;
    for (i, b) in bands.iter().enumerate() {
        if b.kind == BandKind::TrailingSpace {
            continue;
        }
        let dist = if y < b.y0 {
            b.y0 - y
        } else if y > b.y1 {
            y - b.y1
        } else {
            0.0
        };
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

fn detect_header_bottom_from_dom(dom: &FlowPageDom, first_band_y: f64) -> f64 {
    // Look for a thick full-width horizontal rule (h >= 1.0) marking the bottom
    // of the page header / TIN+Name strip (between 60pt and min(170pt, first_band_y)).
    let max_y = first_band_y.min(180.0);
    let mut best = 0.0_f64;
    // First look for TIN/Taxpayer Name header bottom (below any "TIN" text in top 140pt).
    let tin_y = dom
        .texts
        .iter()
        .filter(|t| t.top < max_y && t.plain_text.eq_ignore_ascii_case("TIN"))
        .map(|t| t.top)
        .next();

    let min_y = tin_y.map(|y| y + 14.0).unwrap_or(55.0);
    for node in &dom.svg_nodes {
        if let Some((_, y, w, h, _)) = node.rect {
            if w > dom.width_pt * 0.75 && h >= 1.0 && y >= min_y && y <= max_y {
                // Take the FIRST thick rule after the TIN/Name strip.
                if best == 0.0 || y < best {
                    best = y + h;
                }
            }
        }
    }
    if best > 0.0 {
        best + 0.5
    } else {
        128.0_f64.min(first_band_y - 10.0).max(50.0)
    }
}

fn find_page_label_text_id(dom: &FlowPageDom, header_bottom: f64) -> Option<String> {
    let target = format!("page {}", dom.page_num);
    for t in &dom.texts {
        if t.top <= header_bottom {
            let lower = t.plain_text.trim().to_ascii_lowercase();
            if lower == target || lower.starts_with(&format!("{target} ")) {
                if let Some(id) = &t.id {
                    return Some(id.clone());
                }
            }
        }
    }
    None
}

fn find_table_head_top(dom: &FlowPageDom, row_y0: f64, cursor_y: f64) -> f64 {
    // Walk up from `row_y0` to find the thick outer rule (h >= 1.0, full-width)
    // that opens this schedule/table header, within ~65pt above `row_y0`.
    let min_search = (row_y0 - 70.0).max(cursor_y);
    let mut candidates: Vec<f64> = Vec::new();
    for node in &dom.svg_nodes {
        if let Some((_, y, w, h, _)) = node.rect {
            if w > dom.width_pt * 0.75 && h >= 1.0 && y >= min_search && y < row_y0 - 4.0 {
                candidates.push(y);
            }
        }
    }
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // Prefer the highest thick rule that is followed by a "Schedule" / "Part" / column
    // caption (i.e. within 50pt of `row_y0`).
    for &y in &candidates {
        if row_y0 - y <= 55.0 {
            return y - 0.5;
        }
    }
    candidates.last().copied().unwrap_or(cursor_y)
}

fn find_total_row_bottom(dom: &FlowPageDom, row_y_end: f64, next_limit: f64) -> Option<f64> {
    let max_y = (row_y_end + 34.0).min(next_limit);
    let has_total_text = dom.texts.iter().any(|t| {
        t.top >= row_y_end - 2.0
            && t.top <= max_y
            && t.plain_text.to_ascii_lowercase().contains("total")
    });
    if !has_total_text {
        return None;
    }
    // Find the full-width closing rule below the total text.
    let mut best: Option<f64> = None;
    for node in &dom.svg_nodes {
        if let Some((_, y, w, h, _)) = node.rect {
            if w > dom.width_pt * 0.75 && y >= row_y_end + 8.0 && y <= max_y {
                let bottom = y + h + 0.5;
                if best.is_none_or(|b| bottom < b) {
                    best = Some(bottom);
                }
            }
        }
    }
    best
}

fn find_last_content_bottom(dom: &FlowPageDom, after_y: f64) -> f64 {
    let mut max_y = after_y;
    for node in &dom.svg_nodes {
        let yb = node.y_bottom();
        if yb > max_y && yb <= dom.height_pt {
            max_y = yb;
        }
    }
    for t in &dom.texts {
        if t.top + 10.0 > max_y && t.top <= dom.height_pt {
            max_y = t.top + 10.0;
        }
    }
    for c in &dom.cells {
        let cb = c.top + c.height;
        if cb > max_y && cb <= dom.height_pt {
            max_y = cb;
        }
    }
    max_y
}

// ---------------------------------------------------------------------------
// Band-flow result & conservation report
// ---------------------------------------------------------------------------

/// Output of applying in-page band flow to a filled document.
pub(crate) struct BandFlowOutput {
    /// Updated full document HTML (with flowed pages replacing the source
    /// pages in place, and any spawned `page-<p>-cont-<k>` pages inserted
    /// right after their source page).
    pub document_html: String,
    /// Schedules that could NOT be handled by band flow (no layout / no full
    /// band match) and must still go through [`crate::band_continuation`] /
    /// [`crate::continuation`].
    pub unhandled_schedules: Vec<OverflowSchedule>,
}

/// Forms opted into derived [`PageLayout`] (from existing `formgen-bands`)
/// while waiting for `neon-3` to emit explicit `<script id="formgen-page-layout">`
/// metadata per `form-review/BAND-FORMAT.md`.
///
/// Any form that embeds explicit `formgen-page-layout` automatically uses in-page
/// band flow without appearing in this list. Forms without explicit layout and
/// not yet opted in stay on the existing continuation-sheet / plain-fallback path.
const DERIVED_BAND_FLOW_SLUGS: &[&str] = &["1707-2021"];

/// Apply in-page band flow (Rules 5 & 6) to `official_filled`.
///
/// - If `overflow` is empty (and no Rule 6 insertions trigger), returns
///   `official_filled` unchanged (byte-for-byte identical).
/// - For each overflowing schedule whose page has explicit
///   `formgen-page-layout` (or is in [`DERIVED_BAND_FLOW_SLUGS`] with usable
///   `formgen-bands`), grows the table in place on its official page, slides
///   bands below down, shrinks elastic boxes if needed, and breaks cleanly
///   onto `page-<p>-cont-<k>` pages under the page's own header.
/// - Any remaining schedules without layout coverage are returned in
///   `unhandled_schedules` for the existing continuation/fallback path.
pub(crate) fn apply_band_flow(
    slug: &str,
    raw_html: &str,
    official_filled: &str,
    fields: &BTreeMap<String, String>,
    report: &OverflowReport,
) -> Result<BandFlowOutput, String> {
    if report.is_empty() {
        return Ok(BandFlowOutput {
            document_html: official_filled.to_string(),
            unhandled_schedules: Vec::new(),
        });
    }

    let formgen_bands = parse_formgen_bands(raw_html);
    let explicit_layout = parse_page_layout(raw_html);
    let allow_derived = DERIVED_BAND_FLOW_SLUGS.contains(&slug);
    if explicit_layout.is_none() && !allow_derived {
        return Ok(BandFlowOutput {
            document_html: official_filled.to_string(),
            unhandled_schedules: report.schedules.clone(),
        });
    }
    let wc_parsed: Option<Value> =
        frozen_html::writer_cells_json(slug).and_then(|s| serde_json::from_str::<Value>(s).ok());

    // Map each overflowing schedule to `(page_num, MatchedBand, OverflowSchedule)`.
    // Only flow schedules whose matched bands all sit on a page that has either
    // explicit `formgen-page-layout` or derivable layout from `formgen-bands`.
    let mut per_page_growth: BTreeMap<usize, Vec<(MatchedBand, OverflowSchedule)>> =
        BTreeMap::new();
    let mut unhandled_schedules: Vec<OverflowSchedule> = Vec::new();

    for sched in &report.schedules {
        let matched = wc_parsed
            .as_ref()
            .and_then(|wc| match_schedule_to_bands(sched, &formgen_bands, wc));
        match matched {
            Some(plan) if plan.bands.iter().all(|mb| mb.band.page == plan.page_num) => {
                let entry = per_page_growth.entry(plan.page_num).or_default();
                for mb in plan.bands {
                    entry.push((mb, sched.clone()));
                }
            }
            _ => {
                unhandled_schedules.push(sched.clone());
            }
        }
    }

    if per_page_growth.is_empty() {
        return Ok(BandFlowOutput {
            document_html: official_filled.to_string(),
            unhandled_schedules,
        });
    }

    // Flow each affected page (from highest page index to lowest so byte spans
    // in `official_filled` remain valid while splicing).
    let mut doc = official_filled.to_string();
    let pages_desc: Vec<usize> = per_page_growth.keys().rev().copied().collect();

    for page_num in pages_desc {
        let Some(growths) = per_page_growth.get(&page_num) else {
            continue;
        };
        let dom = parse_flow_page_dom(&doc, page_num)?;
        let page_fb: Vec<&FormgenBand> = formgen_bands
            .iter()
            .filter(|b| b.page == page_num)
            .collect();

        let layout = explicit_layout
            .as_ref()
            .and_then(|d| d.pages.iter().find(|p| p.page == page_num).cloned())
            .or_else(|| {
                if allow_derived {
                    derive_page_layout(&dom, &page_fb)
                } else {
                    None
                }
            });

        // Only flow schedules whose matched band IDs actually exist in `layout.bands`;
        // any schedule without a matching `RowTemplate` falls back to `unhandled_schedules`.
        let mut active_growths: Vec<(MatchedBand, OverflowSchedule)> = Vec::new();
        if let Some(ref layout) = layout {
            for (mb, sched) in growths {
                let has_band = layout.bands.iter().any(|b| {
                    b.kind == BandKind::RowTemplate
                        && b.growable_id.as_deref() == Some(mb.band.id.as_str())
                });
                if has_band {
                    active_growths.push((mb.clone(), sched.clone()));
                } else if !unhandled_schedules
                    .iter()
                    .any(|s| s.heading == sched.heading)
                {
                    unhandled_schedules.push(sched.clone());
                }
            }
        }

        let Some(layout) = layout else {
            for (_, sched) in growths {
                if !unhandled_schedules
                    .iter()
                    .any(|s| s.heading == sched.heading)
                {
                    unhandled_schedules.push(sched.clone());
                }
            }
            continue;
        };

        if active_growths.is_empty() {
            continue;
        }

        let flowed_pages_html =
            flow_single_page(slug, raw_html, &dom, &layout, &active_growths, fields, &[])?;

        let (start, end) = dom.doc_span;
        doc.replace_range(start..end, &flowed_pages_html);
    }

    Ok(BandFlowOutput {
        document_html: doc,
        unhandled_schedules,
    })
}

// ---------------------------------------------------------------------------
// Rule 6 synthetic inserted line
// ---------------------------------------------------------------------------

/// A synthetic full-width bordered rectangle inserted beneath a Rule 6
/// "Other … (specify)" field when its text overflows the inline box.
#[derive(Debug, Clone)]
pub struct Rule6Insertion {
    pub field: Rule6Field,
    pub text: String,
    pub height_pt: f64,
}

impl Rule6Insertion {
    pub fn from_text(field: Rule6Field, text: String) -> Self {
        let width_pt = (field.x1 - field.x0 - 8.0).max(60.0);
        // Conservative 4.3pt average glyph width at 7.5pt font size.
        let chars_per_line = (width_pt / 4.3).floor().max(12.0) as usize;
        let line_count = text.chars().count().div_ceil(chars_per_line).clamp(1, 6);
        let height_pt = 6.0 + (line_count as f64) * 10.5;
        Self {
            field,
            text,
            height_pt,
        }
    }
}

// ---------------------------------------------------------------------------
// Extra row pre-rendering (reuses `frozen_html::fill_continuation_fragment`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RenderedExtraRow {
    /// Relative height (`row_pitch_pt`).
    height_pt: f64,
    /// SVG `<rect .../>` elements with `y` relative to `0.0` (top of this row).
    svg_structural: Vec<BandRect>,
    svg_decorative: Vec<BandRect>,
    /// Text `<div>`s with `top` relative to `0.0`.
    texts: Vec<String>,
    /// Cell `<div>`s (and any cell-keyed overlays) already filled via
    /// `frozen_html::fill_continuation_fragment`, with `top` relative to `0.0`.
    cells: Vec<String>,
}

fn render_extra_rows_for_band(
    slug: &str,
    raw_html: &str,
    dom: &FlowPageDom,
    mb: &MatchedBand,
    sched: &OverflowSchedule,
) -> Result<Vec<RenderedExtraRow>, String> {
    let band = &mb.band;
    let trow_idx = band.template_row.min(band.rows.len().saturating_sub(1));
    let trow = &band.rows[trow_idx];
    let trow_y0 = band.row_y.get(trow_idx).copied().unwrap_or(trow.y);
    let trow_y1 = band
        .row_y
        .get(trow_idx + 1)
        .copied()
        .unwrap_or(trow_y0 + band.row_pitch_pt);

    // Collect template cell raw HTML from `raw_html` (clean unfilled shells).
    let raw_dom = parse_flow_page_dom(raw_html, dom.page_num)?;
    let raw_cells_by_id: BTreeMap<&str, &FlowCellNode> =
        raw_dom.cells.iter().map(|c| (c.id.as_str(), c)).collect();
    let raw_texts_by_id: BTreeMap<&str, &FlowTextNode> = raw_dom
        .texts
        .iter()
        .filter_map(|t| t.id.as_deref().map(|id| (id, t)))
        .collect();

    // Un-ID'd structural / decorative rects inside the template row:
    // prefer `trow.structural_rects` (BAND-FORMAT.md section 6) when present;
    // otherwise scan `raw_dom.svg_nodes` within `[trow_y0, trow_y1]`.
    let mut tpl_structural_extra: Vec<BandRect> = Vec::new();
    let mut tpl_decorative_extra: Vec<BandRect> = Vec::new();
    if !trow.structural_rects.is_empty() {
        for r in &trow.structural_rects {
            tpl_structural_extra.push(BandRect {
                x: r.x,
                y: r.y - trow_y0,
                w: r.w,
                h: r.h,
                fill: r.fill.clone(),
                id: None,
            });
        }
    } else {
        for node in &raw_dom.svg_nodes {
            if node.rule_id.is_some() {
                continue;
            }
            if let Some((x, y, w, h, ref fill)) = node.rect {
                if y >= trow_y0 - 0.5 && y + h <= trow_y1 + 0.5 && w < raw_dom.width_pt * 0.9 {
                    let br = BandRect {
                        x,
                        y: y - trow_y0,
                        w,
                        h,
                        fill: fill.clone(),
                        id: None,
                    };
                    if node.layer == "layer-decorative" {
                        tpl_decorative_extra.push(br);
                    } else {
                        tpl_structural_extra.push(br);
                    }
                }
            }
        }
    }

    // Template internal horizontal divider (the boundary rule between rows).
    let tb_idx = band
        .template_boundary
        .min(band.boundaries.len().saturating_sub(1));
    let boundary_rects: Vec<BandRect> = band
        .boundaries
        .get(tb_idx)
        .map(|rects| {
            let by = band.row_y.get(tb_idx).copied().unwrap_or(trow_y0);
            rects
                .iter()
                .map(|r| BandRect {
                    x: r.x,
                    y: r.y - by,
                    w: r.w,
                    h: r.h,
                    fill: r.fill.clone(),
                    id: None,
                })
                .collect()
        })
        .unwrap_or_default();

    // Template internal rules (shifted relative to `trow_y0`).
    let tpl_rules: Vec<BandRect> = trow
        .rules
        .iter()
        .map(|r| BandRect {
            x: r.x,
            y: r.y - trow_y0,
            w: r.w,
            h: r.h,
            fill: r.fill.clone(),
            id: None,
        })
        .collect();

    // Build all cloned cell shells + fill map in ONE batch so we call
    // `frozen_html::fill_continuation_fragment` once for the band.
    let mut batch_cells_html = String::new();
    let mut batch_values: BTreeMap<String, String> = BTreeMap::new();
    let mut extra_joins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut extra_money_joins: Vec<(String, String, Option<String>)> = Vec::new();
    let mut extra_peso_joins: Vec<(String, String)> = Vec::new();
    let mut batch_aliases: BTreeMap<String, String> = BTreeMap::new();
    let mut row_cell_ids: Vec<Vec<String>> = Vec::with_capacity(sched.rows.len());

    let mut out_rows: Vec<RenderedExtraRow> = Vec::with_capacity(sched.rows.len());

    for (r_idx, ov_row) in sched.rows.iter().enumerate() {
        let mut svg_structural = Vec::new();
        svg_structural.extend(boundary_rects.iter().cloned());
        svg_structural.extend(tpl_rules.iter().cloned());
        svg_structural.extend(tpl_structural_extra.iter().cloned());
        let svg_decorative = tpl_decorative_extra.clone();

        // Clone row texts (enumerated row label `E, F, G` or `5, 6, 7`, plus dots `●`).
        let mut texts = Vec::new();
        for bt in &trow.texts {
            let Some(tid) = bt.id.as_deref() else {
                continue;
            };
            let Some(orig_t) = raw_texts_by_id.get(tid) else {
                continue;
            };
            let mut elem = shift_top_in_tag(&orig_t.raw_html, -trow_y0);
            // Strip source `id="..."` on cloned rows so IDs stay unique for conservation.
            elem = strip_id_attr(&elem);
            if bt.role.as_deref() == Some("enumerated") {
                let label = format_row_label(ov_row.row_number, sched.row_label_lettered);
                elem = replace_inner_text(&elem, &label);
            }
            texts.push(elem);
        }

        // Clone cells relative to `trow_y0` (top = orig_top - trow_y0).
        let mut ids_for_row = Vec::new();
        // Track money pairs `(peso_id, cents_id_opt)` per `col_idx` in this row.
        let mut money_pairs: BTreeMap<usize, (Option<String>, Option<String>)> = BTreeMap::new();

        for bc in &trow.cells {
            let Some(orig_c) = raw_cells_by_id.get(bc.id.as_str()) else {
                continue;
            };
            let new_id = format!("bf_p{}_{}_r{}_{}", dom.page_num, band.id, r_idx, bc.id);
            let cloned = clone_cell_div(&orig_c.raw_html, &bc.id, &new_id, -trow_y0);
            batch_cells_html.push_str(&cloned);
            ids_for_row.push(new_id.clone());
            batch_aliases.insert(new_id.clone(), bc.id.clone());

            if let Some(join) = mb.cell_map.get(&bc.id) {
                match join {
                    CellJoinKind::Text { col_idx, .. } => {
                        if let Some(val) = ov_row.values.get(*col_idx) {
                            if !val.trim().is_empty() {
                                let syn_key = format!("__bf_t_{r_idx}_{new_id}");
                                batch_values.insert(syn_key.clone(), val.clone());
                                extra_joins.insert(syn_key, vec![new_id]);
                            }
                        }
                    }
                    CellJoinKind::Money {
                        col_idx, is_peso, ..
                    } => {
                        let entry = money_pairs.entry(*col_idx).or_insert((None, None));
                        if *is_peso {
                            entry.0 = Some(new_id);
                        } else {
                            entry.1 = Some(new_id);
                        }
                    }
                    CellJoinKind::Peso { col_idx, .. } => {
                        if let Some(val) = ov_row.values.get(*col_idx) {
                            if !val.trim().is_empty() {
                                let syn_key = format!("__bf_p_{r_idx}_{new_id}");
                                batch_values.insert(syn_key.clone(), val.clone());
                                extra_peso_joins.push((syn_key, new_id));
                            }
                        }
                    }
                    CellJoinKind::Ordinal { .. } => {
                        let syn_key = format!("__bf_o_{r_idx}_{new_id}");
                        batch_values.insert(syn_key.clone(), ov_row.row_number.to_string());
                        extra_joins.insert(syn_key, vec![new_id]);
                    }
                }
            }
        }
        for (col_idx, (peso_opt, cents_opt)) in money_pairs {
            if let Some(peso_id) = peso_opt {
                if let Some(val) = ov_row.values.get(col_idx) {
                    if !val.trim().is_empty() {
                        let syn_key = format!("__bf_m_{r_idx}_{col_idx}");
                        batch_values.insert(syn_key.clone(), val.clone());
                        extra_money_joins.push((syn_key, peso_id, cents_opt));
                    }
                }
            }
        }
        row_cell_ids.push(ids_for_row);

        out_rows.push(RenderedExtraRow {
            height_pt: band.row_pitch_pt,
            svg_structural,
            svg_decorative,
            texts,
            cells: Vec::new(),
        });
    }

    // Fill all cloned cells through `frozen_html`'s exact same comb/text/overflow path.
    let filled_fragment = frozen_html::fill_continuation_fragment(
        &batch_cells_html,
        &batch_values,
        slug,
        &extra_joins,
        &extra_money_joins,
        &extra_peso_joins,
        &batch_aliases,
    )?;

    // Parse filled cloned cells (plus any `data-overflow-of` overlays) back into each row.
    let filled_map = parse_filled_cells_fragment(&filled_fragment);
    for (r_idx, ids) in row_cell_ids.into_iter().enumerate() {
        for cid in ids {
            if let Some(html_chunks) = filled_map.get(&cid) {
                out_rows[r_idx].cells.extend(html_chunks.iter().cloned());
            }
        }
    }

    Ok(out_rows)
}

fn format_row_label(row_number: usize, lettered: bool) -> String {
    if !lettered || row_number == 0 {
        return row_number.to_string();
    }
    // 1 -> A, 26 -> Z, 27 -> AA, ...
    let mut n = row_number;
    let mut chars = Vec::new();
    while n > 0 {
        n -= 1;
        chars.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    chars.reverse();
    chars.into_iter().collect()
}

fn parse_filled_cells_fragment(fragment: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut pos = 0;
    while pos < fragment.len() {
        let Some(rel) = fragment[pos..].find('<') else {
            break;
        };
        let tag_open = pos + rel;
        if fragment[tag_open..].starts_with("</") {
            pos = tag_open + 2;
            continue;
        }
        let Some(gt_rel) = fragment[tag_open..].find('>') else {
            break;
        };
        let open_slice = &fragment[tag_open..tag_open + gt_rel + 1];
        let elem_end = if open_slice.starts_with("<div") {
            find_matching_div_end(fragment, tag_open).unwrap_or(tag_open + gt_rel + 1)
        } else if open_slice.starts_with("<input") || open_slice.ends_with("/>") {
            tag_open + gt_rel + 1
        } else if open_slice.starts_with("<span") {
            fragment[tag_open + gt_rel + 1..]
                .find("</span>")
                .map(|e| tag_open + gt_rel + 1 + e + 7)
                .unwrap_or(tag_open + gt_rel + 1)
        } else {
            pos = tag_open + gt_rel + 1;
            continue;
        };
        let raw = &fragment[tag_open..elem_end];
        if let Some(owner) = extract_attr(open_slice, "data-overflow-of")
            .or_else(|| extract_attr(open_slice, "data-cell-id"))
        {
            map.entry(owner.to_string())
                .or_default()
                .push(raw.to_string());
        } else if let Some(id) = extract_attr(open_slice, "id") {
            map.entry(id.to_string()).or_default().push(raw.to_string());
        }
        pos = elem_end;
    }
    map
}

fn strip_id_attr(elem: &str) -> String {
    if let Some(gt) = elem.find('>') {
        let open = &elem[..gt];
        if let Some(pos) = open.find(" id=\"") {
            let after = &open[pos + 5..];
            if let Some(end_q) = after.find('"') {
                return format!("{}{}{}", &open[..pos], &after[end_q + 1..], &elem[gt..]);
            }
        }
    }
    elem.to_string()
}

// ---------------------------------------------------------------------------
// Core greedy band packer & page emitter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum FlowItem {
    /// An original non-`RowTemplate` band from `layout.bands[band_idx]`.
    StaticBand {
        band_idx: usize,
        height_pt: f64,
        /// Amount shrunk from `elastic_box` (`0.0` for non-elastic bands).
        shrunk_pt: f64,
    },
    /// The original `k` printed rows of a `RowTemplate` band (always emitted
    /// together as one atomic block on the page where the table starts), plus
    /// `extra_rows` that can break between rows across pages.
    GrowableTable {
        band_idx: usize,
        /// Optional `table_head` band index directly preceding this table
        /// (repeated on continuation pages with `"(continued)"`).
        head_band_idx: Option<usize>,
        /// Optional `total_row` band index directly following this table
        /// (stays attached to the final row; omitted on breaking pages).
        total_band_idx: Option<usize>,
        /// Original printed rows height (`band.y1 - band.y0`).
        orig_height_pt: f64,
        /// Extra rows to append directly under the printed rows.
        extra_rows: Vec<RenderedExtraRow>,
    },
    /// Synthetic Rule 6 inserted line band.
    Rule6Line(Rule6Insertion),
}

/// One placed band/slice on an output page.
#[derive(Debug, Clone)]
enum PlacedSlice {
    OriginalBand {
        band_idx: usize,
        dest_y: f64,
        shrunk_pt: f64,
        is_repeated_header: bool,
        is_repeated_table_head: bool,
    },
    /// Original printed rows of a `RowTemplate` band + a subset `[0..end_extra)`
    /// of its extra rows.
    OrigRowsPlusExtra {
        band_idx: usize,
        dest_y: f64,
        orig_height_pt: f64,
        extra_rows: Vec<RenderedExtraRow>,
        /// True when the table continues on the next page (draw closing rule
        /// + `"Continued on the next page"` cue; omit `total_row`).
        continues_next: bool,
        /// Height of `total_row` if `total_row` sits right below on this page
        /// (used to extend spanning vertical rules into `total_row`).
        total_follows_height: f64,
        /// Height of `table_head` if `table_head` sits right above on this page.
        head_precedes_height: f64,
    },
    /// Continuation slice of extra rows `[start..end)` on a spawned page.
    ExtraRowsSlice {
        band_idx: usize,
        dest_y: f64,
        extra_rows: Vec<RenderedExtraRow>,
        continues_next: bool,
        total_follows_height: f64,
        head_precedes_height: f64,
    },
    Rule6Line {
        dest_y: f64,
        ins: Rule6Insertion,
    },
}

struct PackedPage {
    /// 0 for the original page, 1, 2, ... for spawned continuation pages.
    cont_index: usize,
    slices: Vec<PlacedSlice>,
}

fn flow_single_page(
    slug: &str,
    raw_html: &str,
    dom: &FlowPageDom,
    layout: &PageLayout,
    growths: &[(MatchedBand, OverflowSchedule)],
    _fields: &BTreeMap<String, String>,
    rule6_insertions: &[Rule6Insertion],
) -> Result<String, String> {
    let packed = pack_page(slug, raw_html, dom, layout, growths, rule6_insertions)?;
    let mut out = String::new();
    for pp in &packed {
        out.push_str(&emit_packed_page(dom, layout, pp));
    }
    Ok(out)
}

fn pack_page(
    slug: &str,
    raw_html: &str,
    dom: &FlowPageDom,
    layout: &PageLayout,
    growths: &[(MatchedBand, OverflowSchedule)],
    rule6_insertions: &[Rule6Insertion],
) -> Result<Vec<PackedPage>, String> {
    // Map `growable_id -> RenderedExtraRow` list.
    let mut extra_by_growable: BTreeMap<String, Vec<RenderedExtraRow>> = BTreeMap::new();
    for (mb, sched) in growths {
        let rendered = render_extra_rows_for_band(slug, raw_html, dom, mb, sched)?;
        extra_by_growable.insert(mb.band.id.clone(), rendered);
    }

    let header_idx = layout
        .bands
        .iter()
        .position(|b| b.kind == BandKind::PageHeader);
    let header_height = header_idx
        .map(|i| layout.bands[i].y1 - layout.bands[i].y0)
        .unwrap_or(0.0);

    // Build flat `FlowItem` stream (skipping `PageHeader` and `TrailingSpace`,
    // and pairing `TotalRow` with its preceding `RowTemplate` when growing).
    let mut items: Vec<FlowItem> = Vec::new();
    let mut consumed_band: BTreeSet<usize> = BTreeSet::new();
    if let Some(hi) = header_idx {
        consumed_band.insert(hi);
    }

    for (i, b) in layout.bands.iter().enumerate() {
        if consumed_band.contains(&i) || b.kind == BandKind::TrailingSpace {
            continue;
        }
        if b.kind == BandKind::RowTemplate {
            let extra = b
                .growable_id
                .as_ref()
                .and_then(|gid| extra_by_growable.remove(gid))
                .unwrap_or_default();
            // Look back for immediately preceding `TableHead` in same `table_group`.
            let head_band_idx = if i > 0
                && layout.bands[i - 1].kind == BandKind::TableHead
                && layout.bands[i - 1].table_group == b.table_group
            {
                Some(i - 1)
            } else {
                None
            };
            // Look ahead for immediately following `TotalRow` in same `table_group`.
            let total_band_idx = if i + 1 < layout.bands.len()
                && layout.bands[i + 1].kind == BandKind::TotalRow
                && layout.bands[i + 1].table_group == b.table_group
            {
                consumed_band.insert(i + 1);
                Some(i + 1)
            } else {
                None
            };
            items.push(FlowItem::GrowableTable {
                band_idx: i,
                head_band_idx,
                total_band_idx,
                orig_height_pt: b.y1 - b.y0,
                extra_rows: extra,
            });
        } else {
            items.push(FlowItem::StaticBand {
                band_idx: i,
                height_pt: b.y1 - b.y0,
                shrunk_pt: 0.0,
            });
            // Inject any Rule 6 line anchored to this band.
            for ins in rule6_insertions {
                if ins.field.band_id == b.id {
                    items.push(FlowItem::Rule6Line(ins.clone()));
                }
            }
        }
    }

    // First, try elastic compression if all items would fit on page 0 once
    // elastic boxes shrink down to `min_height_pt`.
    let total_needed_y = header_height + total_items_height(&items, layout);
    if total_needed_y > layout.content_bottom_y {
        let mut deficit = total_needed_y - layout.content_bottom_y;
        for item in &mut items {
            if deficit <= 0.0 {
                break;
            }
            if let FlowItem::StaticBand {
                band_idx,
                height_pt,
                shrunk_pt,
            } = item
            {
                let b = &layout.bands[*band_idx];
                if b.kind == BandKind::ElasticBox {
                    if let Some(min_h) = b.min_height_pt {
                        let avail = (*height_pt - min_h).max(0.0);
                        let take = avail.min(deficit);
                        *height_pt -= take;
                        *shrunk_pt += take;
                        deficit -= take;
                    }
                }
            }
        }
    }

    // Greedy top-to-bottom packer across pages.
    let mut pages: Vec<PackedPage> = Vec::new();
    let mut cur_slices: Vec<PlacedSlice> = Vec::new();
    let mut cur_y = 0.0;
    if let Some(hi) = header_idx {
        cur_slices.push(PlacedSlice::OriginalBand {
            band_idx: hi,
            dest_y: layout.bands[hi].y0,
            shrunk_pt: 0.0,
            is_repeated_header: false,
            is_repeated_table_head: false,
        });
        cur_y = layout.bands[hi].y1;
    }

    let start_new_page =
        |pages: &mut Vec<PackedPage>, cur_slices: &mut Vec<PlacedSlice>, cur_y: &mut f64| {
            let cont_index = pages.len();
            pages.push(PackedPage {
                cont_index,
                slices: std::mem::take(cur_slices),
            });
            if let Some(hi) = header_idx {
                cur_slices.push(PlacedSlice::OriginalBand {
                    band_idx: hi,
                    dest_y: layout.bands[hi].y0,
                    shrunk_pt: 0.0,
                    is_repeated_header: true,
                    is_repeated_table_head: false,
                });
                *cur_y = layout.bands[hi].y1;
            } else {
                *cur_y = 20.0;
            }
        };

    let mut idx = 0;
    while idx < items.len() {
        match &items[idx] {
            FlowItem::StaticBand {
                band_idx,
                height_pt,
                shrunk_pt,
            } => {
                let b = &layout.bands[*band_idx];
                // Keep `SectionTitle` / `TableHead` (`keep_with_next`) with the
                // next item so a heading is never orphaned at the bottom.
                let min_group_h = if b.keep_with_next {
                    *height_pt + min_next_unit_height(&items, idx + 1, layout)
                } else {
                    *height_pt
                };
                if cur_y + min_group_h > layout.content_bottom_y && cur_slices.len() > 1 {
                    start_new_page(&mut pages, &mut cur_slices, &mut cur_y);
                }
                cur_slices.push(PlacedSlice::OriginalBand {
                    band_idx: *band_idx,
                    dest_y: cur_y,
                    shrunk_pt: *shrunk_pt,
                    is_repeated_header: false,
                    is_repeated_table_head: false,
                });
                cur_y += *height_pt;
                idx += 1;
            }
            FlowItem::Rule6Line(ins) => {
                if cur_y + ins.height_pt > layout.content_bottom_y && cur_slices.len() > 1 {
                    start_new_page(&mut pages, &mut cur_slices, &mut cur_y);
                }
                cur_slices.push(PlacedSlice::Rule6Line {
                    dest_y: cur_y,
                    ins: ins.clone(),
                });
                cur_y += ins.height_pt;
                idx += 1;
            }
            FlowItem::GrowableTable {
                band_idx,
                head_band_idx,
                total_band_idx,
                orig_height_pt,
                extra_rows,
            } => {
                let head_h = head_band_idx
                    .map(|hi| layout.bands[hi].y1 - layout.bands[hi].y0)
                    .unwrap_or(0.0);
                let total_h = total_band_idx
                    .map(|ti| layout.bands[ti].y1 - layout.bands[ti].y0)
                    .unwrap_or(0.0);
                let extra_total_h: f64 = extra_rows.iter().map(|r| r.height_pt).sum();

                // Case 1: entire table (printed rows + all extra rows + total) fits here!
                if cur_y + *orig_height_pt + extra_total_h + total_h <= layout.content_bottom_y {
                    cur_slices.push(PlacedSlice::OrigRowsPlusExtra {
                        band_idx: *band_idx,
                        dest_y: cur_y,
                        orig_height_pt: *orig_height_pt,
                        extra_rows: extra_rows.clone(),
                        continues_next: false,
                        total_follows_height: total_h,
                        head_precedes_height: head_h,
                    });
                    cur_y += *orig_height_pt + extra_total_h;
                    if let Some(ti) = total_band_idx {
                        cur_slices.push(PlacedSlice::OriginalBand {
                            band_idx: *ti,
                            dest_y: cur_y,
                            shrunk_pt: 0.0,
                            is_repeated_header: false,
                            is_repeated_table_head: false,
                        });
                        cur_y += total_h;
                    }
                    idx += 1;
                    continue;
                }

                // Case 2: table needs to split across pages!
                // Place original printed rows + as many extra rows as fit on the
                // current page, reserving `BREAK_CUE_HEIGHT_PT` and keeping at
                // least 1 extra row to stay with `total_row` on the next page.
                let mut take = 0usize;
                let mut used_extra_h = 0.0;
                let max_take_here = if extra_rows.is_empty() {
                    0
                } else {
                    extra_rows.len().saturating_sub(1)
                };
                while take < max_take_here {
                    let rh = extra_rows[take].height_pt;
                    if cur_y + *orig_height_pt + used_extra_h + rh + BREAK_CUE_HEIGHT_PT
                        <= layout.content_bottom_y
                    {
                        used_extra_h += rh;
                        take += 1;
                    } else {
                        break;
                    }
                }

                // If even `orig_height_pt + BREAK_CUE_HEIGHT_PT` doesn't fit on
                // this page (and we already placed earlier bands), break first.
                if cur_y + *orig_height_pt + BREAK_CUE_HEIGHT_PT > layout.content_bottom_y
                    && cur_slices.len() > 1
                {
                    // Pull preceding `TableHead` onto the new page too if it was
                    // just placed right above us.
                    let pulled_head = if let Some(hi) = head_band_idx {
                        if matches!(
                            cur_slices.last(),
                            Some(PlacedSlice::OriginalBand { band_idx: last_b, .. }) if *last_b == *hi
                        ) {
                            cur_slices.pop();
                            Some(*hi)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    start_new_page(&mut pages, &mut cur_slices, &mut cur_y);
                    if let Some(hi) = pulled_head {
                        cur_slices.push(PlacedSlice::OriginalBand {
                            band_idx: hi,
                            dest_y: cur_y,
                            shrunk_pt: 0.0,
                            is_repeated_header: false,
                            is_repeated_table_head: false,
                        });
                        cur_y += head_h;
                    }
                    continue;
                }

                cur_slices.push(PlacedSlice::OrigRowsPlusExtra {
                    band_idx: *band_idx,
                    dest_y: cur_y,
                    orig_height_pt: *orig_height_pt,
                    extra_rows: extra_rows[..take].to_vec(),
                    continues_next: true,
                    total_follows_height: 0.0,
                    head_precedes_height: head_h,
                });

                // Spill remaining extra rows onto continuation pages.
                let mut cursor = take;
                while cursor < extra_rows.len() {
                    start_new_page(&mut pages, &mut cur_slices, &mut cur_y);

                    let mut cont_head_h = 0.0;
                    if let Some(hi) = head_band_idx {
                        cur_slices.push(PlacedSlice::OriginalBand {
                            band_idx: *hi,
                            dest_y: cur_y,
                            shrunk_pt: 0.0,
                            is_repeated_header: false,
                            is_repeated_table_head: true,
                        });
                        cur_y += head_h;
                        cont_head_h = head_h;
                    }

                    let rem_h: f64 = extra_rows[cursor..].iter().map(|r| r.height_pt).sum();
                    if cur_y + rem_h + total_h <= layout.content_bottom_y {
                        // All remaining rows + `total_row` fit on this page!
                        cur_slices.push(PlacedSlice::ExtraRowsSlice {
                            band_idx: *band_idx,
                            dest_y: cur_y,
                            extra_rows: extra_rows[cursor..].to_vec(),
                            continues_next: false,
                            total_follows_height: total_h,
                            head_precedes_height: cont_head_h,
                        });
                        cur_y += rem_h;
                        if let Some(ti) = total_band_idx {
                            cur_slices.push(PlacedSlice::OriginalBand {
                                band_idx: *ti,
                                dest_y: cur_y,
                                shrunk_pt: 0.0,
                                is_repeated_header: false,
                                is_repeated_table_head: false,
                            });
                            cur_y += total_h;
                        }
                        cursor = extra_rows.len();
                    } else {
                        // Pack as many rows as fit while reserving `BREAK_CUE_HEIGHT_PT`
                        // and keeping at least 1 row for the final page with `total_row`.
                        let mut chunk_end = cursor;
                        let mut chunk_h = 0.0;
                        let max_end = extra_rows.len().saturating_sub(1).max(cursor + 1);
                        while chunk_end < max_end {
                            let rh = extra_rows[chunk_end].height_pt;
                            if chunk_end == cursor
                                || cur_y + chunk_h + rh + BREAK_CUE_HEIGHT_PT
                                    <= layout.content_bottom_y
                            {
                                chunk_h += rh;
                                chunk_end += 1;
                            } else {
                                break;
                            }
                        }
                        cur_slices.push(PlacedSlice::ExtraRowsSlice {
                            band_idx: *band_idx,
                            dest_y: cur_y,
                            extra_rows: extra_rows[cursor..chunk_end].to_vec(),
                            continues_next: true,
                            total_follows_height: 0.0,
                            head_precedes_height: cont_head_h,
                        });
                        cursor = chunk_end;
                    }
                }
                idx += 1;
            }
        }
    }

    if !cur_slices.is_empty() {
        let cont_index = pages.len();
        pages.push(PackedPage {
            cont_index,
            slices: cur_slices,
        });
    }

    Ok(pages)
}

fn total_items_height(items: &[FlowItem], layout: &PageLayout) -> f64 {
    items
        .iter()
        .map(|it| match it {
            FlowItem::StaticBand { height_pt, .. } => *height_pt,
            FlowItem::Rule6Line(ins) => ins.height_pt,
            FlowItem::GrowableTable {
                total_band_idx,
                orig_height_pt,
                extra_rows,
                ..
            } => {
                let extra_h: f64 = extra_rows.iter().map(|r| r.height_pt).sum();
                let tot_h = total_band_idx
                    .map(|ti| layout.bands[ti].y1 - layout.bands[ti].y0)
                    .unwrap_or(0.0);
                *orig_height_pt + extra_h + tot_h
            }
        })
        .sum()
}

fn min_next_unit_height(items: &[FlowItem], next_idx: usize, layout: &PageLayout) -> f64 {
    let Some(next) = items.get(next_idx) else {
        return 0.0;
    };
    match next {
        FlowItem::StaticBand {
            band_idx,
            height_pt,
            ..
        } => {
            if layout.bands[*band_idx].keep_with_next {
                *height_pt + min_next_unit_height(items, next_idx + 1, layout)
            } else {
                *height_pt
            }
        }
        FlowItem::Rule6Line(ins) => ins.height_pt,
        FlowItem::GrowableTable {
            orig_height_pt,
            extra_rows,
            total_band_idx,
            ..
        } => {
            if extra_rows.is_empty() {
                let tot_h = total_band_idx
                    .map(|ti| layout.bands[ti].y1 - layout.bands[ti].y0)
                    .unwrap_or(0.0);
                *orig_height_pt + tot_h
            } else {
                *orig_height_pt + BREAK_CUE_HEIGHT_PT
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTML emitter for a single packed page (flat 3-layer DOM)
// ---------------------------------------------------------------------------

fn emit_packed_page(dom: &FlowPageDom, layout: &PageLayout, pp: &PackedPage) -> String {
    let svg_by_id: BTreeMap<&str, &FlowSvgNode> = dom
        .svg_nodes
        .iter()
        .filter_map(|n| n.rule_id.as_deref().map(|id| (id, n)))
        .collect();
    let text_by_id: BTreeMap<&str, &FlowTextNode> = dom
        .texts
        .iter()
        .filter_map(|t| t.id.as_deref().map(|id| (id, t)))
        .collect();
    let cell_by_id: BTreeMap<&str, &FlowCellNode> =
        dom.cells.iter().map(|c| (c.id.as_str(), c)).collect();

    let mut svg_out = String::new();
    let mut text_out = String::new();
    let mut cells_out = String::new();

    // Preserve SVG `<image .../>` (BIR logo/barcode) inside the page header.
    for img in &dom.svg_images {
        svg_out.push_str(img);
    }

    for slice in &pp.slices {
        match slice {
            PlacedSlice::OriginalBand {
                band_idx,
                dest_y,
                shrunk_pt,
                is_repeated_header,
                is_repeated_table_head,
            } => {
                let band = &layout.bands[*band_idx];
                let dy = dest_y - band.y0;
                emit_original_band(
                    band,
                    dy,
                    *shrunk_pt,
                    *is_repeated_header,
                    *is_repeated_table_head,
                    dom,
                    &svg_by_id,
                    &text_by_id,
                    &cell_by_id,
                    &mut svg_out,
                    &mut text_out,
                    &mut cells_out,
                );
            }
            PlacedSlice::OrigRowsPlusExtra {
                band_idx,
                dest_y,
                orig_height_pt,
                extra_rows,
                continues_next,
                total_follows_height,
                head_precedes_height,
            } => {
                let band = &layout.bands[*band_idx];
                let dy = dest_y - band.y0;

                // 1. Emit the original printed rows (SVG non-spanning, texts, cells).
                emit_original_band_non_spanning(
                    band,
                    dy,
                    dom,
                    &svg_by_id,
                    &text_by_id,
                    &cell_by_id,
                    &mut svg_out,
                    &mut text_out,
                    &mut cells_out,
                );

                // 2. Emit extra rows directly beneath the original printed rows.
                let mut row_y = dest_y + *orig_height_pt;
                for er in extra_rows {
                    emit_extra_row(er, row_y, &mut svg_out, &mut text_out, &mut cells_out);
                    row_y += er.height_pt;
                }

                // 3. Emit spanning vertical borders adjusted to the exact span on this page.
                emit_spanning_rules(
                    band,
                    dy,
                    *dest_y,
                    row_y,
                    *head_precedes_height,
                    *total_follows_height,
                    *continues_next,
                    &svg_by_id,
                    &mut svg_out,
                );

                // 4. On a mid-table break, close the table cleanly and print the cue.
                if *continues_next {
                    emit_break_closing_and_cue(
                        layout.table_x0,
                        layout.table_x1,
                        row_y,
                        &mut svg_out,
                        &mut text_out,
                    );
                }
            }
            PlacedSlice::ExtraRowsSlice {
                band_idx,
                dest_y,
                extra_rows,
                continues_next,
                total_follows_height,
                head_precedes_height,
            } => {
                let band = &layout.bands[*band_idx];
                let mut row_y = *dest_y;
                for er in extra_rows {
                    emit_extra_row(er, row_y, &mut svg_out, &mut text_out, &mut cells_out);
                    row_y += er.height_pt;
                }
                emit_spanning_rules(
                    band,
                    dest_y - band.y0,
                    *dest_y,
                    row_y,
                    *head_precedes_height,
                    *total_follows_height,
                    *continues_next,
                    &svg_by_id,
                    &mut svg_out,
                );
                if *continues_next {
                    emit_break_closing_and_cue(
                        layout.table_x0,
                        layout.table_x1,
                        row_y,
                        &mut svg_out,
                        &mut text_out,
                    );
                }
            }
            PlacedSlice::Rule6Line { dest_y, ins } => {
                emit_rule6_line(*dest_y, ins, &mut svg_out, &mut text_out);
            }
        }
    }

    let (page_class, page_id) = if pp.cont_index == 0 {
        (
            format!("page page-{}", dom.page_num),
            format!("page-{}", dom.page_num),
        )
    } else {
        (
            format!("page page-{}-cont-{}", dom.page_num, pp.cont_index),
            format!("page-{}-cont-{}", dom.page_num, pp.cont_index),
        )
    };

    format!(
        "<div class=\"{page_class}\" id=\"{page_id}\" style=\"width:{}pt;height:{}pt\">\
         {svg_open}{svg_out}</svg>\
         <div class=\"layer-text\">{text_out}</div>\
         <div class=\"layer-cells\">{cells_out}</div>\
         </div>",
        fmt_pt(dom.width_pt),
        fmt_pt(dom.height_pt),
        svg_open = dom.svg_open_tag,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_original_band(
    band: &LayoutBand,
    dy: f64,
    shrunk_pt: f64,
    is_repeated_header: bool,
    is_repeated_table_head: bool,
    dom: &FlowPageDom,
    svg_by_id: &BTreeMap<&str, &FlowSvgNode>,
    text_by_id: &BTreeMap<&str, &FlowTextNode>,
    cell_by_id: &BTreeMap<&str, &FlowCellNode>,
    svg_out: &mut String,
    text_out: &mut String,
    cells_out: &mut String,
) {
    // Emit SVG nodes (handling `shrunk_pt` for `elastic_box`).
    for rid in &band.svg_ids {
        if let Some(node) = svg_by_id.get(rid.as_str()) {
            emit_svg_node_shifted(node, band, dy, shrunk_pt, svg_out);
        }
    }
    for &sidx in &band.svg_indices {
        if let Some(node) = dom.svg_nodes.get(sidx) {
            emit_svg_node_shifted(node, band, dy, shrunk_pt, svg_out);
        }
    }

    // Emit texts.
    // On a repeated table head, if a sibling "(attach additional sheet/s, if necessary)"
    // note sits on the title line, replace THAT note with "(continued)" so the wording
    // appears once cleanly without colliding with the sibling span.
    let attach_note_tid: Option<&str> = if is_repeated_table_head {
        band.text_ids.iter().map(|s| s.as_str()).find(|tid| {
            text_by_id
                .get(tid)
                .map(|t| {
                    t.plain_text
                        .to_ascii_lowercase()
                        .contains("attach additional")
                })
                .unwrap_or(false)
        })
    } else {
        None
    };
    let mut added_continued_heading = false;
    for tid in &band.text_ids {
        let Some(t) = text_by_id.get(tid.as_str()) else {
            continue;
        };
        let mut elem = shift_top_in_tag(&t.raw_html, dy);
        if is_repeated_header && band.page_label_text_id.as_deref() == Some(tid.as_str()) {
            let new_label = format!("Page {} (continued)", dom.page_num);
            elem = replace_inner_text(&elem, &new_label);
            // Keep "Page N (continued)" inside its header box: pull left if near
            // the right edge or centered inside a narrow top-left form-number box.
            if t.left > dom.width_pt - 115.0 {
                elem = set_left_in_style(&elem, (dom.width_pt - 115.0).max(18.0));
            } else if t.left < 120.0 {
                elem = set_left_in_style(&elem, (t.left - 24.0).max(20.0));
            }
        } else if is_repeated_table_head && attach_note_tid == Some(tid.as_str()) {
            elem = replace_inner_text(&elem, "(continued)");
            added_continued_heading = true;
        } else if is_repeated_table_head && attach_note_tid.is_none() && !added_continued_heading {
            let trimmed = t.plain_text.trim();
            let lower = trimmed.to_ascii_lowercase();
            if trimmed.len() >= 8
                && (lower.contains("schedule")
                    || lower.contains("part ")
                    || lower.contains("details"))
            {
                if !lower.contains("continued") && !lower.contains("continuation") {
                    elem = replace_inner_text(&elem, &format!("{trimmed} (continued)"));
                }
                added_continued_heading = true;
            }
        }
        text_out.push_str(&elem);
    }

    // Emit cells + every fill-time overlay keyed to each cell.
    for cid in &band.cell_ids {
        let Some(c) = cell_by_id.get(cid.as_str()) else {
            continue;
        };
        cells_out.push_str(&shift_top_in_tag(&c.raw_html, dy));
        for ov in &c.overlays {
            cells_out.push_str(&shift_top_in_tag(ov, dy));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_original_band_non_spanning(
    band: &LayoutBand,
    dy: f64,
    dom: &FlowPageDom,
    svg_by_id: &BTreeMap<&str, &FlowSvgNode>,
    text_by_id: &BTreeMap<&str, &FlowTextNode>,
    cell_by_id: &BTreeMap<&str, &FlowCellNode>,
    svg_out: &mut String,
    text_out: &mut String,
    cells_out: &mut String,
) {
    for rid in &band.svg_ids {
        if let Some(node) = svg_by_id.get(rid.as_str()) {
            emit_svg_node_shifted(node, band, dy, 0.0, svg_out);
        }
    }
    for &sidx in &band.svg_indices {
        if let Some(node) = dom.svg_nodes.get(sidx) {
            emit_svg_node_shifted(node, band, dy, 0.0, svg_out);
        }
    }
    for tid in &band.text_ids {
        if let Some(t) = text_by_id.get(tid.as_str()) {
            text_out.push_str(&shift_top_in_tag(&t.raw_html, dy));
        }
    }
    for cid in &band.cell_ids {
        if let Some(c) = cell_by_id.get(cid.as_str()) {
            cells_out.push_str(&shift_top_in_tag(&c.raw_html, dy));
            for ov in &c.overlays {
                cells_out.push_str(&shift_top_in_tag(ov, dy));
            }
        }
    }
}

fn emit_svg_node_shifted(
    node: &FlowSvgNode,
    band: &LayoutBand,
    dy: f64,
    shrunk_pt: f64,
    svg_out: &mut String,
) {
    if let Some((x, y, w, h, ref fill)) = node.rect {
        let mut new_y = y + dy;
        let mut new_h = h;
        if shrunk_pt > 0.0 {
            if w <= 3.0 && h > 10.0 {
                // Vertical side border of elastic box: shorten by `shrunk_pt`.
                new_h = (h - shrunk_pt).max(1.0);
            } else if y > band.y0 + (band.y1 - band.y0) * 0.5 {
                // Bottom rule of elastic box: shift up by `shrunk_pt`.
                new_y -= shrunk_pt;
            }
        }
        let id_attr = node
            .rule_id
            .as_deref()
            .map(|id| format!(" data-rule-id=\"{id}\""))
            .unwrap_or_default();
        svg_out.push_str(&format!(
            "<g class=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\"{id_attr}/></g>",
            node.layer,
            fmt_pt(x),
            fmt_pt(new_y),
            fmt_pt(w),
            fmt_pt(new_h),
        ));
    } else if dy.abs() > 1e-4 {
        svg_out.push_str(&format!(
            "<g class=\"{}\" transform=\"translate(0,{})\">{}</g>",
            node.layer,
            fmt_pt(dy),
            node.raw_tag,
        ));
    } else {
        svg_out.push_str(&format!("<g class=\"{}\">{}</g>", node.layer, node.raw_tag,));
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_spanning_rules(
    band: &LayoutBand,
    _dy: f64,
    rows_top_y: f64,
    rows_bottom_y: f64,
    head_precedes_height: f64,
    total_follows_height: f64,
    continues_next: bool,
    svg_by_id: &BTreeMap<&str, &FlowSvgNode>,
    svg_out: &mut String,
) {
    for rid in &band.spanning_svg_ids {
        let Some(node) = svg_by_id.get(rid.as_str()) else {
            continue;
        };
        let Some((x, orig_y, w, orig_h, ref fill)) = node.rect else {
            continue;
        };
        let orig_bottom = orig_y + orig_h;
        // Did this rule originally start above `band.y0` (in `table_head`)?
        let starts_in_head = orig_y < band.y0 - 2.0 && head_precedes_height > 0.0;
        let top_y = if starts_in_head {
            (rows_top_y - (band.y0 - orig_y)).max(rows_top_y - head_precedes_height)
        } else {
            rows_top_y + (orig_y - band.y0).max(0.0)
        };
        // Did this rule originally extend below `band.y1` (into `total_row`)?
        let extends_into_total = orig_bottom > band.y1 + 2.0;
        let bottom_y = if !continues_next && extends_into_total && total_follows_height > 0.0 {
            rows_bottom_y + (orig_bottom - band.y1).min(total_follows_height)
        } else {
            rows_bottom_y
        };
        let new_h = (bottom_y - top_y).max(0.5);
        svg_out.push_str(&format!(
            "<g class=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" data-rule-id=\"{rid}\"/></g>",
            node.layer,
            fmt_pt(x),
            fmt_pt(top_y),
            fmt_pt(w),
            fmt_pt(new_h),
        ));
    }
}

fn emit_extra_row(
    er: &RenderedExtraRow,
    row_y: f64,
    svg_out: &mut String,
    text_out: &mut String,
    cells_out: &mut String,
) {
    if !er.svg_decorative.is_empty() {
        let mut buf = String::new();
        for r in &er.svg_decorative {
            push_band_rect(&mut buf, r, row_y);
        }
        svg_out.push_str(&format!("<g class=\"layer-decorative\">{buf}</g>"));
    }
    if !er.svg_structural.is_empty() {
        let mut buf = String::new();
        for r in &er.svg_structural {
            push_band_rect(&mut buf, r, row_y);
        }
        svg_out.push_str(&format!("<g class=\"layer-structural\">{buf}</g>"));
    }
    for t in &er.texts {
        text_out.push_str(&shift_top_in_tag(t, row_y));
    }
    for c in &er.cells {
        cells_out.push_str(&shift_top_in_tag(c, row_y));
    }
}

fn emit_break_closing_and_cue(
    table_x0: f64,
    table_x1: f64,
    y: f64,
    svg_out: &mut String,
    text_out: &mut String,
) {
    let w = (table_x1 - table_x0).max(20.0);
    svg_out.push_str(&format!(
        "<g class=\"layer-structural\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1.2\" fill=\"#000000\"/></g>",
        fmt_pt(table_x0),
        fmt_pt(y),
        fmt_pt(w),
    ));
    text_out.push_str(&format!(
        "<div class=\"t band-break-cue\" style=\"left:{}pt;top:{}pt;width:{}pt;text-align:right;font-family:&quot;eBIRForms Arimo&quot;,Arimo,Arial,sans-serif;font-size:7.5pt;font-weight:700;font-style:italic;color:#333333\">Continued on the next page</div>",
        fmt_pt(table_x0),
        fmt_pt(y + 3.0),
        fmt_pt(w - 4.0),
    ));
}

fn emit_rule6_line(dest_y: f64, ins: &Rule6Insertion, svg_out: &mut String, text_out: &mut String) {
    let w = (ins.field.x1 - ins.field.x0).max(40.0);
    let h = ins.height_pt;
    svg_out.push_str(&format!(
        "<g class=\"layer-structural\">\
         <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"0.48\" fill=\"#000000\"/>\
         <rect x=\"{x}\" y=\"{yb}\" width=\"{w}\" height=\"0.48\" fill=\"#000000\"/>\
         <rect x=\"{x}\" y=\"{y}\" width=\"0.48\" height=\"{h}\" fill=\"#000000\"/>\
         <rect x=\"{xr}\" y=\"{y}\" width=\"0.48\" height=\"{h}\" fill=\"#000000\"/>\
         </g>",
        x = fmt_pt(ins.field.x0),
        y = fmt_pt(dest_y),
        yb = fmt_pt(dest_y + h - 0.48),
        xr = fmt_pt(ins.field.x0 + w - 0.48),
        w = fmt_pt(w),
        h = fmt_pt(h),
    ));
    text_out.push_str(&format!(
        "<div class=\"t rule6-inserted-line\" style=\"left:{}pt;top:{}pt;width:{}pt;font-family:&quot;eBIRForms Arimo&quot;,Arimo,Arial,sans-serif;font-size:7.5pt;line-height:10pt;color:#000000;white-space:normal;word-break:break-word\">{}</div>",
        fmt_pt(ins.field.x0 + 4.0),
        fmt_pt(dest_y + 3.0),
        fmt_pt(w - 8.0),
        escape_html(&ins.text),
    ));
}

fn set_left_in_style(elem: &str, new_left: f64) -> String {
    if let Some(gt) = elem.find('>') {
        let open = &elem[..gt];
        if let Some(pos) = open.find("left:") {
            let after = &open[pos + 5..];
            if let Some(pt_pos) = after.find("pt") {
                return format!(
                    "{}left:{}pt{}{}",
                    &open[..pos],
                    fmt_pt(new_left),
                    &after[pt_pos + 2..],
                    &elem[gt..]
                );
            }
        }
    }
    elem.to_string()
}

// ---------------------------------------------------------------------------
// Tests (conservation invariant, in-place growth, page breaking, Rule 6,
// cell-owned overlays, elastic boxes)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuation::detect_overflow;

    /// Conservation verifier (Requirement #3):
    /// - Every non-repeating `text_id`, `cell_id`, and non-spanning `svg_id`
    ///   of the source page appears **exactly once** across the output pages.
    /// - Every `repeat_on_break` band's `text_id`, `cell_id`, and `svg_id`
    ///   appears **once per page that band opens**.
    /// - Every shifted element lies inside `[0, content_bottom_y]` (or
    ///   `[0, height_pt]` for untouched page headers).
    fn assert_conservation(
        slug: &str,
        raw_html: &str,
        flowed_doc: &str,
        page_num: usize,
        label: &str,
    ) {
        let raw_dom = parse_flow_page_dom(raw_html, page_num).expect("parse raw page");
        let fb = parse_formgen_bands(raw_html);
        let page_fb: Vec<&FormgenBand> = fb.iter().filter(|b| b.page == page_num).collect();
        let layout = parse_page_layout(raw_html)
            .and_then(|d| d.pages.into_iter().find(|p| p.page == page_num))
            .or_else(|| derive_page_layout(&raw_dom, &page_fb))
            .expect("layout for page");

        // Collect all output pages spawned from `page_num` (`page-<N>` and `page-<N>-cont-*`).
        let mut out_pages: Vec<FlowPageDom> = Vec::new();
        out_pages.push(parse_flow_page_dom(flowed_doc, page_num).expect("parse flowed base page"));
        let mut k = 1usize;
        while flowed_doc.contains(&format!("class=\"page page-{page_num}-cont-{k}\"")) {
            let marker = format!("class=\"page page-{page_num}-cont-{k}\"");
            let pos = flowed_doc.find(&marker).unwrap();
            let div_start = flowed_doc[..pos].rfind("<div").unwrap();
            let div_end = find_matching_div_end(flowed_doc, div_start).unwrap();
            let fake_doc = flowed_doc[div_start..div_end].replacen(
                &marker,
                &format!("class=\"page page-{page_num}\""),
                1,
            );
            out_pages.push(parse_flow_page_dom(&fake_doc, page_num).expect("parse cont page"));
            k += 1;
        }

        let mut text_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut cell_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut svg_id_counts: BTreeMap<String, usize> = BTreeMap::new();

        for (pi, op) in out_pages.iter().enumerate() {
            for t in &op.texts {
                if let Some(id) = &t.id {
                    *text_counts.entry(id.clone()).or_default() += 1;
                }
                assert!(
                    t.top >= -1.0 && t.top <= layout.content_bottom_y + 12.0,
                    "{slug} ({label}) page idx {pi}: text {:?} top={} outside [0, {}]",
                    t.id,
                    t.top,
                    layout.content_bottom_y
                );
            }
            for c in &op.cells {
                if !c.id.starts_with("bf_") {
                    *cell_counts.entry(c.id.clone()).or_default() += 1;
                }
                assert!(
                    c.top >= -1.0 && c.top + c.height <= layout.content_bottom_y + 6.0,
                    "{slug} ({label}) page idx {pi}: cell {} [{}..{}] outside [0, {}]",
                    c.id,
                    c.top,
                    c.top + c.height,
                    layout.content_bottom_y
                );
            }
            for s in &op.svg_nodes {
                if let Some(rid) = &s.rule_id {
                    *svg_id_counts.entry(rid.clone()).or_default() += 1;
                }
                if let Some((_, y, _, h, _)) = s.rect {
                    assert!(
                        y >= -1.0 && y + h <= layout.content_bottom_y + 6.0,
                        "{slug} ({label}) page idx {pi}: svg {:?} [{}..{}] outside [0, {}]",
                        s.rule_id,
                        y,
                        y + h,
                        layout.content_bottom_y
                    );
                }
            }
        }

        let page_count = out_pages.len();
        let mut spanning_ids: BTreeSet<&str> = BTreeSet::new();
        for b in &layout.bands {
            for sid in &b.spanning_svg_ids {
                spanning_ids.insert(sid.as_str());
            }
        }

        for b in &layout.bands {
            for tid in &b.text_ids {
                let got = text_counts.get(tid).copied().unwrap_or(0);
                if b.kind == BandKind::PageHeader {
                    assert_eq!(
                        got, page_count,
                        "{slug} ({label}): page_header text {tid} expected {page_count}, got {got}"
                    );
                } else if b.repeat_on_break {
                    assert!(
                        (1..=page_count).contains(&got),
                        "{slug} ({label}): repeating text {tid} expected 1..={page_count}, got {got}"
                    );
                } else {
                    assert_eq!(
                        got, 1,
                        "{slug} ({label}): non-repeating text {tid} expected 1, got {got}"
                    );
                }
            }
            for cid in &b.cell_ids {
                let got = cell_counts.get(cid).copied().unwrap_or(0);
                if b.kind == BandKind::PageHeader {
                    assert_eq!(
                        got, page_count,
                        "{slug} ({label}): page_header cell {cid} expected {page_count}, got {got}"
                    );
                } else if b.repeat_on_break {
                    assert!(
                        (1..=page_count).contains(&got),
                        "{slug} ({label}): repeating cell {cid} expected 1..={page_count}, got {got}"
                    );
                } else {
                    assert_eq!(
                        got, 1,
                        "{slug} ({label}): non-repeating cell {cid} expected 1, got {got}"
                    );
                }
            }
            for sid in &b.svg_ids {
                if spanning_ids.contains(sid.as_str()) {
                    continue;
                }
                let got = svg_id_counts.get(sid).copied().unwrap_or(0);
                if b.kind == BandKind::PageHeader {
                    assert_eq!(
                        got, page_count,
                        "{slug} ({label}): page_header svg {sid} expected {page_count}, got {got}"
                    );
                } else if b.repeat_on_break {
                    assert!(
                        (1..=page_count).contains(&got),
                        "{slug} ({label}): repeating svg {sid} expected 1..={page_count}, got {got}"
                    );
                } else {
                    assert_eq!(
                        got, 1,
                        "{slug} ({label}): non-repeating svg {sid} expected 1, got {got}"
                    );
                }
            }
        }
    }

    fn build_1707_sched3_fields(total_rows: usize) -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        f.insert("frm1707:txtTIN1".into(), "000".into());
        f.insert("frm1707:txtTIN2".into(), "123".into());
        f.insert("frm1707:txtTIN3".into(), "453".into());
        f.insert("frm1707:txtBranchCode".into(), "00000".into());
        f.insert("frm1707:txtTaxpayerName".into(), "SANTOS, JUANA".into());
        let printed = total_rows.min(4);
        for n in 1..=printed {
            f.insert(
                format!("frm1707:txtPg2P4Sch3Particulars{n}"),
                format!("EXPENSE ROW {n}"),
            );
            f.insert(
                format!("frm1707:txtPg2P4Sch3Amount{n}"),
                format!("{}.00", n * 10000),
            );
        }
        for k in 1..=total_rows.saturating_sub(4) {
            f.insert(
                format!("frm1707:txtPg2Pt4S3_{k}Col1"),
                format!("EXTRA EXPENSE {k}"),
            );
            f.insert(
                format!("frm1707:txtPg2Pt4S3_{k}Col2"),
                format!("{}.00", k * 5000),
            );
        }
        f.insert("frm1707:txtPg2P4Sch3TotAmount".into(), "999999.00".into());
        f
    }

    #[test]
    fn form_1707_sched3_grows_in_place_for_7_rows_and_breaks_cleanly_for_18_rows() {
        let slug = "1707-2021";
        let raw_html = frozen_html::raw_bundle_html(slug).expect("1707 bundle");

        // 7 rows (3 extra): fits in place on Page 2 — NO Page 2 (continued) spawned!
        let f7 = build_1707_sched3_fields(7);
        let doc7 = frozen_html::filled_document(slug, &f7).expect("fill 7 rows");
        assert!(
            !doc7.contains("page-2-cont-1"),
            "7 rows of 1707 Schedule 3 must fit on Page 2 without spawning a continuation page"
        );
        // Particulars cells are comb-slotted (`E`, `X`, `T`, ...); verify via `printed_cell`.
        assert!(
            frozen_html::printed_cell(&doc7, "bf_p2_p2g1_r0_p2c76").0 == "EXTRA EXPENSE 1"
                || doc7.contains("bf_p2_p2g1_r0_"),
            "7 rows of 1707 Schedule 3 must render cloned extra rows on Page 2"
        );
        assert!(
            doc7.contains("bf_p2_p2g1_r2_"),
            "all 3 extra rows (E, F, G) must appear on Page 2"
        );
        assert_conservation(slug, raw_html, &doc7, 2, "7 rows in-place");

        // 22 rows (18 extra): forces a mid-table break onto `Page 2 (continued)`!
        let f22 = build_1707_sched3_fields(22);
        let doc22 = frozen_html::filled_document(slug, &f22).expect("fill 22 rows");
        assert!(
            doc22.contains("class=\"page page-2-cont-1\""),
            "22 rows of 1707 Schedule 3 must spawn page-2-cont-1"
        );
        assert!(
            doc22.contains("Page 2 (continued)"),
            "spawned page must use full 'Page 2 (continued)', never '(cont.)'"
        );
        assert!(
            !doc22.contains("(cont.)"),
            "must never use abbreviated '(cont.)'"
        );
        assert!(
            doc22.contains("Continued on the next page"),
            "breaking page must tell the reader 'Continued on the next page'"
        );
        assert!(
            doc22.contains("bf_p2_p2g1_r17_"),
            "18th extra row (row V) must appear on Page 2 (continued)"
        );
        assert_conservation(slug, raw_html, &doc22, 2, "22 rows page-break");
    }

    #[test]
    fn conservation_holds_for_0_capacity_plus1_and_plus40_rows_on_1707() {
        let slug = "1707-2021";
        let raw_html = frozen_html::raw_bundle_html(slug).expect("1707 bundle");
        for &count in &[0usize, 4, 5, 44] {
            let fields = build_1707_sched3_fields(count);
            let doc = frozen_html::filled_document(slug, &fields).expect("fill");
            assert_conservation(slug, raw_html, &doc, 2, &format!("{count} rows"));
        }
    }

    #[test]
    fn rule6_inserted_line_and_cell_overlay_shift_cleanly() {
        let html = r##"<html><body><div class="page page-1" id="page-1" style="width:612pt;height:936pt"><svg class="rl" viewBox="0 0 612 936" style="width:612pt;height:936pt"><g class="layer-structural"><rect x="18" y="40" width="576" height="1" fill="#000" data-rule-id="h0"/><rect x="18" y="200" width="576" height="1" fill="#000" data-rule-id="h1"/><rect x="18" y="260" width="576" height="1" fill="#000" data-rule-id="h2"/></g></svg><div class="layer-text"><div class="t" id="p1t0" style="left:20pt;top:50pt">Page 1</div><div class="t" id="p1t1" style="left:20pt;top:210pt">Item 17</div><div class="t" id="p1t2" style="left:20pt;top:270pt">Below</div></div><div class="layer-cells"><div id="p1c1" class="c" style="left:200pt;top:210pt;width:100pt;height:14pt"></div><div id="p1c2" class="c" style="left:200pt;top:270pt;width:100pt;height:14pt"></div><input type="text" class="fi" data-overflow-of="p1c2" style="left:200pt;top:270pt;width:120pt;height:14pt" value="OVERLAY"></div></div></body></html>"##;
        let dom = parse_flow_page_dom(html, 1).expect("parse");
        let layout = PageLayout {
            page: 1,
            width_pt: 612.0,
            height_pt: 936.0,
            content_bottom_y: 918.0,
            table_x0: 18.0,
            table_x1: 594.0,
            bands: vec![
                LayoutBand {
                    id: "p1b0".into(),
                    kind: BandKind::PageHeader,
                    y0: 0.0,
                    y1: 100.0,
                    keep_with_next: false,
                    keep_with_prev: false,
                    repeat_on_break: true,
                    min_height_pt: None,
                    page_label_text_id: Some("p1t0".into()),
                    growable_id: None,
                    table_group: None,
                    svg_ids: vec!["h0".into()],
                    svg_indices: vec![],
                    text_ids: vec!["p1t0".into()],
                    cell_ids: vec![],
                    spanning_svg_ids: vec![],
                },
                LayoutBand {
                    id: "p1b1".into(),
                    kind: BandKind::PlainBlock,
                    y0: 100.0,
                    y1: 240.0,
                    keep_with_next: false,
                    keep_with_prev: false,
                    repeat_on_break: false,
                    min_height_pt: None,
                    page_label_text_id: None,
                    growable_id: None,
                    table_group: None,
                    svg_ids: vec!["h1".into()],
                    svg_indices: vec![],
                    text_ids: vec!["p1t1".into()],
                    cell_ids: vec!["p1c1".into()],
                    spanning_svg_ids: vec![],
                },
                LayoutBand {
                    id: "p1b2".into(),
                    kind: BandKind::PlainBlock,
                    y0: 240.0,
                    y1: 300.0,
                    keep_with_next: false,
                    keep_with_prev: false,
                    repeat_on_break: false,
                    min_height_pt: None,
                    page_label_text_id: None,
                    growable_id: None,
                    table_group: None,
                    svg_ids: vec!["h2".into()],
                    svg_indices: vec![],
                    text_ids: vec!["p1t2".into()],
                    cell_ids: vec!["p1c2".into()],
                    spanning_svg_ids: vec![],
                },
            ],
            rule6_fields: vec![],
        };
        let ins = Rule6Insertion::from_text(
            Rule6Field {
                writer_key: "txtSpecify".into(),
                cell_id: "p1c1".into(),
                band_id: "p1b1".into(),
                insert_after_y: 240.0,
                x0: 18.0,
                x1: 594.0,
            },
            "Creditable withholding tax per BIR Form 2307 from Q1-Q3".into(),
        );
        let dy = ins.height_pt;
        let out = flow_single_page(
            "1707-2021",
            html,
            &dom,
            &layout,
            &[],
            &BTreeMap::new(),
            &[ins],
        )
        .expect("flow rule6");
        assert!(out.contains("rule6-inserted-line"));
        let expected_top = format!("top:{}pt", fmt_pt(270.0 + dy));
        assert!(
            out.contains(&expected_top),
            "both p1c2 and its data-overflow-of overlay must shift down by {dy}pt"
        );
        // Both p1c2 and its sibling overlay must have shifted to `270 + dy`.
        assert_eq!(out.matches(&expected_top).count(), 3); // p1t2 + p1c2 + overlay
    }

    #[test]
    fn detect_overflow_still_reports_for_band_flow_forms() {
        let fields = build_1707_sched3_fields(7);
        let ov = detect_overflow("1707-2021", &fields).expect("detect");
        assert_eq!(ov.schedules.len(), 1);
        assert_eq!(ov.schedules[0].rows.len(), 3);
    }

    #[test]
    #[ignore]
    fn band_flow_sweep_all_forms() {
        let out_root = std::path::PathBuf::from("/tmp/band-flow-sweep");
        let _ = std::fs::create_dir_all(&out_root);
        let order = [
            "1801-2018",
            "1600wp-2010",
            "2550q-2024",
            "1701a-2018",
            "2000-ot-2018",
            "1800-2018",
            "2552-2018",
            "2551q-2018",
            "1601c-2018",
            "2000-dst-2018",
            "2200m-2018",
            "2200c-2018",
            "2550m-2007",
            "1600-pt-2018",
            "1600-vt-2018",
            "1601-fq-2020",
            "1601eq-2019",
            "1602q-2019",
            "1603q-2018",
            "1700-2018",
            "1702ex-2018",
            "1702mx-2018c",
            "1702rt-2018c",
            "1706-2018",
            "1707-2021",
            "1707a-2021",
            "2200a-2020",
            "2200an-2018",
            "2200p-2020",
            "2200s-2018",
            "2200t-2022",
        ];
        let mut results: Vec<serde_json::Value> = Vec::new();
        for slug in order {
            let Some(raw) = frozen_html::raw_bundle_html(slug) else {
                continue;
            };
            let fb = parse_formgen_bands(raw);
            let fb_pages: BTreeSet<usize> = fb.iter().map(|b| b.page).collect();
            let slug_dir = out_root.join(slug);
            let _ = std::fs::create_dir_all(&slug_dir);

            let mut form_cases = serde_json::Map::new();
            for (tag, extra) in [("cap", 0usize), ("cap_p3", 3usize), ("cap_p40", 40usize)] {
                let Some((fields, expected)) =
                    crate::continuation::tests::build_sweep_fields_for_slug(slug, extra)
                else {
                    form_cases.insert(tag.into(), serde_json::json!({"skipped": "no_schedules"}));
                    continue;
                };
                match frozen_html::filled_document(slug, &fields) {
                    Err(e) => {
                        form_cases.insert(tag.into(), serde_json::json!({"error": e}));
                    }
                    Ok(doc) => {
                        let html_path = slug_dir.join(format!("{tag}.html"));
                        let _ = std::fs::write(&html_path, &doc);
                        let has_bf_cont = doc.contains("-cont-1\"");
                        let has_old_cont = doc.contains("continuation-page");
                        let has_bf_rows = doc.contains("bf_");
                        let page_count = doc.matches("class=\"page ").count();

                        // Missing expected values check
                        let corpus_up = doc.to_ascii_uppercase();
                        let corpus_nc = corpus_up.replace(',', "");
                        let mut missing = Vec::new();
                        for (sh, rn, ch, val, is_amt) in &expected {
                            let up = val.trim().to_ascii_uppercase();
                            let found = up.is_empty()
                                || corpus_up.contains(&up)
                                || (*is_amt && {
                                    let st = up.replace(',', "");
                                    let w = st.split('.').next().unwrap_or(&st);
                                    !w.is_empty() && corpus_nc.contains(w)
                                });
                            if !found && missing.len() < 8 {
                                missing.push(format!("{sh} r{rn} [{ch}]={val}"));
                            }
                        }

                        // Conservation check on every page with formgen-bands
                        let mut cons_errs = Vec::new();
                        for &pnum in &fb_pages {
                            let res =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    assert_conservation(slug, raw, &doc, pnum, tag);
                                }));
                            if let Err(payload) = res {
                                let msg = if let Some(s) = payload.downcast_ref::<String>() {
                                    s.clone()
                                } else if let Some(s) = payload.downcast_ref::<&str>() {
                                    (*s).to_string()
                                } else {
                                    "panic".to_string()
                                };
                                cons_errs.push(format!("p{pnum}: {msg}"));
                            }
                        }
                        form_cases.insert(
                            tag.into(),
                            serde_json::json!({
                                "page_count": page_count,
                                "band_flow_rows": has_bf_rows,
                                "band_flow_cont": has_bf_cont,
                                "fallback_cont": has_old_cont,
                                "missing_count": missing.len(),
                                "missing_sample": missing,
                                "conservation_errors": cons_errs,
                            }),
                        );
                    }
                }
            }
            println!(
                "SWEEP {slug:14} fb={} | {}",
                fb.len(),
                serde_json::to_string(&form_cases).unwrap()
            );
            results.push(serde_json::json!({
                "slug": slug,
                "formgen_bands_count": fb.len(),
                "fb_pages": fb_pages.into_iter().collect::<Vec<_>>(),
                "cases": form_cases,
            }));
        }
        let _ = std::fs::write(
            out_root.join("summary.json"),
            serde_json::to_string_pretty(&results).unwrap(),
        );
    }
}
