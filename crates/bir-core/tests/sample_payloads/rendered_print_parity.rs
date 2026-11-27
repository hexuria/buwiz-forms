//! Rendered print parity: every official sample, printed on its frozen
//! sheet the way the app prints it (`QueueableForm::print_field_map` into
//! `bir_print::frozen_html::filled_document_with_report`, official pages,
//! cloned continuation pages and the plain additional sheet), must show
//! what its official upload (`<stem>.official.xml`) holds.
//!
//! What is read is the rendered page: the characters in a comb's boxes (a
//! preprinted decimal point between the peso boxes and the last two), the
//! value of a plain box, the split peso and centavo parts of a box with a
//! preprinted point, the plain rectangle laid over a comb an amount does not
//! fit, and, for a text shortened with "...", the full text the additional
//! sheet prints.
//!
//! - An amount compares as the signed number it is, centavos included:
//!   `(200,000)` equals `-200000`, `1,000.51` does not equal `1001`. An
//!   untouched amount box stays blank (form-review/RULES.md section 7), so an
//!   official zero may print blank. An amount is never cut with "...".
//! - A TIN compares by its digits, and where the print adds dashes they sit
//!   where the official TIN has them (`123-456-788-000`): `AAA-BBB-CCC-rest`,
//!   or the upload's own dash positions when the upload is dashed.
//! - A rate printed with its percent sign (`5 %`) compares as the number.
//! - Any other text compares exactly, except for case (the print writes BIR
//!   capital letters, as the official `capital()` does, and keeps an e-mail
//!   as typed) and line breaks (a long text takes two lines).
//!
//! Covered: every `joins`, `money_joins`, `peso_joins`, `date_joins` and
//! `xbox_joins` row of the sheet's writer-cells.json (with the print's own
//! SHEET_JOINS merged in), every stamped `name=` input, the 1800/1801 sheet
//! joins of `frozen_html.rs` (DONOR_ESTATE_JOINS), the derived combs (each
//! printed `derived:` value against the print map, and the derived amounts of
//! 1700, 1701A, 1701-MS, 1702-EX and 1702Q against the upload control they
//! show), and the rows no official cell holds (the pop-ups and rows past the
//! sheet's last line): each such upload value must print on a continuation
//! page or the additional sheet, the cloned TOTAL rows must print the
//! upload's total, and the additional sheet's subtotals must equal the sum
//! of the rows they total. The only keys not compared are the
//! [`ALLOWED`] exceptions, each with its reason.

use crate::{
    frozen_slug_for_sample, is_currency_text, norm_numeric, official_amount_keys, official_samples,
    parse_official_xml_fields, samples_dir,
};
use bir_print::frozen_html::{
    filled_document_with_report, merged_writer_cells_json, stacked_lines,
};
use std::collections::{BTreeMap, BTreeSet};

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(&tag[start..start + end])
}

fn unescape(s: &str) -> String {
    s.replace("&#10;", "\n")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// One printed input: its comb slot (if any) and the text it shows.
#[derive(Clone, Debug)]
struct Input {
    slot: Option<usize>,
    value: String,
}

/// What a set of pages prints, cell by cell.
#[derive(Default)]
struct Printed {
    /// Inputs by `name` (and by `#id` where the id is not the name).
    inputs: BTreeMap<String, Vec<Input>>,
    /// A box with a preprinted point: its peso and centavo parts.
    decimal: BTreeMap<String, (String, String)>,
    /// The plain rectangle laid over a comb (`data-overflow-of`).
    overflow: BTreeMap<String, String>,
    /// Cells shortened with "...": their full value.
    truncated: BTreeMap<String, String>,
    /// Every input carrying a `data-writer-value`: (cell, writer value).
    writers: Vec<(String, String)>,
}

impl Printed {
    fn of(pages: &str) -> Self {
        let mut out = Printed::default();
        let mut from = 0;
        while let Some(rel) = pages[from..].find("<input") {
            let start = from + rel;
            let Some(end) = pages[start..].find('>') else {
                break;
            };
            let tag = &pages[start..=start + end];
            from = start + end + 1;
            let value = if attr(tag, "type") == Some("checkbox") {
                if tag.contains(" checked") { "X" } else { "" }.to_string()
            } else {
                unescape(attr(tag, "value").unwrap_or(""))
            };
            let writer = attr(tag, "data-writer-value").map(unescape);
            if attr(tag, "data-overflow-warning") == Some("text-truncated") {
                let cell = attr(tag, "data-overflow-of")
                    .or_else(|| attr(tag, "name"))
                    .unwrap_or("");
                if let Some(full) = &writer {
                    out.truncated.insert(cell.to_string(), full.clone());
                }
            }
            if let Some(of) = attr(tag, "data-decimal-of") {
                let entry = out.decimal.entry(of.to_string()).or_default();
                match attr(tag, "data-decimal-part") {
                    Some("peso") => entry.0 = value,
                    _ => entry.1 = value,
                }
                continue;
            }
            if let Some(of) = attr(tag, "data-overflow-of") {
                out.overflow.insert(of.to_string(), value);
                continue;
            }
            let slot = attr(tag, "data-slot-index").and_then(|s| s.parse().ok());
            let name = attr(tag, "name");
            if let (Some(name), Some(writer)) = (name, &writer) {
                out.writers.push((name.to_string(), writer.clone()));
            }
            let input = Input { slot, value };
            if let Some(name) = name {
                out.inputs
                    .entry(name.to_string())
                    .or_default()
                    .push(input.clone());
            }
            if let Some(id) = attr(tag, "id").filter(|id| name != Some(*id)) {
                out.inputs.entry(format!("#{id}")).or_default().push(input);
            }
        }
        out
    }

    fn slots(&self, cell: &str) -> Vec<Input> {
        let mut slots = self.inputs.get(cell).cloned().unwrap_or_default();
        if slots.is_empty() {
            slots = self
                .inputs
                .get(&format!("#{cell}"))
                .cloned()
                .unwrap_or_default();
        }
        slots.sort_by_key(|input| input.slot.unwrap_or(usize::MAX));
        slots
    }

    /// What prints over cell (or stamped name) `cell`: its boxes in slot
    /// order (a point where the comb leaves a point column before its last
    /// two boxes), else the rectangle laid over it.
    fn cell(&self, cell: &str) -> String {
        if let Some((peso, cents)) = self.decimal.get(cell) {
            return format!("{peso}.{cents}");
        }
        let slots = self.slots(cell);
        let indices: Option<Vec<usize>> = slots.iter().map(|input| input.slot).collect();
        let gaps: Vec<usize> = indices
            .as_deref()
            .map(|indices| {
                (1..indices.len())
                    .filter(|&k| indices[k] != indices[k - 1] + 1)
                    .collect()
            })
            .unwrap_or_default();
        let mut boxes = String::new();
        let point_at = match gaps.as_slice() {
            [gap] if slots.len() - gap == 2 => Some(*gap),
            _ => None,
        };
        for (offset, input) in slots.iter().enumerate() {
            if Some(offset) == point_at && !boxes.trim().is_empty() {
                boxes.push('.');
            }
            boxes.push_str(&input.value);
        }
        if !boxes.trim().is_empty() {
            return boxes;
        }
        self.overflow.get(cell).cloned().unwrap_or(boxes)
    }

    /// An amount comb whose last two boxes take the centavos (a money join's
    /// `decimal_html_id`).
    fn decimal_comb(&self, cell: &str) -> String {
        let slots = self.slots(cell);
        let boxes: String = slots.iter().map(|input| input.value.as_str()).collect();
        if boxes.trim().is_empty() {
            return self.overflow.get(cell).cloned().unwrap_or_default();
        }
        let split = slots.len().saturating_sub(2);
        let peso: String = slots[..split].iter().map(|i| i.value.as_str()).collect();
        let cents: String = slots[split..].iter().map(|i| i.value.as_str()).collect();
        format!("{}.{}", peso.trim(), cents.trim())
    }
}

/// The official pages, the cloned continuation pages and the plain
/// additional sheet of a rendered document.
fn split_pages(document: &str) -> (String, String, String) {
    let marker = "<div class=\"page ";
    let mut starts = Vec::new();
    let mut pos = 0;
    while let Some(rel) = document[pos..].find(marker) {
        starts.push(pos + rel);
        pos += rel + marker.len();
    }
    let (mut official, mut band, mut plain) = (String::new(), String::new(), String::new());
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(document.len());
        let page = &document[start..end];
        let open = &page[..page.find('>').unwrap_or(page.len())];
        if open.contains("page-continuation-band") {
            band.push_str(page);
        } else if open.contains("page-continuation") {
            plain.push_str(page);
        } else if !open.contains("page-receipt") {
            official.push_str(page);
        }
    }
    (official, band, plain)
}

/// The text of every table cell on the plain additional sheet.
fn plain_cells(plain: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = plain[from..].find("<td") {
        let start = from + rel;
        let Some(open_end) = plain[start..].find('>') else {
            break;
        };
        let body_start = start + open_end + 1;
        let Some(close) = plain[body_start..].find("</td>") else {
            break;
        };
        let body = &plain[body_start..body_start + close];
        out.push(unescape(body).trim().to_string());
        from = body_start + close;
    }
    out
}

/// How a value is compared.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Amount,
    Tin,
    Text,
}

fn kind_of(key: &str, official: &str, amounts: &BTreeSet<String>) -> Kind {
    if amounts.contains(key) || is_currency_text(official) {
        Kind::Amount
    } else if key.to_ascii_uppercase().contains("TIN") {
        Kind::Tin
    } else {
        Kind::Text
    }
}

/// Every amount `pages` print in a type the print sized (an inline
/// `font-size`): right-aligned figures and two-decimal amounts, with
/// whether it is marked `amount-small` (carried to the additional sheet).
fn amount_sizes(pages: &str) -> Vec<(f64, String, bool)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = pages[from..].find("<input") {
        let start = from + rel;
        let Some(end) = pages[start..].find('>') else {
            break;
        };
        let tag = &pages[start..=start + end];
        from = start + end + 1;
        let value = unescape(attr(tag, "value").unwrap_or(""))
            .trim()
            .to_string();
        let style = attr(tag, "style").unwrap_or("");
        let Some(size) = style
            .rfind("font-size:")
            .and_then(|at| style[at + "font-size:".len()..].split("pt").next())
            .and_then(|v| v.parse::<f64>().ok())
        else {
            continue;
        };
        let amount = norm_numeric(&value).is_some()
            && (style.contains("text-align:right") || is_currency_text(&value));
        if amount && !value.is_empty() {
            let small = attr(tag, "data-overflow-warning") == Some("amount-small");
            out.push((size, value, small));
        }
    }
    out
}

/// The smallest type (pt) an amount prints in on each sheet over the 79
/// official samples (`RENDERED_PARITY_DUMP=1` prints them): the sheets
/// where some sample amount is sized (a rectangle over a comb, or a box
/// it does not fit at its class size); on every other sheet each sample
/// amount prints at its box's own size. A sheet missing here, or an
/// amount smaller than its entry, fails the parity test.
const SMALLEST_AMOUNT_PT: &[(&str, f64)] = &[
    ("1700-2018", 9.0),
    ("1701-2018", 9.0),
    ("1701a-2018", 9.0),
    ("1701ms-2024", 9.0),
    ("1701q-2018", 9.0),
    ("1702ex-2018", 9.0),
    ("1702q-2018", 9.0),
    ("2200p-2020", 7.76),
];

/// The TIN and ATC joins (sheet, writer key), pinned in
/// `fixtures/identifier-joins.tsv`: read whole from the page, never from the
/// additional sheet, and never shortened. bir-print's
/// `identifier_classing_matches_the_pinned_list` checks the print classes
/// exactly these, so a change to the classing is a diff of the list.
fn pinned_identifiers() -> BTreeSet<(String, String)> {
    include_str!("fixtures/identifier-joins.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let (sheet, key) = line.split_once('\t').expect("sheet<TAB>key");
            (sheet.to_string(), key.to_string())
        })
        .collect()
}

/// Cloned continuation cell id -> the upload key it prints
/// (`data-upload-key` on the cell's `<div>`).
fn upload_keys(pages: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut from = 0;
    while let Some(rel) = pages[from..].find("<div id=\"") {
        let start = from + rel;
        let end = pages[start..].find('>').map_or(pages.len(), |e| start + e);
        let tag = &pages[start..end];
        from = end;
        if let (Some(id), Some(key)) = (attr(tag, "id"), attr(tag, "data-upload-key")) {
            out.insert(id.to_string(), unescape(key));
        }
    }
    out
}

/// The original cell a continuation clone copies: `cont_<schedule>_<page>_
/// <band>_<row>_<cell>` (a cloned row) or `cont_tot_<schedule>_<cell>` (the
/// cloned TOTAL row) -> `(clone prefix, original cell id)`.
fn clone_origin(cell: &str) -> (&str, &str) {
    match cell.rfind("_p") {
        Some(at) => (&cell[..=at], &cell[at + 1..]),
        None => ("", cell),
    }
}

/// The continuation clones among `cells` that carry no `data-upload-key`:
/// every cloned cell except a centavo half (`cents`, the cent cells of the
/// sheet's money joins, read with their peso cell) must name the upload key
/// it prints, so it is compared with the upload, never with itself.
fn clones_without_upload_key<'a>(
    cells: impl IntoIterator<Item = &'a String>,
    upload_keys: &BTreeMap<String, String>,
    cents: &BTreeSet<String>,
) -> Vec<String> {
    cells
        .into_iter()
        .filter(|cell| cell.starts_with("cont_"))
        .filter(|cell| !cents.contains(clone_origin(cell).1))
        .filter(|cell| !upload_keys.contains_key(cell.as_str()))
        .cloned()
        .collect()
}

/// The additional sheet's schedule tables: (schedule heading, row number)
/// -> the row's cells in column order.
fn plain_table_rows(plain: &str) -> BTreeMap<(String, usize), Vec<String>> {
    let mut out = BTreeMap::new();
    let marker = " (continued)</div>";
    let mut from = 0;
    while let Some(rel) = plain[from..].find(marker) {
        let at = from + rel;
        let heading = unescape(&plain[plain[..at].rfind('>').map_or(0, |g| g + 1)..at]);
        from = at + marker.len();
        let Some(body_rel) = plain[from..].find("<tbody>") else {
            break;
        };
        let body_start = from + body_rel;
        let body_end = plain[body_start..]
            .find("</tbody>")
            .map_or(plain.len(), |e| body_start + e);
        for row in plain[body_start..body_end].split("<tr").skip(1) {
            let cells = plain_cells(row);
            let Some(number) = cells.first().and_then(|n| n.parse::<usize>().ok()) else {
                continue;
            };
            out.insert((heading.clone(), number), cells[1..].to_vec());
        }
        from = body_end;
    }
    out
}

/// frozen_html.rs `is_attachment_count_key`: a count printed in digit boxes.
fn is_attachment_count(key: &str) -> bool {
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
    )
}

/// `digits` dashed the way a TIN is: `AAA-BBB-CCC-rest`.
fn dashed_tin(digits: &str) -> String {
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && i % 3 == 0 && i <= 9 {
            out.push('-');
        }
        out.push(ch);
    }
    out
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rate_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let body = s.strip_suffix('%')?.trim();
    norm_numeric(body)
}

/// Whether `printed` shows `official` (see the module docs).
fn shows(official: &str, printed: &str, kind: Kind) -> bool {
    let off = official.trim();
    let prt = printed.trim();
    if off == prt {
        return true;
    }
    if (off == "true" && prt == "X") || (off == "false" && prt.is_empty()) {
        return true;
    }
    if let (Some(a), Some(b)) = (norm_numeric(off), rate_number(prt)) {
        return (a - b).abs() < 1e-9;
    }
    match kind {
        Kind::Amount => {
            if prt.contains("...") || prt.contains('\u{2026}') {
                return false;
            }
            match (norm_numeric(off), norm_numeric(prt)) {
                (Some(a), _) if a == 0.0 && prt.is_empty() => true,
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                _ => false,
            }
        }
        Kind::Tin => {
            let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
            if digits(off) != digits(prt)
                || !prt
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
            {
                return false;
            }
            if !prt.contains(['-', ' ']) {
                return true;
            }
            let expected = if off.contains('-') {
                off.to_string()
            } else {
                dashed_tin(&digits(off))
            };
            prt.replace(' ', "") == expected
        }
        Kind::Text => {
            squash(off).eq_ignore_ascii_case(&squash(prt))
                || off
                    .replace(char::is_whitespace, "")
                    .eq_ignore_ascii_case(&prt.replace(char::is_whitespace, ""))
        }
    }
}

/// Reviewed exceptions: `(sheet, key, reason)`. No other key is skipped.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "1604c-2018",
        "frm1604c:txtYear",
        "the sheet preprints \"20\" before the year's two boxes (\"For the Year (20YY)\")",
    ),
    (
        "1604e-2018",
        "frm1604e:txtYear",
        "the sheet preprints \"20\" before the year's two boxes (\"For the Year (20YY)\")",
    ),
    (
        "1800-2018",
        "frm1800v2018:txtYear",
        "item 1 \"Date of Donation (MM/DD/YY)\": the year's box is two digits (YY)",
    ),
    (
        "1702rt-2018c",
        "frm1702RT:drpPg1I5AtcOther",
        "the upload serializes the disabled \"Other ATC\" select's default IC010 when no other \
         ATC is elected; the box stays blank",
    ),
    (
        "1701-2018",
        "frm1701:txtPg2I2SpouseRDOCode",
        "with the spouse column closed the upload writes 000; the box stays blank",
    ),
];

fn allowed(slug: &str, key: &str, official: &str, printed: &str) -> bool {
    ALLOWED.iter().any(|(sheet, k, _)| {
        *sheet == slug
            && *k == key
            && match key {
                "frm1702RT:drpPg1I5AtcOther" => official == "IC010" && printed.is_empty(),
                "frm1701:txtPg2I2SpouseRDOCode" => official == "000" && printed.is_empty(),
                _ => official.len() == 4 && printed.trim() == &official[2..],
            }
    })
}

/// The upload control a derived amount comb shows, where the print map
/// derives it from one.
fn derived_amount_source(slug: &str, key: &str) -> Option<String> {
    let name = key.strip_prefix("derived:")?;
    match slug {
        "1700-2018" => {
            if name == "amount_36" {
                return Some("frm1700:txtPg1I36".into());
            }
            if let Some(rest) = name.strip_prefix("amount_sched1_") {
                let (row, column) = rest.split_at(rest.len() - 1);
                let control = match column {
                    "c" => "c_{}REG",
                    "d" => "d_{}CIFR",
                    "e" => "e_{}TW",
                    _ => return None,
                };
                return Some(format!(
                    "frm1700:txtPg2ISched1{}",
                    control.replace("{}", row)
                ));
            }
            let rest = name.strip_prefix("amount_")?;
            let (item, column) = rest.split_at(rest.len() - 1);
            let item: u32 = item.parse().ok()?;
            // The sheet prints item 31 Surcharge and item 32 Interest; the
            // page's controls are the other way round (form_1700.rs).
            let item = match item {
                31 => 32,
                32 => 31,
                n => n,
            };
            let page = if item <= 36 { 1 } else { 2 };
            Some(format!("frm1700:txtPg{page}I{item}{column}"))
        }
        "1701a-2018" => {
            let rest = name.strip_prefix("amount_")?;
            Some(format!("frm1701A:txt{rest}"))
        }
        "1701ms-2024" => {
            let control = match name {
                "aggregate" => Some("txtAggregateAmountPayable30a"),
                "refund_amount" => Some("txtToBeRefunded1"),
                "tcc_amount" => Some("txtToBeIssued1"),
                "carry_over_amount" => Some("txtToBeCarried1"),
                _ => None,
            };
            if let Some(control) = control {
                return Some(format!("frm1701MS:{control}"));
            }
            let (side, rest) = name.split_once('_')?;
            let suffix = match side {
                "tp" => 0,
                "sp" => 1,
                _ => return None,
            };
            FORM_1701MS_AMOUNTS
                .iter()
                .find(|(print, _)| *print == rest)
                .map(|(_, controls)| format!("frm1701MS:{}", controls[suffix]))
        }
        "1702ex-2018" => name
            .strip_prefix("amount:")
            .map(|control| format!("frm1702EX:{control}")),
        "1702q-2018" => {
            if name.starts_with("Sched") || name.starts_with("txtTax") {
                Some(format!("frm1702q:{name}"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// form_1701ms.rs `print_amounts` keys with the `page_amounts` controls they
/// print (taxpayer, spouse), in the same order.
const FORM_1701MS_AMOUNTS: &[(&str, [&str; 2])] = &[
    ("p2_19", ["txtIncomeTaxDueNo19a", "txtIncomeTaxDueNo19b"]),
    ("p2_20", ["txtShareOfOtherNo20a", "txtShareOfOtherNo20b"]),
    (
        "p2_21",
        ["txtNetSpecialRateNo21a", "txtNetSpecialRateNo21b"],
    ),
    (
        "p2_22",
        ["txtIncomeTaxRegularNo22a", "txtIncomeTaxRegularNo22b"],
    ),
    (
        "p2_23",
        ["txtTotalIncomeTaxDueNo23a", "txtTotalIncomeTaxDueNo23b"],
    ),
    (
        "p2_24",
        [
            "txtTotalTaxCreditsPartIINo24a",
            "txtTotalTaxCreditsPartIINo24b",
        ],
    ),
    ("p2_25", ["txtTaxPayableNo25a", "txtTaxPayableNo25b"]),
    ("p2_26", ["txtTaxDueAllowedNo26a", "txtTaxDueAllowedNo26b"]),
    ("p2_27", ["txtAmountPayableNo27a", "txtAmountPayableNo27b"]),
    ("p2_28a", ["txtSurchangeNo28a", "txtSurchangeNo28b"]),
    ("p2_28b", ["txtInterestNo28a", "txtInterestNo28b"]),
    ("p2_28c", ["txtCompromise28a", "txtCompromise28b"]),
    (
        "p2_28d",
        ["txtTotalPenaltiesNo28a", "txtTotalPenaltiesNo28b"],
    ),
    (
        "p2_29",
        ["txtTotalAmountPayableNo29a", "txtTotalAmountPayableNo29b"],
    ),
    ("p4_1", ["txtGrossa", "txtGrossb"]),
    ("p4_2", ["txtLessNonTaxablea", "txtLessNonTaxableb"]),
    (
        "p4_3",
        [
            "txtTaxableCompensationIncomea",
            "txtTaxableCompensationIncomeb",
        ],
    ),
    ("p4_4", ["txtTaxpayerNo4a", "txtTaxpayerNo4b"]),
    ("p4_5", ["txtSalesRevenueNo5a", "txtSalesRevenueNo5b"]),
    ("p4_6", ["txtSalesReturnsNo6a", "txtSalesReturnsNo6b"]),
    (
        "p4_7",
        [
            "txtTaxableCompensationIncomeNo7a",
            "txtTaxableCompensationIncomeNo7b",
        ],
    ),
    ("p4_8", ["txtCostofSalesNo8a", "txtCostofSalesNo8b"]),
    ("p4_9", ["txtGrossIncomeNo9a", "txtGrossIncomeNo9b"]),
    (
        "p4_10a",
        [
            "txtAllowableItemizedDeductionsNo10a",
            "txtAllowableItemizedDeductionsNo10aSpouse",
        ],
    ),
    (
        "p4_10b",
        ["txtSpecialAllowableNo10a", "txtSpecialAllowableNo10b"],
    ),
    (
        "p4_10c",
        ["txtNetOperatingLossNo10a", "txtNetOperatingLossNo10b"],
    ),
    (
        "p4_10d",
        [
            "txtTotalAllowableItemixedDeductionsNo10a",
            "txtTotalAllowableItemixedDeductionsNo10b",
        ],
    ),
    (
        "p4_11",
        ["txtOptionalDeductionNo11a", "txtOptionalDeductionNo11b"],
    ),
    ("p4_12", ["txtNetIncomeLossNo12a", "txtNetIncomeLossNo12b"]),
    (
        "p4_13",
        ["txtNonOperatingIncomeNo13a", "txtNonOperatingIncomeNo13b"],
    ),
    (
        "p4_14",
        [
            "txtTaxableIncomeBusinesNo14a",
            "txtTaxableIncomeBusinesNo14b",
        ],
    ),
    (
        "p4_15",
        [
            "txtCompensationBusinessNo15a",
            "txtCompensationBusinessNo15b",
        ],
    ),
    ("p4_17", ["txtTaxDueSpeciala", "txtTaxDueSpecialb"]),
    (
        "p4_18a",
        ["TotalTaxDueCompensationATP", "TotalTaxDueCompensationASP"],
    ),
    (
        "p4_18b",
        ["TotalTaxDueCompensationBTP", "TotalTaxDueCompensationBSP"],
    ),
    (
        "p4_19",
        ["txtSalesRevenuesFeesANo19a", "txtSalesRevenuesFeesANo19b"],
    ),
    (
        "p4_20",
        [
            "txtOtherNonOperatingIncomeNo20a",
            "txtOtherNonOperatingIncomeNo20b",
        ],
    ),
    ("p4_21", ["txtTotalIncomeNo21a", "txtTotalIncomeNo21b"]),
    ("p4_22", ["txtLess22A", "txtLess22B"]),
    (
        "p4_23",
        ["txtTaxableIncomeLoss23A", "txtTaxableIncomeLoss23B"],
    ),
    (
        "p4_24",
        ["txtTaxDueBusinessIncome24A", "txtTaxDueBusinessIncome24B"],
    ),
    (
        "p4_25",
        [
            "txtCompensationBusinessNo25a",
            "txtCompensationBusinessNo25b",
        ],
    ),
    ("p5_1", ["txtPriorYearsNo1a", "txtPriorYearsNo1b"]),
    ("p5_2", ["txtTaxPaymentsNo2a", "txtTaxPaymentsNo2b"]),
    ("p5_3", ["txtCreditableTaxNo3a", "txtCreditableTaxNo3b"]),
    (
        "p5_4",
        [
            "txtCreditableFourthQuarterNo4a",
            "txtCreditableFourthQuarterNo4b",
        ],
    ),
    (
        "p5_5",
        ["txtCreditableForm2316No5a", "txtCreditableForm2316No5b"],
    ),
    (
        "p5_6",
        [
            "txtTaxPaidPreviouslyFiledNo6a",
            "txtTaxPaidPreviouslyFiledNo6b",
        ],
    ),
    (
        "p5_7",
        ["txtForeignTaxCreditsNo7a", "txtForeignTaxCreditsNo7b"],
    ),
    (
        "p5_8",
        ["txtSpecialTaxCreditsNo8a", "txtSpecialTaxCreditsNo8b"],
    ),
    (
        "p5_9",
        ["txtOtherCreditsPaymentsNo9a", "txtOtherCreditsPaymentsNo9b"],
    ),
    (
        "p5_10",
        [
            "txtTotalTaxCreditsPaymentsPartVNo10a",
            "txtTotalTaxCreditsPaymentsPartVNo10b",
        ],
    ),
    (
        "p6_1",
        ["totalTaxReliefAvailmentA", "totalTaxReliefAvailmentB"],
    ),
];

struct Check<'a> {
    stem: &'a str,
    slug: &'a str,
    failures: Vec<String>,
    checked: usize,
    dump: bool,
}

impl Check<'_> {
    fn compare(&mut self, what: &str, key: &str, official: &str, printed: &str, kind: Kind) {
        self.checked += 1;
        let ok = shows(official, printed, kind) || allowed(self.slug, key, official, printed);
        if self.dump || !ok {
            let line = format!(
                "{} ({}) {what} {key}: official={official:?} printed={printed:?}",
                self.stem, self.slug
            );
            if ok {
                println!("ok   {line}");
            } else {
                self.failures.push(line);
            }
        }
    }
}

#[test]
fn every_official_sample_prints_its_upload_on_the_rendered_sheet() {
    let dump = std::env::var_os("RENDERED_PARITY_DUMP").is_some();
    let mut failures = Vec::new();
    let mut checked = 0;
    let samples = official_samples();
    let identifiers = pinned_identifiers();
    let mut smallest: BTreeMap<&str, (f64, String)> = BTreeMap::new();
    for (stem, map) in &samples {
        let slug = frozen_slug_for_sample(stem);
        let xml = std::fs::read_to_string(samples_dir().join(format!("{stem}.official.xml")))
            .expect("official xml");
        let official = parse_official_xml_fields(&xml);
        let get = |key: &str| official.get(key).map(String::as_str).unwrap_or("");
        let amounts = official_amount_keys(slug);
        let rendered = filled_document_with_report(slug, map, None).expect("render");
        let (pages, band, plain) = split_pages(&rendered.html);
        let printed = Printed::of(&pages);
        let band_printed = Printed::of(&band);
        let plain_cells = plain_cells(&plain);
        let mut check = Check {
            stem,
            slug,
            failures: Vec::new(),
            checked: 0,
            dump,
        };
        // A text shortened with "..." counts as printed in full only when
        // the additional sheet carries its full text.
        let full_text = |cell: &str, shown: String| -> String {
            match printed.truncated.get(cell) {
                Some(full) if plain_cells.iter().any(|c| c == full.trim()) => full.clone(),
                _ => shown,
            }
        };
        let wc = merged_writer_cells_json(slug).expect("writer cells");
        let rows = |section: &str| wc[section].as_array().cloned().unwrap_or_default();
        let ids = |row: &serde_json::Value, one: &str, many: &str| -> Vec<String> {
            match (row.get(one), row.get(many)) {
                (Some(id), _) => vec![id.as_str().unwrap().to_string()],
                (_, Some(list)) => list
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect(),
                _ => Vec::new(),
            }
        };
        let mut covered: BTreeSet<String> = BTreeSet::new();

        // Stacked lines (an address over two combs): the item compares whole.
        let joins = rows("joins");
        let key_of_cell: BTreeMap<String, String> = joins
            .iter()
            .flat_map(|row| {
                let key = row["writer_key"].as_str().unwrap().to_string();
                ids(row, "html_id", "html_ids")
                    .into_iter()
                    .map(move |cell| (cell, key.clone()))
            })
            .collect();
        let mut stacked_cells: BTreeSet<String> = BTreeSet::new();
        for group in stacked_lines(slug).expect("stacked lines") {
            let keys: Vec<&String> = group.iter().filter_map(|c| key_of_cell.get(c)).collect();
            if keys.iter().any(|k| k.starts_with("derived:")) || keys.len() != group.len() {
                continue;
            }
            stacked_cells.extend(group.iter().cloned());
            let off: String = keys.iter().map(|k| get(k)).collect::<Vec<_>>().join("");
            let prt = group
                .iter()
                .map(|cell| full_text(cell, printed.cell(cell)))
                .collect::<Vec<_>>()
                .join(" ");
            let off = off.replace(char::is_whitespace, "");
            let prt_squashed = prt.replace(char::is_whitespace, "");
            check.compare("stacked", keys[0], &off, &prt_squashed, Kind::Text);
            covered.extend(keys.into_iter().cloned());
        }

        // 1. Text joins.
        for row in &joins {
            let key = row["writer_key"].as_str().unwrap();
            covered.insert(key.to_string());
            for cell in ids(row, "html_id", "html_ids") {
                if stacked_cells.contains(&cell) {
                    continue;
                }
                // An amount or an identifier (a TIN, an ATC) is never cut on
                // the page: only long text shortened with "..." may count its
                // full text on the additional sheet.
                let raw = printed.cell(&cell);
                let identifier = identifiers.contains(&(slug.to_string(), key.to_string()));
                if identifier {
                    check.compare(
                        &format!("identifier printed whole -> {cell}"),
                        key,
                        "",
                        if raw.contains("...") || printed.truncated.contains_key(&cell) {
                            raw.as_str()
                        } else {
                            ""
                        },
                        Kind::Text,
                    );
                }
                let shown = if identifier {
                    raw.clone()
                } else {
                    full_text(&cell, raw.clone())
                };
                if key.starts_with("derived:") {
                    let value = map.get(key).map(String::as_str).unwrap_or("");
                    // A nine-box TIN comb before a preprinted branch prints
                    // the nine digits; a non-zero branch prints over the
                    // preprinted 00000.
                    let shown = if value.len() == 14
                        && shown.len() == 9
                        && value.ends_with("00000")
                        && value.starts_with(shown.as_str())
                    {
                        value.to_string()
                    } else {
                        shown.clone()
                    };
                    let kind = if key.to_ascii_lowercase().contains("tin") {
                        Kind::Tin
                    } else if norm_numeric(value).is_some() || norm_numeric(&shown).is_some() {
                        Kind::Amount
                    } else {
                        Kind::Text
                    };
                    let shown = if kind == Kind::Amount {
                        raw.clone()
                    } else {
                        shown
                    };
                    check.compare("derived", key, value, &shown, kind);
                    if let Some(source) = derived_amount_source(slug, key) {
                        check.compare(
                            &format!("derived amount ({source})"),
                            key,
                            get(&source),
                            &shown,
                            Kind::Amount,
                        );
                        covered.insert(source);
                    }
                    continue;
                }
                let off = get(key);
                // An attachment count fills the sheet's digit boxes (`00` in
                // three boxes prints `000`): the same count.
                let kind = if is_attachment_count(key) {
                    Kind::Amount
                } else {
                    kind_of(key, off, &amounts)
                };
                let shown = if kind == Kind::Amount { raw } else { shown };
                check.compare(&format!("join -> {cell}"), key, off, &shown, kind);
            }
        }

        // 2. Money joins.
        for row in rows("money_joins") {
            let key = row["writer_key"].as_str().unwrap();
            covered.insert(key.to_string());
            let pesos = ids(&row, "peso_html_id", "peso_html_ids");
            let cents = ids(&row, "cent_html_id", "cent_html_ids");
            let off = get(key);
            let value = if key.starts_with("derived:") {
                map.get(key).map(String::as_str).unwrap_or("")
            } else {
                off
            };
            for (peso, cent) in pesos.iter().zip(&cents) {
                let p = printed.cell(peso);
                let c = printed.cell(cent);
                let shown = if c.trim().is_empty()
                    && !p.trim().is_empty()
                    && printed.overflow.contains_key(peso.as_str())
                {
                    p
                } else if p.trim().is_empty() && c.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}.{}", p.trim(), c.trim())
                };
                check.compare(
                    &format!("money -> {peso}+{cent}"),
                    key,
                    value,
                    &shown,
                    Kind::Amount,
                );
            }
            for cell in ids(&row, "decimal_html_id", "decimal_html_ids") {
                let shown = printed.decimal_comb(&cell);
                check.compare(
                    &format!("money -> {cell}"),
                    key,
                    value,
                    &shown,
                    Kind::Amount,
                );
            }
        }

        // 3. Whole-peso joins.
        for row in rows("peso_joins") {
            let key = row["writer_key"].as_str().unwrap();
            covered.insert(key.to_string());
            for cell in ids(&row, "html_id", "html_ids") {
                let shown = printed.cell(&cell);
                check.compare(
                    &format!("peso -> {cell}"),
                    key,
                    get(key),
                    &shown,
                    Kind::Amount,
                );
            }
        }

        // 4. Dates.
        for row in rows("date_joins") {
            let key = row["writer_key"].as_str().unwrap();
            covered.insert(key.to_string());
            let part = |field: &str| {
                row.get(field)
                    .and_then(|v| v.as_str())
                    .map(|cell| printed.cell(cell).trim().to_string())
            };
            let mm = part("mm_html_id").unwrap_or_default();
            let dd = part("dd_html_id");
            let yyyy = part("yyyy_html_id").unwrap_or_default();
            let shown = match dd {
                _ if mm.is_empty() && yyyy.is_empty() => String::new(),
                Some(dd) => format!("{mm}/{dd}/{yyyy}"),
                None => format!("{mm}/{yyyy}"),
            };
            // A date compares part by part as numbers: the boxes hold two
            // digits for the month (`1/01/2025` prints `01/01/2025`).
            let parts = |date: &str| -> Vec<Option<u32>> {
                date.trim()
                    .split('/')
                    .map(|part| part.trim().parse().ok())
                    .collect()
            };
            let off = get(key);
            let same = !off.trim().is_empty() && parts(off) == parts(&shown);
            check.compare(
                "date",
                key,
                off,
                if same { off } else { &shown },
                Kind::Text,
            );
        }

        // 5. Boxes ticked with an X.
        for row in rows("xbox_joins") {
            let key = row["writer_key"].as_str().unwrap();
            covered.insert(key.to_string());
            let value = if key.starts_with("derived:") {
                map.get(key).map(String::as_str).unwrap_or("")
            } else {
                get(key)
            };
            let on = matches!(value.trim(), "true" | "TRUE" | "1" | "Y" | "y" | "X" | "x");
            for cell in ids(&row, "html_id", "html_ids") {
                let shown = printed.cell(&cell);
                check.compare(
                    &format!("xbox -> {cell}"),
                    key,
                    if on { "true" } else { "false" },
                    if shown.trim() == "X" { "X" } else { "" },
                    Kind::Text,
                );
            }
        }

        // 6. Stamped inputs (`name=` the writer key).
        for name in printed.inputs.keys() {
            if name.starts_with('#') || covered.contains(name) || !official.contains_key(name) {
                continue;
            }
            covered.insert(name.clone());
            let off = get(name);
            let kind = kind_of(name, off, &amounts);
            check.compare("stamped", name, off, &printed.cell(name), kind);
        }

        // 7. The 1800/1801 joins frozen_html.rs adds (DONOR_ESTATE_JOINS).
        for (sheet, cell, keys, how) in DONOR_ESTATE {
            if *sheet != slug {
                continue;
            }
            covered.extend(keys.iter().map(|k| k.to_string()));
            let shown = printed.cell(cell);
            match *how {
                "tick" => {
                    let on = get(keys[0]) == "true";
                    check.compare(
                        &format!("sheet join -> {cell}"),
                        keys[0],
                        if on { "true" } else { "false" },
                        if shown.trim() == "X" { "X" } else { "" },
                        Kind::Text,
                    );
                }
                "date" => {
                    let parts: Vec<String> = keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| {
                            let v = get(k).trim().to_string();
                            if v.chars().all(|c| c == '0') {
                                String::new()
                            } else if i == 2 {
                                v[v.len().saturating_sub(2)..].to_string()
                            } else {
                                format!("{v:0>2}")
                            }
                        })
                        .collect();
                    check.compare(
                        &format!("sheet join -> {cell}"),
                        keys[0],
                        &parts.concat(),
                        &shown.replace(' ', ""),
                        Kind::Text,
                    );
                }
                "tin" => {
                    let off: String = keys.iter().map(|k| get(k)).collect();
                    check.compare(
                        &format!("sheet join -> {cell}"),
                        keys[0],
                        &off,
                        &shown,
                        Kind::Tin,
                    );
                }
                "branch" => {
                    let off = get(keys[0]).trim();
                    let off = if !off.is_empty() && off.len() < 5 {
                        format!("{off:0>5}")
                    } else {
                        off.to_string()
                    };
                    check.compare(
                        &format!("sheet join -> {cell}"),
                        keys[0],
                        &off,
                        &shown,
                        Kind::Text,
                    );
                }
                _ => {
                    let off = get(keys[0]);
                    let shown = full_text(cell, shown);
                    check.compare(
                        &format!("sheet join -> {cell}"),
                        keys[0],
                        off,
                        &shown,
                        Kind::Text,
                    );
                }
            }
        }

        // 8. Continuation pages: every cloned cell prints the upload's value
        //    for the control it holds (`data-upload-key`: the cloned row's
        //    own key, or the TOTAL row's total). Every clone but a centavo
        //    half (read with its peso cell) must carry that key: a clone
        //    without one fails, by sheet and cell.
        let mut cent_of: BTreeMap<String, String> = BTreeMap::new();
        let mut decimal_cells: BTreeSet<String> = BTreeSet::new();
        for row in rows("money_joins") {
            let pesos = ids(&row, "peso_html_id", "peso_html_ids");
            let cents = ids(&row, "cent_html_id", "cent_html_ids");
            cent_of.extend(pesos.into_iter().zip(cents));
            decimal_cells.extend(ids(&row, "decimal_html_id", "decimal_html_ids"));
        }
        let upload_keys = upload_keys(&band);
        let cents: BTreeSet<String> = cent_of.values().cloned().collect();
        let cloned = band_printed.writers.iter().map(|(cell, _)| cell);
        for cell in clones_without_upload_key(cloned, &upload_keys, &cents) {
            check.failures.push(format!(
                "{stem} ({slug}) continuation clone {cell} carries no data-upload-key"
            ));
        }
        for (cell, _) in &band_printed.writers {
            if !cell.starts_with("cont_") {
                continue;
            }
            // `cont_<schedule>_<page>_<band>_<row>_<cell>` (a cloned row) or
            // `cont_tot_<schedule>_<cell>` (the cloned TOTAL row).
            let (prefix, orig) = clone_origin(cell);
            let shown = if let Some(cent) = cent_of.get(orig) {
                let peso_cell = band_printed.cell(cell);
                let cent_cell = band_printed.cell(&format!("{prefix}{cent}"));
                if band_printed.overflow.contains_key(cell.as_str()) && cent_cell.trim().is_empty()
                {
                    peso_cell
                } else if peso_cell.trim().is_empty() && cent_cell.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}.{}", peso_cell.trim(), cent_cell.trim())
                }
            } else if decimal_cells.contains(orig) {
                band_printed.decimal_comb(cell)
            } else {
                band_printed.cell(cell)
            };
            if key_of_cell
                .get(orig)
                .is_some_and(|key| identifiers.contains(&(slug.to_string(), key.clone())))
            {
                check.compare(
                    "identifier printed whole",
                    cell,
                    "",
                    if shown.contains("...") {
                        shown.as_str()
                    } else {
                        ""
                    },
                    Kind::Text,
                );
            }
            if let Some(key) = upload_keys.get(cell) {
                let off = get(key);
                check.compare(
                    &format!("continuation -> {cell}"),
                    key,
                    off,
                    &shown,
                    kind_of(key, off, &amounts),
                );
            }
        }

        // 9. Every row no official cell holds (pop-up rows, rows past the
        //    sheet's last line), by its own keys: the print map holds the
        //    upload's value for each key, and the row prints on a
        //    continuation page (its cells carry the key, checked above) or
        //    on the additional sheet's table for its schedule, in its own
        //    row and column.
        let report = &rendered.overflow;
        let plain_rows = plain_table_rows(&plain);
        let on_band: BTreeSet<&String> = upload_keys.values().collect();
        for schedule in &report.schedules {
            for row in &schedule.rows {
                for (index, key) in row.keys.iter().enumerate() {
                    let off = get(key).trim();
                    if off.is_empty()
                        || matches!(off, "false" | "true")
                        || norm_numeric(off) == Some(0.0)
                    {
                        continue;
                    }
                    let kind = kind_of(key, off, &amounts);
                    let value = row.values.get(index).map(String::as_str).unwrap_or("");
                    check.compare("continued row: print map", key, off, value, kind);
                    if on_band.contains(key) {
                        continue;
                    }
                    let printed = plain_rows
                        .get(&(schedule.heading.clone(), row.row_number))
                        .and_then(|cells| cells.get(index))
                        .map(String::as_str)
                        .unwrap_or("");
                    check.compare(
                        &format!("continued row {} on the additional sheet", row.row_number),
                        key,
                        off,
                        printed,
                        kind,
                    );
                }
            }
        }

        // 10. The additional sheet's subtotals equal the sum of the upload's
        //     values for their rows' keys.
        for schedule in &report.schedules {
            for (index, column) in schedule.columns.iter().enumerate() {
                let Some(subtotal) = &column.subtotal else {
                    continue;
                };
                let sum: f64 = schedule
                    .rows
                    .iter()
                    .filter_map(|r| r.keys.get(index).and_then(|k| norm_numeric(get(k))))
                    .sum();
                let expected = format!("{sum:.2}");
                check.compare(
                    "additional-sheet subtotal",
                    &format!("{} / {}", schedule.heading, column.heading),
                    &expected,
                    subtotal,
                    Kind::Amount,
                );
                if !plain.is_empty() && !band.contains("cont_") {
                    let on_sheet = plain_cells.iter().any(|c| c == subtotal);
                    check.compare(
                        "additional-sheet subtotal printed",
                        &format!("{} / {}", schedule.heading, column.heading),
                        subtotal,
                        if on_sheet { subtotal.as_str() } else { "" },
                        Kind::Text,
                    );
                }
            }
        }

        // N1: every amount printed in a shrunk type: its size, and below
        // 5 pt it is also on the additional sheet.
        for (size, value, small) in amount_sizes(&pages).into_iter().chain(amount_sizes(&band)) {
            let entry = smallest.entry(slug).or_insert((size, stem.to_string()));
            if size < entry.0 {
                *entry = (size, stem.to_string());
            }
            if size < 5.0 {
                let carried = small && plain_cells.iter().any(|c| shows(&value, c, Kind::Amount));
                check.compare(
                    &format!("amount at {size} pt also on the additional sheet"),
                    &value,
                    &value,
                    if carried { value.as_str() } else { "" },
                    Kind::Text,
                );
            }
        }
        checked += check.checked;
        failures.extend(check.failures);
    }
    // N1: the smallest type any amount prints in, per sheet, over the 79
    // samples. A change that shrinks an amount further fails here.
    for (slug, (size, stem)) in &smallest {
        if dump {
            println!("smallest amount {slug}: {size} pt ({stem})");
        }
        let named = SMALLEST_AMOUNT_PT
            .iter()
            .find(|(sheet, _)| sheet == slug)
            .map(|(_, pt)| *pt);
        match named {
            Some(pt) if *size + 0.005 >= pt => {}
            _ => failures.push(format!(
                "{slug}: an amount prints at {size} pt ({stem}); SMALLEST_AMOUNT_PT names {named:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "rendered print parity: {} mismatches in {} checks over {} samples:\n{}",
        failures.len(),
        checked,
        samples.len(),
        failures.join("\n")
    );
}

/// frozen_html.rs DONOR_ESTATE_JOINS: `(sheet, cell, upload keys, how)`.
const DONOR_ESTATE: &[(&str, &str, &[&str], &str)] = &[
    (
        "1801-2018",
        "p1c4",
        &[
            "frm1801v2018:txtDateMonth",
            "frm1801v2018:txtDateDay",
            "frm1801v2018:txtDateYear",
        ],
        "date",
    ),
    (
        "1801-2018",
        "p1c31",
        &[
            "frm1801v2018:txtTINE1",
            "frm1801v2018:txtTINE2",
            "frm1801v2018:txtTINE3",
        ],
        "tin",
    ),
    (
        "1801-2018",
        "p1c32",
        &["frm1801v2018:txtBranchCodeE"],
        "branch",
    ),
    (
        "1801-2018",
        "p1c33",
        &["frm1801v2018:telephoneNumber"],
        "text",
    ),
    ("1801-2018", "p1c38", &["frm1801v2018:treatyY"], "text"),
    ("1801-2018", "p1c50", &["frm1801v2018:freq_Mon"], "tick"),
    ("1801-2018", "p1c53", &["frm1801v2018:freq_Qtr"], "tick"),
    ("1801-2018", "p1c51", &["frm1801v2018:freq_Semi"], "tick"),
    ("1801-2018", "p1c52", &["frm1801v2018:freq_Other"], "tick"),
    (
        "1801-2018",
        "p1c197",
        &["frm1801v2018:freqOtherInput"],
        "text",
    ),
    (
        "1801-2018",
        "p1c201",
        &["frm1801v2018:txtInstallmentYear"],
        "text",
    ),
    ("1800-2018", "p1c26", &["frm1800v2018:txtZipCode"], "text"),
    ("1800-2018", "p1c57", &["frm1800v2018:TaxTreaty"], "text"),
];

/// F1, on the rendered sheet: 1701Q-2025Q2's upload holds
/// `txt42A=-1,000.51`, which the official page shows; the sheet's Item 42
/// box is a whole-peso comb with no centavo boxes (a writer-cells
/// `peso_joins` row, p2c34). Before, the print rounded it to `-1001`.
/// A continuation clone that lost its `data-upload-key` is reported by sheet
/// and cell; a centavo half without one is not (it is read with its peso
/// cell), and a clone with its key passes.
#[test]
fn a_continuation_clone_without_its_upload_key_fails() {
    let clones: Vec<String> = [
        "cont_s1_2_0_1_p2c40",
        "cont_s1_2_0_1_p2c41",
        "cont_tot_s1_p2c60",
        "p2c40",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();
    let mut keys = BTreeMap::new();
    keys.insert("cont_s1_2_0_1_p2c40".to_string(), "frm:txtAmt1".to_string());
    keys.insert("cont_tot_s1_p2c60".to_string(), "frm:txtTotal".to_string());
    let cents: BTreeSet<String> = ["p2c41".to_string()].into_iter().collect();
    assert!(clones_without_upload_key(&clones, &keys, &cents).is_empty());
    keys.remove("cont_tot_s1_p2c60");
    assert_eq!(
        clones_without_upload_key(&clones, &keys, &cents),
        ["cont_tot_s1_p2c60"]
    );
    let no_cents = BTreeSet::new();
    assert_eq!(
        clones_without_upload_key(&clones, &keys, &no_cents),
        ["cont_s1_2_0_1_p2c41", "cont_tot_s1_p2c60"]
    );
}

#[test]
fn form_1701q_item_42_prints_the_uploads_centavos() {
    use bir_core::forms::queueable::QueueableForm as _;
    let map = super::form_1701q::sample_1701q().print_field_map();
    let xml = std::fs::read_to_string(samples_dir().join("1701Q-2025Q2.official.xml"))
        .expect("official xml");
    let official = parse_official_xml_fields(&xml);
    assert_eq!(official["frm1701q:txt42A"], "-1,000.51");
    let rendered = filled_document_with_report("1701q-2018", &map, None).expect("render");
    let (pages, _, _) = split_pages(&rendered.html);
    let printed = Printed::of(&pages);
    assert!(
        printed
            .slots("p2c34")
            .iter()
            .all(|input| input.value.is_empty())
    );
    assert_eq!(printed.cell("p2c34"), "-1,000.51");
}

/// TINs and ATCs are identifiers: they print whole on the page, shrunk to
/// their cell, never shortened with "..." (the 1600-PT/1600-VT Schedule 1
/// payee TINs printed `987654...` and `111222...`; 2200-P's Others ATCs
/// `9...`, 2200-AN's Schedule 1B Others ATC `X...`).
#[test]
fn tins_and_atcs_print_whole_on_the_page() {
    let cases: [(&str, &str, &[(&str, &str)]); 4] = [
        (
            "1600PT-062025",
            "1600-pt-2018",
            &[("p2c16", "987654321000"), ("p2c23", "111222339000")],
        ),
        (
            "1600VT-062025",
            "1600-vt-2018",
            &[("p2c16", "987654321000"), ("p2c23", "111222339000")],
        ),
        (
            "2200P-03132025",
            "2200p-2020",
            &[("p2c376", "999"), ("p2c390", "998")],
        ),
        ("2200AN-06302025", "2200an-2018", &[("p2c245", "XG130")]),
    ];
    let samples = official_samples();
    for (stem, slug, cells) in cases {
        let map = &samples.iter().find(|(s, _)| *s == stem).expect(stem).1;
        let rendered = filled_document_with_report(slug, map, None).expect("render");
        let (pages, _, _) = split_pages(&rendered.html);
        let printed = Printed::of(&pages);
        for (cell, value) in cells {
            assert_eq!(printed.cell(cell), *value, "{stem} {cell}");
            assert!(
                !printed.truncated.contains_key(*cell),
                "{stem} {cell} is marked for the additional sheet"
            );
        }
    }
}
