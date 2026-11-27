//! Every value cell of every frozen sheet lies on the box the sheet draws
//! for it: not on a caption, a shaded banner or blank paper.
//!
//! The sheet's drawing is its SVG layer: black rules (`layer-structural`,
//! `layer-band`) and the grey and white fills (`layer-decorative`,
//! `layer-knockout`) whose edges outline boxes and comb slots. A value cell
//! (any cell something prints into: joined to a writer key, or holding an
//! input stamped with a filing key; a drawn `<div>` with its own `id`,
//! outside any `<template>`) passes when
//!
//! - each of its four edges lies within [`TOL`] of a drawn edge running along
//!   at least half of that side, and
//! - for a comb, at least [`TICKS`] of the boundaries between its slots lie
//!   on a drawn vertical edge (a tick or a fill's side) beside the cell.
//!
//! Hand corrections draw some boxes in the cells themselves: a cell with a
//! fill and an inset border (the TIN rows of 0605, 1600WP, 2550M and 2553),
//! whose slots draw their ticks as a gradient at their right. Such a
//! self-boxed cell is its own box for its sides and slots, but its top and
//! bottom must still lie on a printed horizontal that its row spans: the
//! self-drawn boxes on that line (its groups and the dash cells between
//! them) tile it end to end, each box meeting the next and the first and
//! last meeting the line's ends, all within [`TOL`]. A group shrunk inside
//! its row leaves the printed row showing beside it and fails. Its own edges
//! and ticks are no drawing for any other cell.
//!
//! A hand-placed cell a few points off its box fails both. [`KNOWN`] lists
//! the cells a little off whose values were seen to print inside their box;
//! the test fails on any other cell, and on a listed cell that now passes, so
//! the list stays exact.

use std::collections::BTreeSet;
use std::path::PathBuf;

const TOL: f64 = 2.0;
const TICKS: f64 = 0.6;

/// Value cells a little off their drawn boxes whose values still print fully
/// inside the box (rendered and looked at), each with its measurement (pt).
const KNOWN: &[(&str, &str)] = &[
    // 8A If yes, please specify (frm1707:txtPg1I8TaxReliefSpec): top 2.6 pt
    // below the line under the caption, inside the white box; a 25-character
    // value and a 58-character one both print inside it
    // (docs/proof/value-cells-on-boxes/1707-item8a-specify.png).
    ("1707-2021", "p1c217"),
];

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A cell that draws its own box (any kind): its id and box.
struct Tile {
    id: String,
    rect: Rect,
}

/// A drawn edge: its position across, half its thickness and its extent along.
struct Edge {
    at: f64,
    half: f64,
    from: f64,
    to: f64,
}

struct Cell {
    id: String,
    comb: bool,
    rect: Rect,
    /// Absolute left edge of every slot of a comb.
    slot_lefts: Vec<f64>,
    /// The cell draws its own box (a fill with an inset border).
    self_boxed: bool,
    /// Ticks its own slots draw (a gradient at a slot's right), absolute x.
    own_ticks: Vec<f64>,
}

fn frozen_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../html-frozen")
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    // The attribute itself, not one whose name ends in `name` (data-cell-id).
    let needle = format!("{name}=\"");
    let mut from = 0;
    while let Some(rel) = tag[from..].find(&needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && !tag[..at].ends_with([' ', '\t', '\n']) {
            continue;
        }
        let rest = &tag[from..];
        return rest.find('"').map(|end| &rest[..end]);
    }
    None
}

fn pt(style: &str, key: &str) -> Option<f64> {
    let needle = format!("{key}:");
    let mut from = 0;
    while let Some(rel) = style[from..].find(&needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && !style[..at].ends_with([';', ' ']) {
            continue;
        }
        return style[from..].split("pt").next()?.trim().parse().ok();
    }
    None
}

fn num(tag: &str, name: &str) -> Option<f64> {
    attr(tag, name)?.parse().ok()
}

/// The sheet without its `<template>` contents (band rows kept for cloning,
/// never drawn).
fn drawn(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find("<template") {
        out.push_str(&rest[..open]);
        match rest[open..].find("</template>") {
            Some(close) => rest = &rest[open + close + "</template>".len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A page's horizontal and vertical drawn edges, value cells and
/// self-boxed cells.
type Page = (Vec<Edge>, Vec<Edge>, Vec<Cell>, Vec<Tile>);

/// Each page's drawn edges, value cells and self-boxed cells.
fn pages(html: &str, joined: &BTreeSet<String>) -> Vec<Page> {
    let html = drawn(html);
    let marker = "<div class=\"page page-";
    let mut starts: Vec<usize> = html.match_indices(marker).map(|(at, _)| at).collect();
    starts.push(html.len());
    starts
        .windows(2)
        .map(|w| page(&html[w[0]..w[1]], joined))
        .collect()
}

fn page(body: &str, joined: &BTreeSet<String>) -> Page {
    let mut across = Vec::new(); // horizontal edges: at = y
    let mut down = Vec::new(); // vertical edges: at = x
    if let (Some(open), Some(close)) = (body.find("<svg"), body.find("</svg>")) {
        let svg = &body[open..close];
        let mut layer = "";
        let mut from = 0;
        while let Some(rel) = svg[from..].find('<') {
            let at = from + rel;
            let Some(end) = svg[at..].find('>') else {
                break;
            };
            let tag = &svg[at..=at + end];
            from = at + end + 1;
            if tag.starts_with("<g") {
                layer = attr(tag, "class").unwrap_or("");
            } else if tag.starts_with("<rect") {
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    num(tag, "x"),
                    num(tag, "y"),
                    num(tag, "width"),
                    num(tag, "height"),
                ) else {
                    continue;
                };
                if matches!(layer, "layer-structural" | "layer-band") {
                    if w >= h {
                        across.push(Edge {
                            at: y + h / 2.0,
                            half: h / 2.0,
                            from: x,
                            to: x + w,
                        });
                    } else {
                        down.push(Edge {
                            at: x + w / 2.0,
                            half: w / 2.0,
                            from: y,
                            to: y + h,
                        });
                    }
                } else {
                    for at in [y, y + h] {
                        across.push(Edge {
                            at,
                            half: 0.0,
                            from: x,
                            to: x + w,
                        });
                    }
                    for at in [x, x + w] {
                        down.push(Edge {
                            at,
                            half: 0.0,
                            from: y,
                            to: y + h,
                        });
                    }
                }
            }
        }
    }
    let mut cells: Vec<Cell> = Vec::new();
    let mut tiles: Vec<Tile> = Vec::new();
    // The comb whose slots follow (a slot belongs to the cell just opened).
    let mut comb: Option<usize> = None;
    let mut from = 0;
    while let Some(rel) = body[from..].find("<div ") {
        let at = from + rel;
        let Some(end) = body[at..].find('>') else {
            break;
        };
        let tag = &body[at..at + end];
        from = at + end + 1;
        if tag.starts_with("<div class=\"s\"") {
            if let (Some(index), Some(style)) = (comb, attr(tag, "style")) {
                if let Some(left) = pt(style, "left") {
                    let cell = &mut cells[index];
                    cell.slot_lefts.push(cell.rect.x + left);
                    // A slot that draws its own tick (a gradient at its right):
                    // it vouches for this comb's slots only.
                    if let (true, Some(width)) = (
                        style.contains("linear-gradient") && style.contains("right"),
                        pt(style, "width"),
                    ) {
                        cell.own_ticks.push(cell.rect.x + left + width);
                    }
                }
            }
            continue;
        }
        comb = None;
        let Some(style) = attr(tag, "style") else {
            continue;
        };
        let (Some(x), Some(y), Some(w), Some(h)) = (
            pt(style, "left"),
            pt(style, "top"),
            pt(style, "width"),
            pt(style, "height"),
        ) else {
            continue;
        };
        // A cell that draws its own box (a fill with an inset border, the
        // sheets' hand corrections of a TIN row) is its own box for its sides
        // and slots, but must still sit on the printed row (below); its edges
        // are no drawing for any other cell.
        let self_boxed = style.contains("background:") && style.contains("box-shadow:inset");
        // A value cell is a cell something prints into, whatever its kind
        // (`field`, `mixed` caption-and-boxes, even `label`): a cell a writer
        // key is joined to (`joined_cell_ids`, the engine's own joins
        // included), or one holding an input stamped with a filing key (its
        // name is not the cell's id), which the print fills by name. A cell
        // with no input, or whose inputs nothing fills (named after the cell
        // and joined to nothing), prints nothing and is skipped.
        if attr(tag, "data-cell-kind").is_none() {
            continue;
        }
        let Some(id) = attr(tag, "id") else {
            continue;
        };
        if self_boxed {
            tiles.push(Tile {
                id: id.to_string(),
                rect: Rect { x, y, w, h },
            });
        }
        let rest = &body[from..];
        let own = &rest[..rest.find("data-cell-kind=").unwrap_or(rest.len())];
        let mut inputs = own.match_indices("<input").map(|(at, _)| {
            let tag = &own[at..own[at..].find('>').map_or(own.len(), |end| at + end)];
            attr(tag, "name").unwrap_or(id).to_string()
        });
        let printed = if joined.contains(id) {
            own.contains("<input")
        } else {
            inputs.any(|name| name != id)
        };
        if !printed {
            continue;
        }
        let is_comb = attr(tag, "data-field-kind") == Some("comb");
        if is_comb {
            comb = Some(cells.len());
        }
        cells.push(Cell {
            id: id.to_string(),
            comb: is_comb,
            rect: Rect { x, y, w, h },
            slot_lefts: Vec::new(),
            self_boxed,
            own_ticks: Vec::new(),
        });
    }
    (across, down, cells, tiles)
}

fn overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Whether the self-boxed cells of `cell`'s row (`cell` as placed, the
/// others from `tiles`) tile the printed line `e` end to end: sorted along
/// it, each meets the line's start or the box before it, and the last meets
/// the line's end, all within [`TOL`].
fn row_spans(cell: &Cell, tiles: &[Tile], e: &Edge) -> bool {
    let Rect { y, h, .. } = cell.rect;
    let mut row: Vec<Rect> = tiles
        .iter()
        .filter(|tile| tile.id != cell.id)
        .map(|tile| tile.rect)
        .chain(std::iter::once(cell.rect))
        .filter(|r| {
            (r.y - y).abs() <= TOL
                && (r.y + r.h - (y + h)).abs() <= TOL
                && e.from - TOL <= r.x
                && r.x + r.w <= e.to + TOL
        })
        .collect();
    row.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut end = e.from;
    for r in &row {
        if (r.x - end).abs() > TOL {
            return false;
        }
        end = r.x + r.w;
    }
    (e.to - end).abs() <= TOL
}

/// What is wrong with `cell`'s placement, or nothing.
fn misplacement(cell: &Cell, across: &[Edge], down: &[Edge], tiles: &[Tile]) -> Option<String> {
    let Rect { x, y, w, h } = cell.rect;
    let (x1, y1) = (x + w, y + h);
    let on_across = |at: f64| {
        across.iter().any(|e| {
            (e.at - at).abs() <= TOL + e.half
                && if cell.self_boxed {
                    // A self-boxed cell's row spans the printed line.
                    row_spans(cell, tiles, e)
                } else {
                    overlap(x, x1, e.from, e.to) >= 0.5 * w
                }
        })
    };
    // A self-boxed cell's own sides are its box.
    let on_down = |at: f64| {
        cell.self_boxed
            || down.iter().any(|e| {
                (e.at - at).abs() <= TOL + e.half && overlap(y, y1, e.from, e.to) >= 0.5 * h
            })
    };
    let mut wrong: Vec<&str> = [
        ("top", on_across(y)),
        ("bottom", on_across(y1)),
        ("left", on_down(x)),
        ("right", on_down(x1)),
    ]
    .into_iter()
    .filter(|(_, ok)| !ok)
    .map(|(side, _)| side)
    .collect();
    if cell.comb && cell.slot_lefts.len() > 1 {
        let bounds = &cell.slot_lefts[1..];
        let hit = bounds
            .iter()
            .filter(|&&b| {
                cell.own_ticks.iter().any(|&t| (t - b).abs() <= 1.6)
                    || down.iter().any(|e| {
                        (e.at - b).abs() <= 1.6 + e.half
                            && overlap(y - 2.0, y1 + 2.0, e.from, e.to) > 0.0
                    })
            })
            .count();
        if (hit as f64) < TICKS * bounds.len() as f64 {
            wrong.push("slots");
        }
    }
    (!wrong.is_empty()).then(|| {
        format!(
            "{} at ({x:.2}, {y:.2}) {w:.2}x{h:.2}: {} off the drawing",
            cell.id,
            wrong.join(", ")
        )
    })
}

/// Every misplaced value cell of sheet `slug`.
fn misplaced(slug: &str) -> Vec<(String, String)> {
    let html = std::fs::read_to_string(frozen_root().join(slug).join("index.html")).unwrap();
    let mut out = Vec::new();
    let joined = bir_print::frozen_html::joined_cell_ids(slug);
    for (across, down, cells, tiles) in pages(&html, &joined) {
        for cell in &cells {
            if let Some(why) = misplacement(cell, &across, &down, &tiles) {
                out.push((cell.id.clone(), why));
            }
        }
    }
    out
}

fn sheets() -> Vec<String> {
    let mut slugs: Vec<String> = std::fs::read_dir(frozen_root())
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .join("index.html")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    slugs.sort();
    slugs
}

#[test]
fn every_value_cell_lies_on_its_drawn_box() {
    let known: BTreeSet<(&str, &str)> = KNOWN.iter().copied().collect();
    let mut new = Vec::new();
    let mut found = BTreeSet::new();
    let slugs = sheets();
    assert_eq!(slugs.len(), 43, "{slugs:?}");
    for slug in &slugs {
        for (id, why) in misplaced(slug) {
            if let Some(entry) = known.iter().find(|(s, c)| s == slug && *c == id) {
                found.insert(*entry);
            } else {
                new.push(format!("{slug} {why}"));
            }
        }
    }
    let fixed: Vec<_> = known.difference(&found).collect();
    assert!(
        new.is_empty(),
        "value cells off their drawn boxes ({}):\n{}",
        new.len(),
        new.join("\n")
    );
    assert!(
        fixed.is_empty(),
        "now on their boxes, drop them from KNOWN: {fixed:?}"
    );
}

/// The test catches a cell moved a few points off its box: 2200T's Amended
/// Return Yes box, 12 pt low as it once was, fails; on its box it passes.
#[test]
fn a_cell_a_few_points_off_its_box_fails() {
    let html = std::fs::read_to_string(frozen_root().join("2200t-2020/index.html")).unwrap();
    let joined = bir_print::frozen_html::joined_cell_ids("2200t-2020");
    let (across, down, cells, tiles) = pages(&html, &joined).into_iter().next().unwrap();
    let mut yes = cells.into_iter().find(|c| c.id == "p1c1030").unwrap();
    assert_eq!(misplacement(&yes, &across, &down, &tiles), None);
    yes.rect.x += 3.12;
    yes.rect.y += 11.7;
    assert!(misplacement(&yes, &across, &down, &tiles).is_some());
}

/// A self-boxed cell (0605's hand-drawn TIN 4-6 box) must still sit on the
/// printed row: on it, it passes; moved 4 pt down off the printed lines, its
/// own box does not save it.
#[test]
fn a_self_boxed_cell_off_the_printed_row_fails() {
    let html = std::fs::read_to_string(frozen_root().join("0605-1999/index.html")).unwrap();
    let joined = bir_print::frozen_html::joined_cell_ids("0605-1999");
    let (across, down, cells, tiles) = pages(&html, &joined).into_iter().next().unwrap();
    let mut tin = cells.into_iter().find(|c| c.id == "p1c25").unwrap();
    assert!(tin.self_boxed);
    assert_eq!(misplacement(&tin, &across, &down, &tiles), None);
    tin.rect.y += 4.0;
    assert!(misplacement(&tin, &across, &down, &tiles).is_some());
}

/// A self-boxed cell must span its printed row: 0605's TIN 4-6 group
/// (p1c25) shrunk to half its width still lies on the printed line, but
/// leaves the row showing between it and the dash cell after it, and fails;
/// in place, its row's seven boxes tile the line and it passes.
#[test]
fn a_self_boxed_cell_shrunk_inside_its_row_fails() {
    let html = std::fs::read_to_string(frozen_root().join("0605-1999/index.html")).unwrap();
    let joined = bir_print::frozen_html::joined_cell_ids("0605-1999");
    let (across, down, cells, tiles) = pages(&html, &joined).into_iter().next().unwrap();
    let mut tin = cells.into_iter().find(|c| c.id == "p1c25").unwrap();
    assert!(tin.self_boxed);
    assert_eq!(misplacement(&tin, &across, &down, &tiles), None);
    tin.rect.w /= 2.0;
    let why = misplacement(&tin, &across, &down, &tiles).unwrap();
    assert!(why.contains("top, bottom"), "{why}");
}
