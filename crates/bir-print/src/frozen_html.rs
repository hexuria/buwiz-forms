//! Frozen HTML fill/print: set `input[name]` from the writer map.
//!
//! Layout lives in `html-frozen/<slug>/`. `name=` is a fail-closed catalog
//! join; `id=` stays the cell id. Catalog `official_field_key` stamps are
//! TIN/branch only. Unstamped leftover keys keep cell-id `name=`;
//! `html-frozen/<slug>/writer-cells.json` may copy a writer value onto that
//! cell when the freeze sheet has a 1:1 printed-caption join (`joins`),
//! split a leftover `{:.2}` money key onto a catalog peso comb plus 2-slot
//! cents comb (`money_joins`), or mark a catalog xbox when a leftover
//! boolean/radio writer is `"true"` (`xbox_joins`, glyph `X`). Text `joins`
//! ASCII-uppercase letters into comb slots and `data-writer-value` so BIR
//! CAPITAL LETTERS print matches the form instruction; digits, punctuation,
//! money, and xbox are unchanged. Profile/DB values stay as stored. Those are
//! fill-paths, not `official_field_key` harvests. Do not stamp `name=` on
//! peso/cent/xbox boxes, do not left-align a dotted money string into the
//! peso comb, and do not write `"true"` into an xbox.
//!
//! One writer key may print in several cells (`html_ids` beside `html_id`;
//! `peso_html_ids`/`cent_html_ids` and `decimal_html_ids` for money), so a
//! name, TIN or total repeats in the later pages' headers and schedules.
//! A money comb whose last two slots are the centavos is a
//! `decimal_html_id` (`point_slot` when the slot before them carries the
//! printed point). `date_joins` split one writer date (`MM/DD/YYYY`,
//! `MM/YYYY`, ISO) over separate MM, DD and YYYY combs. An xbox join may
//! name one box of a two-box cell by its input id (`<cell>-i<n>`).
//! `not_joined` lists, with a reason, each writer key the sheet does not
//! print, and `not_joined_cells` each cell no key fills: [`unprinted_keys`]
//! is the coverage gate over both.

use bir_core::forms::form_2551q::Form2551QDraft;
use std::collections::{BTreeMap, BTreeSet};

include!(concat!(env!("OUT_DIR"), "/frozen_bundles.rs"));

const BASE_CSS: &str = include_str!("../../../html-frozen/base.css");
const FONT_ARIMO_NORMAL: &[u8] =
    include_bytes!("../../../html-frozen/fonts/arimo-latin-wght-normal.woff2");
const FONT_ARIMO_ITALIC: &[u8] =
    include_bytes!("../../../html-frozen/fonts/arimo-latin-wght-italic.woff2");
const FONT_TINOS_NORMAL: &[u8] =
    include_bytes!("../../../html-frozen/fonts/tinos-latin-400-normal.woff2");
const FONT_TINOS_BOLD: &[u8] =
    include_bytes!("../../../html-frozen/fonts/tinos-latin-700-normal.woff2");

const STAMPED_TIN_NAMES: [&str; 4] = [
    "frm2551Qv2018:txtTIN1",
    "frm2551Qv2018:txtTIN2",
    "frm2551Qv2018:txtTIN3",
    "frm2551Qv2018:txtBranchCode",
];

struct InputTag<'a> {
    start: usize,
    end: usize,
    tag: &'a str,
    name: &'a str,
    slot: Option<usize>,
}

pub fn html_2551q() -> &'static str {
    bundle("2551q-2018").expect("2551q-2018 freeze bundle").html
}

/// Rewrite matching `<input name>` tags. Comb slots (`data-slot-index`) receive
/// one character each. Unknown keys are ignored. `id=` is never changed.
pub fn fill_by_name(html: &str, fields: &BTreeMap<String, String>) -> String {
    fill_by_name_with_cells(html, fields, &BTreeMap::new(), &BTreeMap::new())
}

/// `plan`: the stacked-line cells whose item's text overflows its boxes
/// ([`line_group_plan`]), printed as planned instead of from their key.
/// Outside a bundle: Arimo, no spacing ([`SheetType::plain`]).
fn fill_by_name_with_cells(
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &BTreeMap<String, Vec<String>>,
    plan: &BTreeMap<String, CellFill>,
) -> String {
    let empty_captions = BTreeMap::new();
    let filled = fill_by_name_set(
        html,
        fields,
        cells,
        &empty_captions,
        plan,
        &SheetType::plain(),
    );
    strip_converted_rect_separators(&filled)
}

/// [`fill_by_name_with_cells`] with a value longer than its boxes measured
/// in `sheet`'s type.
fn try_fill_by_name_set(
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &BTreeMap<String, Vec<String>>,
    captions: &BTreeMap<String, String>,
    plan: &BTreeMap<String, CellFill>,
    sheet: &SheetType,
    forced: &BTreeSet<String>,
) -> Result<String, String> {
    let tags = input_tags(html);
    if tags.is_empty() {
        return Ok(html.to_string());
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let grouped = grouped_input_indices(&tags);

    for (name, value) in fields {
        let targets: Vec<(&str, bool)> = if grouped.contains_key(name.as_str()) {
            vec![(name.as_str(), false)]
        } else if let Some(ids) = cells.get(name) {
            ids.iter().map(|id| (id.as_str(), true)).collect()
        } else {
            continue;
        };
        for (input_name, via_cell) in targets {
            let Some(indices) = grouped.get(input_name) else {
                continue;
            };
            // A stacked line whose item overflows prints as planned below.
            if via_cell && plan.contains_key(input_name) {
                continue;
            }
            let target = TextTarget {
                name,
                input_name,
                value,
                via_cell,
                caption: captions.get(input_name).map(String::as_str),
                sheet,
            };
            fill_text_input(html, &tags, indices, &target, &mut replacements, forced);
        }
    }
    // An overflowing stacked item: re-split over its boxes, or, past them,
    // every slot left empty and each line's part laid over its cell as
    // plain type.
    for (cell, fill) in plan {
        let Some(indices) = grouped.get(cell.as_str()) else {
            continue;
        };
        match fill {
            CellFill::Boxes(key, line) => {
                let target = TextTarget {
                    name: key,
                    input_name: cell,
                    value: line,
                    via_cell: true,
                    caption: captions.get(cell).map(String::as_str),
                    sheet,
                };
                fill_text_input(html, &tags, indices, &target, &mut replacements, forced);
            }
            CellFill::Plain(text, size) => {
                for &index in indices {
                    replacements.push((
                        tags[index].start,
                        tags[index].end,
                        set_value(tags[index].tag, "", None),
                    ));
                }
                if let Some((at, line)) = overflow_input_with_fit(html, cell, text, *size, sheet) {
                    replacements.push((at, at, line));
                }
            }
        }
    }

    apply_replacements_checked(html, replacements)
}

/// [`fill_by_name_with_cells`] with a value longer than its boxes measured
/// in `sheet`'s type, for tests and internal callers outside the fallible
/// document fill. Production document filling calls `try_fill_by_name_set`.
fn fill_by_name_set(
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &BTreeMap<String, Vec<String>>,
    captions: &BTreeMap<String, String>,
    plan: &BTreeMap<String, CellFill>,
    sheet: &SheetType,
) -> String {
    try_fill_by_name_set(html, fields, cells, captions, plan, sheet, &BTreeSet::new())
        .expect("test/internal print replacements overlap")
}

/// How one stacked-line cell prints when its item's text overflows the
/// boxes as the print map laid it out.
#[derive(Debug, Clone, PartialEq)]
enum CellFill {
    /// The item re-split over its boxes (it fits them): this line,
    /// printed as the cell's key (`.0`) prints.
    Boxes(String, String),
    /// Past the boxes: the slots stay empty and this part of the text
    /// prints as plain type of `size` pt over the cell.
    Plain(String, f64),
}

/// The type a comb's boxes print in: the largest an overflow line uses.
const PLAIN_MAX_PT: f64 = 9.0;
/// What the per-character estimate assumed before every sheet's overflow
/// type was measured (P4-7): about 0.58 em a character. Kept for the tests
/// that show how it fell short.
#[cfg(test)]
const EM_PER_CHAR: f64 = 0.58;

/// Units per em of both bundled print faces
/// (html-frozen/fonts/arimo-latin-wght-normal.woff2 and
/// tinos-latin-400-normal.woff2).
const UNITS_PER_EM: f64 = 2048.0;

/// arimo-latin-wght-normal.woff2: units per em 2048; every character its
/// cmap maps from U+0020 up, with its advance width (`hmtx`), by code point.
const ARIMO_ADVANCES: &[(u32, u16)] = &[
    (0x0020, 569),
    (0x0021, 569),
    (0x0022, 727),
    (0x0023, 1139),
    (0x0024, 1139),
    (0x0025, 1821),
    (0x0026, 1366),
    (0x0027, 391),
    (0x0028, 682),
    (0x0029, 682),
    (0x002A, 797),
    (0x002B, 1196),
    (0x002C, 569),
    (0x002D, 682),
    (0x002E, 569),
    (0x002F, 569),
    (0x0030, 1139),
    (0x0031, 1139),
    (0x0032, 1139),
    (0x0033, 1139),
    (0x0034, 1139),
    (0x0035, 1139),
    (0x0036, 1139),
    (0x0037, 1139),
    (0x0038, 1139),
    (0x0039, 1139),
    (0x003A, 569),
    (0x003B, 569),
    (0x003C, 1196),
    (0x003D, 1196),
    (0x003E, 1196),
    (0x003F, 1139),
    (0x0040, 2079),
    (0x0041, 1366),
    (0x0042, 1366),
    (0x0043, 1479),
    (0x0044, 1479),
    (0x0045, 1366),
    (0x0046, 1251),
    (0x0047, 1593),
    (0x0048, 1479),
    (0x0049, 569),
    (0x004A, 1024),
    (0x004B, 1366),
    (0x004C, 1139),
    (0x004D, 1706),
    (0x004E, 1479),
    (0x004F, 1593),
    (0x0050, 1366),
    (0x0051, 1593),
    (0x0052, 1479),
    (0x0053, 1366),
    (0x0054, 1251),
    (0x0055, 1479),
    (0x0056, 1366),
    (0x0057, 1933),
    (0x0058, 1366),
    (0x0059, 1366),
    (0x005A, 1251),
    (0x005B, 569),
    (0x005C, 569),
    (0x005D, 569),
    (0x005E, 961),
    (0x005F, 1139),
    (0x0060, 682),
    (0x0061, 1139),
    (0x0062, 1139),
    (0x0063, 1024),
    (0x0064, 1139),
    (0x0065, 1139),
    (0x0066, 569),
    (0x0067, 1139),
    (0x0068, 1139),
    (0x0069, 455),
    (0x006A, 455),
    (0x006B, 1024),
    (0x006C, 455),
    (0x006D, 1706),
    (0x006E, 1139),
    (0x006F, 1139),
    (0x0070, 1139),
    (0x0071, 1139),
    (0x0072, 682),
    (0x0073, 1024),
    (0x0074, 569),
    (0x0075, 1139),
    (0x0076, 1024),
    (0x0077, 1479),
    (0x0078, 1024),
    (0x0079, 1024),
    (0x007A, 1024),
    (0x007B, 684),
    (0x007C, 532),
    (0x007D, 684),
    (0x007E, 1196),
    (0x00A0, 569),
    (0x00A1, 682),
    (0x00A2, 1139),
    (0x00A3, 1139),
    (0x00A4, 1139),
    (0x00A5, 1139),
    (0x00A6, 532),
    (0x00A7, 1139),
    (0x00A8, 682),
    (0x00A9, 1509),
    (0x00AA, 758),
    (0x00AB, 1139),
    (0x00AC, 1196),
    (0x00AD, 682),
    (0x00AE, 1509),
    (0x00AF, 1131),
    (0x00B0, 819),
    (0x00B1, 1124),
    (0x00B2, 682),
    (0x00B3, 682),
    (0x00B4, 682),
    (0x00B5, 1180),
    (0x00B6, 1100),
    (0x00B7, 682),
    (0x00B8, 682),
    (0x00B9, 682),
    (0x00BA, 748),
    (0x00BB, 1139),
    (0x00BC, 1708),
    (0x00BD, 1708),
    (0x00BE, 1708),
    (0x00BF, 1251),
    (0x00C0, 1366),
    (0x00C1, 1366),
    (0x00C2, 1366),
    (0x00C3, 1366),
    (0x00C4, 1366),
    (0x00C5, 1366),
    (0x00C6, 2048),
    (0x00C7, 1479),
    (0x00C8, 1366),
    (0x00C9, 1366),
    (0x00CA, 1366),
    (0x00CB, 1366),
    (0x00CC, 569),
    (0x00CD, 569),
    (0x00CE, 569),
    (0x00CF, 569),
    (0x00D0, 1479),
    (0x00D1, 1479),
    (0x00D2, 1593),
    (0x00D3, 1593),
    (0x00D4, 1593),
    (0x00D5, 1593),
    (0x00D6, 1593),
    (0x00D7, 1196),
    (0x00D8, 1593),
    (0x00D9, 1479),
    (0x00DA, 1479),
    (0x00DB, 1479),
    (0x00DC, 1479),
    (0x00DD, 1366),
    (0x00DE, 1366),
    (0x00DF, 1251),
    (0x00E0, 1139),
    (0x00E1, 1139),
    (0x00E2, 1139),
    (0x00E3, 1139),
    (0x00E4, 1139),
    (0x00E5, 1139),
    (0x00E6, 1821),
    (0x00E7, 1024),
    (0x00E8, 1139),
    (0x00E9, 1139),
    (0x00EA, 1139),
    (0x00EB, 1139),
    (0x00EC, 569),
    (0x00ED, 569),
    (0x00EE, 569),
    (0x00EF, 569),
    (0x00F0, 1139),
    (0x00F1, 1139),
    (0x00F2, 1139),
    (0x00F3, 1139),
    (0x00F4, 1139),
    (0x00F5, 1139),
    (0x00F6, 1139),
    (0x00F7, 1124),
    (0x00F8, 1251),
    (0x00F9, 1139),
    (0x00FA, 1139),
    (0x00FB, 1139),
    (0x00FC, 1139),
    (0x00FD, 1024),
    (0x00FE, 1139),
    (0x00FF, 1024),
    (0x0131, 569),
    (0x0152, 2048),
    (0x0153, 1933),
    (0x02BB, 455),
    (0x02BC, 455),
    (0x02C6, 683),
    (0x02DA, 682),
    (0x02DC, 682),
    (0x0300, 0),
    (0x0301, 0),
    (0x0303, 0),
    (0x0304, 0),
    (0x0308, 0),
    (0x0309, 0),
    (0x0323, 0),
    (0x0329, 0),
    (0x2002, 1024),
    (0x2009, 410),
    (0x200B, 0),
    (0x2013, 1139),
    (0x2014, 2048),
    (0x2018, 455),
    (0x2019, 455),
    (0x201A, 455),
    (0x201C, 682),
    (0x201D, 682),
    (0x201E, 682),
    (0x2022, 717),
    (0x2026, 2048),
    (0x2032, 384),
    (0x2033, 725),
    (0x2039, 682),
    (0x203A, 682),
    (0x2044, 342),
    (0x20AC, 1139),
    (0x2122, 2048),
    (0x2191, 1024),
    (0x2193, 1024),
    (0x2212, 1196),
    (0x2215, 342),
    (0xFEFF, 0),
    (0xFFFD, 1721),
];
/// tinos-latin-400-normal.woff2: units per em 2048; every character its
/// cmap maps from U+0020 up, with its advance width (`hmtx`), by code point.
const TINOS_ADVANCES: &[(u32, u16)] = &[
    (0x0020, 512),
    (0x0021, 682),
    (0x0022, 836),
    (0x0023, 1024),
    (0x0024, 1024),
    (0x0025, 1706),
    (0x0026, 1593),
    (0x0027, 369),
    (0x0028, 682),
    (0x0029, 682),
    (0x002A, 1024),
    (0x002B, 1155),
    (0x002C, 512),
    (0x002D, 682),
    (0x002E, 512),
    (0x002F, 569),
    (0x0030, 1024),
    (0x0031, 1024),
    (0x0032, 1024),
    (0x0033, 1024),
    (0x0034, 1024),
    (0x0035, 1024),
    (0x0036, 1024),
    (0x0037, 1024),
    (0x0038, 1024),
    (0x0039, 1024),
    (0x003A, 569),
    (0x003B, 569),
    (0x003C, 1155),
    (0x003D, 1155),
    (0x003E, 1155),
    (0x003F, 909),
    (0x0040, 1886),
    (0x0041, 1479),
    (0x0042, 1366),
    (0x0043, 1366),
    (0x0044, 1479),
    (0x0045, 1251),
    (0x0046, 1139),
    (0x0047, 1479),
    (0x0048, 1479),
    (0x0049, 682),
    (0x004A, 797),
    (0x004B, 1479),
    (0x004C, 1251),
    (0x004D, 1821),
    (0x004E, 1479),
    (0x004F, 1479),
    (0x0050, 1139),
    (0x0051, 1479),
    (0x0052, 1366),
    (0x0053, 1139),
    (0x0054, 1251),
    (0x0055, 1479),
    (0x0056, 1479),
    (0x0057, 1933),
    (0x0058, 1479),
    (0x0059, 1479),
    (0x005A, 1251),
    (0x005B, 682),
    (0x005C, 569),
    (0x005D, 682),
    (0x005E, 961),
    (0x005F, 1024),
    (0x0060, 682),
    (0x0061, 909),
    (0x0062, 1024),
    (0x0063, 909),
    (0x0064, 1024),
    (0x0065, 909),
    (0x0066, 682),
    (0x0067, 1024),
    (0x0068, 1024),
    (0x0069, 569),
    (0x006A, 569),
    (0x006B, 1024),
    (0x006C, 569),
    (0x006D, 1593),
    (0x006E, 1024),
    (0x006F, 1024),
    (0x0070, 1024),
    (0x0071, 1024),
    (0x0072, 682),
    (0x0073, 797),
    (0x0074, 569),
    (0x0075, 1024),
    (0x0076, 1024),
    (0x0077, 1479),
    (0x0078, 1024),
    (0x0079, 1024),
    (0x007A, 909),
    (0x007B, 983),
    (0x007C, 410),
    (0x007D, 983),
    (0x007E, 1108),
    (0x00A0, 512),
    (0x00A1, 682),
    (0x00A2, 1024),
    (0x00A3, 1024),
    (0x00A4, 1024),
    (0x00A5, 1024),
    (0x00A6, 410),
    (0x00A7, 1024),
    (0x00A8, 682),
    (0x00A9, 1556),
    (0x00AA, 565),
    (0x00AB, 1024),
    (0x00AC, 1155),
    (0x00AD, 682),
    (0x00AE, 1556),
    (0x00AF, 1024),
    (0x00B0, 819),
    (0x00B1, 1124),
    (0x00B2, 614),
    (0x00B3, 614),
    (0x00B4, 682),
    (0x00B5, 1180),
    (0x00B6, 928),
    (0x00B7, 682),
    (0x00B8, 682),
    (0x00B9, 614),
    (0x00BA, 635),
    (0x00BB, 1024),
    (0x00BC, 1536),
    (0x00BD, 1536),
    (0x00BE, 1536),
    (0x00BF, 909),
    (0x00C0, 1479),
    (0x00C1, 1479),
    (0x00C2, 1479),
    (0x00C3, 1479),
    (0x00C4, 1479),
    (0x00C5, 1479),
    (0x00C6, 1821),
    (0x00C7, 1366),
    (0x00C8, 1251),
    (0x00C9, 1251),
    (0x00CA, 1251),
    (0x00CB, 1251),
    (0x00CC, 682),
    (0x00CD, 682),
    (0x00CE, 682),
    (0x00CF, 682),
    (0x00D0, 1479),
    (0x00D1, 1479),
    (0x00D2, 1479),
    (0x00D3, 1479),
    (0x00D4, 1479),
    (0x00D5, 1479),
    (0x00D6, 1479),
    (0x00D7, 1155),
    (0x00D8, 1479),
    (0x00D9, 1479),
    (0x00DA, 1479),
    (0x00DB, 1479),
    (0x00DC, 1479),
    (0x00DD, 1479),
    (0x00DE, 1139),
    (0x00DF, 1024),
    (0x00E0, 909),
    (0x00E1, 909),
    (0x00E2, 909),
    (0x00E3, 909),
    (0x00E4, 909),
    (0x00E5, 909),
    (0x00E6, 1366),
    (0x00E7, 909),
    (0x00E8, 909),
    (0x00E9, 909),
    (0x00EA, 909),
    (0x00EB, 909),
    (0x00EC, 569),
    (0x00ED, 569),
    (0x00EE, 569),
    (0x00EF, 569),
    (0x00F0, 1024),
    (0x00F1, 1024),
    (0x00F2, 1024),
    (0x00F3, 1024),
    (0x00F4, 1024),
    (0x00F5, 1024),
    (0x00F6, 1024),
    (0x00F7, 1124),
    (0x00F8, 1024),
    (0x00F9, 1024),
    (0x00FA, 1024),
    (0x00FB, 1024),
    (0x00FC, 1024),
    (0x00FD, 1024),
    (0x00FE, 1024),
    (0x00FF, 1024),
    (0x0131, 569),
    (0x0152, 1821),
    (0x0153, 1479),
    (0x02BB, 682),
    (0x02BC, 682),
    (0x02C6, 682),
    (0x02DA, 682),
    (0x02DC, 682),
    (0x0300, 0),
    (0x0301, 0),
    (0x0303, 0),
    (0x0304, 0),
    (0x0308, 0),
    (0x0309, 0),
    (0x0323, 0),
    (0x0329, 0),
    (0x2002, 1024),
    (0x2009, 410),
    (0x200B, 0),
    (0x2013, 1024),
    (0x2014, 2048),
    (0x2018, 682),
    (0x2019, 682),
    (0x201A, 682),
    (0x201C, 909),
    (0x201D, 909),
    (0x201E, 909),
    (0x2022, 717),
    (0x2026, 2048),
    (0x2032, 448),
    (0x2033, 853),
    (0x2039, 682),
    (0x203A, 682),
    (0x2044, 342),
    (0x20AC, 1024),
    (0x2122, 2007),
    (0x2191, 1024),
    (0x2193, 1024),
    (0x2212, 1155),
    (0x2215, 342),
    (0xFEFF, 1638),
    (0xFFFD, 1721),
];

/// What a character a face's cmap does not map counts for, in ems: the
/// browser draws it from another font (Cyrillic, CJK, emoji, Latin
/// Extended), whose advance the print cannot know. 1.3 em is above the
/// about 1.2 em CJK ideographs and emoji take, so the type can only come
/// out smaller.
const FALLBACK_EMS: f64 = 1.3;

/// How many space advances apart Chromium sets tab stops (CSS `tab-size`,
/// 8 by default). A plain input's value keeps a typed tab, which Chromium
/// draws to the next stop. Measured in headless Chrome in an `<input>` with
/// the eBIRForms faces (2048 px type): the stops are 8 × (the face's space
/// advance + word-spacing + letter-spacing) apart from the start of the
/// line, Arimo 8 × 569 and Tinos 8 × 512 font units; the tab takes no
/// letter- or word-spacing of its own; and a tab that would end less than
/// half a space advance short of its stop runs on to the next one. The
/// stacked-line overflow path ([`plain_lines`]) splits a value into words,
/// so there a tab or a no-break space prints as a single space.
const TAB_SPACES: f64 = 8.0;

/// Whether word-spacing applies after `c`: a space or a no-break space
/// (U+00A0); headless Chrome adds an `<input>`'s word-spacing to both.
fn is_word_space(c: char) -> bool {
    c == ' ' || c == '\u{a0}'
}

/// The face a sheet sets its `.fi` inputs in (form.css `font-family`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Arimo,
    Tinos,
}

impl Face {
    fn advances(self) -> &'static [(u32, u16)] {
        match self {
            Self::Arimo => ARIMO_ADVANCES,
            Self::Tinos => TINOS_ADVANCES,
        }
    }

    /// `text`'s advance width in ems of this face; a tab counts a whole
    /// tab stop, [`TAB_SPACES`] space advances (the most it takes; where it
    /// ends is [`SheetType::width`]'s), another character the face does not
    /// map [`FALLBACK_EMS`].
    fn ems(self, text: &str) -> f64 {
        text.chars().map(|c| self.char_ems(c)).sum()
    }

    /// [`Face::ems`] of one character.
    fn char_ems(self, c: char) -> f64 {
        if c == '\t' {
            return TAB_SPACES * self.char_ems(' ');
        }
        let table = self.advances();
        table
            .binary_search_by_key(&u32::from(c), |(cp, _)| *cp)
            .map_or(FALLBACK_EMS, |at| f64::from(table[at].1) / UNITS_PER_EM)
    }
}

/// A CSS length the print adds per character or per space: its `pt` part,
/// fixed, and its `em` part, which scales with the type size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Spacing {
    pt: f64,
    em: f64,
}

impl Spacing {
    /// `word-spacing:0.3238pt`, `letter-spacing:-0.01em` in a CSS rule;
    /// none (normal, 0, absent) is zero.
    fn of(rule: &str, property: &str) -> Self {
        let Some(at) = rule.find(&format!("{property}:")) else {
            return Self::default();
        };
        let value = rule[at + property.len() + 1..]
            .split([';', '}'])
            .next()
            .unwrap_or("")
            .trim();
        let number = |unit: &str| value.strip_suffix(unit).and_then(|n| n.trim().parse().ok());
        if let Some(pt) = number("pt") {
            Self { pt, em: 0.0 }
        } else if let Some(px) = number("px") {
            Self {
                pt: px * 0.75,
                em: 0.0,
            }
        } else if let Some(em) = number("em") {
            Self { pt: 0.0, em }
        } else {
            Self::default()
        }
    }

    fn at(self, size: f64) -> f64 {
        self.pt + self.em * size
    }
}

/// The `{ ... }` body of CSS rule `selector` in `css` (`.fi`, `.fh4`).
fn css_rule<'a>(css: &'a str, selector: &str) -> Option<&'a str> {
    let open = format!("{selector} {{");
    let mut from = 0;
    while let Some(at) = css[from..].find(&open) {
        let at = from + at;
        // A whole selector: at a line start, not `.fi:hover` or `.fh40`.
        if at == 0 || css[..at].ends_with('\n') {
            let body = &css[at + open.len()..];
            return Some(&body[..body.find('}').unwrap_or(body.len())]);
        }
        from = at + open.len();
    }
    None
}

/// How a sheet sets the type of its inputs, from its form.css: the `.fi`
/// rule's face, word-spacing and letter-spacing (P4-7; the sheets set them
/// per sheet: word-spacing is 0.3238pt on 1604F, 0.4092pt on 2200T), and
/// the stylesheet, for each input class's size and letter-spacing.
#[derive(Debug, Clone, Copy)]
struct SheetType {
    face: Face,
    word: Spacing,
    letter: Spacing,
    css: &'static str,
    /// The sheet's [`SINGLE_CHARACTER_CELLS`].
    split: &'static [(&'static str, bool)],
    /// How the sheet's form shows a negative amount ([`NegativeStyle`]).
    negative: NegativeStyle,
}

impl SheetType {
    /// A sheet's type from its stylesheet.
    fn of(css: &'static str) -> Self {
        let rule = css_rule(css, ".fi").unwrap_or("");
        let family = rule
            .find("font-family:")
            .map(|at| &rule[at..])
            .and_then(|tail| tail.split([';', ',']).next())
            .unwrap_or("");
        Self {
            face: if family.contains("Tinos") {
                Face::Tinos
            } else {
                Face::Arimo
            },
            word: Spacing::of(rule, "word-spacing"),
            letter: Spacing::of(rule, "letter-spacing"),
            css,
            split: &[],
            negative: NegativeStyle::Minus,
        }
    }

    /// The type of sheet `slug`, with its split one-character cells and its
    /// form's negative amounts.
    fn of_sheet(slug: &str, css: &'static str) -> Self {
        Self {
            split: SINGLE_CHARACTER_CELLS
                .iter()
                .find(|(sheet, _)| *sheet == slug)
                .map_or(&[], |(_, cells)| *cells),
            negative: NegativeStyle::of_sheet(slug),
            ..Self::of(css)
        }
    }

    /// No stylesheet: Arimo with no spacing (a fill outside any bundle).
    fn plain() -> Self {
        Self {
            face: Face::Arimo,
            word: Spacing::default(),
            letter: Spacing::default(),
            css: "",
            split: &[],
            negative: NegativeStyle::Minus,
        }
    }

    /// Minimum sheet-coordinate font size (pt) that still prints at or above
    /// [`PLAIN_TEXT_MIN_PT`] (7.5 pt) once A4 fit-scaling is applied.
    fn a4_min_text_pt(&self) -> f64 {
        const A4_W: f64 = 595.28;
        const A4_H: f64 = 841.89;
        const SAFETY: f64 = 10.0;
        if self.css.is_empty() || !self.css.contains("@page") {
            return PLAIN_TEXT_MIN_PT;
        }
        let (w_str, h_str) = page_size_from_css(self.css);
        let (Ok(w), Ok(h)) = (
            w_str.trim().trim_end_matches("pt").parse::<f64>(),
            h_str.trim().trim_end_matches("pt").parse::<f64>(),
        ) else {
            return PLAIN_TEXT_MIN_PT;
        };
        let scale = ((A4_W - 2.0 * SAFETY) / w)
            .min((A4_H - 2.0 * SAFETY) / h)
            .min(1.0);
        if scale >= 1.0 || scale <= 0.0 {
            PLAIN_TEXT_MIN_PT
        } else {
            ((PLAIN_TEXT_MIN_PT / scale) * 20.0).ceil() / 20.0
        }
    }

    /// The letter-spacing an input of class list `classes` gets: its own
    /// size class's (`.fh4`, which follows `.fi` in the stylesheet), else
    /// `.fi`'s.
    fn letter_for(&self, classes: &str) -> Spacing {
        classes
            .split_whitespace()
            .filter(|class| class.starts_with("fh"))
            .filter_map(|class| css_rule(self.css, &format!(".{class}")))
            .find(|rule| rule.contains("letter-spacing:"))
            .map_or(self.letter, |rule| Spacing::of(rule, "letter-spacing"))
    }

    /// The type size (pt) class list `classes` sets (`.fh4 { font-size:9pt }`).
    fn size_for(&self, classes: &str) -> Option<f64> {
        classes
            .split_whitespace()
            .filter(|class| class.starts_with("fh"))
            .filter_map(|class| css_rule(self.css, &format!(".{class}")))
            .find_map(|rule| {
                let at = rule.find("font-size:")?;
                rule[at + "font-size:".len()..]
                    .split(';')
                    .next()?
                    .trim()
                    .strip_suffix("pt")?
                    .parse()
                    .ok()
            })
    }

    /// The width (pt) `text` sets at `size` pt with `letter` spacing: its
    /// advances in this face, the word-spacing on every space and the
    /// letter-spacing on every character; a tab runs to the next tab stop
    /// ([`TAB_SPACES`]).
    fn width(&self, letter: Spacing, text: &str, size: f64) -> f64 {
        if text.contains('\t') {
            return self.width_with_tabs(letter, text, size);
        }
        let spaces = text.chars().filter(|c| is_word_space(*c)).count() as f64;
        let chars = text.chars().count() as f64;
        self.face.ems(text) * size + spaces * self.word.at(size) + chars * letter.at(size)
    }

    /// [`SheetType::width`] of a text with tabs, set left to right: each
    /// tab ends at the next multiple of the stop, 8 × (space advance +
    /// word-spacing + letter-spacing), from the line's start, or at the one
    /// after it when it would end less than half a space advance short.
    fn width_with_tabs(&self, letter: Spacing, text: &str, size: f64) -> f64 {
        let (word, letter) = (self.word.at(size), letter.at(size));
        let space = self.face.char_ems(' ') * size;
        let stop = TAB_SPACES * (space + word + letter);
        text.chars().fold(0.0, |at, c| {
            if c == '\t' {
                if stop <= 0.0 {
                    return at;
                }
                let mut advance = stop - at.rem_euclid(stop);
                if advance < space / 2.0 {
                    advance += stop;
                }
                at + advance
            } else if is_word_space(c) {
                at + self.face.char_ems(c) * size + word + letter
            } else {
                at + self.face.char_ems(c) * size + letter
            }
        })
    }

    /// The largest type size (pt), at most `max` and rounded down to 0.01
    /// pt, at which `text` sets within `room` pt: the width is linear in the
    /// size, `ems × size + spaces × word + characters × letter`. A text with
    /// tabs, where a tab ends is neither linear nor monotone in the size
    /// (with spacing in points, a smaller size can push a tab past a
    /// stop), is scanned down from `max` a hundredth of a point at a time
    /// to the first size that sets, returned as that exact hundredth (0.01
    /// pt when none sets).
    fn fit(&self, letter: Spacing, room: f64, text: &str, max: f64) -> f64 {
        if text.contains('\t') {
            let top = ((max * 100.0).floor() as i64).max(1);
            let size = (1..=top)
                .rev()
                .find(|hundredths| self.width(letter, text, *hundredths as f64 / 100.0) <= room)
                .unwrap_or(1);
            return size as f64 / 100.0;
        }
        let spaces = text.chars().filter(|c| is_word_space(*c)).count() as f64;
        let chars = text.chars().count() as f64;
        let per_pt = self.face.ems(text) + spaces * self.word.em + chars * letter.em;
        let fixed = spaces * self.word.pt + chars * letter.pt;
        let size = if per_pt <= 0.0 {
            max
        } else {
            ((room - fixed) / per_pt).min(max)
        };
        floor_size(size)
    }
}

/// A type size rounded down to 0.01 pt (what the print writes), never
/// below 0.01 pt: rounding never adds width.
fn floor_size(size: f64) -> f64 {
    ((size * 100.0).floor() / 100.0).max(0.01)
}

/// `text`'s width in ems of Arimo (the tests that show the old estimate).
#[cfg(test)]
fn arimo_ems(text: &str) -> f64 {
    Face::Arimo.ems(text)
}

/// The type size (pt) a line of `text` past a comb of `width` pt prints
/// at: measured in the sheet's own face with its word- and letter-spacing
/// ([`SheetType::fit`]), within the cell less the line's 1 pt left padding
/// and 1 pt to spare (a cell under 3 pt wide keeps 1 pt, or what is past
/// the padding when that is less), at most [`PLAIN_MAX_PT`], rounded down
/// to 0.01 pt. Every sheet sizes its overflow type this way (maintainer decision P4-7,
/// 2026-10-19): a value that fits its boxes prints byte for byte as
/// before; a value longer than its boxes is measured.
fn fit_line(sheet: &SheetType, width: f64, text: &str) -> f64 {
    sheet.fit(
        sheet.letter,
        (width - 2.0).max((width - 1.0).min(1.0)),
        text,
        PLAIN_MAX_PT,
    )
}

/// How much wider than its room a value may measure and still fit: Chromium
/// lays text out in 1/64 px, so a value the model puts a hair over the room
/// fits on the page.
const FIT_EPSILON_PT: f64 = 0.02;

/// Minimum inner box height (pt, after subtracting top/bottom `inset`) for a
/// plain text box to wrap onto two lines before shrinking (Rule 3: two lines
/// at 7.5pt with 1.04 line-height need 15.60pt, allowing 15.72pt inner-height
/// rows like 2200P Others while keeping 7.71pt boxes like 1601C item 20 on
/// one line).
const PLAIN_WRAP_MIN_HEIGHT_PT: f64 = 15.55;

/// Minimum readable type size (pt) for text in plain cells and converted
/// rectangles (Rule 3). Below this floor, text is truncated with `"..."` and
/// printed in full on the additional sheet.
const PLAIN_TEXT_MIN_PT: f64 = 7.5;

/// An amount or an identifier (a TIN, an ATC) shrunk below this size (pt)
/// to fit its box still prints there, whole, and is also carried, whole, to
/// the additional sheet (`data-overflow-warning="amount-small"` or
/// `"identifier-small"`): legible in full somewhere.
pub(crate) const SMALL_WHOLE_BELOW_PT: f64 = 5.0;

/// Vertical room factor for two wrapped lines (`2 * 1.04 = 2.08`).
const PLAIN_TWO_LINE_HEIGHT_FACTOR: f64 = 2.08;

/// Line-height applied to two-line wrapped plain text cells.
const PLAIN_TWO_LINE_CSS_HEIGHT: &str = "1.04";

/// The width and height (pt) of plain (`data-field-kind="text"`) box `cell` of `html`.
fn plain_cell_dimensions(html: &str, cell: &str) -> Option<(f64, f64)> {
    let (_, tag) = cell_tag(html, cell)?;
    if attr(tag, "data-field-kind") != Some("text") {
        return None;
    }
    let width_at = tag.find(";width:")? + ";width:".len();
    let width = tag[width_at..].split("pt").next()?.parse().ok()?;
    let height_at = tag.find(";height:")? + ";height:".len();
    let height = tag[height_at..].split("pt").next()?.parse().ok()?;
    Some((width, height))
}

/// The width (pt) of plain (`data-field-kind="text"`) box `cell` of `html`.
#[cfg(test)]
fn plain_cell_width(html: &str, cell: &str) -> Option<f64> {
    plain_cell_dimensions(html, cell).map(|(width, _)| width)
}

/// Result of fitting text into a plain (`data-field-kind="text"`) input cell.
struct PlainTextFit {
    text: String,
    font_size_pt: f64,
    two_lines: bool,
    truncated: bool,
    tight_inset_v: Option<f64>,
}

/// Truncate `text` with `"..."` so it fits within `room` pt at `size` pt.
fn truncate_with_ellipsis(
    sheet: &SheetType,
    letter: Spacing,
    room: f64,
    text: &str,
    size: f64,
) -> Option<String> {
    truncate_with_ellipsis_mode(sheet, letter, room, text, size, false)
}

fn truncate_with_ellipsis_mode(
    sheet: &SheetType,
    letter: Spacing,
    room: f64,
    text: &str,
    size: f64,
    whole_words: bool,
) -> Option<String> {
    if sheet.width(letter, "...", size) > room {
        return None;
    }
    let mut best_end = 0;
    for (idx, ch) in text.char_indices() {
        let end = idx + ch.len_utf8();
        let candidate = format!("{}...", text[..end].trim_end());
        if sheet.width(letter, &candidate, size) <= room {
            best_end = end;
        } else {
            break;
        }
    }
    let prefix = text[..best_end].trim_end();
    // A Rule 6 band ends on a whole word: a fragment cut mid-word prints as
    // if it were a word of its own. Back off to the last word boundary the
    // fit reached. A single word longer than the room still cuts at the
    // character: there is no whole word to keep.
    if whole_words && best_end < text.len() {
        let at_word_end = text[best_end..].starts_with(char::is_whitespace)
            || text[..best_end].ends_with(char::is_whitespace);
        if !at_word_end {
            if let Some(space) = prefix.rfind(char::is_whitespace) {
                let word_end = prefix[..space].trim_end();
                if !word_end.is_empty() {
                    return Some(format!("{word_end}..."));
                }
            }
        }
    }
    Some(format!("{prefix}..."))
}

/// Find the best two-line split of `text` that fits within `room` pt at a font
/// size in `[min_size, max_size]`. Prefers word boundaries; falls back to
/// character boundaries when `text` has no spaces.
fn fit_two_lines(
    sheet: &SheetType,
    letter: Spacing,
    room: f64,
    text: &str,
    max_size: f64,
    min_size: f64,
) -> Option<(String, String, f64)> {
    let mut best: Option<(String, String, f64, usize)> = None;
    let mut consider = |l1: &str, l2: &str| {
        if l1.is_empty() || l2.is_empty() {
            return;
        }
        let s1 = sheet.fit(letter, room, l1, max_size);
        let s2 = sheet.fit(letter, room, l2, max_size);
        let sz = s1.min(s2);
        if sz < min_size {
            return;
        }
        let l1_chars = l1.chars().count();
        let better = match &best {
            None => true,
            Some((_, _, best_sz, best_l1)) => {
                sz > *best_sz + 1e-6 || ((sz - *best_sz).abs() <= 1e-6 && l1_chars > *best_l1)
            }
        };
        if better {
            best = Some((l1.to_string(), l2.to_string(), sz, l1_chars));
        }
    };

    let mut had_space = false;
    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            had_space = true;
            let l1 = text[..idx].trim_end();
            let l2 = text[idx + ch.len_utf8()..].trim_start();
            consider(l1, l2);
        }
    }
    if !had_space {
        for (idx, _) in text.char_indices().skip(1) {
            consider(&text[..idx], &text[idx..]);
        }
    }
    best.map(|(l1, l2, sz, _)| (l1, l2, sz))
}

/// Wrap `text` greedily onto line 1 at `size` pt and fit line 2 with `"..."`.
fn wrap_and_truncate_two_lines(
    sheet: &SheetType,
    letter: Spacing,
    room: f64,
    text: &str,
    size: f64,
) -> Option<(String, String)> {
    wrap_and_truncate_two_lines_mode(sheet, letter, room, text, size, false)
}

fn wrap_and_truncate_two_lines_mode(
    sheet: &SheetType,
    letter: Spacing,
    room: f64,
    text: &str,
    size: f64,
    whole_words: bool,
) -> Option<(String, String)> {
    let mut last_fit_char = 0;
    let mut last_fit_space = None;
    for (idx, ch) in text.char_indices() {
        let end = idx + ch.len_utf8();
        let slice = text[..end].trim_end();
        if !slice.is_empty() && sheet.width(letter, slice, size) <= room {
            last_fit_char = end;
            if ch.is_whitespace() {
                last_fit_space = Some(idx);
            }
        } else if !ch.is_whitespace() {
            break;
        }
    }
    let split_at = last_fit_space
        .filter(|&pos| !text[..pos].trim_end().is_empty())
        .unwrap_or(last_fit_char);
    if split_at == 0 || split_at >= text.len() {
        return None;
    }
    let l1 = text[..split_at].trim_end().to_string();
    let rest = text[split_at..].trim_start();
    if l1.is_empty() || rest.is_empty() {
        return None;
    }
    let l2 = truncate_with_ellipsis_mode(sheet, letter, room, rest, size, whole_words)?;
    Some((l1, l2))
}

/// Fit `text` into plain input `tag` inside a cell of `(width, height)` pt:
/// - If it fits on one line at the class's default size, returns `None` (unchanged).
/// - If `allow_wrap && height >= PLAIN_WRAP_MIN_HEIGHT_PT`, wraps onto two lines
///   before shrinking, never below `PLAIN_TEXT_MIN_PT` (7.5 pt); if still too long,
///   ends with `"..."` and marks `truncated`.
/// - Otherwise shrinks on one line down to 7.5 pt; if still too long, ends with
///   `"..."` and marks `truncated`.
/// - An `amount` is never wrapped or cut: it shrinks on one line until it
///   fits, below 7.5 pt if it must: a figure cut short, or broken over two
///   lines, would print a different number.
fn fit_plain_text(
    sheet: &SheetType,
    width: f64,
    height: f64,
    tag: &str,
    text: &str,
    allow_wrap: bool,
    amount: bool,
) -> Option<PlainTextFit> {
    let classes = attr(tag, "class")?;
    let default_size = sheet.size_for(classes)?;
    let letter = sheet.letter_for(classes);
    let room = input_room(width, tag);
    if sheet.width(letter, text, default_size) <= room + FIT_EPSILON_PT {
        return None;
    }
    let shrunk = shrunk_room(room);
    if amount {
        return Some(PlainTextFit {
            text: text.to_string(),
            font_size_pt: sheet.fit(letter, shrunk, text, default_size),
            two_lines: false,
            truncated: false,
            tight_inset_v: None,
        });
    }
    let min_pt = sheet
        .a4_min_text_pt()
        .min(default_size.max(PLAIN_TEXT_MIN_PT));
    let start_size = default_size.max(min_pt);
    let default_inner_height = input_height(height, tag);
    let needed_two_line = min_pt * PLAIN_TWO_LINE_HEIGHT_FACTOR;
    let (inner_height, tight_inset_v) = if default_inner_height >= needed_two_line {
        (default_inner_height, None)
    } else if height - 0.80 >= needed_two_line {
        (height - 0.80, Some(0.40))
    } else if height - 0.20 >= needed_two_line {
        (height - 0.20, Some(0.10))
    } else {
        (default_inner_height, None)
    };

    if allow_wrap && inner_height >= needed_two_line {
        let top_two_line =
            floor_size((inner_height / PLAIN_TWO_LINE_HEIGHT_FACTOR).min(start_size)).max(min_pt);
        if let Some((l1, l2, sz)) = fit_two_lines(sheet, letter, shrunk, text, top_two_line, min_pt)
        {
            return Some(PlainTextFit {
                text: format!("{l1}\n{l2}"),
                font_size_pt: sz,
                two_lines: true,
                truncated: false,
                tight_inset_v,
            });
        }
        if let Some((l1, l2)) = wrap_and_truncate_two_lines(sheet, letter, shrunk, text, min_pt) {
            return Some(PlainTextFit {
                text: format!("{l1}\n{l2}"),
                font_size_pt: min_pt,
                two_lines: true,
                truncated: true,
                tight_inset_v,
            });
        }
    }

    let single_size = sheet.fit(letter, shrunk, text, start_size);
    if single_size >= min_pt {
        return Some(PlainTextFit {
            text: text.to_string(),
            font_size_pt: single_size,
            two_lines: false,
            truncated: false,
            tight_inset_v: None,
        });
    }
    if let Some(truncated) = truncate_with_ellipsis(sheet, letter, shrunk, text, min_pt) {
        return Some(PlainTextFit {
            text: truncated,
            font_size_pt: min_pt,
            two_lines: false,
            truncated: true,
            tight_inset_v: None,
        });
    }
    Some(PlainTextFit {
        text: "...".to_string(),
        font_size_pt: min_pt,
        two_lines: false,
        truncated: true,
        tight_inset_v: None,
    })
}

/// The room (pt) input `tag` leaves for text in a cell `width` pt wide:
/// the width less the input's left and right inset.
fn input_room(width: f64, tag: &str) -> f64 {
    let (_, right, _, left) = inset_box(attr(tag, "style").unwrap_or(""));
    width - left - right
}

/// The vertical room (pt) input `tag` leaves for text in a cell `height` pt
/// tall: the height less the input's top and bottom inset.
fn input_height(height: f64, tag: &str) -> f64 {
    let (top, _, bottom, _) = inset_box(attr(tag, "style").unwrap_or(""));
    (height - top - bottom).max(0.0)
}

/// The `(top, right, bottom, left)` of an input's `inset:` (1 to 4 lengths, pt).
fn inset_box(style: &str) -> (f64, f64, f64, f64) {
    let Some(at) = style.rfind("inset:") else {
        return (0.0, 0.0, 0.0, 0.0);
    };
    let values: Vec<f64> = style[at + "inset:".len()..]
        .split(';')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|v| v.strip_suffix("pt").and_then(|n| n.parse().ok()))
        .collect();
    match values.as_slice() {
        [all] => (*all, *all, *all, *all),
        [tb, lr] => (*tb, *lr, *tb, *lr),
        [top, lr, bottom] => (*top, *lr, *bottom, *lr),
        [top, right, bottom, left] => (*top, *right, *bottom, *left),
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

/// The type size (pt) `text` prints at in plain input `tag` of a cell
/// `width` pt wide when it is longer than the input: wider, at its class's
/// size, than the input's whole room (and [`FIT_EPSILON_PT`]). It is then
/// measured in the sheet's type ([`SheetType::fit`]) to set within
/// [`shrunk_room`], the room less 1 pt to spare, at a whole hundredth of a
/// point. `None` while it fits: it prints byte for byte as before.
#[cfg(test)]
fn plain_input_size(sheet: &SheetType, width: f64, tag: &str, text: &str) -> Option<f64> {
    let classes = attr(tag, "class")?;
    let size = sheet.size_for(classes)?;
    let letter = sheet.letter_for(classes);
    let room = input_room(width, tag);
    (sheet.width(letter, text, size) > room + FIT_EPSILON_PT)
        .then(|| sheet.fit(letter, shrunk_room(room), text, size))
}

/// The width (pt) a shrunk value sets within in an input of `room` pt: the
/// room less 1 pt to spare; an input under 2 pt wide gets 1 pt, or its
/// whole room when that is under 1 pt, never more than the room.
fn shrunk_room(room: f64) -> f64 {
    (room - 1.0).max(room.min(1.0))
}

/// `tag` with `declaration` appended to its inline `style="..."` attribute.
fn with_style_declaration(tag: &str, declaration: &str) -> String {
    if let Some(at) = tag.find(" style=\"") {
        let value_at = at + " style=\"".len();
        if let Some(end) = tag[value_at..].find('"') {
            let end = value_at + end;
            let style = tag[value_at..end].trim_end_matches(';');
            return format!("{}{style};{declaration}{}", &tag[..value_at], &tag[end..]);
        }
    }
    let close = tag.rfind('>').unwrap_or(tag.len());
    let close = if tag[..close].ends_with('/') {
        close - 1
    } else {
        close
    };
    format!("{} style=\"{declaration}\"{}", &tag[..close], &tag[close..])
}

/// `tag` with `font-size:{size}pt` last in its inline style.
fn with_font_size(tag: &str, size: f64) -> String {
    with_style_declaration(tag, &format!("font-size:{size:.2}pt"))
}

/// Whether `value` is formatted like an official two-decimal currency amount
/// (`0.00`, `1,234.50`, `-500.00`), as emitted by `official_amount()` /
/// `formatCurrency()`. Bare integers, TINs, ZIPs, dates, and `%` rates return false.
fn is_formatted_money_amount(value: &str) -> bool {
    let s = value.trim();
    let s = s.strip_prefix('-').unwrap_or(s);
    let Some((whole, cents)) = s.split_once('.') else {
        return false;
    };
    if cents.len() != 2 || !cents.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    !whole.is_empty()
        && whole.bytes().any(|b| b.is_ascii_digit())
        && whole.bytes().all(|b| b.is_ascii_digit() || b == b',')
}

fn conversion_money_amount(value: &str) -> bool {
    let s = value.trim();
    // Bracketed negatives are money too: `(1,234.00)` and `(500)`.
    let s = if let Some(inner) = s.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        inner.trim()
    } else {
        s.strip_prefix('-').unwrap_or(s)
    };
    if s.is_empty() || !s.bytes().any(|b| b.is_ascii_digit()) {
        return false;
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b',' | b'.'))
    {
        return false;
    }
    // The centavos are the money signal: exactly two digits after a point
    // is never a reference, whether or not the pesos carry commas. A bare
    // whole-peso string instead prints comma-grouped through
    // `official_amount()`, so past three places without a comma it is a
    // reference or an identifier, never money.
    let digits_or_commas =
        |whole: &str| !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit() || b == b',');
    match s.split_once('.') {
        Some((whole, cents)) => {
            digits_or_commas(whole) && cents.len() == 2 && cents.bytes().all(|b| b.is_ascii_digit())
        }
        // Whole-peso sheets (1701, 1701Q, 1702…) print no point at all.
        None => digits_or_commas(s) && (s.len() <= 3 || s.contains(',')),
    }
}

/// Whether `value` is a formatted whole-peso amount (`0`, `1,234`, `-500`, `(500)`).
fn is_formatted_whole_peso_amount(value: &str) -> bool {
    let s = value.trim();
    let s = if let Some(inner) = s.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        inner.trim()
    } else {
        s.strip_prefix('-').unwrap_or(s)
    };
    !s.is_empty()
        && !s.contains('.')
        && s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().all(|b| b.is_ascii_digit() || b == b',')
}

/// Whether `key` belongs to one of the nine whole-peso BIR income tax forms
/// whose official sheets instruct "DO NOT enter Centavos".
fn is_whole_peso_writer_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "frm1700:",
        "frm1701:",
        "frm1701a:",
        "frm1701ms:",
        "frm1701q:",
        "frm1702ex:",
        "frm1702mx:",
        "frm1702q:",
        "frm1702rt:",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

/// Format a plain-cell amount on a whole-peso ("DO NOT enter Centavos") form:
/// - strips `.00`, and keeps any other centavos the upload carries (the
///   official page shows them: `"-1,000.51"` stays `"-1,000.51"`, never
///   `"-1,001"`)
/// - groups thousands with commas (`"300,000.00"` -> `"300,000"`,
///   `"150000"` -> `"150,000"`, `"-50000"` -> `"-50,000"`, `"0.00"` -> `"0"`).
///
/// Returns `None` when the text is already a comma-grouped whole peso (or 3 or
/// fewer digits with no decimal point).
fn format_plain_whole_peso_with_commas(text: &str, style: NegativeStyle) -> Option<String> {
    let trimmed = text.trim();
    let (negative, inner) =
        if let Some(inner) = trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            (true, inner.trim())
        } else if let Some(rest) = trimmed.strip_prefix('-') {
            (true, rest.trim())
        } else {
            (false, trimmed)
        };
    let (prefix, suffix) = match (negative, style) {
        (false, _) => ("", ""),
        (true, NegativeStyle::Minus) => ("-", ""),
        (true, NegativeStyle::Parentheses) => ("(", ")"),
    };
    if let Some((whole_part, cents_part)) = inner.split_once('.') {
        if cents_part.len() != 2 || !cents_part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let clean_whole: String = whole_part
            .bytes()
            .filter(|b| *b != b',')
            .map(char::from)
            .collect();
        if clean_whole.is_empty() || !clean_whole.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let grouped = group_thousands(&clean_whole);
        let zero = clean_whole
            .bytes()
            .chain(cents_part.bytes())
            .all(|b| b == b'0');
        let (prefix, suffix) = if zero { ("", "") } else { (prefix, suffix) };
        if cents_part == "00" {
            return Some(format!("{prefix}{grouped}{suffix}"));
        }
        return Some(format!("{prefix}{grouped}.{cents_part}{suffix}"));
    }
    if trimmed.contains(',') {
        return None;
    }
    if inner.len() <= 3 || !inner.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{prefix}{}{suffix}", group_thousands(inner)))
}

/// Whether a cell (by writer key or printed caption) represents a currency/money amount.
fn is_amount_typed_cell(key: &str, caption: Option<&str>) -> bool {
    if let Some(caption) = caption {
        let pl = caption.to_ascii_lowercase();
        let words: Vec<&str> = pl
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        for excluded_phrase in [
            "%",
            "percent",
            "taxable year",
            "calendar year",
            "fiscal year",
            "year ended",
            "year incurred",
            "year of",
            "description",
            "classification",
            "remarks",
            "taxpayer identification",
            "address",
            "email",
            "phone",
            "title",
            "quantity",
            "volume",
        ] {
            if pl.contains(excluded_phrase) {
                return false;
            }
        }
        for excluded_word in [
            "rate", "rates", "date", "dates", "tin", "zip", "rdo", "atc", "code", "codes", "seq",
            "sheet", "sheets", "page", "pages", "name", "names", "tel", "area", "share", "shares",
            "qty", "unit", "units", "count", "stock", "stocks",
        ] {
            if words.contains(&excluded_word) {
                return false;
            }
        }
        if pl.ends_with(" year") {
            return false;
        }
        if is_amount_typed_key(key) {
            return true;
        }
        return [
            "sales",
            "receipt",
            "revenue",
            "fees",
            "returns",
            "cost",
            "gross",
            "income",
            "deduct",
            "nolco",
            "credit",
            "payment",
            "tax",
            "mcit",
            "amount",
            "total",
            "balance",
            "payable",
            "overpay",
            "excess",
            "loss",
            "allowance",
            "exemption",
            "dividend",
            "royalt",
            "compensation",
            "expense",
            "asset",
            "liabilit",
            "equity",
            "capital",
            "profit",
            "fmv",
            "zonal",
            "value",
            "surcharge",
            "interest",
            "compromise",
            "penalt",
            "withheld",
            "withholding",
            "remit",
            "refund",
        ]
        .iter()
        .any(|tok| pl.contains(tok));
    }
    is_amount_typed_key(key)
}

/// Whether a writer key represents a currency/money amount field (not a rate,
/// area, quantity, share count, identifier, date, or text caption).
fn is_amount_typed_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let tail = lower.rsplit(':').next().unwrap_or(&lower);
    // Exclude non-money numeric/code/text keys even if they contain substrings
    // like "tax" (TaxRate, TaxDec, Taxpayer, TaxAgent) or "value".
    for excluded in [
        "rate",
        "percent",
        "pct",
        "area",
        "share",
        "qty",
        "quantity",
        "volume",
        "metric",
        "liter",
        "gram",
        "kilo",
        "weight",
        "ratio",
        "unit",
        "count",
        "taxdec",
        "taxpayer",
        "taxagent",
        "taxableyear",
        "txttin",
        "_tin",
        "tin_",
        "tin1",
        "tin2",
        "tin3",
        "tin4",
        "zip",
        "rdo",
        "date",
        "month",
        "year",
        "day",
        "name",
        "title",
        "addr",
        "email",
        "phone",
        "tel",
        "atc",
        "desc",
        "particular",
        "location",
        "class",
        "oct",
        "tct",
        "cct",
        "sheet",
        "page",
        "seq",
        "memo",
        "bank",
        "check",
        "ror",
        "drawee",
        "num",
        "no_",
        "code",
        "kind",
        "type",
        "status",
        "remarks",
    ] {
        if tail.contains(excluded) {
            return false;
        }
    }
    for money_token in [
        "_base",
        "_withheld",
        "amount",
        "amt",
        "fairmarketvalue",
        "fmv",
        "zonal",
        "ftd",
        "total",
        "tax",
        "income",
        "payment",
        "payable",
        "value",
        "gross",
        "net",
        "deduct",
        "credit",
        "surcharge",
        "interest",
        "compromise",
        "penalt",
        "remit",
        "price",
        "cost",
        "sales",
        "receipt",
        "purchase",
        "loss",
        "claim",
        "estate",
        "realprop",
        "fam_",
        "personalprop",
        "business_",
        "transfer",
        "gift",
        "asset",
        "liab",
        "equity",
        "capital",
        "profit",
        "expense",
        "revenue",
        "allowance",
        "exemption",
        "withholding",
        "withheld",
        "base",
        "shed7_",
        "shed8_",
        "due",
        "paid",
        "overpay",
        "refund",
        "balance",
        "dividend",
        "royalt",
        "compensation",
        "benefit",
        "gain",
        "proceed",
        "consideration",
    ] {
        if tail.contains(money_token) {
            return true;
        }
    }
    false
}

/// Whether `value` is a formatted numeric quantity (integer or decimal with
/// optional thousands commas, e.g. `160000`, `1,000`, `12,345.678`, `250.50`).
fn is_formatted_numeric_quantity(value: &str) -> bool {
    let s = value.trim();
    let s = s.strip_prefix('-').unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    if whole.is_empty()
        || !whole.bytes().any(|b| b.is_ascii_digit())
        || !whole.bytes().all(|b| b.is_ascii_digit() || b == b',')
    {
        return false;
    }
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

/// Whether a writer key represents a numeric quantity column (shares, area,
/// installment count, volume/weight/units) that should right-align in plain
/// table cells, excluding rates, identifiers, dates, and text captions.
fn is_quantity_typed_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let tail = lower.rsplit(':').next().unwrap_or(&lower);
    for excluded in [
        "rate",
        "percent",
        "pct",
        "taxdec",
        "taxpayer",
        "taxagent",
        "taxableyear",
        "txttin",
        "_tin",
        "tin_",
        "tin1",
        "tin2",
        "tin3",
        "tin4",
        "zip",
        "rdo",
        "date",
        "month",
        "year",
        "day",
        "name",
        "title",
        "addr",
        "email",
        "phone",
        "tel",
        "atc",
        "desc",
        "particular",
        "location",
        "class",
        "oct",
        "tct",
        "cct",
        "sheet",
        "page",
        "seq",
        "memo",
        "bank",
        "check",
        "ror",
        "drawee",
        "code",
        "kind",
        "type",
        "status",
        "remarks",
        "cert",
    ] {
        if tail.contains(excluded) {
            return false;
        }
    }
    for qty_token in [
        "area",
        "share",
        "qty",
        "quantity",
        "volume",
        "metric",
        "liter",
        "gram",
        "kilo",
        "weight",
        "installment",
        "txttotaln",
    ] {
        if tail.contains(qty_token) {
            return true;
        }
    }
    false
}

/// One stacked line of a printed item: its cell, the key that fills it,
/// its boxes and its width (pt).
#[derive(Debug, Clone)]
struct LineCell {
    cell: String,
    key: String,
    slots: usize,
    width: f64,
}

/// A printed caption's stem and line number: "9 Registered Address" and
/// "9 Registered Address (second line)" are lines 0 and 1 of one item.
fn caption_line(caption: &str) -> (String, usize) {
    let lower = caption.trim().to_ascii_lowercase();
    const MARKERS: [(&str, usize); 12] = [
        (" (first line)", 0),
        (" (second line)", 1),
        (" (third line)", 2),
        (", first line", 0),
        (", second line", 1),
        (", third line", 2),
        (" (line 1)", 0),
        (" (line 2)", 1),
        (" (line 3)", 2),
        (", line 1", 0),
        (", line 2", 1),
        (", line 3", 2),
    ];
    for (marker, line) in MARKERS {
        if let Some(stem) = lower.strip_suffix(marker) {
            return (stem.trim().to_string(), line);
        }
    }
    (lower, 0)
}

/// A comb cell's boxes and width (pt), read from its `<div id>`.
fn comb_cell(html: &str, cell: &str) -> Option<(usize, f64)> {
    let (_, tag) = cell_tag(html, cell)?;
    if attr(tag, "data-field-kind") != Some("comb") {
        return None;
    }
    let slots = attr(tag, "data-comb-slots")?.parse().ok()?;
    let width_at = tag.find(";width:")? + ";width:".len();
    let width = tag[width_at..].split("pt").next()?.parse().ok()?;
    Some((slots, width))
}

/// The sheet's items printed over stacked comb lines (a two-line address,
/// a three-line registered name), each its lines in order: text joins
/// whose captions name lines 1, 2 (and 3) of one item.
fn line_groups(html: &str, cells: &WriterCells) -> Vec<Vec<LineCell>> {
    let mut items: BTreeMap<String, BTreeMap<usize, LineCell>> = BTreeMap::new();
    for (key, ids) in &cells.joins {
        let Some(first) = ids.first() else {
            continue;
        };
        let (Some(caption), Some((slots, width))) =
            (cells.captions.get(first), comb_cell(html, first))
        else {
            continue;
        };
        let (stem, line) = caption_line(caption);
        items.entry(stem).or_default().insert(
            line,
            LineCell {
                cell: first.clone(),
                key: key.clone(),
                slots,
                width,
            },
        );
    }
    items
        .into_values()
        .filter(|lines| lines.len() >= 2 && lines.keys().copied().eq(0..lines.len()))
        .map(|lines| lines.into_values().collect())
        .collect()
}

/// Lay `text` over lines of `slots` boxes, each line but the last ending at
/// the last space run that starts within its boxes (kept at its end), or
/// at its last box for a word longer than the line; the last line takes
/// the rest (the page's own split, `combs::split_lines` in bir-shell).
fn split_lines(text: &str, slots: &[usize]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest: Vec<char> = text.trim().chars().collect();
    for (i, &n) in slots.iter().enumerate() {
        if i + 1 == slots.len() || rest.len() <= n {
            lines.push(rest.drain(..).collect());
            continue;
        }
        let space = (1..=n)
            .rev()
            .find(|&p| rest[p].is_whitespace() && !rest[p - 1].is_whitespace());
        let cut = match space {
            Some(p) => {
                let mut end = p;
                while end < rest.len() && rest[end].is_whitespace() {
                    end += 1;
                }
                end
            }
            None => n,
        };
        lines.push(rest.drain(..cut).collect());
    }
    lines
}

/// The item's lines put back together: end to end where a line ends or
/// the next starts with a space, or the line runs past its boxes (a field
/// map's fixed-width cut, 1600PT's and 2552's 80 characters), else with
/// one space (a map that broke at a space and dropped it). A word longer
/// than a whole line, cut at its last box, reads as two words.
fn join_lines(lines: &[String], slots: &[usize]) -> String {
    let mut text = String::new();
    let mut previous: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let glued = previous.is_none_or(|j| {
            text.ends_with(char::is_whitespace)
                || line.starts_with(char::is_whitespace)
                || lines[j].chars().count() > slots[j]
        });
        if !glued {
            text.push(' ');
        }
        text.push_str(line);
        previous = Some(i);
    }
    text
}

/// The size the old per-character estimate gave `chars` characters in
/// `width` pt (before P4-7), for the tests that show how it fell short.
#[cfg(test)]
fn fit_size(width: f64, chars: usize) -> f64 {
    if chars == 0 {
        return PLAIN_MAX_PT;
    }
    ((width - 2.0).max(1.0) / (chars as f64 * EM_PER_CHAR)).min(PLAIN_MAX_PT)
}

/// `text` as plain type over lines of `widths` pt, broken only between
/// words: the breaks that let the largest common type size fit, and among
/// those the nearest each line's share of the width, so the text runs
/// across every line. Each line's part, and the size.
fn plain_lines(text: &str, widths: &[f64], sheet: &SheetType) -> (Vec<String>, f64) {
    fn walk(level: usize, words: usize, cuts: &mut Vec<usize>, visit: &mut dyn FnMut(&[usize])) {
        if level == cuts.len() - 1 {
            visit(cuts);
            return;
        }
        for cut in cuts[level - 1]..=words {
            cuts[level] = cut;
            walk(level + 1, words, cuts, visit);
        }
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let total: f64 = widths.iter().sum();
    let length =
        words.iter().map(|w| w.chars().count()).sum::<usize>() + words.len().saturating_sub(1);
    let line_text = |cuts: &[usize], k: usize| words[cuts[k]..cuts[k + 1]].join(" ");
    let mut cuts = vec![0usize; widths.len() + 1];
    cuts[widths.len()] = words.len();
    let mut best: Option<(i64, f64, Vec<usize>)> = None;
    let mut visit = |cuts: &[usize]| {
        let (mut size, mut deviation, mut so_far, mut share) = (PLAIN_MAX_PT, 0.0, 0usize, 0.0);
        for (k, width) in widths.iter().enumerate() {
            let line = line_text(cuts, k);
            let chars = line.chars().count();
            size = size.min(fit_line(sheet, *width, &line));
            so_far += chars;
            share += width / total;
            deviation += (so_far as f64 - share * length as f64).abs();
        }
        // Sizes within a hundredth of a point are the same size.
        let rank = (size * 100.0).floor() as i64;
        if best
            .as_ref()
            .is_none_or(|(r, d, _)| rank > *r || (rank == *r && deviation < *d))
        {
            best = Some((rank, deviation, cuts.to_vec()));
        }
    };
    walk(1, words.len(), &mut cuts, &mut visit);
    let cuts = best.map(|(_, _, cuts)| cuts).unwrap_or(cuts);
    let lines: Vec<String> = (0..widths.len()).map(|k| line_text(&cuts, k)).collect();
    let size = lines
        .iter()
        .zip(widths)
        .map(|(line, width)| fit_line(sheet, *width, line))
        .fold(PLAIN_MAX_PT, f64::min);
    (lines, size)
}

/// For each stacked-line item whose text overflows a line's boxes as the
/// print map laid it out, how each of its cells prints: re-split over the
/// boxes while the whole text fits them, else plain type across its lines,
/// shrunk until it fits. An item whose lines all fit their boxes prints
/// exactly as its keys say (no entry).
fn line_group_plan(
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &WriterCells,
    sheet: &SheetType,
) -> BTreeMap<String, CellFill> {
    let mut plan = BTreeMap::new();
    for group in line_groups(html, cells) {
        // As the fill prints a text join: in BIR capital letters.
        let values: Vec<String> = group
            .iter()
            .map(|line| {
                let value = fields.get(&line.key).map(String::as_str).unwrap_or("");
                if is_email_key(&line.key) {
                    value.to_string()
                } else {
                    value.to_ascii_uppercase()
                }
            })
            .collect();
        let slots: Vec<usize> = group.iter().map(|line| line.slots).collect();
        if values
            .iter()
            .zip(&slots)
            .all(|(value, n)| value.chars().count() <= *n)
        {
            continue;
        }
        let text = join_lines(&values, &slots).trim().to_string();
        let boxed = split_lines(&text, &slots);
        let fits = boxed
            .iter()
            .zip(&slots)
            .all(|(line, n)| line.trim_end().chars().count() <= *n);
        if fits {
            for (line, part) in group.iter().zip(boxed) {
                plan.insert(
                    line.cell.clone(),
                    CellFill::Boxes(line.key.clone(), part.trim_end().to_string()),
                );
            }
            continue;
        }
        let widths: Vec<f64> = group.iter().map(|line| line.width).collect();
        let (parts, size) = plain_lines(&text, &widths, sheet);
        for (line, part) in group.iter().zip(parts) {
            plan.insert(line.cell.clone(), CellFill::Plain(part, size));
        }
    }
    plan
}

/// Writer key `name` printed into input `input_name`: a stamped `name=`
/// (`via_cell` false) or a writer-cells join target.
struct TextTarget<'a> {
    name: &'a str,
    input_name: &'a str,
    value: &'a str,
    via_cell: bool,
    caption: Option<&'a str>,
    /// The sheet's type, which a value longer than its boxes is measured in.
    sheet: &'a SheetType,
}

/// Whether a cell is a tax-rate box (percentage rate, DST rate like 2000OT item 16,
/// or excise unit rate) that RULES 4 centers and never right-aligns as money.
fn is_rate_cell(writer_key: &str, caption: Option<&str>) -> bool {
    if writer_key.starts_with("derived:") {
        return false;
    }
    let Some(caption) = caption else {
        return false;
    };
    let kl = writer_key.to_ascii_lowercase();
    let pl = caption.to_ascii_lowercase();
    if [
        "export", "taxable", "due", "stock", "payment", "excise", "vat", "amount", "income",
    ]
    .iter()
    .any(|bad| kl.contains(bad))
    {
        return false;
    }
    if [
        "amount",
        "income",
        "tax due",
        "payment",
        "export",
        "taxable",
        "stock",
        "(whole)",
        "(tenths)",
        "(decimals)",
    ]
    .iter()
    .any(|bad| pl.contains(bad))
    {
        return false;
    }
    pl.contains("tax rate")
        || pl.contains("schedule 2 rate")
        || pl.contains("schedule 3 rate")
        || pl.ends_with(" rate")
}

fn is_percentage_rate_cell(writer_key: &str, caption: Option<&str>) -> bool {
    if writer_key.starts_with("frm2200") || writer_key.starts_with("frm2000") {
        return false;
    }
    is_rate_cell(writer_key, caption)
}

/// Whether `writer_key`'s cell (printed caption `caption`) holds an
/// identifier, a TIN (or a part of one: its digit groups, its branch code)
/// or an ATC. An identifier is never shortened on the page: like an amount
/// it shrinks to its cell's room, with no 7.5 pt floor, and never ends in
/// "..." with the whole on the additional sheet (a cut TIN or ATC names a
/// different taxpayer or tax). Long free text keeps the long-text rule.
///
/// Read from the writer key's own name, its last `:` segment (a cloned
/// continuation key, `<key>__cont_<cell>`, by its original key): `TIN` or
/// `ATC` anywhere in capitals (`SellerTIN0`, `txtTINE1`, `drpATCCode1`,
/// `Pg2Sc1ATC1`); `Tin` or `Atc` as a word (`txtTin1`, `SignatoryTin1`,
/// `txtAtcCd1`, `drpPg1I5AtcOther`, `_Atc`); `tin` or `atc` as a
/// lowercase word (`derived:tin_digits`, `buyer_tin`, `tinA`, `sched1_atc1`,
/// `tp_atc`); and a branch code (`txtBranchCode`, `derived:att_branch`).
/// Writer keys that say none of that (2200AN `txtXGOthers1`, 2553 `txt18B`)
/// are read from the caption the sheet prints for the cell: it names
/// the cell as a TIN or ATC in its last word (`... row 1, ATC`, `18B ATC`;
/// a trailing note in brackets aside). An ATC description is text, not a
/// code, and a header holding a TIN and a name (`derived:page_header`) is
/// a line of text.
fn is_identifier_cell(writer_key: &str, caption: Option<&str>) -> bool {
    let key = writer_key.split("__cont_").next().unwrap_or(writer_key);
    let last = key.rsplit(':').next().unwrap_or(key);
    let lower = last.to_ascii_lowercase();
    // A description (`txtATCDesc`, `tp_atc_description`), a TIN-and-name
    // header, or a check box or radio button (`chkTIN`, `rdoPg1I5Atc`,
    // `optATC_1`, `rbATC_1`), which holds a tick, not an identifier.
    let control = ["chk", "rdo", "opt", "rb"].iter().any(|prefix| {
        last.strip_prefix(prefix)
            .and_then(|rest| rest.bytes().next())
            .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    });
    if lower.contains("desc") || lower.contains("page_header") || control {
        return false;
    }
    if lower.contains("branch") {
        return true;
    }
    let bytes = last.as_bytes();
    // `TIN` or `ATC` in capitals as a word of its own: after the start, a
    // non-capital, or a one-letter capital prefix (`I18STIN1`, the
    // spouse's TIN), never inside a longer capital run (`ACCOUNTING`); and
    // before the end, a digit, `_`, `of` (`TINofSignatory`), `Code`/`Cd`
    // (`drpATCCode1`, `txtAtcCd1`), or one capital letter that starts no
    // word (`txtTINE1`, `txtTINC1`, `drpPg1I5ATCR1`), never a word of its
    // own (`TINStatus`).
    let capitals = |needle: &str| {
        last.match_indices(needle).any(|(at, _)| {
            let upper = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_uppercase);
            let before = at == 0 || !upper(at - 1) || at == 1 || !upper(at - 2);
            let rest = &last[at + needle.len()..];
            let r = rest.as_bytes();
            let after = rest.is_empty()
                || r[0].is_ascii_digit()
                || r[0] == b'_'
                || (rest.starts_with("of")
                    && r.get(2)
                        .is_none_or(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
                || rest.starts_with("Code")
                || rest.starts_with("Cd")
                || (r[0].is_ascii_uppercase() && r.get(1).is_none_or(|b| !b.is_ascii_lowercase()));
            before && after
        })
    };
    if capitals("TIN") || capitals("ATC") {
        return true;
    }
    let word_at = |at: usize, len: usize, lowercase: bool| {
        let before_ok = !lowercase || at == 0 || bytes[at - 1] == b'_';
        let after = bytes.get(at + len).copied();
        let after_ok = match after {
            None => true,
            Some(b) if lowercase => b == b'_' || b.is_ascii_uppercase() || b.is_ascii_digit(),
            Some(b) => !b.is_ascii_lowercase(),
        };
        before_ok && after_ok
    };
    for (needle, lowercase) in [("Tin", false), ("Atc", false), ("tin", true), ("atc", true)] {
        let mut from = 0;
        while let Some(rel) = last[from..].find(needle) {
            let at = from + rel;
            if word_at(at, needle.len(), lowercase) {
                return true;
            }
            from = at + 1;
        }
    }
    let Some(caption) = caption else {
        return false;
    };
    let caption = match caption
        .trim()
        .strip_suffix(')')
        .and_then(|c| c.rfind('(').map(|at| &c[..at]))
    {
        Some(head) => head.trim(),
        None => caption.trim(),
    };
    !caption.to_ascii_lowercase().contains("desc")
        && caption
            .rsplit(|c: char| !c.is_ascii_alphanumeric())
            .find(|word| !word.is_empty())
            .is_some_and(|word| word == "TIN" || word == "ATC")
}

/// Whether a plain cell or rectangle holds a short code (ATC, treaty code,
/// tax type code, seq no) that Rule 4 centers.
fn is_short_code_cell(writer_key: &str, caption: Option<&str>, value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 12 {
        return false;
    }
    let kl = writer_key.to_ascii_lowercase();
    let tail = kl.rsplit(':').next().unwrap_or(&kl);
    let pl = caption.unwrap_or("").to_ascii_lowercase();
    if tail.contains("desc")
        || tail.contains("name")
        || tail.contains("nature")
        || tail.contains("addr")
        || pl.contains("description")
        || pl.contains("nature")
        || pl.contains("name")
    {
        return false;
    }
    tail.contains("atc")
        || tail.contains("treaty")
        || tail.contains("taxtype")
        || tail.contains("taxcode")
        || pl.ends_with(" atc")
        || pl == "atc"
        || pl.contains("tax type code")
        || pl.contains("treaty code")
}

fn has_preprinted_percent_beside(html: &str, cell: &str) -> bool {
    let Some((left, top, width, height)) = cell_box_pt(html, cell) else {
        return false;
    };
    let Some((open, _)) = cell_tag(html, cell) else {
        return false;
    };
    let page_start = html[..open].rfind("<div class=\"page ").unwrap_or(0);
    let page_end = html[open..]
        .find("<div class=\"page ")
        .map(|rel| open + rel)
        .unwrap_or(html.len());
    let page = &html[page_start..page_end];
    let mut cursor = 0;
    while let Some(rel) = page[cursor..].find("<div class=\"t\"") {
        let start = cursor + rel;
        let Some(tag_end_rel) = page[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_end_rel;
        let Some(close_rel) = page[tag_end + 1..].find("</div>") else {
            break;
        };
        let text = page[tag_end + 1..tag_end + 1 + close_rel].trim();
        if text == "%" {
            let t_tag = &page[start..tag_end];
            if let (Some(tl), Some(tt)) =
                (parse_style_pt(t_tag, "left"), parse_style_pt(t_tag, "top"))
            {
                if (tt - top).abs() <= height.max(14.0) && tl >= left && tl <= left + width + 22.0 {
                    return true;
                }
            }
        }
        cursor = tag_end + 1 + close_rel + "</div>".len();
    }
    false
}

fn format_percentage_rate(raw: &str, preprinted_percent: bool) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.')
        || !trimmed.chars().any(|c| c.is_ascii_digit())
        || trimmed.matches('.').count() > 1
    {
        return None;
    }
    let normalized = if let Some((whole, frac)) = trimmed.split_once('.') {
        let frac = frac.trim_end_matches('0');
        let whole = if whole.is_empty() { "0" } else { whole };
        if frac.is_empty() {
            whole.to_string()
        } else {
            format!("{whole}.{frac}")
        }
    } else {
        trimmed.to_string()
    };
    if preprinted_percent {
        Some(normalized)
    } else {
        Some(format!("{normalized} %"))
    }
}

fn parse_style_pt(tag: &str, prop: &str) -> Option<f64> {
    let needle = format!("{prop}:");
    let start = tag.find(&needle)? + needle.len();
    tag[start..].split("pt").next()?.trim().parse().ok()
}

fn cell_box_pt(html: &str, cell: &str) -> Option<(f64, f64, f64, f64)> {
    let (_, tag) = cell_tag(html, cell)?;
    let w = parse_style_pt(tag, "width")?;
    let h = parse_style_pt(tag, "height").unwrap_or(14.0);
    let l = parse_style_pt(tag, "left").unwrap_or(0.0);
    let t = parse_style_pt(tag, "top").unwrap_or(0.0);
    Some((l, t, w, h))
}

fn parse_svg_path_bbox(d: &str) -> Option<(f64, f64, f64, f64)> {
    let mut nums = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_digit() || ch == '.' || (ch == '-' && cur.is_empty()) {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                if let Ok(v) = cur.parse::<f64>() {
                    nums.push(v);
                }
                cur.clear();
            }
            if ch == '-' {
                cur.push(ch);
            }
        }
    }
    if !cur.is_empty() {
        if let Ok(v) = cur.parse::<f64>() {
            nums.push(v);
        }
    }
    if nums.len() < 4 {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pair in nums.as_chunks::<2>().0 {
        min_x = min_x.min(pair[0]);
        max_x = max_x.max(pair[0]);
        min_y = min_y.min(pair[1]);
        max_y = max_y.max(pair[1]);
    }
    Some((min_x, max_x, min_y, max_y))
}

/// Detects a preprinted decimal mark drawn inside `cell` on its page using
/// sheet geometry (SVG dot path/circle or text `.` in the lower-right
/// centavo zone). Returns `(dot_rel_x, cell_width)` when present.
fn preprinted_decimal_mark(html: &str, cell: &str) -> Option<(f64, f64)> {
    let (left, top, width, height) = cell_box_pt(html, cell)?;
    if width < 30.0 {
        return None;
    }
    let (open, _) = cell_tag(html, cell)?;
    let page_start = html[..open].rfind("<div class=\"page ").unwrap_or(0);
    let page_end = html[open..]
        .find("<div class=\"page ")
        .map(|rel| open + rel)
        .unwrap_or(html.len());
    let page = &html[page_start..page_end];
    let in_decimal_zone = |cx: f64, cy: f64| -> bool {
        let right_gap = (left + width) - cx;
        cx >= left + width * 0.5
            && cx <= left + width - 8.0
            && (8.0..=38.0).contains(&right_gap)
            && cy >= top + height * 0.35
            && cy <= top + height + 1.0
    };

    let mut cursor = 0;
    while let Some(rel) = page[cursor..].find("<path ") {
        let start = cursor + rel;
        let Some(tag_end_rel) = page[start..].find('>') else {
            break;
        };
        let p_tag = &page[start..start + tag_end_rel];
        if let Some(d_at) = p_tag.find("d=\"") {
            let d_rest = &p_tag[d_at + 3..];
            if let Some(d_end) = d_rest.find('"') {
                if let Some((x0, x1, y0, y1)) = parse_svg_path_bbox(&d_rest[..d_end]) {
                    let bw = x1 - x0;
                    let bh = y1 - y0;
                    if (0.4..=4.0).contains(&bw) && (0.4..=4.0).contains(&bh) {
                        let cx = (x0 + x1) * 0.5;
                        let cy = (y0 + y1) * 0.5;
                        if in_decimal_zone(cx, cy) {
                            return Some((cx - left, width));
                        }
                    }
                }
            }
        }
        cursor = start + tag_end_rel + 1;
    }

    let mut cursor = 0;
    while let Some(rel) = page[cursor..].find("<circle ") {
        let start = cursor + rel;
        let Some(tag_end_rel) = page[start..].find('>') else {
            break;
        };
        let c_tag = &page[start..start + tag_end_rel];
        let parse_attr = |k: &str| -> Option<f64> {
            let needle = format!("{k}=\"");
            let s = c_tag.find(&needle)? + needle.len();
            c_tag[s..].split('"').next()?.parse().ok()
        };
        if let (Some(cx), Some(cy), Some(r)) = (parse_attr("cx"), parse_attr("cy"), parse_attr("r"))
        {
            if (0.2..=2.5).contains(&r) && in_decimal_zone(cx, cy) {
                return Some((cx - left, width));
            }
        }
        cursor = start + tag_end_rel + 1;
    }

    let mut cursor = 0;
    while let Some(rel) = page[cursor..].find("<div class=\"t\"") {
        let start = cursor + rel;
        let Some(tag_end_rel) = page[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_end_rel;
        let Some(close_rel) = page[tag_end + 1..].find("</div>") else {
            break;
        };
        if page[tag_end + 1..tag_end + 1 + close_rel].trim() == "." {
            let t_tag = &page[start..tag_end];
            if let (Some(tl), Some(tt)) =
                (parse_style_pt(t_tag, "left"), parse_style_pt(t_tag, "top"))
            {
                if in_decimal_zone(tl + 1.0, tt + 3.0) {
                    return Some((tl + 1.0 - left, width));
                }
            }
        }
        cursor = tag_end + 1 + close_rel + "</div>".len();
    }
    None
}

/// Inserts thousands commas into an unsigned ASCII digit string.
fn group_thousands(digits: &str) -> String {
    let trimmed = digits.trim_start_matches('0');
    let d = if trimmed.is_empty() { "0" } else { trimmed };
    let mut out = String::with_capacity(d.len() + d.len() / 3);
    for (i, ch) in d.chars().enumerate() {
        let rem = d.len() - i;
        if i > 0 && rem.is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Formats an amount for a converted plain-rectangle cell (Rule 2):
/// thousands commas, `.cents` when `cents` is `Some`, and brackets/minus
/// per `style`.
fn format_rect_amount(
    negative: bool,
    peso_digits: &str,
    cents: Option<&str>,
    style: NegativeStyle,
) -> String {
    let grouped = group_thousands(peso_digits);
    let body = match cents {
        Some(c) => format!("{grouped}.{c}"),
        None => grouped,
    };
    if !negative || (peso_digits.chars().all(|c| c == '0') && cents.unwrap_or("00") == "00") {
        return body;
    }
    style.sign(&body)
}

/// Formats a raw numeric amount string (e.g. from a plain/comb amount key)
/// with thousands commas while preserving whether it carries decimals.
fn format_comb_amount_for_rect(raw: &str, style: NegativeStyle) -> String {
    let trimmed = raw.trim();
    let (negative, inner) =
        if let Some(s) = trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            (true, s.trim())
        } else if let Some(s) = trimmed.strip_prefix('-') {
            (true, s.trim())
        } else {
            (false, trimmed)
        };
    let clean: String = inner.chars().filter(|&c| c != ',').collect();
    if let Some((p, c)) = clean.split_once('.') {
        if !p.is_empty()
            && p.bytes().all(|b| b.is_ascii_digit())
            && c.bytes().all(|b| b.is_ascii_digit())
        {
            return format_rect_amount(negative, p, Some(c), style);
        }
    } else if !clean.is_empty() && clean.bytes().all(|b| b.is_ascii_digit()) {
        return format_rect_amount(negative, &clean, None, style);
    }
    trimmed.to_string()
}

/// Removes interior separators (hairlines, thick group marks, caps, grey
/// decimal blocks, preprinted dots, centavo dividers) from the SVG and text
/// layers of every page for every cell converted into a plain rectangle
/// (`data-rect-box="L,T,R,B"`). Outer borders and neighbouring grid lines are
/// preserved; vertical lines that span neighbouring rows are split around the
/// cell's vertical extent rather than deleted.
fn strip_converted_rect_separators(html: &str) -> String {
    if !html.contains("data-rect-box=\"") {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<div class=\"page ") {
        let page_start = pos + rel;
        out.push_str(&html[pos..page_start]);
        let page_end = html[page_start + 1..]
            .find("<div class=\"page ")
            .map(|r| page_start + 1 + r)
            .unwrap_or(html.len());
        let page = &html[page_start..page_end];
        out.push_str(&strip_page_rect_separators(page));
        pos = page_end;
    }
    out.push_str(&html[pos..]);
    out
}

fn strip_page_rect_separators(page: &str) -> String {
    let mut boxes: Vec<(String, f64, f64, f64, f64)> = Vec::new();
    let needle = "data-rect-box=\"";
    let mut cur = 0;
    while let Some(rel) = page[cur..].find(needle) {
        let start = cur + rel + needle.len();
        let Some(end_rel) = page[start..].find('"') else {
            break;
        };
        let tag_start = page[..cur + rel].rfind('<').unwrap_or(0);
        let tag_end = start + end_rel + page[start + end_rel..].find('>').unwrap_or(0);
        let owner = attr(&page[tag_start..=tag_end], "data-overflow-of")
            .unwrap_or("")
            .to_string();
        let parts: Vec<f64> = page[start..start + end_rel]
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.len() == 4 {
            boxes.push((owner, parts[0], parts[1], parts[2], parts[3]));
        }
        cur = start + end_rel + 1;
    }
    if boxes.is_empty() {
        return page.to_string();
    }

    let parse_f64_attr = |tag: &str, k: &str| -> Option<f64> {
        let n = format!(" {k}=\"");
        let s = tag.find(&n)? + n.len();
        tag[s..].split('"').next()?.parse().ok()
    };

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut hidden_rules_by_cell: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // 1. SVG <rect .../> inside <svg class="rl">
    if let (Some(svg_start), Some(svg_end_rel)) = (page.find("<svg "), page.find("</svg>")) {
        let svg_end = svg_end_rel + "</svg>".len();
        let mut sc = svg_start;
        while sc < svg_end {
            let Some(rel) = page[sc..svg_end].find("<rect ") else {
                break;
            };
            let rs = sc + rel;
            let Some(re_rel) = page[rs..svg_end].find('>') else {
                break;
            };
            let re = rs + re_rel + 1;
            let rtag = &page[rs..re];
            sc = re;
            let (Some(x), Some(y), Some(w), Some(h)) = (
                parse_f64_attr(rtag, "x"),
                parse_f64_attr(rtag, "y"),
                parse_f64_attr(rtag, "width"),
                parse_f64_attr(rtag, "height"),
            ) else {
                continue;
            };
            let rule_id = attr(rtag, "data-rule-id").map(str::to_string);
            // Check against every converted box on this page.
            let mut segments: Vec<(f64, f64)> = vec![(y, y + h)];
            let mut modified = false;
            for (owner, bl, bt, br, bb) in &boxes {
                let (bl, bt, br, bb) = (*bl, *bt, *br, *bb);
                let bw = br - bl;
                // Strictly inside horizontally and not a wide horizontal grid border
                if x > bl + 1.2 && x + w < br - 1.2 && w < bw * 0.5 {
                    let mut next = Vec::new();
                    let mut touched_this_box = false;
                    for (sy0, sy1) in segments {
                        if sy1 <= bt - 1.5 || sy0 >= bb + 2.5 {
                            next.push((sy0, sy1));
                        } else if sy0 >= bt - 1.5 && sy1 <= bb + 2.5 {
                            // Purely owned by this cell: remove completely.
                            modified = true;
                            touched_this_box = true;
                        } else {
                            // Extends into a neighbouring row above or below: split cleanly.
                            modified = true;
                            touched_this_box = true;
                            if sy0 < bt + 0.5 {
                                next.push((sy0, bt + 0.5));
                            }
                            if sy1 > bb - 0.5 {
                                next.push((bb - 0.5, sy1));
                            }
                        }
                    }
                    if touched_this_box && !owner.is_empty() {
                        if let Some(ref rid) = rule_id {
                            hidden_rules_by_cell
                                .entry(owner.clone())
                                .or_default()
                                .push(rid.clone());
                        }
                    }
                    segments = next;
                }
            }
            if modified {
                let mut rebuilt = String::new();
                for (sy0, sy1) in segments {
                    let sh = sy1 - sy0;
                    if sh < 0.2 {
                        continue;
                    }
                    let seg = replace_svg_attr(rtag, "y", &format!("{sy0:.2}"));
                    let seg = replace_svg_attr(&seg, "height", &format!("{sh:.2}"));
                    rebuilt.push_str(&seg);
                }
                replacements.push((rs, re, rebuilt));
            }
        }

        // 2. Small SVG <path .../> dots inside converted boxes
        let mut pc = svg_start;
        while pc < svg_end {
            let Some(rel) = page[pc..svg_end].find("<path ") else {
                break;
            };
            let ps = pc + rel;
            let Some(pe_rel) = page[ps..svg_end].find('>') else {
                break;
            };
            let pe = ps + pe_rel + 1;
            let ptag = &page[ps..pe];
            pc = pe;
            if let Some(d_at) = ptag.find("d=\"") {
                let d_rest = &ptag[d_at + 3..];
                if let Some(d_end) = d_rest.find('"') {
                    if let Some((x0, x1, y0, y1)) = parse_svg_path_bbox(&d_rest[..d_end]) {
                        if (x1 - x0) <= 5.0 && (y1 - y0) <= 5.0 {
                            let cx = (x0 + x1) * 0.5;
                            let cy = (y0 + y1) * 0.5;
                            if boxes.iter().any(|(_, bl, bt, br, bb)| {
                                cx > bl + 1.2 && cx < br - 1.2 && cy >= bt - 1.0 && cy <= bb + 2.0
                            }) {
                                replacements.push((ps, pe, String::new()));
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Preprinted "." text elements inside converted boxes
    let mut tc = 0;
    while let Some(rel) = page[tc..].find("<div class=\"t\"") {
        let ts = tc + rel;
        let Some(tag_end_rel) = page[ts..].find('>') else {
            break;
        };
        let tag_end = ts + tag_end_rel;
        let Some(close_rel) = page[tag_end + 1..].find("</div>") else {
            break;
        };
        let te = tag_end + 1 + close_rel + "</div>".len();
        tc = te;
        if matches!(
            page[tag_end + 1..tag_end + 1 + close_rel].trim(),
            "." | "\u{25cf}" | "\u{2022}" | "\u{00b7}"
        ) {
            let t_tag = &page[ts..tag_end];
            if let (Some(tl), Some(tt)) =
                (parse_style_pt(t_tag, "left"), parse_style_pt(t_tag, "top"))
            {
                if boxes.iter().any(|(_, bl, bt, br, bb)| {
                    tl > bl + 1.0 && tl < br - 1.0 && tt >= bt - 2.0 && tt < bb - 1.5
                }) {
                    replacements.push((ts, te, String::new()));
                }
            }
        }
    }

    // 4. Record hidden SVG rule IDs on each converted cell's <div> so band
    // shift and row cloning can mirror the separator removal.
    for (cell_id, rules) in hidden_rules_by_cell {
        if rules.is_empty() {
            continue;
        }
        let needle = format!("<div id=\"{cell_id}\"");
        if let Some(open) = page.find(&needle) {
            if let Some(end_rel) = page[open..].find('>') {
                let close = open + end_rel;
                let joined = rules.join(",");
                replacements.push((close, close, format!(" data-hidden-svg-rules=\"{joined}\"")));
            }
        }
    }

    if replacements.is_empty() {
        page.to_string()
    } else {
        apply_replacements(page, replacements)
    }
}

fn replace_svg_attr(tag: &str, name: &str, new_val: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(at) = tag.find(&needle) else {
        return tag.to_string();
    };
    let val_start = at + needle.len();
    let Some(val_end_rel) = tag[val_start..].find('"') else {
        return tag.to_string();
    };
    format!(
        "{}{new_val}{}",
        &tag[..val_start],
        &tag[val_start + val_end_rel..]
    )
}

/// One text value into one cell: a comb gets a character per slot, a plain
/// box the whole value.
fn fill_text_input(
    html: &str,
    tags: &[InputTag<'_>],
    indices: &[usize],
    target: &TextTarget<'_>,
    replacements: &mut Vec<(usize, usize, String)>,
    forced: &BTreeSet<String>,
) {
    let TextTarget {
        name,
        input_name,
        value,
        via_cell,
        caption,
        sheet,
    } = *target;
    // Writer-cell text joins print as BIR CAPITAL LETTERS. Stamped
    // TIN/branch (`via_cell` false) keep the writer map as-is.
    // Like the official capital(), email inputs keep their typed casing.
    let print_value = (via_cell && !is_email_key(name)).then(|| value.to_ascii_uppercase());
    let print = print_value.as_deref().unwrap_or(value);
    let comb = indices.iter().any(|&index| tags[index].slot.is_some());
    if comb {
        let ordered = ordered_comb_indices(tags, indices);
        // Uniform sections (#477): a comb of a group another amount of
        // overflowed prints as a line even when its own amount fits, so the
        // column stays uniform.
        // The forced set never takes an identifier comb (a TIN, an ATC, a
        // receipt number): it is not an amount, so it keeps its boxes even
        // when an amount of its column overflows.
        if forced.contains(input_name)
            && !is_identifier_cell(name, caption)
            && is_amount_comb_value(print)
            && !print.is_empty()
        {
            let formatted = format_comb_amount_for_rect(print, sheet.negative);
            let writer = via_cell.then_some(print);
            amount_rect(
                html,
                tags,
                &ordered,
                (input_name, None),
                &formatted,
                writer,
                sheet,
                replacements,
            );
            return;
        }
        // Attachment counts occupy the sheet's existing digit boxes.
        // Pad the displayed count only; the upload value stays as written.
        if is_attachment_count_key(name)
            && !print.is_empty()
            && print.bytes().all(|b| b.is_ascii_digit())
            && print.len() < ordered.len()
        {
            let padded = format!("{print:0>width$}", width = ordered.len());
            for (offset, (index, ch)) in ordered.into_iter().zip(padded.chars()).enumerate() {
                let writer = (via_cell && offset == 0).then_some(print);
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, &ch.to_string(), writer),
                ));
            }
            return;
        }
        // An amount into a comb drawn with a decimal-point column (one gap in
        // the slot indices, exactly two slots after it): right-align the peso
        // digits into the slots before the point and put the two centavo
        // digits after it. Spreading the text left-to-right put `0.00` as
        // `0 . 0 0` from the first box (#print-audit decimal mapping).
        if !name.starts_with("derived:") {
            match money_comb_values(tags, &ordered, print, sheet.negative) {
                MoneyComb::Boxes(writes) => {
                    for (offset, (index, ch)) in writes.into_iter().enumerate() {
                        let writer = (via_cell && offset == 0).then_some(print);
                        replacements.push((
                            tags[index].start,
                            tags[index].end,
                            set_value(tags[index].tag, &ch, writer),
                        ));
                    }
                    return;
                }
                // Too many digits for the boxes before the point, or a
                // bracketed negative: one rectangle, never the digits spread
                // over the centavo boxes.
                MoneyComb::Rect(text) => {
                    let writer = via_cell.then_some(print);
                    amount_rect(
                        html,
                        tags,
                        &ordered,
                        (input_name, None),
                        &text,
                        writer,
                        sheet,
                        replacements,
                    );
                    return;
                }
                MoneyComb::NotMoney => {}
            }
        }
        let chars: Vec<char> = print.chars().collect();
        // A 14-slot TIN header comb whose last 5 slots are preprinted `00000`
        // (9 `<input>` slots + 5 empty `<div class="s">` slots, e.g. 1701Q p2c5,
        // 1701 p2c5/p3c5/p4c5): put the 9 TIN digits into slots 0..8 without
        // overflowing into a crushed rectangle, and paint any non-zero branch
        // code into slots 9..13 over the preprinted `00000`.
        if ordered.len() == 9
            && chars.len() > 9
            && chars.len() <= 14
            && chars.iter().all(char::is_ascii_digit)
            && fill_nine_plus_five_tin_comb(
                html,
                tags,
                &ordered,
                input_name,
                print,
                via_cell,
                replacements,
            )
        {
            return;
        }
        let amount_comb = is_amount_comb_value(print)
            && (is_amount_comb_key(name) || is_formatted_money_amount(print));
        // An amount longer than its boxes, or with centavos in an amount comb
        // that has no point column (a whole-peso comb), prints whole as one
        // rectangle: never a character dropped past the last box, never the
        // centavos spread over digit boxes. (A comb built for a point, such
        // as 1701-MS Item 31's, keeps its point in its box.)
        let centavos_in_peso_comb = is_amount_comb_key(name) && print.contains('.');
        if amount_comb && (chars.len() > ordered.len() || centavos_in_peso_comb) {
            let formatted = format_comb_amount_for_rect(print, sheet.negative);
            let writer = via_cell.then_some(print);
            amount_rect(
                html,
                tags,
                &ordered,
                (input_name, None),
                &formatted,
                writer,
                sheet,
                replacements,
            );
            return;
        }
        // A value longer than its boxes is never spread with its tail lost,
        // stamped (`via_cell` false) or not: it prints as one line over the
        // comb. An identifier (a TIN, an ATC) or a figure (digits, a sign,
        // separators) prints whole on that line, shrunk to fit, never cut;
        // other text keeps the long-text rule (two lines, 7.5 pt, "...",
        // the whole on the additional sheet).
        if chars.len() > ordered.len() {
            let identifier = is_identifier_cell(name, caption);
            let line = if identifier || is_amount_comb_value(print) {
                comb_overflow_line_whole(html, input_name, print, sheet, identifier)
            } else {
                comb_overflow_line(html, input_name, print, sheet)
            };
            match line {
                Some((at, line)) => {
                    replacements.push((at, at, line));
                    for (offset, &index) in ordered.iter().enumerate() {
                        let writer = (via_cell && offset == 0).then_some(print);
                        replacements.push((
                            tags[index].start,
                            tags[index].end,
                            set_value(tags[index].tag, "", writer),
                        ));
                    }
                }
                // No geometry to lay a line on: the boxes stay empty and
                // the first carries the whole value to the additional sheet.
                None => amount_rect(
                    html,
                    tags,
                    &ordered,
                    (input_name, None),
                    print,
                    via_cell.then_some(print),
                    sheet,
                    replacements,
                ),
            }
            return;
        }
        for (offset, index) in ordered.into_iter().enumerate() {
            let ch = chars
                .get(offset)
                .copied()
                .map(|c| c.to_string())
                .unwrap_or_default();
            let writer = (via_cell && offset == 0).then_some(print);
            replacements.push((
                tags[index].start,
                tags[index].end,
                set_value(tags[index].tag, &ch, writer),
            ));
        }
    } else {
        // A split cell ([`SINGLE_CHARACTER_CELLS`]) holding no more
        // characters than boxes prints one per box at its class's size
        // (`split_single_character_cells`); a longer value prints once, as
        // a plain line measured across the whole cell, its boxes empty.
        let split =
            sheet.split.iter().any(|(cell, _)| *cell == input_name) && splits(tags, indices);
        let overflow = (split && !fits_one_per_box(print, indices.len()))
            .then(|| {
                let identifier = is_identifier_cell(name, caption);
                if identifier || is_amount_comb_value(print) {
                    comb_overflow_line_whole(html, input_name, print, sheet, identifier)
                } else {
                    comb_overflow_line(html, input_name, print, sheet)
                }
            })
            .flatten();
        if let Some((at, line)) = overflow {
            replacements.push((at, at, line));
            for (offset, &index) in indices.iter().enumerate() {
                let writer = (via_cell && offset == 0).then_some(print);
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, "", writer),
                ));
            }
            return;
        }
        let is_rate_cell = via_cell && !split && is_rate_cell(name, caption);
        let is_pct_rate_cell = is_rate_cell && is_percentage_rate_cell(name, caption);
        let preprinted_dot =
            if !is_rate_cell && via_cell && !split && is_formatted_money_amount(print) {
                preprinted_decimal_mark(html, input_name)
            } else {
                None
            };
        if let Some((dot_rel_x, cell_width)) = preprinted_dot {
            let (neg, raw_peso, raw_cents) = {
                let s = print.trim();
                let (neg, body) = match s.strip_prefix('-') {
                    Some(rest) => (true, rest),
                    None => (false, s),
                };
                let (p, c) = body.split_once('.').unwrap_or((body, "00"));
                (neg, p, c)
            };
            let (peso_text, cents_text) = if neg {
                match sheet.negative {
                    NegativeStyle::Parentheses => (format!("({raw_peso}"), format!("{raw_cents})")),
                    NegativeStyle::Minus => (format!("-{raw_peso}"), raw_cents.to_string()),
                }
            } else {
                (raw_peso.to_string(), raw_cents.to_string())
            };
            let peso_right = (cell_width - dot_rel_x + 1.8).max(10.0);
            let peso_width = (dot_rel_x - 3.0).max(10.0);
            let cents_left = (dot_rel_x + 1.8).min(cell_width - 8.0);
            for &index in indices {
                let orig_tag = tags[index].tag;
                let base_class = attr(orig_tag, "class").unwrap_or("fi");
                let peso_size =
                    fit_plain_text(sheet, peso_width, 12.0, orig_tag, &peso_text, false, true)
                        .map(|f| f.font_size_pt);
                let size_css = peso_size
                    .map(|sz| format!("font-size:{sz:.2}pt;"))
                    .unwrap_or_default();
                let blank_orig = set_value(orig_tag, "", Some(print));
                let split_pair = format!(
                    "{blank_orig}<input type=\"text\" class=\"{base_class}\" name=\"{input_name}-peso\" readonly value=\"{}\" style=\"position:absolute;top:0.75pt;bottom:0.75pt;left:1pt;right:{peso_right:.2}pt;{size_css}text-align:right;justify-content:flex-end;padding-right:1pt\" data-decimal-of=\"{input_name}\" data-decimal-part=\"peso\"><input type=\"text\" class=\"{base_class}\" name=\"{input_name}-cents\" readonly value=\"{}\" style=\"position:absolute;top:0.75pt;bottom:0.75pt;left:{cents_left:.2}pt;right:1pt;{size_css}text-align:left;justify-content:flex-start;padding-left:1pt\" data-decimal-of=\"{input_name}\" data-decimal-part=\"cents\">",
                    html_escape(&peso_text),
                    html_escape(&cents_text),
                );
                replacements.push((tags[index].start, tags[index].end, split_pair));
            }
            return;
        }
        let formatted_rate = is_pct_rate_cell
            .then(|| format_percentage_rate(print, has_preprinted_percent_beside(html, input_name)))
            .flatten();
        let center_short_code = !is_rate_cell
            && via_cell
            && !split
            && is_short_code_cell(name, caption, formatted_rate.as_deref().unwrap_or(print));
        let formatted_plain_whole_peso = (!is_rate_cell
            && !center_short_code
            && via_cell
            && !split
            && is_whole_peso_writer_key(name)
            && is_amount_typed_cell(name, caption)
            && (is_formatted_whole_peso_amount(print) || is_formatted_money_amount(print)))
        .then(|| format_plain_whole_peso_with_commas(print, sheet.negative))
        .flatten();
        let display = formatted_rate
            .as_deref()
            .or(formatted_plain_whole_peso.as_deref())
            .unwrap_or(print);
        // A value longer than its plain input wraps onto two lines when the
        // cell is at least 18 pt tall, shrinks down to PLAIN_TEXT_MIN_PT, and
        // truncates with "..." + an overflow warning if it still does not fit;
        // one that fits keeps its bytes.
        let dims = via_cell
            .then(|| plain_cell_dimensions(html, input_name))
            .flatten();
        let right_align_numeric = !is_rate_cell
            && !center_short_code
            && via_cell
            && !split
            && ((is_amount_typed_cell(name, caption)
                && (is_formatted_money_amount(print) || is_formatted_whole_peso_amount(print)))
                || (is_quantity_typed_key(name) && is_formatted_numeric_quantity(print)));
        // A figure (an amount box, any value written as an amount with or
        // without decimals, `-200000` in a box whose caption names no
        // amount, or an identifier: a TIN or an ATC) is never wrapped or
        // cut: it shrinks until it fits.
        let figure = right_align_numeric
            || (!is_rate_cell
                && via_cell
                && !split
                && (is_formatted_money_amount(print) || is_formatted_whole_peso_amount(print)))
            || is_identifier_cell(name, caption);
        let identifier = is_identifier_cell(name, caption);
        for &index in indices {
            let writer = via_cell.then_some(print);
            let fit = dims.filter(|_| !split).and_then(|(width, height)| {
                fit_plain_text(
                    sheet,
                    width,
                    height,
                    tags[index].tag,
                    display,
                    !right_align_numeric,
                    figure,
                )
            });
            let tag = match &fit {
                Some(fit) => {
                    let mut tag = set_value(tags[index].tag, &fit.text, writer);
                    tag = with_font_size(&tag, fit.font_size_pt);
                    if fit.two_lines {
                        tag = with_style_declaration(
                            &tag,
                            &format!("line-height:{PLAIN_TWO_LINE_CSS_HEIGHT}"),
                        );
                    }
                    if let Some(v) = fit.tight_inset_v {
                        let (_, right, _, left) = inset_box(attr(&tag, "style").unwrap_or(""));
                        tag = with_style_declaration(
                            &tag,
                            &format!("inset:{v:.2}pt {right}pt {v:.2}pt {left}pt"),
                        );
                    }
                    if fit.truncated {
                        let insert_at = tag.rfind('>').unwrap_or(tag.len());
                        tag.insert_str(insert_at, " data-overflow-warning=\"text-truncated\"");
                    }
                    if figure && fit.font_size_pt < SMALL_WHOLE_BELOW_PT {
                        tag = without_writer_value(&tag);
                        let insert_at = tag.rfind('>').unwrap_or(tag.len());
                        tag.insert_str(
                            insert_at,
                            &small_whole_mark(fit.font_size_pt, identifier, &fit.text),
                        );
                    }
                    tag
                }
                None => set_value(tags[index].tag, display, writer),
            };
            let tag = if (is_rate_cell || center_short_code) && !display.is_empty() {
                with_style_declaration(&tag, "text-align:center;justify-content:center")
            } else if right_align_numeric {
                with_style_declaration(
                    &tag,
                    "text-align:right;justify-content:flex-end;padding-right:2pt",
                )
            } else {
                tag
            };
            replacements.push((tags[index].start, tags[index].end, tag));
        }
    }
}

/// Whether a cell's inputs `indices` are split one character per box: two
/// or more one-character inputs that are not a comb.
fn splits(tags: &[InputTag<'_>], indices: &[usize]) -> bool {
    indices.len() >= 2
        && indices.iter().all(|&index| {
            tags[index].slot.is_none() && attr(tags[index].tag, "maxlength") == Some("1")
        })
}

/// Whether `value` goes one character per box into a split cell of
/// `boxes` boxes: no more characters than boxes, letters and digits only.
fn fits_one_per_box(value: &str, boxes: usize) -> bool {
    value.chars().count() <= boxes && value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Attachment-count writer keys used by the reviewed frozen-sheet joins.
/// In particular, 1702RT calls its count PagesFilled. Split tens/units joins
/// already supply one digit each and do not need a different layout.
fn is_attachment_count_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.rsplit(':').next().unwrap_or(&key),
        "txtsheets"
            | "txtnosheets"
            | "txtnoofsheets"
            | "txtpg1i3nosheets"
            | "txtpg1i3noofsheets"
            | "txtpg1i4nosheets"
            | "txtpg1i4noofsheets"
            | "txti3sheets"
            | "txtpg1p2i23numofattachments"
            | "txtpg1pt2pagesfilled"
            | "attachments"
            | "attachments_tens"
            | "attachments_units"
    )
}

/// How an amount prints in a comb drawn with a decimal-point column.
#[derive(Debug, PartialEq)]
enum MoneyComb {
    /// `(slot tag index, char)`: the peso digits right-aligned before the
    /// point, the two centavo digits after it.
    Boxes(Vec<(usize, String)>),
    /// One plain rectangle across the comb holding this text: more peso
    /// digits than boxes, more than two decimals, or a negative the form
    /// brackets (its brackets do not go in digit boxes).
    Rect(String),
    /// The comb has no point column, or the value is not an amount.
    NotMoney,
}

/// How `value` prints into the point-column comb `ordered`, a negative in
/// the sheet's `style`.
fn money_comb_values(
    tags: &[InputTag<'_>],
    ordered: &[usize],
    value: &str,
    style: NegativeStyle,
) -> MoneyComb {
    let Some(slots) = ordered
        .iter()
        .map(|&i| tags[i].slot)
        .collect::<Option<Vec<usize>>>()
    else {
        return MoneyComb::NotMoney;
    };
    let gaps: Vec<usize> = (1..slots.len())
        .filter(|&k| slots[k] != slots[k - 1] + 1)
        .collect();
    let [gap] = gaps.as_slice() else {
        return MoneyComb::NotMoney;
    };
    if slots.len() - gap != 2 || *gap < 1 {
        return MoneyComb::NotMoney;
    }
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-' | '(' | ')' | ' '))
        || !trimmed.chars().any(|c| c.is_ascii_digit())
    {
        return MoneyComb::NotMoney;
    }
    let Some((negative, peso, cents)) = split_writer_money(trimmed) else {
        return if is_amount_comb_value(trimmed) {
            MoneyComb::Rect(format_comb_amount_for_rect(trimmed, style))
        } else {
            MoneyComb::NotMoney
        };
    };
    let zero = peso.chars().chain(cents.chars()).all(|c| c == '0');
    let rect = || MoneyComb::Rect(format_rect_amount(negative, &peso, Some(&cents), style));
    if negative && !zero && style == NegativeStyle::Parentheses {
        return rect();
    }
    let signed = signed_digits(negative, &peso, &cents, style);
    let Some(peso_values) = right_aligned_slot_values(*gap, &signed) else {
        return rect();
    };
    let mut out: Vec<(usize, String)> = ordered[..*gap].iter().copied().zip(peso_values).collect();
    out.extend(
        ordered[*gap..]
            .iter()
            .copied()
            .zip(cents.chars().map(|c| c.to_string())),
    );
    MoneyComb::Boxes(out)
}

/// An amount that does not go in its boxes: every slot of `slots` left
/// empty and one right-aligned plain rectangle laid over `cells` (a peso
/// comb, with its centavo comb if separate) holding `text` whole, its type
/// shrunk until it fits ([`comb_overflow_amount_union`]). Where the sheet
/// gives the cell no geometry to lay a rectangle on, the first slot is
/// marked `text-truncated` with the amount, which the additional sheet
/// then prints in full ([`crate::continuation::collect_truncated_entries`]):
/// the amount is never dropped silently.
#[allow(clippy::too_many_arguments)]
fn amount_rect(
    html: &str,
    tags: &[InputTag<'_>],
    slots: &[usize],
    cells: (&str, Option<&str>),
    text: &str,
    writer: Option<&str>,
    sheet: &SheetType,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    let (peso_cell, cent_cell) = cells;
    let line = comb_overflow_amount_union(html, peso_cell, cent_cell, text, sheet);
    let placed = line.is_some();
    if let Some((start, end, line)) = line {
        replacements.push((start, end, line));
    }
    for (offset, &index) in slots.iter().enumerate() {
        let first = offset == 0;
        let mut tag = set_value(tags[index].tag, "", writer.filter(|_| first && placed));
        if first && !placed {
            tag = without_writer_value(&tag);
            let insert_at = tag.rfind('>').unwrap_or(tag.len());
            tag.insert_str(
                insert_at,
                &format!(
                    " data-overflow-warning=\"text-truncated\" data-writer-value=\"{}\"",
                    html_escape(text)
                ),
            );
        }
        replacements.push((tags[index].start, tags[index].end, tag));
    }
}

fn apply_replacements_checked(
    html: &str,
    mut replacements: Vec<(usize, usize, String)>,
) -> Result<String, String> {
    replacements.sort_by_key(|(start, _, _)| *start);
    let mut out = String::with_capacity(html.len() + replacements.len() * 8);
    let mut last = 0;
    for (start, end, tag) in replacements {
        if start < last || end < start {
            return Err(format!(
                "overlapping print writes: byte range {start}..{end} begins before the previous write ends at {last}"
            ));
        }
        out.push_str(&html[last..start]);
        out.push_str(&tag);
        last = end;
    }
    out.push_str(&html[last..]);
    Ok(out)
}

fn apply_replacements(html: &str, replacements: Vec<(usize, usize, String)>) -> String {
    apply_replacements_checked(html, replacements).expect("print replacements overlap")
}

pub(crate) fn writer_cells_json(slug: &str) -> Option<&'static str> {
    match slug {
        "0605-1999" => Some(include_str!(
            "../../../html-frozen/0605-1999/writer-cells.json"
        )),
        "0619e-2018" => Some(include_str!(
            "../../../html-frozen/0619e-2018/writer-cells.json"
        )),
        "0619f-2018" => Some(include_str!(
            "../../../html-frozen/0619f-2018/writer-cells.json"
        )),
        "1600-pt-2018" => Some(include_str!(
            "../../../html-frozen/1600-pt-2018/writer-cells.json"
        )),
        "1600-vt-2018" => Some(include_str!(
            "../../../html-frozen/1600-vt-2018/writer-cells.json"
        )),
        "1600wp-2010" => Some(include_str!(
            "../../../html-frozen/1600wp-2010/writer-cells.json"
        )),
        "1601-fq-2020" => Some(include_str!(
            "../../../html-frozen/1601-fq-2020/writer-cells.json"
        )),
        "1601c-2018" => Some(include_str!(
            "../../../html-frozen/1601c-2018/writer-cells.json"
        )),
        "1601eq-2019" => Some(include_str!(
            "../../../html-frozen/1601eq-2019/writer-cells.json"
        )),
        "1602q-2019" => Some(include_str!(
            "../../../html-frozen/1602q-2019/writer-cells.json"
        )),
        "1603q-2018" => Some(include_str!(
            "../../../html-frozen/1603q-2018/writer-cells.json"
        )),
        "1604c-2018" => Some(include_str!(
            "../../../html-frozen/1604c-2018/writer-cells.json"
        )),
        "1604e-2018" => Some(include_str!(
            "../../../html-frozen/1604e-2018/writer-cells.json"
        )),
        "1604f-2018" => Some(include_str!(
            "../../../html-frozen/1604f-2018/writer-cells.json"
        )),
        "1606-2018" => Some(include_str!(
            "../../../html-frozen/1606-2018/writer-cells.json"
        )),
        "1700-2018" => Some(include_str!(
            "../../../html-frozen/1700-2018/writer-cells.json"
        )),
        "1701-2018" => Some(include_str!(
            "../../../html-frozen/1701-2018/writer-cells.json"
        )),
        "1701a-2018" => Some(include_str!(
            "../../../html-frozen/1701a-2018/writer-cells.json"
        )),
        "1701ms-2024" => Some(include_str!(
            "../../../html-frozen/1701ms-2024/writer-cells.json"
        )),
        "1701q-2018" => Some(include_str!(
            "../../../html-frozen/1701q-2018/writer-cells.json"
        )),
        "1702ex-2018" => Some(include_str!(
            "../../../html-frozen/1702ex-2018/writer-cells.json"
        )),
        "1702mx-2018c" => Some(include_str!(
            "../../../html-frozen/1702mx-2018c/writer-cells.json"
        )),
        "1702q-2018" => Some(include_str!(
            "../../../html-frozen/1702q-2018/writer-cells.json"
        )),
        "1702rt-2018c" => Some(include_str!(
            "../../../html-frozen/1702rt-2018c/writer-cells.json"
        )),
        "1706-2018" => Some(include_str!(
            "../../../html-frozen/1706-2018/writer-cells.json"
        )),
        "1707-2021" => Some(include_str!(
            "../../../html-frozen/1707-2021/writer-cells.json"
        )),
        "1707a-2021" => Some(include_str!(
            "../../../html-frozen/1707a-2021/writer-cells.json"
        )),
        "1800-2018" => Some(include_str!(
            "../../../html-frozen/1800-2018/writer-cells.json"
        )),
        "1801-2018" => Some(include_str!(
            "../../../html-frozen/1801-2018/writer-cells.json"
        )),
        "2000-dst-2018" => Some(include_str!(
            "../../../html-frozen/2000-dst-2018/writer-cells.json"
        )),
        "2000-ot-2018" => Some(include_str!(
            "../../../html-frozen/2000-ot-2018/writer-cells.json"
        )),
        "2200a-2020" => Some(include_str!(
            "../../../html-frozen/2200a-2020/writer-cells.json"
        )),
        "2200an-2018" => Some(include_str!(
            "../../../html-frozen/2200an-2018/writer-cells.json"
        )),
        "2200c-2018" => Some(include_str!(
            "../../../html-frozen/2200c-2018/writer-cells.json"
        )),
        "2200m-2018" => Some(include_str!(
            "../../../html-frozen/2200m-2018/writer-cells.json"
        )),
        "2200p-2020" => Some(include_str!(
            "../../../html-frozen/2200p-2020/writer-cells.json"
        )),
        "2200s-2018" => Some(include_str!(
            "../../../html-frozen/2200s-2018/writer-cells.json"
        )),
        "2200t-2020" => Some(include_str!(
            "../../../html-frozen/2200t-2020/writer-cells.json"
        )),
        "2550m-2007" => Some(include_str!(
            "../../../html-frozen/2550m-2007/writer-cells.json"
        )),
        "2550q-2024" => Some(include_str!(
            "../../../html-frozen/2550q-2024/writer-cells.json"
        )),
        "2551q-2018" => Some(include_str!(
            "../../../html-frozen/2551q-2018/writer-cells.json"
        )),
        "2552-2018" => Some(include_str!(
            "../../../html-frozen/2552-2018/writer-cells.json"
        )),
        "2553-1999" => Some(include_str!(
            "../../../html-frozen/2553-1999/writer-cells.json"
        )),
        _ => None,
    }
}

const XBOX_CHECKED_GLYPH: &str = "X";

/// Top-level `writer-cells.json` sections; anything else is a format error
/// (the old catch-all `left_out` is now the reviewed `not_joined_cells`).
const WRITER_CELLS_SECTIONS: [&str; 10] = [
    "form_id",
    "html_slug",
    "rule",
    "joins",
    "money_joins",
    "xbox_joins",
    "peso_joins",
    "date_joins",
    "not_joined",
    "not_joined_cells",
];

/// One printed amount box: a peso comb plus its 2-slot centavo comb, or
/// (`cent_html_id` `None`, a row's `decimal_html_ids`) one comb whose last
/// two slots take the centavos. With `point_slot` the slot before them holds
/// the printed decimal point and stays blank; without it the sheet prints
/// the point on the line between the peso and centavo slots.
struct MoneyCells {
    peso_html_id: String,
    cent_html_id: Option<String>,
    point_slot: bool,
}

/// How a negative amount prints, the way the official eBIRForms page shows
/// it in that form's boxes (its printout is the page itself, `window.print()`).
/// One form per sheet, from the official package (eBIRForms 7.9.6, the HTAs
/// whose SHA-256 the official layouts in `crates/bir-core/data/official-xml/`
/// record):
///
/// - [`Parentheses`](Self::Parentheses) where the form writes its amounts
///   through a bracketing formatter: 1702MX January 2018C
///   (`BIR-Form1702MXv2018C.hta`: `getDifference`, l. 15595-15606, and
///   `summation` write `NegativeValue(formatCurrencyWOC(..))`; Schedule 8
///   item 3 is `getDifference('txtPg4Sc8It1', 'txtPg4Sc8It2',
///   'txtPg4Sc8It3')`, l. 14529), 1702RT January 2018C (its sum, difference
///   and product helpers, l. 6912-6972, write `NegativeValue(..)`), both
///   through `js/string-util2014.js` `NegativeValue` (l. 66-93:
///   `'(' + num + ')'`), and 1707A 2021 (`BIR-Form1707Av2021.hta`
///   `roundAmount`, l. 4271-4301: `returnAmount = '(' + splitCap[1] + ')'`).
/// - [`Minus`](Self::Minus) everywhere else: those forms write their amounts
///   with `formatCurrency` (`js/string-util.js` l. 321-355:
///   `((sign) ? '' : '-') + num + '.' + cents`) and have no bracketing
///   formatter on an amount box; their official uploads, which copy the box
///   text, carry the minus (`1701Q-2025Q2` `txt42A=-1,000.51`, `1801-06052025`
///   `txtTaxableEstate=-3,489,999.38`, `1701MS-122025-special`
///   `txtTaxPayableNo25a=-26,000`, `1702EX-122025`
///   `txtPg1Pt2I20TotalOverpmt=-1,000.00`).
///
/// A negative never prints without its sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum NegativeStyle {
    #[default]
    Minus,
    Parentheses,
}

/// The sheets whose forms bracket a negative ([`NegativeStyle::Parentheses`]).
const BRACKET_SHEETS: [&str; 3] = ["1702mx-2018c", "1702rt-2018c", "1707a-2021"];

impl NegativeStyle {
    /// The official negative form of the form printed on sheet `slug`.
    pub(crate) fn of_sheet(slug: &str) -> Self {
        if BRACKET_SHEETS.contains(&slug) {
            Self::Parentheses
        } else {
            Self::Minus
        }
    }

    /// `body` (digits, commas, a decimal part) signed in this form.
    pub(crate) fn sign(self, body: &str) -> String {
        match self {
            Self::Parentheses => format!("({body})"),
            Self::Minus => format!("-{body}"),
        }
    }
}

/// A `{:.2}` writer amount printed in every listed peso + cents pair.
struct MoneyJoin {
    writer_key: String,
    cells: Vec<MoneyCells>,
    negative_style: NegativeStyle,
}

struct XboxJoin {
    writer_key: String,
    html_ids: Vec<String>,
}

/// A peso comb with no centavo comb beside it ("Do not enter centavos"):
/// the amount prints rounded to whole pesos, 50 centavos and up rounding up.
struct PesoJoin {
    writer_key: String,
    html_ids: Vec<String>,
    negative_style: NegativeStyle,
}

/// One writer date printed across the sheet's separate MM / DD / YYYY combs
/// (`dd_html_id` absent for an MM / YYYY pair).
struct DateJoin {
    writer_key: String,
    mm_html_id: String,
    dd_html_id: Option<String>,
    yyyy_html_id: String,
}

struct WriterCells {
    /// Text joins: one writer key, every cell it prints in (page-1 box and
    /// the later pages' header repeats).
    joins: BTreeMap<String, Vec<String>>,
    /// A text join's first cell -> its printed caption (`printed`), which
    /// says when cells are the stacked lines of one item.
    captions: BTreeMap<String, String>,
    money_joins: Vec<MoneyJoin>,
    xbox_joins: Vec<XboxJoin>,
    peso_joins: Vec<PesoJoin>,
    date_joins: Vec<DateJoin>,
    /// Reviewed exceptions: writer keys that print nowhere, with the reason.
    not_joined: BTreeMap<String, String>,
    /// Reviewed sheet cells no writer key fills, with the reason.
    not_joined_cells: BTreeMap<String, String>,
}

impl WriterCells {
    fn empty() -> Self {
        Self {
            joins: BTreeMap::new(),
            captions: BTreeMap::new(),
            money_joins: Vec::new(),
            xbox_joins: Vec::new(),
            peso_joins: Vec::new(),
            date_joins: Vec::new(),
            not_joined: BTreeMap::new(),
            not_joined_cells: BTreeMap::new(),
        }
    }

    fn has_fill_join(&self) -> bool {
        !self.joins.is_empty()
            || !self.money_joins.is_empty()
            || !self.xbox_joins.is_empty()
            || !self.peso_joins.is_empty()
            || !self.date_joins.is_empty()
    }

    /// Every writer key any join prints.
    fn joined_keys(&self) -> BTreeSet<&str> {
        self.joins
            .keys()
            .map(String::as_str)
            .chain(self.money_joins.iter().map(|j| j.writer_key.as_str()))
            .chain(self.xbox_joins.iter().map(|j| j.writer_key.as_str()))
            .chain(self.peso_joins.iter().map(|j| j.writer_key.as_str()))
            .chain(self.date_joins.iter().map(|j| j.writer_key.as_str()))
            .collect()
    }

    /// `(writer key, cell)` for every fill target, in file order.
    fn targets(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        for (key, ids) in &self.joins {
            out.extend(ids.iter().map(|id| (key.as_str(), id.as_str())));
        }
        for join in &self.money_joins {
            for cells in &join.cells {
                out.push((join.writer_key.as_str(), cells.peso_html_id.as_str()));
                if let Some(cent) = &cells.cent_html_id {
                    out.push((join.writer_key.as_str(), cent.as_str()));
                }
            }
        }
        for join in &self.xbox_joins {
            out.extend(
                join.html_ids
                    .iter()
                    .map(|id| (join.writer_key.as_str(), id.as_str())),
            );
        }
        for join in &self.peso_joins {
            out.extend(
                join.html_ids
                    .iter()
                    .map(|id| (join.writer_key.as_str(), id.as_str())),
            );
        }
        for join in &self.date_joins {
            out.push((join.writer_key.as_str(), join.mm_html_id.as_str()));
            if let Some(dd) = &join.dd_html_id {
                out.push((join.writer_key.as_str(), dd.as_str()));
            }
            out.push((join.writer_key.as_str(), join.yyyy_html_id.as_str()));
        }
        out
    }
}

fn required_str<'a>(row: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    row.get(field)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
}

/// A row's cell ids: `single` (`"html_id"`) and/or `plural`
/// (`"html_ids": [...]`), at least one, each a cell id and listed once.
fn cell_ids(
    row: &serde_json::Value,
    key: &str,
    single: &str,
    plural: &str,
) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = Vec::new();
    if let Some(value) = row.get(single) {
        let id = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("writer-cells.json {key} {single} must be a cell id"))?;
        ids.push(id.to_string());
    }
    if let Some(value) = row.get(plural) {
        let list = value
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| format!("writer-cells.json {key} {plural} must list cell ids"))?;
        for id in list {
            let id = id
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("writer-cells.json {key} {plural} must list cell ids"))?;
            ids.push(id.to_string());
        }
    }
    if ids.is_empty() {
        return Err(format!("writer-cells.json {key} missing {single}"));
    }
    for (at, id) in ids.iter().enumerate() {
        if id.starts_with("frm") {
            return Err(format!(
                "writer-cells.json {key} must target a cell id, not {id}"
            ));
        }
        if ids[..at].contains(id) {
            return Err(format!("writer-cells.json {key} lists {id} twice"));
        }
    }
    Ok(ids)
}

/// [`cell_ids`] for a row where these cells are optional: empty when the
/// row names neither field.
fn optional_cell_ids(
    row: &serde_json::Value,
    key: &str,
    single: &str,
    plural: &str,
) -> Result<Vec<String>, String> {
    if row.get(single).is_none() && row.get(plural).is_none() {
        return Ok(Vec::new());
    }
    cell_ids(row, key, single, plural)
}

fn join_rows<'a>(
    payload: &'a serde_json::Value,
    section: &str,
) -> Result<&'a [serde_json::Value], String> {
    match payload.get(section) {
        None => Ok(&[]),
        Some(value) => value
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| format!("writer-cells.json {section} must be a list")),
    }
}

fn row_key<'a>(row: &'a serde_json::Value, section: &str) -> Result<&'a str, String> {
    required_str(row, "writer_key")
        .ok_or_else(|| format!("writer-cells.json {section} row missing writer_key"))
}

fn parse_writer_cells(json: &str) -> Result<WriterCells, String> {
    let payload: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("writer-cells.json: {error}"))?;
    let object = payload
        .as_object()
        .ok_or_else(|| "writer-cells.json must be an object".to_string())?;
    if let Some(unknown) = object
        .keys()
        .find(|key| !WRITER_CELLS_SECTIONS.contains(&key.as_str()))
    {
        return Err(format!("writer-cells.json unknown section {unknown}"));
    }
    if payload
        .get("joins")
        .and_then(|value| value.as_array())
        .is_none()
    {
        return Err("writer-cells.json missing joins[]".to_string());
    }
    let mut cells = WriterCells::empty();
    // One writer key is one row across every join kind; a key that prints
    // in several boxes lists them all on that row.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut claim = |key: &str| -> Result<(), String> {
        if seen.insert(key.to_string()) {
            Ok(())
        } else {
            Err(format!(
                "writer-cells.json duplicate writer_key {key} across joins"
            ))
        }
    };

    for row in join_rows(&payload, "joins")? {
        let key = row_key(row, "join")?;
        let ids = cell_ids(row, key, "html_id", "html_ids")?;
        claim(key)?;
        if let (Some(first), Some(caption)) = (ids.first(), required_str(row, "printed")) {
            cells.captions.insert(first.clone(), caption.to_string());
        }
        cells.joins.insert(key.to_string(), ids);
    }

    for row in join_rows(&payload, "money_joins")? {
        let key = row_key(row, "money_join")?;
        let pesos = optional_cell_ids(row, key, "peso_html_id", "peso_html_ids")?;
        let cents = optional_cell_ids(row, key, "cent_html_id", "cent_html_ids")?;
        let decimals = optional_cell_ids(row, key, "decimal_html_id", "decimal_html_ids")?;
        let point_slot = match row.get("point_slot") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                format!("writer-cells.json {key} point_slot must be true or false")
            })?,
        };
        if pesos.len() != cents.len() {
            return Err(format!(
                "writer-cells.json {key} pairs {} peso cells with {} cent cells",
                pesos.len(),
                cents.len()
            ));
        }
        if pesos.is_empty() && decimals.is_empty() {
            return Err(format!("writer-cells.json {key} missing peso_html_id"));
        }
        claim(key)?;
        let mut pairs = Vec::new();
        for (peso_html_id, cent_html_id) in pesos.into_iter().zip(cents) {
            if peso_html_id == cent_html_id {
                return Err(format!(
                    "writer-cells.json {key} peso and cent cells must differ"
                ));
            }
            pairs.push(MoneyCells {
                peso_html_id,
                cent_html_id: Some(cent_html_id),
                point_slot: false,
            });
        }
        pairs.extend(decimals.into_iter().map(|peso_html_id| MoneyCells {
            peso_html_id,
            cent_html_id: None,
            point_slot,
        }));
        cells.money_joins.push(MoneyJoin {
            writer_key: key.to_string(),
            cells: pairs,
            negative_style: NegativeStyle::Minus,
        });
    }

    for row in join_rows(&payload, "xbox_joins")? {
        let key = row_key(row, "xbox_join")?;
        let html_ids = cell_ids(row, key, "html_id", "html_ids")?;
        claim(key)?;
        cells.xbox_joins.push(XboxJoin {
            writer_key: key.to_string(),
            html_ids,
        });
    }

    for row in join_rows(&payload, "peso_joins")? {
        let key = row_key(row, "peso_join")?;
        let html_ids = cell_ids(row, key, "html_id", "html_ids")?;
        claim(key)?;
        cells.peso_joins.push(PesoJoin {
            writer_key: key.to_string(),
            html_ids,
            negative_style: NegativeStyle::Minus,
        });
    }

    for row in join_rows(&payload, "date_joins")? {
        let key = row_key(row, "date_join")?;
        let one = |field: &str| -> Result<Option<String>, String> {
            match row.get(field) {
                None => Ok(None),
                Some(value) => value
                    .as_str()
                    .filter(|id| !id.is_empty() && !id.starts_with("frm"))
                    .map(|id| Some(id.to_string()))
                    .ok_or_else(|| format!("writer-cells.json {key} {field} must be a cell id")),
            }
        };
        let mm_html_id = one("mm_html_id")?
            .ok_or_else(|| format!("writer-cells.json {key} missing mm_html_id"))?;
        let yyyy_html_id = one("yyyy_html_id")?
            .ok_or_else(|| format!("writer-cells.json {key} missing yyyy_html_id"))?;
        let dd_html_id = one("dd_html_id")?;
        claim(key)?;
        cells.date_joins.push(DateJoin {
            writer_key: key.to_string(),
            mm_html_id,
            dd_html_id,
            yyyy_html_id,
        });
    }

    // A not_joined row names one key (`writer_key`) or a family sharing one
    // reason (`writer_keys`, e.g. table rows past the sheet's last row).
    for row in join_rows(&payload, "not_joined")? {
        let keys: Vec<&str> = match (required_str(row, "writer_key"), row.get("writer_keys")) {
            (Some(key), None) => vec![key],
            (None, Some(list)) => list
                .as_array()
                .filter(|list| !list.is_empty())
                .and_then(|list| list.iter().map(|key| key.as_str()).collect())
                .ok_or_else(|| {
                    "writer-cells.json not_joined writer_keys must list keys".to_string()
                })?,
            _ => {
                return Err(
                    "writer-cells.json not_joined row needs writer_key or writer_keys".to_string(),
                );
            }
        };
        let reason = required_str(row, "reason")
            .ok_or_else(|| format!("writer-cells.json not_joined {} needs a reason", keys[0]))?;
        for key in keys {
            if key.is_empty() {
                return Err("writer-cells.json not_joined lists an empty key".to_string());
            }
            if cells.joined_keys().contains(key) {
                return Err(format!(
                    "writer-cells.json {key} is joined; remove it from not_joined"
                ));
            }
            if cells
                .not_joined
                .insert(key.to_string(), reason.to_string())
                .is_some()
            {
                return Err(format!("writer-cells.json duplicate not_joined {key}"));
            }
        }
    }

    let mut used_cells: BTreeSet<String> = BTreeSet::new();
    for (key, id) in cells.targets() {
        if !used_cells.insert(id.to_string()) {
            return Err(format!(
                "writer-cells.json {key} cell {id} is already a fill target"
            ));
        }
    }
    {
        for row in join_rows(&payload, "not_joined_cells")? {
            let id = required_str(row, "html_id").ok_or_else(|| {
                "writer-cells.json not_joined_cells row missing html_id".to_string()
            })?;
            let reason = required_str(row, "reason")
                .ok_or_else(|| format!("writer-cells.json not_joined_cells {id} needs a reason"))?;
            if used_cells.contains(id) {
                return Err(format!(
                    "writer-cells.json cell {id} is a fill target; remove it from not_joined_cells"
                ));
            }
            if cells
                .not_joined_cells
                .insert(id.to_string(), reason.to_string())
                .is_some()
            {
                return Err(format!("writer-cells.json duplicate not_joined_cells {id}"));
            }
        }
    }

    Ok(cells)
}

fn writer_cells(slug: &str) -> Result<WriterCells, String> {
    let mut cells = match writer_cells_json(slug) {
        Some(json) => parse_writer_cells(json)?,
        None => WriterCells::empty(),
    };
    merge_sheet_joins(&mut cells, sheet_joins(slug));
    add_donor_estate_joins(slug, &mut cells);
    let negative = NegativeStyle::of_sheet(slug);
    for join in &mut cells.money_joins {
        join.negative_style = negative;
    }
    for join in &mut cells.peso_joins {
        join.negative_style = negative;
    }
    Ok(cells)
}

/// `slug`'s [`SHEET_JOINS`].
fn sheet_joins(slug: &str) -> &'static [SheetJoin] {
    SHEET_JOINS
        .iter()
        .find(|(sheet, _)| *sheet == slug)
        .map_or(&[], |(_, joins)| *joins)
}

/// Add `joins` to `cells`, each key leaving `not_joined`. A join whose key
/// writer-cells.json already joins, or whose cell it already fills, is
/// skipped: a later re-sync that adds the same join never fills a cell
/// twice (two replacements of one input would overlap).
fn merge_sheet_joins(cells: &mut WriterCells, joins: &[SheetJoin]) {
    for (key, cell, _) in joins {
        let filled: BTreeSet<String> = cells
            .targets()
            .into_iter()
            .map(|(_, cell)| cell.to_string())
            .collect();
        if cells.joins.contains_key(*key) || filled.contains(*cell) {
            continue;
        }
        cells.not_joined.remove(*key);
        cells
            .joins
            .entry((*key).to_string())
            .or_default()
            .push((*cell).to_string());
    }
}

/// `slug`'s writer-cells.json as JSON with [`SHEET_JOINS`] merged in the
/// way the print merges them (the same skip rules): each join added to
/// `joins` with its printed caption, its key taken off `not_joined`. The one
/// source the screen (comb capacity, the audit's not-joined reasons) and
/// the print share. `None` for a sheet with no writer-cells.json.
pub fn merged_writer_cells_json(slug: &str) -> Option<serde_json::Value> {
    let mut cells: serde_json::Value = serde_json::from_str(writer_cells_json(slug)?).ok()?;
    let mut joins: Vec<serde_json::Value> = cells["joins"].as_array().cloned().unwrap_or_default();
    for (key, cell, printed) in sheet_joins(slug) {
        let taken = joins
            .iter()
            .any(|join| join["writer_key"] == *key || join["html_id"] == *cell);
        if taken {
            continue;
        }
        joins.push(serde_json::json!({
            "writer_key": key,
            "html_id": cell,
            "printed": printed,
            "reason": "frozen_html.rs SHEET_JOINS (the frozen writer-cells.json leaves it out)",
        }));
        if let Some(rows) = cells["not_joined"].as_array_mut() {
            for row in rows.iter_mut() {
                if let Some(list) = row["writer_keys"].as_array_mut() {
                    list.retain(|k| k != *key);
                }
            }
            rows.retain(|row| {
                row["writer_key"] != *key
                    && row["writer_keys"]
                        .as_array()
                        .is_none_or(|list| !list.is_empty())
            });
        }
    }
    cells["joins"] = serde_json::Value::Array(joins);
    Some(cells)
}

/// Every cell of sheet `slug` a writer key prints into: the cells of its
/// writer-cells joins (text, money, peso, check box and date), with the
/// engine's own joins ([`SHEET_JOINS`], the donor and estate joins) merged
/// in. Cells filled by an input's stamped name are not listed here.
pub fn joined_cell_ids(slug: &str) -> BTreeSet<String> {
    let Ok(cells) = writer_cells(slug) else {
        return BTreeSet::new();
    };
    let mut ids: BTreeSet<String> = cells.joins.values().flatten().cloned().collect();
    for join in &cells.money_joins {
        for pair in &join.cells {
            ids.insert(pair.peso_html_id.clone());
            ids.extend(pair.cent_html_id.clone());
        }
    }
    for join in &cells.xbox_joins {
        ids.extend(join.html_ids.iter().cloned());
    }
    for join in &cells.peso_joins {
        ids.extend(join.html_ids.iter().cloned());
    }
    for join in &cells.date_joins {
        ids.insert(join.mm_html_id.clone());
        ids.extend(join.dd_html_id.clone());
        ids.insert(join.yyyy_html_id.clone());
    }
    ids
}

/// `slug`'s [`SHEET_JOINS`] that print in a comb: `(writer key, comb
/// slots, printed caption)`, read from the bundled sheet, for the screen's
/// character boxes.
pub fn sheet_join_combs(slug: &str) -> Vec<(String, usize, String)> {
    let Some(loaded) = bundle(slug) else {
        return Vec::new();
    };
    let merged = merged_writer_cells_json(slug).unwrap_or(serde_json::Value::Null);
    sheet_joins(slug)
        .iter()
        .filter(|(key, cell, _)| {
            merged["joins"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|join| {
                    join["writer_key"] == *key
                        && join["html_id"] == *cell
                        && join["reason"]
                            .as_str()
                            .is_some_and(|r| r.starts_with("frozen_html.rs SHEET_JOINS"))
                })
        })
        .filter_map(|(key, cell, printed)| {
            let (_, tag) = cell_tag(loaded.html, cell)?;
            let slots = attr(tag, "data-comb-slots")?.parse().ok()?;
            Some((key.to_string(), slots, printed.to_string()))
        })
        .collect()
}

/// What a PR 10 Rust-side join prints in its cell.
#[derive(Clone, Copy)]
enum SheetValue {
    /// The writer key's own value.
    Key(&'static str),
    /// The writer key's value as a `derived:line:` value: spread over the
    /// comb, or printed whole on one line when it is longer.
    Line(&'static str),
    /// The writer keys' values one after another (a TIN kept in three
    /// boxes, printed in one comb).
    Concat(&'static [&'static str]),
    /// Month, day and year keys in one `MMDDYY` comb, each typed part in its
    /// own two slots (the year's last two digits); a part not chosen (`00`)
    /// leaves its slots blank.
    ShortDate(&'static str, &'static str, &'static str),
    /// A tick: `X` in the box while the writer key is `true`.
    Tick(&'static str),
}

/// PR 10: joins the frozen 1800 and 1801 sheets' writer-cells.json leaves
/// out although the sheet has the cell (html-frozen is synced from upstream
/// and never edited here), `(sheet, cell, value)`. Each join supersedes the
/// `not_joined` / `not_joined_cells` reason writer-cells.json gives for its
/// key and cell; the sheet's own geometry and printed text
/// (html-frozen/<slug>/index.html) place it:
/// - 1801 p1c4: item 1 "Date of Death (MM/DD/YY)", one 6-slot comb
///   (data-comb-slots 6). Supersedes "item 1 Date of Death: one 6-slot comb
///   (p1c4) for MMDDYY; three writer keys onto one cell is not a join": the
///   join is not a writer-cells text join but [`fill_donor_estate_dates`],
///   which puts each part in its own two slots.
/// - 1801 p1c31 / p1c32: item 11 "TIN of Executor/Administrator". p1c31
///   has 9 inputs at slot indices 0-2, 4-6 and 8-10 (the printed dashes
///   take slots 3 and 7) and its first slot starts 114.74pt in, past the
///   caption; p1c32 is the 5-slot branch comb, first slot 14.28pt in.
///   Supersedes "executor TIN (item 11): one mixed 11-slot comb (p1c31)
///   with printed dashes inside; not split": the nine digits fill the nine
///   inputs in order.
/// - 1801 p1c33: item 12 "Contact Number", 11 slots, the first 56.64pt in,
///   past the caption. Supersedes "item 12 Contact Number comb (p1c33) is
///   mixed: its printed caption sits inside the slots".
/// - 1801 p1c38: item 14A "If yes, specify", 15 slots. Supersedes "item 14A
///   'If yes, specify': the writer holds an option code, not printable
///   text": 14A is the filer's text on this form.
/// - 1801 item 15D's frequency boxes, each the box left of its printed
///   choice on the row at top 344.42pt: p1c50 Monthly (x 303.12pt, text at
///   315.41pt), p1c53 Quarterly (352.2pt; 364.63pt), p1c51 Semi-Annual
///   (402.48pt; 415.27pt), p1c52 Others (459.72pt; 471.7pt). Supersedes
///   "frequency-of-payment options (p1c50 to p1c53): no printed option
///   labels in the text layer, so not guessed": the labels are in the text
///   layer at those positions. p1c197 is the input after "(specify)" (its
///   input starts 46.58pt into the cell, past the text at 491.86pt).
///   Supersedes "frequency-of-payment 'other' specify: no matching labelled
///   cell".
/// - 1801 p1c201: the blank box at the end of item 21's caption "...to be
///   paid on or before" (its input starts 289.65pt into the caption cell).
///   Supersedes "no fillable cell on the frozen sheet for this key
///   (installment year blank)".
/// - 1800 p1c26: item 8A "ZIP Code", 4 slots after the caption (the first
///   slot starts 56pt into the cell). Supersedes "8A ZIP cell p1c26 is
///   mixed: the printed '8A ZIP Code' caption overlaps its slots".
///   p1c57: item 13A "If yes, specify", 19 slots. Supersedes "item 13A 'If
///   yes, specify': the writer holds an option code, not printable text".
const DONOR_ESTATE_JOINS: &[(&str, &str, SheetValue)] = &[
    (
        "1801-2018",
        "p1c4",
        SheetValue::ShortDate(
            "frm1801v2018:txtDateMonth",
            "frm1801v2018:txtDateDay",
            "frm1801v2018:txtDateYear",
        ),
    ),
    (
        "1801-2018",
        "p1c31",
        SheetValue::Concat(&[
            "frm1801v2018:txtTINE1",
            "frm1801v2018:txtTINE2",
            "frm1801v2018:txtTINE3",
        ]),
    ),
    (
        "1801-2018",
        "p1c32",
        SheetValue::Key("frm1801v2018:txtBranchCodeE"),
    ),
    (
        "1801-2018",
        "p1c33",
        SheetValue::Line("frm1801v2018:telephoneNumber"),
    ),
    (
        "1801-2018",
        "p1c38",
        SheetValue::Line("frm1801v2018:treatyY"),
    ),
    (
        "1801-2018",
        "p1c50",
        SheetValue::Tick("frm1801v2018:freq_Mon"),
    ),
    (
        "1801-2018",
        "p1c53",
        SheetValue::Tick("frm1801v2018:freq_Qtr"),
    ),
    (
        "1801-2018",
        "p1c51",
        SheetValue::Tick("frm1801v2018:freq_Semi"),
    ),
    (
        "1801-2018",
        "p1c52",
        SheetValue::Tick("frm1801v2018:freq_Other"),
    ),
    (
        "1801-2018",
        "p1c197",
        SheetValue::Key("frm1801v2018:freqOtherInput"),
    ),
    (
        "1801-2018",
        "p1c201",
        SheetValue::Key("frm1801v2018:txtInstallmentYear"),
    ),
    (
        "1800-2018",
        "p1c26",
        SheetValue::Key("frm1800v2018:txtZipCode"),
    ),
    (
        "1800-2018",
        "p1c57",
        SheetValue::Line("frm1800v2018:TaxTreaty"),
    ),
];

/// How a PR 10 sheet prints a writer value (print only; the upload is
/// unchanged).
#[derive(Clone, Copy)]
enum SheetRewrite {
    /// A date part: `00` (not chosen) prints nothing.
    ZeroBlank,
    /// A year in a two-slot `YY` comb: its last two digits; `0` prints
    /// nothing.
    TwoDigitYear,
    /// Without its surrounding spaces.
    Trim,
    /// Digits padded with leading zeros to the comb's slots.
    ZeroPad(usize),
}

/// PR 10's print rewrites, `(sheet, writer key, rewrite)`. 1800 item 1
/// "Date of Donation (MM/DD/YY)": writer-cells.json joins txtMonth to p1c5,
/// txtDate to p1c6 and txtYear to p1c7, each a 2-slot comb; form_1800.rs
/// writes the 4-digit year and `00`/`0` for parts not chosen, which the
/// page shows blank. 1800 8A's ZIP is trimmed; 1801 item 11's branch code
/// fills its 5-slot comb (p1c32).
const DONOR_ESTATE_REWRITES: &[(&str, &str, SheetRewrite)] = &[
    (
        "1800-2018",
        "frm1800v2018:txtMonth",
        SheetRewrite::ZeroBlank,
    ),
    ("1800-2018", "frm1800v2018:txtDate", SheetRewrite::ZeroBlank),
    (
        "1800-2018",
        "frm1800v2018:txtYear",
        SheetRewrite::TwoDigitYear,
    ),
    ("1800-2018", "frm1800v2018:txtZipCode", SheetRewrite::Trim),
    (
        "1801-2018",
        "frm1801v2018:txtBranchCodeE",
        SheetRewrite::ZeroPad(5),
    ),
];

/// A date part's digits, or `None` for a part not chosen (blank, all
/// zeros) or not a number.
fn date_part(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit()) && text.chars().any(|c| c != '0'))
        .then_some(text)
}

/// A date part as the two slots it prints in: a month or day padded, a
/// year's last two digits. `None` when it prints nothing.
fn two_slots(text: &str, year: bool) -> Option<String> {
    let digits = date_part(text)?;
    match (year, digits.len()) {
        (true, n) if n >= 2 => Some(digits[n - 2..].to_string()),
        (false, 1 | 2) => Some(format!("{digits:0>2}")),
        _ => None,
    }
}

/// The writer key a [`DONOR_ESTATE_JOINS`] value prints under: the key
/// itself, or a derived key named after its cell.
fn donor_estate_key(cell: &str, value: SheetValue) -> String {
    match value {
        SheetValue::Key(key) | SheetValue::Tick(key) => key.to_string(),
        SheetValue::Line(_) => format!("derived:line:pr10:{cell}"),
        SheetValue::Concat(_) | SheetValue::ShortDate(..) => format!("derived:pr10:{cell}"),
    }
}

/// The writer keys a derived [`DONOR_ESTATE_JOINS`] value reads.
fn donor_estate_sources(value: SheetValue) -> Vec<&'static str> {
    match value {
        SheetValue::Key(_) | SheetValue::Tick(_) => Vec::new(),
        SheetValue::Line(key) => vec![key],
        SheetValue::Concat(keys) => keys.to_vec(),
        SheetValue::ShortDate(month, day, year) => vec![month, day, year],
    }
}

/// `slug`'s [`DONOR_ESTATE_JOINS`] merged into its writer cells: a join
/// for each (a date is filled by [`fill_donor_estate_dates`] instead), its
/// keys out of `not_joined` and its cell out of `not_joined_cells`. A key
/// or cell writer-cells.json already joins is left as it is (a later
/// re-sync must not fill a cell twice).
fn add_donor_estate_joins(slug: &str, cells: &mut WriterCells) {
    for (sheet, cell, value) in DONOR_ESTATE_JOINS {
        if *sheet != slug {
            continue;
        }
        let key = donor_estate_key(cell, *value);
        let filled = cells.targets().iter().any(|(_, target)| target == cell);
        if filled || cells.joined_keys().contains(key.as_str()) {
            continue;
        }
        match value {
            SheetValue::Tick(_) => cells.xbox_joins.push(XboxJoin {
                writer_key: key.clone(),
                html_ids: vec![(*cell).to_string()],
            }),
            SheetValue::ShortDate(..) => {}
            _ => {
                cells.joins.insert(key.clone(), vec![(*cell).to_string()]);
            }
        }
        cells.not_joined.remove(&key);
        for source in donor_estate_sources(*value) {
            cells.not_joined.remove(source);
        }
        cells.not_joined_cells.remove(*cell);
    }
}

/// `fields` with `slug`'s print rewrites applied and its derived
/// [`DONOR_ESTATE_JOINS`] values added.
fn with_donor_estate_values(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut out = fields.clone();
    for (sheet, key, rewrite) in DONOR_ESTATE_REWRITES {
        if *sheet != slug {
            continue;
        }
        let Some(value) = out.get_mut(*key) else {
            continue;
        };
        *value = match rewrite {
            SheetRewrite::ZeroBlank => two_slots(value, false).unwrap_or_default(),
            SheetRewrite::TwoDigitYear => two_slots(value, true).unwrap_or_default(),
            SheetRewrite::Trim => value.trim().to_string(),
            SheetRewrite::ZeroPad(width) => {
                let text = value.trim();
                if !text.is_empty()
                    && text.len() < *width
                    && text.chars().all(|c| c.is_ascii_digit())
                {
                    format!("{text:0>width$}", width = *width)
                } else {
                    text.to_string()
                }
            }
        };
    }
    let get = |key: &str| fields.get(key).map(|v| v.trim()).unwrap_or("");
    for (sheet, cell, value) in DONOR_ESTATE_JOINS {
        if *sheet != slug {
            continue;
        }
        let printed = match value {
            SheetValue::Key(_) | SheetValue::Tick(_) | SheetValue::ShortDate(..) => continue,
            SheetValue::Line(key) => get(key).to_string(),
            SheetValue::Concat(keys) => keys.iter().map(|key| get(key)).collect(),
        };
        if !printed.is_empty() {
            out.insert(donor_estate_key(cell, *value), printed);
        }
    }
    out
}

/// Each [`SheetValue::ShortDate`] of `slug`: every typed part in its own
/// two slots of the cell; a part not chosen leaves its slots as they are.
fn fill_donor_estate_dates(html: &str, fields: &BTreeMap<String, String>, slug: &str) -> String {
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (sheet, cell, value) in DONOR_ESTATE_JOINS {
        let (true, SheetValue::ShortDate(month, day, year)) = (*sheet == slug, value) else {
            continue;
        };
        let ordered = grouped
            .get(*cell)
            .map(|indices| ordered_comb_indices(&tags, indices))
            .unwrap_or_default();
        if ordered.len() != 6 {
            continue;
        }
        for (at, (key, is_year)) in [(*month, false), (*day, false), (*year, true)]
            .into_iter()
            .enumerate()
        {
            let get = fields.get(key).map(String::as_str).unwrap_or("");
            let Some(digits) = two_slots(get, is_year) else {
                continue;
            };
            for (offset, ch) in digits.chars().enumerate() {
                let index = ordered[at * 2 + offset];
                replacements.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, &ch.to_string(), None),
                ));
            }
        }
    }
    apply_replacements(html, replacements)
}

/// Whether `slug`'s PR 10 joins print the value `value` of writer key
/// `key` (`Some(true)`), hold nothing to print (`Some(false)`: a date part
/// not chosen, as the page shows it blank), or do not read the key
/// (`None`). A date part counts as printed only when it reaches the sheet.
fn donor_estate_prints(slug: &str, key: &str, value: &str) -> Option<bool> {
    DONOR_ESTATE_JOINS
        .iter()
        .filter(|(sheet, _, _)| *sheet == slug)
        .find_map(|(_, _, joined)| match joined {
            SheetValue::ShortDate(month, day, _) if key == *month || key == *day => {
                Some(two_slots(value, false).is_some())
            }
            SheetValue::ShortDate(_, _, year) if key == *year => {
                Some(two_slots(value, true).is_some())
            }
            other => donor_estate_sources(*other).contains(&key).then_some(true),
        })
}

/// Whether `value` is a PR 10 date part of `slug` not chosen (all zeros),
/// which the page and the print show blank.
fn donor_estate_unchosen(slug: &str, key: &str, value: &str) -> bool {
    let value = value.trim();
    DONOR_ESTATE_JOINS.iter().any(|(sheet, _, joined)| {
        *sheet == slug
            && matches!(joined, SheetValue::ShortDate(..))
            && donor_estate_sources(*joined).contains(&key)
    }) && !value.is_empty()
        && value.chars().all(|c| c == '0')
}

fn grouped_input_indices<'a>(tags: &[InputTag<'a>]) -> BTreeMap<&'a str, Vec<usize>> {
    let mut grouped: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, tag) in tags.iter().enumerate() {
        grouped.entry(tag.name).or_default().push(index);
    }
    grouped
}

fn ordered_comb_indices(tags: &[InputTag<'_>], indices: &[usize]) -> Vec<usize> {
    let mut ordered = indices.to_vec();
    ordered.sort_by_key(|&index| tags[index].slot.unwrap_or(usize::MAX));
    ordered
}

/// Splits a writer amount into `(negative, peso digits, two cent digits)`.
fn split_writer_money(value: &str) -> Option<(bool, String, String)> {
    let cleaned: String = value
        .chars()
        .filter(|ch| *ch != ',' && !ch.is_whitespace())
        .collect();
    let (negative, cleaned) = if let Some(rest) = cleaned.strip_prefix('-') {
        (true, rest)
    } else if let Some(inner) = cleaned.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        (true, inner)
    } else {
        (false, cleaned.as_str())
    };
    if cleaned.is_empty() {
        return None;
    }
    let (peso, cents) = match cleaned.split_once('.') {
        Some((peso, cents)) => (peso, cents),
        None => (cleaned, "00"),
    };
    if peso.chars().any(|ch| !ch.is_ascii_digit()) {
        return None;
    }
    if cents.is_empty() || cents.len() > 2 || cents.chars().any(|ch| !ch.is_ascii_digit()) {
        return None;
    }
    Some((negative, peso.to_string(), format!("{cents:0<2}")))
}

/// Formats the peso digits for a money or whole-peso comb in the sheet's
/// official [`NegativeStyle`]; a zero amount (`-0`, `-0.00`) prints unsigned.
fn signed_digits(negative: bool, digits: &str, cents: &str, style: NegativeStyle) -> String {
    let zero = digits.chars().chain(cents.chars()).all(|ch| ch == '0');
    if !negative || zero {
        return digits.to_string();
    }
    style.sign(digits)
}

fn right_aligned_slot_values(slots: usize, digits: &str) -> Option<Vec<String>> {
    if digits.len() > slots {
        return None;
    }
    let pad = slots - digits.len();
    let mut values = vec![String::new(); slots];
    for (offset, ch) in digits.chars().enumerate() {
        values[pad + offset] = ch.to_string();
    }
    Some(values)
}

#[allow(dead_code)]
fn fill_money_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    money_joins: &[MoneyJoin],
) -> String {
    fill_money_joins_with_sheet(
        html,
        fields,
        money_joins,
        &SheetType::plain(),
        &BTreeSet::new(),
    )
}

fn fill_money_joins_with_sheet(
    html: &str,
    fields: &BTreeMap<String, String>,
    money_joins: &[MoneyJoin],
    sheet: &SheetType,
    forced: &BTreeSet<String>,
) -> String {
    if money_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in money_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        // `None`: not peso digits and at most two decimals (`1,000.505`,
        // or no amount at all). It still prints, whole, in the rectangle.
        let parsed = split_writer_money(value);
        let rect_text = match &parsed {
            Some((negative, raw_peso, cents)) => {
                format_rect_amount(*negative, raw_peso, Some(cents), join.negative_style)
            }
            None => format_comb_amount_for_rect(value, join.negative_style),
        };
        let boxes = parsed.as_ref().and_then(|(negative, raw_peso, cents)| {
            let zero = raw_peso.chars().chain(cents.chars()).all(|ch| ch == '0');
            let parenthesized =
                *negative && !zero && join.negative_style == NegativeStyle::Parentheses;
            (!parenthesized).then(|| {
                (
                    signed_digits(*negative, raw_peso, cents, join.negative_style),
                    cents.clone(),
                )
            })
        });
        for cells in &join.cells {
            let Some(peso_indices) = grouped.get(cells.peso_html_id.as_str()) else {
                continue;
            };
            let mut peso_ordered = ordered_comb_indices(&tags, peso_indices);
            let mut cent_cell = None;
            let cent_ordered = match &cells.cent_html_id {
                Some(cent) => grouped.get(cent.as_str()).map(|cent_indices| {
                    cent_cell = Some(cent.as_str());
                    ordered_comb_indices(&tags, cent_indices)
                }),
                // The comb's own last two slots; a point slot before them is
                // left blank (the sheet prints the point there).
                None if peso_ordered.len() >= 5 => {
                    let cents = peso_ordered.split_off(peso_ordered.len() - 2);
                    // The point is a real slot only when the slot indices run
                    // straight into the cents; a sheet that already leaves the
                    // point column out of the comb (an index gap) must not
                    // lose its last peso box to it (2200-S shifted every
                    // amount one box left).
                    let point_is_gap = match (peso_ordered.last(), cents.first()) {
                        (Some(&last), Some(&first)) => matches!(
                            (tags[last].slot, tags[first].slot),
                            (Some(a), Some(b)) if b > a + 1
                        ),
                        _ => false,
                    };
                    if cells.point_slot && !point_is_gap {
                        let point = peso_ordered.pop().expect("comb has a decimal slot");
                        replacements.push((
                            tags[point].start,
                            tags[point].end,
                            set_value(tags[point].tag, "", None),
                        ));
                    }
                    Some(cents)
                }
                None => None,
            };
            if let (Some((peso, cents)), Some(cent_ordered)) = (&boxes, &cent_ordered) {
                let peso_values = (cent_ordered.len() == 2)
                    .then(|| right_aligned_slot_values(peso_ordered.len(), peso))
                    .flatten()
                    .filter(|_| !forced.contains(&cells.peso_html_id));
                if let Some(peso_values) = peso_values {
                    for (offset, &index) in peso_ordered.iter().enumerate() {
                        let writer = (offset == 0).then_some(value.as_str());
                        replacements.push((
                            tags[index].start,
                            tags[index].end,
                            set_value(tags[index].tag, &peso_values[offset], writer),
                        ));
                    }
                    for (offset, &index) in cent_ordered.iter().enumerate() {
                        let ch = cents
                            .chars()
                            .nth(offset)
                            .map(|ch| ch.to_string())
                            .unwrap_or_default();
                        replacements.push((
                            tags[index].start,
                            tags[index].end,
                            set_value(tags[index].tag, &ch, None),
                        ));
                    }
                    continue;
                }
            }
            // Rule 2: an amount that does not fit its digit boxes (too many
            // digits, a negative that needs brackets, more than two decimals,
            // or no two centavo boxes to put them in) becomes one plain
            // rectangle across the full peso+centavo union, right-aligned
            // with thousands commas: never blank, never cut.
            let mut slots = peso_ordered;
            slots.extend(cent_ordered.unwrap_or_default());
            amount_rect(
                html,
                &tags,
                &slots,
                (&cells.peso_html_id, cent_cell),
                &rect_text,
                Some(value.as_str()),
                sheet,
                &mut replacements,
            );
        }
    }
    apply_replacements(html, replacements)
}

/// How a whole-peso comb (no centavo boxes) prints an amount.
#[derive(Debug, PartialEq)]
enum PesoPrint {
    /// Whole pesos, right-aligned in the boxes: `1,234.00` prints `1234`.
    Boxes(String),
    /// One plain rectangle over the comb: the upload carries centavos the
    /// boxes cannot hold (`-1,000.51` prints `-1,000.51`, never `-1001`; the
    /// official page shows the centavos the filer typed), or the value is
    /// not a plain amount.
    Rect(String),
}

/// What the whole-peso comb of a `style` sheet prints for `value`: the
/// upload's own figure, sign, digits and centavos, never rounded.
fn peso_print(value: &str, style: NegativeStyle) -> PesoPrint {
    match split_writer_money(value) {
        Some((negative, peso, cents)) if cents == "00" => {
            let peso = peso.trim_start_matches('0');
            let digits = if peso.is_empty() { "0" } else { peso };
            PesoPrint::Boxes(signed_digits(negative, digits, "", style))
        }
        Some((negative, peso, cents)) => {
            PesoPrint::Rect(format_rect_amount(negative, &peso, Some(&cents), style))
        }
        None => PesoPrint::Rect(format_comb_amount_for_rect(value, style)),
    }
}

#[allow(dead_code)]
fn fill_peso_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    peso_joins: &[PesoJoin],
) -> String {
    fill_peso_joins_with_sheet(
        html,
        fields,
        peso_joins,
        &SheetType::plain(),
        &BTreeSet::new(),
    )
}

fn fill_peso_joins_with_sheet(
    html: &str,
    fields: &BTreeMap<String, String>,
    peso_joins: &[PesoJoin],
    sheet: &SheetType,
    forced: &BTreeSet<String>,
) -> String {
    if peso_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in peso_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        let print = peso_print(value, join.negative_style);
        for html_id in &join.html_ids {
            let Some(indices) = grouped.get(html_id.as_str()) else {
                continue;
            };
            let ordered = ordered_comb_indices(&tags, indices);
            let text = match &print {
                PesoPrint::Boxes(pesos) => {
                    let fit = right_aligned_slot_values(ordered.len(), pesos)
                        .filter(|_| !forced.contains(html_id.as_str()));
                    if let Some(values) = fit {
                        for (offset, index) in ordered.into_iter().enumerate() {
                            let writer = (offset == 0).then_some(value.as_str());
                            replacements.push((
                                tags[index].start,
                                tags[index].end,
                                set_value(tags[index].tag, &values[offset], writer),
                            ));
                        }
                        continue;
                    }
                    format_comb_amount_for_rect(pesos, join.negative_style)
                }
                PesoPrint::Rect(text) => text.clone(),
            };
            amount_rect(
                html,
                &tags,
                &ordered,
                (html_id, None),
                &text,
                Some(value.as_str()),
                sheet,
                &mut replacements,
            );
        }
    }
    apply_replacements(html, replacements)
}

/// Splits a writer date into `(MM, DD, YYYY)`: `MM/DD/YYYY`, `MM/YYYY`
/// (no day) or ISO `YYYY-MM-DD`. Month and day are zero-padded to two
/// digits; anything else is not a date and prints nowhere.
fn split_writer_date(value: &str) -> Option<(String, Option<String>, String)> {
    let value = value.trim();
    let parts: Vec<&str> = if value.contains('-') {
        let iso: Vec<&str> = value.split('-').collect();
        match iso.as_slice() {
            [year, month, day] if year.len() == 4 => vec![month, day, year],
            _ => return None,
        }
    } else {
        value.split('/').collect()
    };
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return None;
    }
    let two = |part: &str| (part.len() <= 2).then(|| format!("{part:0>2}"));
    match parts.as_slice() {
        [month, day, year] if year.len() == 4 => {
            Some((two(month)?, Some(two(day)?), year.to_string()))
        }
        [month, year] if year.len() == 4 => Some((two(month)?, None, year.to_string())),
        _ => None,
    }
}

fn fill_date_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    date_joins: &[DateJoin],
) -> String {
    if date_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in date_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        let Some((month, day, year)) = split_writer_date(value) else {
            continue;
        };
        // A day the sheet has no comb for (or a comb with no day to fill)
        // is not this date's shape: print nothing rather than part of it.
        let day_cells = match (&join.dd_html_id, day) {
            (Some(html_id), Some(day)) => Some((html_id.as_str(), day)),
            (None, None) => None,
            _ => continue,
        };
        let combs = std::iter::once((join.mm_html_id.as_str(), month))
            .chain(day_cells)
            .chain(std::iter::once((join.yyyy_html_id.as_str(), year)));
        let mut pending = Vec::new();
        let mut fits = true;
        for (at, (html_id, mut digits)) in combs.enumerate() {
            let ordered = grouped
                .get(html_id)
                .map(|indices| ordered_comb_indices(&tags, indices))
                .unwrap_or_default();
            // A two-slot year comb takes the year's last two digits.
            if ordered.len() == 2 && digits.len() == 4 {
                digits = digits[2..].to_string();
            }
            if ordered.len() != digits.len() {
                fits = false;
                break;
            }
            for (offset, (index, ch)) in ordered.into_iter().zip(digits.chars()).enumerate() {
                let writer = (at == 0 && offset == 0).then_some(value.as_str());
                pending.push((
                    tags[index].start,
                    tags[index].end,
                    set_value(tags[index].tag, &ch.to_string(), writer),
                ));
            }
        }
        if fits {
            replacements.extend(pending);
        }
    }
    apply_replacements(html, replacements)
}

fn writer_is_checked(value: &str) -> bool {
    matches!(value.trim(), "true" | "TRUE" | "1" | "Y" | "y" | "X" | "x")
}

fn fill_xbox_joins(
    html: &str,
    fields: &BTreeMap<String, String>,
    xbox_joins: &[XboxJoin],
) -> String {
    if xbox_joins.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for join in xbox_joins {
        let Some(value) = fields.get(&join.writer_key) else {
            continue;
        };
        if !writer_is_checked(value) {
            continue;
        }
        for html_id in &join.html_ids {
            let Some(index) = xbox_input(&tags, &grouped, html_id) else {
                continue;
            };
            // Char boxes carry `fc` (text-align:center); the catalog xbox inputs
            // do not, so the X sat against the box's left wall.
            replacements.push((
                tags[index].start,
                tags[index].end,
                with_class(
                    &set_value(tags[index].tag, XBOX_CHECKED_GLYPH, Some(value.as_str())),
                    "fc",
                ),
            ));
        }
    }
    apply_replacements(html, replacements)
}

/// The one input an xbox join marks: the single unslotted input of cell
/// `html_id`, or, in a cell that holds several boxes under one name, the
/// box whose input id is `html_id` (`<cell>-i<n>`).
fn xbox_input(
    tags: &[InputTag<'_>],
    grouped: &BTreeMap<&str, Vec<usize>>,
    html_id: &str,
) -> Option<usize> {
    let index = match grouped.get(html_id) {
        Some(indices) if indices.len() == 1 => indices[0],
        Some(_) => return None,
        None => tags
            .iter()
            .position(|tag| attr(tag.tag, "id") == Some(html_id))?,
    };
    tags[index].slot.is_none().then_some(index)
}

/// The tag with `class` present in its class list (added if missing).
fn with_class(tag: &str, class: &str) -> String {
    let needle = "class=\"";
    let Some(at) = tag.find(needle) else {
        let insert_at = tag.rfind('>').unwrap_or(tag.len());
        return format!(
            "{} class=\"{class}\"{}",
            &tag[..insert_at],
            &tag[insert_at..]
        );
    };
    let start = at + needle.len();
    let Some(len) = tag[start..].find('"') else {
        return tag.to_string();
    };
    let classes = &tag[start..start + len];
    if classes.split_whitespace().any(|c| c == class) {
        return tag.to_string();
    }
    let joined = if classes.trim().is_empty() {
        class.to_string()
    } else {
        format!("{classes} {class}")
    };
    format!("{}{}{}", &tag[..start], joined, &tag[start + len..])
}

/// A raw sheet cell id used as a field key (`p4c213`), as opposed to a
/// writer key from the form's print map. Writer-cell xbox pseudo ids
/// (`cell-i2`) are raw cell addressing too.
fn raw_cell_id_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'p' {
        return false;
    }
    let mut at = 1;
    let digits = |at: &mut usize| {
        let start = *at;
        while *at < bytes.len() && bytes[*at].is_ascii_digit() {
            *at += 1;
        }
        *at > start
    };
    if !digits(&mut at) || at >= bytes.len() || bytes[at] != b'c' {
        return false;
    }
    at += 1;
    if !digits(&mut at) {
        return false;
    }
    if at == bytes.len() {
        return true;
    }
    if at + 2 > bytes.len() || bytes[at] != b'-' || bytes[at + 1] != b'i' {
        return false;
    }
    at += 2;
    digits(&mut at) && at == bytes.len()
}

fn claim_input_group<'a>(
    grouped: &BTreeMap<&str, Vec<usize>>,
    claims: &mut BTreeMap<usize, Vec<(&'a str, bool)>>,
    key: &'a str,
    writer: bool,
    group: &str,
) {
    if let Some(indices) = grouped.get(group) {
        for &index in indices {
            claims.entry(index).or_default().push((key, writer));
        }
    }
}

fn claim_writer_key_inputs<'a>(
    tags: &[InputTag],
    grouped: &BTreeMap<&str, Vec<usize>>,
    cells: &WriterCells,
    key: &'a str,
    xbox_active: bool,
    claims: &mut BTreeMap<usize, Vec<(&'a str, bool)>>,
) {
    for (writer, ids) in &cells.joins {
        if writer == key {
            for id in ids {
                claim_input_group(grouped, claims, key, true, id);
            }
        }
    }
    for join in &cells.money_joins {
        if join.writer_key == key {
            for pair in &join.cells {
                claim_input_group(grouped, claims, key, true, &pair.peso_html_id);
                if let Some(cent) = &pair.cent_html_id {
                    claim_input_group(grouped, claims, key, true, cent);
                }
            }
        }
    }
    for join in &cells.peso_joins {
        if join.writer_key == key {
            for id in &join.html_ids {
                claim_input_group(grouped, claims, key, true, id);
            }
        }
    }
    for join in &cells.date_joins {
        if join.writer_key == key {
            claim_input_group(grouped, claims, key, true, &join.mm_html_id);
            if let Some(dd) = &join.dd_html_id {
                claim_input_group(grouped, claims, key, true, dd);
            }
            claim_input_group(grouped, claims, key, true, &join.yyyy_html_id);
        }
    }
    if xbox_active {
        for join in &cells.xbox_joins {
            if join.writer_key != key {
                continue;
            }
            for id in &join.html_ids {
                if let Some(index) = xbox_input(tags, grouped, id) {
                    claims.entry(index).or_default().push((key, true));
                }
            }
        }
    }
}

/// Inputs claimed by more than one writer key in a print map. Xbox claims
/// are topological here: a map that puts two writer keys on one box is
/// conflicting even when a particular draft checks only one of them.
#[cfg(test)]
fn writer_input_claim_conflicts<'a>(
    html: &str,
    cells: &'a WriterCells,
) -> Vec<(usize, BTreeSet<&'a str>)> {
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut claims: BTreeMap<usize, Vec<(&str, bool)>> = BTreeMap::new();
    for key in cells.joined_keys() {
        claim_writer_key_inputs(&tags, &grouped, cells, key, true, &mut claims);
    }
    claims
        .into_iter()
        .filter_map(|(index, claims)| {
            let keys: BTreeSet<&str> = claims.into_iter().map(|(key, _)| key).collect();
            (keys.len() > 1).then_some((index, keys))
        })
        .collect()
}

/// Resolve field keys whose writes target the same input before any byte
/// replacement is made. A writer key wins over raw cell-id addressing of
/// the same input; two writer keys for one input are refused. This is the
/// print input's conflict boundary: raw ids are diagnostic/ad hoc input,
/// while writer keys are the form's own print map.
fn resolve_overlapping_input_writes(
    slug: &str,
    html: &str,
    fields: &BTreeMap<String, String>,
    cells: &WriterCells,
) -> Result<BTreeMap<String, String>, String> {
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut claims: BTreeMap<usize, Vec<(&str, bool)>> = BTreeMap::new();

    for key in fields.keys() {
        let key = key.as_str();
        if raw_cell_id_key(key) {
            claim_input_group(&grouped, &mut claims, key, false, key);
            continue;
        }

        // A writer key that is itself a stamped input name takes the direct
        // write path, not a writer-cell join path.
        claim_input_group(&grouped, &mut claims, key, true, key);
        // At print time only a checked xbox key makes its mark; the static
        // writer-map invariant treats the same targets as conflicting claims.
        let xbox_active = fields
            .get(key)
            .is_some_and(|value| writer_is_checked(value));
        claim_writer_key_inputs(&tags, &grouped, cells, key, xbox_active, &mut claims);
    }
    let mut resolved = fields.clone();
    for (index, claims) in claims {
        let writers: BTreeSet<&str> = claims
            .iter()
            .filter(|(_, writer)| *writer)
            .map(|(key, _)| *key)
            .collect();
        let raws: BTreeSet<&str> = claims
            .iter()
            .filter(|(_, writer)| !*writer)
            .map(|(key, _)| *key)
            .collect();
        if writers.len() > 1 {
            let mut keys = writers.iter();
            let first = keys.next().unwrap();
            let second = keys.next().unwrap();
            return Err(format!(
                "{slug}: writer keys {first} and {second} both write input {}",
                tags[index].name
            ));
        }
        if writers.len() == 1 {
            for raw in raws {
                resolved.remove(raw);
            }
        }
    }
    Ok(resolved)
}

fn validate_writer_cells(html: &str, slug: &str, cells: &WriterCells) -> Result<(), String> {
    if !cells.has_fill_join() && cells.not_joined.is_empty() && cells.not_joined_cells.is_empty() {
        return Ok(());
    }
    let tags = input_tags(html);
    let present: BTreeSet<&str> = tags.iter().map(|tag| tag.name).collect();
    let grouped = grouped_input_indices(&tags);
    let slots = |html_id: &str| grouped.get(html_id).map_or(0, Vec::len);
    let xbox_ids: BTreeSet<&str> = cells
        .xbox_joins
        .iter()
        .flat_map(|join| join.html_ids.iter().map(String::as_str))
        .collect();
    for (key, html_id) in cells.targets() {
        if !present.contains(html_id) && !xbox_ids.contains(html_id) {
            return Err(format!(
                "{slug}: writer-cells html id {html_id} for {key} is not an input name="
            ));
        }
        if present.contains(key) {
            return Err(format!(
                "{slug}: writer-cells {key} is already a stamped name=; remove the fill join"
            ));
        }
    }
    for join in &cells.money_joins {
        for pair in &join.cells {
            // A decimal comb also holds the point and two centavos.
            let least = if pair.cent_html_id.is_some() { 3 } else { 6 };
            if slots(&pair.peso_html_id) < least {
                return Err(format!(
                    "{slug}: writer-cells {} peso comb {} must have at least {least} slots",
                    join.writer_key, pair.peso_html_id
                ));
            }
            if let Some(cent) = pair.cent_html_id.as_ref().filter(|cent| slots(cent) != 2) {
                return Err(format!(
                    "{slug}: writer-cells {} cent comb {cent} must be a 2-slot comb",
                    join.writer_key
                ));
            }
        }
    }
    for join in &cells.peso_joins {
        for html_id in &join.html_ids {
            if slots(html_id) < 3 {
                return Err(format!(
                    "{slug}: writer-cells {} peso comb {html_id} must have at least 3 slots",
                    join.writer_key
                ));
            }
        }
    }
    for join in &cells.date_joins {
        let parts = [
            (Some(&join.mm_html_id), &[2][..]),
            (join.dd_html_id.as_ref(), &[2][..]),
            (Some(&join.yyyy_html_id), &[2, 4][..]),
        ];
        for (html_id, allowed) in parts {
            let Some(html_id) = html_id else {
                continue;
            };
            let indices = grouped.get(html_id.as_str()).cloned().unwrap_or_default();
            let comb = indices.iter().all(|&index| tags[index].slot.is_some());
            if !comb || !allowed.contains(&indices.len()) {
                return Err(format!(
                    "{slug}: writer-cells {} date comb {html_id} must be a {allowed:?}-slot comb",
                    join.writer_key
                ));
            }
        }
    }
    for join in &cells.xbox_joins {
        for html_id in &join.html_ids {
            if xbox_input(&tags, &grouped, html_id).is_none() {
                return Err(format!(
                    "{slug}: writer-cells {} xbox {html_id} must be a single unslotted input",
                    join.writer_key
                ));
            }
        }
    }
    for key in cells.not_joined.keys() {
        if present.contains(key.as_str()) {
            return Err(format!(
                "{slug}: writer-cells not_joined {key} is a stamped name= and prints"
            ));
        }
    }
    for html_id in cells.not_joined_cells.keys() {
        if !html.contains(&format!("<div id=\"{html_id}\"")) {
            return Err(format!(
                "{slug}: writer-cells not_joined_cells {html_id} is not a cell on the sheet"
            ));
        }
    }
    Ok(())
}

/// Fill a cloned-band continuation page fragment through the exact same fill
/// path (`line_group_plan`, `fill_by_name_set`, `fill_money_joins`,
/// `fill_peso_joins`, `fill_date_joins`, `split_single_character_cells`,
/// `fill_xbox_joins`) used for official pages.
pub(crate) fn fill_continuation_fragment(
    html: &str,
    fields: &BTreeMap<String, String>,
    slug: &str,
    extra_joins: &BTreeMap<String, Vec<String>>,
    extra_money_joins: &[(String, String, Option<String>)],
    extra_peso_joins: &[(String, String)],
    caption_aliases: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut cells = writer_cells(slug)?;
    for (syn_id, orig_id) in caption_aliases {
        if let Some(cap) = cells.captions.get(orig_id).cloned() {
            cells.captions.insert(syn_id.clone(), cap);
        }
    }
    for (k, v) in extra_joins {
        cells.joins.entry(k.clone()).or_default().extend(v.clone());
    }
    for (k, peso_id, cents_id) in extra_money_joins {
        let negative_style = NegativeStyle::of_sheet(slug);
        cells.money_joins.push(MoneyJoin {
            writer_key: k.clone(),
            cells: vec![MoneyCells {
                peso_html_id: peso_id.clone(),
                cent_html_id: cents_id.clone(),
                point_slot: false,
            }],
            negative_style,
        });
    }
    for (k, html_id) in extra_peso_joins {
        let negative_style = NegativeStyle::of_sheet(slug);
        cells.peso_joins.push(PesoJoin {
            writer_key: k.clone(),
            html_ids: vec![html_id.clone()],
            negative_style,
        });
    }
    let donor_fields = with_donor_estate_values(slug, fields);
    let resolved_fields = resolve_overlapping_input_writes(slug, html, &donor_fields, &cells)?;
    let fields = &resolved_fields;
    let filled = fill_donor_estate_dates(html, fields, slug);
    let sheet = bundle(slug).map_or_else(SheetType::plain, |loaded| {
        SheetType::of_sheet(slug, loaded.css)
    });
    let base = filled;
    let plan = line_group_plan(&base, fields, &cells, &sheet);
    // Uniform amount sections (#477): fill once, then widen any per-cell
    // conversion to its whole (section, column) group and fill again.
    let fill_pass = |forced: &BTreeSet<String>| -> Result<String, String> {
        let filled = try_fill_by_name_set(
            &base,
            fields,
            &cells.joins,
            &cells.captions,
            &plan,
            &sheet,
            forced,
        )?;
        let filled =
            fill_money_joins_with_sheet(&filled, fields, &cells.money_joins, &sheet, forced);
        let filled = fill_peso_joins_with_sheet(&filled, fields, &cells.peso_joins, &sheet, forced);
        let filled = fill_date_joins(&filled, fields, &cells.date_joins);
        let filled = split_single_character_cells(
            slug,
            &fill_xbox_joins(&filled, fields, &cells.xbox_joins),
        );
        let filled = fill_preprinted_branch_combs(&filled, fields, &cells.joins);
        let cent_pairs = cent_pair_map(&cells.money_joins);
        let filled = blank_empty_forced_cells(&filled, forced, &sheet, &cent_pairs);
        let filled = unify_group_font_sizes(&filled);
        let filled = reconcile_amount_warning_marks(&filled, forced);
        Ok(strip_converted_rect_separators(&filled))
    };
    let first = fill_pass(&BTreeSet::new())?;
    let forced = uniform_amount_forced_cells(html, &converted_amount_cells(&first));
    if forced.is_empty() {
        return Ok(first);
    }
    fill_pass(&forced)
}

/// Promote any empty `<div id="{html_id}" ...></div>` cell explicitly joined in
/// `writer-cells.json` (e.g. `1600wp-2010` Schedule 1.1 totals `p1c241`/`p1c243`,
/// which the upstream generator emitted as empty `data-cell-kind="label"` boxes)
/// into a fillable `<input>` without editing the frozen sheet on disk.
fn promote_empty_joined_cells(html: &str, cells: &WriterCells) -> String {
    if cells.joins.is_empty() {
        return html.to_string();
    }
    let existing: BTreeSet<&str> = input_tags(html).into_iter().map(|t| t.name).collect();
    let mut missing: Vec<&str> = Vec::new();
    for html_ids in cells.joins.values() {
        for html_id in html_ids {
            if !existing.contains(html_id.as_str()) && !missing.contains(&html_id.as_str()) {
                missing.push(html_id.as_str());
            }
        }
    }
    if missing.is_empty() {
        return html.to_string();
    }
    let mut out = html.to_string();
    for html_id in missing {
        let needle = format!("<div id=\"{html_id}\"");
        let Some(start) = out.find(&needle) else {
            continue;
        };
        let Some(gt_rel) = out[start..].find('>') else {
            continue;
        };
        let open_end = start + gt_rel + 1;
        // Only promote empty <div></div> cells so real printed text labels are never overwritten.
        if !out[open_end..].starts_with("</div>") {
            continue;
        }
        let injected = format!(
            "<input type=\"text\" class=\"fi fh5\" id=\"{html_id}-i\" name=\"{html_id}\" style=\"inset:0.96pt 1.92pt 0.96pt 2pt\" autocomplete=\"off\" spellcheck=\"false\">"
        );
        out.insert_str(open_end, &injected);
    }
    out
}

fn fill_bundle(
    html: &str,
    fields: &BTreeMap<String, String>,
    slug: &str,
) -> Result<String, String> {
    let cells = writer_cells(slug)?;
    let promoted = promote_empty_joined_cells(html, &cells);
    let html = promoted.as_str();
    validate_writer_cells(html, slug, &cells)?;
    let donor_fields = with_donor_estate_values(slug, fields);
    let resolved_fields = resolve_overlapping_input_writes(slug, html, &donor_fields, &cells)?;
    let fields = &resolved_fields;
    let base = fill_donor_estate_dates(html, fields, slug);
    let sheet = bundle(slug).map_or_else(SheetType::plain, |loaded| {
        SheetType::of_sheet(slug, loaded.css)
    });
    let plan = line_group_plan(&base, fields, &cells, &sheet);
    // Uniform amount sections (#477): fill once, then widen any per-cell
    // conversion to its whole (section, column) group and fill again.
    let fill_pass = |forced: &BTreeSet<String>| -> Result<String, String> {
        let filled = try_fill_by_name_set(
            &base,
            fields,
            &cells.joins,
            &cells.captions,
            &plan,
            &sheet,
            forced,
        )?;
        let filled =
            fill_money_joins_with_sheet(&filled, fields, &cells.money_joins, &sheet, forced);
        let filled = fill_peso_joins_with_sheet(&filled, fields, &cells.peso_joins, &sheet, forced);
        let filled = fill_date_joins(&filled, fields, &cells.date_joins);
        let filled = split_single_character_cells(
            slug,
            &fill_xbox_joins(&filled, fields, &cells.xbox_joins),
        );
        let filled = fill_preprinted_branch_combs(&filled, fields, &cells.joins);
        let cent_pairs = cent_pair_map(&cells.money_joins);
        let filled = blank_empty_forced_cells(&filled, forced, &sheet, &cent_pairs);
        let filled = unify_group_font_sizes(&filled);
        let filled = reconcile_amount_warning_marks(&filled, forced);
        let filled = strip_converted_rect_separators(&filled);
        Ok(apply_rule6_specify_bands(&filled, fields, &sheet))
    };
    let first = fill_pass(&BTreeSet::new())?;
    let forced = uniform_amount_forced_cells(html, &converted_amount_cells(&first));
    if forced.is_empty() {
        return Ok(first);
    }
    fill_pass(&forced)
}

#[derive(serde::Deserialize)]
struct Rule6PageLayoutDoc {
    pages: Vec<Rule6PageLayout>,
}

#[derive(serde::Deserialize)]
struct Rule6PageLayout {
    page: usize,
    bands: Vec<Rule6Band>,
    #[serde(default)]
    rule6_fields: Vec<Rule6FieldSpec>,
}

#[derive(serde::Deserialize)]
struct Rule6Band {
    id: String,
    kind: String,
    y0: f64,
    y1: f64,
    min_height_pt: Option<f64>,
}

#[derive(serde::Deserialize)]
struct Rule6FieldSpec {
    writer_key: String,
    cell_id: String,
    band_id: String,
    insert_after_y: f64,
    x0: f64,
    x1: f64,
}

struct Rule6Insertion {
    cell_id: String,
    cut_y: f64,
    dh: f64,
    x0: f64,
    x1: f64,
    display: String,
    full_text: String,
    font_size_pt: f64,
    two_lines: bool,
    truncated: bool,
    input_classes: String,
}

/// RULES.md rule 6: when a named inline "(specify)" field in
/// `<script id="formgen-page-layout">` overflows its small label-column box,
/// move its text to a full-width rectangle on a newly inserted band directly
/// under the item's row while keeping the amount box level with the item
/// number and shifting everything below down (shrinking an `elastic_box` band
/// if trailing space alone is not enough).
fn apply_rule6_specify_bands(
    html: &str,
    fields: &BTreeMap<String, String>,
    sheet: &SheetType,
) -> String {
    const START_TAG: &str = "<script type=\"application/json\" id=\"formgen-page-layout\">";
    let Some(s_at) = html.find(START_TAG) else {
        return html.to_string();
    };
    let json_start = s_at + START_TAG.len();
    let Some(e_rel) = html[json_start..].find("</script>") else {
        return html.to_string();
    };
    let Ok(layout) =
        serde_json::from_str::<Rule6PageLayoutDoc>(&html[json_start..json_start + e_rel])
    else {
        return html.to_string();
    };
    if !layout.pages.iter().any(|p| !p.rule6_fields.is_empty()) {
        return html.to_string();
    }

    let mut out = html.to_string();
    for page in &layout.pages {
        if page.rule6_fields.is_empty() {
            continue;
        }
        let Some((pg_start, pg_end)) = find_page_div_span(&out, page.page) else {
            continue;
        };
        let mut page_html = out[pg_start..pg_end].to_string();
        let mut insertions: Vec<Rule6Insertion> = Vec::new();
        let value_cells = page_value_cells(&page_html);

        for r6 in &page.rule6_fields {
            let Some(full_val) = fields
                .get(&r6.writer_key)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
            else {
                continue;
            };
            let Some((inp_start, inp_end, inp_tag)) = find_cell_input_span(&page_html, &r6.cell_id)
            else {
                continue;
            };
            let style = attr(&inp_tag, "style").unwrap_or("");
            let overflowed = inp_tag.contains("data-overflow-warning=\"text-truncated\"")
                || style.contains("font-size:");
            if !overflowed {
                continue;
            }
            let input_classes = "fi".to_string();
            let letter = sheet.letter;

            let base_cut_y = page
                .bands
                .iter()
                .find(|b| b.id == r6.band_id)
                .map(|b| {
                    if b.y1 > r6.insert_after_y && (b.y1 - r6.insert_after_y) <= 6.0 {
                        b.y1
                    } else {
                        r6.insert_after_y
                    }
                })
                .unwrap_or(r6.insert_after_y);

            let (x1, cut_y) = rule6_place_band(r6.x0, r6.x1, base_cut_y, &r6.cell_id, &value_cells);

            let rect_w = rule6_band_width(r6.x0, x1);
            let room = shrunk_room((rect_w - 6.0).max(20.0));
            let single_size = sheet.fit(letter, room, full_val, 8.5);
            let (dh, display, font_size_pt, two_lines, truncated) =
                if single_size >= PLAIN_TEXT_MIN_PT {
                    (14.0, full_val.to_string(), single_size, false, false)
                } else if let Some((l1, l2, sz)) =
                    fit_two_lines(sheet, letter, room, full_val, 8.5, PLAIN_TEXT_MIN_PT)
                {
                    (20.0, format!("{l1}\n{l2}"), sz, true, false)
                } else if let Some((l1, l2)) = wrap_and_truncate_two_lines_mode(
                    sheet,
                    letter,
                    room,
                    full_val,
                    PLAIN_TEXT_MIN_PT,
                    true,
                ) {
                    (20.0, format!("{l1}\n{l2}"), PLAIN_TEXT_MIN_PT, true, true)
                } else {
                    (
                        14.0,
                        truncate_with_ellipsis_mode(
                            sheet,
                            letter,
                            room,
                            full_val,
                            PLAIN_TEXT_MIN_PT,
                            true,
                        )
                        .unwrap_or_else(|| full_val.to_string()),
                        PLAIN_TEXT_MIN_PT,
                        false,
                        true,
                    )
                };

            let cleared = clear_rule6_inline_input(&inp_tag);
            page_html.replace_range(inp_start..inp_end, &cleared);

            insertions.push(Rule6Insertion {
                cell_id: r6.cell_id.clone(),
                cut_y,
                dh,
                x0: r6.x0,
                x1,
                display,
                full_text: full_val.to_string(),
                font_size_pt,
                two_lines,
                truncated,
                input_classes,
            });
        }

        if insertions.is_empty() {
            continue;
        }
        insertions.sort_by(|a, b| a.cut_y.total_cmp(&b.cut_y));

        let total_dh: f64 = insertions.iter().map(|i| i.dh).sum();
        let trailing_room: f64 = (page
            .bands
            .iter()
            .filter(|b| b.kind == "trailing_space")
            .map(|b| (b.y1 - b.y0).max(0.0))
            .sum::<f64>()
            - 4.0)
            .max(0.0);
        let last_cut = insertions.last().map_or(0.0, |i| i.cut_y);
        let (elastic_cut_y, elastic_shrink) = if total_dh > trailing_room {
            let needed = total_dh - trailing_room;
            page.bands
                .iter()
                .find(|b| b.kind == "elastic_box" && b.y0 >= last_cut - 0.5)
                .and_then(|eb| {
                    let min_h = eb.min_height_pt?;
                    let avail = (eb.y1 - eb.y0 - min_h).max(0.0);
                    let shrink = needed.min(avail);
                    (shrink > 0.1).then_some((eb.y0 + min_h, shrink))
                })
                .unwrap_or((f64::INFINITY, 0.0))
        } else {
            (f64::INFINITY, 0.0)
        };

        let transformed =
            shift_page_and_insert_rule6(&page_html, &insertions, elastic_cut_y, elastic_shrink);
        out.replace_range(pg_start..pg_end, &transformed);
    }
    out
}

/// How narrow a Rule 6 band may become by narrowing, in pt: below this floor
/// the band moves down below the obstructing cell instead, so the text always
/// has room a person can read.
const RULE6_BAND_FLOOR_PT: f64 = 120.0;

/// The width a Rule 6 band's text is fitted to and drawn at: one value, so
/// nothing is ever fitted wider than it is drawn (a value fitted to a wider
/// measure would print clipped, and the fitter would not carry it to the
/// additional sheet).
fn rule6_band_width(x0: f64, x1: f64) -> f64 {
    (x1 - x0 - 3.0).max(10.0)
}

/// The owner's placement rule: the band runs from its cell's left to just
/// before the first other value cell its strip crosses (a 2 pt gap), so an
/// amount on the row is never covered; if narrowing would squeeze it below
/// [`RULE6_BAND_FLOOR_PT`], or it would still cross a value cell, it moves
/// down below that cell instead. Only cells starting above the cut matter:
/// anything below it shifts down with the rest of the page, out of the
/// inserted strip. Returns the band's right edge and its cut height.
fn rule6_place_band(
    x0: f64,
    x1_full: f64,
    base_cut_y: f64,
    cell_id: &str,
    value_cells: &[(String, f64, f64, f64, f64)],
) -> (f64, f64) {
    let mut x1 = x1_full;
    let mut cut_y = base_cut_y;
    for _ in 0..8 {
        let mut narrow: Option<f64> = None;
        let mut move_below: Option<f64> = None;
        for (id, left, top, _width, height) in value_cells {
            if *id == cell_id {
                continue;
            }
            let bottom = top + height;
            // Cells starting at or below the cut shift down with the page.
            // The band's top edge lies 0.5 pt under the cut, so a cell ending
            // above that never meets it; an overlap of 0.05 pt or less is
            // below the assertion's measure.
            if *top >= cut_y - 0.05 || bottom - (cut_y + 0.5) <= 0.05 {
                continue;
            }
            let limit = *left - 2.0;
            if *left > x0 + 1.0 && rule6_band_width(x0, limit) >= RULE6_BAND_FLOOR_PT {
                narrow = Some(narrow.map_or(limit, |n: f64| n.min(limit)));
            } else {
                move_below = Some(move_below.map_or(bottom, |m: f64| m.max(bottom)));
            }
        }
        if let Some(bottom) = move_below {
            cut_y = bottom + 0.5;
            continue;
        }
        if let Some(limit) = narrow {
            if limit < x1 - 0.05 {
                x1 = limit;
                continue;
            }
        }
        break;
    }
    (x1, cut_y)
}

/// `(id, left, top, width, height)` of every `data-cell-kind` `field`/`mixed`
/// div of a page, in document order, for the Rule 6 placement rule.
fn page_value_cells(page_html: &str) -> Vec<(String, f64, f64, f64, f64)> {
    let mut cells = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = page_html[cursor..].find("<div ") {
        let start = cursor + rel;
        let Some(end_rel) = page_html[start..].find('>') else {
            break;
        };
        let end = start + end_rel + 1;
        let tag = &page_html[start..end];
        if matches!(attr(tag, "data-cell-kind"), Some("field" | "mixed")) {
            let geometry = ["left", "top", "width", "height"].map(|p| style_pt_prop(tag, p));
            if let (Some(id), [Some(left), Some(top), Some(width), Some(height)]) =
                (attr(tag, "id"), geometry)
            {
                cells.push((id.to_string(), left, top, width, height));
            }
        }
        cursor = end;
    }
    cells
}

fn find_page_div_span(html: &str, page_num: usize) -> Option<(usize, usize)> {
    let needle = format!("id=\"page-{page_num}\"");
    let id_at = html.find(&needle)?;
    let start = html[..id_at].rfind("<div ")?;
    let mut depth: i32 = 0;
    let mut i = start;
    while i < html.len() {
        let next_open = html[i..].find("<div").map(|o| i + o);
        let next_close = html[i..].find("</div>").map(|o| i + o);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                i = o + 4;
            }
            (_, Some(c)) => {
                depth -= 1;
                i = c + 6;
                if depth == 0 {
                    return Some((start, i));
                }
            }
            _ => break,
        }
    }
    None
}

fn find_cell_input_span(page_html: &str, cell_id: &str) -> Option<(usize, usize, String)> {
    let div_needle = format!("id=\"{cell_id}\"");
    let div_at = page_html.find(&div_needle)?;
    let div_close = page_html[div_at..].find("</div>")? + div_at;
    let slice = &page_html[div_at..div_close];
    let inp_rel = slice.find("<input")?;
    let inp_end_rel = slice[inp_rel..].find('>')? + inp_rel + 1;
    let abs_start = div_at + inp_rel;
    let abs_end = div_at + inp_end_rel;
    Some((
        abs_start,
        abs_end,
        page_html[abs_start..abs_end].to_string(),
    ))
}

fn clear_rule6_inline_input(tag: &str) -> String {
    let mut cleaned = set_value(tag, "", None);
    for attr_name in [
        "data-overflow-warning",
        "data-writer-value",
        "data-overflow-of",
    ] {
        cleaned = remove_html_attr(&cleaned, attr_name);
    }
    cleaned
}

fn remove_html_attr(tag: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(start) = tag.find(&needle) else {
        return tag.to_string();
    };
    let val_start = start + needle.len();
    let Some(end_rel) = tag[val_start..].find('"') else {
        return tag.to_string();
    };
    format!("{}{}", &tag[..start], &tag[val_start + end_rel + 1..])
}

fn rule6_delta_top(
    y: f64,
    insertions: &[Rule6Insertion],
    elastic_cut_y: f64,
    elastic_shrink: f64,
) -> f64 {
    let mut d = 0.0;
    for ins in insertions {
        if y >= ins.cut_y - 0.5 {
            d += ins.dh;
        }
    }
    if elastic_shrink > 0.0 && y >= elastic_cut_y - 0.5 {
        d -= elastic_shrink;
    }
    d
}

fn rule6_delta_bot(
    y_top: f64,
    y_bot: f64,
    insertions: &[Rule6Insertion],
    elastic_cut_y: f64,
    elastic_shrink: f64,
) -> f64 {
    let mut d = 0.0;
    for ins in insertions {
        if y_top >= ins.cut_y - 0.5 || y_bot > ins.cut_y + 1.5 {
            d += ins.dh;
        }
    }
    if elastic_shrink > 0.0 && (y_top >= elastic_cut_y - 0.5 || y_bot > elastic_cut_y + 1.5) {
        d -= elastic_shrink;
    }
    d
}

fn shift_page_and_insert_rule6(
    page_html: &str,
    insertions: &[Rule6Insertion],
    elastic_cut_y: f64,
    elastic_shrink: f64,
) -> String {
    let svg_end = page_html.find("</svg>");
    let mut out = String::with_capacity(page_html.len() + 2048);

    let body_start = if let Some(se) = svg_end {
        let svg_part = &page_html[..se];
        // Vertical frame lines (column dividers, table edges) are drawn in
        // per-row segments that exactly abut at a row boundary: one segment
        // ends at the boundary, the next row's segment starts there. A strip
        // inserted at that boundary opens a gap in the line unless the upper
        // segment is stretched through the strip. Comb ticks abut nothing
        // below the boundary, so the same-x pairing keeps them out.
        let mut verticals: Vec<(f64, f64, f64)> = Vec::new();
        for m in svg_part.match_indices("<rect ") {
            let start = m.0;
            let Some(end_rel) = svg_part[start..].find('>') else {
                continue;
            };
            let tag = &svg_part[start..start + end_rel + 1];
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                attr(tag, "x").and_then(|s| s.parse::<f64>().ok()),
                attr(tag, "y").and_then(|s| s.parse::<f64>().ok()),
                attr(tag, "width").and_then(|s| s.parse::<f64>().ok()),
                attr(tag, "height").and_then(|s| s.parse::<f64>().ok()),
            ) {
                if w <= 1.6 && h >= 3.0 {
                    verticals.push((x, y, h));
                }
            }
        }
        let continues_below = |x: f64, cut: f64| {
            verticals
                .iter()
                .any(|&(vx, vy, _vh)| (vx - x).abs() <= 1.0 && (vy - cut).abs() <= 0.75)
        };
        let mut cursor = 0;
        while let Some(rel) = svg_part[cursor..].find("<rect ") {
            let start = cursor + rel;
            out.push_str(&svg_part[cursor..start]);
            let Some(end_rel) = svg_part[start..].find('>') else {
                out.push_str(&svg_part[start..]);
                cursor = svg_part.len();
                break;
            };
            let end = start + end_rel + 1;
            let tag = &svg_part[start..end];
            if let (Some(y_val), Some(h_val)) = (
                attr(tag, "y").and_then(|s| s.parse::<f64>().ok()),
                attr(tag, "height").and_then(|s| s.parse::<f64>().ok()),
            ) {
                let x_val = attr(tag, "x").and_then(|s| s.parse::<f64>().ok());
                let w_val = attr(tag, "width").and_then(|s| s.parse::<f64>().ok());
                let d_top = rule6_delta_top(y_val, insertions, elastic_cut_y, elastic_shrink);
                let mut d_bot = rule6_delta_bot(
                    y_val,
                    y_val + h_val,
                    insertions,
                    elastic_cut_y,
                    elastic_shrink,
                );
                if let (Some(x), Some(w)) = (x_val, w_val) {
                    if w <= 1.6 && h_val >= 3.0 && d_top.abs() <= 1e-4 {
                        for ins in insertions {
                            if ((y_val + h_val) - ins.cut_y).abs() <= 0.75
                                && continues_below(x, ins.cut_y)
                            {
                                d_bot += ins.dh;
                            }
                        }
                    }
                }
                if d_top.abs() > 1e-4 || (d_bot - d_top).abs() > 1e-4 {
                    let new_y = y_val + d_top;
                    let new_h = (h_val + d_bot - d_top).max(0.1);
                    let mut updated = replace_attr_value(tag, "y", &format!("{new_y:.2}"));
                    updated = replace_attr_value(&updated, "height", &format!("{new_h:.2}"));
                    out.push_str(&updated);
                } else {
                    out.push_str(tag);
                }
            } else {
                out.push_str(tag);
            }
            cursor = end;
        }
        out.push_str(&svg_part[cursor..]);

        let mut prior_dh = 0.0;
        out.push_str("<g class=\"layer-rule6\">");
        for ins in insertions {
            let y = ins.cut_y + prior_dh;
            let w = (ins.x1 - ins.x0).max(10.0);
            let h = ins.dh;
            out.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\"/>",
                ins.x0 + 1.0,
                y + 0.25,
                (w - 2.0).max(1.0),
                (h - 0.5).max(1.0)
            ));
            out.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"0.5\" fill=\"#000000\"/>",
                ins.x0, y, w
            ));
            out.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"0.5\" fill=\"#000000\"/>",
                ins.x0,
                y + h - 0.5,
                w
            ));
            out.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"1.2\" height=\"{:.2}\" fill=\"#000000\"/>",
                ins.x0, y, h
            ));
            out.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"1.2\" height=\"{:.2}\" fill=\"#000000\"/>",
                ins.x1 - 1.2,
                y,
                h
            ));
            prior_dh += ins.dh;
        }
        out.push_str("</g></svg>");
        se + "</svg>".len()
    } else {
        0
    };

    let rest = &page_html[body_start..];
    let last_close = rest.rfind("</div>").unwrap_or(rest.len());
    let inner_divs = &rest[..last_close];
    let mut cursor = 0;
    while let Some(rel) = inner_divs[cursor..].find("<div ") {
        let start = cursor + rel;
        out.push_str(&inner_divs[cursor..start]);
        let Some(end_rel) = inner_divs[start..].find('>') else {
            out.push_str(&inner_divs[start..]);
            cursor = inner_divs.len();
            break;
        };
        let end = start + end_rel + 1;
        let tag = &inner_divs[start..end];
        if let Some(top_val) = style_pt_prop(tag, "top") {
            let d_top = rule6_delta_top(top_val, insertions, elastic_cut_y, elastic_shrink);
            let mut updated = if d_top.abs() > 1e-4 {
                replace_style_pt_prop(tag, "top", top_val + d_top)
            } else {
                tag.to_string()
            };
            if let Some(h_val) = style_pt_prop(&updated, "height") {
                let d_bot = rule6_delta_bot(
                    top_val,
                    top_val + h_val,
                    insertions,
                    elastic_cut_y,
                    elastic_shrink,
                );
                if (d_bot - d_top).abs() > 1e-4 {
                    updated =
                        replace_style_pt_prop(&updated, "height", (h_val + d_bot - d_top).max(1.0));
                }
            }
            out.push_str(&updated);
        } else {
            out.push_str(tag);
        }
        cursor = end;
    }
    out.push_str(&inner_divs[cursor..]);

    let mut prior_dh = 0.0;
    for ins in insertions {
        let y = ins.cut_y + prior_dh;
        let w = rule6_band_width(ins.x0, ins.x1);
        let h = (ins.dh - 1.0).max(8.0);
        let lh_css = if ins.two_lines {
            format!("line-height:{PLAIN_TWO_LINE_CSS_HEIGHT};")
        } else {
            String::new()
        };
        let warn_attr = if ins.truncated {
            format!(
                " data-overflow-warning=\"text-truncated\" data-writer-value=\"{}\"",
                html_escape(&ins.full_text)
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "<div class=\"c f\" data-rule6-band=\"true\" data-rule6-of=\"{}\" style=\"left:{:.2}pt;top:{:.2}pt;width:{:.2}pt;height:{:.2}pt\"><input type=\"text\" class=\"{}\" name=\"{}\" readonly value=\"{}\" style=\"inset:1pt 3pt 1pt 3pt;font-size:{:.2}pt;{}text-align:left;background:#fff;z-index:2\" data-overflow-of=\"{}\"{}></div>",
            ins.cell_id,
            ins.x0 + 1.5,
            y + 0.5,
            w,
            h,
            ins.input_classes,
            ins.cell_id,
            html_escape(&ins.display),
            ins.font_size_pt,
            lh_css,
            ins.cell_id,
            warn_attr
        ));
        prior_dh += ins.dh;
    }
    out.push_str(&rest[last_close..]);
    out
}

fn replace_attr_value(tag: &str, name: &str, new_val: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(start) = tag.find(&needle) else {
        return tag.to_string();
    };
    let val_start = start + needle.len();
    let Some(end_rel) = tag[val_start..].find('"') else {
        return tag.to_string();
    };
    format!(
        "{}{}{}",
        &tag[..val_start],
        new_val,
        &tag[val_start + end_rel..]
    )
}

fn style_pt_prop(tag: &str, prop: &str) -> Option<f64> {
    let style = attr(tag, "style")?;
    for decl in style.split(';') {
        let trimmed = decl.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{prop}:")) {
            let num = rest.trim().strip_suffix("pt")?;
            return num.parse::<f64>().ok();
        }
    }
    None
}

fn replace_style_pt_prop(tag: &str, prop: &str, new_val: f64) -> String {
    let Some(style) = attr(tag, "style") else {
        return tag.to_string();
    };
    let prefix = format!("{prop}:");
    let new_style: Vec<String> = style
        .split(';')
        .map(|decl| {
            let trimmed = decl.trim();
            if trimmed.starts_with(&prefix) {
                format!("{prop}:{new_val:.2}pt")
            } else {
                decl.to_string()
            }
        })
        .collect();
    replace_attr_value(tag, "style", &new_style.join(";"))
}

/// One Rust-side join: `(writer key, cell, printed caption)`.
type SheetJoin = (&'static str, &'static str, &'static str);

/// Text joins a frozen sheet's writer-cells.json leaves out although the
/// sheet has the cell (html-frozen is synced from upstream and never edited
/// here), per sheet. 2550M (2007) item 4: the TIN's three 3-slot combs and
/// the 5-slot branch comb of the item 4 row (data-row 7: p1c127, p1c9,
/// p1c11, p1c13). The branch comb's inputs carry the sheet's 00000 as
/// values; the join writes the draft's branch over them.
///
/// For these keys this table supersedes 2550m-2007/writer-cells.json's
/// rule text ("The TIN boxes ... are not fillable cells on this sheet") and
/// its not_joined reason for the TIN row ("the 2007 sheet's TIN row has no
/// fillable cells in the frozen page ... reported on #113"): the frozen page
/// has those cells. writer_cells() and merged_writer_cells_json() merge it,
/// skipping any join writer-cells.json itself gains on a later re-sync.
const SHEET_JOINS: &[(&str, &[SheetJoin])] = &[(
    "2550m-2007",
    &[
        ("frm2550m:txtTIN1", "p1c127", "4 TIN (1-3)"),
        ("frm2550m:txtTIN2", "p1c9", "4 TIN (4-6)"),
        ("frm2550m:txtTIN3", "p1c11", "4 TIN (7-9)"),
        ("frm2550m:txtBranchCode", "p1c13", "4 TIN branch"),
    ],
)];

/// Text cells drawn as a row of one-character inputs that are not a comb
/// (no `data-slot-index`), per sheet, each with how a code shorter than its
/// boxes sits in them. A text join writes its whole value into every input
/// of its cell, so these are re-split one character per box. A value with
/// more characters than boxes prints once as a plain line across the cell
/// (`fill_text_input`). Every joined cell of this shape on the frozen
/// sheets is listed.
const SINGLE_CHARACTER_CELLS: &[(&str, &[(&str, bool)])] = &[
    (
        "1600wp-2010",
        // The attachment's "FOR THE PERIOD (MM/DD/YYYY)": from MM, DD and
        // YYYY, to MM, DD and YYYY, one-character boxes (maxlength 1).
        &[
            ("p1c62", false),
            ("p1c63", false),
            ("p1c64", false),
            ("p1c65", false),
            ("p1c66", false),
            ("p1c67", false),
        ],
    ),
    (
        "2550m-2007",
        // Item 3 sheets (a count, right-aligned), item 5 RDO, item 10 ZIP.
        &[("p1c3", true), ("p1c130", false), ("p1c21", false)],
    ),
];

/// The sheets of [`SINGLE_CHARACTER_CELLS`] whose split cells have been
/// rewritten since they were listed (PR 4) whether a value was written or
/// not. 1600WP's, listed in round 6, are rewritten only when filled, so a
/// document that leaves them empty prints them as before.
const SPLIT_REWRITES_UNFILLED: &[&str] = &["2550m-2007"];

/// One character per input of `slug`'s [`SINGLE_CHARACTER_CELLS`], in the
/// inputs' order. A value with more characters than boxes, or with
/// characters other than letters and digits, was printed by the fill as a
/// line across the cell, its boxes empty, and is left as it is.
fn split_single_character_cells(slug: &str, html: &str) -> String {
    let Some((_, cells)) = SINGLE_CHARACTER_CELLS.iter().find(|(s, _)| *s == slug) else {
        return html.to_string();
    };
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements = Vec::new();
    for (cell, right) in *cells {
        let Some(indices) = grouped.get(cell) else {
            continue;
        };
        if !splits(&tags, indices) {
            continue;
        }
        let first = tags[indices[0]].tag;
        let written = attr(first, "data-writer-value").or_else(|| attr(first, "value"));
        // An unfilled cell is left as the sheet draws it, except on the
        // sheets whose split cells every fill has rewritten, filled or not
        // (an empty box gains `value=""`; the goldens pin those bytes).
        if written.is_none() && !SPLIT_REWRITES_UNFILLED.contains(&slug) {
            continue;
        }
        let value = written.unwrap_or("");
        let chars: Vec<char> = value.chars().collect();
        let boxes = indices.len();
        if !fits_one_per_box(value, boxes) {
            continue;
        }
        let pad = if *right {
            boxes.saturating_sub(chars.len())
        } else {
            0
        };
        for (offset, &index) in indices.iter().enumerate() {
            let shown = offset
                .checked_sub(pad)
                .and_then(|at| chars.get(at))
                .map(char::to_string)
                .unwrap_or_default();
            // The writer's whole value stays on the first box only, as a
            // comb keeps it on its first slot.
            let tag = if offset == 0 {
                tags[index].tag.to_string()
            } else {
                without_writer_value(tags[index].tag)
            };
            replacements.push((
                tags[index].start,
                tags[index].end,
                set_value(&tag, &shown, None),
            ));
        }
    }
    apply_replacements(html, replacements)
}

/// `tag` without its `data-writer-value` attribute.
fn without_writer_value(tag: &str) -> String {
    let needle = " data-writer-value=\"";
    let Some(at) = tag.find(needle) else {
        return tag.to_string();
    };
    let rest = &tag[at + needle.len()..];
    match rest.find('"') {
        Some(close) => format!("{}{}", &tag[..at], &rest[close + 1..]),
        None => tag.to_string(),
    }
}

/// Writer keys in `fields` holding a value that `slug`'s sheet prints
/// nowhere: not a stamped `name=`, in no writer-cells join, and not listed
/// in `not_joined` with its reason. The print coverage gate: every value
/// the print map carries has a box, or a reviewed reason it has none.
pub fn unprinted_keys(
    slug: &str,
    fields: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let loaded = bundle(slug).ok_or_else(|| format!("no frozen HTML bundle for {slug}"))?;
    let cells = writer_cells(slug)?;
    let promoted = promote_empty_joined_cells(loaded.html, &cells);
    validate_writer_cells(&promoted, slug, &cells)?;
    let tags = input_tags(&promoted);
    let stamped: BTreeSet<&str> = tags.iter().map(|tag| tag.name).collect();
    let joined = cells.joined_keys();
    Ok(fields
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        // A PR 10 date part not chosen (`00`) holds nothing to print.
        .filter(|(key, value)| !donor_estate_unchosen(slug, key, value))
        .filter(|(key, value)| {
            !stamped.contains(key.as_str())
                && !joined.contains(key.as_str())
                && donor_estate_prints(slug, key, value) != Some(true)
                && !cells.not_joined.contains_key(key.as_str())
        })
        .map(|(key, _)| key)
        .cloned()
        .collect())
}

/// The characters a filled document prints in input `name` (a cell id or
/// a stamped key), slots in order: what a reader sees in that box.
pub fn printed_text(document: &str, name: &str) -> String {
    let tags = input_tags(document);
    let mut decimal_peso: Option<&str> = None;
    let mut decimal_cents: Option<&str> = None;
    for tag in &tags {
        if attr(tag.tag, "data-decimal-of") == Some(name) {
            match attr(tag.tag, "data-decimal-part") {
                Some("peso") => decimal_peso = attr(tag.tag, "value"),
                Some("cents") => decimal_cents = attr(tag.tag, "value"),
                _ => {}
            }
        }
    }
    if let (Some(peso), Some(cents)) = (decimal_peso, decimal_cents) {
        return format!("{peso}.{cents}");
    }
    let indices: Vec<usize> = tags
        .iter()
        .enumerate()
        .filter(|(_, tag)| tag.name == name)
        .map(|(index, _)| index)
        .collect();
    ordered_comb_indices(&tags, &indices)
        .into_iter()
        .map(|index| attr(tags[index].tag, "value").unwrap_or(""))
        .collect()
}

/// Frozen 2551Q HTML with writer values on stamped `name=` inputs,
/// writer-cell identity, money, and xbox joins.
pub fn fill_2551q(draft: &Form2551QDraft) -> String {
    fill_bundle(html_2551q(), &draft.to_print_field_map(), "2551q-2018")
        .expect("2551q-2018 writer-cells")
}

/// Self-contained document for a WebView `with_html` host (inline CSS, fonts, PNGs).
pub fn filled_document(slug: &str, fields: &BTreeMap<String, String>) -> Result<String, String> {
    filled_document_with_receipt(slug, fields, None)
}

/// BIR's receipt confirmation, printed as one more page after the form.
///
/// Before the HTML-only print pipeline the PDF preview appended this from
/// the receipt row linked by `draft.receipt_id`; the migration dropped it.
/// `body_html` must already be sanitised by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiptPage {
    pub filename: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub received_at: String,
    /// When the email arrived, already formatted for display; falls back to
    /// `received_at` (BIR's stamp) when the row predates the column.
    pub email_received_at: Option<String>,
    pub body_text: String,
    pub body_html: Option<String>,
}

/// `filled_document` plus continuation pages when schedule rows overflow,
/// plus the receipt page when the return is confirmed, along with the
/// structured overflow report for preview warnings.
pub fn filled_document_with_report(
    slug: &str,
    fields: &BTreeMap<String, String>,
    receipt: Option<&ReceiptPage>,
) -> Result<crate::continuation::FilledDocument, String> {
    let Some(loaded) = bundle(slug) else {
        return Err(format!("no frozen HTML bundle for {slug}"));
    };
    let overflow = crate::continuation::detect_overflow(slug, fields)?;
    let (width, height) = page_size_from_css(loaded.css);
    let cont = if !overflow.is_empty() {
        Some(crate::band_continuation::build_continuation_pages(
            slug,
            loaded.html,
            fields,
            &overflow,
            &width,
            &height,
        )?)
    } else {
        None
    };

    let official_filled = fill_bundle(loaded.html, fields, slug)?;
    let mut document = inline_local_assets(&official_filled, &loaded);
    let mut fallback_schedules = Vec::new();
    if let Some(c) = cont {
        if !c.pages_html.is_empty() {
            let inlined_pages = inline_local_assets(&c.pages_html, &loaded);
            document = append_before_body_end(&document, &inlined_pages);
        }
        fallback_schedules = c.fallback_schedules;
    }
    let truncated_entries =
        crate::continuation::collect_truncated_entries(slug, &document, &overflow);
    if !fallback_schedules.is_empty() || !truncated_entries.is_empty() {
        let fb_report = crate::continuation::OverflowReport {
            schedules: fallback_schedules,
        };
        let plain_pages = crate::continuation::continuation_pages_html_with_truncated(
            slug,
            fields,
            &fb_report,
            &truncated_entries,
            &width,
            &height,
        );
        if !plain_pages.is_empty() {
            document = append_before_body_end(&document, &plain_pages);
        }
    }
    if let Some(receipt) = receipt {
        document = append_before_body_end(&document, &receipt_page_html(receipt, &width, &height));
    }
    Ok(crate::continuation::FilledDocument {
        html: document,
        overflow,
    })
}

/// `filled_document` plus, when the return is confirmed, the receipt page.
pub fn filled_document_with_receipt(
    slug: &str,
    fields: &BTreeMap<String, String>,
    receipt: Option<&ReceiptPage>,
) -> Result<String, String> {
    Ok(filled_document_with_report(slug, fields, receipt)?.html)
}

/// `@page { size:612pt 936pt; … }` from the bundle's form.css, so the receipt
/// sheet matches the form's sheet; letter-long fallback if a bundle omits it.
fn page_size_from_css(css: &str) -> (String, String) {
    let fallback = ("612pt".to_string(), "936pt".to_string());
    let Some(start) = css.find("@page") else {
        return fallback;
    };
    let Some(size_at) = css[start..].find("size:") else {
        return fallback;
    };
    let rest = &css[start + size_at + "size:".len()..];
    let end = rest.find([';', '}']).unwrap_or(rest.len());
    let mut parts = rest[..end].split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(w), Some(h)) => (w.to_string(), h.to_string()),
        _ => fallback,
    }
}

fn append_before_body_end(document: &str, fragment: &str) -> String {
    match document.rfind("</body>") {
        Some(at) => format!("{}{}{}", &document[..at], fragment, &document[at..]),
        None => format!("{document}{fragment}"),
    }
}

/// A `div` like the form pages on purpose: `.page:last-of-type` in base.css
/// is per element type, so a `<section>` here would leave page 2 counted as
/// "last" — no page break before the receipt when printing, and no gap on
/// screen.
/// The receipt sheet, laid out like a mail client's print of the message:
/// the mailbox top right, the subject as the title, sender and arrival time,
/// the recipient, then BIR's email as sent. No labels of our own — the file
/// name and BIR's stamp are in the email. `.page` in base.css already breaks
/// after each page and not after the last; `height:auto;overflow:visible`
/// let a long email flow onto a further sheet.
fn receipt_page_html(receipt: &ReceiptPage, width: &str, height: &str) -> String {
    let arrived = receipt
        .email_received_at
        .as_deref()
        .unwrap_or(receipt.received_at.as_str());
    let mut html = String::new();
    html.push_str(&format!(
        "<div class=\"page page-receipt\" id=\"page-receipt\" style=\"width:{width};min-height:{height};height:auto;overflow:visible;padding:28pt 36pt;font-family:'eBIRForms Arimo',Arial,Helvetica,sans-serif;font-size:10.5pt;line-height:1.35;color:#000;background:#fff\">"
    ));
    html.push_str(
        "<style>\
#page-receipt .receipt-html p{margin:0 0 1em 0}\
#page-receipt .receipt-html ul,#page-receipt .receipt-html ol{margin:0.5em 0 1em 0;padding-left:2.4em}\
#page-receipt .receipt-html li{margin:0.15em 0}\
#page-receipt .receipt-html b,#page-receipt .receipt-html strong{font-weight:700}\
#page-receipt .receipt-html table{border-collapse:collapse}\
#page-receipt .receipt-html img{max-width:100%}\
</style>",
    );
    html.push_str(&format!(
        "<div style=\"text-align:right;color:#666;font-weight:700;font-size:10pt;margin:0 0 10pt 0\">{}</div>",
        html_escape(&receipt.to)
    ));
    html.push_str("<hr style=\"border:0;border-top:1px solid #999;margin:0 0 10pt 0\">");
    html.push_str(&format!(
        "<h1 style=\"font-size:17pt;font-weight:700;margin:0 0 10pt 0\">{}</h1>",
        html_escape(&receipt.subject)
    ));
    html.push_str("<hr style=\"border:0;border-top:1px solid #999;margin:0 0 8pt 0\">");
    html.push_str(&format!(
        "<div style=\"display:flex;justify-content:space-between;gap:12pt\"><div><b>{from}</b> &lt;{from}&gt;</div><div style=\"white-space:nowrap\">{when}</div></div>",
        from = html_escape(&receipt.from),
        when = html_escape(arrived)
    ));
    html.push_str(&format!(
        "<div style=\"margin:0 0 14pt 0\">To: {}</div>",
        html_escape(&receipt.to)
    ));
    // The email as BIR sent it when its HTML survived ingest; the plain text
    // only as a fallback. Both said the same thing, and printing both read
    // as a duplicate.
    match &receipt.body_html {
        Some(body_html) if !body_html.trim().is_empty() => {
            html.push_str("<div class=\"receipt-html\" style=\"padding:0 12pt\">");
            html.push_str(body_html);
            html.push_str("</div>");
        }
        _ => html.push_str(&format!(
            "<pre class=\"receipt-text\" style=\"white-space:pre-wrap;word-break:break-word;margin:0;padding:0 12pt;font-family:'eBIRForms Arimo',Arial,Helvetica,sans-serif;font-size:10.5pt;line-height:1.35\">{}</pre>",
            html_escape(&receipt.body_text)
        )),
    }
    html.push_str("</div>");
    html
}

pub fn filled_2551q_document(draft: &Form2551QDraft) -> String {
    filled_document("2551q-2018", &draft.to_print_field_map()).expect("2551q-2018 freeze bundle")
}

pub fn stamped_tin_names() -> &'static [&'static str] {
    &STAMPED_TIN_NAMES
}

fn input_tags(html: &str) -> Vec<InputTag<'_>> {
    let mut tags = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = html[search_from..].find("<input") {
        let start = search_from + rel;
        let Some(gt) = html[start..].find('>') else {
            break;
        };
        let end = start + gt + 1;
        let tag = &html[start..end];
        if let Some(name) = attr(tag, "name") {
            tags.push(InputTag {
                start,
                end,
                tag,
                name,
                slot: attr(tag, "data-slot-index").and_then(|value| value.parse().ok()),
            });
        }
        search_from = end;
    }
    tags
}

fn attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=\"");
    let mut from = 0;
    loop {
        let at = tag.get(from..)?.find(&needle)? + from;
        // The name must be exact: a byte before it that is not whitespace or
        // a tag edge means a longer name ending in `key` (data-cell-id vs id).
        let before = at
            .checked_sub(1)
            .and_then(|i| tag.as_bytes().get(i))
            .copied();
        let exact = matches!(before, None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'/'));
        if exact {
            let start = at + needle.len();
            let rest = tag.get(start..)?;
            let end = rest.find('"')?;
            return rest.get(..end);
        }
        from = at + needle.len();
    }
}

fn set_value(tag: &str, value: &str, writer_value: Option<&str>) -> String {
    let escaped = html_escape(value);
    let needle = "value=\"";
    let mut out = if let Some(value_at) = tag.find(needle) {
        let content_at = value_at + needle.len();
        if let Some(close) = tag[content_at..].find('"') {
            let close_at = content_at + close;
            let mut rewritten = String::with_capacity(tag.len() + escaped.len());
            rewritten.push_str(&tag[..content_at]);
            rewritten.push_str(&escaped);
            rewritten.push_str(&tag[close_at..]);
            rewritten
        } else {
            tag.to_string()
        }
    } else {
        let insert_at = tag.rfind('>').unwrap_or(tag.len());
        format!(
            "{} value=\"{}\"{}",
            &tag[..insert_at],
            escaped,
            &tag[insert_at..]
        )
    };
    if let Some(writer) = writer_value.filter(|value| !value.is_empty()) {
        let attr = format!(" data-writer-value=\"{}\"", html_escape(writer));
        if !out.contains("data-writer-value=\"") {
            let insert_at = out.rfind('>').unwrap_or(out.len());
            out.insert_str(insert_at, &attr);
        }
    }
    out
}

fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Replace one `<link rel="stylesheet" href="{href}">` tag with an inline
/// `<style>`, whatever the attribute order or self-closing: sheets are
/// maintained by hand after freezing, and a correction round-trip once
/// alphabetised one sheet's attributes (`<link href=... rel="stylesheet"/>`),
/// which the exact-byte match before this helper silently left external.
fn inline_stylesheet_link(document: &str, href: &str, css: &str) -> Option<String> {
    let mut scan = 0;
    while let Some(at) = document[scan..].find("<link").map(|i| scan + i) {
        let tag_end = document[at..].find('>').map(|i| at + i + 1)?;
        let tag = &document[at..tag_end];
        let is_stylesheet = tag.contains("rel=\"stylesheet\"") || tag.contains("rel='stylesheet'");
        let has_href =
            tag.contains(&format!("href=\"{href}\"")) || tag.contains(&format!("href='{href}'"));
        if is_stylesheet && has_href {
            return Some(format!(
                "{}<style>{css}</style>{}",
                &document[..at],
                &document[tag_end..]
            ));
        }
        scan = tag_end;
    }
    None
}

fn inline_local_assets(html: &str, loaded: &FrozenBundle) -> String {
    let mut document = html.to_string();
    let base = BASE_CSS.replace(
        "url(\"fonts/arimo-latin-wght-normal.woff2\")",
        &format!("url(\"{}\")", data_uri("font/woff2", FONT_ARIMO_NORMAL)),
    );
    let form = loaded
        .css
        .replace(
            "url(\"../fonts/arimo-latin-wght-italic.woff2\")",
            &format!("url(\"{}\")", data_uri("font/woff2", FONT_ARIMO_ITALIC)),
        )
        .replace(
            "url(\"../fonts/tinos-latin-400-normal.woff2\")",
            &format!("url(\"{}\")", data_uri("font/woff2", FONT_TINOS_NORMAL)),
        )
        .replace(
            "url(\"../fonts/tinos-latin-700-normal.woff2\")",
            &format!("url(\"{}\")", data_uri("font/woff2", FONT_TINOS_BOLD)),
        );
    for (href, css) in [("../base.css", &base), ("form.css", &form)] {
        if let Some(inlined) = inline_stylesheet_link(&document, href, css) {
            document = inlined;
        }
    }
    document = strip_relative_font_preloads(&document);
    for (name, bytes) in loaded.assets {
        let uri = data_uri("image/png", bytes);
        document = document.replace(&format!("../assets/{name}"), &uri);
    }
    document
}

/// Remove `<link ... href="../fonts/..." ...>` preload tags because every font
/// face is already inlined as a `data:font/woff2;base64,...` URI in `<style>`.
fn strip_relative_font_preloads(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("<link ") {
        let Some(end_rel) = rest[pos..].find('>') else {
            break;
        };
        let tag = &rest[pos..=pos + end_rel];
        if tag.contains("../fonts/") {
            out.push_str(&rest[..pos]);
            rest = &rest[pos + end_rel + 1..];
        } else {
            out.push_str(&rest[..=pos + end_rel]);
            rest = &rest[pos + end_rel + 1..];
        }
    }
    out.push_str(rest);
    out
}

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// True when a writer key represents a numeric amount printed into a comb
/// (derived whole-peso/money combs on 1700, 1701A, 1701MS, or money-typed keys).
fn is_amount_comb_key(key: &str) -> bool {
    is_amount_typed_key(key)
        || key.starts_with("derived:amount_")
        || key.starts_with("derived:tp_p")
        || key.starts_with("derived:sp_p")
        || key.starts_with("derived:p2_")
        || key.starts_with("derived:p3_")
        || key.starts_with("derived:Sched")
}

/// Normalize a branch code's digits into 5 characters; returns `None` when
/// empty or all zeros (so preprinted `00000` stays untouched).
fn non_zero_branch_five(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.bytes().all(|b| b == b'0') {
        return None;
    }
    let five = if digits.len() >= 5 {
        digits[digits.len() - 5..].to_string()
    } else {
        format!("{digits:0>5}")
    };
    Some(five)
}

/// Fill a 14-slot TIN comb that has 9 `<input>` slots and 5 empty preprinted
/// `00000` `<div class="s" data-slot="9..13"></div>` slots.
fn fill_nine_plus_five_tin_comb(
    html: &str,
    tags: &[InputTag<'_>],
    ordered: &[usize],
    cell_id: &str,
    print: &str,
    via_cell: bool,
    replacements: &mut Vec<(usize, usize, String)>,
) -> bool {
    let marker = format!("id=\"{cell_id}\"");
    let Some(cell_start) = html.find(&marker) else {
        return false;
    };
    let Some(open_end) = html[cell_start..].find('>').map(|n| cell_start + n + 1) else {
        return false;
    };
    let cell_head = &html[cell_start..open_end];
    if !cell_head.contains("data-comb-slots=\"14\"") {
        return false;
    }
    let cell_end = html[open_end..]
        .find("<div id=\"p")
        .map_or(html.len(), |n| open_end + n);
    let cell_body = &html[open_end..cell_end];
    let mut empty_slots = Vec::with_capacity(5);
    for slot in 9..14 {
        let needle = format!("data-slot=\"{slot}\"");
        let Some(rel) = cell_body.find(&needle) else {
            return false;
        };
        let slot_abs = open_end + rel;
        let Some(gt) = html[slot_abs..cell_end].find('>').map(|n| slot_abs + n + 1) else {
            return false;
        };
        if !html[gt..cell_end].starts_with("</div>") {
            return false;
        }
        empty_slots.push((slot, gt));
    }
    let chars: Vec<char> = print.chars().collect();
    for (offset, &index) in ordered.iter().enumerate() {
        let ch = chars
            .get(offset)
            .copied()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let writer = (via_cell && offset == 0).then_some(print);
        replacements.push((
            tags[index].start,
            tags[index].end,
            set_value(tags[index].tag, &ch, writer),
        ));
    }
    if let Some(branch) = non_zero_branch_five(&print[9..]) {
        let base_class = attr(tags[ordered[0]].tag, "class").unwrap_or("fi fh20 fc");
        for ((slot, insert_at), digit) in empty_slots.into_iter().zip(branch.chars()) {
            let input = format!(
                "<input type=\"text\" class=\"{base_class}\" id=\"{cell_id}-s{slot}\" name=\"{cell_id}\" data-slot-index=\"{slot}\" maxlength=\"1\" style=\"background:#fff\" value=\"{digit}\">"
            );
            replacements.push((insert_at, insert_at, input));
        }
    }
    true
}

/// Fill 5-slot empty preprinted `00000` branch-code combs (such as 1701 Part IV
/// Item 1 spouse branch `p2c15`, 1700 `p1c67`, 1701A `p2c124`, 1701Q `p1c72`,
/// and primary TIN branch combs) whenever the draft carries a non-zero
/// `*BranchCode` value and the frozen sheet has no `<input>` in that comb.
fn fill_preprinted_branch_combs(
    html: &str,
    fields: &BTreeMap<String, String>,
    joins: &BTreeMap<String, Vec<String>>,
) -> String {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (key, value) in fields {
        if !key.ends_with("BranchCode") {
            continue;
        }
        let Some(branch) = non_zero_branch_five(value) else {
            continue;
        };
        if joins.contains_key(key) {
            continue;
        }
        let key_attr = format!("name=\"{key}\"");
        if html.contains(&key_attr) {
            continue;
        }
        let tin3_key = format!("{}TIN3", &key[..key.len() - "BranchCode".len()]);
        let tin3_cell_pos = if let Some(cell) = joins.get(&tin3_key).and_then(|v| v.first()) {
            html.find(&format!("id=\"{cell}\""))
        } else {
            html.find(&format!("name=\"{tin3_key}\""))
                .and_then(|pos| html[..pos].rfind("<div id=\"p"))
        };
        let Some(tin3_pos) = tin3_cell_pos else {
            continue;
        };
        let Some(tin3_open_end) = html[tin3_pos..].find('>').map(|n| tin3_pos + n + 1) else {
            continue;
        };
        let tin3_row = attr(&html[tin3_pos..tin3_open_end], "data-row");
        let tin3_next = html[tin3_open_end..]
            .find("<div id=\"p")
            .map_or(html.len(), |n| tin3_open_end + n);
        let base_class = html[tin3_open_end..tin3_next]
            .find("<input ")
            .and_then(|rel| {
                let s = tin3_open_end + rel;
                let e = html[s..tin3_next].find('>').map(|n| s + n + 1)?;
                attr(&html[s..e], "class")
            })
            .unwrap_or("fi fh20 fc");
        // Scan the next few sibling cells on the same row for an empty 5-slot comb.
        let mut cursor = tin3_next;
        for _ in 0..4 {
            let Some(rel) = html[cursor..].find("<div id=\"p") else {
                break;
            };
            let cell_start = cursor + rel;
            let Some(open_end) = html[cell_start..].find('>').map(|n| cell_start + n + 1) else {
                break;
            };
            let next_cell = html[open_end..]
                .find("<div id=\"p")
                .map_or(html.len(), |n| open_end + n);
            let head = &html[cell_start..open_end];
            if attr(head, "data-row") != tin3_row {
                break;
            }
            if head.contains("data-comb-slots=\"5\"")
                && !html[open_end..next_cell].contains("<input ")
            {
                let cell_id = attr(head, "id").unwrap_or("branch");
                let mut slots = Vec::with_capacity(5);
                let mut ok = true;
                for slot in 0..5 {
                    let needle = format!("data-slot=\"{slot}\"");
                    let Some(srel) = html[open_end..next_cell].find(&needle) else {
                        ok = false;
                        break;
                    };
                    let sabs = open_end + srel;
                    let Some(gt) = html[sabs..next_cell].find('>').map(|n| sabs + n + 1) else {
                        ok = false;
                        break;
                    };
                    if !html[gt..next_cell].starts_with("</div>") {
                        ok = false;
                        break;
                    }
                    slots.push((slot, gt));
                }
                if ok {
                    for ((slot, insert_at), digit) in slots.into_iter().zip(branch.chars()) {
                        let input = format!(
                            "<input type=\"text\" class=\"{base_class}\" id=\"{cell_id}-s{slot}\" name=\"{key}\" data-slot-index=\"{slot}\" maxlength=\"1\" style=\"background:#fff\" value=\"{digit}\">"
                        );
                        replacements.push((insert_at, insert_at, input));
                    }
                    break;
                }
            }
            cursor = next_cell;
        }
    }
    if replacements.is_empty() {
        return html.to_string();
    }
    apply_replacements(html, replacements)
}

/// True when `text` is a formatted numeric amount (optional leading `-` or
/// surrounding `(...)`, digits, commas, and at most one decimal point).
fn is_amount_comb_value(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let body = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .or_else(|| trimmed.strip_prefix('-'))
        .unwrap_or(trimmed);
    let mut digits = 0usize;
    let mut dots = 0usize;
    for ch in body.chars() {
        match ch {
            '0'..='9' => digits += 1,
            ',' => {}
            '.' => dots += 1,
            _ => return false,
        }
    }
    digits > 0 && dots <= 1
}

/// Rule 2: one right-aligned plain-rectangle amount spanning `peso_cell`
/// (and `cent_cell` if separate), with `data-rect-box` marking the union
/// rectangle for SVG/text separator removal, `data-overflow-of` (white
/// knockout band) and `data-overflow-warning="amount-comb"` for the
/// preview's warnings.
///
/// The type is measured to set the whole amount within the rectangle
/// ([`SheetType::fit`]), with no floor: an amount is shrunk below the 7.5 pt
/// floor long text keeps rather than clipped past the rectangle or cut with
/// "..." (a figure cut short is a different figure, and BIR reads the
/// amount where its line is, not on an additional sheet).
fn comb_overflow_amount_union(
    html: &str,
    peso_cell: &str,
    cent_cell: Option<&str>,
    text: &str,
    sheet: &SheetType,
) -> Option<(usize, usize, String)> {
    let (pl, pt, pw, ph) = cell_box_pt(html, peso_cell)?;
    let (l, t, r, b) = if let Some(cent) = cent_cell.and_then(|c| cell_box_pt(html, c)) {
        (
            pl.min(cent.0),
            pt.min(cent.1),
            (pl + pw).max(cent.0 + cent.2),
            (pt + ph).max(cent.1 + cent.3),
        )
    } else {
        (pl, pt, pl + pw, pt + ph)
    };
    let union_w = r - l;
    // The rectangle's input sits 1 pt in from each side of the cell and
    // keeps 1 pt of right padding: the amount sets in what is left, with
    // 1 pt to spare.
    let size = sheet.fit(sheet.letter, (union_w - 4.0).max(1.0), text, PLAIN_MAX_PT);
    let (open, tag) = cell_tag(html, peso_cell)?;
    let close = open + tag.len();
    let width_css = if cent_cell.is_some() {
        format!(
            "inset:0.75pt auto 0.75pt 1pt;width:{:.2}pt;",
            (union_w - 2.0).max(10.0)
        )
    } else {
        String::new()
    };
    let warning = if size < SMALL_WHOLE_BELOW_PT {
        format!("amount-small\" data-writer-value=\"{}", html_escape(text))
    } else {
        "amount-comb".to_string()
    };
    let input = format!(
        "<input type=\"text\" class=\"fi\" readonly value=\"{}\" style=\"{width_css}font-size:{size:.2}pt;text-align:right;justify-content:flex-end;padding-right:1pt;z-index:1\" data-overflow-warning=\"{warning}\" data-overflow-of=\"{peso_cell}\" data-rect-box=\"{l:.2},{t:.2},{r:.2},{b:.2}\">",
        html_escape(text)
    );
    if cent_cell.is_some() {
        let open_tag = with_style_declaration(&html[open..=close], "overflow:visible");
        Some((open, close + 1, format!("{open_tag}{input}")))
    } else {
        Some((close + 1, close + 1, input))
    }
}

/// One cell whose printed value overflowed its official box (e.g. an amount
/// wider than its comb slots), exposed for continuation-sheet / UI warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct OverflowWarning {
    pub cell: String,
    pub kind: String,
    pub value: String,
    pub font_size_pt: Option<f64>,
}

/// Lists every cell in `document` carrying a `data-overflow-warning` marker
/// (works identically on official cells and cloned continuation cells).
pub fn overflow_warnings(document: &str) -> Vec<OverflowWarning> {
    let needle = " data-overflow-warning=\"";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = document[from..].find(needle) {
        let at = from + at;
        let start = document[..at].rfind('<').unwrap_or(0);
        let end = at + document[at..].find('>').unwrap_or(0);
        let tag = &document[start..=end];
        from = end + 1;
        let Some(kind) = attr(tag, "data-overflow-warning") else {
            continue;
        };
        let cell = attr(tag, "data-overflow-of")
            .or_else(|| attr(tag, "name"))
            .or_else(|| attr(tag, "id").map(|id| id.strip_suffix("-i").unwrap_or(id)))
            .unwrap_or("")
            .to_string();
        let value = attr(tag, "data-writer-value")
            .or_else(|| attr(tag, "value"))
            .unwrap_or("")
            .replace("&#10;", "\n")
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        let font_size_pt = tag
            .find("font-size:")
            .and_then(|pos| tag[pos + "font-size:".len()..].split("pt").next())
            .and_then(|s| s.parse::<f64>().ok());
        out.push(OverflowWarning {
            cell,
            kind: kind.to_string(),
            value,
            font_size_pt,
        });
    }
    out
}

// ---- Uniform amount sections (issue #477) -------------------------------
//
// When one amount in a section's column overflows its comb, every amount of
// that column prints as a plain right-aligned line, so the column stays
// uniform. A group is a printed section's column: from the section's
// Part/Schedule band heading to the next band on that page, at one column x
// (within 2 pt) — never a pitch ratio. Spouse columns sit at a different x,
// so they group independently.

/// One comb cell's geometry, parsed from the frozen sheet.
#[derive(Clone, Debug)]
struct CombGeom {
    id: String,
    page: String,
    x: f64,
    y: f64,
    width: f64,
}

/// The sheet page a cell id belongs to: `p4c66` is on page `p4`. Ids of
/// another shape key on their leading letters, so cells of different pages
/// never share a column.
fn cell_page_key(id: &str) -> String {
    let rest = id.strip_prefix('p').unwrap_or(id);
    let digits: usize = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0 && id.starts_with('p') {
        return id[..1 + digits].to_string();
    }
    id.chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect()
}

/// Every comb cell of `html` with at least five boxes (amount-sized; cent
/// combs and short identifiers stay out).
fn comb_geometries(html: &str) -> Vec<CombGeom> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = html[from..].find("<div") {
        let open = from + rel;
        let Some(end) = html[open..].find('>').map(|e| open + e) else {
            break;
        };
        let tag = &html[open..end];
        from = end + 1;
        // A band row inside a `<template>` is the sheet's cloning master,
        // never a printed row: where a cell id appears in both, the printed
        // row's coordinates are the truth (1600PT Item 13's tax bases).
        if inside_template(html, open) {
            continue;
        }
        let Some(cap) = attr(tag, "data-comb-capacity").and_then(|v| v.parse::<u32>().ok()) else {
            continue;
        };
        if cap < 5 {
            continue;
        }
        let Some(id) = attr(tag, "id") else {
            continue;
        };
        let Some(x) = parse_style_pt(tag, "left") else {
            continue;
        };
        let Some(y) = parse_style_pt(tag, "top") else {
            continue;
        };
        out.push(CombGeom {
            id: id.to_string(),
            page: cell_page_key(id),
            x,
            y,
            width: parse_style_pt(tag, "width").unwrap_or(f64::from(cap) * 5.5),
        });
    }
    out
}

/// A printed section heading of the sheet: the page and top y of a
/// `Part`/`Schedule` band's text line, from the sheet's own text divs.
fn section_headings(html: &str) -> Vec<(String, f64, String)> {
    let mut out = Vec::new();
    let needle = "<div class=\"t\"";
    let mut from = 0;
    while let Some(rel) = html[from..].find(needle) {
        let open = from + rel;
        let Some(end) = html[open..].find('>').map(|e| open + e) else {
            break;
        };
        let tag = &html[open..end];
        let close = html[end..].find("</div>").map(|c| end + c);
        from = end + 1;
        // A band inside a `<template>` is the cloning master, never printed.
        if inside_template(html, open) {
            continue;
        }
        let (Some(id), Some(y), Some(close)) = (attr(tag, "id"), parse_style_pt(tag, "top"), close)
        else {
            continue;
        };
        let inner = &html[end + 1..close];
        // Strip any inline markup and entities enough to read the band text.
        let mut text = String::new();
        let mut rest = inner;
        while let Some(lt) = rest.find('<') {
            text.push_str(&rest[..lt]);
            match rest[lt..].find('>') {
                Some(gt) => rest = &rest[lt + gt + 1..],
                None => break,
            }
        }
        text.push_str(rest);
        let text = text.trim();
        if is_section_band_heading(text) {
            out.push((cell_page_key(id), y, text.to_string()));
        }
    }
    // A heading split across adjacent divs ("P" + "art II"): a div whose
    // whole text is a strict prefix of `part`/`schedule` joins its nearest
    // right neighbour on the same line, and the joined text is tested again.
    let mut fragments: Vec<(String, f64, f64, String)> = Vec::new();
    let mut from2 = 0;
    while let Some(rel) = html[from2..].find(needle) {
        let open = from2 + rel;
        let Some(end) = html[open..].find('>').map(|e| open + e) else {
            break;
        };
        let tag = &html[open..end];
        let close = html[end..].find("</div>").map(|c| end + c);
        from2 = end + 1;
        if inside_template(html, open) {
            continue;
        }
        let (Some(id), Some(y), Some(x), Some(close)) = (
            attr(tag, "id"),
            parse_style_pt(tag, "top"),
            parse_style_pt(tag, "left"),
            close,
        ) else {
            continue;
        };
        let inner = &html[end + 1..close];
        let mut text = String::new();
        let mut rest = inner;
        while let Some(lt) = rest.find('<') {
            text.push_str(&rest[..lt]);
            match rest[lt..].find('>') {
                Some(gt) => rest = &rest[lt + gt + 1..],
                None => break,
            }
        }
        text.push_str(rest);
        fragments.push((cell_page_key(id), y, x, text.trim().to_string()));
    }
    for (page, y, x, text) in &fragments {
        let lower = text.to_ascii_lowercase();
        let partial = !lower.is_empty()
            && ("part".starts_with(lower.as_str()) || "schedule".starts_with(lower.as_str()))
            && lower.len() < 8;
        if !partial {
            continue;
        }
        // The nearest fragment to the right on the same line.
        let mut best: Option<(&String, f64)> = None;
        for (page2, y2, x2, text2) in &fragments {
            if page2 == page
                && (y2 - y).abs() <= 2.0
                && *x2 > *x
                && best.is_none_or(|(_, bx)| *x2 < bx)
            {
                best = Some((text2, *x2));
            }
        }
        if let Some((tail, _)) = best {
            let joined = format!("{text}{tail}");
            if is_section_band_heading(&joined) && !is_section_band_heading(text) {
                out.push((page.clone(), *y, joined));
            }
        }
    }
    out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup();
    out
}

/// Whether a text line is a section band heading: `Part`/`Schedule` (any
/// case), a word boundary, then a numeral — arabic digits, or a roman
/// numeral run (`i`/`v`/`x`/`l`/`c`/`d`/`m` letters) ended by a non-letter
/// or the line's end. Never `Particulars`, never a body line mentioning a
/// part.
fn is_section_band_heading(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let after = if let Some(a) = lower.strip_prefix("part") {
        a
    } else if let Some(a) = lower.strip_prefix("schedule") {
        a
    } else {
        return false;
    };
    // A word boundary must separate the word from what follows: the next
    // character is not a letter (`Part IV`, `Part-IV`, never `Particulars`).
    if after
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    let rest = after.trim_start_matches(|c: char| c.is_whitespace() || c == '-' || c == '\u{2013}');
    let mut chars = rest.chars().peekable();
    let numeral = match chars.peek() {
        Some(c) if c.is_ascii_digit() => true,
        Some('i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm') => {
            while matches!(chars.peek(), Some(c) if matches!(c, 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm'))
            {
                chars.next();
            }
            // The roman run must end at a non-letter or the line's end:
            // `Part IV` yes, a letter tail no.
            !chars.peek().is_some_and(|c| c.is_ascii_alphabetic())
        }
        _ => false,
    };
    if !numeral {
        return false;
    }
    // A band heading names its section, never another: a line that goes on
    // to mention an item or a second part/schedule is a body cross-reference
    // ("Schedule 2 Item 4A and Part VII Item 5A" is a note inside Schedule 1
    // of 1701, not a band).
    let tail = &lower[lower.len() - rest.len()..];
    // Only a mention that itself carries a numeral is a cross-reference:
    // "Schedule 3 – Schedule of Cost and Other Allowable Expenses" is the
    // band of Schedule 3 (its title repeats the bare word), while "(from
    // Part IV-Schedule 1, Item 4)" points at another section's rows.
    let body_mention = ["item", "part", "schedule"].iter().any(|word| {
        tail.match_indices(word).any(|(at, _)| {
            let before_ok = tail[..at]
                .chars()
                .last()
                .is_none_or(|c| !c.is_ascii_alphabetic());
            let after = &tail[at + word.len()..];
            let after_boundary = after
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphabetic());
            let trimmed = after.trim_start_matches(|c: char| {
                c.is_whitespace() || matches!(c, '-' | '\u{2013}' | '(' | ')')
            });
            let carries_numeral = if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                true
            } else {
                // A standalone roman numeral: the roman run ends at a
                // non-letter, so "Item IV" counts but "Schedule of Items"
                // or "Installment" never do.
                let run = trimmed.len()
                    - trimmed
                        .trim_start_matches(['i', 'v', 'x', 'l', 'c', 'd', 'm'])
                        .len();
                run > 0
                    && !trimmed[run..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic())
            };
            before_ok && after_boundary && carries_numeral
        })
    });
    !body_mention
}

/// The section a row belongs to: the nearest Part/Schedule band above it on
/// its page (`None` above the first band: the header rows or a part continued
/// from the page before).
fn section_of(headings: &[(String, f64, String)], page: &str, y: f64) -> Option<f64> {
    headings
        .iter()
        .filter(|(hp, hy, _)| hp == page && *hy <= y + 1.0)
        .map(|(_, hy, _)| *hy)
        .reduce(f64::max)
}

/// A text line of the sheet with its geometry: page, top y, and the x
/// interval it occupies (width estimated from its length and font size).
fn text_lines(html: &str) -> Vec<(String, f64, f64, f64, bool)> {
    let mut out = Vec::new();
    let needle = "<div class=\"t\"";
    let mut from = 0;
    while let Some(rel) = html[from..].find(needle) {
        let open = from + rel;
        let Some(end) = html[open..].find('>').map(|e| open + e) else {
            break;
        };
        let tag = &html[open..end];
        let close = html[end..].find("</div>").map(|c| end + c);
        from = end + 1;
        let (Some(id), Some(y), Some(x), Some(close)) = (
            attr(tag, "id"),
            parse_style_pt(tag, "top"),
            parse_style_pt(tag, "left"),
            close,
        ) else {
            continue;
        };
        if inside_template(html, open) {
            continue;
        }
        let inner = &html[end + 1..close];
        let mut len = inner.len() as f64;
        let mut rest = inner;
        let mut inner_text_has_letter = false;
        while let Some(lt) = rest.find('<') {
            if rest[..lt].bytes().any(|b| b.is_ascii_alphabetic()) {
                inner_text_has_letter = true;
            }
            match rest[lt..].find('>') {
                Some(gt) => {
                    len -= (gt + 1) as f64;
                    rest = &rest[lt + gt + 1..];
                }
                None => break,
            }
        }
        if rest.bytes().any(|b| b.is_ascii_alphabetic()) {
            inner_text_has_letter = true;
        }
        let size = tag
            .split("font-size:")
            .nth(1)
            .and_then(|s| s.split("pt").next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(8.0);
        // Only lettered lines split a column: the pre-printed zeros and
        // percent signs of rate boxes sit inside the comb's width, and no
        // column header is a bare digit or punctuation mark (1702EX page 2
        // split at the rate row's printed `0`s otherwise).
        if len > 0.0 && !inner_text_has_letter {
            continue;
        }
        // Arimo sets about half its point size per character.
        let width = len * size * 0.5;
        // A lettered sub-heading that names a person or a schedule of its
        // own — "5.A – Taxpayer/Filer", "5.B – Spouse" — splits a column even
        // though it starts far left of the amounts: what sits above it is
        // never the same column of figures as what sits below.
        let stripped: String = {
            let mut t = String::new();
            let mut r = inner;
            while let Some(lt) = r.find('<') {
                t.push_str(&r[..lt]);
                match r[lt..].find('>') {
                    Some(gt) => r = &r[lt + gt + 1..],
                    None => break,
                }
            }
            t.push_str(r);
            t.trim().to_string()
        };
        let bytes = stripped.as_bytes();
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        let sub = digits > 0
            && bytes.len() > digits + 1
            && bytes[digits] == b'.'
            && bytes[digits + 1].is_ascii_uppercase()
            && (stripped.contains("Taxpayer")
                || stripped.contains("Spouse")
                || stripped.contains("Filer"));
        out.push((cell_page_key(id), y, x, x + width, sub));
    }
    out
}

/// The amount column groups of a frozen sheet by slug, for tests and audit.
pub fn uniform_amount_groups_of(slug: &str) -> Vec<Vec<String>> {
    let Some(loaded) = bundle(slug) else {
        return Vec::new();
    };
    amount_column_groups(loaded.html)
}

/// Group amount-sized comb cells into a printed section's columns. A group is
/// one section's column: from the section's Part/Schedule band to the next
/// band on that page, at one column x (within 2 pt). Never a pitch ratio:
/// the bands are the sheet's own text lines. A column also splits where a
/// text line STARTS inside the comb's width between two rows — a sub-table's
/// column headers (1702RT Schedule III's stacked NOLCO tables share one x but
/// are separate columns of figures). The start is what counts, so the tail of
/// a row label that reaches into the column still splits it when the label
/// itself begins inside the comb's span; a label beginning far left of the
/// column never does, however far its tail reaches.
fn amount_column_groups(html: &str) -> Vec<Vec<String>> {
    let geoms = comb_geometries(html);
    let headings = section_headings(html);
    let lines = text_lines(html);
    let mut columns: Vec<(String, Option<f64>, Vec<&CombGeom>)> = Vec::new();
    'cells: for geom in &geoms {
        let section = section_of(&headings, &geom.page, geom.y);
        for (cpage, csection, column) in &mut columns {
            if *cpage == geom.page && *csection == section {
                let first = column[0];
                if (first.x - geom.x).abs() <= 2.0 {
                    column.push(geom);
                    continue 'cells;
                }
            }
        }
        columns.push((geom.page.clone(), section, vec![geom]));
    }
    let mut groups = Vec::new();
    for (page, _, mut column) in columns {
        column.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
        // A text line overlapping the column's x between two rows breaks the
        // group. The window is the cell's real printed width (170-182 pt on
        // the wide combs): a header centred over the column starts inside
        // it, while a row label's tail ends left of the column and never
        // splits it.
        let col_x = column[0].x;
        let mut current: Vec<String> = Vec::new();
        let mut prev_y: Option<f64> = None;
        for geom in column {
            if let Some(py) = prev_y {
                // A column header sits OVER its column: it starts inside
                // the comb's own horizontal span. A row label or a marginal
                // note comes from the left margin and never splits anything.
                let comb_right = col_x + geom.width;
                let crossed = lines.iter().any(|(lp, ly, lx0, _lx1, sub)| {
                    *lp == page
                        && *ly > py + 1.0
                        && *ly < geom.y - 1.0
                        && (*sub || (*lx0 >= col_x - 2.0 && *lx0 <= comb_right))
                });
                if crossed && !current.is_empty() {
                    groups.push(std::mem::take(&mut current));
                }
            }
            if !current.contains(&geom.id) {
                current.push(geom.id.clone());
            }
            prev_y = Some(geom.y);
        }
        if !current.is_empty() {
            groups.push(current);
        }
    }
    groups
}

/// Cells converted to a line by the ordinary per-cell rules: the union rects
/// carry `data-overflow-of` naming the converted comb.
fn converted_amount_cells(html: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let needle = "data-overflow-of=\"";
    let mut from = 0;
    while let Some(rel) = html[from..].find(needle) {
        let attr_at = from + rel;
        let start = attr_at + needle.len();
        let Some(end) = html[start..].find('\"').map(|e| start + e) else {
            break;
        };
        // Anchor on the one `<input` element this attribute belongs to: the
        // nearest `<input` before it, with no `>` in between (anything else
        // means the attribute is read past its element's end, e.g. into an
        // added line's span).
        let tag_start = html[..attr_at].rfind("<input");
        let owned = tag_start.is_some_and(|ts| !html[ts..attr_at].contains('>'));
        if owned {
            let ts = tag_start.unwrap();
            let tag_end = html[attr_at..]
                .find('>')
                .map(|e| attr_at + e)
                .unwrap_or(html.len());
            let tag = &html[ts..tag_end.min(html.len())];
            // Only amount conversions widen to the section, not identifier
            // or date overflow lines.
            if tag.contains("amount-comb") {
                // Only money forces the column: a TIN-shaped, a reference or
                // a bare digit string that outgrows its own boxes prints as
                // a line itself, but it never converts the column around it.
                let money = attr(tag, "data-writer-value")
                    .or_else(|| attr(tag, "value"))
                    .is_some_and(conversion_money_amount);
                if money {
                    out.insert(html[start..end].to_string());
                }
            } else if tag.contains("amount-small") {
                // A shrunk line widens its column only when it is money:
                // `amount-small` also marks long non-money figures, which
                // must never force a column of amount boxes into lines.
                let money = attr(tag, "data-writer-value")
                    .or_else(|| attr(tag, "value"))
                    .is_some_and(conversion_money_amount);
                if money {
                    out.insert(html[start..end].to_string());
                }
            }
        }
        from = end + 1;
    }
    out
}

/// Widen the per-cell conversions to whole (section, column) groups: the
/// group mates of every converted cell, minus the cells already converted.
fn uniform_amount_forced_cells(
    html: &str,
    converted: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut forced = std::collections::BTreeSet::new();
    if converted.is_empty() {
        return forced;
    }
    // `converted` is already money-gated by converted_amount_cells.
    for group in amount_column_groups(html) {
        if group.iter().any(|id| converted.contains(id)) {
            for id in group {
                if !converted.contains(&id) {
                    forced.insert(id);
                }
            }
        }
    }
    forced
}

/// The peso-comb → centavo-comb pairing of a sheet's money joins, so a
/// blanked row can blank both combs.
fn cent_pair_map(joins: &[MoneyJoin]) -> std::collections::HashMap<&str, &str> {
    let mut out = std::collections::HashMap::new();
    for join in joins {
        for cells in &join.cells {
            if let Some(cent) = &cells.cent_html_id {
                out.insert(cells.peso_html_id.as_str(), cent.as_str());
            }
        }
    }
    out
}

/// Reconcile the overflow marks of a converted column after the font sizes
/// are unified:
///
/// - a line whose size ended below [`SMALL_WHOLE_BELOW_PT`] — the real
///   overflow's or a forced group mate's — is marked `amount-small` with its
///   value, so the additional sheet carries it in full (a line that small
///   cannot be read on the page);
/// - a forced conversion that fits at a readable size carries no warning at
///   all: nothing went wrong for it, so the preview reports only the real
///   overflow, never ten "fitted lines" for one.
fn reconcile_amount_warning_marks(
    html: &str,
    forced: &std::collections::BTreeSet<String>,
) -> String {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let needle = "data-overflow-warning=\"amount-comb\"";
    let mut from = 0;
    while let Some(rel) = html[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        let Some(ts) = html[..at].rfind("<input") else {
            continue;
        };
        if html[ts..at].contains('>') {
            continue;
        }
        let Some(end) = html[at..].find('>').map(|e| at + e) else {
            continue;
        };
        let tag = &html[ts..end];
        let Some(cell) = attr(tag, "data-overflow-of") else {
            continue;
        };
        let size = tag
            .split("font-size:")
            .nth(1)
            .and_then(|s| s.split("pt").next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(PLAIN_MAX_PT);
        if size < SMALL_WHOLE_BELOW_PT {
            // Small enough that the additional sheet must carry it: upgrade
            // the mark and put the value where the sheet reader looks.
            let value = attr(tag, "value").unwrap_or("");
            let mut out = tag.to_string();
            out = out.replacen(
                "data-overflow-warning=\"amount-comb\"",
                &format!(
                    "data-overflow-warning=\"amount-small\" data-writer-value=\"{}\"",
                    html_escape(value)
                ),
                1,
            );
            replacements.push((ts, end, out));
        } else if forced.contains(cell) {
            // A forced conversion at a readable size: no warning.
            let out = tag.replacen(" data-overflow-warning=\"amount-comb\"", "", 1);
            replacements.push((ts, end, out));
        }
    }
    if replacements.is_empty() {
        return html.to_string();
    }
    replacements.sort_by_key(|(at, _, _)| *at);
    apply_replacements(html, replacements)
}

/// Blank the comb boxes of a forced group's cells that hold no amount: in a
/// converted column the boxes go even where the row is empty, so the column
/// never shows boxes beside lines. Cells already converted or holding a
/// written value are untouched.
fn blank_empty_forced_cells(
    html: &str,
    forced: &std::collections::BTreeSet<String>,
    sheet: &SheetType,
    cent_pairs: &std::collections::HashMap<&str, &str>,
) -> String {
    if forced.is_empty() {
        return html.to_string();
    }
    let tags = input_tags(html);
    let grouped = grouped_input_indices(&tags);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for cell in forced {
        if html.contains(&format!("data-overflow-of=\"{cell}\"")) {
            continue;
        }
        let Some(indices) = grouped.get(cell.as_str()) else {
            continue;
        };
        let mut ordered = ordered_comb_indices(&tags, indices);
        // The paired centavo comb (two slots) never joins a group on its
        // own, but in a blanked row its boxes and the printed point go with
        // the peso boxes: the union rectangle covers both, so the column
        // never shows boxes or a stray point beside a blank line.
        let cent_cell = cent_pairs.get(cell.as_str()).copied();
        if let Some(cent) = cent_cell {
            if let Some(cent_indices) = grouped.get(cent) {
                ordered.extend(ordered_comb_indices(&tags, cent_indices));
            }
        }
        let written = ordered
            .iter()
            .any(|&index| attr(tags[index].tag, "value").is_some_and(|v| !v.is_empty()));
        if written {
            continue;
        }
        amount_rect(
            html,
            &tags,
            &ordered,
            (cell, cent_cell),
            "",
            None,
            sheet,
            &mut replacements,
        );
    }
    if replacements.is_empty() {
        return html.to_string();
    }
    replacements.sort_by_key(|(at, _, _)| *at);
    apply_replacements(html, replacements)
}

/// One font size per converted group: the smallest size any amount of the
/// group needs applies to every line of the group, so the column reads
/// uniform.
fn unify_group_font_sizes(html: &str) -> String {
    // Nothing converted: no rect inputs, nothing to unify — never scan 1.8 MB
    // per candidate cell.
    if !html.contains("data-overflow-of=\"") {
        return html.to_string();
    }
    // One scan: every amount conversion's rect input, keyed by its cell.
    struct Rect {
        at: usize,
        end: usize,
        size: f64,
    }
    let mut rects_by_cell: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let needle = "data-overflow-of=\"";
    let mut from = 0;
    while let Some(rel) = html[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        let Some(id_end) = html[at + needle.len()..]
            .find('"')
            .map(|e| at + needle.len() + e)
        else {
            break;
        };
        let cell = &html[at + needle.len()..id_end];
        let Some(ts) = html[..at].rfind("<input") else {
            continue;
        };
        if html[ts..at].contains('>') {
            continue;
        }
        let end = html[at..]
            .find('>')
            .map(|e| at + e + 1)
            .unwrap_or(html.len());
        let tag = &html[ts..end.min(html.len())];
        // Only amount conversions read as one column: identifier and
        // text overflow lines keep their own sizes.
        if !tag.contains("amount-comb") && !tag.contains("amount-small") {
            continue;
        }
        let size = tag
            .split("font-size:")
            .nth(1)
            .and_then(|s| s.split("pt").next())
            .and_then(|s| s.trim().parse::<f64>().ok());
        if let Some(size) = size {
            rects_by_cell
                .entry(cell.to_string())
                .or_insert(Rect { at: ts, end, size });
        }
    }
    if rects_by_cell.is_empty() {
        return html.to_string();
    }
    let groups = amount_column_groups(html);
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for group in &groups {
        let members: Vec<&Rect> = group
            .iter()
            .filter_map(|cell| rects_by_cell.get(cell))
            .collect();
        if members.len() < 2 {
            continue;
        }
        let min = members
            .iter()
            .map(|r| r.size)
            .reduce(f64::min)
            .unwrap_or(0.0);
        for rect in members {
            if (rect.size - min).abs() < 0.001 {
                continue;
            }
            let tag = &html[rect.at..rect.end];
            if let Some(pos) = tag.find("font-size:") {
                let num_start = pos + "font-size:".len();
                let Some(pt) = tag[num_start..].find("pt") else {
                    continue;
                };
                let rewritten = format!(
                    "{}font-size:{}pt{}",
                    &tag[..pos],
                    min,
                    &tag[num_start + pt + 2..]
                );
                replacements.push((rect.at, rect.end, rewritten));
            }
        }
    }
    if replacements.is_empty() {
        return html.to_string();
    }
    apply_replacements(html, replacements)
}

/// holding `text` left-aligned in a font small enough to fit. Returns the
/// insert position just inside the cell and the input tag.
///
/// Only the drawn cell counts: the `<div>` whose own `id` attribute is
/// `cell` (not `data-cell-id` or another name ending in `id`), outside any
/// `<template>` (the band rows a sheet keeps for cloning are never drawn, so
/// a line put there would print nowhere).
/// One read-only input laid over a whole comb cell (`<div id="{cell}">`),
fn cell_tag<'a>(html: &'a str, cell: &str) -> Option<(usize, &'a str)> {
    let needle = format!("id=\"{cell}\"");
    let mut from = 0;
    while let Some(rel) = html[from..].find(&needle) {
        let id = from + rel;
        from = id + needle.len();
        // The attribute name must be `id` itself.
        if !html[..id].ends_with([' ', '\t', '\n', '\r']) {
            continue;
        }
        let Some(open) = html[..id].rfind("<div") else {
            continue;
        };
        if html[open..id].contains('>') {
            continue;
        }
        if inside_template(html, open) {
            continue;
        }
        let end = open + html[open..].find('>')?;
        return Some((open, &html[open..end]));
    }
    None
}

/// Whether byte `at` of `html` lies between a `<template` and its
/// `</template>`.
fn inside_template(html: &str, at: usize) -> bool {
    match html[..at].rfind("<template") {
        Some(open) => !html[open..at].contains("</template>"),
        None => false,
    }
}

fn comb_overflow_line(
    html: &str,
    cell: &str,
    text: &str,
    sheet: &SheetType,
) -> Option<(usize, String)> {
    let (_, _, width, _) = cell_box_pt(html, cell)?;
    overflow_input_with_fit(html, cell, text, fit_line(sheet, width, text), sheet)
}

/// [`comb_overflow_line`] for an identifier ([`is_identifier_cell`]) or a
/// figure: the whole text on one line, its type measured to set within the
/// line's real room (the cell less the input's 1 pt insets and 1 pt left
/// padding, with 1 pt to spare) with no 7.5 pt floor, never cut with "...".
/// Below [`SMALL_WHOLE_BELOW_PT`] it is also marked for the additional
/// sheet ([`small_whole_mark`]).
fn comb_overflow_line_whole(
    html: &str,
    cell: &str,
    text: &str,
    sheet: &SheetType,
    identifier: bool,
) -> Option<(usize, String)> {
    let (_, _, width, _) = cell_box_pt(html, cell)?;
    let letter = sheet.letter_for("fi");
    let size = sheet.fit(letter, (width - 4.0).max(1.0), text, PLAIN_MAX_PT);
    let (l, t, _, height) = cell_box_pt(html, cell)?;
    let (r, b) = (l + width, t + height);
    let (open, tag) = cell_tag(html, cell)?;
    let close = open + tag.len();
    Some((
        close + 1,
        format!(
            "<input type=\"text\" class=\"fi\" readonly value=\"{}\" style=\"font-size:{size:.2}pt;text-align:left;padding-left:1pt;z-index:1\"{} data-overflow-of=\"{cell}\" data-rect-box=\"{l:.2},{t:.2},{r:.2},{b:.2}\">",
            html_escape(text),
            small_whole_mark(size, identifier, text),
        ),
    ))
}

/// The mark a value printed whole but shrunk below
/// [`SMALL_WHOLE_BELOW_PT`] carries, so the additional sheet prints it whole
/// as well and the preview warns ([`crate::continuation::collect_truncated_entries`]):
/// `identifier-small` for a TIN or an ATC, `amount-small` for an amount or
/// another figure. Two kinds, so the preview can say which. Empty at or
/// above the size.
fn small_whole_mark(size: f64, identifier: bool, text: &str) -> String {
    if size >= SMALL_WHOLE_BELOW_PT {
        return String::new();
    }
    let kind = if identifier {
        "identifier-small"
    } else {
        "amount-small"
    };
    format!(
        " data-overflow-warning=\"{kind}\" data-writer-value=\"{}\"",
        html_escape(text)
    )
}

/// Rule 1 + Rule 3: plain-rectangle text for an overflowing comb cell,
/// wrapped onto 2 lines if tall enough, floored at [`PLAIN_TEXT_MIN_PT`]
/// (7.5 pt), truncated with `"..."` + `text-truncated` warning if still too
/// long, and carrying `data-rect-box` for SVG separator removal.
fn overflow_input_with_fit(
    html: &str,
    cell: &str,
    text: &str,
    proposed_size: f64,
    sheet: &SheetType,
) -> Option<(usize, String)> {
    let (l, t, width, height) = cell_box_pt(html, cell)?;
    let r = l + width;
    let b = t + height;
    let (open, tag) = cell_tag(html, cell)?;
    let close = open + tag.len();
    if text.is_empty() {
        return Some((
            close + 1,
            format!(
                "<input type=\"text\" class=\"fi\" readonly value=\"\" style=\"font-size:{:.2}pt;text-align:left;padding-left:1pt;z-index:1\" data-overflow-of=\"{cell}\" data-rect-box=\"{l:.2},{t:.2},{r:.2},{b:.2}\">",
                proposed_size.max(PLAIN_TEXT_MIN_PT)
            ),
        ));
    }
    let (display, size, two_lines, truncated, tight_inset) = if proposed_size < PLAIN_TEXT_MIN_PT {
        let letter = sheet.letter_for("fi");
        // `[data-overflow-of]` has `inset: 0.75pt 1pt 0.75pt 1pt` plus
        // `padding-left: 1pt`, leaving `width - 3pt` horizontal room and
        // `height - 1.5pt` vertical room; keep 1pt extra horizontal spare.
        // Rows just under `17.05pt` (such as 1702Q Sched 4 item 6a at
        // `17.04pt`) still fit two 7.5pt lines (`15.60pt`) cleanly when
        // vertical inset tightens to `0.40pt` (`height - 0.80pt`).
        let room = (width - 4.0).max(4.0);
        let default_inner_height = (height - 1.5).max(0.0);
        let (inner_height, use_tight_inset) = if default_inner_height >= PLAIN_WRAP_MIN_HEIGHT_PT {
            (default_inner_height, false)
        } else {
            let tight_inner = (height - 0.80).max(0.0);
            if tight_inner >= PLAIN_TWO_LINE_HEIGHT_FACTOR * PLAIN_TEXT_MIN_PT {
                (tight_inner, true)
            } else {
                (default_inner_height, false)
            }
        };
        if inner_height >= PLAIN_WRAP_MIN_HEIGHT_PT {
            let max_two_line =
                floor_size((inner_height / PLAIN_TWO_LINE_HEIGHT_FACTOR).min(PLAIN_MAX_PT));
            if max_two_line >= PLAIN_TEXT_MIN_PT {
                if let Some((l1, l2, sz)) =
                    fit_two_lines(sheet, letter, room, text, max_two_line, PLAIN_TEXT_MIN_PT)
                {
                    (format!("{l1}\n{l2}"), sz, true, false, use_tight_inset)
                } else if let Some((l1, l2)) =
                    wrap_and_truncate_two_lines(sheet, letter, room, text, PLAIN_TEXT_MIN_PT)
                {
                    (
                        format!("{l1}\n{l2}"),
                        PLAIN_TEXT_MIN_PT,
                        true,
                        true,
                        use_tight_inset,
                    )
                } else {
                    let cut = truncate_with_ellipsis(sheet, letter, room, text, PLAIN_TEXT_MIN_PT)
                        .unwrap_or_else(|| "...".to_string());
                    (cut, PLAIN_TEXT_MIN_PT, false, true, false)
                }
            } else {
                let cut = truncate_with_ellipsis(sheet, letter, room, text, PLAIN_TEXT_MIN_PT)
                    .unwrap_or_else(|| "...".to_string());
                (cut, PLAIN_TEXT_MIN_PT, false, true, false)
            }
        } else {
            let cut = truncate_with_ellipsis(sheet, letter, room, text, PLAIN_TEXT_MIN_PT)
                .unwrap_or_else(|| "...".to_string());
            (cut, PLAIN_TEXT_MIN_PT, false, true, false)
        }
    } else {
        (text.to_string(), proposed_size, false, false, false)
    };
    let inset_css = if tight_inset { "inset:0.40pt 1pt;" } else { "" };
    let lh_css = if two_lines {
        format!("line-height:{PLAIN_TWO_LINE_CSS_HEIGHT};")
    } else {
        String::new()
    };
    let warn_attr = if truncated {
        format!(
            " data-overflow-warning=\"text-truncated\" data-writer-value=\"{}\"",
            html_escape(text)
        )
    } else {
        String::new()
    };
    Some((
        close + 1,
        format!(
            "<input type=\"text\" class=\"fi\" readonly value=\"{}\" style=\"{inset_css}font-size:{size:.2}pt;{lh_css}text-align:left;padding-left:1pt;z-index:1\"{warn_attr} data-overflow-of=\"{cell}\" data-rect-box=\"{l:.2},{t:.2},{r:.2},{b:.2}\">",
            html_escape(&display)
        ),
    ))
}

/// Every official writer key mapped to a printed text or money box on `slug`,
/// paired with whether its printed caption describes a non-numeric text box.
/// Used by RULES 7 so official zero-default filling only targets boxes that
/// actually print on the sheet (never internal or unprinted keys).
pub fn printed_writer_keys(slug: &str) -> BTreeMap<String, bool> {
    let mut keys = BTreeMap::new();
    let Ok(wc) = writer_cells(slug) else {
        return keys;
    };
    for (key, cids) in &wc.joins {
        let caption = cids
            .iter()
            .filter_map(|cid| wc.captions.get(cid))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        let is_text_only = (caption.contains("legal basis")
            || caption.contains("registered activity")
            || caption.contains("description")
            || caption.contains("specify"))
            && !caption.contains("amount")
            && !caption.contains("tax")
            && !caption.contains("rate")
            && !caption.contains("number")
            && !caption.contains("no.");
        keys.insert(key.clone(), is_text_only);
    }
    for join in &wc.money_joins {
        keys.insert(join.writer_key.clone(), false);
    }
    keys
}

/// [`printed_cell`] for every comb cell a filled document has, read in one
/// pass: cell id -> what prints over it and the plain line's type size.
pub fn printed_cells(document: &str) -> BTreeMap<String, (String, Option<f64>)> {
    let tags = input_tags(document);
    let grouped = grouped_input_indices(&tags);
    let mut out: BTreeMap<String, (String, Option<f64>)> = BTreeMap::new();
    for (name, indices) in &grouped {
        if !indices.iter().any(|&index| tags[index].slot.is_some()) {
            continue;
        }
        let boxes: String = ordered_comb_indices(&tags, indices)
            .into_iter()
            .map(|index| attr(tags[index].tag, "value").unwrap_or(""))
            .collect();
        out.insert((*name).to_string(), (boxes, None));
    }
    let needle = " data-overflow-of=\"";
    let mut from = 0;
    while let Some(at) = document[from..].find(needle) {
        let at = from + at;
        let start = document[..at].rfind("<input").unwrap_or(0);
        let end = at + document[at..].find('>').unwrap_or(0);
        let tag = &document[start..=end];
        from = end + 1;
        let Some(cell) = attr(tag, "data-overflow-of") else {
            continue;
        };
        let entry = out.entry(cell.to_string()).or_default();
        if !entry.0.trim().is_empty() {
            continue;
        }
        let text = attr(tag, "value")
            .unwrap_or("")
            .replace("&#10;", "\n")
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        let size = tag
            .find("font-size:")
            .and_then(|at| tag[at + "font-size:".len()..].split("pt").next())
            .and_then(|size| size.parse().ok());
        *entry = (text, size);
    }
    out
}

/// The cells of each item `slug`'s sheet prints over stacked comb lines (a
/// two-line address, a three-line registered name), each item's lines in
/// order: what an overflowing item is laid across.
pub fn stacked_lines(slug: &str) -> Result<Vec<Vec<String>>, String> {
    let loaded = bundle(slug).ok_or_else(|| format!("no frozen HTML bundle for {slug}"))?;
    let cells = writer_cells(slug)?;
    Ok(line_groups(loaded.html, &cells)
        .into_iter()
        .map(|group| group.into_iter().map(|line| line.cell).collect())
        .collect())
}

/// What a filled document prints over comb cell `cell`: its boxes'
/// characters in slot order, or, where every box is empty, the plain line
/// laid over the cell (`data-overflow-of`) with its type size.
pub fn printed_cell(document: &str, cell: &str) -> (String, Option<f64>) {
    let boxes = printed_text(document, cell);
    if !boxes.trim().is_empty() {
        return (boxes, None);
    }
    let needle = format!(" data-overflow-of=\"{cell}\"");
    let Some(at) = document.find(&needle) else {
        return (boxes, None);
    };
    let start = document[..at].rfind("<input").unwrap_or(0);
    let end = at + document[at..].find('>').unwrap_or(needle.len());
    let tag = &document[start..=end];
    let text = attr(tag, "value")
        .unwrap_or("")
        .replace("&#10;", "\n")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let size = tag
        .find("font-size:")
        .and_then(|at| tag[at + "font-size:".len()..].split("pt").next())
        .and_then(|size| size.parse().ok());
    (text, size)
}

/// Email inputs (`txtEmail`, `txtI11Email`, `txtPg1Pt1I12Email`, ...) are
/// never uppercased on print; matched case-insensitively on "email".
fn is_email_key(name: &str) -> bool {
    name.to_ascii_lowercase().contains("email")
}

#[cfg(test)]
mod tests {
    /// Writes the (section, column) amount groups of every sheet as a
    /// Markdown table for review: `cargo test -p bir-print uniform_amount_group_table -- --ignored --nocapture`.
    #[test]
    #[ignore = "writes the group table report"]
    fn uniform_amount_group_table() {
        let path =
            std::env::var("GROUP_TABLE_OUT").unwrap_or_else(|_| "../../group-table.md".to_string());
        let out = uniform_amount_group_table_markdown();
        std::fs::write(&path, &out).expect("write group table");
        eprintln!("wrote {path} ({} bytes)", out.len());
    }

    /// The group table must be regenerated whenever a sheet's grouping moves:
    /// this test fails until docs/proof/print-uniform-amount-sections/GROUPS.md
    /// is updated with `cargo test -p bir-print uniform_amount_group_table --
    /// --ignored` (GROUP_TABLE_OUT=<the file>).
    #[test]
    fn uniform_amount_group_table_matches_the_committed_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/proof/print-uniform-amount-sections/GROUPS.md"
        );
        let committed = std::fs::read_to_string(path)
            .expect("docs/proof/print-uniform-amount-sections/GROUPS.md must exist")
            .replace("\r\n", "\n");
        assert_eq!(
            uniform_amount_group_table_markdown(),
            committed,
            "GROUPS.md is stale: regenerate it (GROUP_TABLE_OUT) and commit"
        );
    }

    fn uniform_amount_group_table_markdown() -> String {
        let slugs = [
            "0605-1999",
            "0619e-2018",
            "0619f-2018",
            "1600-pt-2018",
            "1600-vt-2018",
            "1600wp-2010",
            "1601-fq-2020",
            "1601c-2018",
            "1601eq-2019",
            "1602q-2019",
            "1603q-2018",
            "1604c-2018",
            "1604e-2018",
            "1604f-2018",
            "1606-2018",
            "1700-2018",
            "1701-2018",
            "1701a-2018",
            "1701ms-2024",
            "1701q-2018",
            "1702ex-2018",
            "1702mx-2018c",
            "1702q-2018",
            "1702rt-2018c",
            "1706-2018",
            "1707-2021",
            "1707a-2021",
            "1800-2018",
            "1801-2018",
            "2000-dst-2018",
            "2000-ot-2018",
            "2200a-2020",
            "2200an-2018",
            "2200c-2018",
            "2200m-2018",
            "2200p-2020",
            "2200s-2018",
            "2200t-2020",
            "2550m-2007",
            "2550q-2024",
            "2551q-2018",
            "2552-2018",
            "2553-1999",
        ];
        let mut out = String::new();
        out.push_str(
            "<!--
             Notes on the grouping rule: a group is one printed section's column,
             from the section's Part/Schedule band heading to the next heading on
             that page, at one column x (within 2 pt). A column also splits where
             a sub-table's column headers START inside the comb's width between
             two rows, and at a lettered sub-heading that names a different person
             or schedule of its own (such as 1701 Schedule 5's \"5.A –
             Taxpayer/Filer\" and \"5.B – Spouse\"): what sits above it is never
             the same column of figures as what sits below. Hidden template rows
             never decide geometry.
-->

",
        );
        for slug in slugs {
            let Some(loaded) = bundle(slug) else {
                out.push_str(&format!(
                    "## {slug}

no bundle

"
                ));
                continue;
            };
            let cells = writer_cells(slug).expect("writer cells");
            let mut joined: BTreeMap<&str, &str> = BTreeMap::new();
            for join in &cells.peso_joins {
                for id in &join.html_ids {
                    joined.insert(id.as_str(), join.writer_key.as_str());
                }
            }
            for join in &cells.money_joins {
                for c in &join.cells {
                    joined.insert(c.peso_html_id.as_str(), join.writer_key.as_str());
                }
            }
            // Ordinary text joins write amounts too whenever their cell is a
            // comb (1702q, 1701a, 1700 fill every amount this way): without
            // them the audit's key column read blank for whole sheets.
            for (key, ids) in &cells.joins {
                for id in ids {
                    joined.entry(id.as_str()).or_insert(key.as_str());
                }
            }
            let groups = amount_column_groups(loaded.html);
            let n_amount = groups
                .iter()
                .filter(|g| g.iter().any(|c| joined.contains_key(c.as_str())))
                .count();
            out.push_str(&format!(
                "## {slug}: {} groups, {} with amount joins

",
                groups.len(),
                n_amount
            ));
            let headings = section_headings(loaded.html);
            out.push_str(
                "| rows | cells | section | amount writer keys |
|---|---|---|---|
",
            );
            for group in &groups {
                let keys: Vec<&str> = group
                    .iter()
                    .filter_map(|c| joined.get(c.as_str()).copied())
                    .collect();
                let first = keys.first().copied().unwrap_or("");
                let first_cell = group.first().unwrap();
                let section = cell_tag(loaded.html, first_cell)
                    .and_then(|(_, tag)| parse_style_pt(tag, "top"))
                    .and_then(|y| {
                        let band_y = section_of(&headings, &cell_page_key(first_cell), y)?;
                        headings
                            .iter()
                            .find(|(hp, hy, _)| hp == &cell_page_key(first_cell) && *hy == band_y)
                            .map(|(_, _, text)| text.as_str())
                    })
                    .unwrap_or("(header)");
                out.push_str(&format!(
                    "| {} | {}..{} | {} | {} |
",
                    group.len(),
                    group.first().unwrap(),
                    group.last().unwrap(),
                    section,
                    first
                ));
            }
            out.push('\n');
        }
        out
    }

    /// P5/P2: the reconcile pass upgrades a sub-floor line to amount-small
    /// with its value, and strips the warning of a forced conversion that
    /// reads fine.
    #[test]
    fn reconcile_marks_tiny_lines_small_and_strips_forced_warnings() {
        let html = concat!(
            "<input type=\"text\" readonly value=\"1,234,567,890\" style=\"font-size:4.20pt\" data-overflow-warning=\"amount-comb\" data-overflow-of=\"p1c10\" data-rect-box=\"1,2,3,4\">",
            "<input type=\"text\" readonly value=\"1,000\" style=\"font-size:9.00pt\" data-overflow-warning=\"amount-comb\" data-overflow-of=\"p1c11\" data-rect-box=\"1,2,3,4\">",
        );
        let mut forced = std::collections::BTreeSet::new();
        forced.insert("p1c11".to_string());
        let out = super::reconcile_amount_warning_marks(html, &forced);
        // The tiny line becomes amount-small and carries its value.
        assert!(out.contains(
            "data-overflow-warning=\"amount-small\" data-writer-value=\"1,234,567,890\""
        ));
        assert!(out.contains("data-overflow-of=\"p1c10\""));
        // The forced conversion at a readable size warns not at all.
        let pos = out.find("data-overflow-of=\"p1c11\"").unwrap();
        let tag_start = out[..pos].rfind("<input").unwrap();
        assert!(!out[tag_start..pos].contains("data-overflow-warning"));
    }

    /// P3: the band-heading rule — numerals yes, cross-references and
    /// particulars no.
    #[test]
    fn conversion_money_amount_table() {
        for money in [
            "0",
            "0.00",
            "1,234",
            "(1,234.00)",
            "(500)",
            "-500",
            "12",
            "999",
            "1234.50",
        ] {
            assert!(conversion_money_amount(money), "{money} is money");
        }
        for not_money in [
            "1500",
            "250000",
            "1234.5",
            "123-456-789",
            "2024",
            "WC010",
            "1200",
            "09171234567",
        ] {
            assert!(
                !conversion_money_amount(not_money),
                "{not_money} is not money"
            );
        }
        // the shared strict function is unchanged: centavos only, no brackets
        assert!(is_formatted_money_amount("1234.50"));
        assert!(!is_formatted_money_amount("(500)"));
        assert!(!is_formatted_money_amount("0"));
    }

    fn section_band_heading_rule() {
        for yes in [
            "Part IV",
            "PART II",
            "Schedule 1",
            "SCHEDULE 4",
            "Part IV – Computation",
            "Schedule 1 – Gross Compensation Income and Tax Withheld",
            "Part-IV",
            "part ii",
            // 1707: the band's title repeats the bare word — still a band.
            "Schedule 3 – Schedule of Cost and Other Allowable Expenses",
            "Schedule 2 – Installment Sale",
        ] {
            assert!(
                super::is_section_band_heading(yes),
                "{yes} must be a heading"
            );
        }
        for no in [
            "Particulars",
            "PARTICULARS",
            "Part-iculars",
            "Schedule 2 Item 4A and Part VII Item 5A",
            "Partial",
            "Part",
            "Schedule",
            "See Part IV",
            "Particularly",
        ] {
            assert!(
                !super::is_section_band_heading(no),
                "{no} must not be a heading"
            );
        }
    }

    #[test]
    fn attachment_counts_fill_both_digits_for_one_through_ninety_nine() {
        let html = r#"<input name="frm1601c:txtSheets" data-slot-index="0" value=""><input name="frm1601c:txtSheets" data-slot-index="1" value="">"#;
        for n in 0..=99 {
            let fields = BTreeMap::from([("frm1601c:txtSheets".to_string(), n.to_string())]);
            let filled = super::fill_by_name(html, &fields);
            assert_eq!(comb_text(&filled, "frm1601c:txtSheets"), format!("{n:02}"));
        }
        let fields = BTreeMap::from([("frm1601c:txtSheets".to_string(), String::new())]);
        assert_eq!(
            comb_text(&super::fill_by_name(html, &fields), "frm1601c:txtSheets"),
            ""
        );
    }

    #[test]
    fn every_frozen_attachment_count_join_pads_to_its_existing_capacity() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
        let mut checked = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let dir = entry.unwrap().path();
            let path = dir.join("writer-cells.json");
            if !path.exists() {
                continue;
            }
            let doc: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let slug = dir.file_name().unwrap().to_str().unwrap();
            for join in doc["joins"].as_array().unwrap() {
                let caption = join["printed"].as_str().unwrap_or("").to_ascii_lowercase();
                if !(caption.contains("number of sheet")
                    || caption.contains("no. of sheet")
                    || caption.contains("number of attachment"))
                {
                    continue;
                }
                let key = join["writer_key"].as_str().unwrap();
                let cell = join["html_id"].as_str().unwrap();
                assert!(
                    super::is_attachment_count_key(key),
                    "{slug}: missing count key {key}"
                );
                let empty = super::filled_document(slug, &BTreeMap::new()).unwrap();
                let capacity = super::input_tags(&empty)
                    .iter()
                    .filter(|t| t.name == cell && t.slot.is_some())
                    .count();
                if capacity < 2 {
                    continue;
                } // Separate tens/units or plain fields.
                for value in ["", "0", "1", "9", "10", "99"] {
                    let fields = BTreeMap::from([(key.to_string(), value.to_string())]);
                    let filled = super::filled_document(slug, &fields).unwrap();
                    let expected = if value.is_empty() {
                        String::new()
                    } else {
                        format!("{value:0>capacity$}")
                    };
                    assert_eq!(
                        comb_text(&filled, cell),
                        expected,
                        "{slug}: {key} = {value}"
                    );
                }
                checked += 1;
            }
        }
        assert!(
            checked >= 30,
            "attachment coverage unexpectedly fell to {checked}"
        );
    }

    /// A comb drawn with a decimal-point column: slots 0..=4 for pesos, an
    /// index gap where the point is printed, then two centavo slots.
    fn point_column_comb() -> String {
        [0, 1, 2, 3, 4, 6, 7]
            .iter()
            .map(|i| format!("<input name=\"amt\" data-slot-index=\"{i}\" value=\"\">"))
            .collect()
    }

    fn money_comb(value: &str, style: super::NegativeStyle) -> super::MoneyComb {
        let html = point_column_comb();
        let tags = super::input_tags(&html);
        let ordered: Vec<usize> = (0..tags.len()).collect();
        super::money_comb_values(&tags, &ordered, value, style)
    }

    fn money_writes(value: &str) -> Option<Vec<String>> {
        match money_comb(value, super::NegativeStyle::Minus) {
            super::MoneyComb::Boxes(writes) => Some(writes.into_iter().map(|(_, ch)| ch).collect()),
            _ => None,
        }
    }

    #[test]
    fn amounts_right_align_pesos_and_put_cents_after_the_point_gap() {
        // Before the fix `0.00` spread from the first box as `0 . 0 0`.
        assert_eq!(
            money_writes("0.00").unwrap(),
            ["", "", "", "", "0", "0", "0"]
        );
        assert_eq!(
            money_writes("1,234.56").unwrap(),
            ["", "1", "2", "3", "4", "5", "6"]
        );
        assert_eq!(
            money_writes("12").unwrap(),
            ["", "", "", "1", "2", "0", "0"]
        );
    }

    #[test]
    fn non_amounts_and_plain_combs_keep_the_left_to_right_spread() {
        assert!(money_writes("ABC").is_none());
        assert!(money_writes("").is_none());
        // No index gap: not a point-column comb.
        let html: String = (0..7)
            .map(|i| format!("<input name=\"amt\" data-slot-index=\"{i}\" value=\"\">"))
            .collect();
        let tags = super::input_tags(&html);
        let ordered: Vec<usize> = (0..tags.len()).collect();
        assert_eq!(
            super::money_comb_values(&tags, &ordered, "0.00", super::NegativeStyle::Minus),
            super::MoneyComb::NotMoney
        );
    }

    /// F3 (a): an amount with more peso digits than the boxes before the
    /// point never spreads into the centavo boxes. Before, `12345678901` in
    /// a 9 + 2 comb printed `123456789` before the point and `01` after it,
    /// reading 123,456,789.01; it now prints whole as one rectangle.
    #[test]
    fn a_too_long_amount_never_spreads_into_the_centavo_boxes() {
        let html: String = (0..9)
            .chain(10..12)
            .map(|i| format!("<input name=\"p1c9\" data-slot-index=\"{i}\" value=\"\">"))
            .collect();
        let html = format!(
            "<div id=\"p1c9\" style=\"left:10pt;top:10pt;width:150pt;height:14pt\">{html}</div>"
        );
        let fields = BTreeMap::from([("frm1601C:txtTax".to_string(), "12345678901".to_string())]);
        let cells = BTreeMap::from([("frm1601C:txtTax".to_string(), vec!["p1c9".to_string()])]);
        let doc = super::fill_by_name_with_cells(&html, &fields, &cells, &BTreeMap::new());
        assert_eq!(comb_values(&doc, "p1c9"), vec![""; 11]);
        assert_eq!(super::printed_cell(&doc, "p1c9").0, "12,345,678,901.00");
        // The same with centavos, and with more than two decimals.
        assert_eq!(
            money_comb("1234567.89", super::NegativeStyle::Minus),
            super::MoneyComb::Rect("1,234,567.89".into())
        );
        assert_eq!(
            money_comb("1,000.505", super::NegativeStyle::Minus),
            super::MoneyComb::Rect("1,000.505".into())
        );
    }

    /// F4: a point-column comb on a sheet whose form brackets its negatives
    /// (1702MX, 1702RT, 1707A) prints the brackets; before, the minus was
    /// hard-coded (`-1234` in the peso boxes, `56` after the point).
    #[test]
    fn a_point_column_comb_prints_the_bracket_forms_negative_in_brackets() {
        assert_eq!(
            money_comb("-1,234.56", super::NegativeStyle::Parentheses),
            super::MoneyComb::Rect("(1,234.56)".into())
        );
        assert_eq!(
            money_writes("-1,234.56").unwrap(),
            ["-", "1", "2", "3", "4", "5", "6"]
        );
        // A zero is never signed.
        assert!(matches!(
            money_comb("-0.00", super::NegativeStyle::Parentheses),
            super::MoneyComb::Boxes(_)
        ));
        // Through the fill, on the 1707A sheet's own type.
        let loaded = super::bundle("1707a-2021").expect("1707a bundle");
        let sheet = super::SheetType::of_sheet("1707a-2021", loaded.css);
        assert_eq!(sheet.negative, super::NegativeStyle::Parentheses);
        let html = format!(
            "<div id=\"amt\" style=\"left:10pt;top:10pt;width:120pt;height:14pt\">{}</div>",
            point_column_comb()
        );
        let fields = BTreeMap::from([("frm1707A:txtAmount".to_string(), "-1,234.56".to_string())]);
        let cells = BTreeMap::from([("frm1707A:txtAmount".to_string(), vec!["amt".to_string()])]);
        let doc = super::fill_by_name_set(
            &html,
            &fields,
            &cells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        assert_eq!(super::printed_cell(&doc, "amt").0, "(1,234.56)");
        assert!(comb_values(&doc, "amt").iter().all(String::is_empty));
    }

    #[test]
    fn every_print_map_is_registered_and_fills_its_sheet() {
        use std::collections::BTreeMap;
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
        let mut checked = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let dir = entry.unwrap().path();
            if !dir.join("writer-cells.json").exists() {
                continue;
            }
            let slug = dir.file_name().unwrap().to_str().unwrap();
            assert!(
                super::writer_cells_json(slug).is_some(),
                "{slug}: not registered"
            );
            let cells = super::writer_cells(slug).unwrap_or_else(|e| panic!("{slug}: {e}"));
            let mut fields = BTreeMap::new();
            for key in cells.joins.keys() {
                fields.insert(key.clone(), "1".to_string());
            }
            for join in &cells.money_joins {
                fields.insert(join.writer_key.clone(), "1234.56".to_string());
            }
            for join in &cells.xbox_joins {
                fields.insert(join.writer_key.clone(), "true".to_string());
            }
            for join in &cells.peso_joins {
                fields.insert(join.writer_key.clone(), "1234.56".to_string());
            }
            for join in &cells.date_joins {
                let date = if join.dd_html_id.is_some() {
                    "06/15/2025"
                } else {
                    "06/2025"
                };
                fields.insert(join.writer_key.clone(), date.to_string());
            }
            let filled =
                super::filled_document(slug, &fields).unwrap_or_else(|e| panic!("{slug}: {e}"));
            if cells.has_fill_join() {
                let empty = super::filled_document(slug, &BTreeMap::new())
                    .unwrap_or_else(|e| panic!("{slug}: {e}"));
                assert_ne!(filled, empty, "{slug}: fill joins changed nothing");
            }
            checked += 1;
        }
        assert!(checked >= 30, "only {checked} print maps found");
    }

    /// F1: a whole-peso comb prints the upload's own figure. Before, the
    /// centavos were rounded half up (`-1,000.51` printed `-1001`, the
    /// official page shows `-1,000.51`); an amount with centavos now prints
    /// whole in a rectangle, one without them in the boxes.
    #[test]
    fn peso_only_combs_print_the_uploads_centavos() {
        use super::{peso_print, NegativeStyle::*, PesoPrint::*};
        assert_eq!(peso_print("-1,000.51", Minus), Rect("-1,000.51".into()));
        assert_eq!(peso_print("1234.50", Minus), Rect("1,234.50".into()));
        assert_eq!(peso_print("0.49", Minus), Rect("0.49".into()));
        assert_eq!(
            peso_print("-1,234.50", Parentheses),
            Rect("(1,234.50)".into())
        );
        assert_eq!(peso_print("1,234.00", Minus), Boxes("1234".into()));
        assert_eq!(peso_print("12", Minus), Boxes("12".into()));
        assert_eq!(peso_print("-5.00", Minus), Boxes("-5".into()));
        assert_eq!(peso_print("-0.00", Minus), Boxes("0".into()));
        // F3 (c): more than two decimals still prints, whole.
        assert_eq!(peso_print("1,000.505", Minus), Rect("1,000.505".into()));
    }

    /// F1: a plain amount box on a whole-peso form keeps the upload's
    /// centavos; before, `-1,000.51` printed `-1,001`.
    #[test]
    fn whole_peso_plain_boxes_print_the_uploads_centavos() {
        use super::{format_plain_whole_peso_with_commas as plain, NegativeStyle::*};
        assert_eq!(plain("-1,000.51", Minus).as_deref(), Some("-1,000.51"));
        assert_eq!(plain("1000.49", Minus).as_deref(), Some("1,000.49"));
        assert_eq!(
            plain("-1000.51", Parentheses).as_deref(),
            Some("(1,000.51)")
        );
        assert_eq!(plain("300,000.00", Minus).as_deref(), Some("300,000"));
        assert_eq!(plain("-0.00", Minus).as_deref(), Some("0"));
    }

    /// F3 (b), (c): no amount prints blank or loses a character. A cell the
    /// sheet gives no geometry cannot take a rectangle: its first box is
    /// marked for the additional sheet with the whole amount. A money join
    /// whose value has more than two decimals prints whole (before it was
    /// skipped and the boxes stayed blank).
    #[test]
    fn an_amount_never_prints_blank_or_short() {
        // (b) A stamped (not writer-cell) amount longer than its comb, and a
        // comb with no geometry: before, the characters past the last box
        // were dropped.
        let html = comb("frm1601C:txtTax", 4);
        let fields = BTreeMap::from([("frm1601C:txtTax".to_string(), "123,456.78".to_string())]);
        let doc = super::fill_by_name(&html, &fields);
        assert!(comb_values(&doc, "frm1601C:txtTax")
            .iter()
            .all(String::is_empty));
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1, "{doc}");
        assert_eq!(warnings[0].kind, "text-truncated");
        assert_eq!(warnings[0].value, "123,456.78");

        // (c) Three decimals into a peso + centavo money join.
        let html = format!(
            "<div id=\"p1c56\" style=\"left:10pt;top:10pt;width:90pt;height:14pt\">{}</div>\
             <div id=\"p1c57\" style=\"left:100pt;top:10pt;width:24pt;height:14pt\">{}</div>",
            comb("p1c56", 8),
            comb("p1c57", 2)
        );
        let fields = BTreeMap::from([("k".to_string(), "1,000.505".to_string())]);
        let joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p1c56".into(),
                cent_html_id: Some("p1c57".into()),
                point_slot: false,
            }],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(super::printed_cell(&doc, "p1c56").0, "1,000.505");

        // (c) A money join whose comb has no two centavo boxes.
        let html = format!(
            "<div id=\"p1c58\" style=\"left:10pt;top:10pt;width:60pt;height:14pt\">{}</div>",
            comb("p1c58", 3)
        );
        let fields = BTreeMap::from([("k".to_string(), "12.50".to_string())]);
        let joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p1c58".into(),
                cent_html_id: None,
                point_slot: false,
            }],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(super::printed_cell(&doc, "p1c58").0, "12.50");

        // A whole-peso comb with no geometry and too many digits: before,
        // no rectangle could be built and the boxes stayed blank.
        let html = comb("p", 3);
        let fields = BTreeMap::from([("k".to_string(), "1,234,567.00".to_string())]);
        let joins = [super::PesoJoin {
            writer_key: "k".into(),
            html_ids: vec!["p".into()],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_peso_joins(&html, &fields, &joins);
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1, "{doc}");
        assert_eq!(warnings[0].kind, "text-truncated");
        assert_eq!(warnings[0].value, "1,234,567");
    }

    /// F2: an amount in a rectangle too narrow for it at 7.5 pt is shrunk
    /// until it fits, never clipped. Before, the size was floored at 7.5 pt
    /// and `-1,234,567,890,123.45` set 103 pt wide in a 60 pt rectangle.
    #[test]
    fn a_long_amount_in_a_narrow_rectangle_prints_whole() {
        let loaded = super::bundle("1601c-2018").expect("1601c bundle");
        let sheet = super::SheetType::of_sheet("1601c-2018", loaded.css);
        let html = format!(
            "<div id=\"p1c90\" style=\"left:10pt;top:10pt;width:60pt;height:14pt\">{}</div>",
            comb("p1c90", 4)
        );
        let fields = BTreeMap::from([(
            "frm1601C:txtTax".to_string(),
            "-1,234,567,890,123.45".to_string(),
        )]);
        let cells = BTreeMap::from([("frm1601C:txtTax".to_string(), vec!["p1c90".to_string()])]);
        let doc = super::fill_by_name_set(
            &html,
            &fields,
            &cells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        let (text, size) = super::printed_cell(&doc, "p1c90");
        assert_eq!(text, "-1,234,567,890,123.45");
        let size = size.expect("a fitted size");
        assert!(size < super::PLAIN_TEXT_MIN_PT, "{size}");
        // It sets within the rectangle's room: 60 pt less the input's 1 pt
        // insets and 1 pt right padding.
        let width = sheet.width(sheet.letter, &text, size);
        assert!(width <= 60.0 - 3.0, "{width} pt at {size} pt");
        assert!(!doc.contains("text-truncated"));

        // A plain amount box: shrunk on one line, never wrapped or cut.
        let tag = r#"<input class="fi fh5" style="inset:0.96pt 1.92pt 0.96pt 2pt">"#;
        let fit = super::fit_plain_text(&sheet, 30.0, 30.0, tag, "1,234,567.89", true, true)
            .expect("too wide at its class size");
        assert_eq!(fit.text, "1,234,567.89");
        assert!(!fit.two_lines && !fit.truncated);
        assert!(fit.font_size_pt < super::PLAIN_TEXT_MIN_PT);
    }

    /// TINs and ATCs are identifiers, never shortened on the page: by the
    /// writer key's name, else by the caption the sheet prints for the cell.
    #[test]
    fn tin_and_atc_cells_are_identifiers() {
        use super::is_identifier_cell as id;
        for key in [
            "frm1600PT:Pg2Sc1TIN1",
            "frm1600PT:Pg2Sc1ATC1",
            "frm1707:txtPg1I6SellerTIN0",
            "frm1801v2018:txtTINE1",
            "frm1600WP:dtSched:txtTin1",
            "frm1600WP:dtSched:drpAtcCode1",
            "frm1702RT:txtPg1Pt2SignatoryTin1",
            "frm1702RT:drpPg1I5AtcOther",
            "frm2200Av2020:txtSched1_xa_others1_Atc",
            "frm2200P:txtSched1ATC26",
            "frm1604c:tinA",
            "frm1601FQ:txtBranchCode",
            "derived:tin_digits",
            "derived:buyer_tin",
            "derived:employer1_tin",
            "derived:sched1_atc1",
            "derived:tp_atc",
            "frm1600PT:Pg2Sc1TIN1__cont_cont_s0_p0_b0_r0_p2c16",
        ] {
            assert!(id(key, None), "{key}");
        }
        assert!(id(
            "frm2200ANv2018:txtXGOthers1",
            Some("Schedule 1B Others (specify) row 1, ATC")
        ));
        assert!(id("frm2553:txt18B", Some("18B ATC")));
        for (key, caption) in [
            (
                "derived:tp_atc_description",
                Some("18 ATC (option text beside the code)"),
            ),
            (
                "derived:atc_description",
                Some("5 ATC description (second line)"),
            ),
            (
                "derived:page_header",
                Some("Page 2 header: TIN / Taxpayer's Name"),
            ),
            (
                "txtTaxAgentNo",
                Some("Tax Agent Accreditation No. / (Indicate Title/Designation and TIN)"),
            ),
            (
                "frm2200ANv2018:txtXG021Exempt1",
                Some("ATC XG021 row, Exempt1"),
            ),
            ("frm1602Q:txtSched3TotInterest1", None),
            ("derived:crediting_month", None),
            (
                "frm1601FQ:txtNatIncPayment0",
                Some("Schedule 1 row 1, Nature of Income Payment"),
            ),
        ] {
            assert!(!id(key, caption), "{key}");
        }
    }

    /// N2: a TIN or ATC is never shortened on the page. Before, 1600-PT's
    /// Schedule 1 payee TIN (plain box p2c16) printed `987654...` and
    /// 2200-P's Others ATC (p2c376) `9...`, the whole only on the
    /// additional sheet.
    #[test]
    fn tins_and_atcs_print_whole_in_their_boxes() {
        for (slug, key, cell, value) in [
            (
                "1600-pt-2018",
                "frm1600PT:Pg2Sc1TIN1",
                "p2c16",
                "987654321000",
            ),
            ("2200p-2020", "frm2200P:txtSched1ATC26", "p2c376", "999"),
        ] {
            let fields = BTreeMap::from([(key.to_string(), value.to_string())]);
            let doc = super::filled_document(slug, &fields).unwrap();
            assert_eq!(super::printed_text(&doc, cell), value, "{slug} {cell}");
            assert!(
                !super::overflow_warnings(&doc)
                    .iter()
                    .any(|w| w.kind == "text-truncated" && w.cell == cell),
                "{slug} {cell}"
            );
        }
    }

    /// N2: a value longer than its comb is never spread with its tail lost.
    /// A stamped comb (`via_cell` false) printed the first boxes' worth
    /// (`["1", "2", "3", "4"]` of `123456788000`); it now prints whole over
    /// the comb, and where the cell has no geometry its boxes stay empty
    /// and the whole value goes to the additional sheet.
    #[test]
    fn a_long_comb_value_never_loses_its_tail() {
        let html = format!(
            "<div id=\"frm1601C:txtTIN1\" style=\"left:10pt;top:10pt;width:40pt;height:14pt\">{}</div>",
            comb("frm1601C:txtTIN1", 4)
        );
        let fields = BTreeMap::from([("frm1601C:txtTIN1".to_string(), "123456788000".to_string())]);
        let doc = super::fill_by_name(&html, &fields);
        assert!(comb_values(&doc, "frm1601C:txtTIN1")
            .iter()
            .all(String::is_empty));
        assert_eq!(
            super::printed_cell(&doc, "frm1601C:txtTIN1").0,
            "123456788000"
        );
        assert!(!doc.contains("..."));

        let html = comb("frm1601C:txtName", 4);
        let fields = BTreeMap::from([("frm1601C:txtName".to_string(), "SAMPLE NAME".to_string())]);
        let doc = super::fill_by_name(&html, &fields);
        assert!(comb_values(&doc, "frm1601C:txtName")
            .iter()
            .all(String::is_empty));
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1, "{doc}");
        assert_eq!(warnings[0].value, "SAMPLE NAME");
    }

    /// N3: a figure without decimals in a box whose caption names no amount
    /// is never cut. 2200-P's Place of Removal box (p2c99) printed
    /// `-200000000...`; it now shrinks and prints whole.
    #[test]
    fn a_decimal_less_figure_is_never_cut() {
        let fields = BTreeMap::from([(
            "frm2200P:txtSched1PlaceOfRemoval6".to_string(),
            "-200000000000".to_string(),
        )]);
        let doc = super::filled_document("2200p-2020", &fields).unwrap();
        assert_eq!(super::printed_text(&doc, "p2c99"), "-200000000000");
        assert!(!super::overflow_warnings(&doc)
            .iter()
            .any(|w| w.kind == "text-truncated"));
    }

    /// N1: an amount shrunk below 5 pt to fit its box still prints there,
    /// whole, and is also carried whole to the additional sheet. Before, it
    /// printed at 2-4 pt with nothing else.
    #[test]
    fn an_amount_below_5_pt_is_also_on_the_additional_sheet() {
        let loaded = super::bundle("1601c-2018").expect("1601c bundle");
        let sheet = super::SheetType::of_sheet("1601c-2018", loaded.css);
        let html = format!(
            "<div class=\"page page-1\"><div id=\"p1c90\" style=\"left:10pt;top:10pt;width:40pt;height:14pt\">{}</div></div>",
            comb("p1c90", 4)
        );
        let fields = BTreeMap::from([(
            "frm1601C:txtTax".to_string(),
            "-1,234,567,890,123.45".to_string(),
        )]);
        let cells = BTreeMap::from([("frm1601C:txtTax".to_string(), vec!["p1c90".to_string()])]);
        let doc = super::fill_by_name_set(
            &html,
            &fields,
            &cells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        let (text, size) = super::printed_cell(&doc, "p1c90");
        assert_eq!(text, "-1,234,567,890,123.45");
        assert!(size.unwrap() < super::SMALL_WHOLE_BELOW_PT);
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "amount-small");
        assert_eq!(warnings[0].value, "-1,234,567,890,123.45");
        let report = crate::continuation::OverflowReport::default();
        let entries = crate::continuation::collect_truncated_entries("1601c-2018", &doc, &report);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].full_text, "-1,234,567,890,123.45");
    }

    /// The print classes exactly the TIN and ATC joins the rendered parity
    /// test pins (`crates/bir-core/tests/sample_payloads/fixtures/identifier-joins.tsv`:
    /// 268 writer-cells.json joins on 41 sheets and the 4 SHEET_JOINS TIN
    /// parts of 2550M): a change to the classing is a diff of that list.
    #[test]
    fn identifier_classing_matches_the_pinned_list() {
        let pinned: std::collections::BTreeSet<(String, String)> =
            include_str!("../../bir-core/tests/sample_payloads/fixtures/identifier-joins.tsv")
                .lines()
                .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
                .map(|line| {
                    let (sheet, key) = line.split_once('\t').expect("sheet<TAB>key");
                    (sheet.to_string(), key.to_string())
                })
                .collect();
        let mut classed = std::collections::BTreeSet::new();
        for slug in sheet_slugs() {
            let Some(cells) = super::merged_writer_cells_json(&slug) else {
                continue;
            };
            for join in cells["joins"].as_array().unwrap() {
                let key = join["writer_key"].as_str().unwrap();
                if super::is_identifier_cell(key, join["printed"].as_str()) {
                    classed.insert((slug.clone(), key.to_string()));
                }
            }
        }
        assert_eq!(classed, pinned);
        assert_eq!(pinned.len(), 272);
    }

    /// The capitals rule wants `TIN` or `ATC` as a word of its own, and no
    /// description, check box or radio button is an identifier.
    #[test]
    fn identifier_capitals_need_a_word_boundary() {
        use super::is_identifier_cell as id;
        for key in [
            "frm1700:txtPg1I18STIN1",
            "frm1702MX:txtPg2Pt3TINofSignatory",
            "frm1801v2018:txtTINE1",
            "frm1606:txtTINS1",
            "frm1800v2018:txtDoneeTINA",
            "frm0605:txtATCCode",
            "frm1702MX:drpPg1I5ATCR1",
        ] {
            assert!(id(key, None), "{key}");
        }
        for key in [
            "frm9999:txtTINStatus",
            "frm9999:chkTIN",
            "frm9999:txtATCDesc",
            "frm9999:txtACCOUNTING",
            "frm9999:txtACCOUNTINGPERIOD",
            "frm9999:rdoATC1",
            "frm9999:optTIN_1",
        ] {
            assert!(!id(key, None), "{key}");
        }
    }

    /// I1: a TIN shrunk below 5 pt to fit its box prints there whole AND is
    /// carried whole to the additional sheet, with a preview warning.
    /// Before, a dashed 14-digit TIN in 1600-PT's Schedule 1 TIN box
    /// printed whole at under 5 pt and nowhere else.
    #[test]
    fn a_tiny_tin_is_also_on_the_additional_sheet() {
        let tin = "987-654-321-00000";
        let fields = BTreeMap::from([("frm1600PT:Pg2Sc1TIN1".to_string(), tin.to_string())]);
        let doc = super::filled_document_with_report("1600-pt-2018", &fields, None).unwrap();
        assert_eq!(super::printed_text(&doc.html, "p2c16"), tin);
        let tag = super::input_tags(&doc.html)
            .into_iter()
            .find(|t| t.name == "p2c16")
            .expect("the TIN box")
            .tag
            .to_string();
        let size: f64 = tag
            .rsplit("font-size:")
            .next()
            .and_then(|v| v.split("pt").next())
            .and_then(|v| v.parse().ok())
            .expect("a fitted size");
        assert!(size < super::SMALL_WHOLE_BELOW_PT, "{size}");
        assert!(
            tag.contains("data-overflow-warning=\"identifier-small\""),
            "{tag}"
        );
        let additional = &doc.html[doc
            .html
            .find("page-continuation")
            .expect("an additional sheet")..];
        assert!(
            additional.contains(tin),
            "the additional sheet carries the TIN whole"
        );
        assert!(
            doc.warning_lines().iter().any(|l| l.contains("TIN or ATC")),
            "{:?}",
            doc.warning_lines()
        );
    }

    /// (a): a figure printed whole over a comb it does not fit, shrunk below
    /// 5 pt, is marked for the additional sheet as well.
    #[test]
    fn a_tiny_whole_line_figure_is_also_on_the_additional_sheet() {
        let html = format!(
            "<div id=\"frm1601C:txtPhone\" style=\"left:10pt;top:10pt;width:20pt;height:14pt\">{}</div>",
            comb("frm1601C:txtPhone", 4)
        );
        let fields = BTreeMap::from([(
            "frm1601C:txtPhone".to_string(),
            "09170000000123456".to_string(),
        )]);
        let doc = super::fill_by_name(&html, &fields);
        let (text, size) = super::printed_cell(&doc, "frm1601C:txtPhone");
        assert_eq!(text, "09170000000123456");
        assert!(size.unwrap() < super::SMALL_WHOLE_BELOW_PT);
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1, "{doc}");
        assert_eq!(warnings[0].kind, "amount-small");
        assert_eq!(warnings[0].value, "09170000000123456");
    }

    /// F6: every sheet the negative-style table names a bracket form is a
    /// real bundle with writer cells: a slug that names no sheet would leave
    /// its form printing a minus.
    #[test]
    fn every_bracket_sheet_is_a_real_bundle() {
        for slug in super::BRACKET_SHEETS {
            assert!(super::bundle(slug).is_some(), "{slug}: no frozen bundle");
            assert!(
                super::writer_cells_json(slug).is_some(),
                "{slug}: no writer-cells.json"
            );
            assert_eq!(
                super::NegativeStyle::of_sheet(slug),
                super::NegativeStyle::Parentheses
            );
        }
        assert_eq!(
            super::NegativeStyle::of_sheet("1702ex-2018"),
            super::NegativeStyle::Minus
        );
    }

    fn comb(name: &str, slots: usize) -> String {
        (0..slots)
            .map(|i| format!("<input name=\"{name}\" data-slot-index=\"{i}\" value=\"\">"))
            .collect()
    }

    fn comb_values(doc: &str, name: &str) -> Vec<String> {
        super::input_tags(doc)
            .iter()
            .filter(|tag| tag.name == name)
            .map(|tag| super::attr(tag.tag, "value").unwrap_or("").to_string())
            .collect()
    }

    #[test]
    fn negative_money_join_puts_minus_left_of_pesos() {
        use std::collections::BTreeMap;
        let html = format!("{}{}", comb("p", 8), comb("c", 2));
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "-122,500.25".to_string());
        let joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p".into(),
                cent_html_id: Some("c".into()),
                point_slot: false,
            }],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(
            comb_values(&doc, "p"),
            ["", "-", "1", "2", "2", "5", "0", "0"]
        );
        assert_eq!(comb_values(&doc, "c"), ["2", "5"]);
        fields.insert("k".to_string(), "-12,345,678.00".to_string());
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(
            comb_values(&doc, "p"),
            vec![String::new(); 8],
            "no room for '-'"
        );
    }

    #[test]
    fn parenthesized_negative_money_join_puts_parens_around_amount() {
        use std::collections::BTreeMap;
        let html = format!(
            "<div id=\"p\" style=\"left:0pt;top:0pt;width:90pt;height:14pt\">{}</div><div id=\"c\" style=\"left:95pt;top:0pt;width:24pt;height:14pt\">{}</div>",
            comb("p", 8),
            comb("c", 2)
        );
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "(799,061.01)".to_string());
        let joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p".into(),
                cent_html_id: Some("c".into()),
                point_slot: false,
            }],
            negative_style: super::NegativeStyle::Parentheses,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        // Rule 2: a bracketed negative on a split peso+centavo comb becomes a
        // clean plain rectangle across the full amount, right-aligned with
        // commas and two decimals.
        let (printed, size) = super::printed_cell(&doc, "p");
        assert_eq!(printed, "(799,061.01)");
        assert!(size.unwrap() >= super::PLAIN_TEXT_MIN_PT);
        assert_eq!(comb_values(&doc, "p"), [""; 8]);
        assert_eq!(comb_values(&doc, "c"), [""; 2]);
        fields.insert("k".to_string(), "(0.00)".to_string());
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(comb_values(&doc, "p"), ["", "", "", "", "", "", "", "0"]);
        assert_eq!(comb_values(&doc, "c"), ["0", "0"]);
    }

    #[test]
    fn decimal_comb_money_puts_cents_in_the_last_two_slots() {
        use std::collections::BTreeMap;
        let html = comb("p", 8);
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "1,234.50".to_string());
        let mut joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p".into(),
                cent_html_id: None,
                point_slot: true,
            }],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(
            comb_values(&doc, "p"),
            ["", "1", "2", "3", "4", "", "5", "0"]
        );
        joins[0].cells[0].point_slot = false;
        let doc = super::fill_money_joins(&html, &fields, &joins);
        assert_eq!(
            comb_values(&doc, "p"),
            ["", "", "1", "2", "3", "4", "5", "0"]
        );
    }

    #[test]
    fn one_writer_key_fills_every_listed_cell() {
        use std::collections::BTreeMap;
        let html = format!("{}{}", comb("a", 4), comb("b", 3));
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "Ab1".to_string());
        let mut cells = BTreeMap::new();
        cells.insert("k".to_string(), vec!["a".to_string(), "b".to_string()]);
        let doc = super::fill_by_name_with_cells(&html, &fields, &cells, &BTreeMap::new());
        assert_eq!(comb_values(&doc, "a"), ["A", "B", "1", ""]);
        assert_eq!(comb_values(&doc, "b"), ["A", "B", "1"]);
    }

    #[test]
    fn date_join_splits_one_writer_date_over_its_combs() {
        use std::collections::BTreeMap;
        assert_eq!(
            super::split_writer_date("6/9/2025"),
            Some(("06".into(), Some("09".into()), "2025".into()))
        );
        assert_eq!(
            super::split_writer_date("2025-06-09"),
            Some(("06".into(), Some("09".into()), "2025".into()))
        );
        assert_eq!(
            super::split_writer_date("04/2025"),
            Some(("04".into(), None, "2025".into()))
        );
        assert_eq!(super::split_writer_date("June 9"), None);
        assert_eq!(super::split_writer_date(""), None);
        let html = format!("{}{}{}", comb("m", 2), comb("d", 2), comb("y", 4));
        let join = super::DateJoin {
            writer_key: "k".into(),
            mm_html_id: "m".into(),
            dd_html_id: Some("d".into()),
            yyyy_html_id: "y".into(),
        };
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "05/09/2025".to_string());
        let doc = super::fill_date_joins(&html, &fields, std::slice::from_ref(&join));
        assert_eq!(comb_values(&doc, "m"), ["0", "5"]);
        assert_eq!(comb_values(&doc, "d"), ["0", "9"]);
        assert_eq!(comb_values(&doc, "y"), ["2", "0", "2", "5"]);
        // A month/year value has no day for the DD comb: nothing prints.
        fields.insert("k".to_string(), "05/2025".to_string());
        let doc = super::fill_date_joins(&html, &fields, &[join]);
        assert_eq!(comb_values(&doc, "y"), ["", "", "", ""]);
    }

    #[test]
    fn xbox_join_names_one_box_of_a_two_box_cell_by_input_id() {
        use std::collections::BTreeMap;
        let html = concat!(
            r#"<input type="text" id="c-i0" name="c" maxlength="1">"#,
            r#"<input type="text" id="c-i1" name="c" maxlength="1">"#,
        );
        let join = super::XboxJoin {
            writer_key: "k".into(),
            html_ids: vec!["c-i1".into()],
        };
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "true".to_string());
        let doc = super::fill_xbox_joins(html, &fields, &[join]);
        assert_eq!(comb_values(&doc, "c"), ["", "X"]);
    }

    #[test]
    fn writer_cells_reject_reused_cells_stale_exceptions_and_old_sections() {
        let parse = |json: &str| super::parse_writer_cells(json).err().unwrap_or_default();
        assert!(parse(r#"{"joins":[{"writer_key":"a","html_id":"p1c1"},{"writer_key":"b","html_ids":["p1c2","p1c1"]}]}"#)
            .contains("already a fill target"));
        assert!(parse(r#"{"joins":[{"writer_key":"a","html_id":"p1c1"}],"money_joins":[{"writer_key":"a","decimal_html_id":"p1c2"}]}"#)
            .contains("duplicate writer_key a"));
        assert!(parse(r#"{"joins":[{"writer_key":"a","html_id":"p1c1"}],"not_joined":[{"writer_key":"a","reason":"x"}]}"#)
            .contains("remove it from not_joined"));
        assert!(
            parse(r#"{"joins":[],"not_joined":[{"writer_keys":["a","b"]}]}"#)
                .contains("needs a reason")
        );
        assert!(parse(r#"{"joins":[{"writer_key":"a","html_id":"p1c1"}],"not_joined_cells":[{"html_id":"p1c1","reason":"x"}]}"#)
            .contains("remove it from not_joined_cells"));
        assert!(parse(r#"{"joins":[],"left_out":[]}"#).contains("unknown section left_out"));
        let ok = super::parse_writer_cells(
            r#"{"joins":[{"writer_key":"a","html_id":"p1c1","html_ids":["p2c1"]}],
                "money_joins":[{"writer_key":"m","peso_html_id":"p1c2","cent_html_id":"p1c3","decimal_html_ids":["p2c2"],"point_slot":true}],
                "date_joins":[{"writer_key":"d","mm_html_id":"p1c4","yyyy_html_id":"p1c5"}],
                "not_joined":[{"writer_keys":["x","y"],"reason":"not on the paper form"}]}"#,
        )
        .unwrap();
        assert_eq!(ok.joins["a"], ["p1c1", "p2c1"]);
        assert_eq!(ok.money_joins[0].cells.len(), 2);
        assert!(ok.money_joins[0].cells[1].point_slot);
        assert_eq!(ok.not_joined["y"], "not on the paper form");
    }

    #[test]
    fn unprinted_keys_lists_values_with_no_box_and_no_reason() {
        use std::collections::BTreeMap;
        let mut fields = BTreeMap::new();
        for (key, value) in [
            ("frm1601c:txtTIN1", "111"),
            ("frm1601c:txtTaxpayerName", "NAME"),
            ("frm1601c:txtCurrentPage", "1"),
            ("frm1601c:txtTax14", "1.00"),
            ("frm1601c:unknownBox", "5"),
            ("frm1601c:emptyBox", ""),
        ] {
            fields.insert(key.to_string(), value.to_string());
        }
        assert_eq!(
            super::unprinted_keys("1601c-2018", &fields).unwrap(),
            ["frm1601c:unknownBox"]
        );
    }

    #[test]
    fn negative_peso_join_puts_minus_left_of_pesos() {
        use std::collections::BTreeMap;
        let html = comb("p", 5);
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "-123.00".to_string());
        let joins = [super::PesoJoin {
            writer_key: "k".into(),
            html_ids: vec!["p".into()],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_peso_joins(&html, &fields, &joins);
        assert_eq!(comb_values(&doc, "p"), ["", "-", "1", "2", "3"]);
        // Centavos the boxes cannot hold print whole in a rectangle, sign
        // kept (before, -122.50 rounded to -123).
        let html = format!(
            "<div id=\"p\" style=\"left:10pt;top:10pt;width:70pt;height:14pt\">{html}</div>"
        );
        fields.insert("k".to_string(), "-122.50".to_string());
        let doc = super::fill_peso_joins(&html, &fields, &joins);
        assert_eq!(comb_values(&doc, "p"), ["", "", "", "", ""]);
        assert_eq!(super::printed_cell(&doc, "p").0, "-122.50");
    }

    #[test]
    fn negative_style_follows_the_official_form() {
        use std::collections::BTreeMap;
        // The forms whose official page brackets a negative amount
        // (NegativeValue / roundAmount) and the rest (formatCurrency's minus).
        for slug in ["1702mx-2018c", "1702rt-2018c", "1707a-2021"] {
            assert_eq!(
                super::NegativeStyle::of_sheet(slug),
                super::NegativeStyle::Parentheses,
                "{slug}"
            );
        }
        for slug in [
            "1600-pt-2018",
            "1600-vt-2018",
            "1601-fq-2020",
            "1700-2018",
            "1701-2018",
            "1701a-2018",
            "1701ms-2024",
            "1701q-2018",
            "1702ex-2018",
            "1702q-2018",
            "1801-2018",
            "2550q-2024",
        ] {
            assert_eq!(
                super::NegativeStyle::of_sheet(slug),
                super::NegativeStyle::Minus,
                "{slug}"
            );
        }
        // 1702MX Schedule 8 item 3, a net operating loss: the official page
        // writes it through getDifference -> NegativeValue, so it prints in
        // brackets; before, the line's wording printed it with no sign at all.
        let mut m = BTreeMap::new();
        m.insert("frm1702MX:txtPg4Sc8It3".to_string(), "-200000".to_string());
        let doc = super::filled_document("1702mx-2018c", &m).unwrap();
        assert_eq!(super::printed_cell(&doc, "p4c53").0, "(200000)");
        // A minus form's split peso + centavo comb keeps the minus left of
        // the pesos (1600PT item 22, formatCurrency).
        let mut m = BTreeMap::new();
        m.insert(
            "frm1600PT:txtPg1P2I22TaxStillDueOverremit".to_string(),
            "-1,234.56".to_string(),
        );
        let doc = super::filled_document("1600-pt-2018", &m).unwrap();
        let (pesos, _) = super::printed_cell(&doc, "p1c116");
        let (cents, _) = super::printed_cell(&doc, "p1c118");
        assert!(
            pesos.starts_with('-') && pesos.contains("1") && !pesos.contains('('),
            "{pesos:?} {cents:?}"
        );

        let html = comb("p", 8);
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "-123".to_string());
        let paren = [super::PesoJoin {
            writer_key: "k".into(),
            html_ids: vec!["p".into()],
            negative_style: super::NegativeStyle::Parentheses,
        }];
        let doc = super::fill_peso_joins(&html, &fields, &paren);
        assert_eq!(
            comb_values(&doc, "p"),
            ["", "", "", "(", "1", "2", "3", ")"]
        );
        let minus = [super::PesoJoin {
            writer_key: "k".into(),
            html_ids: vec!["p".into()],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_peso_joins(&html, &fields, &minus);
        assert_eq!(comb_values(&doc, "p"), ["", "", "", "", "-", "1", "2", "3"]);
    }

    #[test]
    fn overwide_comb_amounts_print_right_aligned_with_warning_on_official_and_cloned_cells() {
        use std::collections::BTreeMap;
        let comb_div = |id: &str, width: f64, slots: usize| {
            format!(
                "<div id=\"{id}\" style=\"left:10pt;top:10pt;width:{width}pt;height:14pt\">{}</div>",
                comb(id, slots)
            )
        };
        // 1. MoneyJoin with separate peso + centavo combs (e.g. 2000OT / 1801)
        let html = format!(
            "{}{}",
            comb_div("p1c56", 90.0, 8),
            comb_div("p1c57", 24.0, 2)
        );
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "-1,134,107,265.03".to_string());
        let joins = [super::MoneyJoin {
            writer_key: "k".into(),
            cells: vec![super::MoneyCells {
                peso_html_id: "p1c56".into(),
                cent_html_id: Some("p1c57".into()),
                point_slot: false,
            }],
            negative_style: super::NegativeStyle::Minus,
        }];
        let doc = super::fill_money_joins(&html, &fields, &joins);
        let (printed, size) = super::printed_cell(&doc, "p1c56");
        assert_eq!(printed, "-1,134,107,265.03");
        assert!(size.unwrap() >= super::PLAIN_TEXT_MIN_PT);
        assert!(doc.contains("text-align:right;justify-content:flex-end;padding-right:1pt"));
        assert_eq!(comb_values(&doc, "p1c57"), ["", ""]);
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].cell, "p1c56");
        assert_eq!(warnings[0].kind, "amount-comb");
        assert_eq!(warnings[0].value, "-1,134,107,265.03");

        // 2. PesoJoin (e.g. 1701)
        let html = comb_div("cont2-p1c80", 72.0, 8);
        fields.insert("k".to_string(), "-199,914,152.00".to_string());
        let pjoins = [super::PesoJoin {
            writer_key: "k".into(),
            html_ids: vec!["cont2-p1c80".into()],
            negative_style: super::NegativeStyle::Parentheses,
        }];
        let doc = super::fill_peso_joins(&html, &fields, &pjoins);
        let (printed, size) = super::printed_cell(&doc, "cont2-p1c80");
        assert_eq!(printed, "(199,914,152)");
        assert!(size.unwrap() >= super::PLAIN_TEXT_MIN_PT);
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].cell, "cont2-p1c80");
        assert_eq!(warnings[0].kind, "amount-comb");

        // 3. Derived amount comb via fill_by_name_with_cells (1700 / 1701A /
        // 1701MS); outside a sheet a negative takes the minus, however the
        // value came in.
        let html = comb_div("p1c90", 72.0, 8);
        let mut dfields = BTreeMap::new();
        dfields.insert("derived:amount_22A".to_string(), "(199914152)".to_string());
        let mut cells = BTreeMap::new();
        cells.insert("derived:amount_22A".to_string(), vec!["p1c90".to_string()]);
        let doc = super::fill_by_name_with_cells(&html, &dfields, &cells, &BTreeMap::new());
        let (printed, _) = super::printed_cell(&doc, "p1c90");
        assert_eq!(printed, "-199,914,152");
        assert!(doc.contains("text-align:right;justify-content:flex-end;padding-right:1pt"));
        assert_eq!(super::overflow_warnings(&doc).len(), 1);
    }

    #[test]
    fn plain_text_wraps_above_18pt_floors_at_6_5pt_and_ellipsizes_on_official_and_cloned_cells() {
        use std::collections::BTreeMap;

        let plain_div = |id: &str, width: f64, height: f64| {
            format!(
                "<style>.fh0{{font-size:9pt;line-height:{height}pt}}</style>\
                 <div id=\"{id}\" class=\"c f\" data-cell-kind=\"field\" data-field-kind=\"text\" \
                 style=\"left:0pt;top:0pt;width:{width}pt;height:{height}pt\">\
                 <input type=\"text\" class=\"fi fh0\" id=\"{id}-i\" name=\"{id}\" style=\"inset:0.75pt\" value=\"\">\
                 </div>"
            )
        };

        // 1. Tall cell (>= 18pt) wraps onto 2 lines before shrinking below 6.5pt (no warning).
        let html = plain_div("p1c100_clone2", 120.0, 20.0);
        let medium = "AYALA LAND INCORPORATED PREFERRED B SHARES";
        let mut fields = BTreeMap::new();
        fields.insert("derived:stock_name".to_string(), medium.to_string());
        let mut cells = BTreeMap::new();
        cells.insert(
            "derived:stock_name".to_string(),
            vec!["p1c100_clone2".to_string()],
        );
        let sheet = super::SheetType::of(".fh0 { font-size:9pt;line-height:20pt }");
        let doc = super::fill_by_name_set(
            &html,
            &fields,
            &cells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        assert!(
            doc.contains("&#10;"),
            "tall cell should wrap onto two lines: {doc}"
        );
        assert!(
            doc.contains("line-height:1.04"),
            "two-line wrap sets line-height: {doc}"
        );
        assert!(
            super::overflow_warnings(&doc).is_empty(),
            "wrapped value that fits at >=7.5pt emits no truncation warning"
        );

        // 2. Very long text in tall cloned cell floors at 7.50pt, ends with "...", and warns with full text.
        let huge = "AYALA LAND INCORPORATED CUMULATIVE REDEEMABLE NON-VOTING CONVERTIBLE PREFERRED SERIES B CLASS TWO SHARES OF STOCK";
        fields.insert("derived:stock_name".to_string(), huge.to_string());
        let doc = super::fill_by_name_set(
            &html,
            &fields,
            &cells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        assert!(
            doc.contains("font-size:7.50pt"),
            "must floor at 7.50pt: {doc}"
        );
        assert!(doc.contains("...\""), "must end with ellipsis: {doc}");
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].cell, "p1c100_clone2");
        assert_eq!(warnings[0].kind, "text-truncated");
        assert_eq!(warnings[0].value, huge);
        assert_eq!(warnings[0].font_size_pt, Some(7.5));

        // 3. Short cell (< 18pt) does NOT wrap; floors at 7.50pt with "..." and warns.
        let short_html = plain_div("p1c50", 90.0, 12.0);
        let mut scells = BTreeMap::new();
        scells.insert("derived:stock_name".to_string(), vec!["p1c50".to_string()]);
        let doc = super::fill_by_name_set(
            &short_html,
            &fields,
            &scells,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &sheet,
        );
        assert!(
            !doc.contains("&#10;"),
            "short cell (<18pt) must not wrap: {doc}"
        );
        assert!(
            doc.contains("font-size:7.50pt"),
            "short cell floors at 7.50pt: {doc}"
        );
        assert!(
            doc.contains("...\""),
            "short cell ends with ellipsis: {doc}"
        );
        let warnings = super::overflow_warnings(&doc);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].cell, "p1c50");
        assert_eq!(warnings[0].kind, "text-truncated");
        assert_eq!(warnings[0].value, huge);
    }

    #[test]
    fn checked_xbox_is_centred_like_a_char_box() {
        use std::collections::BTreeMap;
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:AmendedRtn_1".to_string(), "true".to_string());
        let doc = super::filled_document("1601c-2018", &fields).expect("form");
        let tag_at = doc.find("id=\"p1c21-i\"").expect("amended-yes xbox");
        let tag =
            &doc[doc[..tag_at].rfind('<').unwrap()..doc[tag_at..].find('>').unwrap() + tag_at + 1];
        assert!(tag.contains("value=\"X\""), "{tag}");
        assert!(tag.contains("class=\"fi fh3 fc\""), "{tag}");
        assert_eq!(
            super::with_class("<input class=\"fi fc\">", "fc"),
            "<input class=\"fi fc\">"
        );
        assert_eq!(super::with_class("<input>", "fc"), "<input class=\"fc\">");
    }

    #[test]
    fn receipt_page_follows_the_last_form_page() {
        use std::collections::BTreeMap;
        let receipt = super::ReceiptPage {
            filename: "00000000000000-1601Cv2018-112026.xml".into(),
            subject: "Tax Return Receipt Confirmation".into(),
            from: "ebirforms-noreply@bir.gov.ph".into(),
            to: "codeitlikemiley@gmail.com".into(),
            received_at: "12 September 2026, 02:52 PM".into(),
            email_received_at: Some("12 September 2026, 02:59 PM".into()),
            body_text: "This confirms receipt <of> your submission".into(),
            body_html: Some("<p>This confirms <b>receipt</b></p>".into()),
        };
        let fields = BTreeMap::new();
        let plain = super::filled_document("1601c-2018", &fields).expect("form");
        assert!(!plain.contains("page-receipt"));

        let with = super::filled_document_with_receipt("1601c-2018", &fields, Some(&receipt))
            .expect("form + receipt");
        let page2 = with.find("id=\"page-2\"").expect("page 2");
        let receipt_at = with.find("id=\"page-receipt\"").expect("receipt page");
        let body_end = with.rfind("</body>").expect("body end");
        assert!(page2 < receipt_at && receipt_at < body_end);
        assert!(
            with.contains("<div class=\"page page-receipt\""),
            "same element type as the form pages so :last-of-type moves to the receipt"
        );
        assert!(
            with.contains("style=\"width:612pt;min-height:936pt"),
            "sheet matches @page"
        );
        assert!(
            with.contains("<p>This confirms <b>receipt</b></p>"),
            "html body embedded as given"
        );
        assert!(with.contains("<h1 style=\"font-size:17pt;font-weight:700;margin:0 0 10pt 0\">Tax Return Receipt Confirmation</h1>"));
        assert!(with
            .contains("<b>ebirforms-noreply@bir.gov.ph</b> &lt;ebirforms-noreply@bir.gov.ph&gt;"));
        assert!(
            with.contains("12 September 2026, 02:59 PM"),
            "email arrival time shown"
        );
        assert!(with.contains("To: codeitlikemiley@gmail.com"));
        assert!(!with.contains("<th"), "no label table of our own");
        assert!(
            !with.contains("receipt &lt;of&gt;"),
            "text is not printed beside the html"
        );

        let text_only = super::ReceiptPage {
            body_html: None,
            ..receipt.clone()
        };
        let with_text =
            super::filled_document_with_receipt("1601c-2018", &fields, Some(&text_only))
                .expect("form + text receipt");
        assert!(
            with_text.contains("receipt &lt;of&gt; your submission"),
            "text fallback is escaped"
        );
        assert!(!with_text.contains("class=\"receipt-html\""));
        assert!(with_text.contains("class=\"receipt-text\""));
    }

    #[test]
    fn page_size_comes_from_the_bundle_css() {
        assert_eq!(
            super::page_size_from_css("@page { size:612pt 936pt;margin:0 }"),
            ("612pt".to_string(), "936pt".to_string())
        );
        assert_eq!(
            super::page_size_from_css("@page{size:595pt 842pt}"),
            ("595pt".to_string(), "842pt".to_string())
        );
        assert_eq!(
            super::page_size_from_css(".page{}"),
            ("612pt".to_string(), "936pt".to_string())
        );
    }

    use super::*;
    use serde_json::json;

    fn sample_draft() -> Form2551QDraft {
        let mut draft: Form2551QDraft = serde_json::from_value(json!({
            "id": null,
            "tin": "261708015000",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01",
            "taxable_year": 2026,
            "quarter": 1,
            "tax_period_basis": "calendar",
            "year_end_month": 12,
            "eopt_tier": null,
            "is_amended": false,
            "original_return_filed_and_paid_on_time": true,
            "number_of_attached_sheets": 0,
            "tax_relief": false,
            "tax_relief_specification": "",
            "item_13_election": "graduated",
            "rdo_code": "018",
            "taxpayer_name": "Frozen Html Fixture",
            "registered_address": "New Cabalan",
            "zip_code": "2200",
            "contact_number": "09170000000",
            "email": "tax@example.com",
            "schedule_1": [],
            "total_tax_due": 0.0,
            "creditable_tax_withheld": 0.0,
            "tax_paid_previous": 0.0,
            "other_tax_credit": 0.0,
            "other_tax_credit_description": "",
            "total_tax_credits": 0.0,
            "tax_payable": 0.0,
            "auto_compute_penalties": false,
            "surcharge": 0.0,
            "interest": 0.0,
            "compromise": 0.0,
            "total_penalties": 0.0,
            "total_amount_payable": 0.0,
            "overpayment_disposition": "none",
            "status": "Draft",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "submitted_at": null,
            "confirmed_at": null,
            "submission_filename": null,
            "receipt_id": null,
            "submission_attempts": 0,
            "next_retry_at": null,
            "last_error": null,
            "carried_forward_from": null,
            "payment_receipt_path": null
        }))
        .expect("fixture draft");
        draft.recompute(None);
        draft
    }

    fn named_values(html: &str, name: &str) -> Vec<String> {
        input_tags(html)
            .into_iter()
            .filter(|tag| tag.name == name)
            .map(|tag| attr(tag.tag, "value").unwrap_or("").to_string())
            .collect()
    }

    #[test]
    fn frozen_2551q_stamps_only_catalog_tin_names_from_fields_json() {
        let fields: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/forms/2551q-v2018/fields.json"))
                .unwrap();
        let allowed: std::collections::BTreeSet<String> = fields["fields"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["serialized_key"].as_str().map(str::to_string))
            .collect();

        let present: std::collections::BTreeSet<String> = input_tags(html_2551q())
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();

        assert_eq!(
            present,
            STAMPED_TIN_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        );
        for name in &present {
            assert!(allowed.contains(name), "{name} is not in fields.json");
        }
        assert!(html_2551q().contains("id=\"p1c20-s0\""));
        assert!(input_tags(html_2551q())
            .into_iter()
            .all(|tag| tag.name != "p1c20"));
    }

    #[test]
    fn frozen_0619e_stamps_only_catalog_tin_names() {
        let html = bundle("0619e-2018").expect("0619e bundle").html;
        let present: std::collections::BTreeSet<String> = input_tags(html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm0619E:"))
            .collect();
        assert_eq!(
            present,
            [
                "frm0619E:txtTIN1",
                "frm0619E:txtTIN2",
                "frm0619E:txtTIN3",
                "frm0619E:txtBranchCode",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
        let document = filled_document("0619e-2018", &BTreeMap::new()).unwrap();
        assert!(document.contains("<style>"));
        assert!(!document.contains("href=\"../base.css\""));
    }

    #[test]
    fn fill_by_name_distributes_tin_digits_across_comb_slots() {
        let html = concat!(
            "<div data-field-name=\"p1c20\">",
            "<input id=\"p1c20-s0\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"0\" maxlength=\"1\">",
            "<input id=\"p1c20-s1\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"1\" maxlength=\"1\">",
            "<input id=\"p1c20-s2\" name=\"frm2551Qv2018:txtTIN1\" data-slot-index=\"2\" maxlength=\"1\">",
            "</div>"
        );
        let mut fields = BTreeMap::new();
        fields.insert("frm2551Qv2018:txtTIN1".to_string(), "261".to_string());
        let filled = fill_by_name(html, &fields);
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN1"),
            ["2", "6", "1"]
        );
        assert!(filled.contains("id=\"p1c20-s0\""));
        assert!(filled.contains("data-field-name=\"p1c20\""));
    }

    #[test]
    fn fill_by_name_with_cells_uppercases_letter_combs_not_stamped_digits() {
        let html = concat!(
            r#"<input name="p1c36" data-slot-index="0" maxlength="1">"#,
            r#"<input name="p1c36" data-slot-index="1" maxlength="1">"#,
            r#"<input name="p1c36" data-slot-index="2" maxlength="1">"#,
            r#"<input name="frm1601c:txtTIN1" data-slot-index="0" maxlength="1">"#,
        );
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:txtTaxpayerName".to_string(), "Ab".to_string());
        fields.insert("frm1601c:txtTIN1".to_string(), "9".to_string());
        let mut cells = BTreeMap::new();
        cells.insert(
            "frm1601c:txtTaxpayerName".to_string(),
            vec!["p1c36".to_string()],
        );
        let filled = fill_by_name_with_cells(html, &fields, &cells, &BTreeMap::new());
        assert_eq!(comb_text(&filled, "p1c36"), "AB");
        assert!(filled.contains("data-writer-value=\"AB\""));
        assert!(!filled.contains("data-writer-value=\"Ab\""));
        assert_eq!(named_values(&filled, "frm1601c:txtTIN1"), ["9"]);
    }

    #[test]
    fn fill_2551q_sets_stamped_tin_names_from_the_writer_map() {
        let filled = fill_2551q(&sample_draft());
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN1"),
            ["2", "6", "1"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN2"),
            ["7", "0", "8"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtTIN3"),
            ["0", "1", "5"]
        );
        assert_eq!(
            named_values(&filled, "frm2551Qv2018:txtBranchCode"),
            ["0", "0", "0", "0", "0"]
        );
        let document = filled_2551q_document(&sample_draft());
        assert!(document.contains("data:image/png;base64,"));
        assert!(document.contains("<style>"));
        assert!(!document.contains("href=\"../base.css\""));
    }

    #[test]
    fn frozen_preview_bundles_stamp_only_catalog_frm_names() {
        let cases = [
            ("0619f-2018", "frm0619F:", 4usize),
            ("0605-1999", "frm0605:", 4),
            ("1601c-2018", "frm1601c:", 4),
            ("1701q-2018", "frm1701q:", 6),
            ("2550q-2024", "frm2550qv2024:", 3),
            ("1701-2018", "frm1701:", 6),
            ("1702rt-2018c", "frm1702RT:", 3),
            ("1702mx-2018c", "frm1702MX:", 3),
        ];
        for (slug, prefix, count) in cases {
            let html = bundle(slug).unwrap_or_else(|| panic!("{slug}")).html;
            let present: std::collections::BTreeSet<String> = input_tags(html)
                .into_iter()
                .map(|tag| tag.name.to_string())
                .filter(|name| name.starts_with(prefix))
                .collect();
            assert_eq!(present.len(), count, "{slug} stamped names");
            let document = filled_document(slug, &BTreeMap::new()).unwrap();
            assert!(document.contains("<style>"), "{slug}");
        }
    }

    fn comb_text(html: &str, name: &str) -> String {
        named_values(html, name).concat()
    }

    fn identity_map(
        name_key: &str,
        name: &str,
        address_key: &str,
        address: &str,
    ) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        fields.insert(name_key.to_string(), name.to_string());
        fields.insert(address_key.to_string(), address.to_string());
        fields
    }

    #[test]
    fn fill_by_name_without_writer_cells_leaves_unstamped_identity_blank() {
        let html = bundle("1601c-2018").expect("1601c").html;
        let mut fields = identity_map(
            "frm1601c:txtTaxpayerName",
            "NEXUS PAYROLL CORP",
            "frm1601c:txtAddress",
            "42 Banahaw Street",
        );
        fields.insert("txtEmail".to_string(), "juan@example.com".to_string());
        fields.insert("frm1601c:txtTax14".to_string(), "8888.88".to_string());
        let filled = fill_by_name(html, &fields);
        assert_eq!(comb_text(&filled, "p1c36"), "");
        assert_eq!(comb_text(&filled, "p1c38"), "");
        assert_eq!(comb_text(&filled, "p1c48"), "");
        assert_eq!(comb_text(&filled, "p1c56"), "");
        assert!(!filled.contains("NEXUS PAYROLL CORP"));
        assert!(!filled.contains("42 Banahaw Street"));
        assert!(!filled.contains("juan@example.com"));
        assert!(!filled.contains("8888.88"));
    }

    /// A 1601-C for the test TIN 111-111-114 (never a real TIN), every
    /// Part II item non-zero, Item 13 Yes and one Schedule I row.
    fn draft_1601c() -> bir_core::forms::form_1601c::Form1601CDraft {
        use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row};
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Andrea Mae Alicando Galang",
                "tin": {"segment1": "111", "segment2": "111", "segment3": "114", "branch": "00000"},
                "rdo_code": "039", "line_of_business": "Payroll",
                "registered_address": "42 Banahaw Street, Barangay Example, Quezon City",
                "zip_code": "1100", "phone": "09170000000", "email": "juan@example.com",
                "default_form_type": "1601Cv2018", "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Form1601CDraft::new_from_profile(&profile, 2025, 6);
        draft.auto_compute_penalties = false;
        draft.tax_relief = true;
        draft.tax_relief_specification = "International Tax Treaty".to_string();
        draft.tax_14_total_compensation = 250_000.0;
        draft.tax_15_statutory_minimum_wage = 1_500.0;
        draft.tax_16_holiday_pay = 1_600.0;
        draft.tax_17_13th_month_pay = 20_000.0;
        draft.tax_18_de_minimis = 5_000.0;
        draft.tax_19_sss_gsis = 12_500.0;
        draft.tax_20_other_name = "Rice subsidy".to_string();
        draft.tax_20_other_amount = 2_000.0;
        draft.tax_23_not_subject = 3_000.0;
        draft.tax_25_total_taxes_withheld = 18_750.0;
        draft.tax_29_other_remittances_name = "Prior credit".to_string();
        draft.tax_29_other_remittances_amount = 1_000.0;
        for (component, value) in [
            (bir_core::penalties::PenaltyComponent::Surcharge, 100.0),
            (bir_core::penalties::PenaltyComponent::Interest, 200.0),
            (bir_core::penalties::PenaltyComponent::Compromise, 300.0),
        ] {
            draft.set_penalty_override(component, Some(value));
        }
        draft.schedule_1 = vec![Form1601CSchedule1Row {
            previous_month: "04/2025".to_string(),
            date_paid: "05/09/2025".to_string(),
            drawee_bank_code_or_agency: "BANK1".to_string(),
            payment_number: "REF001".to_string(),
            tax_paid: 15_000.0,
            should_be_tax_due: 15_250.0,
            adjustment: 250.0,
        }];
        draft.compute();
        draft
    }

    /// The pesos and centavos a peso + 2-slot cents pair prints.
    fn printed_amount(html: &str, peso: &str, cent: &str) -> String {
        format!("{}.{}", comb_text(html, peso).trim(), comb_text(html, cent))
    }

    #[test]
    fn filled_document_1601c_fills_taxpayer_name_and_address() {
        let html = filled_document("1601c-2018", &draft_1601c().to_print_field_map()).unwrap();
        assert_eq!(
            comb_text(&html, "p1c36").trim(),
            "ANDREA MAE ALICANDO GALANG"
        );
        // Item 9 over both lines, broken at a space, not cut at 40 slots.
        assert_eq!(
            comb_text(&html, "p1c38").trim(),
            "42 BANAHAW STREET, BARANGAY EXAMPLE,"
        );
        assert_eq!(comb_text(&html, "p1c39").trim(), "QUEZON CITY");
        assert!(html.contains("data-writer-value=\"ANDREA MAE ALICANDO GALANG\""));
        assert!(!html.contains("Andrea Mae Alicando Galang"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm1601c:"))
            .collect();
        assert_eq!(stamped.len(), 4);
    }

    /// #113 acceptance: Part II items 14-36, Item 13/13A, Schedule I and the
    /// page 2 header (name, TIN and branch) print from a 1601-C draft; the
    /// ATC is the sheet's preprinted WW010.
    #[test]
    fn filled_document_1601c_prints_every_part_ii_item_and_the_page_two_header() {
        let draft = draft_1601c();
        let fields = draft.to_print_field_map();
        let html = filled_document("1601c-2018", &fields).unwrap();
        let money = bir_core::official_xml::official_amount;
        let items: [(u8, &str, &str, f64); 23] = [
            (14, "p1c56", "p1c58", draft.tax_14_total_compensation),
            (15, "p1c61", "p1c63", draft.tax_15_statutory_minimum_wage),
            (16, "p1c65", "p1c67", draft.tax_16_holiday_pay),
            (17, "p1c69", "p1c71", draft.tax_17_13th_month_pay),
            (18, "p1c73", "p1c75", draft.tax_18_de_minimis),
            (19, "p1c77", "p1c79", draft.tax_19_sss_gsis),
            (20, "p1c81", "p1c83", draft.tax_20_other_amount),
            (21, "p1c85", "p1c87", draft.tax_21_total_non_taxable),
            (22, "p1c89", "p1c91", draft.tax_22_total_taxable),
            (23, "p1c93", "p1c95", draft.tax_23_not_subject),
            (24, "p1c97", "p1c99", draft.tax_24_net_taxable),
            (25, "p1c101", "p1c103", draft.tax_25_total_taxes_withheld),
            (26, "p1c105", "p1c107", draft.tax_26_adjustment),
            (
                27,
                "p1c109",
                "p1c111",
                draft.tax_27_taxes_withheld_for_remittance,
            ),
            (28, "p1c113", "p1c115", 0.0),
            (
                29,
                "p1c117",
                "p1c119",
                draft.tax_29_other_remittances_amount,
            ),
            (30, "p1c121", "p1c123", draft.tax_30_total_tax_remittances),
            (31, "p1c125", "p1c127", draft.tax_31_tax_still_due),
            (32, "p1c129", "p1c131", draft.tax_32_surcharge),
            (33, "p1c133", "p1c135", draft.tax_33_interest),
            (34, "p1c137", "p1c139", draft.tax_34_compromise),
            (35, "p1c141", "p1c143", draft.tax_35_total_penalties),
            (36, "p1c145", "p1c147", draft.tax_36_total_amount_payable),
        ];
        for (item, peso, cent, value) in items {
            assert_eq!(
                printed_amount(&html, peso, cent),
                money(value).replace(',', ""),
                "item {item}"
            );
        }
        assert_ne!(draft.tax_36_total_amount_payable, 0.0);
        assert_eq!(named_values(&html, "p1c223"), ["RICE SUBSIDY"]);
        assert_eq!(named_values(&html, "p1c225"), ["PRIOR CREDIT"]);
        // Item 13 Yes, 13A printed whole (longer than its 17 slots).
        assert_eq!(named_values(&html, "p1c52"), ["X"]);
        assert_eq!(named_values(&html, "p1c53"), [""]);
        assert!(html.contains("readonly value=\"INTERNATIONAL TAX TREATY\""));
        // Item 5 ATC: preprinted on the sheet, so a reviewed not_joined key.
        assert_eq!(fields["frm1601c:txtATC"], "WW010");
        assert!(html.contains(">WW010</div>"));
        let cells = writer_cells("1601c-2018").unwrap();
        assert!(cells.not_joined["frm1601c:txtATC"].contains("preprinted WW010"));
        // Page 1 TIN (stamped) and the page 2 header: TIN, branch and name.
        assert_eq!(comb_text(&html, "frm1601c:txtTIN1"), "111");
        assert_eq!(comb_text(&html, "frm1601c:txtTIN3"), "114");
        assert_eq!(comb_text(&html, "frm1601c:txtBranchCode"), "00000");
        assert_eq!(comb_text(&html, "p2c5"), "11111111400000");
        assert_eq!(
            comb_text(&html, "p2c6").trim(),
            "ANDREA MAE ALICANDO GALANG"
        );
        // Schedule I row 1: both dates split over their combs; rows 2-3 blank.
        assert_eq!(comb_text(&html, "p2c15"), "04");
        assert_eq!(comb_text(&html, "p2c16"), "2025");
        assert_eq!(comb_text(&html, "p2c17"), "05");
        assert_eq!(comb_text(&html, "p2c18"), "09");
        assert_eq!(comb_text(&html, "p2c19"), "2025");
        assert_eq!(comb_text(&html, "p2c20"), "BANK1");
        assert_eq!(printed_amount(&html, "p2c22", "p2c24"), "15000.00");
        assert_eq!(printed_amount(&html, "p2c53", "p2c55"), "15250.00");
        assert_eq!(printed_amount(&html, "p2c57", "p2c59"), "250.00");
        assert_eq!(comb_text(&html, "p2c33").trim(), "");
        assert_eq!(printed_amount(&html, "p2c83", "p2c85"), "250.00");
        // Every value the map carries prints or has a reviewed reason.
        assert_eq!(
            unprinted_keys("1601c-2018", &fields).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn filled_document_keeps_mixed_case_email_but_uppercases_name() {
        let mut fields = identity_map(
            "frm1601c:txtTaxpayerName",
            "Andrea Mae Galang",
            "frm1601c:txtAddress",
            "42 Banahaw Street",
        );
        fields.insert(
            "txtEmail".to_string(),
            "Andrea.Galang@Example.com".to_string(),
        );
        let html = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c48"), "Andrea.Galang@Example.com");
        assert!(html.contains("data-writer-value=\"Andrea.Galang@Example.com\""));
        assert!(!html.contains("ANDREA.GALANG@EXAMPLE.COM"));
        assert_eq!(comb_text(&html, "p1c36"), "ANDREA MAE GALANG");
        assert!(!html.contains("Andrea Mae Galang"));
        assert!(is_email_key("txtEmail"));
        assert!(is_email_key("frm1700:txtI11Email"));
        assert!(!is_email_key("txtTaxpayerName"));
    }

    #[test]
    fn sheet_2000ot_prints_date_tin_and_whole_email() {
        let long = "Andrea.Mae.Galang.Accounting@Example.ph";
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), long.to_string());
        for (key, value) in [
            ("derived:transaction_mm", "06"),
            ("derived:transaction_dd", "15"),
            ("derived:transaction_yyyy", "2025"),
            ("derived:tin", "12345678800000"),
            ("derived:pg2_tin", "12345678800000"),
        ] {
            fields.insert(key.to_string(), value.to_string());
        }
        let html = filled_document("2000-ot-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c7"), "06");
        assert_eq!(comb_text(&html, "p1c8"), "15");
        assert_eq!(comb_text(&html, "p1c9"), "2025");
        assert_eq!(comb_text(&html, "p1c18"), "12345678800000");
        assert_eq!(comb_text(&html, "p2c5"), "12345678800000");
        // 39 characters over the 24-slot comb: printed whole, as typed.
        assert!(html.contains(&format!("readonly value=\"{long}\"")));
        assert_eq!(comb_text(&html, "p1c31"), "");
    }

    #[test]
    fn sheet_1707a_shares_the_contact_email_comb() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "derived:contact_email".to_string(),
            "09171234567 Me@Example.ph".to_string(),
        );
        let html = filled_document("1707a-2021", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c39"), "09171234567 Me@Example.ph");
    }

    #[test]
    fn shared_contact_email_comb_prints_long_values_whole_in_smaller_type() {
        for (slug, cell, slot) in [
            ("2200a-2020", "p1c34", 12),
            ("2200c-2018", "p1c36", 13),
            ("2200p-2020", "p1c34", 12),
            ("1707a-2021", "p1c39", 12),
        ] {
            let long = format!(
                "{:<slot$}{}",
                "09171234567", "andrea.mae.galang.accounting@example.com.ph"
            );
            assert!(long.chars().count() > 40);
            let mut fields = BTreeMap::new();
            fields.insert("derived:contact_email".to_string(), long.clone());
            let html = filled_document(slug, &fields).unwrap();
            assert!(
                html.contains(&format!("readonly value=\"{long}\" style=\"font-size:")),
                "{slug}: long line printed whole, in smaller type"
            );
            assert_eq!(comb_text(&html, cell), "", "{slug}: boxes left empty");
        }
    }

    #[test]
    fn shared_contact_email_comb_prints_short_values_in_the_boxes() {
        for (slug, cell) in [
            ("2200a-2020", "p1c34"),
            ("2200c-2018", "p1c36"),
            ("2200p-2020", "p1c34"),
            ("1707a-2021", "p1c39"),
        ] {
            let short = "0917123      a@b.ph";
            let mut fields = BTreeMap::new();
            fields.insert("derived:contact_email".to_string(), short.to_string());
            let html = filled_document(slug, &fields).unwrap();
            assert!(!html.contains("readonly value="), "{slug}: no overlay");
            assert_eq!(
                comb_text(&html, cell).replace(' ', ""),
                "0917123a@b.ph",
                "{slug}"
            );
        }
    }

    #[test]
    fn email_longer_than_its_comb_prints_whole() {
        let long = "Andrea.Mae.Galang.Accounting@Example-Company.com.ph";
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), long.to_string());
        let html = filled_document("2200an-2018", &fields).unwrap();
        assert!(html.contains(&format!("readonly value=\"{long}\"")));
        assert_eq!(comb_text(&html, "p1c35"), "");
        fields.insert("txtEmail".to_string(), "juan@example.com".to_string());
        let html = filled_document("2200an-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c35"), "juan@example.com");
        assert!(!html.contains("readonly value="));
    }

    #[test]
    fn filled_document_1601c_fills_email_and_splits_tax_money() {
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), "juan@example.com".to_string());
        fields.insert("frm1601c:txtTax14".to_string(), "8888.88".to_string());
        fields.insert("frm1601c:txtTax25".to_string(), "7777.77".to_string());
        let html = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c48"), "juan@example.com");
        assert!(html.contains("juan@example.com"));
        assert!(!html.contains("JUAN@EXAMPLE.COM"));
        assert_eq!(comb_text(&html, "p1c56"), "8888");
        assert_eq!(comb_text(&html, "p1c58"), "88");
        assert_eq!(comb_text(&html, "p1c101"), "7777");
        assert_eq!(comb_text(&html, "p1c103"), "77");
        assert!(!comb_text(&html, "p1c56").contains('.'));
        assert!(!comb_text(&html, "p1c101").contains('.'));
        assert!(html.contains("8888.88"));
        assert!(html.contains("7777.77"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm1601c:"))
            .collect();
        assert_eq!(stamped.len(), 4);
        assert!(!html.contains("name=\"frm1601c:txtTax14\""));
        assert!(!html.contains("name=\"txtEmail\""));
    }

    #[test]
    fn filled_document_1601c_fills_month_year_amended_and_withheld() {
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:txtMonth".to_string(), "08".to_string());
        fields.insert("frm1601c:txtYear".to_string(), "2026".to_string());
        fields.insert("frm1601c:AmendedRtn_1".to_string(), "false".to_string());
        fields.insert("frm1601c:AmendedRtn_2".to_string(), "true".to_string());
        fields.insert("frm1601c:TaxWithheld_1".to_string(), "false".to_string());
        fields.insert("frm1601c:TaxWithheld_2".to_string(), "true".to_string());
        let html = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c9"), "08");
        assert_eq!(comb_text(&html, "p1c10"), "2026");
        assert!(html.contains("2026"));
        assert_eq!(named_values(&html, "p1c22"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c20"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c21"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c23"), vec!["".to_string()]);
        assert!(!comb_text(&html, "p1c9").contains("true"));
        assert_ne!(named_values(&html, "p1c22"), vec!["true".to_string()]);
        assert!(!html.contains("name=\"frm1601c:txtMonth\""));
        assert!(!html.contains("name=\"frm1601c:txtYear\""));
        assert!(!html.contains("name=\"frm1601c:AmendedRtn_2\""));
        assert!(!html.contains("name=\"frm1601c:TaxWithheld_2\""));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm1601c:"))
            .collect();
        assert_eq!(stamped.len(), 4);

        fields.insert("frm1601c:AmendedRtn_1".to_string(), "true".to_string());
        fields.insert("frm1601c:AmendedRtn_2".to_string(), "false".to_string());
        fields.insert("frm1601c:TaxWithheld_1".to_string(), "true".to_string());
        fields.insert("frm1601c:TaxWithheld_2".to_string(), "false".to_string());
        let yes = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(named_values(&yes, "p1c21"), vec!["X".to_string()]);
        assert_eq!(named_values(&yes, "p1c23"), vec!["X".to_string()]);
        assert_eq!(named_values(&yes, "p1c22"), vec!["".to_string()]);
        assert_eq!(named_values(&yes, "p1c20"), vec!["".to_string()]);
    }

    #[test]
    fn filled_document_1601c_marks_item_11_category_of_withholding_agent() {
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:CatAgent_P".to_string(), "true".to_string());
        fields.insert("frm1601c:CatAgent_G".to_string(), "false".to_string());
        let private = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(named_values(&private, "p1c45"), vec!["X".to_string()]);
        assert_eq!(named_values(&private, "p1c46"), vec!["".to_string()]);
        assert!(!private.contains("name=\"frm1601c:CatAgent_P\""));

        fields.insert("frm1601c:CatAgent_P".to_string(), "false".to_string());
        fields.insert("frm1601c:CatAgent_G".to_string(), "true".to_string());
        let government = filled_document("1601c-2018", &fields).unwrap();
        assert_eq!(named_values(&government, "p1c45"), vec!["".to_string()]);
        assert_eq!(named_values(&government, "p1c46"), vec!["X".to_string()]);
        assert!(!government.contains("name=\"frm1601c:CatAgent_G\""));
    }

    #[test]
    fn filled_document_2551q_fills_taxpayer_name_and_address() {
        let draft = sample_draft();
        assert_eq!(draft.taxpayer_name, "Frozen Html Fixture");
        assert_eq!(draft.registered_address, "New Cabalan");
        assert_eq!(draft.email, "tax@example.com");
        let html = filled_2551q_document(&draft);
        assert_eq!(draft.taxpayer_name, "Frozen Html Fixture");
        assert_eq!(comb_text(&html, "p1c30"), "FROZEN HTML FIXTURE");
        assert_eq!(comb_text(&html, "p1c32"), "NEW CABALAN");
        assert_eq!(comb_text(&html, "p1c39"), "tax@example.com");
        assert_eq!(comb_text(&html, "p1c9"), "12");
        assert_eq!(comb_text(&html, "p1c10"), "2026");
        assert_eq!(named_values(&html, "p1c7"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c15"), vec!["".to_string()]);
        assert!(html.contains("FROZEN HTML FIXTURE"));
        assert!(html.contains("data-writer-value=\"FROZEN HTML FIXTURE\""));
        assert!(html.contains("NEW CABALAN"));
        assert!(html.contains("tax@example.com"));
        assert!(!html.contains("Frozen Html Fixture"));
        assert!(!html.contains("New Cabalan"));
        assert!(!html.contains("TAX@EXAMPLE.COM"));
        assert!(html.contains("2026"));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();
        assert_eq!(
            stamped,
            STAMPED_TIN_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        );
    }

    #[test]
    fn filled_document_2551q_fills_email_and_splits_tax_money() {
        let mut fields = BTreeMap::new();
        fields.insert("txtEmail".to_string(), "andrea@example.com".to_string());
        fields.insert("frm2551Qv2018:txt14".to_string(), "1643.10".to_string());
        fields.insert("txtATCAmt1".to_string(), "54770.00".to_string());
        let html = filled_document("2551q-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c39"), "andrea@example.com");
        assert!(html.contains("andrea@example.com"));
        assert!(!html.contains("ANDREA@EXAMPLE.COM"));
        assert_eq!(comb_text(&html, "p1c50"), "1643");
        assert_eq!(comb_text(&html, "p1c52"), "10");
        assert_eq!(comb_text(&html, "p2c15"), "54770");
        assert_eq!(comb_text(&html, "p2c17"), "00");
        assert!(!comb_text(&html, "p1c50").contains('.'));
        assert!(!comb_text(&html, "p2c15").contains('.'));
        assert!(html.contains("1643.10"));
        assert!(html.contains("54770.00"));
        assert!(!html.contains("name=\"frm2551Qv2018:txt14\""));
        assert!(!html.contains("name=\"txtATCAmt1\""));
        assert!(!html.contains("name=\"txtEmail\""));
    }

    #[test]
    fn filled_document_2551q_fills_year_ended_and_quarter() {
        let mut fields = BTreeMap::new();
        fields.insert("frm2551Qv2018:txtYear".to_string(), "2026".to_string());
        fields.insert("frm2551Qv2018:rtnMonth".to_string(), "12".to_string());
        fields.insert("frm2551Qv2018:qtr_1".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:qtr_2".to_string(), "true".to_string());
        fields.insert("frm2551Qv2018:qtr_3".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:qtr_4".to_string(), "false".to_string());
        fields.insert("frm2551Qv2018:forThe_1".to_string(), "true".to_string());
        fields.insert("frm2551Qv2018:forThe_2".to_string(), "false".to_string());
        let html = filled_document("2551q-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c10"), "2026");
        assert_eq!(comb_text(&html, "p1c9"), "12");
        assert!(html.contains("2026"));
        assert_eq!(named_values(&html, "p1c15"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c12"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c13"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c7"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c8"), vec!["".to_string()]);
        assert!(!comb_text(&html, "p1c10").contains("true"));
        assert_ne!(named_values(&html, "p1c15"), vec!["true".to_string()]);
        assert!(!html.contains("name=\"frm2551Qv2018:txtYear\""));
        assert!(!html.contains("name=\"frm2551Qv2018:qtr_2\""));
        let stamped: std::collections::BTreeSet<String> = input_tags(&html)
            .into_iter()
            .map(|tag| tag.name.to_string())
            .filter(|name| name.starts_with("frm2551Qv2018:"))
            .collect();
        assert_eq!(stamped.len(), 4);
    }

    #[test]
    fn filled_document_1700_fills_identity_whole_peso_combs_and_boxes() {
        use bir_core::forms::form_1700::{
            Form1700CivilStatus, Form1700Column, Form1700Draft, Form1700Employer,
            Form1700TaxpayerType,
        };
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Fixture, Sample Employee", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Employee", "registered_address":
                "123 Sample Street, Barangay Example, Quezon City Metro Manila",
                "zip_code": "1100", "phone": "09170000000", "email": "fixture@example.com",
                "default_form_type": "1700", "taxpayer_type": "Individual"
            }))
            .unwrap();
        let mut draft = Form1700Draft::new_from_profile(&profile, 2025);
        draft.taxpayer_type = Form1700TaxpayerType::Employee;
        draft.birth_date = "01/15/1980".into();
        draft.citizenship = "Filipino".into();
        draft.foreign_tax_credits = Some(false);
        draft.civil_status = Form1700CivilStatus::Single;
        draft.number_of_attachments = 2;
        draft.taxpayer = Form1700Column {
            non_taxable: 90_000.0,
            surcharge: 1_000.0,
            ..Form1700Column::default()
        };
        // The fixture enters its penalties by hand.
        bir_core::forms::queueable::QueueableForm::keep_entered_penalties(&mut draft);
        let employer = |name: &str, regular: f64, withheld: f64| Form1700Employer {
            for_spouse: false,
            name: name.into(),
            name2: String::new(),
            tin: "12345678800000".into(),
            regular,
            flat: 0.0,
            withheld,
        };
        draft.employers = vec![employer("Sample Employer Inc", 600_000.5, 40_000.0)];
        draft.recompute();
        let html = filled_document("1700-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "2025");
        assert_eq!(comb_text(&html, "p1c14"), "123");
        assert_eq!(comb_text(&html, "p1c21"), "039");
        assert!(html.contains("FIXTURE, SAMPLE EMPLOYEE"));
        assert_eq!(
            comb_text(&html, "p1c25"),
            "123 SAMPLE STREET, BARANGAY EXAMPLE,"
        );
        assert_eq!(comb_text(&html, "p1c26"), "QUEZON CITY METRO MANILA");
        assert_eq!(comb_text(&html, "p1c31"), "01");
        assert_eq!(comb_text(&html, "p1c33"), "1980");
        assert_eq!(comb_text(&html, "p1c118"), "02");
        // Whole pesos, right-aligned: Item 42A 600,001 in the 8-slot comb.
        let item_42a = named_values(&html, "p2c13");
        assert_eq!(item_42a.len(), 8);
        assert_eq!(item_42a.concat().trim(), "600001");
        assert_eq!(item_42a[7], "1");
        assert_eq!(comb_text(&html, "p2c16").trim(), "90000");
        // Item 31 Surcharge on the sheet; Item 32 Interest stays blank.
        assert_eq!(comb_text(&html, "p1c98").trim(), "1000");
        assert_eq!(comb_text(&html, "p1c101").trim(), "");
        // Column B of a non-joint return stays blank.
        assert_eq!(comb_text(&html, "p1c84").trim(), "");
        // Schedule 1 Item 1 and the 7-slot tax withheld comb.
        assert!(comb_text(&html, "p2c73").starts_with("SAMPLE EMPLOYER INC"));
        assert_eq!(comb_text(&html, "p2c76"), "12345678800000");
        assert_eq!(comb_text(&html, "p2c99").trim(), "40000");
        assert_eq!(named_values(&html, "p2c99").len(), 7);
        assert_eq!(comb_text(&html, "p2c5"), "123456788");
        assert_eq!(comb_text(&html, "p2c6"), "FIXTURE");
        assert_eq!(named_values(&html, "p1c9"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c8"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c42"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c47"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c55"), vec!["".to_string()]);
        assert!(!html.contains("name=\"frm1700:"));
        assert!(!html.contains("name=\"derived:"));
    }

    fn annual_fixture_profile(form: &str) -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "Annual Fixture Corp", "tin": {"segment1": "123",
            "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "Unit 1 Fixture Tower, 123 Sample Street, Barangay Example, Quezon City",
            "zip_code": "1100", "phone": "5551234", "email": "fixture@example.com",
            "default_form_type": form, "taxpayer_type": "Corporation"
        }))
        .unwrap()
    }

    #[test]
    fn filled_document_1604f_fills_identity_schedules_and_boxes() {
        use bir_core::forms::form_1604f::{Form1604fAgentCategory, Form1604fDraft};
        let mut draft = Form1604fDraft::new_from_profile(&annual_fixture_profile("1604F"), 2025);
        draft.agent_category = Form1604fAgentCategory::Private;
        draft.schedules[1][2].date = "10/30/2025".into();
        draft.schedules[1][2].reference = "tra-9".into();
        draft.schedules[1][2].taxes_withheld = 1_234.5;
        draft.schedules[1][2].penalties = 10.0;
        draft.recompute();
        let html = filled_document("1604f-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "2025");
        assert_eq!(comb_text(&html, "p1c12"), "123");
        assert_eq!(comb_text(&html, "p1c18"), "00000");
        assert_eq!(comb_text(&html, "p1c20"), "039");
        assert_eq!(comb_text(&html, "p1c22"), "ANNUAL FIXTURE CORP");
        assert_eq!(
            comb_text(&html, "p1c24"),
            "UNIT 1 FIXTURE TOWER, 123 SAMPLE STREET,"
        );
        assert!(comb_text(&html, "p1c25").starts_with(" BARANGAY EXAMPLE, QUEZON CITY"));
        assert!(comb_text(&html, "p1c25").ends_with("1100"));
        assert_eq!(comb_text(&html, "p2c5"), "12345678800000");
        assert_eq!(named_values(&html, "p1c96"), vec!["TRA-9".to_string()]);
        assert_eq!(named_values(&html, "p1c97"), vec!["1,234.50".to_string()]);
        assert_eq!(named_values(&html, "p1c99"), vec!["1,244.50".to_string()]);
        assert_eq!(named_values(&html, "p1c109"), vec!["1,244.50".to_string()]);
        assert_eq!(named_values(&html, "p1c9"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c8"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c27"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c38"), vec!["X".to_string()]);
        assert!(!html.contains("name=\"frm1604f:"));
        assert!(!html.contains("name=\"derived:"));
    }

    #[test]
    fn filled_document_1604c_fills_months_refund_items_and_boxes() {
        use bir_core::forms::form_1604c::{Form1604cAgentCategory, Form1604cDraft};
        let mut draft = Form1604cDraft::new_from_profile(&annual_fixture_profile("1604C"), 2025);
        draft.agent_category = Form1604cAgentCategory::Government;
        let row = &mut draft.months[11];
        row.date = "01/15/2026".into();
        row.bank = "Bank 12".into();
        row.reference = "TRA12".into();
        row.taxes_withheld = 5_000.0;
        row.adjustment = -250.25;
        draft.recompute();
        let html = filled_document("1604c-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c27"), "1100");
        assert_eq!(named_values(&html, "p1c114"), vec!["BANK 12".to_string()]);
        assert_eq!(named_values(&html, "p1c116"), vec!["5,000.00".to_string()]);
        assert_eq!(named_values(&html, "p1c169"), vec!["-250.25".to_string()]);
        assert_eq!(named_values(&html, "p1c171"), vec!["4,749.75".to_string()]);
        assert_eq!(named_values(&html, "p1c175"), vec!["4,749.75".to_string()]);
        // Item 11 "No": Items 11A-13 stay blank on paper.
        assert_eq!(comb_text(&html, "p1c40"), "");
        assert_eq!(comb_text(&html, "p1c46"), "");
        assert_eq!(named_values(&html, "p1c44"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c31"), vec!["X".to_string()]);

        draft.refunds_released = true;
        draft.refund_date = "03/15/2026".into();
        draft.overremittance = 4_321.56;
        draft.first_crediting_month = 2;
        draft.recompute();
        let html = filled_document("1604c-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c40"), "03");
        assert_eq!(comb_text(&html, "p1c41"), "15");
        assert_eq!(comb_text(&html, "p1c42"), "2026");
        assert!(comb_text(&html, "p1c46").ends_with("4321"));
        assert_eq!(comb_text(&html, "p1c48"), "56");
        assert_eq!(comb_text(&html, "p1c50"), "02");
        assert_eq!(named_values(&html, "p1c43"), vec!["X".to_string()]);
        assert!(!html.contains("name=\"derived:"));
    }

    #[test]
    fn filled_document_1604e_fills_both_schedules_and_page_two() {
        use bir_core::forms::form_1604e::{Form1604eAgentCategory, Form1604eDraft};
        let mut draft = Form1604eDraft::new_from_profile(&annual_fixture_profile("1604E"), 2025);
        draft.agent_category = Form1604eAgentCategory::Private;
        draft.top_withholding_agent = Some(true);
        draft.schedule1[3].date = "01/30/2026".into();
        draft.schedule1[3].bank = "Bank4".into();
        draft.schedule1[3].reference = "Tra4".into();
        draft.schedule1[3].taxes_withheld = 250_000.0;
        draft.schedule2[11].date = "01/15/2026".into();
        draft.schedule2[11].bank = "BankDec".into();
        draft.schedule2[11].reference = "TRA12".into();
        draft.schedule2[11].taxes_withheld = 333.33;
        draft.schedule2[11].penalties = 12.0;
        draft.recompute();
        let html = filled_document("1604e-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c22"), "ANNUAL FIXTURE CORP");
        assert_eq!(comb_text(&html, "p1c25"), " BARANGAY EXAMPLE, QUEZON CITY");
        assert_eq!(named_values(&html, "p1c71"), vec!["TRA4".to_string()]);
        assert_eq!(named_values(&html, "p1c72"), vec!["250,000.00".to_string()]);
        assert_eq!(named_values(&html, "p1c78"), vec!["250,000.00".to_string()]);
        assert_eq!(named_values(&html, "p1c170"), vec!["345.33".to_string()]);
        assert_eq!(named_values(&html, "p1c174"), vec!["345.33".to_string()]);
        assert_eq!(comb_text(&html, "p2c5"), "12345678800000");
        assert_eq!(comb_text(&html, "p2c6"), "ANNUAL FIXTURE CORP");
        assert_eq!(named_values(&html, "p1c30"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c32"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c33"), vec!["".to_string()]);
        assert!(!html.contains("name=\"frm1604e:"));
        assert!(!html.contains("name=\"derived:"));
    }

    #[test]
    fn filled_document_2553_fills_identity_rows_totals_and_boxes() {
        use bir_core::forms::form_2553::{Form2553Draft, FORM_2553_ATC_OPTIONS};
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Special Law Fixture Inc", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Gaming", "registered_address": "1 Fixture St",
                "zip_code": "1100", "phone": "5551234", "email": "fixture@example.com",
                "default_form_type": "2553", "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Form2553Draft::new_from_profile(&profile, 2025, 2);
        draft.set_atc(0, &FORM_2553_ATC_OPTIONS[0]).unwrap();
        draft.schedule[0].taxable_amount = 123_456.79;
        draft.creditable_tax_withheld = 50_000.0;
        draft.recompute();
        let html = filled_document("2553-1999", &draft.to_bir_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c18"), "123");
        assert_eq!(comb_text(&html, "p1c24"), "00000");
        assert_eq!(comb_text(&html, "p1c16"), "039");
        assert_eq!(comb_text(&html, "p1c4"), "12");
        assert_eq!(comb_text(&html, "p1c5"), "2025");
        assert!(html.contains("SPECIAL LAW FIXTURE INC"));
        assert!(html.contains("123,456.79"));
        assert!(html.contains("6,172.84"));
        assert_eq!(named_values(&html, "p1c2"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c3"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c8"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c12"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c34"), vec!["X".to_string()]);
        assert!(!html.contains("name=\"frm2553:"));
    }

    fn fixture_profile(form: &str) -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "Print Fixture Corp", "tin": {"segment1": "123",
            "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
            "line_of_business": "Brokerage", "registered_address": "1 Fixture St",
            "zip_code": "1100", "phone": "5551234", "email": "fixture@example.com",
            "default_form_type": form, "taxpayer_type": "Corporation"
        }))
        .unwrap()
    }

    #[test]
    fn filled_document_2552_fills_identity_money_schedules_and_boxes() {
        use bir_core::forms::form_2552::{Form2552Draft, Form2552ShareRow, Form2552Transaction};
        let mut draft = Form2552Draft::new_from_profile(&fixture_profile("2552"), 2025, 3, 14);
        draft.set_transaction(Form2552Transaction::LocalStockExchange);
        draft.schedule.push(Form2552ShareRow {
            date: "03/14/2025".into(),
            seller: "Seller One".into(),
            buyer: "Buyer One".into(),
            issuing_corporation: "Listed Corp".into(),
            number_of_shares: 1_000.0,
            tax_base: 1_234_567.89,
            ..Default::default()
        });
        draft.recompute();
        let html = filled_document("2552-2018", &draft.to_bir_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c6"), "03");
        assert_eq!(comb_text(&html, "p1c8"), "2025");
        assert_eq!(comb_text(&html, "p1c14"), "123");
        assert_eq!(comb_text(&html, "p1c20"), "00000");
        assert_eq!(comb_text(&html, "p1c22"), "039");
        assert!(
            html.contains("PRINT FIXTURE CORP") || comb_text(&html, "p1c24").starts_with("PRINT")
        );
        // Item 13: 7,407.00 splits into the peso and centavo combs.
        assert_eq!(comb_text(&html, "p1c51").trim(), "7407");
        assert_eq!(comb_text(&html, "p1c53"), "00");
        assert_eq!(named_values(&html, "p1c41"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["X".to_string()]);
        assert!(html.contains("1,234,567.89"));
        assert!(html.contains("SELLER ONE"));
        assert!(!html.contains("name=\"frm2552:"));
    }

    #[test]
    fn filled_document_2550m_fills_items_and_schedules() {
        use bir_core::forms::form_2550m::{Form2550MDraft, Form2550MWithholdingRow};
        let mut draft = Form2550MDraft::new_from_profile(&fixture_profile("2550M"), 2022, 6);
        draft.add_sales_atc("VB010").unwrap();
        draft.sales_schedule[0].amount = 100_000.0;
        draft.schedule_6.push(Form2550MWithholdingRow {
            period_covered: "06/30/2022".into(),
            withholding_agent: "Agent Corp".into(),
            income_payment: 1_000.0,
            total_withheld: 50.0,
            applied_current_month: 40.0,
        });
        draft.recompute();
        let html = filled_document("2550m-2007", &draft.to_bir_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c1"), "06");
        assert_eq!(comb_text(&html, "p1c2"), "2022");
        assert!(html.contains("PRINT FIXTURE CORP"));
        assert!(html.contains("100,000.00"));
        assert!(html.contains("12,000.00"));
        assert!(html.contains("VB010"));
        assert!(html.contains("AGENT CORP"));
        assert_eq!(named_values(&html, "p1c5"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c25"), vec!["X".to_string()]);
    }

    /// A Rust-side join that writer-cells.json already has (a later
    /// re-sync), by key or by cell, is skipped: the cell is filled once.
    #[test]
    fn a_sheet_join_the_writer_cells_already_have_is_not_added_twice() {
        let mut cells = WriterCells::empty();
        cells
            .joins
            .insert("frm2550m:txtTIN1".into(), vec!["p1c127".into()]);
        cells
            .joins
            .insert("frm2550m:txtOther".into(), vec!["p1c9".into()]);
        cells
            .not_joined
            .insert("frm2550m:txtTIN3".into(), "old reason".into());
        merge_sheet_joins(&mut cells, sheet_joins("2550m-2007"));
        assert_eq!(cells.joins["frm2550m:txtTIN1"], vec!["p1c127".to_string()]);
        assert!(
            !cells.joins.contains_key("frm2550m:txtTIN2"),
            "p1c9 is taken"
        );
        assert_eq!(cells.joins["frm2550m:txtTIN3"], vec!["p1c11".to_string()]);
        assert!(!cells.not_joined.contains_key("frm2550m:txtTIN3"));
        let targets: Vec<&str> = cells.targets().into_iter().map(|(_, c)| c).collect();
        let unique: BTreeSet<&str> = targets.iter().copied().collect();
        assert_eq!(targets.len(), unique.len(), "{targets:?}");
        // The fill of a sheet whose joins name a cell twice would overlap
        // its replacements; the merged print fills each once.
        let real = writer_cells("2550m-2007").unwrap();
        let targets: Vec<&str> = real.targets().into_iter().map(|(_, c)| c).collect();
        let unique: BTreeSet<&str> = targets.iter().copied().collect();
        assert_eq!(targets.len(), unique.len());
        let merged = merged_writer_cells_json("2550m-2007").unwrap();
        let joined: Vec<&str> = merged["joins"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|j| {
                j["writer_key"].as_str().unwrap().contains("txtTIN")
                    || j["writer_key"] == "frm2550m:txtBranchCode"
            })
            .map(|j| j["html_id"].as_str().unwrap())
            .collect();
        assert_eq!(joined, ["p1c127", "p1c9", "p1c11", "p1c13"]);
        assert!(!merged["not_joined"].to_string().contains("txtTIN1"));
        assert_eq!(
            sheet_join_combs("2550m-2007"),
            vec![
                ("frm2550m:txtTIN1".to_string(), 3, "4 TIN (1-3)".to_string()),
                ("frm2550m:txtTIN2".to_string(), 3, "4 TIN (4-6)".to_string()),
                ("frm2550m:txtTIN3".to_string(), 3, "4 TIN (7-9)".to_string()),
                (
                    "frm2550m:txtBranchCode".to_string(),
                    5,
                    "4 TIN branch".to_string()
                ),
            ]
        );
    }

    /// PR 4 (2550M manual-3): item 4 prints the TIN's three parts and the
    /// branch in the row's combs (p1c127, p1c9, p1c11, p1c13; frozen_html.rs
    /// SHEET_JOINS, since the frozen writer-cells.json leaves them out),
    /// and items 3, 5 and
    /// 10, rows of one-character boxes, print one character per box: never
    /// the whole value in each.
    #[test]
    fn filled_document_2550m_prints_the_tin_and_one_character_per_box() {
        use bir_core::forms::form_2550m::Form2550MDraft;
        let mut draft = Form2550MDraft::new_from_profile(&fixture_profile("2550M"), 2022, 6);
        draft.number_of_attached_sheets = 3;
        draft.recompute();
        let fields = draft.to_bir_field_map();
        let tin: String = draft.tin.chars().filter(char::is_ascii_digit).collect();
        let html = filled_document("2550m-2007", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c127"), tin[0..3]);
        assert_eq!(comb_text(&html, "p1c9"), tin[3..6]);
        assert_eq!(comb_text(&html, "p1c11"), tin[6..9]);
        assert_eq!(comb_text(&html, "p1c13"), "00000");
        // Each box of a split cell keeps only its own character; the
        // writer's whole value stays on the first box alone.
        let writer_values: Vec<Option<String>> = input_tags(&html)
            .into_iter()
            .filter(|tag| tag.name == "p1c130")
            .map(|tag| attr(tag.tag, "data-writer-value").map(str::to_string))
            .collect();
        assert_eq!(writer_values, vec![Some("039".to_string()), None, None]);
        let boxes = |name: &str| named_values(&html, name);
        let one_each = |value: &str| value.chars().map(String::from).collect::<Vec<_>>();
        assert_eq!(boxes("p1c130"), one_each(&fields["frm2550m:txtRDOCode"]));
        assert_eq!(fields["frm2550m:txtRDOCode"].len(), 3);
        assert_eq!(boxes("p1c21"), one_each(&fields["frm2550m:txtZipCode"]));
        assert_eq!(fields["frm2550m:txtZipCode"].len(), 4);
        // A one-digit count sits in the right-hand box.
        assert_eq!(fields["frm2550m:txtSheets"], "3");
        assert_eq!(boxes("p1c3"), vec![String::new(), "3".to_string()]);
        // A value longer than its boxes prints once, as a plain line across
        // the cell; its boxes stay empty, the writer's value on the first.
        // (Replaced in round 6, D4: it printed whole in the first box.)
        let mut long = fields.clone();
        long.insert("frm2550m:txtZipCode".into(), "123456".into());
        let html = filled_document("2550m-2007", &long).unwrap();
        assert_eq!(named_values(&html, "p1c21"), vec![String::new(); 4]);
        let writer_values: Vec<Option<&str>> = input_tags(&html)
            .into_iter()
            .filter(|tag| tag.name == "p1c21")
            .map(|tag| attr(tag.tag, "data-writer-value"))
            .collect();
        assert_eq!(writer_values, vec![Some("123456"), None, None, None]);
        let (text, size) = printed_cells(&html).remove("p1c21").unwrap();
        assert_eq!(text, "123456");
        // The line prints at 9 pt.
        assert_eq!(size, Some(9.0));
        // A branch other than 00000 prints over the sheet's 00000 (the
        // comb's inputs carry 0s as values; the html-frozen file is not
        // edited).
        let mut branch = draft.clone();
        branch.tin = "12345678800001".into();
        let html = filled_document("2550m-2007", &branch.to_bir_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c13"), "00001");
        assert_eq!(comb_text(&html, "p1c127"), "123");
    }

    #[test]
    fn filled_document_2200m_fills_identity_schedule_money_and_boxes() {
        use bir_core::forms::form_2200m::{Form2200MDraft, Form2200MPayment};
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Mineral Fixture Corp", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Mining", "registered_address": "1 Fixture St",
                "zip_code": "1100", "phone": "5551234", "email": "fixture@example.com",
                "default_form_type": "2200M", "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Form2200MDraft::new_from_profile(&profile, 2025, 3);
        draft.day = 14;
        draft.manner_of_payment = Form2200MPayment::Prepayment;
        let row = draft.row_mut(3).unwrap();
        row.place_of_removal = "Rizal".into();
        row.local_value_taxable = Some(100_000.0);
        row.local_tax_due = Some(4_000.0);
        draft.surcharge = 25.5;
        // The fixture enters its penalties by hand.
        bir_core::forms::queueable::QueueableForm::keep_entered_penalties(&mut draft);
        draft.recompute();
        draft.tax_deposit = draft.tax_still_due;
        draft.recompute();
        let html = filled_document("2200m-2018", &draft.to_bir_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "03");
        assert_eq!(comb_text(&html, "p1c6"), "14");
        assert_eq!(comb_text(&html, "p1c8"), "2025");
        assert_eq!(comb_text(&html, "p1c16"), "123");
        assert_eq!(comb_text(&html, "p1c22"), "00000");
        assert_eq!(comb_text(&html, "p1c24"), "039");
        assert!(html.contains("MINERAL FIXTURE CORP"));
        assert!(html.contains("100,000.00"));
        assert!(html.contains("RIZAL"));
        // Item 16 4,000.00 and Item 22 4,025.50 in peso + centavo combs.
        assert_eq!(comb_text(&html, "p1c65"), "4000");
        assert_eq!(comb_text(&html, "p1c67"), "00");
        assert_eq!(comb_text(&html, "p1c109"), "4025");
        assert_eq!(comb_text(&html, "p1c111"), "50");
        assert_eq!(named_values(&html, "p1c13"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c12"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c59"), vec!["X".to_string()]);
        assert!(!html.contains("name=\"frm2200M:"));
    }

    #[test]
    fn filled_document_2200a_fills_shared_combs_and_others_rows() {
        use bir_core::forms::form_2200a::{Form2200ADraft, Form2200APlace};
        let mut draft = Form2200ADraft::new_from_profile(&fixture_profile("2200A"), 2025, 6);
        draft.contact_number = "09171234567".into();
        draft.email = "Me@Example.ph".into();
        draft.place_of_production = Form2200APlace {
            region: "010000000".into(),
            province: "012800000".into(),
            city: "012801000".into(),
        };
        let row = draft.other_mut(1).expect("Others row 2");
        row.atc_digits = "123".into();
        row.description = "Fruit wine".into();
        row.bracket = "per liter".into();
        row.rate = "52.00".into();
        let html = filled_document("2200a-2020", &draft.to_print_field_map()).unwrap();
        // Items 8-9 share comb p1c34: contact from slot 0, email (as typed) from slot 12.
        assert_eq!(comb_text(&html, "p1c34"), "09171234567 Me@Example.ph");
        // Item 10 comb p1c39: region from slot 0, province from 10, city from 24.
        assert_eq!(
            comb_text(&html, "p1c39"),
            "010000000 012800000     012801000"
        );
        assert_eq!(comb_text(&html, "p1c44"), "");
        // Others row 2: ATC p3c103, description p3c104, bracket p3c105, rate p3c106
        // (the sheet prints in capitals; only the email stays as typed).
        for (cell, value) in [
            ("p3c103", "123"),
            ("p3c104", "FRUIT WINE"),
            ("p3c105", "PER LITER"),
            ("p3c106", "52.00"),
        ] {
            assert_eq!(named_values(&html, cell), vec![value.to_string()], "{cell}");
        }
    }

    #[test]
    fn filled_document_2200c_fills_item_6_and_schedule_1_rows() {
        use bir_core::forms::form_2200c::{Form2200CDraft, Form2200CManner, Form2200CRow};
        let mut draft = Form2200CDraft::new_from_profile(&fixture_profile("2200C"), 2025);
        draft.month = 6;
        draft.day = 30;
        draft.rdo_code = "039".into();
        // Part V is cleared until a manner of payment is picked (resetPartFive).
        draft.manner = Form2200CManner::ActualRemoval;
        // Row 1 is populated, rows 2-9 stay blank, row 10 is populated.
        let mut schedule = vec![Form2200CRow {
            exempt_procedures: 1.0,
            excisable_procedures: 3.0,
            non_excisable_procedures: 2.0,
            net_of_vat: 10_000.0,
            excisable_net: 50_010.0,
            non_excisable: 5_000.0,
            ..Default::default()
        }];
        schedule.resize(9, Form2200CRow::default());
        schedule.push(Form2200CRow {
            exempt_procedures: 4.0,
            net_of_vat: 2_000.0,
            ..Default::default()
        });
        draft.schedule = schedule;
        draft.recompute();
        draft.contact_number = "09171234567".into();
        draft.email = "Me@Example.ph".into();
        draft.place = bir_core::forms::excise_places::ExcisePlace {
            region: "010000000".into(),
            province: "012800000".into(),
            city: "012801000".into(),
        };
        let fields = draft.to_print_field_map();
        let html = filled_document("2200c-2018", &fields).unwrap();
        // Item 6 RDO comb (p1c26), 4 slots.
        assert_eq!(comb_text(&html, "p1c26"), "039");
        // Items 9-10 share comb p1c36: contact from slot 0, email (as typed) from slot 13.
        assert_eq!(comb_text(&html, "p1c36"), "09171234567  Me@Example.ph");
        // Schedule 1 row 1 runs p2c25 (A) to p2c34 (J), top to bottom.
        let row_1 = [
            "1.00",
            "3.00",
            "2.00",
            "10,000.00",
            "50,010.00",
            "0.00",
            "5,000.00",
            "2,501.00",
            "8,101.00",
            "75,612.00",
        ];
        for (offset, value) in row_1.iter().enumerate() {
            assert_eq!(
                named_values(&html, &format!("p2c{}", 25 + offset)),
                vec![value.to_string()],
                "schedule row 1 column {offset}"
            );
        }
        // Schedule 1 row 10 runs p2c115 to p2c124; its values come from the field map.
        assert_eq!(named_values(&html, "p2c115"), vec!["4.00".to_string()]);
        let columns = [
            "CPPexmpt",
            "CPPexcise",
            "CPPnonexcise",
            "GRnetVat",
            "GRexciseNet",
            "GRexciseExmpt",
            "GRnonexcise",
            "ACexcise",
            "ACvat",
            "ACtotAmountBill",
        ];
        for (offset, column) in columns.iter().enumerate() {
            let key = format!("frm2200C:{column}10");
            assert_eq!(
                named_values(&html, &format!("p2c{}", 115 + offset)),
                vec![fields[&key].clone()],
                "{key}"
            );
        }
        // A blank row (row 2) still prints the map's zero amounts, as the official XML does.
        assert_eq!(named_values(&html, "p2c35"), vec!["0.00".to_string()]);
        // TOTAL EXCISE TAX DUE (p2c126) carries the ACexciseTotal value.
        assert_eq!(
            named_values(&html, "p2c126"),
            vec![fields["frm2200C:ACexciseTotal"].clone()]
        );
        // Item 11 shares comb p1c41: region from slot 0, province from 10, city from 24.
        assert_eq!(
            comb_text(&html, "p1c41"),
            "010000000 012800000     012801000"
        );
    }

    fn withholding_profile(code: &str) -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "Withholding Fixture Inc", "tin": {"segment1": "123",
            "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
            "line_of_business": "Fixture", "registered_address": "1 Fixture St",
            "zip_code": "1100", "phone": "5551234", "email": "fixture@example.com",
            "default_form_type": code, "taxpayer_type": "Corporation"
        }))
        .unwrap()
    }

    #[test]
    fn filled_document_1606_fills_parties_atc_box_amounts_and_schedules() {
        use bir_core::forms::form_1606::{
            Form1606Draft, Form1606SellerType, Form1606TaxRate, Form1606Transaction,
        };
        let mut draft = Form1606Draft::new_from_profile(&withholding_profile("1606"), 2025, 1);
        draft.transaction_month = 6;
        draft.transaction_day = 15;
        draft.is_amended = true;
        draft.seller_tin = "98765432100000".into();
        draft.seller_name = "Seller Fixture".into();
        draft.seller_type = Some(Form1606SellerType::Corporation);
        draft.tct_number = "T-1".into();
        draft.set_transaction(Form1606Transaction::InstallmentSale);
        draft.gross_selling_price = 5_000_000.0;
        draft.installment_collected = 250_000.0;
        draft.tax_rate = Some(Form1606TaxRate::OnePointFive);
        draft.recompute();
        let html = filled_document("1606-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "06");
        assert_eq!(comb_text(&html, "p1c6"), "2025");
        assert_eq!(named_values(&html, "p1c14"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["".to_string()]);
        assert!(html.contains("WITHHOLDING FIXTURE INC"));
        assert_eq!(comb_text(&html, "p1c27"), "123-456-788-00000");
        assert_eq!(comb_text(&html, "p1c42"), "987-654-321-00000");
        // Corporate seller at 1.5%: the WC 555 box.
        assert_eq!(named_values(&html, "p1c57"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c53"), vec!["".to_string()]);
        assert_eq!(comb_text(&html, "p1c74"), "1");
        assert_eq!(comb_text(&html, "p1c76"), "5");
        assert_eq!(comb_text(&html, "p1c70"), "250000");
        assert_eq!(comb_text(&html, "p1c79"), "3750");
        assert_eq!(comb_text(&html, "p1c81"), "00");
        assert_eq!(named_values(&html, "p2c85"), vec!["X".to_string()]);
        assert_eq!(comb_text(&html, "p2c81"), "5000000");
        assert_eq!(comb_text(&html, "p2c5"), "12345678800000");
        assert_eq!(comb_text(&html, "p2c13"), "T-1");
        assert!(!html.contains("name=\"frm1606:"));
        assert!(!html.contains("name=\"derived:"));
    }

    #[test]
    fn print_carries_the_used_penalty_values() {
        // A late June 2025 1600-VT filed 13 October 2026: the print shows the
        // computed surcharge and interest and the compromise override, the
        // same values the XML and the queue fingerprint carry.
        use bir_core::forms::form_1600vt::{Form1600VtAgentCategory, Form1600VtDraft};
        use bir_core::forms::penalty_items::with_clock;
        use bir_core::forms::queueable::QueueableForm;
        use bir_core::penalties::PenaltyComponent;
        let mut vt = Form1600VtDraft::new_from_profile(&withholding_profile("1600VT"), 2025, 6);
        vt.set_agent_category(Form1600VtAgentCategory::Government);
        vt.set_taxes_withheld(true);
        vt.toggle_atc(2).unwrap();
        vt.atc_rows[0].tax_base = 100_000.0;
        vt.set_penalty_override(PenaltyComponent::Compromise, Some(500.0));
        let filed = serde_json::from_value(serde_json::json!("2026-10-13")).unwrap();
        with_clock(filed, || vt.recompute());
        let computed = vt.penalty.computed_penalties.clone().unwrap();
        assert!(computed.surcharge > 0.0 && computed.interest > 0.0);
        assert_eq!(vt.surcharge, computed.surcharge);
        assert_eq!(vt.interest, computed.interest);
        assert_eq!(vt.compromise, 500.0);

        let fields = vt.to_print_field_map();
        let amount = bir_core::official_xml::official_amount;
        assert_eq!(
            fields["frm1600VT:txtPg1P2I23Surcharge"],
            amount(computed.surcharge)
        );
        assert_eq!(
            fields["frm1600VT:txtPg1P2I24Interest"],
            amount(computed.interest)
        );
        assert_eq!(fields["frm1600VT:txtPg1P2I25Compromise"], amount(500.0));
        assert_eq!(fields, {
            let mut xml = vt.field_map();
            xml.extend(fields.iter().map(|(k, v)| (k.clone(), v.clone())));
            xml
        });
        let html = filled_document("1600-vt-2018", &fields).unwrap();
        let total_penalties = amount(vt.total_penalties);
        assert!(
            html.contains(&total_penalties) || html.contains(&total_penalties.replace(',', "")),
            "the print shows Item 26 = {total_penalties}"
        );
    }

    #[test]
    fn filled_document_1600vt_and_1600pt_fill_identity_part_ii_and_schedule() {
        use bir_core::forms::form_1600pt::{Form1600PtAgentCategory, Form1600PtDraft};
        use bir_core::forms::form_1600vt::{
            Form1600VtAgentCategory, Form1600VtDraft, Form1600VtScheduleRow,
        };
        let mut vt = Form1600VtDraft::new_from_profile(&withholding_profile("1600VT"), 2025, 6);
        // The fixture enters its penalties by hand (none), so the late
        // period's computed penalties do not move the printed totals.
        bir_core::forms::queueable::QueueableForm::keep_entered_penalties(&mut vt);
        vt.set_agent_category(Form1600VtAgentCategory::Government);
        vt.set_taxes_withheld(true);
        vt.toggle_atc(2).unwrap();
        vt.atc_rows[0].tax_base = 10_000.0;
        vt.schedule.push(Form1600VtScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Payee Fixture".into(),
            atc: "WV110".into(),
            income_payment: 10_000.0,
            tax_rate: 12.0,
            tax_withheld: 0.0,
        });
        vt.recompute();
        let html = filled_document("1600-vt-2018", &vt.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "06");
        assert_eq!(comb_text(&html, "p1c5"), "25");
        assert_eq!(comb_text(&html, "p1c16"), "123");
        assert_eq!(comb_text(&html, "p1c22"), "00000");
        assert_eq!(named_values(&html, "p1c35"), vec!["X".to_string()]);
        assert_eq!(comb_text(&html, "p1c50"), "WV110");
        assert_eq!(comb_text(&html, "p1c51"), "10000");
        assert_eq!(comb_text(&html, "p1c54"), "12");
        assert_eq!(comb_text(&html, "p1c56"), "1200");
        assert_eq!(comb_text(&html, "p1c136"), "1200");
        assert_eq!(comb_text(&html, "p2c5"), "12345678800000");
        assert_eq!(comb_text(&html, "p2c17"), "PAYEE FIXTURE");
        assert_eq!(comb_text(&html, "p2c21"), "1,200.00");
        assert!(!html.contains("name=\"frm1600VT:"));

        let mut pt = Form1600PtDraft::new_from_profile(&withholding_profile("1600PT"), 2025, 6);
        bir_core::forms::queueable::QueueableForm::keep_entered_penalties(&mut pt);
        pt.set_agent_category(Form1600PtAgentCategory::Government);
        pt.set_taxes_withheld(true);
        let wb200 = bir_core::forms::form_1600pt::form_1600pt_atc_index("WB200", "0.6").unwrap();
        pt.toggle_atc(wb200).unwrap();
        pt.atc_rows[0].tax_base = 10_000.0;
        pt.recompute();
        let html = filled_document("1600-pt-2018", &pt.to_print_field_map()).unwrap();
        // 1600-PT prints Private left of Government (1600-VT the other way).
        assert_eq!(named_values(&html, "p1c36"), vec!["X".to_string()]);
        assert_eq!(comb_text(&html, "p1c50"), "WB200");
        assert_eq!(comb_text(&html, "p1c54"), ".6");
        assert_eq!(comb_text(&html, "p1c56"), "60");
    }

    #[test]
    fn filled_document_1600wp_fills_part_ii_by_atc_row_and_schedule() {
        use bir_core::forms::form_1600wp::{
            Form1600WpAgentCategory, Form1600WpDraft, Form1600WpScheduleRow,
        };
        let mut draft = Form1600WpDraft::new_from_profile(&withholding_profile("1600WP"), 2025, 6);
        draft.day = 15;
        draft.set_taxes_withheld(true);
        draft.set_agent_category(Form1600WpAgentCategory::Private);
        draft.toggle_atc("WB194").unwrap();
        draft.atc_rows[0].tax_base = 2_500.0;
        draft.schedule.push(Form1600WpScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Winner Fixture".into(),
            atc_code: "WB194".into(),
            amount: 2_500.0,
            tax_withheld: 0.0,
        });
        draft.recompute();
        let html = filled_document("1600wp-2010", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c1"), "06152025");
        assert_eq!(comb_text(&html, "p1c4"), "2025");
        assert_eq!(named_values(&html, "p1c8"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c18"), vec!["X".to_string()]);
        // The only ticked ATC is WB194: its printed row, not WB191's.
        assert_eq!(comb_text(&html, "p1c47"), "2,500.00");
        assert_eq!(comb_text(&html, "p1c49"), "250.00");
        assert_eq!(comb_text(&html, "p1c32"), "");
        assert_eq!(comb_text(&html, "p1c51"), "250.00");
        assert_eq!(comb_text(&html, "p1c96"), "WINNER FIXTURE");
        assert_eq!(comb_text(&html, "p1c74"), "00000");
        assert!(!html.contains("name=\"frm1600WP:"));
    }

    #[test]
    fn filled_document_1702q_fills_identity_amounts_rates_and_boxes() {
        use bir_core::forms::form_1702q::{
            Form1702qDeduction, Form1702qDraft, Form1702qOtherCredit, Form1702qSchedule1Column,
        };
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Sample Dummy Corporation", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Consulting",
                "registered_address": "123 Sample Street, Barangay Example, Quezon City",
                "zip_code": "1100", "phone": "09170000000", "email": "sample@example.com",
                "default_form_type": "1702Q", "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Form1702qDraft::new_from_profile(&profile, 2025, 2);
        draft.atc_mcit = true;
        draft.atc = "IC010_25%".to_string();
        draft.deduction = Form1702qDeduction::Itemized;
        draft.special = Form1702qSchedule1Column {
            sales: 800_000.5,
            cost_of_sales: 300_000.0,
            rate: 10.0,
            ..Form1702qSchedule1Column::default()
        };
        draft.sales = 5_000_000.5;
        draft.cost_of_sales = 1_200_000.0;
        draft.mcit_gross_income_q2 = 3_800_000.5;
        draft.other_credits = vec![Form1702qOtherCredit {
            description: "Sample credit".to_string(),
            amount: 1_234.0,
        }];
        draft.recompute();
        let html = filled_document("1702q-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(named_values(&html, "p1c8"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c7"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c13"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c16"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c19"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c49"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c54"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c20"), vec!["1".to_string()]);
        assert_eq!(named_values(&html, "p1c23"), vec!["5".to_string()]);
        assert_eq!(named_values(&html, "p1c17"), vec!["IC 010".to_string()]);
        assert_eq!(comb_text(&html, "p1c26"), "123");
        assert_eq!(comb_text(&html, "p1c34"), "039");
        assert_eq!(comb_text(&html, "p1c37"), "SAMPLE DUMMY CORPORATION");
        assert_eq!(
            comb_text(&html, "p1c40"),
            "123 SAMPLE STREET, BARANGAY EXAMPLE, Q"
        );
        assert_eq!(comb_text(&html, "p1c41"), "UEZON CITY");
        assert_eq!(comb_text(&html, "p1c47"), "sample@example.com");
        assert_eq!(comb_text(&html, "p2c5"), "123456788");
        assert_eq!(comb_text(&html, "p2c55"), "     5000001");
        assert_eq!(comb_text(&html, "p2c57"), "     1200000");
        assert_eq!(comb_text(&html, "p2c13"), "      800001");
        assert_eq!(comb_text(&html, "p2c42"), "10");
        assert_eq!(comb_text(&html, "p2c44"), "0");
        assert_eq!(comb_text(&html, "p2c73"), "25");
        assert_eq!(comb_text(&html, "p2c110"), "SAMPLE CREDIT");
        assert_eq!(comb_text(&html, "p2c112"), "        1234");
        assert_eq!(
            comb_text(&html, "p1c57"),
            format!("{:>12}", (draft.income_tax_due.abs() + 0.5).floor() as u64)
        );
        assert!(!html.contains("name=\"frm1702q:"));
    }

    #[test]
    fn filled_document_1701a_fills_identity_whole_peso_combs_and_boxes() {
        use bir_core::forms::form_1701a::{
            Form1701AAtc, Form1701ACivilStatus, Form1701AColumn, Form1701ADraft, Form1701AFilerType,
        };
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Dummy, Sample Taxpayer", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Consulting",
                "registered_address": "123 Sample Street, Barangay Example, Quezon City",
                "zip_code": "1100", "phone": "09170000000", "email": "sample@example.com",
                "default_form_type": "1701A", "taxpayer_type": "Individual"
            }))
            .unwrap();
        let mut draft = Form1701ADraft::new_from_profile(&profile, 2025);
        draft.filer_type = Form1701AFilerType::SingleProprietor;
        draft.atc = Form1701AAtc::II012;
        draft.civil_status = Form1701ACivilStatus::Single;
        draft.birth_date = "01/15/1980".to_string();
        draft.citizenship = "Filipino".to_string();
        draft.other_income_41_description = "Interest income".to_string();
        draft.taxpayer = Form1701AColumn {
            sales: 1_234_567.89,
            sales_returns: 10_000.5,
            other_income_41: 5_000.55,
            gpp_share: 20_000.0,
            quarterly_payments: 30_000.0,
            surcharge: 1_000.0,
            ..Form1701AColumn::default()
        };
        draft.recompute();
        let html = filled_document("1701a-2018", &draft.to_print_field_map()).unwrap();
        // Header and identity.
        assert_eq!(comb_text(&html, "p1c4"), "12");
        assert_eq!(comb_text(&html, "p1c5"), "2025");
        assert_eq!(comb_text(&html, "p1c16"), "123");
        assert_eq!(comb_text(&html, "p1c20"), "788");
        assert_eq!(comb_text(&html, "p1c23"), "039");
        assert_eq!(comb_text(&html, "p1c32"), "DUMMY, SAMPLE TAXPAYER");
        assert_eq!(
            comb_text(&html, "p1c34"),
            "123 SAMPLE STREET, BARANGAY EXAMPLE,"
        );
        assert_eq!(comb_text(&html, "p1c35"), "QUEZON CITY");
        assert_eq!(comb_text(&html, "p1c40"), "01");
        assert_eq!(comb_text(&html, "p1c42"), "1980");
        assert_eq!(comb_text(&html, "p1c43"), "sample@example.com");
        assert_eq!(comb_text(&html, "p1c47"), "FILIPINO");
        assert_eq!(comb_text(&html, "p2c5"), "123456788");
        assert_eq!(comb_text(&html, "p2c6"), "DUMMY");
        assert!(html.contains("INTEREST INCOME"));
        // Whole pesos, right-aligned in the 8-slot comb; no decimal glyph.
        // An amount with centavos prints them, whole, in one rectangle over
        // the comb (before, 1,234,567.89 rounded to 1234568).
        assert_eq!(comb_text(&html, "p2c13"), "");
        assert_eq!(printed_cell(&html, "p2c13").0, "1,234,567.89");
        // Uniform sections (#477): the centavo-bearing amounts convert their
        // whole (section, column) groups, so the whole-peso amounts of those
        // columns print as lines too, comma-formatted like the rest.
        assert_eq!(comb_text(&html, "p2c48"), "");
        assert_eq!(printed_cell(&html, "p2c48").0, "94,448");
        assert_eq!(comb_text(&html, "p1c74"), "");
        assert_eq!(printed_cell(&html, "p1c74").0, "94,448");
        assert_eq!(comb_text(&html, "p1c89"), "");
        assert_eq!(printed_cell(&html, "p1c89").0, "1,000");
        assert!(!printed_cell(&html, "p1c74").0.contains('.'));
        // Zero and column B (not joint) stay blank.
        assert_eq!(comb_text(&html, "p2c52").trim(), "");
        assert_eq!(comb_text(&html, "p1c75").trim(), "");
        // X boxes.
        assert_eq!(named_values(&html, "p1c9"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c163"), vec!["".to_string()]);
        assert_eq!(named_values(&html, "p1c11"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c24"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c27"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c195"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c56"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p1c67"), vec!["X".to_string()]);
        assert_eq!(named_values(&html, "p2c126"), vec!["".to_string()]);
        assert!(!html.contains("name=\"frm1701A:"));
    }

    #[test]
    fn filled_document_1701ms_fills_both_columns_amounts_and_boxes() {
        use bir_core::forms::form_1701ms::{
            Form1701MsCivilStatus, Form1701MsColumn, Form1701MsDeduction, Form1701MsDraft,
            Form1701MsFiling, Form1701MsSource, Form1701MsSpouseInfo, Form1701MsTaxOption,
        };
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Sample Dummy Taxpayer", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Retail", "registered_address": "1 Fixture St",
                "zip_code": "1100", "phone": "09170000000", "email": "fixture@example.com",
                "default_form_type": "1701MS", "taxpayer_type": "Individual"
            }))
            .unwrap();
        let mut draft = Form1701MsDraft::new_from_profile(&profile, 2025);
        draft.civil_status = Form1701MsCivilStatus::Married;
        draft.filing = Form1701MsFiling::Jointly;
        draft.spouse_info = Form1701MsSpouseInfo {
            tin: "12345678800000".to_string(),
            rdo_code: "039".to_string(),
            name: "Sample Dummy Spouse".to_string(),
            email: "spouse@example.com".to_string(),
            contact_number: "09170000001".to_string(),
        };
        draft.taxpayer = Form1701MsColumn {
            source: Form1701MsSource::Mixed,
            tax_option: Form1701MsTaxOption::Graduated,
            deduction: Form1701MsDeduction::Osd,
            gross_compensation: 900_000.0,
            non_taxable_compensation: 90_000.0,
            sales: 1_500_000.0,
            sales_returns: 10_000.0,
            quarterly_payments: 50_000.0,
            cwt_2316: 80_000.0,
            ..Form1701MsColumn::default()
        };
        draft.spouse = Form1701MsColumn {
            source: Form1701MsSource::Compensation,
            gross_compensation: 600_000.0,
            non_taxable_compensation: 90_000.0,
            cwt_2316: 44_500.0,
            ..Form1701MsColumn::default()
        };
        draft.perjury_agreed = true;
        draft.recompute();
        let fields = draft.to_print_field_map();
        let html = filled_document("1701ms-2024", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "12");
        assert_eq!(comb_text(&html, "p1c5"), "2025");
        assert_eq!(comb_text(&html, "p1c23"), "123-456-788-00000");
        assert_eq!(comb_text(&html, "p1c29"), "SAMPLE DUMMY TAXPAYER");
        assert_eq!(comb_text(&html, "p1c30"), "SAMPLE DUMMY SPOUSE");
        assert_eq!(comb_text(&html, "p1c85"), "II013");
        assert_eq!(comb_text(&html, "p1c90"), "II011");
        // Whole-peso 9-slot combs, right-aligned digits without separators.
        assert_eq!(named_values(&html, "p2c13").len(), 9);
        assert_eq!(comb_text(&html, "p2c13"), "   900000");
        assert_eq!(comb_text(&html, "p2c14"), "   600000");
        assert_eq!(comb_text(&html, "p2c27"), "  1500000");
        assert_eq!(comb_text(&html, "p2c126"), "    80000");
        let aggregate = format!("{:>9}", draft.aggregate_amount_payable as i64);
        assert_eq!(comb_text(&html, "p1c141"), aggregate);
        assert_eq!(comb_text(&html, "p2c5"), "123456788");
        assert_eq!(comb_text(&html, "p2c6"), "SAMPLE DUMMY TAXPAYER");
        // Comma-formatted taxpayer name prints only the last name on page 2,
        // and a joint return with an empty spouse TIN prints blank (not "---00000").
        let mut c9_draft = draft.clone();
        c9_draft.taxpayer_name = "Santos, Juana".to_string();
        c9_draft.spouse_info.tin.clear();
        let c9_html = filled_document("1701ms-2024", &c9_draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&c9_html, "p2c6"), "SANTOS");
        assert_eq!(comb_text(&c9_html, "p1c24"), "");
        for x in [
            "p1c9", "p1c11", "p1c15", "p1c44", "p1c46", "p1c52", "p1c81", "p1c153",
        ] {
            assert_eq!(named_values(&html, x), vec!["X".to_string()], "{x}");
        }
        for blank in ["p1c8", "p1c10", "p1c43", "p1c79", "p1c144"] {
            assert_eq!(named_values(&html, blank), vec!["".to_string()], "{blank}");
        }
        assert!(!html.contains("name=\"frm1701MS:"));
        // A separate return prints no spouse column.
        draft.filing = Form1701MsFiling::Separately;
        draft.recompute();
        let html = filled_document("1701ms-2024", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c24"), "");
        assert_eq!(comb_text(&html, "p2c14"), "");
    }

    #[test]
    fn filled_document_1702ex_fills_identity_whole_pesos_schedules_and_boxes() {
        use bir_core::forms::form_1702ex::{
            Form1702ExAtc, Form1702ExDeduction, Form1702ExDraft, Form1702ExOverpayment,
            Form1702ExRow, Form1702ExSpecialRow,
        };
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Exempt Fixture Foundation Inc", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "Charity", "registered_address": "1 Fixture St",
                "zip_code": "1100", "phone": "09170000000", "email": "fixture@example.com",
                "default_form_type": "1702EX", "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Form1702ExDraft::new_from_profile(&profile, 2025);
        draft.atc = Form1702ExAtc::IC011;
        draft.deduction = Form1702ExDeduction::Itemized;
        draft.date_of_incorporation = "03/01/2010".to_string();
        draft.sales = 1_234_567.5;
        draft.ordinary_items[1] = 10_000.0;
        draft.other_deductions = vec![Form1702ExRow {
            description: "Sample expense".to_string(),
            amount: 5_000.0,
        }];
        draft.special_rows = vec![Form1702ExSpecialRow {
            description: "Sample deduction".to_string(),
            legal_basis: "RA 0000".to_string(),
            amount: 15_000.0,
        }];
        draft.cwt_q4 = 1_000.0;
        draft.overpayment = Form1702ExOverpayment::Refund;
        draft.recompute();
        let fields = draft.to_print_field_map();
        let cells = writer_cells("1702ex-2018").unwrap();
        for key in cells
            .joins
            .keys()
            .chain(cells.xbox_joins.iter().map(|join| &join.writer_key))
        {
            assert!(fields.contains_key(key), "print map lacks {key}");
        }
        let html = filled_document("1702ex-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c25"), "123");
        assert_eq!(comb_text(&html, "p1c33"), "039");
        assert_eq!(comb_text(&html, "p2c5"), "123456788");
        assert_eq!(comb_text(&html, "p3c5"), "123456788");
        assert!(comb_text(&html, "p1c36").starts_with("EXEMPT FIXTURE FOUNDATION INC"));
        // 29 characters past the page-2 header's 24 boxes: the whole name
        // as plain type over the boxes (was cut at 24); at the boxes' 9 pt
        // it fits, so it is not shrunk.
        assert_eq!(comb_text(&html, "p2c6"), "");
        let (header, size) = printed_cell(&html, "p2c6");
        assert_eq!(header, "EXEMPT FIXTURE FOUNDATION INC");
        assert_eq!(size, Some(9.0));
        assert_eq!(comb_text(&html, "p1c50"), "2010");
        assert_eq!(named_values(&html, "p1c19"), vec!["1".to_string()]);
        assert_eq!(named_values(&html, "p1c20"), vec!["2".to_string()]);
        assert_eq!(named_values(&html, "p1c22"), vec!["5".to_string()]);
        // Whole pesos, right-aligned in the 12-slot comb; centavos the
        // upload carries print, whole, in one rectangle (before, rounded).
        assert_eq!(comb_text(&html, "p2c9"), "");
        assert_eq!(printed_cell(&html, "p2c9").0, "1,234,567.50");
        assert_eq!(comb_text(&html, "p3c12"), "       10000");
        assert_eq!(comb_text(&html, "p3c52"), "SAMPLE EXPENSE");
        assert_eq!(comb_text(&html, "p3c54"), "        5000");
        assert_eq!(comb_text(&html, "p3c58"), "");
        // Past its 15 boxes the description prints whole (was cut at 15).
        assert_eq!(comb_text(&html, "p3c81"), "");
        assert_eq!(printed_cell(&html, "p3c81").0, "SAMPLE DEDUCTION");
        assert_eq!(comb_text(&html, "p3c87"), "");
        // Part IV is one section on page 2 (no Schedule band inside it):
        // the centavo conversion of p2c9 widens to the whole column (#477).
        assert_eq!(comb_text(&html, "p2c47"), "");
        assert_eq!(printed_cell(&html, "p2c47").0, "1,000");
        // 1702EX shows a negative with a minus (formatCurrency).
        assert_eq!(comb_text(&html, "p1c83"), "       -1000");
        for checked in ["p1c7", "p1c13", "p1c15", "p1c11", "p1c56", "p1c86"] {
            assert_eq!(
                named_values(&html, checked),
                vec!["X".to_string()],
                "{checked}"
            );
        }
        for unchecked in ["p1c8", "p1c12", "p1c18", "p1c57", "p1c87", "p1c88"] {
            assert_eq!(
                named_values(&html, unchecked),
                vec!["".to_string()],
                "{unchecked}"
            );
        }
        // Items 18 and 41 (freeze preprints 000), signatory and Part III stay blank.
        assert_eq!(comb_text(&html, "p1c75"), "");
        assert_eq!(comb_text(&html, "p2c38"), "");
        assert_eq!(comb_text(&html, "p1c116"), "");
        assert!(!html.contains("name=\"frm1702EX:"));
    }

    #[test]
    fn writer_cells_target_catalog_cell_ids_not_stamps() {
        // Sheets that print amounts in one text cell, not peso + cent combs, say false.
        // 2553 (1999) prints each amount in one text cell and 1702Q (2018) in
        // one 12-slot comb, not peso + cent combs.
        for (slug, split_money) in [
            ("1601c-2018", true),
            ("2551q-2018", true),
            ("2553-1999", false),
            ("2552-2018", true),
            ("2550m-2007", false),
            ("2200m-2018", true),
            ("1606-2018", true),
            ("1600-vt-2018", true),
            ("1600-pt-2018", true),
            ("1600wp-2010", false),
            ("1702q-2018", false),
            // 1700 (2018) prints whole pesos in one comb per column.
            ("1700-2018", false),
            // 1701A (2018) prints whole pesos in one 8-slot comb per column.
            ("1701a-2018", false),
            ("1701ms-2024", false),
            ("1702ex-2018", false),
            // 1604F/1604E print amounts in single text cells; 1604C splits Item 12.
            ("1604f-2018", false),
            ("1604c-2018", true),
            ("1604e-2018", false),
        ] {
            let html = bundle(slug).unwrap().html;
            let cells = writer_cells(slug).unwrap();
            assert!(!cells.joins.is_empty(), "{slug}");
            assert_eq!(
                !cells.money_joins.is_empty(),
                split_money,
                "{slug} money_joins"
            );
            assert!(!cells.xbox_joins.is_empty(), "{slug} xbox_joins");
            let present: std::collections::BTreeSet<&str> =
                input_tags(html).into_iter().map(|tag| tag.name).collect();
            for (key, html_id) in cells.targets() {
                assert!(
                    present.contains(html_id) || html.contains(&format!("id=\"{html_id}\"")),
                    "{slug} {key} -> {html_id}"
                );
                assert!(
                    !key.starts_with('p'),
                    "{slug} writer_key looks like a cell id: {key}"
                );
                assert!(
                    html_id.starts_with('p'),
                    "{slug} html_id must be a cell id: {html_id}"
                );
                assert!(!present.contains(key), "{slug} {key} is stamped");
            }
            for join in &cells.money_joins {
                for pair in &join.cells {
                    assert_ne!(Some(&pair.peso_html_id), pair.cent_html_id.as_ref());
                }
            }
        }
    }

    /// Every input of plain (`data-field-kind="text"`) cell `cell` of
    /// `html`, in order: its whole room (pt), its class list and its tag.
    fn plain_inputs<'a>(html: &'a str, cell: &str) -> Vec<(f64, &'a str, &'a str)> {
        let Some(width) = plain_cell_width(html, cell) else {
            return Vec::new();
        };
        let (open, _) = cell_tag(html, cell).unwrap();
        let inner = &html[open..open + html[open..].find("</div>").unwrap()];
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = inner[from..].find("<input") {
            let at = from + at;
            let end = at + inner[at..].find('>').unwrap();
            let tag = &inner[at..end];
            from = end;
            if tag.contains(" data-overflow-of=\"") {
                continue;
            }
            if let Some(classes) = attr(tag, "class") {
                out.push((input_room(width, tag), classes, tag));
            }
        }
        out
    }

    /// Whether a plain overflow line `text` at `size` pt sets within a
    /// comb `width` pt wide on sheet `slug`: measured in the sheet's face
    /// with its word- and letter-spacing, after the line's 1 pt padding.
    fn line_fits(slug: &str, text: &str, size: f64, width: f64) -> bool {
        let sheet = SheetType::of(bundle(slug).unwrap().css);
        1.0 + sheet.width(sheet.letter, text, size) <= width
    }

    /// Every frozen sheet slug with a writer-cells file.
    fn sheet_slugs() -> Vec<String> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
        let mut slugs: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry
                    .path()
                    .join("writer-cells.json")
                    .exists()
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        slugs.sort();
        slugs
    }

    /// Upper-case text of exactly `len` characters: an address's words
    /// with single spaces, never ending in a space.
    fn words_of(len: usize) -> String {
        const WORDS: [&str; 12] = [
            "UNIT",
            "1203",
            "TWELFTH",
            "FLOOR",
            "CURVE",
            "BUILDING",
            "BONIFACIO",
            "GLOBAL",
            "CITY",
            "TAGUIG",
            "METRO",
            "MANILA",
        ];
        let mut text = String::new();
        for word in WORDS.iter().cycle() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(word);
            if text.len() >= len {
                break;
            }
        }
        text.truncate(len);
        if text.ends_with(' ') {
            text.pop();
            text.push('X');
        }
        assert_eq!(text.chars().count(), len);
        text
    }

    /// An independent reading of a printed cell, straight from the
    /// document's tags: the slot inputs' values, and the plain overlay
    /// (`data-overflow-of`) with its type size.
    fn read_cell(document: &str, cell: &str) -> (String, Option<(String, f64)>) {
        let boxes: String = named_values(document, cell).concat();
        let needle = format!("data-overflow-of=\"{cell}\"");
        let overlay = document.find(&needle).map(|at| {
            let start = document[..at].rfind("<input").unwrap();
            let tag = &document[start..at];
            let value = tag
                .split("value=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let size: f64 = tag
                .split("font-size:")
                .nth(1)
                .unwrap()
                .split("pt")
                .next()
                .unwrap()
                .parse()
                .unwrap();
            (value.replace("&amp;", "&"), size)
        });
        (boxes, overlay)
    }

    /// Rules page question 4 (decided 2026-10-17) on every sheet's item
    /// printed over stacked comb lines: text that fits the boxes prints
    /// exactly as before; past them, the whole text prints as plain type
    /// across every line, broken between words; too long for that at the
    /// boxes' 9 pt, it shrinks until it fits. Nothing is lost.
    #[test]
    fn a_stacked_item_past_its_boxes_prints_whole_as_plain_type_on_every_sheet() {
        let mut items = 0;
        for slug in sheet_slugs() {
            let loaded = bundle(&slug).unwrap();
            let cells = writer_cells(&slug).unwrap();
            for group in line_groups(loaded.html, &cells) {
                items += 1;
                let slots: Vec<usize> = group.iter().map(|l| l.slots).collect();
                let capacity: usize = slots.iter().sum();
                let fill = |values: &[String]| {
                    let mut fields = BTreeMap::new();
                    for (line, value) in group.iter().zip(values) {
                        fields.insert(line.key.clone(), value.clone());
                    }
                    let document = filled_document(&slug, &fields).unwrap();
                    let before = fill_by_name_with_cells(
                        loaded.html,
                        &fields,
                        &cells.joins,
                        &BTreeMap::new(),
                    );
                    (document, before)
                };
                // Fits: each line within its boxes prints exactly as the
                // fill without this rule prints it.
                let fitting: Vec<String> = slots.iter().map(|n| words_of((*n).min(12))).collect();
                let (document, before) = fill(&fitting);
                assert!(!document.contains("data-overflow-of"), "{slug}");
                for (line, value) in group.iter().zip(&fitting) {
                    assert_eq!(
                        read_cell(&document, &line.cell),
                        (value.clone(), None),
                        "{slug}"
                    );
                    assert_eq!(
                        read_cell(&before, &line.cell),
                        (value.clone(), None),
                        "{slug}"
                    );
                }
                // Past the boxes: plain type, every line used, nothing lost,
                // each line inside its cell. Fed whole into the first line,
                // and as a print map cuts it (the first line at its boxes).
                let plain_at_9: usize = group
                    .iter()
                    .map(|l| ((l.width - 2.0) / (9.0 * 0.58)) as usize)
                    .sum();
                for (len, shrunk) in [
                    (90.max(capacity + 10), false),
                    (200.max(plain_at_9 + 20), true),
                ] {
                    let text = words_of(len);
                    let cut = text[..=slots[0]].rfind(' ').unwrap();
                    let mut first_cut = vec![text[..cut].to_string(), text[cut + 1..].to_string()];
                    first_cut.resize(group.len(), String::new());
                    let mut whole = vec![text.clone()];
                    whole.resize(group.len(), String::new());
                    for values in [whole, first_cut] {
                        let (document, _) = fill(&values);
                        let mut parts = Vec::new();
                        let mut sizes = Vec::new();
                        for line in &group {
                            let (boxes, overlay) = read_cell(&document, &line.cell);
                            assert_eq!(boxes.trim(), "", "{slug} {}: boxes empty", line.cell);
                            let (part, size) = overlay
                                .unwrap_or_else(|| panic!("{slug} {}: a plain line", line.cell));
                            assert!(!part.is_empty(), "{slug} {}: every line used", line.cell);
                            for subline in part.replace("&#10;", "\n").lines() {
                                assert!(
                                    line_fits(&slug, subline, size, line.width),
                                    "{slug} {}: {subline:?} at {size}pt fits {}pt",
                                    line.cell,
                                    line.width
                                );
                            }
                            assert_eq!(
                                printed_cell(&document, &line.cell),
                                (part.replace("&#10;", "\n"), Some(size))
                            );
                            parts.push(part.replace("&#10;", " "));
                            sizes.push(size);
                        }
                        if parts.iter().any(|p| p.ends_with("...")) {
                            assert!(
                                sizes.iter().all(|s| *s >= super::PLAIN_TEXT_MIN_PT),
                                "{slug}: floored at 7.5pt {sizes:?}"
                            );
                        } else {
                            assert_eq!(parts.join(" "), text, "{slug}: the whole text, in order");
                        }
                        if shrunk {
                            assert!(
                                sizes.iter().all(|s| *s >= super::PLAIN_TEXT_MIN_PT)
                                    && (sizes.iter().any(|s| *s < 9.0)
                                        || parts.iter().any(|p| p.contains('\n'))),
                                "{slug}: {len} characters shrink or wrap ({sizes:?})"
                            );
                        } else {
                            assert_eq!(
                                sizes[0], 9.0,
                                "{slug}: {len} characters at the boxes' size"
                            );
                        }
                    }
                }
            }
        }
        assert!(items >= 40, "{items} stacked items");
    }

    /// A single-line name past its boxes prints whole as plain type across
    /// them, shrunk to fit; within them it prints in the boxes as before.
    #[test]
    fn a_single_line_name_past_its_boxes_prints_whole_on_every_sheet() {
        let mut names = 0;
        for slug in sheet_slugs() {
            let loaded = bundle(&slug).unwrap();
            let cells = writer_cells(&slug).unwrap();
            let stacked: BTreeSet<String> = line_groups(loaded.html, &cells)
                .into_iter()
                .flatten()
                .map(|line| line.cell)
                .collect();
            for (key, ids) in &cells.joins {
                let cell = &ids[0];
                let caption = cells.captions.get(cell).map(|c| c.to_ascii_lowercase());
                if !caption.is_some_and(|c| c.contains("name") && !c.contains("tin"))
                    || stacked.contains(cell)
                {
                    continue;
                }
                let Some((slots, width)) = comb_cell(loaded.html, cell) else {
                    continue;
                };
                names += 1;
                let print = |value: &str| {
                    let fields = BTreeMap::from([(key.clone(), value.to_string())]);
                    filled_document(&slug, &fields).unwrap()
                };
                let short = words_of(slots.min(14));
                let document = print(&short);
                assert_eq!(
                    read_cell(&document, cell),
                    (short.clone(), None),
                    "{slug} {key}"
                );
                let plain_at_9 = ((width - 2.0) / (9.0 * 0.58)) as usize;
                for (len, shrunk) in [(slots + 5, false), (200.max(plain_at_9 + 20), true)] {
                    let text = words_of(len);
                    let document = print(&text);
                    let (boxes, overlay) = read_cell(&document, cell);
                    assert_eq!(boxes.trim(), "", "{slug} {key}");
                    let (part, size) = overlay.unwrap_or_else(|| panic!("{slug} {key}: plain"));
                    assert!(
                        size >= super::PLAIN_TEXT_MIN_PT,
                        "{slug} {key}: floor 7.5pt ({size})"
                    );
                    let normalized = part.replace("&#10;", "\n");
                    if normalized.ends_with("...") {
                        assert!(
                            document.contains("data-overflow-warning=\"text-truncated\""),
                            "{slug} {key}: truncated value carries warning for additional sheet"
                        );
                    } else {
                        assert_eq!(normalized.replace('\n', " "), text, "{slug} {key}: whole");
                    }
                    for subline in normalized.lines() {
                        assert!(
                            line_fits(&slug, subline, size, width),
                            "{slug} {key}: {subline:?} at {size}pt fit {width}pt"
                        );
                    }
                    if shrunk {
                        assert!(
                            size < 9.0 || normalized.contains('\n'),
                            "{slug} {key}: {size}"
                        );
                    }
                }
            }
        }
        assert!(names >= 40, "{names} single-line names");
    }

    /// A fitting two-line address prints byte for byte as before the
    /// overflow rule: 0619E's whole address item (its two comb lines, from
    /// `<div id="p1c41"` to the next cell) equals what the fill printed at
    /// `pr0b-push`, captured then into `fixtures/0619e-fitting-address.html`.
    #[test]
    fn a_fitting_address_prints_byte_for_byte_as_before() {
        let fields = BTreeMap::from([
            (
                "frm0619E:txtAddress".to_string(),
                "1 Example Street".to_string(),
            ),
            (
                "frm0619E:txtAddress2".to_string(),
                "Quezon City".to_string(),
            ),
        ]);
        let document = filled_document("0619e-2018", &fields).unwrap();
        let start = document.find("<div id=\"p1c41\"").unwrap();
        let end = document.find("<div id=\"p1c43\"").unwrap();
        assert_eq!(
            &document[start..end],
            include_str!("fixtures/0619e-fitting-address.html"),
            "the address item as it printed before"
        );
        assert!(!document.contains("data-overflow-of"));
    }

    /// One long address, pinned: 200 characters over 0619E's two address
    /// lines print as plain type in one size; far longer, the type keeps
    /// shrinking so the text still fits its cells (no floor). The size was
    /// 8.61 pt by the per-character estimate, 8.56 pt measured to the
    /// nearest 0.01 pt, and 8.55 pt now that the measured size is rounded
    /// down to 0.01 pt (rounding never adds width; [`floor_size`]).
    #[test]
    fn a_long_address_prints_at_a_pinned_size() {
        let text = words_of(200);
        let fields = BTreeMap::from([("frm0619E:txtAddress".to_string(), text.clone())]);
        let document = filled_document("0619e-2018", &fields).unwrap();
        let (first, size) = printed_cell(&document, "p1c41");
        let (second, again) = printed_cell(&document, "p1c42");
        assert_eq!(format!("{first} {second}"), text);
        assert_eq!(size, again);
        assert_eq!(size, Some(8.55));
        assert!(document.contains(
            "font-size:8.55pt;text-align:left;padding-left:1pt;z-index:1\" data-overflow-of=\"p1c41\""
        ));
        // 600 characters: floored at 7.5 pt (Rule 3), fits inside each cell,
        // and carries the truncation warning for the additional sheet.
        let text = words_of(600);
        let fields = BTreeMap::from([("frm0619E:txtAddress".to_string(), text.clone())]);
        let document = filled_document("0619e-2018", &fields).unwrap();
        for cell in ["p1c41", "p1c42"] {
            let (part, size) = printed_cell(&document, cell);
            let size = size.unwrap();
            let width = comb_cell(&document, cell).unwrap().1;
            assert!(size >= super::PLAIN_TEXT_MIN_PT, "{cell}: {size}");
            for subline in part.lines() {
                assert!(
                    line_fits("0619e-2018", subline, size, width),
                    "{cell}: {} characters at {size}pt fit {width}pt",
                    subline.chars().count()
                );
            }
        }
        assert!(document.contains("data-overflow-warning=\"text-truncated\""));
        let sheet = SheetType::plain();
        assert_eq!(
            sheet.fit(sheet.letter, 998.0, "UNIT 1", PLAIN_MAX_PT),
            PLAIN_MAX_PT
        );
    }

    /// The bug as found: a 200-character 0619E registered address
    /// overflowing item 10. The inputs are synthetic: the address typed
    /// into the two writer keys as 111 characters (the trailing space
    /// included) and 89, the shape the review's screenshot showed; the print
    /// rejoins the lines either way. (BIR's character-127 cut of the escaped
    /// address does not apply: this address's cut lands inside a %20, so
    /// form_0619e_official refuses it.) By the old estimate, line 2 printed
    /// at 8.61 pt and measured 453.1 pt in Arimo, past its 446.5 pt cell,
    /// cut at the 10A ZIP Code caption. Measured, each line's text at its
    /// size fits its cell less the input's 1 pt left padding, so line 2 ends
    /// before the caption; the ZIP prints in its own boxes and the whole
    /// address prints.
    #[test]
    fn a_200_character_0619e_address_keeps_line_2_off_the_zip_box() {
        let first = "UNIT 4512 TOWER B THE RESIDENCES AT GREENBELT 123 ESPERANZA STREET COR. \
                     MAKATI AVENUE BRGY. SAN LORENZO MAKATI";
        let second = "CITY METRO MANILA PHILIPPINES NEAR AYALA TRIANGLE GARDENS ACROSS \
                      GREENBELT 5 MALL GATE 12";
        assert_eq!(first.chars().count() + 1 + second.chars().count(), 200);
        // The estimate's size, and what it measured: past the cell.
        assert!((fit_size(446.52, 89) - 8.61).abs() < 0.005);
        assert!(arimo_ems(second) * 8.61 > 446.52);
        let fields = BTreeMap::from([
            ("frm0619E:txtAddress".to_string(), format!("{first} ")),
            ("frm0619E:txtAddress2".to_string(), second.to_string()),
            ("frm0619E:txtZipCode".to_string(), "1226".to_string()),
        ]);
        let document = filled_document("0619e-2018", &fields).unwrap();
        let mut parts = Vec::new();
        for cell in ["p1c41", "p1c42"] {
            let (part, size) = printed_cell(&document, cell);
            let size = size.unwrap_or_else(|| panic!("{cell}: plain type"));
            let width = comb_cell(&document, cell).unwrap().1;
            assert!(
                1.0 + arimo_ems(&part) * size <= width,
                "{cell}: {part:?} at {size}pt measures {}pt in {width}pt",
                arimo_ems(&part) * size
            );
            parts.push(part);
        }
        assert_eq!(parts.join(" "), format!("{first} {second}"));
        // Line 2's cell ends where the 10A ZIP Code caption starts.
        assert!(document.contains("<div id=\"p1c42\" class=\"c f\""));
        assert!(document.contains("left:17.62pt;top:240.62pt;width:446.52pt"));
        assert!(document.contains("<div id=\"p1c43\" class=\"c\" data-cell-kind=\"label\" data-row=\"11\" data-col=\"34\" style=\"left:464.14pt"));
        assert_eq!(printed_cell(&document, "p1c44"), ("1226".to_string(), None));
    }

    /// Per cell, on every sheet (P4-7): every text join filled with a
    /// 200-character text, then with a 90-character one, then the review's
    /// sweep probe (the 200-character address below in each join whose
    /// printed caption contains "address", "name", "email", "employer" or
    /// "business", in any case: "6A Seller name", "Donee A name"). Each
    /// plain overflow line over a comb, measured in the sheet's face with
    /// its word- and letter-spacing at its printed size, sets within its
    /// cell after the line's 1 pt padding; each plain box's value, at the
    /// size it prints (its class's, or the smaller inline size a value
    /// longer than the box takes), sets within the box less its input's
    /// inset. The inputs are synthetic. On 0619E and 0619F the address
    /// typed across its two keys (111 + 89 characters) prints the very same
    /// document as typed whole, so it is checked once.
    #[test]
    fn every_sheet_fits_every_plain_line_in_its_cell() {
        let first = "UNIT 4512 TOWER B THE RESIDENCES AT GREENBELT 123 ESPERANZA STREET COR. \
                     MAKATI AVENUE BRGY. SAN LORENZO MAKATI";
        let second = "CITY METRO MANILA PHILIPPINES NEAR AYALA TRIANGLE GARDENS ACROSS \
                      GREENBELT 5 MALL GATE 12";
        let whole = format!("{first} {second}");
        for (slug, prefix) in [("0619e-2018", "frm0619E"), ("0619f-2018", "frm0619F")] {
            let address = |one: String, two: &str| {
                BTreeMap::from([
                    (format!("{prefix}:txtAddress"), one),
                    (format!("{prefix}:txtAddress2"), two.to_string()),
                    (format!("{prefix}:txtZipCode"), "1226".to_string()),
                ])
            };
            assert_eq!(
                filled_document(slug, &address(format!("{first} "), second)).unwrap(),
                filled_document(slug, &address(whole.clone(), "")).unwrap(),
                "{slug}: typed across the two lines, it prints as typed whole"
            );
        }
        let (mut lines, mut boxes, mut swept) = (0, 0, 0);
        for slug in sheet_slugs() {
            let loaded = bundle(&slug).unwrap();
            let sheet = SheetType::of(loaded.css);
            let cells = writer_cells(&slug).unwrap();
            let every = |text: &str| -> BTreeMap<String, String> {
                cells
                    .joins
                    .keys()
                    .map(|key| (key.clone(), text.to_string()))
                    .collect()
            };
            let sweep: BTreeMap<String, String> = cells
                .joins
                .iter()
                .filter(|(_, ids)| {
                    ids.first()
                        .and_then(|id| cells.captions.get(id))
                        .is_some_and(|caption| {
                            let caption = caption.to_lowercase();
                            ["address", "name", "email", "employer", "business"]
                                .iter()
                                .any(|word| caption.contains(word))
                        })
                })
                .map(|(key, _)| (key.clone(), whole.clone()))
                .collect();
            swept += sweep.len();
            for (case, fields) in [
                ("200", every(&words_of(200))),
                ("90", every(&words_of(90))),
                ("sweep", sweep),
            ] {
                let document = filled_document(&slug, &fields).unwrap();
                for (cell, (text, size)) in printed_cells(&document) {
                    let Some(size) = size else {
                        continue;
                    };
                    // Over a Rule 6 full-width band, a comb, or a split cell's boxes.
                    let rule6_width = document
                        .find(&format!("data-rule6-of=\"{cell}\""))
                        .and_then(|at| {
                            let start = document[..at].rfind("<div ")?;
                            let end = at + document[at..].find('>')? + 1;
                            super::style_pt_prop(&document[start..end], "width")
                        });
                    let width = rule6_width
                        .or_else(|| comb_cell(&document, &cell).map(|(_, width)| width))
                        .or_else(|| plain_cell_width(&document, &cell))
                        .unwrap();
                    for subline in text.lines() {
                        let measured = sheet.width(sheet.letter, subline, size);
                        assert!(
                            1.0 + measured <= width,
                            "{slug} {case} {cell}: {subline:?} at {size}pt measures {measured}pt in {width}pt"
                        );
                    }
                    lines += 1;
                }
                for ids in cells.joins.values() {
                    for cell in ids {
                        // Every input of the cell, each by its own room.
                        for (room, classes, input) in plain_inputs(&document, cell) {
                            let text = attr(input, "value").unwrap_or("").replace("&amp;", "&");
                            if text.is_empty() {
                                continue;
                            }
                            let style = attr(input, "style").unwrap_or("");
                            let shrunk: Option<f64> = style
                                .find("font-size:")
                                .and_then(|at| style[at + "font-size:".len()..].split("pt").next())
                                .and_then(|size| size.parse().ok());
                            let letter = sheet.letter_for(classes);
                            if let Some(size) = shrunk {
                                // Shrunk (or two-line wrapped / ellipsized): every printed
                                // line sets within the room less 1 pt; a room under 2 pt,
                                // within the room.
                                let bound = if room >= 2.0 { room - 1.0 } else { room };
                                for line in text.split("&#10;") {
                                    let measured = sheet.width(letter, line, size);
                                    assert!(
                                        measured <= bound,
                                        "{slug} {case} {cell}: {line:?} shrunk to {size}pt measures {measured}pt in a {room}pt box"
                                    );
                                }
                            } else {
                                // Unshrunk: it fit at its class size.
                                let size = sheet
                                    .size_for(classes)
                                    .unwrap_or_else(|| panic!("{slug} {cell}: a type size"));
                                let measured = sheet.width(letter, &text, size);
                                assert!(
                                    measured <= room + FIT_EPSILON_PT,
                                    "{slug} {case} {cell}: {text:?} unshrunk at {size}pt measures {measured}pt in a {room}pt box"
                                );
                            }
                            boxes += 1;
                        }
                    }
                }
            }
        }
        assert!(lines > 500, "{lines} plain lines checked");
        assert!(boxes > 500, "{boxes} plain boxes checked");
        assert!(swept > 100, "{swept} joins swept");
    }

    /// Every sheet filled three ways: each text join's key holding a
    /// 200-character text, a 90-character one and a 12-character one (past
    /// the boxes of most combs, past a line, and fitting), hashed together.
    fn overflow_print_hash(slug: &str) -> String {
        use sha2::{Digest, Sha256};
        let cells = writer_cells(slug).unwrap();
        let mut hasher = Sha256::new();
        for len in [200, 90, 12] {
            let fields: BTreeMap<String, String> = cells
                .joins
                .keys()
                .map(|key| (key.clone(), words_of(len)))
                .collect();
            hasher.update(filled_document(slug, &fields).unwrap().as_bytes());
            hasher.update([0u8]);
        }
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The sheets whose long-text print changed when every sheet's overflow
    /// type was measured (P4-7), against `fixtures/overflow-goldens-estimate.json`
    /// (what every sheet printed at f7b7f475, by the 0.58 em estimate, written
    /// once and never rewritten). 0619E and 0619F, measured first, are among
    /// them. Every sheet changed: the probe fills every text join with 200
    /// characters, so each sheet has some plain line to size.
    const CHANGED_BY_MEASURING: &[&str] = &[
        "0605-1999",
        "0619e-2018",
        "0619f-2018",
        "1600-pt-2018",
        "1600-vt-2018",
        "1600wp-2010",
        "1601-fq-2020",
        "1601c-2018",
        "1601eq-2019",
        "1602q-2019",
        "1603q-2018",
        "1604c-2018",
        "1604e-2018",
        "1604f-2018",
        "1606-2018",
        "1700-2018",
        "1701-2018",
        "1701a-2018",
        "1701ms-2024",
        "1701q-2018",
        "1702ex-2018",
        "1702mx-2018c",
        "1702q-2018",
        "1702rt-2018c",
        "1706-2018",
        "1707-2021",
        "1707a-2021",
        "1800-2018",
        "1801-2018",
        "2000-dst-2018",
        "2000-ot-2018",
        "2200a-2020",
        "2200an-2018",
        "2200c-2018",
        "2200m-2018",
        "2200p-2020",
        "2200s-2018",
        "2200t-2020",
        "2550m-2007",
        "2550q-2024",
        "2551q-2018",
        "2552-2018",
        "2553-1999",
    ];

    /// The documents `long_text_prints_as_pinned_on_every_sheet` hashes for
    /// `OVERFLOW_PROBE_SHEETS` (comma-separated slugs), written to
    /// `OVERFLOW_PROBE_OUT/<slug>.html`, one tag per line and the inlined
    /// fonts and images left out, for a readable diff between two commits
    /// (docs/proof/field-rules-overflow/).
    #[test]
    #[ignore = "writes the overflow probe's documents for a diff"]
    fn write_overflow_probe_documents() {
        let (Some(out), Some(sheets)) = (
            std::env::var_os("OVERFLOW_PROBE_OUT"),
            std::env::var("OVERFLOW_PROBE_SHEETS").ok(),
        ) else {
            return;
        };
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for slug in sheets.split(',') {
            let cells = writer_cells(slug).unwrap();
            let mut text = String::new();
            for len in [200, 90, 12] {
                let fields: BTreeMap<String, String> = cells
                    .joins
                    .keys()
                    .map(|key| (key.clone(), words_of(len)))
                    .collect();
                let document = filled_document(slug, &fields).unwrap();
                text.push_str(&format!(
                    "<!-- probe: every text join filled with {len} characters -->\n"
                ));
                let mut rest = document.as_str();
                while let Some(at) = rest.find("data:") {
                    let end = rest[at..]
                        .find(['"', ')', '\''])
                        .map_or(rest.len(), |e| at + e);
                    text.push_str(&rest[..at]);
                    text.push_str("data:(inlined)");
                    rest = &rest[end..];
                }
                text.push_str(rest);
                text.push('\n');
            }
            let lines = text.replace("><", ">\n<");
            std::fs::write(out.join(format!("{slug}.html")), lines).unwrap();
        }
    }

    /// Every overflow line a fill writes (`data-overflow-of="X"`) lies inside
    /// the drawn cell `<div id="X">` itself (or the Rule 6 band printing X's
    /// value, `data-rule6-of="X"`, which must sit directly below X:
    /// [`bands_not_below_their_cell`]), never inside a `<template>` (the
    /// band rows a sheet keeps for cloning, which are never drawn) nor in
    /// another element. Otherwise a long value prints nowhere while
    /// `printed_cells`, which reads the text, still counts it, and a golden
    /// would record the vanished value.
    #[test]
    fn every_overflow_line_lies_in_its_drawn_cell() {
        let mut misplaced = Vec::new();
        let mut bands = 0;
        for slug in sheet_slugs() {
            let cells = writer_cells(&slug).unwrap();
            for len in [200, 90, 12] {
                let fields: BTreeMap<String, String> = cells
                    .joins
                    .keys()
                    .map(|key| (key.clone(), words_of(len)))
                    .collect();
                let doc = filled_document(&slug, &fields).unwrap();
                for (cell, place) in overflow_line_places(&doc) {
                    if place.as_deref() != Some(cell.as_str()) {
                        misplaced.push(format!("{slug} ({len}): {cell} in {place:?}"));
                    }
                }
                bands += doc.matches("data-rule6-of=\"").count();
                misplaced.extend(
                    bands_not_below_their_cell(&doc)
                        .into_iter()
                        .map(|w| format!("{slug} ({len}): {w}")),
                );
                misplaced.extend(
                    bands_sideways_off_their_cell(&doc)
                        .into_iter()
                        .map(|w| format!("{slug} ({len}): {w}")),
                );
                misplaced.extend(
                    bands_covering_other_value_cells(&doc)
                        .into_iter()
                        .map(|w| format!("{slug} ({len}): {w}")),
                );
                for band in drawn_divs(&doc).iter().filter(|d| d.band_of.is_some()) {
                    if band.width + 0.01 < super::RULE6_BAND_FLOOR_PT {
                        misplaced.push(format!(
                            "{slug} ({len}): {} band narrower than the floor: {:.2} pt",
                            band.band_of.as_deref().unwrap(),
                            band.width
                        ));
                    }
                }
            }
        }
        // Every Rule 6 band the long fills print (14 sheets), counted so a
        // change that drops them cannot pass by checking none.
        assert_eq!(bands, 62, "Rule 6 bands printed by the long fills");
        misplaced.dedup();
        assert!(
            misplaced.is_empty(),
            "overflow lines outside their drawn cell ({}):\n{}",
            misplaced.len(),
            misplaced.join("\n")
        );
    }

    /// For every overflow input of `doc`: the cell it names and the real
    /// `id` of the `<div>` it lies in (`None` inside a `<template>` or
    /// outside any `<div>`).
    fn overflow_line_places(doc: &str) -> Vec<(String, Option<String>)> {
        let mut out = Vec::new();
        let mut divs: Vec<Option<String>> = Vec::new();
        let mut templates = 0usize;
        let mut from = 0;
        while let Some(rel) = doc[from..].find('<') {
            let at = from + rel;
            let Some(end_rel) = doc[at..].find('>') else {
                break;
            };
            let tag = &doc[at..=at + end_rel];
            from = at + end_rel + 1;
            if tag.starts_with("<template") {
                templates += 1;
            } else if tag.starts_with("</template") {
                templates = templates.saturating_sub(1);
            } else if tag.starts_with("<div") {
                // The `id` attribute itself, not `data-cell-id` or any
                // other name ending in `id`; or the cell a drawn Rule 6
                // band lies over (`data-rule6-of`, the band that prints a
                // long value at its cell's place).
                let id = tag
                    .find(" id=\"")
                    .map(|at| &tag[at + 5..])
                    .and_then(|rest| rest.split('"').next())
                    .or_else(|| attr(tag, "data-rule6-of"))
                    .map(str::to_string);
                divs.push(id);
            } else if tag.starts_with("</div") {
                divs.pop();
            } else if tag.starts_with("<input") {
                if let Some(cell) = attr(tag, "data-overflow-of") {
                    let place = if templates > 0 {
                        None
                    } else {
                        divs.last().cloned().flatten()
                    };
                    out.push((cell.to_string(), place));
                }
            }
        }
        out
    }

    /// A drawn `<div>` of a filled document (outside every `<template>`)
    /// with a position: its page's id, its own `id`, the cell it is the
    /// Rule 6 band of, whether it is a value cell (`field` or `mixed`), and
    /// its box in points within the page.
    #[derive(Clone, Debug)]
    struct DrawnDiv {
        page: Option<String>,
        id: Option<String>,
        band_of: Option<String>,
        value_cell: bool,
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    }

    fn drawn_divs(doc: &str) -> Vec<DrawnDiv> {
        let mut out = Vec::new();
        let mut pages: Vec<Option<String>> = Vec::new();
        let mut templates = 0usize;
        let mut from = 0;
        while let Some(rel) = doc[from..].find('<') {
            let at = from + rel;
            let Some(end_rel) = doc[at..].find('>') else {
                break;
            };
            let tag = &doc[at..=at + end_rel];
            from = at + end_rel + 1;
            if tag.starts_with("<template") {
                templates += 1;
            } else if tag.starts_with("</template") {
                templates = templates.saturating_sub(1);
            } else if tag.starts_with("<div") {
                let id = tag
                    .find(" id=\"")
                    .map(|at| &tag[at + 5..])
                    .and_then(|rest| rest.split('"').next())
                    .map(str::to_string);
                let page = match &id {
                    Some(id) if id.starts_with("page-") => Some(id.clone()),
                    _ => pages.last().cloned().flatten(),
                };
                pages.push(page.clone());
                let geometry = ["left", "top", "width", "height"].map(|p| style_pt_prop(tag, p));
                if let (0, [Some(left), Some(top), Some(width), Some(height)]) =
                    (templates, geometry)
                {
                    let kind = attr(tag, "data-cell-kind");
                    out.push(DrawnDiv {
                        page,
                        id,
                        band_of: attr(tag, "data-rule6-of").map(str::to_string),
                        value_cell: matches!(kind, Some("field" | "mixed")),
                        left,
                        top,
                        width,
                        height,
                    });
                }
            } else if tag.starts_with("</div") {
                pages.pop();
            }
        }
        out
    }

    /// How far a Rule 6 band's top may lie below its cell's bottom (pt):
    /// the band starts 0.5 pt under its cut, and the cut moves down to the
    /// sheet band's edge when that is within 6 pt ([`apply_rule6_specify_bands`]).
    /// Measured over every sheet: 0.50 to 5.33 pt. A row is 11 pt or more.
    const BAND_GAP_BELOW: f64 = 6.5;
    /// How far a Rule 6 band's top may lie above its cell's bottom (pt):
    /// none, it never overlaps its cell.
    const BAND_GAP_ABOVE: f64 = 0.0;

    /// Every Rule 6 band of `doc` not directly below its own cell: on the
    /// cell's page, its top at or just under the cell's bottom
    /// ([`BAND_GAP_ABOVE`] above to [`BAND_GAP_BELOW`] below), and no other
    /// value cell or band starting between the two (overlapping the band
    /// across), so a band that drifted onto another row fails.
    fn bands_not_below_their_cell(doc: &str) -> Vec<String> {
        let divs = drawn_divs(doc);
        let mut wrong = Vec::new();
        for band in divs.iter().filter(|d| d.band_of.is_some()) {
            let of = band.band_of.as_deref().unwrap();
            let Some(cell) = divs.iter().find(|d| {
                d.band_of.is_none() && d.id.as_deref() == Some(of) && d.page == band.page
            }) else {
                wrong.push(format!(
                    "{of}: band on {:?}, its cell not on that page",
                    band.page
                ));
                continue;
            };
            let bottom = cell.top + cell.height;
            let gap = band.top - bottom;
            if !(-BAND_GAP_ABOVE..=BAND_GAP_BELOW).contains(&gap) {
                wrong.push(format!(
                    "{of}: band top {:.2} is {gap:.2} pt from its cell's bottom {bottom:.2}",
                    band.top
                ));
            }
            let between: Vec<&str> = divs
                .iter()
                .filter(|d| !std::ptr::eq(*d, band) && !std::ptr::eq(*d, cell))
                .filter(|d| d.page == band.page && (d.value_cell || d.band_of.is_some()))
                .filter(|d| d.top >= bottom - 0.5 && d.top < band.top - 0.5)
                .filter(|d| {
                    d.left.max(band.left) + 1.0 < (d.left + d.width).min(band.left + band.width)
                })
                .map(|d| d.id.as_deref().or(d.band_of.as_deref()).unwrap_or("?"))
                .collect();
            if !between.is_empty() {
                wrong.push(format!(
                    "{of}: rows between the cell and its band: {between:?}"
                ));
            }
        }
        wrong
    }

    /// D1, the band's sideways position: a band must start within 1 pt of
    /// its own cell's left edge, or span the cell's whole width (a wider
    /// band is fine: it still reads as the cell's value). Anything else is
    /// the value printing under a neighbouring column.
    fn bands_sideways_off_their_cell(doc: &str) -> Vec<String> {
        let divs = drawn_divs(doc);
        let mut wrong = Vec::new();
        for band in divs.iter().filter(|d| d.band_of.is_some()) {
            let of = band.band_of.as_deref().unwrap();
            let Some(cell) = divs.iter().find(|d| {
                d.band_of.is_none() && d.id.as_deref() == Some(of) && d.page == band.page
            }) else {
                continue;
            };
            let aligned = (band.left - cell.left).abs() <= 1.0;
            let covers = band.left <= cell.left + 0.5
                && band.left + band.width >= cell.left + cell.width - 0.5;
            if !aligned && !covers {
                wrong.push(format!(
                    "{of}: band left {:.2} width {:.2} neither within 1 pt of its cell's left {:.2} nor covering the cell's span to {:.2}",
                    band.left,
                    band.width,
                    cell.left,
                    cell.left + cell.width
                ));
            }
        }
        wrong
    }

    /// D2, the band is white on top: the band prints its value on white
    /// over whatever it overlaps, so it must intersect no other value cell
    /// of the page — anything under it would be hidden. Its own cell is
    /// excepted: the band never overlaps it (the vertical check), and it
    /// is the value the band prints.
    fn bands_covering_other_value_cells(doc: &str) -> Vec<String> {
        let divs = drawn_divs(doc);
        let mut wrong = Vec::new();
        for band in divs.iter().filter(|d| d.band_of.is_some()) {
            let of = band.band_of.as_deref().unwrap();
            for cell in divs.iter().filter(|d| d.value_cell && d.page == band.page) {
                if cell.id.as_deref() == Some(of) {
                    continue;
                }
                let x =
                    (band.left + band.width).min(cell.left + cell.width) - band.left.max(cell.left);
                let y =
                    (band.top + band.height).min(cell.top + cell.height) - band.top.max(cell.top);
                if x > 0.05 && y > 0.05 {
                    wrong.push(format!(
                        "{of}: band covers value cell {} by {x:.2} pt sideways, {y:.2} pt down",
                        cell.id.as_deref().unwrap_or("?")
                    ));
                }
            }
        }
        wrong
    }

    /// The band check fails a band that drifted a row down: 1700's band for
    /// p2c153, moved below the next value row under its cell (same gap),
    /// is too far from its cell and has that row between them; in place it
    /// passes.
    #[test]
    fn a_rule6_band_moved_down_a_row_fails() {
        let slug = "1700-2018";
        let cells = writer_cells(slug).unwrap();
        let fields: BTreeMap<String, String> = cells
            .joins
            .keys()
            .map(|key| (key.clone(), words_of(200)))
            .collect();
        let doc = filled_document(slug, &fields).unwrap();
        assert_eq!(bands_not_below_their_cell(&doc), Vec::<String>::new());

        let divs = drawn_divs(&doc);
        let band = divs
            .iter()
            .find(|d| d.band_of.as_deref() == Some("p2c153"))
            .unwrap();
        let cell = divs
            .iter()
            .find(|d| d.band_of.is_none() && d.id.as_deref() == Some("p2c153"))
            .unwrap();
        let bottom = cell.top + cell.height;
        let next = divs
            .iter()
            .filter(|d| d.page == band.page && d.value_cell && d.id != cell.id)
            .filter(|d| d.top >= bottom - 0.5)
            .filter(|d| {
                d.left.max(band.left) + 1.0 < (d.left + d.width).min(band.left + band.width)
            })
            .min_by(|a, b| a.top.total_cmp(&b.top))
            .unwrap();
        let moved_top = band.top + (next.top + next.height - bottom);
        let marker = "data-rule6-of=\"p2c153\" style=\"";
        let at = doc.find(marker).unwrap() + marker.len();
        let old_top = format!("top:{:.2}pt", band.top);
        assert!(doc[at..].starts_with(&format!("left:{:.2}pt;{old_top}", band.left)));
        let mut moved = doc.clone();
        let top_at = at + doc[at..].find(&old_top).unwrap();
        moved.replace_range(
            top_at..top_at + old_top.len(),
            &format!("top:{moved_top:.2}pt"),
        );

        let wrong = bands_not_below_their_cell(&moved);
        let next_id = next.id.clone().unwrap();
        assert_eq!(wrong.len(), 2, "{wrong:?}");
        assert!(wrong[0].starts_with("p2c153: band top "), "{wrong:?}");
        assert!(
            wrong[1].starts_with("p2c153: rows between") && wrong[1].contains(&next_id),
            "{wrong:?}"
        );
    }

    /// The strip a Rule 6 band inserts at a row boundary must not break the
    /// frame lines running down the sheet's columns: on 1701MS page 2 the
    /// amount-column dividers abut the item-9 cut (one segment ends exactly
    /// at it, the next row's starts exactly at it), so the insertion
    /// stretches the upper segment through the strip; comb ticks, which
    /// abut nothing below the boundary, stay inside their own row.
    #[test]
    fn a_rule6_strip_keeps_the_frame_lines_it_crosses() {
        let slug = "1701ms-2024";
        let cells = writer_cells(slug).unwrap();
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        for key in cells.joins.keys() {
            if key.contains("description") || key.contains("specify") {
                fields.insert(key.clone(), words_of(200));
            } else {
                fields.insert(key.clone(), "1000".to_string());
            }
        }
        let doc = filled_document(slug, &fields).unwrap();

        let band_marker = "data-rule6-of=\"p2c179\" style=\"";
        let at = doc.find(band_marker).unwrap() + band_marker.len();
        let style = &doc[at..at + 120];
        let pt = |name: &str| -> f64 {
            let key = format!("{name}:");
            let from = style.find(&key).unwrap() + key.len();
            style[from..].split("pt").next().unwrap().parse().unwrap()
        };
        let band_top = pt("top");
        let band_bottom = band_top + pt("height");

        let rects = |f: &dyn Fn(f64, f64, f64, f64) -> bool| -> Vec<(f64, f64, f64, f64)> {
            let mut out = Vec::new();
            let mut cursor = 0;
            while let Some(rel) = doc[cursor..].find("<rect ") {
                let start = cursor + rel;
                let Some(end_rel) = doc[start..].find('>') else {
                    break;
                };
                let tag = &doc[start..start + end_rel + 1];
                let num = |a: &str| attr(tag, a).and_then(|v| v.parse::<f64>().ok());
                if let (Some(x), Some(y), Some(w), Some(h)) =
                    (num("x"), num("y"), num("width"), num("height"))
                {
                    if f(x, y, w, h) {
                        out.push((x, y, w, h));
                    }
                }
                cursor = start + end_rel + 1;
            }
            out
        };

        // Both amount-column dividers run continuously from above the strip
        // to below the band's bottom edge.
        for divider_x in [327.45, 461.58] {
            let bridging = rects(&|x, y, w, h| {
                w <= 1.6
                    && (x - divider_x).abs() <= 1.0
                    && y < band_top - 1.0
                    && y + h >= band_bottom - 0.75
            });
            assert!(
                !bridging.is_empty(),
                "no frame line at x~{divider_x} bridges the strip (band {band_top}..{band_bottom})"
            );
        }
    }

    /// D1's negative: 1700's band for p2c153 moved sideways under the
    /// next column of its row fails the sideways check (and nothing else
    /// moves, so exactly one failure names it); in place it passes.
    #[test]
    fn a_rule6_band_moved_sideways_a_column_fails() {
        let slug = "1700-2018";
        let cells = writer_cells(slug).unwrap();
        let fields: BTreeMap<String, String> = cells
            .joins
            .keys()
            .map(|key| (key.clone(), words_of(200)))
            .collect();
        let doc = filled_document(slug, &fields).unwrap();
        assert_eq!(bands_sideways_off_their_cell(&doc), Vec::<String>::new());

        let divs = drawn_divs(&doc);
        let band = divs
            .iter()
            .find(|d| d.band_of.as_deref() == Some("p2c153"))
            .unwrap();
        let cell = divs
            .iter()
            .find(|d| d.band_of.is_none() && d.id.as_deref() == Some("p2c153"))
            .unwrap();
        // The neighbouring column: the nearest other value cell whose row
        // overlaps the cell's, sideways of it; falling back to a plain
        // 30 pt shift where the row has none.
        let neighbour = divs
            .iter()
            .filter(|d| d.page == band.page && d.value_cell && d.id != cell.id)
            .filter(|d| d.top.max(cell.top) + 1.0 < (d.top + d.height).min(cell.top + cell.height))
            .map(|d| d.left)
            .min_by(|a, b| (a - cell.left).abs().total_cmp(&(b - cell.left).abs()));
        let moved_left = neighbour.unwrap_or(cell.left + 30.0);
        let marker = "data-rule6-of=\"p2c153\" style=\"";
        let at = doc.find(marker).unwrap() + marker.len();
        let old_left = format!("left:{:.2}pt", band.left);
        assert!(doc[at..].starts_with(&old_left));
        let mut moved = doc.clone();
        moved.replace_range(at..at + old_left.len(), &format!("left:{moved_left:.2}pt"));

        let wrong = bands_sideways_off_their_cell(&moved);
        assert_eq!(wrong.len(), 1, "{wrong:?}");
        assert!(wrong[0].starts_with("p2c153: band left "), "{wrong:?}");
    }

    /// D2's negative: 1700's band for p2c153 moved down over the next
    /// value cell under its own fails the white-on-top check, naming the
    /// covered cell; in place it passes.
    #[test]
    fn a_rule6_band_moved_over_another_value_cell_fails() {
        let slug = "1700-2018";
        let cells = writer_cells(slug).unwrap();
        let fields: BTreeMap<String, String> = cells
            .joins
            .keys()
            .map(|key| (key.clone(), words_of(200)))
            .collect();
        let doc = filled_document(slug, &fields).unwrap();
        assert_eq!(bands_covering_other_value_cells(&doc), Vec::<String>::new());

        let divs = drawn_divs(&doc);
        let band = divs
            .iter()
            .find(|d| d.band_of.as_deref() == Some("p2c153"))
            .unwrap();
        let cell = divs
            .iter()
            .find(|d| d.band_of.is_none() && d.id.as_deref() == Some("p2c153"))
            .unwrap();
        let bottom = cell.top + cell.height;
        let next = divs
            .iter()
            .filter(|d| d.page == band.page && d.value_cell && d.id != cell.id)
            .filter(|d| d.top >= bottom - 0.5)
            .filter(|d| {
                d.left.max(band.left) + 1.0 < (d.left + d.width).min(band.left + band.width)
            })
            .min_by(|a, b| a.top.total_cmp(&b.top))
            .unwrap();
        let moved_top = next.top + next.height / 2.0 - band.height / 2.0;
        let marker = "data-rule6-of=\"p2c153\" style=\"";
        let at = doc.find(marker).unwrap() + marker.len();
        let old_top = format!("top:{:.2}pt", band.top);
        assert!(doc[at..].starts_with(&format!("left:{:.2}pt;{old_top}", band.left)));
        let mut moved = doc.clone();
        let top_at = at + doc[at..].find(&old_top).unwrap();
        moved.replace_range(
            top_at..top_at + old_top.len(),
            &format!("top:{moved_top:.2}pt"),
        );

        let wrong = bands_covering_other_value_cells(&moved);
        let next_id = next.id.clone().unwrap();
        assert!(
            wrong
                .iter()
                .any(|w| w.starts_with("p2c153: band covers value cell ") && w.contains(&next_id)),
            "{wrong:?}"
        );
    }

    /// A value cell placed just right of a band's cell, so close that
    /// narrowing would squeeze the band below its floor, makes the band move
    /// down below the cell instead of shrinking.
    #[test]
    fn a_value_cell_just_right_of_the_band_cell_moves_the_band_down_not_shrinks() {
        let cells = vec![
            ("c1".to_string(), 14.0, 100.0, 312.0, 12.0),
            ("c2".to_string(), 40.0, 99.0, 100.0, 30.0),
        ];
        let (x1, cut_y) = super::rule6_place_band(14.0, 590.0, 112.0, "c1", &cells);
        assert_eq!(x1, 590.0, "the band must not shrink below the floor");
        assert!(
            cut_y >= 129.5,
            "the band must move below the obstructing cell, cut_y {cut_y}"
        );
        // Far enough right that narrowing keeps the floor: the band narrows
        // and stays on its line.
        let cells = vec![
            ("c1".to_string(), 14.0, 100.0, 312.0, 12.0),
            ("c2".to_string(), 200.0, 99.0, 100.0, 30.0),
        ];
        let (x1, cut_y) = super::rule6_place_band(14.0, 590.0, 112.0, "c1", &cells);
        assert_eq!(x1, 198.0, "the band ends 2 pt before the cell");
        assert_eq!(cut_y, 112.0, "the band keeps its line");
    }

    /// A truncated Rule 6 band ends on a whole word, then the ellipsis: a
    /// fragment cut mid-word would print as if it were a word of its own.
    #[test]
    fn a_truncated_rule6_band_ends_on_a_whole_word() {
        let text = "UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG METRO MANILA UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG";
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm1701:txtPg4IPart7_9Specify".to_string(),
            text.to_string(),
        );
        let html = filled_document("1701-2018", &fields).unwrap();
        let tags = super::input_tags(&html);
        let band = tags
            .iter()
            .find(|t| t.tag.contains("data-overflow-of=\"p4c213\""))
            .expect("1701 item 9 Rule 6 band")
            .tag;
        let value = super::attr(band, "value").unwrap().replace("&#10;", "\n");
        assert!(
            value.contains("..."),
            "the narrowed band must truncate this fill: {value:?}"
        );
        assert!(
            !value.contains("GLOB..."),
            "a mid-word fragment must never print: {value:?}"
        );
        for piece in value.split('\n') {
            let Some(head) = piece.strip_suffix("...") else {
                continue;
            };
            let head = head.trim_end();
            let word = head.split_whitespace().last().unwrap_or("");
            assert!(
                !word.is_empty(),
                "the ellipsis must follow a word: {value:?}"
            );
            let whole = text.match_indices(word).any(|(at, _)| {
                let after = at + word.len();
                after >= text.len() || text[after..].starts_with(char::is_whitespace)
            });
            assert!(whole, "{word:?} is cut mid-word in {value:?}");
        }
    }

    /// Goldens: every sheet's long-text print pinned exactly
    /// (`fixtures/overflow-goldens.json`, `BIR_PRINT_BLESS_OVERFLOW=1`), and
    /// the sheets whose print measuring changed against the estimate's
    /// print, listed exactly ([`CHANGED_BY_MEASURING`]); the rest print
    /// byte for byte as they did.
    #[test]
    fn long_text_prints_as_pinned_on_every_sheet() {
        let now: BTreeMap<String, String> = sheet_slugs()
            .iter()
            .map(|slug| (slug.clone(), overflow_print_hash(slug)))
            .collect();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/fixtures/overflow-goldens.json"
        );
        if std::env::var_os("BIR_PRINT_BLESS_OVERFLOW").is_some() {
            std::fs::write(path, serde_json::to_string_pretty(&now).unwrap() + "\n").unwrap();
            return;
        }
        let golden: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/overflow-goldens.json")).unwrap();
        assert_eq!(golden, now, "every sheet prints long text as pinned");
        let estimate: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/overflow-goldens-estimate.json")).unwrap();
        assert_eq!(
            estimate.keys().collect::<Vec<_>>(),
            now.keys().collect::<Vec<_>>()
        );
        let changed: Vec<&str> = now
            .iter()
            .filter(|(slug, hash)| estimate[*slug] != **hash)
            .map(|(slug, _)| slug.as_str())
            .collect();
        assert_eq!(
            changed, CHANGED_BY_MEASURING,
            "the sheets measuring changed"
        );
    }

    /// Each sheet's type is read from its form.css: the 11 sheets that set
    /// `.fi` in Tinos measure in Tinos, the rest in Arimo, and the `.fi`
    /// word-spacing is each sheet's own (fixed points: it does not scale
    /// with the size); no `.fi` sets a letter-spacing, the size classes do.
    #[test]
    fn each_sheet_measures_in_its_own_face_and_spacing() {
        const TINOS: [&str; 10] = [
            "0605-1999",
            "1601c-2018",
            "1602q-2019",
            "1603q-2018",
            "1604e-2018",
            "1604f-2018",
            "2200c-2018",
            "2200p-2020",
            "2550m-2007",
            "2553-1999",
        ];
        for slug in sheet_slugs() {
            let sheet = SheetType::of(bundle(&slug).unwrap().css);
            let face = if TINOS.contains(&slug.as_str()) {
                Face::Tinos
            } else {
                Face::Arimo
            };
            assert_eq!(sheet.face, face, "{slug}");
            assert_eq!(sheet.word.em, 0.0, "{slug}");
            assert_eq!(sheet.letter, Spacing::default(), "{slug}");
        }
        for (slug, pt) in [
            ("1604f-2018", 0.3238),
            ("2200t-2020", 0.0),
            ("1600-pt-2018", 0.2778),
            ("1604e-2018", 0.1518),
            ("1600wp-2010", 0.0546),
            ("0605-1999", -0.1071),
            ("0619e-2018", 0.0),
        ] {
            assert_eq!(
                SheetType::of(bundle(slug).unwrap().css).word.pt,
                pt,
                "{slug}"
            );
        }
        // A size class's letter-spacing (0605's .fh10) and size.
        let sheet = SheetType::of(bundle("0605-1999").unwrap().css);
        assert_eq!(
            sheet.letter_for("fi fh10"),
            Spacing {
                pt: 0.0102,
                em: 0.0
            }
        );
        assert_eq!(sheet.size_for("fi fh10"), Some(9.47));
        // The measure: advances, plus the word-spacing on each space, plus
        // the letter-spacing on each character.
        let tinos = SheetType::of(bundle("1604f-2018").unwrap().css);
        let text = "AB CD";
        let expected = Face::Tinos.ems(text) * 9.0 + 0.3238;
        assert!((tinos.width(tinos.letter, text, 9.0) - expected).abs() < 1e-9);
        let letter = Spacing { pt: 0.5, em: 0.1 };
        let expected = Face::Tinos.ems(text) * 9.0 + 0.3238 + 5.0 * (0.5 + 0.9);
        assert!((tinos.width(letter, text, 9.0) - expected).abs() < 1e-9);
        // The review's word-spacing regression texts (narrow letters, many
        // spaces), on every sheet at each width: measured with the sheet's
        // spacing, each line sets within its comb after its 1 pt padding.
        let texts = [
            "ILIGAN LILIO IMPERIAL LIMITLESS INDIVISIBLE LIMITING INITIAL ILLUMINATED ILIGAN CITY",
            "LILIAN ILIGAN LILI INI LIMIT INDIA LIMITED PILI LILIO INC",
            "IVIRIS IN ILIAN LIMIT LILIA SILI TIPIL INITIAL LIMITING INC",
            "UNIT 1011 TILL INTERNATIONAL ILLINOIS MILL ISLAND LIMITED ITI BILIT",
            "DELA CRUZ JUAN CARLO SANTOS ILAGAN MIRANDA VILLANUEVA IMPORT INC",
        ];
        for slug in sheet_slugs() {
            let sheet = SheetType::of(bundle(&slug).unwrap().css);
            for width in [100.0, 150.0, 200.0, 300.0, 446.52] {
                for text in texts {
                    let size = fit_line(&sheet, width, text);
                    let set = sheet.width(sheet.letter, text, size);
                    assert!(
                        1.0 + set <= width,
                        "{slug} {width}pt: {text} sets {set}pt at {size}pt"
                    );
                }
            }
        }
        // The fit is the largest size that sets within the room, rounded down.
        let size = tinos.fit(letter, 100.0, text, 50.0);
        assert!(tinos.width(letter, text, size) <= 100.0);
        assert!(tinos.width(letter, text, size + 0.01) > 100.0);
    }

    /// A character the face does not map is drawn from another font: it
    /// counts 1.3 em, more than CJK or an emoji takes (about 1.2 em), so the
    /// type comes out small enough; a mapped one counts its own advance.
    #[test]
    fn an_unmapped_character_counts_wide() {
        for face in [Face::Arimo, Face::Tinos] {
            for c in ['\u{0416}', '\u{4E2D}', '\u{1F600}', '\u{0150}'] {
                assert_eq!(face.ems(&c.to_string()), FALLBACK_EMS, "{face:?} {c}");
            }
            const { assert!(FALLBACK_EMS >= 1.3) };
            // Latin-1's Ñ is mapped, at its own width.
            let n = face.ems("\u{00D1}");
            assert!(n > 0.5 && n < 1.0, "{face:?} Ñ: {n}");
        }
        // A no-break space takes the word-spacing as a space does.
        let sheet = SheetType::of(bundle("1604f-2018").unwrap().css);
        let spacing =
            |text: &str| sheet.width(sheet.letter, text, 9.0) - Face::Tinos.ems(text) * 9.0;
        assert!((spacing("A\u{a0}B") - 0.3238).abs() < 1e-9);
        assert!((spacing("A B") - 0.3238).abs() < 1e-9);
        assert!(spacing("AB").abs() < 1e-9);
        assert_eq!(Face::Arimo.ems("A"), 1366.0 / 2048.0);
        assert_eq!(
            Face::Tinos.ems("A"),
            Face::Tinos.advances()[33].1 as f64 / 2048.0
        );
    }

    /// Round 6, D8: tabs and no-break spaces measure what headless Chrome
    /// draws. Each width is an `<input>`'s scrollWidth in Chrome
    /// (`--headless=new`) with the eBIRForms Arimo or Tinos face at 2048 px
    /// type, where one font unit is one pixel, with no spacing, 100 px
    /// word-spacing or 50 px letter-spacing: a tab runs to the next stop,
    /// 8 × (space + word-spacing + letter-spacing) from the line's start,
    /// or the one after when it would end less than half a space short
    /// (`AAA'`); a no-break space takes the word-spacing.
    #[test]
    fn tabs_and_no_break_spaces_measure_as_headless_chrome_draws_them() {
        let measured: [(Face, f64, f64, &str, f64); 33] = [
            (Face::Arimo, 0.0, 0.0, "A\tB", 5918.0),
            (Face::Arimo, 0.0, 0.0, "\tB", 5918.0),
            (Face::Arimo, 0.0, 0.0, "AAA\tB", 5918.0),
            (Face::Arimo, 0.0, 0.0, "AAA'\tB", 10470.0),
            (Face::Arimo, 0.0, 0.0, "AAAAAAAAAA\tB", 19574.0),
            (Face::Arimo, 0.0, 0.0, "A\t\tB", 10470.0),
            (Face::Arimo, 0.0, 0.0, "A\tB\tC", 10583.0),
            (Face::Arimo, 0.0, 0.0, "A\u{a0}B", 3301.0),
            (Face::Arimo, 0.0, 0.0, "A B", 3301.0),
            (Face::Arimo, 100.0, 0.0, "A\tB", 6718.0),
            (Face::Arimo, 100.0, 0.0, "AAA'\tB", 6718.0),
            (Face::Arimo, 100.0, 0.0, "AAAAAAAAAA\tB", 17422.0),
            (Face::Arimo, 100.0, 0.0, "A\t\tB", 12070.0),
            (Face::Arimo, 100.0, 0.0, "A\u{a0}B", 3401.0),
            (Face::Arimo, 0.0, 50.0, "A\tB", 6368.0),
            (Face::Arimo, 0.0, 50.0, "AAA'\tB", 11320.0),
            (Face::Arimo, 0.0, 50.0, "AAAAAAAAAA\tB", 16272.0),
            (Face::Arimo, 0.0, 50.0, "A\tB\tC", 11433.0),
            (Face::Arimo, 0.0, 50.0, "A\u{a0}B", 3451.0),
            (Face::Tinos, 0.0, 0.0, "A\tB", 5462.0),
            (Face::Tinos, 0.0, 0.0, "AAA\tB", 9558.0),
            (Face::Tinos, 0.0, 0.0, "AAA'\tB", 9558.0),
            (Face::Tinos, 0.0, 0.0, "A\t\tB", 9558.0),
            (Face::Tinos, 0.0, 0.0, "AAAAAAAAAA\tB", 17750.0),
            (Face::Tinos, 0.0, 0.0, "A\u{a0}B", 3357.0),
            (Face::Tinos, 100.0, 0.0, "AAA\tB", 6262.0),
            (Face::Tinos, 100.0, 0.0, "AAA'\tB", 11158.0),
            (Face::Tinos, 100.0, 0.0, "A\tB\tC", 11158.0),
            (Face::Tinos, 100.0, 0.0, "A\u{a0}B", 3457.0),
            (Face::Tinos, 0.0, 50.0, "AAA\tB", 10408.0),
            (Face::Tinos, 0.0, 50.0, "AAAAAAAAAA\tB", 19400.0),
            (Face::Tinos, 0.0, 50.0, "A\tB\tC", 10408.0),
            (Face::Tinos, 0.0, 50.0, "A\u{a0}B", 3507.0),
        ];
        for (face, word, letter, text, chrome) in measured {
            let sheet = SheetType {
                face,
                word: Spacing { pt: word, em: 0.0 },
                ..SheetType::plain()
            };
            let letter = Spacing {
                pt: letter,
                em: 0.0,
            };
            assert_eq!(
                sheet.width(letter, text, 2048.0),
                chrome,
                "{face:?} word {word} {text:?}"
            );
        }
        // A tab's most is a whole stop: 8 space advances.
        assert_eq!(Face::Arimo.ems("\t"), 8.0 * 569.0 / 2048.0);
        assert_eq!(Face::Tinos.ems("\t"), 8.0 * 512.0 / 2048.0);
    }

    /// Round 7, F1: where a tab ends is not monotone in the size, so the fit
    /// scans down from the largest size instead of bisecting. The
    /// reviewer's case on 0605's Tinos spacing (word -0.1071 pt, letter
    /// 0.0113 pt) sets at 7.83 pt in Chrome; bisecting returned 5.97 pt.
    #[test]
    fn a_tab_fit_finds_the_largest_size_that_sets() {
        let sheet = SheetType {
            face: Face::Tinos,
            word: Spacing {
                pt: -0.1071,
                em: 0.0,
            },
            ..SheetType::plain()
        };
        let letter = Spacing {
            pt: 0.0113,
            em: 0.0,
        };
        let text = "E  FFAEADF\t";
        let size = sheet.fit(letter, 44.724, text, 9.0);
        assert_eq!(size, 7.83);
        assert!(sheet.width(letter, text, size) <= 44.724);
        // No larger hundredth up to 9 pt sets.
        assert!((784..=900).all(|h| sheet.width(letter, text, h as f64 / 100.0) > 44.724));
    }

    /// Round 8: a tab fit returns the hundredth that sets as it is, not
    /// rounded down again (0.29 * 100 is 28.999..., which floor_size would
    /// print as 0.28). With no spacing the width scales with the size, so a
    /// room exactly as wide as the text at h/100 pt fits it at h/100 pt.
    #[test]
    fn a_tab_fit_returns_its_exact_hundredth() {
        let sheet = SheetType::plain();
        let letter = Spacing::default();
        let text = "\tA";
        for hundredths in 1..=900 {
            let size = hundredths as f64 / 100.0;
            let room = sheet.width(letter, text, size);
            assert_eq!(sheet.fit(letter, room, text, 9.0), size, "{hundredths}");
        }
        let room = sheet.width(letter, text, 0.29);
        assert_eq!(sheet.fit(letter, room, text, 9.0), 0.29);
    }

    /// A printed size is rounded down to 0.01 pt, never to the nearest:
    /// rounding never adds width.
    #[test]
    fn a_printed_size_rounds_down() {
        assert_eq!(floor_size(8.559), 8.55);
        assert_eq!(floor_size(8.5), 8.5);
        assert_eq!(floor_size(0.001), 0.01);
        let sheet = SheetType::plain();
        let text = words_of(200);
        let size = sheet.fit(sheet.letter, 444.52, &text, PLAIN_MAX_PT);
        assert_eq!(size, floor_size(size));
        assert!(sheet.width(sheet.letter, &text, size) <= 444.52);
    }

    /// A plain box's value longer than the box prints smaller, measured, so
    /// it stays inside (0605's 80-character registered address, p1c38,
    /// 365.51 pt, showed 61 characters); one that fits keeps its bytes.
    #[test]
    fn a_plain_box_value_longer_than_its_box_is_measured() {
        let slug = "0605-1999";
        let loaded = bundle(slug).unwrap();
        let sheet = SheetType::of(loaded.css);
        let cells = writer_cells(slug).unwrap();
        let (key, _) = cells
            .joins
            .iter()
            .find(|(_, ids)| ids.iter().any(|id| id == "p1c38"))
            .expect("a join prints in p1c38");
        let (room, classes, _) = plain_inputs(loaded.html, "p1c38")[0];
        let class_size = sheet.size_for(classes).unwrap();
        let read = |document: &str| -> (String, Option<f64>) {
            let at = document.find("<div id=\"p1c38\"").unwrap();
            let at = at + document[at..].find("<input").unwrap();
            let input = &document[at..at + document[at..].find('>').unwrap()];
            let style = attr(input, "style").unwrap_or("");
            let size = style
                .find("font-size:")
                .and_then(|at| style[at + "font-size:".len()..].split("pt").next())
                .and_then(|size| size.parse().ok());
            (attr(input, "value").unwrap_or("").to_string(), size)
        };
        let long = words_of(80);
        let document =
            filled_document(slug, &BTreeMap::from([(key.clone(), long.clone())])).unwrap();
        let (value, size) = read(&document);
        assert_eq!(
            value.replace("&#10;", " "),
            long,
            "the whole value prints (wrapped onto two lines in a >=18pt box)"
        );
        let size = size.expect("a smaller size");
        assert!(size < class_size);
        assert!(size >= PLAIN_TEXT_MIN_PT);
        assert!(room >= 2.0);
        for line in value.split("&#10;") {
            assert!(sheet.width(sheet.letter_for(classes), line, size) <= room - 1.0);
        }
        let fitting = words_of(20);
        let document =
            filled_document(slug, &BTreeMap::from([(key.clone(), fitting.clone())])).unwrap();
        assert_eq!(
            read(&document),
            (fitting, None),
            "a value that fits keeps its size"
        );
        // The size goes last in the input's own style.
        assert_eq!(
            with_font_size(
                "<input class=\"fi fh15\" style=\"inset:0.75pt\" value=\"X\">",
                7.5
            ),
            "<input class=\"fi fh15\" style=\"inset:0.75pt;font-size:7.50pt\" value=\"X\">"
        );
        assert_eq!(
            with_font_size("<input class=\"fi\" value=\"X\">", 7.5),
            "<input class=\"fi\" value=\"X\" style=\"font-size:7.50pt\">"
        );
    }

    /// Every sheet filled with values that fit their combs: each text join
    /// whose cells are all combs holding as many characters as its smallest
    /// comb has boxes (at most 12), hashed. Nothing overflows, so the
    /// overflow sizing never runs.
    fn fitting_print_hash(slug: &str) -> String {
        use sha2::{Digest, Sha256};
        let loaded = bundle(slug).unwrap();
        let cells = writer_cells(slug).unwrap();
        let tags = input_tags(loaded.html);
        let grouped = grouped_input_indices(&tags);
        // Only the joins whose every cell is a comb (its boxes are its slot
        // inputs; a 20YY comb has two): a plain box's room is in points,
        // not boxes, and `fitting_plain_print_hash` fills those.
        let fields: BTreeMap<String, String> = cells
            .joins
            .iter()
            .filter_map(|(key, ids)| {
                let slots: Option<Vec<usize>> = ids
                    .iter()
                    .map(|cell| {
                        grouped
                            .get(cell.as_str())
                            .filter(|indices| indices.iter().all(|&i| tags[i].slot.is_some()))
                            .map(Vec::len)
                    })
                    .collect();
                let slots = slots?.into_iter().min()?.min(12);
                Some((key.clone(), words_of(slots.max(1))))
            })
            .collect();
        let document = filled_document(slug, &fields).unwrap();
        let overflowing: Vec<String> = printed_cells(&document)
            .into_iter()
            .filter(|(_, (_, size))| size.is_some())
            .map(|(cell, (text, _))| format!("{cell} {text:?}"))
            .collect();
        assert!(
            overflowing.is_empty(),
            "{slug}: nothing overflows: {overflowing:?}"
        );
        Sha256::digest(document.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// Every sheet filled with values that fit its plain boxes: each text
    /// join whose cells are all plain boxes holding the longest address
    /// text that sets within each of them at its own class's size, in the
    /// sheet's face with its word- and letter-spacing, hashed, with the
    /// number of keys filled.
    fn fitting_plain_print_hash(slug: &str) -> (String, usize) {
        use sha2::{Digest, Sha256};
        let fields = fitting_plain_fields(slug);
        let document = filled_document(slug, &fields).unwrap();
        let hash = Sha256::digest(document.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        (hash, fields.len())
    }

    /// Whether `cell` of sheet `slug` is one whose layout round 6 fixed
    /// (1600WP's attachment-period dates, which every earlier fill printed
    /// whole into each one-character box): the goldens recorded with the
    /// earlier fills leave its joins out, and
    /// `split_cells_print_one_character_per_box_or_one_line` pins it.
    fn layout_fixed_in_round_6(slug: &str, cell: &str) -> bool {
        slug == "1600wp-2010"
            && SheetType::of_sheet(slug, "")
                .split
                .iter()
                .any(|(split, _)| *split == cell)
    }

    /// The fields [`fitting_plain_print_hash`] fills.
    fn fitting_plain_fields(slug: &str) -> BTreeMap<String, String> {
        let loaded = bundle(slug).unwrap();
        let cells = writer_cells(slug).unwrap();
        let sheet = SheetType::of_sheet(slug, loaded.css);
        let mut fields = BTreeMap::new();
        for (key, ids) in &cells.joins {
            // A split cell takes one character per box, not a plain line.
            if ids
                .iter()
                .any(|cell| sheet.split.iter().any(|(split, _)| split == cell))
            {
                continue;
            }
            // As recorded: each cell's first input, its room less 1 pt.
            let boxes: Option<Vec<(f64, &str)>> = ids
                .iter()
                .map(|cell| {
                    plain_inputs(loaded.html, cell)
                        .first()
                        .map(|(room, classes, _)| (room - 1.0, *classes))
                })
                .collect();
            let Some(boxes) = boxes.filter(|boxes| !boxes.is_empty()) else {
                continue;
            };
            let fits = |text: &str| {
                boxes.iter().all(|(room, classes)| {
                    sheet.size_for(classes).is_some_and(|size| {
                        sheet.width(sheet.letter_for(classes), text, size) <= *room
                    })
                })
            };
            if let Some(len) = (1..=120).rev().find(|len| fits(&words_of(*len))) {
                fields.insert(key.clone(), words_of(len));
            }
        }
        fields
    }

    /// A value that fits its boxes prints byte for byte as before; a value
    /// longer than its boxes is measured (P4-7). The goldens were written
    /// once by the fill before every sheet's type was measured
    /// (`fixtures/fitting-goldens.json` for the combs,
    /// `fixtures/fitting-plain-goldens.json` for the plain boxes) and are
    /// never rewritten: the test has no way to bless them. Round 6 left the
    /// split cells (2550M's and 1600WP's) out of the plain boxes, since
    /// they take one character per box, and recorded those two sheets'
    /// entries again with dcc6386a's fill, the way
    /// `fitting-band-goldens.json` was. The 12b merge into integration
    /// recorded the two 1800/1801 comb entries and 1801's plain-box entry
    /// again the same way: PR 10's donor-estate joins, which the branch's
    /// pins predate, are the only values that moved them.
    #[test]
    fn fitting_text_prints_as_before_on_every_sheet() {
        let combs: BTreeMap<String, String> = sheet_slugs()
            .iter()
            .map(|slug| (slug.clone(), fitting_print_hash(slug)))
            .collect();
        let golden: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/fitting-goldens.json")).unwrap();
        assert_eq!(
            golden, combs,
            "every sheet prints fitting comb values as before"
        );
        let mut keys = 0;
        let plain: BTreeMap<String, String> = sheet_slugs()
            .iter()
            .map(|slug| {
                let (hash, filled) = fitting_plain_print_hash(slug);
                keys += filled;
                (slug.clone(), hash)
            })
            .collect();
        assert!(keys > 100, "{keys} plain-box keys filled to fit");
        let golden: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/fitting-plain-goldens.json")).unwrap();
        assert_eq!(
            golden, plain,
            "every sheet prints fitting plain-box values as before"
        );
    }

    /// The values of every text join of sheet `slug` that fit their boxes
    /// to the edge: the longest start of an address (letters and digits for
    /// a split one-character cell) that, in every cell the join prints in,
    /// takes no more characters than a comb has boxes, one character per
    /// box in a split cell, and in each input of a plain cell sets within
    /// that input's whole room at its class's size, measured in the sheet's
    /// type. Some plain-box values reach the band where the round-4 fill
    /// shrank a value that fit (the 1 pt spare); [`band_box_fields`] puts
    /// one in every band box. The joins of [`layout_fixed_in_round_6`]
    /// cells are left out.
    fn band_fields(slug: &str) -> BTreeMap<String, String> {
        const ADDRESS: &str =
            "UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG METRO MANILA";
        const CODE: &str = "1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let loaded = bundle(slug).unwrap();
        let sheet = SheetType::of_sheet(slug, loaded.css);
        let cells = writer_cells(slug).unwrap();
        let tags = input_tags(loaded.html);
        let grouped = grouped_input_indices(&tags);
        let mut fields = BTreeMap::new();
        for (key, ids) in &cells.joins {
            if ids.iter().any(|cell| layout_fixed_in_round_6(slug, cell)) {
                continue;
            }
            let split = ids.iter().any(|cell| {
                sheet.split.iter().any(|(split, _)| split == cell)
                    && grouped
                        .get(cell.as_str())
                        .is_some_and(|indices| splits(&tags, indices))
            });
            let pool = if split { CODE } else { ADDRESS };
            // Each cell's limit, read once: a comb's or a split cell's
            // boxes, a plain cell's inputs (room, size, letter-spacing).
            enum Limit {
                Boxes(usize),
                Split(usize),
                Plain(Vec<Option<(f64, f64, Spacing)>>),
            }
            let limits: Vec<Limit> = ids
                .iter()
                .filter_map(|cell| {
                    let indices = grouped.get(cell.as_str())?;
                    if indices.iter().all(|&i| tags[i].slot.is_some()) {
                        return Some(Limit::Boxes(indices.len()));
                    }
                    if sheet.split.iter().any(|(split, _)| split == cell) && splits(&tags, indices)
                    {
                        return Some(Limit::Split(indices.len()));
                    }
                    Some(Limit::Plain(
                        plain_inputs(loaded.html, cell)
                            .into_iter()
                            .map(|(room, classes, _)| {
                                sheet
                                    .size_for(classes)
                                    .map(|size| (room, size, sheet.letter_for(classes)))
                            })
                            .collect(),
                    ))
                })
                .collect();
            let fits = |text: &str| {
                limits.iter().all(|limit| match limit {
                    Limit::Boxes(boxes) => text.chars().count() <= *boxes,
                    Limit::Split(boxes) => fits_one_per_box(text, *boxes),
                    Limit::Plain(inputs) => {
                        !inputs.is_empty()
                            && inputs.iter().all(|input| {
                                input.is_some_and(|(room, size, letter)| {
                                    sheet.width(letter, text, size) <= room
                                })
                            })
                    }
                })
            };
            let best = (1..=pool.len())
                .rev()
                .map(|n| pool[..n].trim_end())
                .find(|text| !text.is_empty() && fits(text));
            if let Some(text) = best {
                fields.insert(key.clone(), text.to_string());
            }
        }
        fields
    }

    /// A value that fits its boxes to the edge prints byte for byte as at
    /// dcc6386a, the last commit before every sheet's type was measured
    /// (round 5: the 1 pt spare and the split cells shrank such values).
    /// `fixtures/fitting-band-goldens.json` holds, per sheet, the SHA-256 of
    /// the document dcc6386a's fill printed for [`band_fields`]: the fields
    /// were written out by this branch's code, dcc6386a was exported with
    /// `git archive dcc6386a` into a scratch directory, and a scratch binary
    /// depending on its bir-print filled and hashed each sheet. Round 6
    /// recorded the 1600wp-2010 entry again that way, without its
    /// attachment dates ([`layout_fixed_in_round_6`]). The test cannot
    /// bless it. The 12b merge into integration recorded the 1800/1801
    /// entries again too: PR 10's donor-estate joins are the only values
    /// that moved them, and the branch's pins predate those joins.
    #[test]
    fn fitting_band_prints_as_at_dcc6386a_on_every_sheet() {
        let golden: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/fitting-band-goldens.json")).unwrap();
        let mut filled = 0;
        let now: BTreeMap<String, String> = sheet_slugs()
            .iter()
            .map(|slug| {
                let fields = band_fields(slug);
                filled += fields.len();
                (slug.clone(), document_hash(slug, &fields))
            })
            .collect();
        assert!(filled > 500, "{filled} joins filled to the edge");
        assert!(
            band_fields("1706-2018").contains_key("frm1706:txtBuyerName"),
            "1706's buyer name is filled"
        );
        assert_eq!(
            golden, now,
            "every sheet prints values that fit to the edge as before"
        );
    }

    /// The SHA-256 of the document sheet `slug` prints for `fields`.
    fn document_hash(slug: &str, fields: &BTreeMap<String, String>) -> String {
        use sha2::{Digest, Sha256};
        let document = filled_document(slug, fields).unwrap();
        Sha256::digest(document.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// One band box of a sheet: a text join printing in a single plain cell
    /// with one input, not a comb or a split cell, filled with a value that
    /// sets at the input's class size within its room but not within the
    /// room less 1 pt (the band the round-4 fill shrank). The value is the
    /// longest start of the address, else of "1234567890 ABC...", else of
    /// "1234567890ABC...", that lands in the band.
    struct BandBox {
        key: String,
        cell: String,
        value: String,
        room: f64,
        width: f64,
    }

    /// Every band box of sheet `slug` ([`BandBox`]).
    fn band_boxes(slug: &str) -> Vec<BandBox> {
        const POOLS: [&str; 3] = [
            "UNIT 1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG METRO MANILA",
            "1234567890 ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        ];
        let loaded = bundle(slug).unwrap();
        let sheet = SheetType::of_sheet(slug, loaded.css);
        let cells = writer_cells(slug).unwrap();
        let tags = input_tags(loaded.html);
        let grouped = grouped_input_indices(&tags);
        let mut boxes = Vec::new();
        for (key, ids) in &cells.joins {
            let [cell] = ids.as_slice() else {
                continue;
            };
            let Some(indices) = grouped.get(cell.as_str()) else {
                continue;
            };
            if indices.len() != 1
                || tags[indices[0]].slot.is_some()
                || sheet.split.iter().any(|(split, _)| split == cell)
            {
                continue;
            }
            let [(room, classes, _)] = plain_inputs(loaded.html, cell)[..] else {
                continue;
            };
            let Some(size) = sheet.size_for(classes) else {
                continue;
            };
            let letter = sheet.letter_for(classes);
            let found = POOLS.iter().find_map(|pool| {
                (1..=pool.len())
                    .rev()
                    .map(|n| pool[..n].trim_end())
                    .filter(|text| !text.is_empty())
                    .map(|text| (text, sheet.width(letter, text, size)))
                    .find(|(_, width)| room - 1.0 < *width && *width <= room)
            });
            if let Some((value, width)) = found {
                boxes.push(BandBox {
                    key: key.clone(),
                    cell: cell.clone(),
                    value: value.to_string(),
                    room,
                    width,
                });
            }
        }
        boxes
    }

    /// Every band box of every sheet ([`band_boxes`], 1,458 of them) holds
    /// a value in its 1 pt band and prints byte for byte as at dcc6386a:
    /// `fixtures/fitting-band-box-goldens.json` holds, per sheet, the
    /// SHA-256 of the document dcc6386a's fill printed for that sheet's
    /// band-box values (recorded as `fitting-band-goldens.json` was). A
    /// fill that kept the 1 pt spare shrinks every one of these values
    /// (round 6 checked it fails this test). The test cannot bless it.
    /// The 12b merge into integration added two band boxes on 1801 —
    /// PR 10's `freqOtherInput` and `txtInstallmentYear` joins, which the
    /// branch's pin predates — and re-recorded that sheet's golden; every
    /// other sheet stays byte-pinned at dcc6386a.
    #[test]
    fn every_band_box_prints_as_at_dcc6386a() {
        let golden: BTreeMap<String, String> =
            serde_json::from_str(include_str!("fixtures/fitting-band-box-goldens.json")).unwrap();
        let mut total = 0;
        let mut now = BTreeMap::new();
        for slug in sheet_slugs() {
            let boxes = band_boxes(&slug);
            if boxes.is_empty() {
                continue;
            }
            total += boxes.len();
            let fields: BTreeMap<String, String> = boxes
                .iter()
                .map(|band| (band.key.clone(), band.value.clone()))
                .collect();
            assert_eq!(fields.len(), boxes.len(), "{slug}: one key per box");
            let document = filled_document(&slug, &fields).unwrap();
            for band in &boxes {
                assert!(
                    band.room - 1.0 < band.width && band.width <= band.room,
                    "{slug} {}: {:?} measures {} in {}",
                    band.cell,
                    band.value,
                    band.width,
                    band.room
                );
                let inputs = plain_inputs(&document, &band.cell);
                let [(_, _, tag)] = inputs[..] else {
                    panic!("{slug} {}: one input", band.cell);
                };
                assert!(
                    !tag.contains("font-size"),
                    "{slug} {}: {:?} keeps its class size: {tag}",
                    band.cell,
                    band.value
                );
            }
            now.insert(slug.clone(), document_hash(&slug, &fields));
        }
        assert_eq!(total, 1433, "band boxes");
        assert_eq!(golden, now, "every band box prints as at dcc6386a");
    }

    /// The 0.02 pt allowance ([`FIT_EPSILON_PT`]): a value the model puts
    /// 0.01 pt over its input's room prints at its class size (unshrunk);
    /// 0.03 pt over, it shrinks to set within the room less 1 pt.
    #[test]
    fn a_value_a_hair_over_its_room_keeps_its_size() {
        let slug = "0605-1999";
        let loaded = bundle(slug).unwrap();
        let sheet = SheetType::of_sheet(slug, loaded.css);
        let [(room, classes, tag)] = plain_inputs(loaded.html, "p1c33")[..] else {
            panic!("0605 p1c33 has one input");
        };
        let width = plain_cell_width(loaded.html, "p1c33").unwrap();
        let inset = width - room;
        let size = sheet.size_for(classes).unwrap();
        let letter = sheet.letter_for(classes);
        let text = "UNIT 1203 TWELFTH";
        let measured = sheet.width(letter, text, size);
        // A cell whose input's room is the value's width less `over` pt.
        let at = |over: f64| plain_input_size(&sheet, measured - over + inset, tag, text);
        assert_eq!(at(0.0), None);
        assert_eq!(at(0.01), None, "0.01 pt over: unshrunk");
        let shrunk = at(0.03).expect("0.03 pt over: shrunk");
        let room = measured - 0.03;
        assert!(room >= 2.0);
        assert_eq!(shrunk, sheet.fit(letter, room - 1.0, text, size));
        assert!(shrunk < size);
        assert!(sheet.width(letter, text, shrunk) <= room - 1.0);
    }

    /// Round 6, D4: a split one-character cell (1600WP's attachment-period
    /// dates, 2550M's sheets count, RDO and ZIP) prints a value of no more
    /// characters than boxes one character per box at its class's size,
    /// like a comb, and a longer one once, as a single plain line measured
    /// across the whole cell, its boxes empty. The split cells listed are
    /// every joined cell of one-character inputs on the frozen sheets.
    #[test]
    fn split_cells_print_one_character_per_box_or_one_line() {
        let mut listed = Vec::new();
        let mut found = Vec::new();
        for slug in sheet_slugs() {
            let loaded = bundle(&slug).unwrap();
            let sheet = SheetType::of_sheet(&slug, loaded.css);
            let cells = writer_cells(&slug).unwrap();
            let tags = input_tags(loaded.html);
            let grouped = grouped_input_indices(&tags);
            for ids in cells.joins.values() {
                for cell in ids {
                    if grouped
                        .get(cell.as_str())
                        .is_some_and(|indices| splits(&tags, indices))
                    {
                        found.push(format!("{slug} {cell}"));
                    }
                }
            }
            for (cell, _) in sheet.split {
                listed.push(format!("{slug} {cell}"));
                let indices = &grouped[*cell];
                let (key, _) = cells
                    .joins
                    .iter()
                    .find(|(_, ids)| ids.iter().any(|id| id == cell))
                    .unwrap();
                let fill = |value: &str| {
                    filled_document(&slug, &BTreeMap::from([(key.clone(), value.to_string())]))
                        .unwrap()
                };
                // As many characters as boxes: one per box.
                let value = &"1234567890"[..indices.len()];
                let document = fill(value);
                let inputs = plain_inputs(&document, cell);
                assert_eq!(inputs.len(), indices.len(), "{slug} {cell}");
                for (offset, (_, _, tag)) in inputs.iter().enumerate() {
                    assert!(!tag.contains("font-size"), "{slug} {cell}: {tag}");
                    assert_eq!(attr(tag, "value"), Some(&value[offset..=offset]));
                }
                assert!(!document.contains(&format!("data-overflow-of=\"{cell}\"")));
                // Longer: one line over the cell, the boxes empty.
                let long = "1234567890";
                let document = fill(long);
                for (_, _, tag) in plain_inputs(&document, cell) {
                    assert_eq!(attr(tag, "value"), Some(""), "{slug} {cell}: {tag}");
                }
                let line = printed_cells(&document).remove(*cell);
                let (text, size) = line.expect("an overflow line");
                let size = size.unwrap();
                // A figure is never cut: it prints whole, shrunk below the
                // 7.5 pt floor long text keeps if it must.
                assert_eq!(text, long, "{slug} {cell}");
                assert!(size > 0.0);
                let width = plain_cell_width(&document, cell).unwrap();
                if text.ends_with("...") {
                    assert!(document.contains("data-overflow-warning=\"text-truncated\""));
                } else {
                    assert_eq!(text, long);
                }
                for subline in text.lines() {
                    assert!(1.0 + sheet.width(sheet.letter, subline, size) <= width);
                }
                assert_eq!(
                    document
                        .matches(&format!("data-overflow-of=\"{cell}\""))
                        .count(),
                    1
                );
            }
        }
        found.sort();
        listed.sort();
        assert_eq!(found, listed, "every joined split cell is listed");
        assert_eq!(listed.len(), 9);
        // 1600WP's dates: "01" and "2024" one per box, "20240101" one line.
        let slug = "1600wp-2010";
        let loaded = bundle(slug).unwrap();
        let sheet = SheetType::of_sheet(slug, loaded.css);
        let fields = BTreeMap::from([
            ("derived:att_from_mm".to_string(), "01".to_string()),
            ("derived:att_from_yyyy".to_string(), "2024".to_string()),
            ("derived:att_to_yyyy".to_string(), "20240101".to_string()),
        ]);
        let document = filled_document(slug, &fields).unwrap();
        let boxes = |cell: &str| -> Vec<String> {
            plain_inputs(&document, cell)
                .iter()
                .map(|(_, _, tag)| {
                    assert!(!tag.contains("font-size"), "{tag}");
                    attr(tag, "value").unwrap().to_string()
                })
                .collect()
        };
        assert_eq!(boxes("p1c62"), ["0", "1"]);
        assert_eq!(boxes("p1c64"), ["2", "0", "2", "4"]);
        assert_eq!(boxes("p1c67"), ["", "", "", ""]);
        let (text, size) = printed_cells(&document).remove("p1c67").unwrap();
        assert_eq!(text, "20240101");
        let width = plain_cell_width(&document, "p1c67").unwrap();
        assert!(1.0 + sheet.width(sheet.letter, &text, size.unwrap()) <= width);
        assert_eq!(document.matches("data-overflow-of=\"p1c6").count(), 1);
    }

    /// The writer key of the text join that prints in `cell` of `slug`.
    fn key_of(slug: &str, cell: &str) -> String {
        writer_cells(slug)
            .unwrap()
            .joins
            .into_iter()
            .find(|(_, ids)| ids.iter().any(|id| id == cell))
            .unwrap()
            .0
    }

    /// The `<div id="{cell}" ...>...</div>` of `cell` in `document`.
    fn cell_html<'a>(document: &'a str, cell: &str) -> &'a str {
        let (open, _) = super::cell_tag(document, cell).unwrap();
        let close = open + document[open..].find("</div>").unwrap() + "</div>".len();
        &document[open..close]
    }

    /// Round 6, D3: values with a tab or a no-break space, printed. On a
    /// Tinos sheet (0605, 1604F) and an Arimo sheet (2200AN), a value that
    /// fits prints its cell byte for byte as dcc6386a's fill did (the
    /// literals are that fill's output, from a scratch binary on `git
    /// archive dcc6386a`); a longer one shrinks to set within its room less
    /// 1 pt, measured with the tab's stop and the no-break space's
    /// word-spacing.
    #[test]
    fn tab_and_no_break_space_values_print_as_measured() {
        let fitting: [(&str, &str, &str, &str, &str); 6] = [
            (
                "0605-1999",
                "frm0605:txtLineBus",
                "p1c33",
                "UNIT\t1203 TWELFTH FLOOR",
                P1C33_TAB,
            ),
            (
                "0605-1999",
                "frm0605:txtLineBus",
                "p1c33",
                "UNIT\u{a0}1203 TWELFTH FLOOR",
                P1C33_NBSP,
            ),
            (
                "1604f-2018",
                "frm1604f:txtSched1Date1",
                "p1c48",
                "UNIT\t1203 TWELFTH F",
                P1C48_TAB,
            ),
            (
                "1604f-2018",
                "frm1604f:txtSched1Date1",
                "p1c48",
                "UNIT\u{a0}1203 TWELFTH FLOO",
                P1C48_NBSP,
            ),
            (
                "2200an-2018",
                "frm2200ANv2018:txtXGOthers1Name",
                "p2c246",
                "UNIT\t1203 TWELFTH FLOOR CURVE BUILDIN",
                P2C246_TAB,
            ),
            (
                "2200an-2018",
                "frm2200ANv2018:txtXGOthers1Name",
                "p2c246",
                "UNIT\u{a0}1203 TWELFTH FLOOR CURVE BUILDIN",
                P2C246_NBSP,
            ),
        ];
        for (slug, key, cell, value, dcc6386a) in fitting {
            let document = filled_document(
                slug,
                &BTreeMap::from([(key.to_string(), value.to_string())]),
            )
            .unwrap();
            assert_eq!(
                cell_html(&document, cell),
                dcc6386a,
                "{slug} {cell} {value:?}"
            );
        }
        // Headless Chrome's widths (pt) of such values at their class
        // sizes, each set at 100 times its size, spacing included (an
        // `<input>`'s scrollWidth px × 0.75 / 100): the model's are within
        // 0.01 pt. 1604F's "UNIT<TAB>1203 TWELFTH FLOO" overflows its
        // 99.6 pt room: "UNIT" (16.62 pt) is past the first stop (16.51
        // pt), so the tab runs to the second.
        for (slug, cell, value, chrome) in [
            ("1604f-2018", "p1c48", "UNIT\t1203 TWELFTH FLOO", 101.715),
            (
                "1604f-2018",
                "p1c48",
                "UNIT\u{a0}1203 TWELFTH FLOO",
                87.3825,
            ),
            ("0605-1999", "p1c33", "UNIT\t1203 TWELFTH FLOOR", 149.94),
            (
                "0605-1999",
                "p1c33",
                "UNIT\u{a0}1203 TWELFTH FLOOR",
                137.145,
            ),
            (
                "2200an-2018",
                "p2c246",
                "UNIT\t1203 TWELFTH FLOOR CURVE BUILDIN",
                187.26,
            ),
            (
                "2200an-2018",
                "p2c246",
                "UNIT\u{a0}1203 TWELFTH FLOOR CURVE BUILDIN",
                172.7025,
            ),
        ] {
            let loaded = bundle(slug).unwrap();
            let sheet = SheetType::of_sheet(slug, loaded.css);
            let [(room, classes, _)] = plain_inputs(loaded.html, cell)[..] else {
                panic!("{slug} {cell}: one input");
            };
            let size = sheet.size_for(classes).unwrap();
            let model = sheet.width(sheet.letter_for(classes), value, size);
            assert!(
                (model - chrome).abs() <= 0.01,
                "{slug} {cell} {value:?}: {model} against Chrome's {chrome}"
            );
            let document = filled_document(
                slug,
                &BTreeMap::from([(key_of(slug, cell), value.to_string())]),
            )
            .unwrap();
            let [(_, _, tag)] = plain_inputs(&document, cell)[..] else {
                panic!("{slug} {cell}: one input");
            };
            assert_eq!(
                tag.contains("font-size"),
                chrome > room,
                "{slug} {cell} {value:?}: shrunk only past its {room} pt room"
            );
        }
        for (slug, key, cell, face) in [
            ("0605-1999", "frm0605:txtLineBus", "p1c33", Face::Tinos),
            (
                "2200an-2018",
                "frm2200ANv2018:txtXGOthers1Name",
                "p2c246",
                Face::Arimo,
            ),
        ] {
            let loaded = bundle(slug).unwrap();
            let sheet = SheetType::of_sheet(slug, loaded.css);
            assert_eq!(sheet.face, face);
            for space in ["\t", "\u{a0}"] {
                let value = format!(
                    "UNIT{space}1203 TWELFTH FLOOR CURVE BUILDING BONIFACIO GLOBAL CITY TAGUIG"
                );
                let document =
                    filled_document(slug, &BTreeMap::from([(key.to_string(), value.clone())]))
                        .unwrap();
                let [(room, classes, tag)] = plain_inputs(&document, cell)[..] else {
                    panic!("{slug} {cell}: one input");
                };
                let printed = attr(tag, "value").expect("printed value");
                if printed.ends_with("...") {
                    assert_eq!(attr(tag, "data-overflow-warning"), Some("text-truncated"));
                    assert_eq!(attr(tag, "data-writer-value"), Some(value.as_str()));
                } else {
                    assert_eq!(printed.replace("&#10;", " "), value);
                }
                let letter = sheet.letter_for(classes);
                let class_size = sheet.size_for(classes).unwrap();
                assert!(sheet.width(letter, &value, class_size) > room + FIT_EPSILON_PT);
                let style = attr(tag, "style").unwrap();
                let at = style.find("font-size:").expect("an inline size") + "font-size:".len();
                let size: f64 = style[at..].split("pt").next().unwrap().parse().unwrap();
                assert!(size <= class_size, "{slug} {cell} {space:?}");
                assert!(size >= PLAIN_TEXT_MIN_PT, "{slug} {cell} {space:?}");
                assert!(room >= 2.0);
                for line in printed.split("&#10;") {
                    assert!(
                        sheet.width(letter, line, size) <= room - 1.0,
                        "{slug} {cell} {space:?}: {line:?} at {size}pt"
                    );
                }
            }
        }
    }

    // dcc6386a's fill of the fitting values of
    // `tab_and_no_break_space_values_print_as_measured`, cell by cell.
    const P1C33_TAB: &str = "<div id=\"p1c33\" class=\"c f\" data-cell-kind=\"field\" data-row=\"14\" data-col=\"72\" data-field-kind=\"text\" data-field-name=\"p1c33\" style=\"left:418.44pt;top:246.12pt;width:158.4pt;height:18.96pt\"><input type=\"text\" class=\"fi fh22\" id=\"p1c33-i\" name=\"p1c33\" style=\"inset:0.75pt 0.75pt 0.75pt 0.75pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\t1203 TWELFTH FLOOR\" data-writer-value=\"UNIT\t1203 TWELFTH FLOOR\"></div>";
    const P1C33_NBSP: &str = "<div id=\"p1c33\" class=\"c f\" data-cell-kind=\"field\" data-row=\"14\" data-col=\"72\" data-field-kind=\"text\" data-field-name=\"p1c33\" style=\"left:418.44pt;top:246.12pt;width:158.4pt;height:18.96pt\"><input type=\"text\" class=\"fi fh22\" id=\"p1c33-i\" name=\"p1c33\" style=\"inset:0.75pt 0.75pt 0.75pt 0.75pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\u{a0}1203 TWELFTH FLOOR\" data-writer-value=\"UNIT\u{a0}1203 TWELFTH FLOOR\"></div>";
    const P1C48_TAB: &str = "<div id=\"p1c48\" class=\"c f\" data-cell-kind=\"field\" data-row=\"22\" data-col=\"1\" data-field-kind=\"text\" data-field-name=\"p1c48\" style=\"left:86.42pt;top:360.17pt;width:100.56pt;height:24.6pt\"><input type=\"text\" class=\"fi fh25\" id=\"p1c48-i\" name=\"p1c48\" style=\"inset:0.48pt 0.48pt 0.48pt 0.48pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\t1203 TWELFTH F\" data-writer-value=\"UNIT\t1203 TWELFTH F\"></div>";
    const P1C48_NBSP: &str = "<div id=\"p1c48\" class=\"c f\" data-cell-kind=\"field\" data-row=\"22\" data-col=\"1\" data-field-kind=\"text\" data-field-name=\"p1c48\" style=\"left:86.42pt;top:360.17pt;width:100.56pt;height:24.6pt\"><input type=\"text\" class=\"fi fh25\" id=\"p1c48-i\" name=\"p1c48\" style=\"inset:0.48pt 0.48pt 0.48pt 0.48pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\u{a0}1203 TWELFTH FLOO\" data-writer-value=\"UNIT\u{a0}1203 TWELFTH FLOO\"></div>";
    const P2C246_TAB: &str = "<div id=\"p2c246\" class=\"c f\" data-cell-kind=\"field\" data-row=\"38\" data-col=\"1\" data-field-kind=\"text\" data-field-name=\"p2c246\" style=\"left:60.72pt;top:685.18pt;width:189.41pt;height:14.76pt\"><input type=\"text\" class=\"fi fh7\" id=\"p2c246-i\" name=\"p2c246\" style=\"inset:0.48pt 0.48pt 0.48pt 0.48pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\t1203 TWELFTH FLOOR CURVE BUILDIN\" data-writer-value=\"UNIT\t1203 TWELFTH FLOOR CURVE BUILDIN\"></div>";
    const P2C246_NBSP: &str = "<div id=\"p2c246\" class=\"c f\" data-cell-kind=\"field\" data-row=\"38\" data-col=\"1\" data-field-kind=\"text\" data-field-name=\"p2c246\" style=\"left:60.72pt;top:685.18pt;width:189.41pt;height:14.76pt\"><input type=\"text\" class=\"fi fh7\" id=\"p2c246-i\" name=\"p2c246\" style=\"inset:0.48pt 0.48pt 0.48pt 0.48pt\" autocomplete=\"off\" spellcheck=\"false\" value=\"UNIT\u{a0}1203 TWELFTH FLOOR CURVE BUILDIN\" data-writer-value=\"UNIT\u{a0}1203 TWELFTH FLOOR CURVE BUILDIN\"></div>";
}

#[cfg(test)]
mod donor_estate_print_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// Every writer key a sheet's writer-cells.json joins, with a value
    /// each join kind prints (a deterministic stand-in for a draft).
    fn every_join_filled(slug: &str) -> BTreeMap<String, String> {
        let doc: serde_json::Value =
            serde_json::from_str(writer_cells_json(slug).unwrap()).unwrap();
        let mut fields = BTreeMap::new();
        for (section, value) in [
            ("joins", "ABC123"),
            ("money_joins", "1,234.56"),
            ("peso_joins", "1,234.56"),
            ("xbox_joins", "true"),
            ("date_joins", "06/15/2025"),
        ] {
            for row in doc[section].as_array().into_iter().flatten() {
                if let Some(key) = row["writer_key"].as_str() {
                    fields.insert(key.to_string(), value.to_string());
                }
            }
        }
        // PR 10's joins read their sources too: a value each prints, and
        // each date part as its form writes it.
        for (sheet, _, value) in DONOR_ESTATE_JOINS {
            if *sheet != slug {
                continue;
            }
            for key in donor_estate_sources(*value) {
                fields.insert(key.to_string(), "123".to_string());
            }
            if let SheetValue::Key(key) | SheetValue::Tick(key) = value {
                let shown = if matches!(value, SheetValue::Tick(_)) {
                    "true"
                } else {
                    "12"
                };
                fields.insert(key.to_string(), shown.to_string());
            }
        }
        for (key, part) in [
            ("frm1801v2018:txtDateMonth", "06"),
            ("frm1801v2018:txtDateDay", "15"),
            ("frm1801v2018:txtDateYear", "2025"),
            ("frm1800v2018:txtMonth", "06"),
            ("frm1800v2018:txtDate", "15"),
            ("frm1800v2018:txtYear", "2025"),
        ] {
            if fields.contains_key(key) || key.contains(&slug[..4]) {
                fields.insert(key.to_string(), part.to_string());
            }
        }
        fields
    }

    fn digest(text: &str) -> String {
        Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// A deterministic value in every key writer-cells.json joins (and, on
    /// 1800 and 1801, every source PR 10's joins read, with each date part
    /// as the form writes it), filled and hashed. All four digests were
    /// re-pinned on the 12b merge into integration: 12b's per-sheet fitting
    /// (P4-7) moved 1706's and 2000-OT's long-text print, and PR 10's joins
    /// moved 1800's and 1801's. With the donor joins disabled, the merged
    /// fill reproduces every pre-merge golden byte for byte — the merge
    /// changed nothing else.
    #[test]
    fn pr10_joins_leave_every_other_print_byte_for_byte() {
        for (slug, golden) in [
            (
                "1706-2018",
                "b1161301be938a1b947987814010acd9f096ad811c70e3f8d5ec2e4fe85be8b5",
            ),
            (
                "2000-ot-2018",
                "02ff6f2d8df0cac5bed665828020396013572f25e53b59e0b1a2ebdf185baaa6",
            ),
            (
                "1801-2018",
                "afa7e865fe386c53fc53b6b8636932de8e1edd3aeabdf7972644399f5046f02a",
            ),
            (
                "1800-2018",
                "437c7fdb7235f990a065b286f60ac9e9a5681b3b0e76e4a3b080dd66236c102b",
            ),
        ] {
            let html = filled_document(slug, &every_join_filled(slug)).unwrap();
            assert_eq!(digest(&html), golden, "{slug}");
        }
        // Only 1800 and 1801 have PR 10 joins.
        assert!(DONOR_ESTATE_JOINS
            .iter()
            .all(|(slug, _, _)| matches!(*slug, "1800-2018" | "1801-2018")));
    }

    #[test]
    fn field_rules_svg_removal_records_hidden_rules_splits_shared_and_supports_clones() {
        use std::collections::BTreeMap;
        let page = r##"<div class="page page-1"><svg class="rl"><rect x="30" y="102" width="0.5" height="12" fill="#000" data-rule-id="v_inner"/><rect x="50" y="80" width="0.5" height="60" fill="#000" data-rule-id="v_shared"/><rect x="10" y="100" width="0.5" height="16" fill="#000" data-rule-id="v_outer_left"/></svg><div id="cont_sched1_r2_p1c10" style="left:10pt;top:100pt;width:80pt;height:16pt"><input type="text" name="cont_sched1_r2_p1c10" data-slot-index="0" value=""><input type="text" name="cont_sched1_r2_p1c10" data-slot-index="1" value=""><input type="text" name="cont_sched1_r2_p1c10" data-slot-index="2" value=""></div></div>"##;
        let mut fields = BTreeMap::new();
        fields.insert("k".to_string(), "OVERFLOWING TEXT".to_string());
        let mut cells = BTreeMap::new();
        cells.insert("k".to_string(), vec!["cont_sched1_r2_p1c10".to_string()]);
        let filled = super::fill_by_name_with_cells(page, &fields, &cells, &BTreeMap::new());
        let out = super::strip_converted_rect_separators(&filled);
        // Interior separator removed
        assert!(!out.contains("data-rule-id=\"v_inner\""));
        // Outer border kept
        assert!(out.contains("data-rule-id=\"v_outer_left\""));
        // Shared vertical rule spanning rows above & below is split into two pieces, not deleted
        assert_eq!(out.matches("data-rule-id=\"v_shared\"").count(), 2);
        // Hidden SVG rule IDs recorded on owning cloned cell
        assert!(out.contains("data-hidden-svg-rules=\"v_inner,v_shared\""));
        assert!(out.contains("data-overflow-of=\"cont_sched1_r2_p1c10\""));
    }

    #[test]
    fn field_rules_rate_centered_space_percent_and_space_over_remittance() {
        assert_eq!(
            super::format_percentage_rate("5.00", false).as_deref(),
            Some("5 %")
        );
        assert_eq!(
            super::format_percentage_rate("2.50", false).as_deref(),
            Some("2.5 %")
        );
        assert_eq!(
            super::format_percentage_rate("15.00", true).as_deref(),
            Some("15")
        );
    }

    fn fixture_profile() -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "Print Fixture Estate", "tin": {"segment1": "123",
            "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
            "line_of_business": "Brokerage", "registered_address": "1 Fixture St",
            "zip_code": "1100", "phone": "0281234567", "email": "fixture@example.com",
            "default_form_type": "1801", "taxpayer_type": "Individual"
        }))
        .unwrap()
    }

    fn estate() -> bir_core::forms::form_1801::Form1801Draft {
        use bir_core::forms::form_1801::{Form1801Draft, Form1801Frequency};
        let mut draft = Form1801Draft::new_from_profile(&fixture_profile(), 2026, 1);
        draft.death_month = 6;
        draft.death_day = 15;
        draft.death_year = 2025;
        draft.administrator_tin = "987654321".into();
        draft.administrator_branch_code = "00001".into();
        draft.tax_relief = Some("RA 9504".into());
        draft.installment_granted = true;
        draft.installment_frequency = Some(Form1801Frequency::Others);
        draft.installment_frequency_other = "Annual".into();
        draft.installment_year = "2027".into();
        draft.recompute();
        draft
    }

    fn comb_text(html: &str, name: &str) -> String {
        named_values(html, name).concat()
    }

    fn named_values(html: &str, name: &str) -> Vec<String> {
        input_tags(html)
            .into_iter()
            .filter(|tag| tag.name == name)
            .map(|tag| attr(tag.tag, "value").unwrap_or("").to_string())
            .collect()
    }

    /// PR 10 (1801 manual-0): item 1 prints the date of death in its one
    /// 6-slot comb p1c4 as the sheet's MM/DD/YY; a date not chosen prints
    /// nothing.
    #[test]
    fn pr10_1801_item_1_prints_the_date_of_death() {
        let draft = estate();
        let html = filled_document("1801-2018", &draft.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "061525");
        let mut blank = estate();
        blank.death_month = 0;
        blank.death_day = 0;
        blank.death_year = 0;
        let upload = blank.to_bir_field_map();
        assert_eq!(upload["frm1801v2018:txtDateMonth"], "00");
        let fields = blank.to_print_field_map();
        assert_eq!(fields["frm1801v2018:txtDateMonth"], "");
        let html = filled_document("1801-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c4"), "");
    }

    /// PR 10: what the 1801 page holds prints in its cell: the executor's
    /// TIN and branch (item 11), the contact number (12), 14A, 15D's
    /// frequency tick and its Others text, and item 21's year. Every value
    /// the print map carries still has a box or a reviewed reason.
    #[test]
    fn pr10_1801_page_values_print_in_their_cells() {
        use bir_core::forms::form_1801::Form1801Frequency;
        let draft = estate();
        let fields = draft.to_print_field_map();
        let html = filled_document("1801-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c31"), "987654321");
        assert_eq!(comb_text(&html, "p1c32"), "00001");
        assert_eq!(comb_text(&html, "p1c33"), "0281234567");
        assert_eq!(comb_text(&html, "p1c38"), "RA 9504");
        assert_eq!(named_values(&html, "p1c52"), vec!["X".to_string()]);
        for other in ["p1c50", "p1c51", "p1c53"] {
            assert_eq!(named_values(&html, other), vec![String::new()], "{other}");
        }
        assert_eq!(named_values(&html, "p1c197"), vec!["ANNUAL".to_string()]);
        assert_eq!(named_values(&html, "p1c201"), vec!["2027".to_string()]);
        assert!(
            unprinted_keys("1801-2018", &fields).unwrap().is_empty(),
            "{:?}",
            unprinted_keys("1801-2018", &fields)
        );
        // Each choice ticks its own box.
        for (frequency, cell) in [
            (Form1801Frequency::Monthly, "p1c50"),
            (Form1801Frequency::Quarterly, "p1c53"),
            (Form1801Frequency::SemiAnnual, "p1c51"),
        ] {
            let mut picked = estate();
            picked.installment_frequency = Some(frequency);
            let html = filled_document("1801-2018", &picked.to_print_field_map()).unwrap();
            assert_eq!(named_values(&html, cell), vec!["X".to_string()], "{cell}");
            assert_eq!(named_values(&html, "p1c52"), vec![String::new()]);
            assert_eq!(named_values(&html, "p1c197"), vec![String::new()]);
        }
        // No executor TIN, no relief: the cells stay blank.
        let mut none = estate();
        none.administrator_tin.clear();
        none.tax_relief = None;
        let html = filled_document("1801-2018", &none.to_print_field_map()).unwrap();
        assert_eq!(comb_text(&html, "p1c31"), "");
        assert_eq!(comb_text(&html, "p1c38"), "");
    }

    /// PR 10: 1800's 13A and 8A ZIP print in their cells.
    #[test]
    fn pr10_1800_13a_and_8a_print_in_their_cells() {
        use bir_core::forms::form_1800::Form1800Draft;
        let mut draft = Form1800Draft::new_from_profile(&fixture_profile(), 2026, 1);
        draft.tax_relief = Some("RA 9504".into());
        draft.recompute();
        let fields = draft.to_print_field_map();
        let html = filled_document("1800-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c57"), "RA 9504");
        assert_eq!(comb_text(&html, "p1c26"), "1100");
        assert!(unprinted_keys("1800-2018", &fields).unwrap().is_empty());
        // A relief longer than the comb prints whole on one line.
        draft.tax_relief = Some("REPUBLIC ACT NO. 9504 SECTION 4".into());
        let html = filled_document("1800-2018", &draft.to_print_field_map()).unwrap();
        assert!(html.contains("value=\"REPUBLIC ACT NO. 9504 SECTION 4\""));
    }

    fn donor(month: u8, day: u8, year: u16) -> BTreeMap<String, String> {
        use bir_core::forms::form_1800::Form1800Draft;
        let mut draft = Form1800Draft::new_from_profile(&fixture_profile(), 2026, 1);
        draft.donation_month = month;
        draft.donation_day = day;
        draft.donation_year = year;
        draft.recompute();
        draft.to_print_field_map()
    }

    /// Second fix check, 1800 item 1: the YY comb p1c7 prints the year's
    /// last two digits (form_1800.rs writes the 4-digit year); a part not
    /// chosen prints nothing, as the page shows it blank; a typed part
    /// prints in its own slots.
    #[test]
    fn pr10_1800_item_1_prints_as_the_page_shows_it() {
        let parts = |fields: &BTreeMap<String, String>| {
            let html = filled_document("1800-2018", fields).unwrap();
            ["p1c5", "p1c6", "p1c7"].map(|cell| comb_text(&html, cell))
        };
        let whole = donor(6, 15, 2025);
        assert_eq!(whole["frm1800v2018:txtYear"], "2025", "the upload keeps it");
        assert_eq!(parts(&whole), ["06", "15", "25"]);
        assert_eq!(parts(&donor(3, 1, 2009)), ["03", "01", "09"]);
        let fresh = donor(0, 0, 0);
        assert_eq!(
            [
                &fresh["frm1800v2018:txtMonth"],
                &fresh["frm1800v2018:txtDate"],
                &fresh["frm1800v2018:txtYear"]
            ],
            ["", "", ""]
        );
        assert_eq!(parts(&fresh), ["", "", ""]);
        assert_eq!(parts(&donor(6, 0, 0)), ["06", "", ""]);
        assert!(unprinted_keys("1800-2018", &fresh).unwrap().is_empty());
    }

    /// Second fix check, 1801 item 1: each typed part prints in its own
    /// MM / DD / YY slots of p1c4; a part not chosen leaves its slots
    /// blank and is no unprinted value (the page shows it blank); a part
    /// that cannot print is reported.
    #[test]
    fn pr10_1801_item_1_prints_each_typed_part() {
        let slots = |draft: &bir_core::forms::form_1801::Form1801Draft| {
            let html = filled_document("1801-2018", &draft.to_print_field_map()).unwrap();
            named_values(&html, "p1c4")
        };
        let mut month = estate();
        month.death_day = 0;
        month.death_year = 0;
        assert_eq!(slots(&month), ["0", "6", "", "", "", ""]);
        let fields = month.to_print_field_map();
        assert!(unprinted_keys("1801-2018", &fields).unwrap().is_empty());
        let mut year = estate();
        year.death_month = 0;
        year.death_day = 0;
        assert_eq!(slots(&year), ["", "", "", "", "2", "5"]);
        let mut odd = fields.clone();
        odd.insert("frm1801v2018:txtDateMonth".into(), "123".into());
        assert_eq!(
            unprinted_keys("1801-2018", &odd).unwrap(),
            ["frm1801v2018:txtDateMonth"]
        );
    }

    /// Second fix check: 1800's 8A ZIP prints trimmed, and 1801's executor
    /// branch code fills its 5 slots ("1" prints "00001"); the upload keeps
    /// what the draft holds.
    #[test]
    fn pr10_zip_and_branch_print_in_their_slots() {
        let mut fields = donor(6, 15, 2025);
        fields.insert("frm1800v2018:txtZipCode".into(), " 1100 ".into());
        let html = filled_document("1800-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c26"), "1100");
        let mut draft = estate();
        draft.administrator_branch_code = "1".into();
        let fields = draft.to_print_field_map();
        assert_eq!(fields["frm1801v2018:txtBranchCodeE"], "1");
        assert_eq!(draft.to_bir_field_map()["frm1801v2018:txtBranchCodeE"], "1");
        let html = filled_document("1801-2018", &fields).unwrap();
        assert_eq!(comb_text(&html, "p1c32"), "00001");
    }

    #[test]
    fn plain_money_inputs_right_align_with_inset_while_non_money_stays_left() {
        // 1800 Schedule A Fair Market Value (money) vs Schedule B2 Area (non-money)
        let mut fields = donor(6, 15, 2025);
        fields.insert(
            "frm1800v2018:schedA:txtFairMarketValue0".into(),
            "150,000.00".into(),
        );
        fields.insert("frm1800v2018:schedB2:txtArea0".into(), "250.00".into());
        fields.insert("frm1800v2018:schedB2:txtFTD0".into(), "1,200,000.00".into());
        let html = filled_document("1800-2018", &fields).unwrap();
        let fmv_tag = super::input_tags(&html)
            .into_iter()
            .find(|t| t.name == "p2c76")
            .expect("p2c76 input")
            .tag
            .to_string();
        assert!(
            fmv_tag.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "Schedule A FMV should right-align: {fmv_tag}"
        );
        let ftd_tag = super::input_tags(&html)
            .into_iter()
            .find(|t| t.name == "p2c179")
            .expect("p2c179 input")
            .tag
            .to_string();
        assert!(
            ftd_tag.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "Schedule B2 FMV per TD should right-align: {ftd_tag}"
        );
        let area_tag = super::input_tags(&html)
            .into_iter()
            .find(|t| t.name == "p2c178")
            .expect("p2c178 input")
            .tag
            .to_string();
        assert!(
            area_tag.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "Schedule B2 Area (numeric quantity) should right-align: {area_tag}"
        );

        // 2552 Schedule 1 & Part VI Number of Shares and 1706 Schedule 1 Area
        let mut f2552 = BTreeMap::new();
        f2552.insert("frm2552:txtPg2Pt5Sch1NumShares1".into(), "150000".into());
        f2552.insert("frm2552:txtPg2Pt6NumShares1".into(), "25,000".into());
        f2552.insert("frm2552:txtPg2Pt5Sch1Seller1".into(), "SELLER CORP".into());
        let html_2552 = filled_document("2552-2018", &f2552).unwrap();
        let sch1_shares = super::input_tags(&html_2552)
            .into_iter()
            .find(|t| t.name == "p2c48")
            .expect("p2c48 input")
            .tag
            .to_string();
        assert!(
            sch1_shares.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "2552 Schedule 1 Number of Shares should right-align: {sch1_shares}"
        );
        let seller_tag = super::input_tags(&html_2552)
            .into_iter()
            .find(|t| t.name == "p2c23")
            .expect("p2c23 input")
            .tag
            .to_string();
        assert!(
            !seller_tag.contains("text-align:right"),
            "2552 Schedule 1 Seller (text) must stay left-aligned: {seller_tag}"
        );

        let mut f1706 = BTreeMap::new();
        f1706.insert("frm1706:txtArea".into(), "1,250.50".into());
        let html_1706 = filled_document("1706-2018", &f1706).unwrap();
        let area_1706 = super::input_tags(&html_1706)
            .into_iter()
            .find(|t| t.name == "p2c42")
            .expect("p2c42 input")
            .tag
            .to_string();
        assert!(
            area_1706.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "1706 Schedule 1 Area should right-align: {area_1706}"
        );

        // 1600WP Part II tax base (money) vs Schedule 1.1 rate (non-money)
        let mut f1600 = BTreeMap::new();
        f1600.insert("derived:WB191_base".into(), "500,000.00".into());
        f1600.insert("frm1600WP:dtSched:txtRatePercent1".into(), "20.00".into());
        f1600.insert("frm1600WP:txtTaxpayerName".into(), "SAMPLE CORP".into());
        let html_wp = filled_document("1600wp-2010", &f1600).unwrap();
        let base_tag = super::input_tags(&html_wp)
            .into_iter()
            .find(|t| t.name == "p1c32")
            .expect("p1c32 input")
            .tag
            .to_string();
        assert!(
            base_tag.contains("text-align:right;justify-content:flex-end;padding-right:2pt"),
            "1600WP tax base should right-align: {base_tag}"
        );
        let rate_tag = super::input_tags(&html_wp)
            .into_iter()
            .find(|t| t.name == "p1c64")
            .expect("p1c64 input")
            .tag
            .to_string();
        assert!(
            !rate_tag.contains("text-align:right"),
            "1600WP tax rate (non-money) must stay left-aligned: {rate_tag}"
        );
    }

    #[test]
    fn rule3_2200p_others_wraps_at_7_5pt_and_never_drops_below_floor() {
        let mut fields = BTreeMap::new();
        let long_desc = "REFINED PETROLEUM NAPHTHA AND SPECIALIZED INDUSTRIAL LUBRICANT BLEND";
        fields.insert("frm2200P:txtSched1Description26".into(), long_desc.into());
        fields.insert(
            "frm2200P:txtSched1Description27".into(),
            "REFINED PETROLEUM NAPHTHA".into(),
        );
        let html = filled_document("2200p-2020", &fields).unwrap();

        let p2c377 = super::input_tags(&html)
            .into_iter()
            .find(|t| t.name == "p2c377")
            .expect("2200P Others row 1 description");
        assert!(
            p2c377.tag.contains("font-size:8.55pt"),
            "2200P Others row 1 must stay at >= 7.50pt A4 floor (8.55pt on folio): {}",
            p2c377.tag
        );
        assert!(
            p2c377.tag.contains("..."),
            "2200P Others row 1 must ellipsize at the A4 floor: {}",
            p2c377.tag
        );
        assert!(
            p2c377
                .tag
                .contains("data-overflow-warning=\"text-truncated\""),
            "2200P Others row 1 must trigger the additional sheet: {}",
            p2c377.tag
        );
        assert!(
            html.contains("page-continuation") && html.contains(long_desc),
            "2200P full Others description must appear on the additional sheet"
        );

        let p2c391 = super::input_tags(&html)
            .into_iter()
            .find(|t| t.name == "p2c391")
            .expect("2200P Others row 2 description");
        assert!(
            p2c391.tag.contains("..."),
            "2200P Others row 2 (16.68pt tall on folio) must ellipsize at the 8.55pt A4 floor rather than dropping below 7.5pt: {}",
            p2c391.tag
        );
        let sz = super::attr(p2c391.tag, "style")
            .and_then(|s| s.split("font-size:").nth(1))
            .and_then(|s| s.split("pt").next())
            .and_then(|s| s.parse::<f64>().ok())
            .expect("font-size on p2c391");
        assert!(
            sz >= 8.55,
            "p2c391 font-size {sz}pt went below 7.5pt A4 floor"
        );
    }

    #[test]
    fn form_2000ot_item_16_tax_rate_centers_default_and_atc_rate() {
        for value in ["0.00", "P15 / P1,000", "P1.50 / P200"] {
            let mut fields = BTreeMap::new();
            fields.insert("frm2000OT:txtTax16".to_string(), value.to_string());
            let html = filled_document("2000-ot-2018", &fields).expect("fill 2000OT");
            let tag = super::input_tags(&html)
                .into_iter()
                .find(|t| t.name == "p1c58")
                .expect("2000OT item 16 input");
            assert!(
                tag.tag.contains("text-align:center;justify-content:center"),
                "2000OT item 16 ({value}) must be centered: {}",
                tag.tag
            );
            assert!(
                !tag.tag.contains("text-align:right"),
                "2000OT item 16 ({value}) must not be right-aligned: {}",
                tag.tag
            );
            assert!(
                tag.tag.contains(&format!("value=\"{value}\"")),
                "2000OT item 16 ({value}) must keep its rate value without appending %: {}",
                tag.tag
            );
        }
    }

    #[test]
    fn form_1702mx_schedules_2_and_3_whole_peso_zeros_right_align() {
        let mut fields = BTreeMap::new();
        fields.insert("frm1702MX:txtPg2Sc2Itm1A".to_string(), "0".to_string());
        fields.insert(
            "frm1702MX:txtPg2Sc2Itm1B".to_string(),
            "150,000".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg2Sc2Itm5C".to_string(),
            "(50,000)".to_string(),
        );
        fields.insert("frm1702MX:txtPg2Sc3It20A".to_string(), "0".to_string());
        let html = filled_document("1702mx-2018c", &fields).expect("fill 1702MX");
        for id in ["p2c70", "p2c71", "p2c92", "p2c180"] {
            let tag = super::input_tags(&html)
                .into_iter()
                .find(|t| t.name == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert!(
                tag.tag
                    .contains("text-align:right;justify-content:flex-end"),
                "1702MX {id} whole-peso cell must be right-aligned: {}",
                tag.tag
            );
        }
    }

    #[test]
    fn continuation_page_headers_keep_gap_between_tin_and_name() {
        let long_header = bir_core::forms::print_map::tin_name_header(
            "000-123-453-00000",
            "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA",
        );
        for slug in ["2200p-2020", "2000-dst-2018", "2200a-2020", "2200c-2018"] {
            let mut fields = BTreeMap::new();
            fields.insert("derived:page_header".to_string(), long_header.clone());
            let html = filled_document(slug, &fields).unwrap_or_else(|e| panic!("{slug}: {e}"));
            assert!(
                html.contains("00012345300000  SANTOS, JUANA MARIE"),
                "{slug} overflowing page_header must separate TIN and name with a gap"
            );
        }
    }

    #[test]
    fn form_1701q_page2_header_tin_does_not_crush_next_to_preprinted_branch() {
        for (tin, expect_branch) in [
            ("000123453", None),
            ("00012345300000", None),
            ("00012345300001", Some("00001")),
        ] {
            let mut fields = BTreeMap::new();
            fields.insert("derived:tin_digits".to_string(), tin.to_string());
            let html = filled_document("1701q-2018", &fields).expect("fill 1701Q");
            let tags: Vec<_> = super::input_tags(&html)
                .into_iter()
                .filter(|t| t.name == "p2c5" || t.name == "p2c5-line")
                .collect();
            assert!(
                !tags.iter().any(|t| t.name == "p2c5-line"),
                "1701Q p2c5 ({tin}) must not overflow into a crushed plain line"
            );
            let digits: String = tags
                .iter()
                .filter_map(|t| super::attr(t.tag, "value"))
                .collect();
            match expect_branch {
                None => assert_eq!(digits, "000123453", "1701Q p2c5 ({tin})"),
                Some(br) => assert_eq!(digits, format!("000123453{br}"), "1701Q p2c5 ({tin})"),
            }
        }
    }

    #[test]
    fn form_1702q_part2_prints_whole_pesos_without_centavos_or_doubled_commas() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:txtTax14".to_string(), "      875000".to_string());
        fields.insert("derived:txtTax25".to_string(), "      739265".to_string());
        fields.insert("derived:txtTax15".to_string(), "1234567890123".to_string());
        let html = filled_document("1702q-2018", &fields).expect("fill 1702Q");
        let values_of = |cell: &str| -> String {
            super::input_tags(&html)
                .into_iter()
                .filter(|t| t.name == cell)
                .filter_map(|t| super::attr(t.tag, "value"))
                .collect()
        };
        // Uniform sections (#477): item 15 overflows the column Part II
        // shares, so items 14 and 25 print as lines too, comma-formatted.
        assert_eq!(values_of("p1c57"), "");
        assert_eq!(values_of("p1c79"), "");
        assert_eq!(printed_cell(&html, "p1c57").0, "875,000");
        assert_eq!(printed_cell(&html, "p1c79").0, "739,265");
        assert!(
            html.contains("data-overflow-of=\"p1c57\"")
                && html.contains("data-overflow-of=\"p1c79\""),
            "1702Q Part II items 14 and 25 must convert with item 15 (#477)"
        );
        assert!(
            html.contains("data-overflow-of=\"p1c59\"")
                && html.contains("value=\"1,234,567,890,123\"")
                && html.contains("text-align:right;justify-content:flex-end"),
            "13-digit 1702Q Part II amount must print as right-aligned comma-formatted whole-peso plain rectangle"
        );
    }

    #[test]
    fn no_frozen_sheet_input_is_claimed_by_two_writer_keys() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
        let mut slugs: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry
                    .path()
                    .join("writer-cells.json")
                    .exists()
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        slugs.sort();
        assert_eq!(slugs.len(), 43);
        for slug in slugs {
            let html = bundle(&slug).unwrap().html;
            let cells = writer_cells(&slug).unwrap();
            let conflicts = writer_input_claim_conflicts(html, &cells);
            if let Some((index, keys)) = conflicts.first() {
                let mut keys = keys.iter();
                let first = keys.next().unwrap();
                let second = keys.next().unwrap();
                panic!(
                    "{slug}: input {} is claimed by writer keys {first} and {second}",
                    input_tags(html)[*index].name
                );
            }
        }
    }

    #[test]
    fn two_writer_keys_claiming_one_input_are_found_in_the_map_scan() {
        let html = r#"<input type="text" id="p1c1-i" name="p1c1">"#;
        let mut cells = WriterCells::empty();
        cells
            .joins
            .insert("writer.one".to_string(), vec!["p1c1".to_string()]);
        cells
            .joins
            .insert("writer.two".to_string(), vec!["p1c1".to_string()]);
        let conflicts = writer_input_claim_conflicts(html, &cells);
        let (index, keys) = conflicts.first().expect("one conflicting input");
        assert_eq!(input_tags(html)[*index].name, "p1c1");
        assert!(keys.contains("writer.one"));
        assert!(keys.contains("writer.two"));
    }

    #[test]
    fn overlapping_raw_cell_write_loses_to_the_writer_key() {
        let mut fields = every_join_filled("1701-2018");
        fields.insert(
            "frm1701:txtPg4IPart7_9Specify".to_string(),
            "writer value".to_string(),
        );
        fields.insert("p4c213".to_string(), "raw value".to_string());
        let html = filled_document("1701-2018", &fields).expect("overlapping raw write");
        assert_eq!(printed_text(&html, "p4c213"), "WRITER VALUE");
    }

    #[test]
    fn overlapping_byte_ranges_are_an_error_not_a_panic() {
        let err = apply_replacements_checked(
            "abcdef",
            vec![(1, 4, "one".to_string()), (2, 3, "two".to_string())],
        )
        .expect_err("overlapping byte ranges");
        assert!(err.contains("overlapping print writes"), "{err}");
        assert!(err.contains("2..3"), "{err}");
    }

    #[test]
    fn two_writer_keys_for_one_input_are_refused_naming_keys_and_cell() {
        let html = r#"<input type="text" id="p1c1-i" name="p1c1">"#;
        let mut extra_joins = BTreeMap::new();
        extra_joins.insert("writer.one".to_string(), vec!["p1c1".to_string()]);
        extra_joins.insert("writer.two".to_string(), vec!["p1c1".to_string()]);
        let mut fields = BTreeMap::new();
        fields.insert("writer.one".to_string(), "one".to_string());
        fields.insert("writer.two".to_string(), "two".to_string());
        let err = fill_continuation_fragment(
            html,
            &fields,
            "1701-2018",
            &extra_joins,
            &[],
            &[],
            &BTreeMap::new(),
        )
        .expect_err("two writer keys on one input");
        assert!(err.contains("writer.one"), "{err}");
        assert!(err.contains("writer.two"), "{err}");
        assert!(err.contains("p1c1"), "{err}");
    }

    #[test]
    fn second_person_tins_print_all_fourteen_digits_including_branch_code() {
        let cases = [
            (
                "1701-2018",
                "frm1701:txtPg2I1TIN1",
                "frm1701:txtPg2I1TIN2",
                "frm1701:txtPg2I1TIN3",
                "frm1701:txtPg2I1BranchCode",
            ),
            (
                "1700-2018",
                "frm1700:txtPg1I18STIN1",
                "frm1700:txtPg1I18STIN2",
                "frm1700:txtPg1I18STIN3",
                "frm1700:txtPg1I18SBranchCode",
            ),
            (
                "1701a-2018",
                "frm1701A:txtSpouseTIN1",
                "frm1701A:txtSpouseTIN2",
                "frm1701A:txtSpouseTIN3",
                "frm1701A:txtSpouseBranchCode",
            ),
            (
                "1701q-2018",
                "frm1701q:txtSpouseTIN1",
                "frm1701q:txtSpouseTIN2",
                "frm1701q:txtSpouseTIN3",
                "frm1701q:txtSpouseBranchCode",
            ),
        ];
        for (slug, k1, k2, k3, kb) in cases {
            let mut fields = BTreeMap::new();
            fields.insert(k1.to_string(), "111".to_string());
            fields.insert(k2.to_string(), "222".to_string());
            fields.insert(k3.to_string(), "333".to_string());
            fields.insert(kb.to_string(), "00001".to_string());
            let html = filled_document(slug, &fields).unwrap_or_else(|e| panic!("{slug}: {e}"));
            let branch_digits: String = super::input_tags(&html)
                .into_iter()
                .filter(|t| t.name == kb)
                .filter_map(|t| super::attr(t.tag, "value"))
                .collect();
            assert_eq!(
                branch_digits, "00001",
                "{slug} second-person branch code {kb} must print over preprinted 00000"
            );
        }
    }

    #[test]
    fn form_1701_schedule_3a_ten_digit_overflow_and_nine_digit_neighbor() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm1701:txtPg2IShed3_8A".to_string(),
            "1234567890".to_string(),
        );
        fields.insert(
            "frm1701:txtPg2IShed3_8B".to_string(),
            "987654321".to_string(),
        );
        let html = filled_document("1701-2018", &fields).expect("1701-2018 renders");
        let tags = super::input_tags(&html);

        // 10-digit amount overflows the 9-slot comb into a plain right-aligned
        // comma-formatted rectangle (RULES 2).
        let marker = "data-overflow-of=\"p2c98\"";
        let pos = html
            .find(marker)
            .expect("10-digit Schedule 3.A 8A must render as a plain overflow rectangle");
        let start = html[..pos].rfind("<input").expect("overflow <input");
        let end = pos + html[pos..].find('>').expect("closing >") + 1;
        let overflow_tag = &html[start..end];
        assert_eq!(super::attr(overflow_tag, "value"), Some("1,234,567,890"));
        let style = super::attr(overflow_tag, "style").unwrap_or("");
        assert!(
            style.contains("text-align:right") && style.contains("justify-content:flex-end"),
            "10-digit overflow must right-align, got: {style}"
        );
        assert!(
            super::attr(overflow_tag, "data-rect-box").is_some(),
            "10-digit overflow must strip internal comb ticks via data-rect-box"
        );

        // Neighboring 9-digit amount keeps its 9 comb boxes untouched.
        assert!(
            !html.contains("data-overflow-of=\"p2c99\""),
            "9-digit neighbor 8B must stay in its comb boxes"
        );
        let comb_digits: String = tags
            .iter()
            .filter(|t| t.name == "p2c99")
            .filter_map(|t| super::attr(t.tag, "value"))
            .collect();
        assert_eq!(comb_digits, "987654321");
    }

    #[test]
    fn form_1701_schedule_6_amounts_right_align_with_commas_and_respect_75pt_floor() {
        let mut fields = BTreeMap::new();
        fields.insert("frm1701:txtPg3IShed6_4Year".to_string(), "2023".to_string());
        fields.insert("frm1701:txtPg3IShed6_4A".to_string(), "1250000".to_string());
        fields.insert("frm1701:txtPg3IShed6_4B".to_string(), "250000".to_string());
        fields.insert("frm1701:txtPg3IShed6_4C".to_string(), "0".to_string());
        fields.insert("frm1701:txtPg3IShed6_4D".to_string(), "500000".to_string());
        fields.insert("frm1701:txtPg3IShed6_4E".to_string(), "500000".to_string());
        fields.insert("frm1701:txtPg3IShed6_8D".to_string(), "1500000".to_string());
        fields.insert("frm1701:txtPg4IShed6_9A".to_string(), "850000".to_string());
        fields.insert("frm1701:txtPg4IShed6_13D".to_string(), "425000".to_string());
        let html = filled_document("1701-2018", &fields).expect("1701-2018 renders");
        let tags = super::input_tags(&html);

        // Year Incurred stays unformatted (no comma, not right-aligned as money).
        let year = tags
            .iter()
            .find(|t| t.name == "p3c157")
            .expect("p3c157 year incurred");
        assert_eq!(super::attr(year.tag, "value"), Some("2023"));

        // Every Schedule 6 amount cell right-aligns with thousands commas and no centavos.
        for (cell_id, expected) in [
            ("p3c158", "1,250,000"),
            ("p3c159", "250,000"),
            ("p3c160", "0"),
            ("p3c161", "500,000"),
            ("p3c162", "500,000"),
            ("p3c186", "1,500,000"),
            ("p4c18", "850,000"),
            ("p4c46", "425,000"),
        ] {
            let tag = tags
                .iter()
                .find(|t| t.name == cell_id)
                .unwrap_or_else(|| panic!("missing Schedule 6 cell {cell_id}"));
            assert_eq!(
                super::attr(tag.tag, "value"),
                Some(expected),
                "Schedule 6 cell {cell_id} must format whole pesos with commas"
            );
            let style = super::attr(tag.tag, "style").unwrap_or("");
            assert!(
                style.contains("text-align:right") && style.contains("justify-content:flex-end"),
                "Schedule 6 cell {cell_id} must right-align, got: {style}"
            );
        }

        // No 4.5px / sub-7.5pt print-overrides shrink rule remains on p3c186-i.
        assert!(
            !html.contains("#p3c186-i"),
            "print-overrides must not shrink #p3c186-i below 7.5pt"
        );
    }

    #[test]
    fn form_1701a_part_ivb_additional_sheet_omits_stray_zero_centavo_boxes() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm1701A:txtTaxpayerName".to_string(),
            "SANTOS, JUANA MARIE".to_string(),
        );
        for n in 1..=4 {
            fields.insert(
                format!("frm1701A:txtmodalPartIVBDesc_{n}C1"),
                format!("OTHER INCOME ITEM {n}"),
            );
            fields.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C2"),
                format!("{}.00", n * 50_000),
            );
            // Unused spouse column carries "0.00" in the official modal payload.
            fields.insert(
                format!("frm1701A:txtmodalPartIVB_{n}C3"),
                "0.00".to_string(),
            );
        }
        let html = filled_document("1701a-2018", &fields).expect("1701a-2018 renders");
        let tags = super::input_tags(&html);

        // Overflow rows 3 and 4 print on the band continuation page.
        let cont_p2c64_digits: String = tags
            .iter()
            .filter(|t| t.name.starts_with("cont_") && t.name.ends_with("_p2c64"))
            .filter_map(|t| super::attr(t.tag, "value"))
            .collect();
        assert!(
            cont_p2c64_digits.contains("150000") && cont_p2c64_digits.contains("200000"),
            "continuation Part IV-B taxpayer amounts must print as whole pesos, got: {cont_p2c64_digits:?}"
        );
        assert!(
            !cont_p2c64_digits.contains('.'),
            "continuation Part IV-B taxpayer whole-peso comb must not emit centavos, got: {cont_p2c64_digits:?}"
        );

        // Unused spouse column (0.00) must stay blank — no stray "0 . 0 0" boxes.
        let cont_p2c65_digits: String = tags
            .iter()
            .filter(|t| t.name.starts_with("cont_") && t.name.ends_with("_p2c65"))
            .filter_map(|t| super::attr(t.tag, "value"))
            .collect();
        assert!(
            cont_p2c65_digits.trim().is_empty(),
            "unused spouse column on 1701A Part IV-B additional sheet must stay blank, got: {cont_p2c65_digits:?}"
        );
    }

    #[test]
    fn form_1702mx_schedules_7_and_8_whole_peso_right_align_and_row7_default() {
        // Part 1j: 1702MX page 4 Schedules 7.1 & 8.1 ("DO NOT enter Centavos").
        // Typed columns A-D carry ".00" in the official XML (once #326 merges),
        // column E carries whole pesos, and untouched row 7 carries the BIR
        // program's official default "0" (runtime-control-inventory-v796.json).
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm1702MX:txtPg3IShed7_4Year".to_string(),
            "2022".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg3IShed7_4A".to_string(),
            "300,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg3IShed7_4B".to_string(),
            "100,000.00".to_string(),
        );
        fields.insert("frm1702MX:txtPg3IShed7_4C".to_string(), "0.00".to_string());
        fields.insert(
            "frm1702MX:txtPg3IShed7_4D".to_string(),
            "50,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg3IShed7_4E".to_string(),
            "150,000".to_string(),
        );
        fields.insert("frm1702MX:txtPg3IShed7_7A".to_string(), "0".to_string());
        fields.insert("frm1702MX:txtPg3IShed7_7E".to_string(), "0".to_string());
        fields.insert("frm1702MX:txtPg3IShed78D".to_string(), "50,000".to_string());
        fields.insert(
            "frm1702MX:txtPg3IShed8_4Year".to_string(),
            "2023".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg4IShed8_4A".to_string(),
            "450,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg4IShed8_4B".to_string(),
            "125,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg4IShed8_4C".to_string(),
            "25,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg4IShed8_4D".to_string(),
            "75,000.00".to_string(),
        );
        fields.insert(
            "frm1702MX:txtPg4IShed8_4E".to_string(),
            "225000".to_string(),
        );
        fields.insert("frm1702MX:txtPg4IShed8_7A".to_string(), "0".to_string());
        fields.insert("frm1702MX:txtPg4IShed8_7E".to_string(), "0".to_string());
        fields.insert("frm1702MX:txtPg3IShed88D".to_string(), "75000".to_string());

        let html = filled_document("1702mx-2018c", &fields).unwrap();
        let tags = super::input_tags(&html);
        let tag_for = |cell_id: &str| {
            tags.iter()
                .find(|t| t.name == cell_id)
                .unwrap_or_else(|| panic!("missing cell {cell_id}"))
                .tag
        };

        // Schedule 7.1 row 4 A-E + row 7 A/E: whole pesos, no ".00", right-aligned.
        for (cell_id, expected) in [
            ("p4c17", "300,000"),
            ("p4c18", "100,000"),
            ("p4c19", "0"),
            ("p4c20", "50,000"),
            ("p4c21", "150,000"),
            ("p4c38", "0"),
            ("p4c42", "0"),
        ] {
            let tag = tag_for(cell_id);
            assert_eq!(super::attr(tag, "value"), Some(expected), "{cell_id} value");
            assert!(
                tag.contains("text-align:right"),
                "{cell_id} must right-align under RULES 4, got: {tag}"
            );
        }

        // Schedule 8.1 row 4 A-E + row 7 A/E: whole pesos, no ".00", right-aligned.
        for (cell_id, expected) in [
            ("p4c64", "450,000"),
            ("p4c65", "125,000"),
            ("p4c66", "25,000"),
            ("p4c67", "75,000"),
            ("p4c68", "225,000"),
            ("p4c85", "0"),
            ("p4c89", "0"),
        ] {
            let tag = tag_for(cell_id);
            assert_eq!(super::attr(tag, "value"), Some(expected), "{cell_id} value");
            assert!(
                tag.contains("text-align:right"),
                "{cell_id} must right-align under RULES 4, got: {tag}"
            );
        }

        // Year columns stay plain 4-digit years (no commas, not right-aligned).
        for (cell_id, expected) in [("p4c16", "2022"), ("p4c63", "2023")] {
            let tag = tag_for(cell_id);
            assert_eq!(super::attr(tag, "value"), Some(expected), "{cell_id} year");
            assert!(
                !tag.contains("text-align:right"),
                "{cell_id} year must not right-align as money, got: {tag}"
            );
        }
    }

    #[test]
    fn form_1600wp_schedule_1_1_totals_print_in_p1c241_and_p1c243() {
        let mut fields = BTreeMap::new();
        fields.insert("derived:sched_seq1".to_string(), "1".to_string());
        fields.insert(
            "frm1600WP:dtSched:txtName1".to_string(),
            "SANTOS, JUANA MARIE".to_string(),
        );
        fields.insert(
            "frm1600WP:dtSched:txtIncome1".to_string(),
            "1,250,000.50".to_string(),
        );
        fields.insert(
            "frm1600WP:dtSched:txtTaxWithheld1".to_string(),
            "125,000.05".to_string(),
        );
        fields.insert(
            "derived:sched_total_amount".to_string(),
            "1,250,000.50".to_string(),
        );
        fields.insert(
            "frm1600WP:dtSched:TotaltaxWithheld".to_string(),
            "125,000.05".to_string(),
        );

        let html = filled_document("1600wp-2010", &fields).unwrap();
        let tags = super::input_tags(&html);
        for (cell_id, expected) in [("p1c241", "1,250,000.50"), ("p1c243", "125,000.05")] {
            let tag = tags
                .iter()
                .find(|t| t.name == cell_id)
                .unwrap_or_else(|| panic!("missing promoted 1600WP total cell {cell_id}"))
                .tag;
            assert_eq!(super::attr(tag, "value"), Some(expected), "{cell_id}");
            assert!(
                tag.contains("text-align:right"),
                "{cell_id} must right-align, got: {tag}"
            );
        }
    }

    #[test]
    fn form_1601c_item20_fits_one_line_with_ellipsis_and_inlines_tinos() {
        let long_specify = "SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS FOR OVERFLOW CHECK";
        let mut fields = BTreeMap::new();
        fields.insert("frm1601c:txt20Other".to_string(), long_specify.to_string());

        let html = filled_document("1601c-2018", &fields).unwrap();
        assert!(
            !html.contains("url(\"../fonts/tinos-latin-400-normal.woff2\")"),
            "1601c-2018 must inline tinos-latin-400-normal.woff2 as a data URI"
        );
        assert!(
            html.contains("data-rule6-band=\"true\" data-rule6-of=\"p1c223\""),
            "1601C item 20 must insert a Rule 6 full-width band"
        );
        let tags = super::input_tags(&html);
        let inline_tag = tags
            .iter()
            .find(|t| t.name == "p1c223" && !t.tag.contains("data-overflow-of=\"p1c223\""))
            .expect("1601c inline p1c223")
            .tag;
        assert_eq!(
            super::attr(inline_tag, "value"),
            Some(""),
            "inline p1c223 must be cleared when Rule 6 band is inserted"
        );
        let band_tag = tags
            .iter()
            .find(|t| t.tag.contains("data-overflow-of=\"p1c223\""))
            .expect("1601c Rule 6 band input")
            .tag;
        assert_eq!(
            super::attr(band_tag, "value"),
            Some(long_specify),
            "1601C item 20 full-width band must hold the full 90-char text"
        );
    }

    #[test]
    fn rule6_named_specify_boxes_insert_full_width_band_and_preserve_truncation() {
        let long_specify = "SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS TO EXERCISE WRAPPING";
        let cases = [
            ("2550q-2024", "frm2550qv2024:addSpecifyNo19", "p1c205"),
            ("2551q-2018", "frm2551Qv2018:txt17Specify", "p1c187"),
            ("1601c-2018", "frm1601c:txt20Other", "p1c223"),
            ("1603q-2018", "frm1603Q:txt16Other", "p1c181"),
            ("1700-2018", "frm1700:txtPg2I57Desc", "p2c157"),
            ("1701a-2018", "frm1701A:txt63Desc", "p2c176"),
            ("1701q-2018", "frm1701q:txt61Desc", "p2c152"),
            (
                "1701ms-2024",
                "derived:foreign_tax_credits_description",
                "p2c177",
            ),
            ("1701ms-2024", "derived:other_credits_description", "p2c179"),
        ];
        for (slug, key, cell_id) in cases {
            let mut short_fields = BTreeMap::new();
            short_fields.insert(key.to_string(), "SHORT".to_string());
            let short_html = filled_document(slug, &short_fields).unwrap();
            assert!(
                !short_html.contains("data-rule6-band=\"true\""),
                "{slug} {cell_id}: short value must not insert a Rule 6 band"
            );

            let mut long_fields = BTreeMap::new();
            long_fields.insert(key.to_string(), long_specify.to_string());
            let long_html = filled_document(slug, &long_fields).unwrap();
            let marker = format!("data-rule6-band=\"true\" data-rule6-of=\"{cell_id}\"");
            assert!(
                long_html.contains(&marker),
                "{slug} {cell_id}: overflowing specify must insert Rule 6 full-width band"
            );
            // A band narrowed clear of the row's amount cells wraps the value
            // to a second line instead of holding it on one; the line break
            // takes the space it wraps at, so normalizing it back must show
            // the value whole.
            assert!(
                long_html.replace("&#10;", " ").contains(long_specify),
                "{slug} {cell_id}: full 90-char specify text must appear whole in the band"
            );

            let extreme =
                format!("{long_specify} {long_specify} {long_specify} {long_specify} EXTRA TAIL");
            let mut ext_fields = BTreeMap::new();
            ext_fields.insert(key.to_string(), extreme.clone());
            let ext_html = filled_document(slug, &ext_fields).unwrap();
            assert!(
                ext_html.contains(&marker),
                "{slug} {cell_id}: extreme specify must still use Rule 6 full-width band"
            );
            assert!(
                ext_html.contains("Entries continued from the form"),
                "{slug} {cell_id}: extreme truncated Rule 6 specify must emit Additional Sheet"
            );
            assert!(
                ext_html.contains(&extreme),
                "{slug} {cell_id}: Additional Sheet must hold full extreme text"
            );
        }
    }

    #[test]
    fn form_0605_long_word_name_fits_two_lines_within_inner_height_and_never_three() {
        let checker_name = "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA EXTRAORDINARILY LONG FAMILY NAME WITH MIDDLE NAMES AND SUFFIXES FOR PRINT OVERFLOW TESTING";
        let loaded = super::bundle("0605-1999").unwrap();
        let sheet = super::SheetType::of(loaded.css);
        let (cell_w, cell_h) = super::plain_cell_dimensions(loaded.html, "p1c35").unwrap();

        let mut fields = BTreeMap::new();
        fields.insert(
            "frm0605:txtTaxPayerName".to_string(),
            checker_name.to_string(),
        );
        let html = filled_document("0605-1999", &fields).unwrap();
        assert!(
            !html.contains("url(\"../fonts/tinos-latin-400-normal.woff2\")"),
            "0605-1999 must inline tinos-latin-400-normal.woff2 as a data URI"
        );
        let tags = super::input_tags(&html);
        let tag = tags
            .iter()
            .find(|t| t.name == "p1c35")
            .expect("0605 p1c35")
            .tag;
        let inner_w = super::input_room(cell_w, tag);
        let inner_h = super::input_height(cell_h, tag);
        let val = super::attr(tag, "value").unwrap().replace("&#10;", "\n");
        let lines: Vec<&str> = val.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "0605 137-char long-word name must wrap to exactly 2 lines, got: {val:?}"
        );
        let size: f64 = tag
            .split("font-size:")
            .nth(1)
            .unwrap()
            .split("pt")
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(size >= 7.5, "font size below 7.5pt floor: {size}");
        assert!(
            2.0 * size * 1.04 <= inner_h + 1e-6,
            "two lines at {size}pt (height {:.2}pt) exceed inner box height {inner_h:.2}pt",
            2.0 * size * 1.04
        );
        let letter = sheet.letter_for(super::attr(tag, "class").unwrap());
        for (idx, line) in lines.iter().enumerate() {
            let w = sheet.width(letter, line, size);
            assert!(
                w <= inner_w,
                "line {} width {w:.2}pt exceeds inner width {inner_w:.2}pt: {line:?}",
                idx + 1
            );
        }

        // Unbreakably long words past 2 lines at 7.5pt: stay on 2 lines with "..." + additional sheet.
        let extreme_name = "SUPEREXTRAORDINARILYLONGHYPHENATEDFAMILYNAME-VILLANUEVACRUZDELASANTOS EXTRAORDINARILYLONGSECONDWORDWITHOUTBREAKSFORPRINTINGOVERFLOWTESTINGANDADDITIONALTEXT BEYOND TWO LINES";
        let mut fields2 = BTreeMap::new();
        fields2.insert(
            "frm0605:txtTaxPayerName".to_string(),
            extreme_name.to_string(),
        );
        let html2 = filled_document("0605-1999", &fields2).unwrap();
        let tags2 = super::input_tags(&html2);
        let tag2 = tags2
            .iter()
            .find(|t| t.name == "p1c35")
            .expect("0605 p1c35")
            .tag;
        let val2 = super::attr(tag2, "value").unwrap().replace("&#10;", "\n");
        let lines2: Vec<&str> = val2.lines().collect();
        assert_eq!(
            lines2.len(),
            2,
            "extreme long-word name must stay on 2 lines, got: {val2:?}"
        );
        assert!(
            lines2[1].ends_with("..."),
            "line 2 must ellipsize: {:?}",
            lines2[1]
        );
        assert!(tag2.contains("font-size:8.55pt"), "got: {tag2}");
        assert_eq!(
            super::attr(tag2, "data-overflow-warning"),
            Some("text-truncated")
        );
        assert_eq!(super::attr(tag2, "data-writer-value"), Some(extreme_name));
    }

    #[test]
    fn form_2000ot_continuation_name_fits_two_lines_within_inner_box() {
        let checker_name = "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA EXTRAORDINARILY LONG FAMILY NAME WITH MIDDLE NAMES AND SUFFIXES FOR PRINT OVERFLOW TESTING";
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm2000OT:txtPg2TaxpayerName".to_string(),
            checker_name.to_string(),
        );
        let html = filled_document("2000-ot-2018", &fields).unwrap();
        let cells = super::printed_cells(&html);
        let (val, sz) = cells.get("p2c6").expect("2000OT p2c6");
        let lines: Vec<&str> = val.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "2000OT p2c6 must wrap to 2 lines, got: {val:?}"
        );
        let size = sz.expect("p2c6 font size");
        assert!(size >= 7.5, "font size below floor: {size}");
        let loaded = super::bundle("2000-ot-2018").unwrap();
        let sheet = super::SheetType::of(loaded.css);
        let (_, _, w, h) = super::cell_box_pt(loaded.html, "p2c6").unwrap();
        assert!(
            2.0 * size * 1.04 <= (h - 1.5) + 1e-6,
            "2000OT p2c6 two lines exceed inner height {:.2}pt",
            h - 1.5
        );
        let letter = sheet.letter_for("fi");
        for line in &lines {
            assert!(
                sheet.width(letter, line, size) <= w - 3.0,
                "2000OT p2c6 line exceeds inner width {:.2}pt: {line:?}",
                w - 3.0
            );
        }
    }

    #[test]
    fn form_1701_item_6_marks_compensation_earner_only_when_mixed_income_flag_is_set() {
        use bir_core::forms::form_1701::{Form1701Atc, Form1701Draft, Form1701TaxpayerType};

        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null,
                "full_name": "SANTOS, JUANA",
                "tin": bir_core::Tin::parse_digits("00012345300000").unwrap(),
                "rdo_code": "039",
                "line_of_business": "CONSULTING SERVICES",
                "registered_address": "123 SAMPLE STREET, MAKATI",
                "zip_code": "1229",
                "phone": "09171234567",
                "email": "juana.santos@example.com",
                "default_form_type": "1701"
            }))
            .unwrap();

        let mut single_only = Form1701Draft::new_from_profile(&profile, 2025, 12);
        single_only.taxpayer_type = Some(Form1701TaxpayerType::SingleProprietor);
        single_only.taxpayer_also_compensation_earner = false;
        single_only.atc = Some(Form1701Atc::Ii013);
        let html_single = filled_document("1701-2018", &single_only.to_print_field_map()).unwrap();
        let tags_single = super::input_tags(&html_single);
        fn val_of<'a>(tags: &[super::InputTag<'a>], name: &str) -> Option<&'a str> {
            tags.iter()
                .find(|t| t.name == name)
                .and_then(|t| super::attr(t.tag, "value"))
        }
        assert_eq!(val_of(&tags_single, "p1c23"), Some("X"));
        assert_eq!(val_of(&tags_single, "p1c27"), None);

        let mut mixed = single_only.clone();
        mixed.taxpayer_also_compensation_earner = true;
        let html_mixed = filled_document("1701-2018", &mixed.to_print_field_map()).unwrap();
        let tags_mixed = super::input_tags(&html_mixed);
        assert_eq!(val_of(&tags_mixed, "p1c23"), Some("X"));
        assert_eq!(val_of(&tags_mixed, "p1c27"), Some("X"));
    }

    #[test]
    fn form_1702q_schedule_4_item_6a_fits_two_lines_without_truncation() {
        let specify = "SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS TO EXERCISE WRAPPING";
        let mut fields = BTreeMap::new();
        fields.insert(
            "frm1702q:Sched4:txtOthrTxCrdts0".to_string(),
            specify.to_string(),
        );
        let html = filled_document("1702q-2018", &fields).unwrap();
        let cells = super::printed_cells(&html);
        let (val, sz) = cells.get("p2c110").expect("1702Q p2c110");
        let lines: Vec<&str> = val.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "1702Q Sched 4 item 6a must wrap to 2 lines, got: {val:?}"
        );
        assert!(
            !val.contains("..."),
            "1702Q Sched 4 item 6a must not ellipsize when 2 lines fit, got: {val:?}"
        );
        assert!(
            sz.expect("p2c110 font size") >= super::PLAIN_TEXT_MIN_PT,
            "1702Q Sched 4 item 6a font size below floor: {sz:?}"
        );
        assert!(
            !html.contains("data-overflow-warning=\"text-truncated\""),
            "1702Q Sched 4 item 6a must not be marked text-truncated"
        );
    }

    #[test]
    fn form_0605_a4_scaled_font_size_never_below_7_5pt_at_100_137_200() {
        // 0605-1999 is a 612pt x 936pt folio sheet printed onto A4 (595.28pt x 841.89pt)
        // with 10pt safety margin -> fit_scale = 821.89 / 936.0 ~= 0.8781.
        // Effective printed font size (sheet_pt * fit_scale) must never drop below 7.5pt
        // on Page 1 (p1c33 line of business, p1c35 taxpayer name, p1c38 address) or on
        // the Additional Sheet table.
        let fit_scale = ((595.28_f64 - 20.0) / 612.0)
            .min((841.89_f64 - 20.0) / 936.0)
            .min(1.0);
        let alpha = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let make_val = |prefix: &str, len: usize| -> String {
            let mut s = format!("{prefix}{len}-");
            while s.len() < len {
                s.push_str(alpha);
            }
            s.truncate(len);
            s
        };

        for len in [60usize, 100, 137, 200] {
            let name = make_val("NAME", len);
            let addr = make_val("ADDRESS", len);
            let lob = make_val("BUSINESS", len);
            let mut fields = BTreeMap::new();
            fields.insert("frm0605:txtTaxPayerName".to_string(), name.clone());
            fields.insert("frm0605:txtAddress".to_string(), addr.clone());
            fields.insert("frm0605:txtLineBus".to_string(), lob.clone());
            let html = filled_document("0605-1999", &fields).unwrap();
            let tags = super::input_tags(&html);
            for (cid, full) in [("p1c33", &lob), ("p1c35", &name), ("p1c38", &addr)] {
                let tag = tags
                    .iter()
                    .find(|t| t.name == cid)
                    .unwrap_or_else(|| panic!("0605 {cid} at n={len}"))
                    .tag;
                let val = super::attr(tag, "value")
                    .unwrap_or("")
                    .replace("&#10;", "\n");
                let sheet_pt: f64 = tag
                    .split("font-size:")
                    .nth(1)
                    .and_then(|s| s.split("pt").next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10.0);
                let printed_pt = sheet_pt * fit_scale;
                assert!(
                    printed_pt >= super::PLAIN_TEXT_MIN_PT - 0.01,
                    "0605 n={len} {cid}: sheet {sheet_pt:.2}pt -> A4 {printed_pt:.2}pt < 7.5pt (val={val:?})"
                );
                assert!(
                    val.lines().count() <= 2,
                    "0605 n={len} {cid}: must never exceed 2 lines, got {val:?}"
                );
                if val.contains("...") {
                    assert!(
                        html.contains(full.as_str()),
                        "0605 n={len} {cid}: truncated field must carry full value to Additional Sheet"
                    );
                }
            }
            if len >= 100 {
                // Additional Sheet table itself must also print >= 7.5pt after A4 fit_scale
                assert!(
                    !html.contains("table-layout:fixed;font-size:7.5pt"),
                    "0605 n={len}: Additional Sheet table font-size:7.5pt scales to 6.59pt on A4"
                );
            }
        }
    }

    #[test]
    fn no_printed_document_keeps_relative_fonts_reference() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
        let mut slugs: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry
                    .path()
                    .join("index.html")
                    .exists()
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        slugs.sort();
        assert_eq!(slugs.len(), 43, "expected all 43 frozen sheets");
        let empty = BTreeMap::new();
        let mut offenders = Vec::new();
        for slug in &slugs {
            let html = filled_document(slug, &empty).unwrap();
            if html.contains("../fonts/") {
                offenders.push(slug.as_str());
            }
        }
        assert!(
            offenders.is_empty(),
            "printed documents must inline all fonts and keep no ../fonts/ reference; found in: {:?}",
            offenders
        );
    }
}
