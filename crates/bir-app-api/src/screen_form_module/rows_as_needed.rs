//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552). Boxes the host writes through the typed draft because
//! the probed bindings cannot (a box showing one part of a draft field, a
//! whole number shown as one tick box per value), boxes the official page
//! keeps shut until another answer opens them, and the answers that remove
//! what the filer typed.
//!
//! - 0605 Item 3: one tick box per quarter, all four the draft's `quarter`
//!   (form_0605_xml.rs `insert_one_of_four`). Ticking one sets the quarter;
//!   unticking the ticked one clears it (the official page opens with none
//!   ticked). The return's identity is its entry number
//!   (Form0605Draft::filing_period, `OpenEnded(month)`), never the quarter.
//! - 1606 item 8C and 1706 item 7C: the other party's TIN, one draft field
//!   (`seller_tin`, `buyer_tin`) shown as three boxes and a branch code;
//!   the boxes fill in order (rows_1700.rs `with_tin_part`).
//! - Boxes the official page enables only on another answer refuse every
//!   write but a blank while that answer does not hold, with the tag the
//!   page shows on them ([`off`], rule 6), judged on the draft as the batch
//!   has recomputed it so far: 0605's item 17 Others box; 1606's relief list
//!   (item 11 Yes), Schedule 1 "Others" classification, Schedule 2
//!   specification and item 18's boxes (an over-remittance); 1706's items 8
//!   and 9 (an individual seller at item 3), relief list, Schedule 2
//!   specification and item 18's boxes (all rules/forms fields.json
//!   `enabled_when`); 2552's items 12A and 12B (an initial public offering
//!   at item 11) and Part IV rows 2-5 (the row before them used), which
//!   rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json
//!   keeps disabled at load. A blank (clearing) always goes through.
//! - 2552 item 11 picks the Part V schedule, and all three schedules keep
//!   their rows in the one draft list (form_2552.rs `schedule`): the
//!   binding alone would move the rows typed under one schedule into the
//!   next. The host answers it with `Form2552Draft::set_transaction`, which
//!   clears them as the official `kindOfTransaction` does, and reports the
//!   removal ([`removes`]) so the page asks first and offers Undo.
//! - 2552 boxes the corpus marks computed or the probe could not bind,
//!   which the filer types: Part IV rows 2-5's amounts
//!   (`exempt_transactions`), 12B (`outstanding_shares`) and Part V row 5
//!   while the schedule is not folded (row 5 reads OTHERS once the add-more
//!   rows hold rows 5 onward; they are edited there).
//! - 1707 item 8's Yes and No are the draft's `tax_relief` (`Some` holds
//!   8A): No clears what 8A holds, so it asks first. Schedule 2 row A's
//!   No. of Shares (no fields.json box) and Schedules 2 and 3's row D (row
//!   D reads OTHERS once the add-more rows hold row D onward) are written
//!   here. Its profile copy of the address shows the whole address the
//!   draft keeps, never the upload's cut first line.
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_0605::{Form0605DateField, Form0605Draft};
use bir_core::forms::form_1606::Form1606Draft;
use bir_core::forms::form_1706::Form1706Draft;
use bir_core::forms::form_1707::{
    FORM_1707_SCHEDULE_ROWS, Form1707Draft, Form1707Expense, Form1707Shares,
};
use bir_core::forms::form_2552::{
    FORM_2552_ROWS, Form2552Draft, Form2552ExemptRow, Form2552ShareRow, Form2552Transaction,
};
use serde_json::{Map, Value};

use super::rows_1700::with_tin_part;

/// A box this module writes or gates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// 0605 Item 3, quarter 1-4.
    Quarter0605(u8),
    /// 0605 item 17's Others (Specify) box.
    Others0605,
    /// 1606 item 8C: seller TIN box 0-2, or 3 (branch code).
    SellerTin1606(usize),
    /// 1706 item 7C: buyer TIN box 0-2.
    BuyerTin1706(usize),
    /// A box the official page opens only on another answer.
    Gate(Gate),
    /// 2552 item 11: Part V schedule 1-3.
    Transaction2552(u8),
    /// 2552 Part IV row 2-5's amount (row index 1-4).
    PartIvAmount2552(usize),
    /// 2552 item 12B.
    OutstandingShares2552,
    /// 2552 Part V row 5 of schedule 1-3.
    Row5_2552(u8, Row5),
    /// 1707 item 8 Yes (`true`) or No.
    TaxRelief1707(bool),
    /// 1707 Schedule 2 row `.0` (0 = A, 3 = D), column `.1` (0-3).
    Shares1707(usize, usize),
    /// 1707 Schedule 3 row D, column `.0` (0 particulars, 1 amount).
    ExpenseD1707(usize),
    /// 1606 and 1706 item 11 Yes.
    ReliefYes,
    /// A part of a one-date box (1706 Schedule 2 item 7, 1707 item 1).
    DatePart(DatePart),
}

/// Which part of a date a one-date box's key holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DatePart {
    Month,
    Day,
    Year,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DateWriter {
    /// Ordinary bindings; blank whole-number parts are written as zero.
    Ordinary,
    /// The date-part writer validates each part.
    Parts,
    /// The date-part writer also stores a cleared whole-number part as zero.
    WholeParts,
    /// The nullable 0605 date is judged and written by its own batch writer.
    Nullable,
    /// The estate date's own writer handles blank parts.
    Estate,
    /// The date identifies the return and cannot be edited.
    Identity,
}

pub(super) struct OneDate {
    pub(super) form: &'static str,
    pub(super) keys: [&'static str; 3],
    /// The old shell already drew this triple as one date.
    old_shell: bool,
    writer: DateWriter,
    /// A zero month or day hides all three shown parts, retaining the draft.
    hide_zero_parts: bool,
    /// The draft keeps each part as a plain whole number, so a typed "0" is
    /// how the page clears a part (1606, 1707); elsewhere "0" is not a date
    /// part and is refused with the date sentence.
    zero_blank: bool,
}

impl OneDate {
    const fn new(form: &'static str, keys: [&'static str; 3]) -> Self {
        Self {
            form,
            keys,
            old_shell: false,
            writer: DateWriter::Ordinary,
            hide_zero_parts: false,
            zero_blank: false,
        }
    }
    const fn old_shell(mut self) -> Self {
        self.old_shell = true;
        self
    }
    const fn writer(mut self, writer: DateWriter) -> Self {
        self.writer = writer;
        self
    }
    const fn hide_zero_parts(mut self) -> Self {
        self.hide_zero_parts = true;
        self
    }
    const fn zero_blank(mut self) -> Self {
        self.zero_blank = true;
        self
    }
}

/// A month and year the page shows as one MM/YYYY field (PR 3, P3a): the
/// month's key and the year's, in that order. `two_digit` marks a year box
/// that keeps 20YY (1600PT/1600VT's two-digit selects, 1702EX/1702MX/1702Q/
/// 1702RT's year boxes): the field takes and shows four digits but writes
/// the box's two-digit form.
pub(super) struct OnePeriod {
    pub(super) form: &'static str,
    /// [month, year] keys.
    pub(super) keys: [&'static str; 2],
    /// Which key's item the pair is drawn on when they sit apart: 0 the
    /// month's (2000), 1 the year's (1701, whose month the layout parked
    /// under Not on the printed sheet).
    at: usize,
    two_digit: bool,
}

impl OnePeriod {
    const fn new(form: &'static str, keys: [&'static str; 2]) -> Self {
        Self {
            form,
            keys,
            at: 0,
            two_digit: false,
        }
    }
    const fn year_item(mut self) -> Self {
        self.at = 1;
        self
    }
    const fn two_digit(mut self) -> Self {
        self.two_digit = true;
        self
    }
}

/// Every enumerated month+year pair (briefs/pr3-enumeration.md, P3a).
pub(super) const ONE_PERIODS: &[OnePeriod] = &[
    OnePeriod::new("0605", ["frm0605:itemYearEndMonth", "frm0605:txtYearEnded"]),
    OnePeriod::new("0619E", ["frm0619E:txtMonth", "frm0619E:txtYear"]),
    OnePeriod::new("0619F", ["frm0619F:txtMonth", "frm0619F:txtYear"]),
    OnePeriod::new(
        "1600PT",
        ["frm1600PT:txtPg1I1Month", "frm1600PT:txtPg1I1Year"],
    )
    .two_digit(),
    OnePeriod::new(
        "1600VT",
        ["frm1600VT:txtPg1I1Month", "frm1600VT:txtPg1I1Year"],
    )
    .two_digit(),
    OnePeriod::new("1601C", ["frm1601c:txtMonth", "frm1601c:txtYear"]),
    OnePeriod::new("1701", ["frm1701:txtPg1I1Month", "frm1701:txtPg1I1Year"]).year_item(),
    OnePeriod::new("1701A", ["frm1701A:txtMonth", "frm1701A:txtYear"]),
    OnePeriod::new("1701MS", ["frm1701MS:txtMonthNo1", "frm1701MS:txtYearNo1"]),
    OnePeriod::new("2000", ["frm2000:txtMonth", "frm2000:txtYear"]),
    OnePeriod::new("2550M", ["frm2550m:RtnYear", "frm2550m:txtYear"]),
    OnePeriod::new(
        "2550Q",
        ["frm2550qv2024:selectedMonthNo2", "frm2550qv2024:txtYearNo2"],
    ),
    OnePeriod::new("2551Q", ["frm2551Qv2018:rtnMonth", "frm2551Qv2018:txtYear"]),
    OnePeriod::new("2553", ["frm2553:itemYearEndMonth", "frm2553:txtYearEnded"]),
    OnePeriod::new(
        "1702EX",
        ["frm1702EX:ddlPg1I2Date", "frm1702EX:txtPg1I2YearEnd"],
    )
    .two_digit(),
    OnePeriod::new(
        "1702MX",
        ["frm1702MX:ddlPg1I2Date", "frm1702MX:txtPg1I2YearEnd"],
    )
    .two_digit(),
    OnePeriod::new("1702Q", ["frm1702q:rbYrEndMonth", "frm1702q:txtYrEndYear"]).two_digit(),
    OnePeriod::new(
        "1702RT",
        ["frm1702RT:ddlPg1I2Month", "frm1702RT:txtPg1I2Year"],
    )
    .two_digit(),
];

/// A from-to pair of dates on one item (PR 3, R1). Each end is anchored by
/// a whole-date box's key (`t: "date"`), or by a one-date's month key (its
/// triple's first): 1600WP's two triples.
pub(super) struct DateRange {
    pub(super) form: &'static str,
    /// [from, to] anchors.
    pub(super) ends: [&'static str; 2],
    /// The To end is copied from From by the form, so it is never an independent end.
    pub(super) derived_to: bool,
}

/// Every enumerated range (briefs/pr3-enumeration.md, R1).
pub(super) const DATE_RANGES: &[DateRange] = &[
    DateRange {
        form: "1600WP",
        ends: [
            "frm1600WP:DateWithholdingMonth",
            "frm1600WP:DateWithholdingToMonth",
        ],
        derived_to: true,
    },
    DateRange {
        form: "2550Q",
        ends: [
            "frm2550qv2024:RtnPeriodFromNo4",
            "frm2550qv2024:RtnPeriodToNo4",
        ],
        derived_to: false,
    },
    DateRange {
        form: "1702EX",
        ends: [
            "frm1702EX:txtPg1Pt1I17EffectivityFrom",
            "frm1702EX:txtPg1Pt1I17EffectivityTo",
        ],
        derived_to: false,
    },
    DateRange {
        form: "1701MS",
        ends: ["frm1701MS:txtTaxpayerNo16a", "frm1701MS:txtTaxpayerNo16a1"],
        derived_to: false,
    },
    DateRange {
        form: "1701MS",
        ends: ["frm1701MS:txtTaxpayerNo16b", "frm1701MS:txtTaxpayerNo16b1"],
        derived_to: false,
    },
    DateRange {
        form: "2551Q",
        ends: ["txtDateIssue", "txtDateExpiry"],
        derived_to: false,
    },
];

/// The page's own message for a month+year that is not one (PR 3, P3a).
pub(super) const PERIOD_ERROR: &str = "Type the month and year as MM/YYYY, for example 12/2023.";

/// The page's own message for a from-to pair whose end precedes its start
/// (PR 3, R1): the To end carries it.
pub(super) const RANGE_ERROR: &str = "The end date is before the start date.";

/// The month, day and year keys and their host and old-shell behavior.
pub(super) const ONE_DATES: &[OneDate] = &[
    OneDate::new(
        "0605",
        [
            "frm0605:txtDueDateMonth",
            "frm0605:txtDueDateDay",
            "frm0605:txtDueDateYear",
        ],
    )
    .writer(DateWriter::Nullable),
    OneDate::new(
        "0605",
        [
            "frm0605:txtReturnPeriodMonth",
            "frm0605:txtReturnPeriodDay",
            "frm0605:txtReturnPeriodYear",
        ],
    )
    .writer(DateWriter::Nullable),
    OneDate::new(
        "0619E",
        [
            "frm0619E:txtDueMonth",
            "frm0619E:txtDueDay",
            "frm0619E:txtDueYear",
        ],
    ),
    OneDate::new(
        "0619F",
        [
            "frm0619F:txtDueMonth",
            "frm0619F:txtDueDay",
            "frm0619F:txtDueYear",
        ],
    ),
    OneDate::new(
        "1604C",
        [
            "frm1604c:txtRefMonth",
            "frm1604c:txtRefDate",
            "frm1604c:txtRefYear",
        ],
    ),
    OneDate::new(
        "1606",
        [
            "frm1606:txtDateMonth",
            "frm1606:txtDateDay",
            "frm1606:txtDateYear",
        ],
    )
    .zero_blank(),
    OneDate::new(
        "1701Q",
        [
            "frm1701q:txtBirthMonth",
            "frm1701q:txtBirthDay",
            "frm1701q:txtBirthYear",
        ],
    )
    .old_shell(),
    OneDate::new(
        "1706",
        [
            "frm1706:txtDateMonth",
            "frm1706:txtDateDay",
            "frm1706:txtDateYear",
        ],
    ),
    OneDate::new(
        "1706",
        [
            "frm1706:txtDateMonthI",
            "frm1706:txtDateDayI",
            "frm1706:txtDateYearI",
        ],
    )
    .old_shell()
    .writer(DateWriter::Parts)
    .hide_zero_parts(),
    OneDate::new(
        "1707",
        [
            "frm1707:txtPg1I1Month",
            "frm1707:txtPg1I1Day",
            "frm1707:txtPg1I1Year",
        ],
    )
    .old_shell()
    .writer(DateWriter::WholeParts)
    .hide_zero_parts()
    .zero_blank(),
    OneDate::new(
        "2552",
        [
            "frm2552:txtPg1I1Month",
            "frm2552:txtPg1I1Day",
            "frm2552:txtPg1I1Year",
        ],
    )
    .old_shell()
    .writer(DateWriter::Identity)
    .hide_zero_parts(),
    OneDate::new(
        "1707A",
        [
            "frm1707Av2021:txtI1YearEndMonth",
            "frm1707Av2021:txtI1YearEndDay",
            "frm1707Av2021:txtI1YearEndYear",
        ],
    ),
    OneDate::new(
        "1800",
        [
            "frm1800v2018:txtMonth",
            "frm1800v2018:txtDate",
            "frm1800v2018:txtYear",
        ],
    ),
    OneDate::new(
        "1801",
        [
            "frm1801v2018:txtDateMonth",
            "frm1801v2018:txtDateDay",
            "frm1801v2018:txtDateYear",
        ],
    )
    .old_shell()
    .writer(DateWriter::Estate),
    OneDate::new(
        "2200A",
        [
            "frm2200Av2020:txtDateMonth",
            "frm2200Av2020:txtDateDay",
            "frm2200Av2020:txtDateYear",
        ],
    ),
    OneDate::new(
        "2200AN",
        [
            "frm2200ANv2018:txtMonth",
            "frm2200ANv2018:txtDate",
            "frm2200ANv2018:txtYear",
        ],
    ),
    OneDate::new(
        "2200C",
        [
            "frm2200C:txtPg1I1Month",
            "frm2200C:txtPg1I1Day",
            "frm2200C:txtPg1I1Year",
        ],
    ),
    OneDate::new(
        "2200M",
        [
            "frm2200M:txtPg1I1Month",
            "frm2200M:txtPg1I1Day",
            "frm2200M:txtPg1I1Year",
        ],
    ),
    OneDate::new(
        "2200P",
        [
            "frm2200P:txtPg1I1Month",
            "frm2200P:txtPg1I1Day",
            "frm2200P:txtPg1I1Year",
        ],
    ),
    OneDate::new(
        "2200S",
        [
            "frm2200S:txtMonth1",
            "frm2200S:txtDate",
            "frm2200S:txtForYr",
        ],
    ),
    OneDate::new(
        "2200T",
        [
            "frm2200Tv2020:txtDateMonth",
            "frm2200Tv2020:txtDateDay",
            "frm2200Tv2020:txtDateYear",
        ],
    ),
    // 1600WP item 1's from-to pair: two triples the range row draws as its
    // two ends (PR 3, R1). The To triple's boxes are the form's computed
    // ones (`c`): shown, never typed.
    OneDate::new(
        "1600WP",
        [
            "frm1600WP:DateWithholdingMonth",
            "frm1600WP:DateWithholdingDay",
            "frm1600WP:DateWithholdingYear",
        ],
    ),
    OneDate::new(
        "1600WP",
        [
            "frm1600WP:DateWithholdingToMonth",
            "frm1600WP:DateWithholdingToDay",
            "frm1600WP:DateWithholdingToYear",
        ],
    ),
];

fn registered_date(code: &str, key: &str) -> Option<&'static OneDate> {
    ONE_DATES
        .iter()
        .find(|date| date.form == code && date.keys.contains(&key))
}

pub(super) fn clear_date_part_as_zero(code: &str, key: &str) -> bool {
    registered_date(code, key).is_some_and(|date| date.writer == DateWriter::Ordinary)
}

fn date_part(code: &str, key: &str) -> Option<DatePart> {
    let date = registered_date(code, key)?;
    if !matches!(date.writer, DateWriter::Parts | DateWriter::WholeParts) {
        return None;
    }
    let at = date.keys.iter().position(|k| *k == key)?;
    Some([DatePart::Month, DatePart::Day, DatePart::Year][at])
}

/// The NSH page reads triples from `one_dates`; the old shell keeps only
/// the `one_date` marks it already used.
pub(super) fn mark_one_date(form: &mut Value, code: &str) {
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let fields: Vec<&str> = item["f"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|field| field["k"].as_str())
            .collect();
        let dates: Vec<_> = ONE_DATES
            .iter()
            .filter(|date| date.form == code && date.keys.iter().all(|key| fields.contains(key)))
            .collect();
        if !dates.is_empty() {
            if dates.len() == 1 && dates[0].old_shell && fields.len() == 3 {
                item["one_date"] = Value::Bool(true);
            }
            item["one_dates"] =
                serde_json::json!(dates.iter().map(|date| date.keys).collect::<Vec<_>>());
        }
    }
}

/// Where each drawn field of the form sits: key -> (part, item).
fn field_places(form: &Value) -> std::collections::BTreeMap<String, (usize, usize)> {
    let mut out = std::collections::BTreeMap::new();
    for (pi, part) in form["parts"].as_array().into_iter().flatten().enumerate() {
        for (ii, item) in part["items"].as_array().into_iter().flatten().enumerate() {
            for field in item["f"].as_array().into_iter().flatten() {
                if let Some(key) = field["k"].as_str() {
                    out.insert(key.to_string(), (pi, ii));
                }
            }
        }
    }
    out
}

/// The field object `key` of item `at`, taken out of its `f`.
fn take_field(form: &mut Value, at: (usize, usize), key: &str) -> Option<Value> {
    let fields = form["parts"][at.0]["items"][at.1]["f"].as_array_mut()?;
    let index = fields.iter().position(|f| f["k"] == key)?;
    Some(fields.remove(index))
}

/// The keys one range end writes: a one-date's whole triple, else the
/// whole-date box itself.
fn range_end_keys<'a>(code: &str, anchor: &'a str) -> Vec<&'a str> {
    ONE_DATES
        .iter()
        .find(|date| date.form == code && date.keys[0] == anchor)
        .map(|date| date.keys.to_vec())
        .unwrap_or_else(|| vec![anchor])
}

/// PR 3: bring each registered period's two boxes and each range's two ends
/// onto one item (1701's month out of Not on the printed sheet, 2000's year
/// beside its month, 1701MS item 16's second To, 1600WP's To triple), then
/// publish the `one_periods` and `date_ranges` marks the page draws the
/// combined controls from. An item the moves empty is dropped by the
/// page's withoutHidden.
pub(super) fn mark_periods(form: &mut Value, code: &str) {
    // (home key, the keys that join its item)
    let mut moves: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for period in ONE_PERIODS.iter().filter(|p| p.form == code) {
        moves.push((period.keys[period.at], vec![period.keys[1 - period.at]]));
    }
    for range in DATE_RANGES.iter().filter(|r| r.form == code) {
        let mut keys = range_end_keys(code, range.ends[0]);
        keys.extend(range_end_keys(code, range.ends[1]));
        // The from anchor itself stays; every other key of both ends joins
        // its item.
        keys.retain(|key| *key != range.ends[0]);
        if !keys.is_empty() {
            moves.push((range.ends[0], keys));
        }
    }
    if !moves.is_empty() {
        let places = field_places(form);
        for (home, keys) in moves {
            let Some(&at) = places.get(home) else {
                continue;
            };
            for key in keys {
                let Some(&from) = places.get(key) else {
                    continue;
                };
                if from == at {
                    continue;
                }
                if let Some(field) = take_field(form, from, key)
                    && let Some(fields) = form["parts"][at.0]["items"][at.1]["f"].as_array_mut()
                {
                    fields.push(field);
                }
            }
        }
    }
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let fields: Vec<(&str, &str)> = item["f"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|field| Some((field["k"].as_str()?, field["t"].as_str()?)))
            .collect();
        let keys: Vec<&str> = fields.iter().map(|(k, _)| *k).collect();
        let periods: Vec<[&str; 2]> = ONE_PERIODS
            .iter()
            .filter(|p| p.form == code && keys.contains(&p.keys[0]) && keys.contains(&p.keys[1]))
            .map(|p| p.keys)
            .collect();
        let ranges: Vec<[&str; 2]> = DATE_RANGES
            .iter()
            .filter(|range| {
                if range.form != code
                    || !keys.contains(&range.ends[0])
                    || !keys.contains(&range.ends[1])
                {
                    return false;
                }
                range.ends.iter().all(|anchor| {
                    // A whole-date box, or a date kept in a text box
                    // (2550Q's From), or a one-date's month key.
                    fields
                        .iter()
                        .any(|(k, t)| k == anchor && matches!(*t, "date" | "text"))
                        || ONE_DATES
                            .iter()
                            .any(|date| date.form == code && date.keys[0] == *anchor)
                })
            })
            .map(|range| range.ends)
            .collect();
        // The registry's two_digit pairs, by year key: the page writes the
        // box's own two-digit form without guessing from labels.
        let two_digit: Vec<&str> = ONE_PERIODS
            .iter()
            .filter(|p| {
                p.form == code
                    && p.two_digit
                    && keys.contains(&p.keys[0])
                    && keys.contains(&p.keys[1])
            })
            .map(|p| p.keys[1])
            .collect();
        let derived_to: Vec<&str> = DATE_RANGES
            .iter()
            .filter(|range| {
                range.form == code
                    && range.derived_to
                    && keys.contains(&range.ends[0])
                    && keys.contains(&range.ends[1])
            })
            .map(|range| range.ends[1])
            .collect();
        // All reads of `f` are done; write the marks now.
        drop(fields);
        drop(keys);
        if !periods.is_empty() {
            item["one_periods"] = serde_json::json!(periods);
        }
        if !two_digit.is_empty() {
            item["one_period_two_digit"] = serde_json::json!(two_digit);
        }
        if !ranges.is_empty() {
            item["date_ranges"] = serde_json::json!(ranges);
        }
        if !derived_to.is_empty() {
            item["derived_to"] = serde_json::json!(derived_to);
        }
    }
}

/// Write a whole-number box's blank, 0.
fn fill_whole_zero(kind: AgentFormKind, draft: &mut Value, key: &str) -> Result<(), String> {
    bir_core::forms::agent::fill_as_user(kind, draft, key, &Value::from("0"))
}

/// Lesson 13 on a one-date box's part: blank clears it; anything else is
/// the part of a MM/DD/YYYY date (a month 01-12, a day 01-31, a 4-digit
/// year), as the page sends it.
fn date_part_refusal(part: DatePart, value: &Value) -> Option<String> {
    let shown = text(value);
    if shown.is_empty() {
        return None;
    }
    let number: Option<u32> = shown
        .chars()
        .all(|c| c.is_ascii_digit())
        .then(|| shown.parse().ok())
        .flatten();
    let fits = match (part, number) {
        (DatePart::Month, Some(n)) => shown.len() <= 2 && (1..=12).contains(&n),
        (DatePart::Day, Some(n)) => shown.len() <= 2 && (1..=31).contains(&n),
        (DatePart::Year, Some(_)) => shown.len() == 4,
        _ => false,
    };
    (!fits).then(|| DATE_ERROR.to_string())
}

/// The page's own message for a date that is not one (form_page.js
/// `DATE_ERROR`), so the page and the host read the same.
pub(super) const DATE_ERROR: &str =
    "Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.";

/// Whether month/day/year make a date the page's `parseTypedDate` reads: a
/// real calendar day of a year 1900-2100.
fn real_date(month: u32, day: u32, year: u32) -> bool {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        _ => 0,
    };
    (1900..=2100).contains(&year) && day >= 1 && day <= days
}

/// A batch's one-date writes judged whole (lesson 13): once its month, day
/// and year are all there (the batch's own, else what the box shows), they
/// must be a real date; else every part of it the batch writes is refused
/// with the page's message, so no part lands alone.
pub(super) fn refuse_dates(
    kind: AgentFormKind,
    shown: &Value,
    edits: &[(String, Value)],
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for date in ONE_DATES
        .iter()
        .filter(|date| date.form == kind.form_code())
    {
        // An identity lock must answer before a date check; nullable dates
        // are judged by their own batch writer.
        if matches!(date.writer, DateWriter::Identity | DateWriter::Nullable) {
            continue;
        }
        let keys = date.keys;
        if !edits.iter().any(|(key, _)| keys.contains(&key.as_str())) {
            continue;
        }
        // A typed part is missing only when empty (or a whole-number "0"
        // where the draft spells blank as zero); a shown one also when it
        // is the official zero blank ("00", 1707's "0").
        let zero_blank = date.zero_blank;
        let part = |key: &str| -> Option<String> {
            match edits.iter().find(|(k, _)| k == key) {
                Some((_, v)) => {
                    Some(text(v)).filter(|v| !v.is_empty() && !(zero_blank && v == "0"))
                }
                None => Some(shown[key].as_str().unwrap_or_default().trim().to_string())
                    .filter(|v| !v.chars().all(|c| c == '0')),
            }
        };
        let [Some(month), Some(day), Some(year)] = keys.map(part) else {
            continue;
        };
        let number = |v: &str| v.parse::<u32>().ok();
        let fits = match (number(&month), number(&day), number(&year)) {
            (Some(m), Some(d), Some(y)) => year.len() == 4 && real_date(m, d, y),
            _ => false,
        };
        if !fits {
            for (key, _) in edits.iter().filter(|(key, _)| keys.contains(&key.as_str())) {
                out.insert(key.clone(), keys[0].to_string());
            }
        }
    }
    out
}

/// A period year's text as the box keeps it: four digits for a year box,
/// or the box's own 20YY/sel two-digit form (`two_digit`), which the page
/// and the field map spell 2000+n.
fn period_year_ok(value: &str, two_digit: bool) -> bool {
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match value.len() {
        4 => value
            .parse::<u32>()
            .is_ok_and(|y| (1900..=2100).contains(&y)),
        1..=2 if two_digit => value.parse::<u32>().is_ok_and(|n| n <= 99),
        _ => false,
    }
}

/// A batch's period writes judged whole (PR 3, P3a): once its month and
/// year are all there (the batch's own, else what the boxes show), the
/// month is 01-12 and the year the box's form; else every part of it the
/// batch writes is refused with the page's message, keyed to the month's
/// box, so no part lands alone. A blank (or a "00" month, 2553's empty
/// choice) on either side clears or holds the pair unjudged.
pub(super) fn refuse_periods(
    kind: AgentFormKind,
    shown: &Value,
    edits: &[(String, Value)],
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for period in ONE_PERIODS.iter().filter(|p| p.form == kind.form_code()) {
        let [month_key, year_key] = period.keys;
        if !edits
            .iter()
            .any(|(key, _)| key == month_key || key == year_key)
        {
            continue;
        }
        let resolve = |key: &str| -> Option<String> {
            match edits.iter().find(|(k, _)| k == key) {
                Some((_, v)) => Some(text(v)),
                None => Some(shown[key].as_str().unwrap_or_default().trim().to_string()),
            }
        };
        let (Some(month), Some(year)) = (resolve(month_key), resolve(year_key)) else {
            continue;
        };
        let unset = |v: &str| v.is_empty() || v.chars().all(|c| c == '0');
        if unset(&month) || unset(&year) {
            continue;
        }
        let month_ok = month.len() <= 2
            && month.chars().all(|c| c.is_ascii_digit())
            && month.parse::<u32>().is_ok_and(|n| (1..=12).contains(&n));
        if !(month_ok && period_year_ok(&year, period.two_digit)) {
            for (key, _) in edits
                .iter()
                .filter(|(key, _)| key == month_key || key == year_key)
            {
                out.insert(key.clone(), month_key.to_string());
            }
        }
    }
    out
}

/// One range end's date as (year, month, day): a one-date's triple from
/// the batch's writes else what the boxes show, or a whole-date box's own
/// MM/DD/YYYY text. None while either part is unset.
fn range_end_date(
    code: &str,
    shown: &Value,
    edits: &[(String, Value)],
    anchor: &str,
) -> Option<(u32, u32, u32)> {
    if let Some(date) = ONE_DATES
        .iter()
        .find(|date| date.form == code && date.keys[0] == anchor)
    {
        let zero_blank = date.zero_blank;
        let part = |key: &str| -> Option<String> {
            match edits.iter().find(|(k, _)| k == key) {
                Some((_, v)) => {
                    Some(text(v)).filter(|v| !v.is_empty() && !(zero_blank && v == "0"))
                }
                None => Some(shown[key].as_str().unwrap_or_default().trim().to_string())
                    .filter(|v| !v.chars().all(|c| c == '0')),
            }
        };
        let [Some(month), Some(day), Some(year)] = date.keys.map(part) else {
            return None;
        };
        let number = |v: &str| v.parse::<u32>().ok();
        let (m, d, y) = (number(&month)?, number(&day)?, number(&year)?);
        return Some((if y < 100 { 2000 + y } else { y }, m, d));
    }
    let raw = match edits.iter().find(|(k, _)| k == anchor) {
        Some((_, v)) => text(v),
        None => shown[anchor]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
    };
    shown_ymd(&raw)
}

/// A shown whole date as (year, month, day): the draft's MM/DD/YYYY, or an
/// ISO day.
fn shown_ymd(text: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = text
        .trim()
        .split(['/', '-', '.'])
        .filter(|p| !p.is_empty())
        .collect();
    let (ys, ms, ds) = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => (*y, *m, *d),
        [m, d, y] if y.len() == 4 => (*y, *m, *d),
        _ => return None,
    };
    Some((ys.parse().ok()?, ms.parse().ok()?, ds.parse().ok()?))
}

/// A batch's range writes judged whole (PR 3, R1): when a touched from-to
/// pair resolves both ends to dates — the batch's own writes else what the
/// boxes show, so both ends in one batch and a stored far end both count —
/// and the end precedes the start, every key of either end the batch
/// writes is refused and the To anchor carries the page's message, so the
/// assistant/agent path cannot store a reversed range either.
pub(super) fn refuse_ranges(
    kind: AgentFormKind,
    shown: &Value,
    edits: &[(String, Value)],
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for range in DATE_RANGES
        .iter()
        .filter(|range| range.form == kind.form_code() && !range.derived_to)
    {
        let ends: Vec<Vec<&str>> = range
            .ends
            .iter()
            .map(|anchor| range_end_keys(kind.form_code(), anchor))
            .collect();
        let touched = |key: &str| ends.iter().flatten().any(|end| *end == key);
        if !edits.iter().any(|(key, _)| touched(key)) {
            continue;
        }
        let (Some(from), Some(to)) = (
            range_end_date(kind.form_code(), shown, edits, range.ends[0]),
            range_end_date(kind.form_code(), shown, edits, range.ends[1]),
        ) else {
            continue;
        };
        if from <= to {
            continue;
        }
        for (key, _) in edits.iter().filter(|(key, _)| touched(key)) {
            out.insert(key.clone(), range.ends[1].to_string());
        }
    }
    out
}

/// 0605's Items 4 and 7 keep three month/day/year boxes on a draft whose
/// dates start empty (B1): the parts are whole-number bindings into
/// `Option` dates, so a batch touching a group is judged as one date and
/// written whole — an impossible complete date refuses every part it
/// sent, a partial one lands its parts.
pub(super) struct SplitDateGroup {
    /// Which draft date the group writes.
    pub which: Form0605DateField,
    /// The (month, day, year) box keys, in part order.
    pub keys: [&'static str; 3],
    /// What the batch sends per part; `None` = the batch does not touch it.
    pub sent: [Option<String>; 3],
    /// What each box shows now (the draft's parts), `None` = blank.
    pub current: [Option<u16>; 3],
}

/// The two split dates of Form 0605: `(which, [month, day, year] keys)`.
const SPLIT_DATES_0605: [(Form0605DateField, [&str; 3]); 2] = [
    (
        Form0605DateField::DueDate,
        [
            "frm0605:txtDueDateMonth",
            "frm0605:txtDueDateDay",
            "frm0605:txtDueDateYear",
        ],
    ),
    (
        Form0605DateField::ReturnPeriod,
        [
            "frm0605:txtReturnPeriodMonth",
            "frm0605:txtReturnPeriodDay",
            "frm0605:txtReturnPeriodYear",
        ],
    ),
];

/// The split-date groups of this batch: only 0605 has them, and only a
/// group the batch actually writes is returned.
pub(super) fn split_date_groups(
    kind: AgentFormKind,
    shown: &Value,
    edits: &[(String, Value)],
) -> Vec<SplitDateGroup> {
    if kind.form_code() != "0605" {
        return Vec::new();
    }
    let sent_text = |key: &str| -> Option<String> {
        edits.iter().find(|(k, _)| k == key).map(|(_, v)| text(v))
    };
    let shown_part = |key: &str| -> Option<u16> {
        let shown = shown[key].as_str().unwrap_or_default().trim();
        if shown.is_empty() || shown.chars().all(|c| c == '0') {
            return None;
        }
        shown.parse::<u16>().ok()
    };
    SPLIT_DATES_0605
        .iter()
        .map(|(which, keys)| SplitDateGroup {
            which: *which,
            keys: *keys,
            sent: keys.map(&sent_text),
            current: keys.map(&shown_part),
        })
        .filter(|group| group.sent.iter().any(Option::is_some))
        .collect()
}

/// What a stored one-date box holding only part of a date (a month with
/// no day, a day with no month) says: the page shows it blank.
fn partial_date(
    [month, day, year]: [&str; 3],
    values: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let get = |key: &str| {
        values
            .get(key)
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    };
    let (m, d, y) = (get(month), get(day), get(year));
    let unset = |v: &str| v.is_empty() || v.chars().all(|c| c == '0');
    let (no_month, no_day) = (unset(&m), unset(&d));
    if no_month == no_day {
        return None;
    }
    let what = if no_day { "day" } else { "month" };
    let m = if no_month {
        "__".to_string()
    } else {
        format!("{m:0>2}")
    };
    let d = if no_day {
        "__".to_string()
    } else {
        format!("{d:0>2}")
    };
    Some(format!(
        "The stored date has no {what} ({m}/{d}/{y}); type the full date."
    ))
}

/// The page's notices for a draft, shown with its messages: a stored
/// partial one-date (display only; the draft and upload are unchanged).
pub(super) fn notices(kind: AgentFormKind, draft: &Value) -> Vec<Value> {
    let Ok(map) = bir_core::forms::agent::field_map(kind, draft) else {
        return Vec::new();
    };
    ONE_DATES
        .iter()
        .filter(|date| date.form == kind.form_code())
        .filter_map(|date| {
            partial_date(date.keys, &map)
                .map(|message| serde_json::json!({ "field": "one_date", "message": message, "keys": date.keys }))
        })
        .collect()
}

/// A column of 2552's Part V row 5.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Row5 {
    Date,
    Seller,
    Buyer,
    Corp,
    NumShares,
    TaxBase,
    TaxRate,
}

const ROW5_COLUMNS: [(&str, Row5); 7] = [
    ("Date", Row5::Date),
    ("Seller", Row5::Seller),
    ("Buyer", Row5::Buyer),
    ("Corp", Row5::Corp),
    ("NumShares", Row5::NumShares),
    ("TaxBase", Row5::TaxBase),
    ("TaxRate", Row5::TaxRate),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Gate {
    /// 1606 item 11's list: item 11 Yes.
    Relief1606,
    /// 1606 Schedule 1 "Others (Specify)": classification Others.
    ClassOther1606,
    /// 1606 Schedule 2 specification: Exempt or Others.
    Transaction1606,
    /// 1706 items 8 and 9: an individual seller (item 3 WI 450).
    Individual1706,
    /// 1706 item 11's list: item 11 Yes.
    Relief1706,
    /// 1706 Schedule 2 specification: Exempt or Others.
    Transaction1706,
    /// 1606 and 1706 item 18's boxes: item 18 below zero.
    Overpaid,
    /// 2552 items 12A and 12B: item 11 an initial public offering.
    Offering2552,
    /// 2552 Part IV row `.0` (index 1-4): the row before it used.
    PartIvRow2552(usize),
    /// 0605 item 17's Others box: Others is the manner.
    Others0605,
}

/// The tag of a 2552 gate (2552's corpus records no `enabled_when`; the
/// official runtime keeps these boxes disabled at load, rules/forms/
/// 2552-v2018/fixtures/runtime-control-inventory-v796.json).
pub(super) const OFFERING_2552: &str =
    "Needs item 11 = Initial Public Offering (Primary or Secondary)";

/// The tag of 2552's Part IV row `row` (1-based, 2-5).
pub(super) fn part_iv_tag(row: usize) -> String {
    format!("Needs Part IV row {}", row - 1)
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    if let Some(part) = date_part(kind.form_code(), key) {
        return Some(Cell::DatePart(part));
    }
    let tin_part = |suffix: &str| match suffix {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        _ => None,
    };
    match kind.form_code() {
        "0605" => {
            if key == "frm0605:txtOthersName" {
                return Some(Cell::Others0605);
            }
            let n: u8 = key.strip_prefix("itemQuarter_")?.parse().ok()?;
            (1..=4).contains(&n).then_some(Cell::Quarter0605(n))
        }
        "1606" => match key {
            "frm1606:txtBranchCodeS" => Some(Cell::SellerTin1606(3)),
            "frm1606:rdTreaty:_1" => Some(Cell::ReliefYes),
            "frm1606:selTreaty" => Some(Cell::Gate(Gate::Relief1606)),
            "frm1606:j_id393_7" => Some(Cell::Gate(Gate::ClassOther1606)),
            "frm1606:txtOthers20" => Some(Cell::Gate(Gate::Transaction1606)),
            "frm1606:opt37" | "frm1606:opt37:_1" | "frm1606:opt37:_2" => {
                Some(Cell::Gate(Gate::Overpaid))
            }
            _ => tin_part(key.strip_prefix("frm1606:txtTINS")?).map(Cell::SellerTin1606),
        },
        "1706" => match key {
            "frm1706:j_id392" | "frm1706:j_id392:_1" | "frm1706:j_id392:_2" | "frm1706:j_id393"
            | "frm1706:j_id393:_1" | "frm1706:j_id393:_2" => Some(Cell::Gate(Gate::Individual1706)),
            "frm1706:opt37" | "frm1706:opt37:_1" | "frm1706:opt37:_2" => {
                Some(Cell::Gate(Gate::Overpaid))
            }
            "frm1706:selTreaty" => Some(Cell::Gate(Gate::Relief1706)),
            "frm1706:rdTreaty:_1" => Some(Cell::ReliefYes),
            "frm1706:txtOthers21" => Some(Cell::Gate(Gate::Transaction1706)),
            _ => tin_part(key.strip_prefix("frm1706:txtTINB")?).map(Cell::BuyerTin1706),
        },
        "2552" => cell_2552(key),
        "1707" => cell_1707(key),
        _ => None,
    }
}

fn cell_1707(key: &str) -> Option<Cell> {
    let rest = key.strip_prefix("frm1707:")?;
    match rest {
        "rdoPg1I8TaxReliefYes" => return Some(Cell::TaxRelief1707(true)),
        "rdoPg1I8TaxReliefNo" => return Some(Cell::TaxRelief1707(false)),
        // Row A's No. of Shares: fields.json lists no box for it.
        "txtPg2P4Sch2NoOfShares1" => return Some(Cell::Shares1707(0, 1)),
        "txtPg2P4Sch3Particulars4" => return Some(Cell::ExpenseD1707(0)),
        "txtPg2P4Sch3Amount4" => return Some(Cell::ExpenseD1707(1)),
        _ => {}
    }
    let row_d = rest.strip_prefix("txtPg2P4Sch2")?.strip_suffix('4')?;
    let column = [
        "NameOfCorpStock",
        "NoOfShares",
        "StockCertNo",
        "TaxBaseSellPrice",
    ]
    .iter()
    .position(|name| *name == row_d)?;
    Some(Cell::Shares1707(FORM_1707_SCHEDULE_ROWS - 1, column))
}

fn cell_2552(key: &str) -> Option<Cell> {
    let rest = key.strip_prefix("frm2552:")?;
    match rest {
        "rdoPg1I11TransactionLSE" => return Some(Cell::Transaction2552(1)),
        "rdoPg1I11TransactionPrimary" => return Some(Cell::Transaction2552(2)),
        "rdoPg1I11TransactionSecondary" => return Some(Cell::Transaction2552(3)),
        "txtPg1Pt1I12TotShare" => return Some(Cell::OutstandingShares2552),
        "txtPg1Pt1I12NumOfShare" => return Some(Cell::Gate(Gate::Offering2552)),
        _ => {}
    }
    if let Some(n) = rest
        .strip_prefix("txtPg2P4I")
        .and_then(|r| r.strip_suffix("TransClass"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        return (2..=FORM_2552_ROWS)
            .contains(&n)
            .then_some(Cell::Gate(Gate::PartIvRow2552(n - 1)));
    }
    if let Some(n) = rest
        .strip_prefix("txtPg2P4I")
        .and_then(|r| r.strip_suffix("Amount"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        return (2..=FORM_2552_ROWS)
            .contains(&n)
            .then_some(Cell::PartIvAmount2552(n - 1));
    }
    let rest = rest.strip_prefix("txtPg2Pt5Sch")?;
    let schedule = rest.chars().next()?.to_digit(10)? as u8;
    let column = rest[1..].strip_suffix('5')?;
    let (_, col) = ROW5_COLUMNS.iter().find(|(name, _)| *name == column)?;
    // Schedule 1's rate is the fixed 6/10 of 1%.
    ((1..=3).contains(&schedule) && !(schedule == 1 && *col == Row5::TaxRate))
        .then_some(Cell::Row5_2552(schedule, *col))
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page). A gated box keeps its own binding.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    cell(kind, key).is_some_and(|cell| !matches!(cell, Cell::Gate(_) | Cell::ReliefYes))
}

/// A batch fills a TIN's boxes left to right, and a gated box after the
/// answer that opens it (item 11 Yes, then its list).
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    match cell(kind, key)? {
        Cell::SellerTin1606(part) | Cell::BuyerTin1706(part) => Some(1 + part),
        Cell::Gate(_) => Some(1),
        // Item 11 picks the schedule before its rows are typed.
        Cell::Transaction2552(_) => Some(0),
        Cell::PartIvAmount2552(_) | Cell::OutstandingShares2552 | Cell::Row5_2552(..) => Some(1),
        // Item 8's answer before its 8A text in one batch.
        Cell::TaxRelief1707(_) => Some(0),
        Cell::Shares1707(..) | Cell::ExpenseD1707(_) => Some(1),
        // Item 17's manner before its Others text in one batch.
        Cell::Others0605 => Some(1),
        Cell::Quarter0605(_) | Cell::ReliefYes | Cell::DatePart(_) => None,
    }
}

fn ticked(value: &Value) -> bool {
    matches!(value, Value::Bool(true)) || value.as_str().is_some_and(|s| s.trim() == "true")
}

fn text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().trim().to_string(),
    }
}

/// A value that writes nothing to `gate`'s box: blank, or an untick of a
/// tick box ("false" is text a text box keeps).
fn clears(gate: Gate, value: &Value) -> bool {
    let shown = text(value);
    shown.is_empty() || (shown == "false" && matches!(gate, Gate::Individual1706 | Gate::Overpaid))
}

fn typed<T: serde::de::DeserializeOwned>(draft: &Value) -> Result<T, String> {
    serde_json::from_value(draft.clone()).map_err(|e| e.to_string())
}

fn store<T: serde::Serialize>(draft: &mut Value, form: &T) -> Result<(), String> {
    *draft = serde_json::to_value(form).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write `key` when this module owns it: `None` leaves it to the ordinary
/// fill. `ordinary` is that fill, for a box this module only gates.
pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    ordinary: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Option<Result<(), String>> {
    let cell = cell(kind, key)?;
    // A gated box is judged on the draft the batch has made so far, as the
    // form recomputes it (item 18's over-remittance is a computed total).
    if let Some(gate) = gate_of(cell)
        && !clears(gate, value)
        && !open(gate, &recomputed(kind, draft))
    {
        return Some(Err(gate_tag(kind, gate, key)));
    }
    Some(match cell {
        Cell::Quarter0605(quarter) => fill_0605_quarter(draft, quarter, value),
        Cell::Others0605 => fill_0605_others(draft, value),
        Cell::SellerTin1606(part) => fill_1606_seller_tin(draft, part, value),
        Cell::BuyerTin1706(part) => fill_1706_buyer_tin(draft, part, value),
        Cell::Transaction2552(schedule) => fill_2552_transaction(draft, schedule, value),
        Cell::PartIvAmount2552(row) => fill_2552_part_iv(draft, row, value),
        Cell::OutstandingShares2552 => fill_2552_outstanding(draft, value),
        Cell::Row5_2552(_, column) => fill_2552_row5(draft, key, column, value),
        Cell::TaxRelief1707(yes) => fill_1707_tax_relief(draft, yes, value),
        Cell::Shares1707(row, column) => fill_1707_shares(draft, row, column, value),
        Cell::ExpenseD1707(column) => fill_1707_expense_d(draft, column, value),
        Cell::Gate(gate) => {
            let untick = text(value) == "false";
            // Item 18's two boxes are one official radio pair (rules/forms/
            // 1606-v2018 and 1706-v2018 fixtures/runtime-control-inventory-
            // v796.json: frm1606:opt37:_1/_2 and frm1706:opt37:_1/_2 share the
            // name frm1606:opt37 / frm1706:opt37), which the official page
            // never unticks: an untick answers nothing on either form (the
            // probe bound 1606's to "none" and 1706's to the other box).
            // Unticking a box of a shut pair writes nothing either: its
            // binding's "false" would set the other answer.
            if untick && (gate == Gate::Overpaid || !open(gate, &recomputed(kind, draft))) {
                Ok(())
            } else {
                ordinary(draft)
            }
        }
        Cell::DatePart(part) => match date_part_refusal(part, value) {
            Some(why) => Err(why),
            None if registered_date(kind.form_code(), key)
                .is_some_and(|date| date.writer == DateWriter::WholeParts)
                && text(value).is_empty() =>
            {
                fill_whole_zero(kind, draft, key)
            }
            None => ordinary(draft),
        },
        Cell::ReliefYes => {
            // Yes again keeps the relief chosen from the list (Yes is "yes,
            // unspecified": probe.rs CHOICE_OPTION_SETS).
            let held = draft["tax_relief"]
                .as_str()
                .is_some_and(|relief| relief != "no");
            if ticked(value) && held {
                Ok(())
            } else {
                ordinary(draft)
            }
        }
    })
}

#[inline(never)]
fn fill_0605_quarter(draft: &mut Value, quarter: u8, value: &Value) -> Result<(), String> {
    let mut form: Form0605Draft = typed(draft)?;
    if ticked(value) {
        form.quarter = quarter;
    } else if form.quarter == quarter {
        form.quarter = 0;
    } else {
        return Ok(());
    }
    form.recompute();
    store(draft, &form)
}

/// `draft` as the form recomputes it (unchanged when it cannot).
fn recomputed(kind: AgentFormKind, draft: &Value) -> Value {
    let mut copy = draft.clone();
    if bir_core::forms::agent::compute(kind, &mut copy).is_err() {
        return draft.clone();
    }
    copy
}

/// The gate a box waits behind, if any.
fn gate_of(cell: Cell) -> Option<Gate> {
    match cell {
        Cell::Gate(gate) => Some(gate),
        Cell::OutstandingShares2552 => Some(Gate::Offering2552),
        Cell::Others0605 => Some(Gate::Others0605),
        Cell::PartIvAmount2552(row) => Some(Gate::PartIvRow2552(row)),
        _ => None,
    }
}

/// What a shut gate says, on the page (`off`) and in a refusal: the
/// corpus condition's reviewed tag (app/forms/condition-labels.json), or
/// 2552's own.
fn gate_tag(kind: AgentFormKind, gate: Gate, key: &str) -> String {
    match gate {
        Gate::Offering2552 => OFFERING_2552.to_string(),
        Gate::PartIvRow2552(row) => part_iv_tag(row + 1),
        // A group key (frm1706:j_id392) has its first member's tag.
        _ => super::conditions::tag(kind.form_code(), key)
            .or_else(|| super::conditions::tag(kind.form_code(), &format!("{key}:_1")))
            .unwrap_or_else(|| "Needs another answer on this form".to_string()),
    }
}

/// Every gated box of `kind`, with its gate.
fn gated_keys(kind: AgentFormKind) -> Vec<(String, Gate)> {
    let keys: Vec<String> = match kind.form_code() {
        "0605" => vec!["frm0605:txtOthersName".into()],
        "1606" => [
            "selTreaty",
            "j_id393_7",
            "txtOthers20",
            "opt37:_1",
            "opt37:_2",
        ]
        .iter()
        .map(|k| format!("frm1606:{k}"))
        .collect(),
        "1706" => [
            "j_id392:_1",
            "j_id392:_2",
            "j_id393:_1",
            "j_id393:_2",
            "selTreaty",
            "txtOthers21",
            "opt37:_1",
            "opt37:_2",
        ]
        .iter()
        .map(|k| format!("frm1706:{k}"))
        .collect(),
        "2552" => {
            let mut keys = vec![
                "frm2552:txtPg1Pt1I12NumOfShare".to_string(),
                "frm2552:txtPg1Pt1I12TotShare".to_string(),
            ];
            for n in 2..=FORM_2552_ROWS {
                keys.push(format!("frm2552:txtPg2P4I{n}TransClass"));
                keys.push(format!("frm2552:txtPg2P4I{n}Amount"));
            }
            keys
        }
        _ => Vec::new(),
    };
    keys.into_iter()
        .filter_map(|key| {
            let gate = gate_of(cell(kind, &key)?)?;
            Some((key, gate))
        })
        .collect()
}

/// A value shown as nothing (conditions.rs `is_blank`).
fn shows_nothing(shown: &str) -> bool {
    let shown = shown.trim();
    shown == "false" || shown.chars().all(|c| matches!(c, '0' | '.' | ',' | '-'))
}

/// The host gates that are shut on `draft` right now, each with its tag,
/// for the boxes that show nothing (rule 6; conditions.rs `off_boxes`
/// takes these, since its probe sees a refusal rather than a clearing).
pub(super) fn off(kind: AgentFormKind, draft: &Value, values: &Value) -> Map<String, Value> {
    let gated = gated_keys(kind);
    if gated.is_empty() {
        return Map::new();
    }
    let seen = recomputed(kind, draft);
    gated
        .into_iter()
        .filter(|(key, gate)| {
            shows_nothing(values[key.as_str()].as_str().unwrap_or_default()) && !open(*gate, &seen)
        })
        .map(|(key, gate)| {
            let tag = gate_tag(kind, gate, &key);
            (key, Value::from(tag))
        })
        .collect()
}

#[cfg(test)]
/// Whether `code`'s box `key` is one of this module's gates (the dropdown
/// audit skips only these while they are off).
pub(super) fn is_gated(code: &str, key: &str) -> bool {
    AgentFormKind::from_code(code)
        .is_some_and(|kind| gated_keys(kind).iter().any(|(k, _)| k == key))
}

/// 0605 item 17's Others box: written while Others is the manner
/// (form_0605_official.rs clears it otherwise, as `disabletxtOthers()`).
#[inline(never)]
fn fill_0605_others(draft: &mut Value, value: &Value) -> Result<(), String> {
    let mut form: Form0605Draft = typed(draft)?;
    form.other_manner_description = text(value);
    form.recompute();
    store(draft, &form)
}

/// Whether a gated box's answer holds on `draft`.
fn open(gate: Gate, draft: &Value) -> bool {
    let is = |path: &str, values: &[&str]| {
        draft
            .pointer(path)
            .and_then(Value::as_str)
            .is_some_and(|v| values.contains(&v))
    };
    let below_zero = |path: &str| {
        draft
            .pointer(path)
            .and_then(Value::as_f64)
            .is_some_and(|n| n < 0.0)
    };
    match gate {
        // fields.json enabled_when, frm1606:selTreaty "Item 19 indicates
        // treaty or special-law relief." and frm1706:selTreaty "Item 20
        // indicates tax-treaty relief.": item 19/20 is the old numbering of
        // item 11, whose Yes answers "tax relief under an International Tax
        // Treaty or Special Law" (the printed question); the list then
        // picks which. So Yes opens it on both forms.
        Gate::Relief1606 | Gate::Relief1706 => {
            !is("/tax_relief", &["no"]) && draft.pointer("/tax_relief").is_some()
        }
        Gate::ClassOther1606 => is("/property_class", &["others"]),
        Gate::Transaction1606 => is("/transaction", &["exempt", "others"]),
        Gate::Individual1706 => is("/seller_type", &["individual"]),
        Gate::Transaction1706 => is("/transaction_type", &["exempt", "others"]),
        // fields.json "Item 36 is negative.": 1606's total_amount_due,
        // 1706's total_amount_payable.
        Gate::Overpaid => below_zero("/total_amount_due") || below_zero("/total_amount_payable"),
        Gate::Others0605 => is("/manner_of_payment", &["Others"]),
        Gate::Offering2552 => is("/transaction", &["primary_offering", "secondary_offering"]),
        // Inferred: the official runtime keeps rows 2-5 disabled at load
        // (runtime-control-inventory-v796.json txtPg2P4I2..I5 disabled:
        // true); what enables them (enablePartIVRow) is tracked as a hash
        // only, so the port keeps the rows in order: row N opens once row
        // N-1 is used.
        Gate::PartIvRow2552(row) => {
            draft["exempt_transactions"]
                .get(row - 1)
                .is_some_and(|before| {
                    before["classification"]
                        .as_str()
                        .is_some_and(|c| !c.trim().is_empty())
                        || before["amount"].as_f64().is_some_and(|a| a != 0.0)
                })
        }
    }
}

#[inline(never)]
fn fill_1606_seller_tin(draft: &mut Value, part: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1606Draft = typed(draft)?;
    form.seller_tin = with_tin_part(&form.seller_tin, part, &text(value))?;
    form.recompute();
    store(draft, &form)
}

/// 1706 keeps the buyer's nine TIN digits and the branch code apart
/// (`buyer_tin`, `buyer_branch_code`, its own bound box): a whole TIN
/// pasted into the first box fills both.
#[inline(never)]
fn fill_1706_buyer_tin(draft: &mut Value, part: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1706Draft = typed(draft)?;
    let digits = with_tin_part(&form.buyer_tin, part, &text(value))?;
    if digits.len() > 9 {
        form.buyer_branch_code = digits[9..].to_string();
    }
    form.buyer_tin = digits.chars().take(9).collect();
    form.recompute();
    store(draft, &form)
}

/// A typed amount: blank is zero.
fn amount(value: &Value) -> Result<f64, String> {
    let shown = text(value);
    if shown.is_empty() {
        return Ok(0.0);
    }
    bir_core::forms::agent::parse_amount(&shown)
        .filter(|amount| *amount >= 0.0)
        .ok_or_else(|| format!("`{shown}` is not an amount."))
}

/// 2552 item 11: the official `kindOfTransaction` clears the Part V rows
/// when the schedule changes (`Form2552Draft::set_transaction`); unticking
/// a box answers nothing (a radio: another box is ticked instead).
#[inline(never)]
fn fill_2552_transaction(draft: &mut Value, schedule: u8, value: &Value) -> Result<(), String> {
    if !ticked(value) {
        return Ok(());
    }
    let mut form: Form2552Draft = typed(draft)?;
    form.set_transaction(match schedule {
        1 => Form2552Transaction::LocalStockExchange,
        2 => Form2552Transaction::PrimaryOffering,
        _ => Form2552Transaction::SecondaryOffering,
    });
    store(draft, &form)
}

#[inline(never)]
fn fill_2552_part_iv(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let amount = amount(value)?;
    let mut form: Form2552Draft = typed(draft)?;
    if amount == 0.0 && form.exempt_transactions.len() <= row {
        return Ok(());
    }
    if form.exempt_transactions.len() <= row {
        form.exempt_transactions
            .resize(row + 1, Form2552ExemptRow::default());
    }
    form.exempt_transactions[row].amount = amount;
    while form
        .exempt_transactions
        .last()
        .is_some_and(|last| *last == Form2552ExemptRow::default())
    {
        form.exempt_transactions.pop();
    }
    form.recompute();
    store(draft, &form)
}

#[inline(never)]
fn fill_2552_outstanding(draft: &mut Value, value: &Value) -> Result<(), String> {
    let mut form: Form2552Draft = typed(draft)?;
    form.outstanding_shares = text(value);
    form.recompute();
    store(draft, &form)
}

/// A typed date as rows 1-4's binding takes it (agent `reformat_date`,
/// MM/DD/YYYY): blank stays blank, anything else must read as a date.
fn mdy(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some(String::new());
    }
    let parts: Vec<&str> = text
        .trim()
        .split(['/', '-', '.'])
        .filter(|part| !part.is_empty())
        .collect();
    let (year, month, day) = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => (*y, *m, *d),
        [m, d, y] if y.len() == 4 => (*y, *m, *d),
        _ => return None,
    };
    let year: u16 = year.parse().ok()?;
    let month: u8 = month.parse().ok().filter(|m| (1..=12).contains(m))?;
    let day: u8 = day.parse().ok().filter(|d| (1..=31).contains(d))?;
    Some(format!("{month:02}/{day:02}/{year:04}"))
}

/// What a 2552 row 5 box says once the add-more rows hold row 5 onward.
pub(super) const ROW5_FOLDED: &str = "Row 5 reads OTHERS while the add-more rows hold row 5 onward: change it in the add-more table.";

#[inline(never)]
fn fill_2552_row5(draft: &mut Value, key: &str, column: Row5, value: &Value) -> Result<(), String> {
    let mut form: Form2552Draft = typed(draft)?;
    if form.schedule_is_folded() {
        return Err(ROW5_FOLDED.to_string());
    }
    let at = FORM_2552_ROWS - 1;
    let shown = text(value);
    let number = matches!(column, Row5::NumShares | Row5::TaxBase | Row5::TaxRate);
    let filling = if number {
        amount(value)? != 0.0
    } else {
        !shown.is_empty()
    };
    if form.schedule.len() <= at {
        if !filling {
            return Ok(());
        }
        form.schedule.resize(at + 1, Form2552ShareRow::default());
    }
    let row = &mut form.schedule[at];
    match column {
        Row5::Date => {
            row.date = mdy(&shown)
                .ok_or_else(|| format!("`{key}` must be a date as MM/DD/YYYY (got `{shown}`)"))?
        }
        Row5::Seller => row.seller = shown,
        Row5::Buyer => row.buyer = shown,
        Row5::Corp => row.issuing_corporation = shown,
        Row5::NumShares => row.number_of_shares = amount(value)?,
        Row5::TaxBase => row.tax_base = amount(value)?,
        Row5::TaxRate => row.tax_rate = amount(value)?,
    }
    // A cleared last row goes, as the official Delete does.
    let lse = form.transaction == Form2552Transaction::LocalStockExchange;
    while form.schedule.last().is_some_and(|last| {
        last.date.is_empty()
            && last.seller.is_empty()
            && last.buyer.is_empty()
            && last.issuing_corporation.is_empty()
            && last.number_of_shares == 0.0
            && last.tax_base == 0.0
            && (last.tax_rate == 0.0 || lse)
    }) {
        form.schedule.pop();
    }
    form.recompute();
    store(draft, &form)
}

/// 1707 item 8: the ticked box carries the answer; Yes keeps what 8A
/// holds, No clears it (asked first: [`removes`]).
#[inline(never)]
fn fill_1707_tax_relief(draft: &mut Value, yes: bool, value: &Value) -> Result<(), String> {
    if !ticked(value) {
        return Ok(());
    }
    let mut form: Form1707Draft = typed(draft)?;
    form.tax_relief = if yes {
        Some(form.tax_relief.take().unwrap_or_default())
    } else {
        None
    };
    form.recompute();
    store(draft, &form)
}

/// What a 1707 row D box says once the add-more rows hold row D onward.
pub(super) const ROW_D_FOLDED: &str = "Row D reads OTHERS while the add-more rows hold row D onward: change it in the add-more table.";

#[inline(never)]
fn fill_1707_shares(
    draft: &mut Value,
    row: usize,
    column: usize,
    value: &Value,
) -> Result<(), String> {
    let mut form: Form1707Draft = typed(draft)?;
    if row == FORM_1707_SCHEDULE_ROWS - 1 && form.shares.len() > FORM_1707_SCHEDULE_ROWS {
        return Err(ROW_D_FOLDED.to_string());
    }
    let shown = text(value);
    let filling = match column {
        1 | 3 => amount(value)? != 0.0,
        _ => !shown.is_empty(),
    };
    if form.shares.len() <= row {
        if !filling {
            return Ok(());
        }
        form.shares.resize(row + 1, Form1707Shares::default());
    }
    // The official boxes' limits (form_1707.rs validate: corporation names
    // hold 100 characters, certificate numbers up to 40 digits).
    match column {
        0 if shown.chars().count() > 100 => {
            return Err("Corporation names hold at most 100 characters.".to_string());
        }
        // Rows A-C bind it as text (rules/forms/1707-v2021 corpus: string,
        // max_length 40): the same here.
        2 if shown.chars().count() > 40 => {
            return Err("Stock certificate numbers hold at most 40 characters.".to_string());
        }
        _ => {}
    }
    let entry = &mut form.shares[row];
    match column {
        0 => entry.corporation = shown,
        1 => entry.number_of_shares = (!shown.is_empty()).then(|| amount(value)).transpose()?,
        2 => entry.certificate_number = shown,
        _ => entry.selling_price = amount(value)?,
    }
    while form
        .shares
        .last()
        .is_some_and(|last| *last == Form1707Shares::default())
    {
        form.shares.pop();
    }
    form.recompute();
    store(draft, &form)
}

#[inline(never)]
fn fill_1707_expense_d(draft: &mut Value, column: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1707Draft = typed(draft)?;
    if form.expenses.len() > FORM_1707_SCHEDULE_ROWS {
        return Err(ROW_D_FOLDED.to_string());
    }
    let row = FORM_1707_SCHEDULE_ROWS - 1;
    let shown = text(value);
    let filling = if column == 1 {
        amount(value)? != 0.0
    } else {
        !shown.is_empty()
    };
    if form.expenses.len() <= row {
        if !filling {
            return Ok(());
        }
        form.expenses.resize(row + 1, Form1707Expense::default());
    }
    if column == 0 && shown.chars().count() > 100 {
        return Err("Particulars hold at most 100 characters.".to_string());
    }
    if column == 1 {
        form.expenses[row].amount = amount(value)?;
    } else {
        form.expenses[row].particulars = shown;
    }
    while form
        .expenses
        .last()
        .is_some_and(|last| *last == Form1707Expense::default())
    {
        form.expenses.pop();
    }
    form.recompute();
    store(draft, &form)
}

/// What the page shows where the field map's value is not what the draft
/// keeps (display only; the field map, upload and print are unchanged):
/// 1707's profile copy of the address is the whole address, never the
/// upload's cut first line (form_1707.rs address_parts), so an edit there
/// never drops the second line.
pub(super) fn values(
    kind: AgentFormKind,
    draft: &Value,
    map: &mut std::collections::BTreeMap<String, String>,
) {
    // A one-date box whose month and day are both still the official blank
    // `00` (a return not yet dated) shows no date at all, never "00/00/2026".
    for date in ONE_DATES
        .iter()
        .filter(|date| date.form == kind.form_code() && date.hide_zero_parts)
    {
        let [month, day, year] = date.keys;
        // Nor one holding only part of a date (03/00/2026): blank, with a
        // notice saying what is stored (`notices`, `waits`).
        let unset = |key: &str| map.get(key).is_some_and(|v| v.trim() == "00");
        if unset(month) || unset(day) {
            for key in [month, day, year] {
                map.insert(key.to_string(), String::new());
            }
        }
    }
    if kind.form_code() == "1707"
        && let Some(address) = draft["registered_address"].as_str()
        && map.contains_key("frm1707:ProfileAddress1")
    {
        map.insert("frm1707:ProfileAddress1".into(), address.to_uppercase());
    }
}

// 2552's computed cells (add-more tax due, Schedule 1's fixed rate, the
// add-more subtotals) are locked by bir-core
// (`agent::draft_locks::as_needed`, decision P4-19), for the page and the
// assistant alike.

/// Rule 4 for a schedule's date boxes: 2552's and 1707's tables with a date
/// column carry `calendar: true`, so each of their date boxes gets its own
/// calendar on the page (form_page.js `scheduleTable`, PR 4's hunk for
/// 2000, taken unchanged). Other forms' tables are their own groups'.
pub(super) fn mark_calendar_tables(form: &mut Value, code: &str) {
    if !matches!(code, "2552" | "1707") {
        return;
    }
    if code == "1707" {
        order_1707_sellers(form);
    }
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let dated = item["cols"]
            .as_array()
            .is_some_and(|cols| cols.iter().any(|col| col["t"] == "date"));
        if dated {
            item["calendar"] = Value::Bool(true);
        }
    }
}

/// 1707 item 6's extracted table keeps its columns in key order (address,
/// name, TIN): the sheet prints Name | Registered Address | TIN on rows
/// 6A-6C (html-frozen/1707-2021/writer-cells.json "6A Seller name", "6A
/// Seller's Registered Address", "6A Seller's Taxpayer Identification
/// Number").
fn order_1707_sellers(form: &mut Value) {
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        if item["keys"][0][0] != "frm1707:txtPg1I6SellerAddr0" {
            continue;
        }
        let order = [1, 0, 2];
        let cols = item["cols"].clone();
        item["cols"] = Value::Array(order.iter().map(|&c| cols[c].clone()).collect());
        for (col, caption) in item["cols"].as_array_mut().unwrap().iter_mut().zip([
            "Seller's Name",
            "Seller's Registered Address",
            "Seller's Taxpayer Identification Number (TIN)",
        ]) {
            col["l"] = Value::from(caption);
        }
        if let Some(rows) = item["keys"].as_array_mut() {
            for row in rows {
                let old = row.clone();
                *row = Value::Array(order.iter().map(|&c| old[c].clone()).collect());
            }
        }
        item["no"] = Value::from("6");
        item["l"] = Value::from("Seller's Name, Registered Address and TIN (rows 6A-6C)");
        item["row_labels"] = serde_json::json!({"caption": "Row", "rows": ["6A", "6B", "6C"]});
    }
}

/// The error keys of a PR 9 form's message, reordered only where its
/// subject (first) key is drawn under Not on the printed sheet while
/// another of its keys is drawn under a printed item: that key comes first,
/// since the page draws a message under its first key's item
/// (form_page.js `setErrors`). On PR 9's forms that is 1707's "ATC Code on
/// Page 1 Item 3 is required." (bound with item 3's boxes to the page's own
/// domestic/foreign question); every other message keeps its order
/// (tests::as_needed golden). Keys in the part's tables count too.
pub(super) fn order_error_keys(kind: AgentFormKind, keys: &mut [String]) {
    if !matches!(
        kind.form_code(),
        "0605" | "1606" | "1706" | "1707" | "2000OT" | "2552"
    ) {
        return;
    }
    if let Some(form) = super::schema_entry(kind.form_code()) {
        order_keys_in(form, keys);
    }
}

/// `order_error_keys` on one form's schema.
pub(super) fn order_keys_in(form: &Value, keys: &mut [String]) {
    let mut off_sheet = std::collections::BTreeSet::new();
    let mut drawn = std::collections::BTreeSet::new();
    for part in form["parts"].as_array().into_iter().flatten() {
        let off = part["n"] == "Not on the printed sheet";
        for item in part["items"].as_array().into_iter().flatten() {
            let fields = item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|field| field["t"] != "hidden")
                .filter_map(|field| field["k"].as_str());
            let cells = item["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|row| row.as_array().into_iter().flatten())
                .filter_map(Value::as_str);
            for key in fields.chain(cells) {
                if off {
                    off_sheet.insert(key.to_string());
                } else {
                    drawn.insert(key.to_string());
                }
            }
        }
    }
    let Some(first) = keys.first() else {
        return;
    };
    if !off_sheet.contains(first) {
        return;
    }
    if let Some(at) = keys.iter().position(|key| drawn.contains(key)) {
        keys[..=at].rotate_right(1);
    }
}

/// What Schedule 1's add-more rate shows while its row is blank.
pub(super) const LSE_RATE_WAITS: &str = "Schedule 1's rate is 6/10 of 1%.";

/// What each box the form fills from another box waits for while it shows
/// blank (the page shows it in the box).
pub(super) fn waits(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    for date in ONE_DATES
        .iter()
        .filter(|date| date.form == kind.form_code() && date.writer == DateWriter::Identity)
    {
        for key in date.keys {
            out.insert(key.to_string(), Value::from(DATE_FROM_NEW_RETURN));
        }
        if let Some(why) = bir_core::forms::agent::field_map(kind, draft)
            .ok()
            .and_then(|map| partial_date(date.keys, &map))
        {
            for key in date.keys {
                out.insert(key.to_string(), Value::from(why.clone()));
            }
        }
    }
    let fmv_boxes = |prefix: &str| {
        [
            ("A", "FMV of Land per Tax Declaration"),
            ("B", "FMV of Improvements per Tax Declaration"),
            ("C", "FMV of Land per BIR (zonal value)"),
            ("D", "FMV of Improvements per BIR"),
        ]
        .map(|(letter, what)| {
            (
                format!("{prefix}{letter}"),
                format!("Ticked by the form once Schedule 1's {what} holds an amount."),
            )
        })
    };
    match kind.form_code() {
        "2000OT" => {
            // Item 1 is the return's identity (decision 10: matched by its
            // exact date); a return opened without one says what sets it.
            out.insert(
                "frm2000OT:txtTransactionDate".into(),
                Value::from(DATE_FROM_NEW_RETURN),
            );
        }
        // The older layout's FMV-basis boxes follow Schedule 1's amounts
        // (form_1606.rs / form_1706.rs `to_bir_field_map`).
        "1606" => {
            for (key, why) in fmv_boxes("frm1606:opt27") {
                out.insert(key, Value::from(why));
            }
        }
        "1706" => {
            for (key, why) in fmv_boxes("frm1706:opt29") {
                out.insert(key, Value::from(why));
            }
        }
        "2552" => {
            for n in 1..=9 {
                out.insert(
                    format!("frm2552:txtPg2Pt5Sch1_{n}Col7"),
                    Value::from(LSE_RATE_WAITS),
                );
            }
        }
        _ => {}
    }
    out
}

/// What a blank identity date waits for.
pub(super) const DATE_FROM_NEW_RETURN: &str =
    "Set by the date of transaction you start this return with (New return).";

/// Validation messages that name the official program's item numbers,
/// rewritten to the 2018 sheet's printed ones (maintainer decision D6,
/// 2026-10-18: the wording only; the checks and the filed data stay the
/// same). `(form, official message, printed message)`.
pub(super) const PRINTED_MESSAGES: &[(&str, &str, &str)] = &[
    // 1606: the official program numbers the older layout's items.
    (
        "1606",
        "Item 3 holds at most two digits.",
        "Item 4 holds at most two digits.",
    ),
    (
        "1606",
        "Please select an option for Item 4.",
        "Please select an option for Item 3.",
    ),
    (
        "1606",
        "Item 9 holds at most 50 characters.",
        "Item 7A Buyer's Name holds at most 50 characters.",
    ),
    (
        "1606",
        "Item 10 holds at most 50 characters.",
        "Item 8A Seller's Name holds at most 50 characters.",
    ),
    (
        "1606",
        "Item 11 holds at most 70 characters.",
        "Item 7A Buyer's Registered Address holds at most 70 characters.",
    ),
    (
        "1606",
        "Item 12 holds at most 70 characters.",
        "Item 8A Seller's Registered Address holds at most 70 characters.",
    ),
    (
        "1606",
        "Please select an option for Item 13.",
        "Please select an option for Item 9.",
    ),
    (
        "1606",
        "Please select an option for Item 14.",
        "Please select an option for Item 6.",
    ),
    (
        "1606",
        "Please select an option for Item 15.",
        "Please select an option for the Part IV Schedule 1 Classification.",
    ),
    (
        "1606",
        "Specify the classification of the property on Item 15.",
        "Specify the classification of the property in Part IV Schedule 1.",
    ),
    (
        "1606",
        "Item 15 \"Others\" is specified only when Others is marked.",
        "Part IV Schedule 1 \"Others\" is specified only when Others is marked.",
    ),
    (
        "1606",
        "Item 15 \"Others\" holds at most 60 characters.",
        "Part IV Schedule 1 \"Others\" holds at most 60 characters.",
    ),
    (
        "1606",
        "Please enter the Location of the Property on Item 16.",
        "Please enter the Location of the Property in Part IV Schedule 1.",
    ),
    (
        "1606",
        "Item 16 holds at most 125 characters.",
        "Part IV Schedule 1 Location holds at most 125 characters.",
    ),
    (
        "1606",
        "Please enter the RDO Code on Item 16A.",
        "Please enter the RDO Code on Item 5.",
    ),
    (
        "1606",
        "Item 17 TCT/OCT/CCT No. holds at most 20 letters, digits or dashes.",
        "Part IV Schedule 1 OCT/TCT/CCT No. holds at most 20 letters, digits or dashes.",
    ),
    (
        "1606",
        "Item 17 Area Sold holds at most 20 digits.",
        "Part IV Schedule 1 Area holds at most 20 digits.",
    ),
    (
        "1606",
        "Item 17 Tax Dec. No. holds at most 25 digits.",
        "Part IV Schedule 1 Tax Declaration No. holds at most 25 digits.",
    ),
    (
        "1606",
        "Item 17 Others holds at most 25 characters.",
        "The Others description (the older layout's item 17; the 2018 sheet prints no box for it, see Not on the printed sheet) holds at most 25 characters.",
    ),
    (
        "1606",
        "Please select a tax relief for Item 19.",
        "Please select a tax relief for Item 11.",
    ),
    (
        "1606",
        "Please select an option for Item 20.",
        "Please select an option for Part IV Schedule 2 Item 1.",
    ),
    (
        "1606",
        "Specify the Exempt or Others transaction on Item 20.",
        "Specify the Exempt or Others transaction on Part IV Schedule 2 Item 1.",
    ),
    (
        "1606",
        "Item 20 is specified only for an Exempt or Others transaction.",
        "Part IV Schedule 2 Item 1 is specified only for an Exempt or Others transaction.",
    ),
    (
        "1606",
        "Item 20 specification holds at most 60 characters.",
        "Part IV Schedule 2 Item 1 specification holds at most 60 characters.",
    ),
    (
        "1606",
        "Items 21 to 26 apply only to an installment sale.",
        "Part IV Schedule 2 Items 2 to 7 apply only to an installment sale.",
    ),
    (
        "1606",
        "Items 28C, 28D and 28E are disabled for a cash sale.",
        "Part IV Schedule 3's Bid Price (Item 1A), Items 1C and 1D are disabled for a cash sale.",
    ),
    (
        "1606",
        "Enter all four Item 27 values (0 where there is none) so Item 28B is computed.",
        "Enter all four Part IV Schedule 1 FMV values (0 where there is none) so Schedule 3 Item 1B is computed.",
    ),
    (
        "1606",
        "Item 28E description holds at most 40 characters.",
        "Part IV Schedule 3 Item 1D description holds at most 40 characters.",
    ),
    (
        "1606",
        "Please select the tax rate on Item 31.",
        "Please select the tax rate on Item 13.",
    ),
    (
        "1606",
        "Items 31 and 35 are disabled when Item 4 is No.",
        "Items 13 and 17A to 17C are disabled when Item 3 is No.",
    ),
    (
        "1606",
        "Item 33 applies only to an amended return.",
        "Item 15 applies only to an amended return.",
    ),
    (
        "1606",
        "Please indicate refund type for overremittance in Item 37.",
        "Please indicate refund type for overremittance in Item 18.",
    ),
    // 1706: likewise.
    (
        "1706",
        "Item 3 holds at most two digits.",
        "Item 4 holds at most two digits.",
    ),
    (
        "1706",
        "Please select an option for Item 4.",
        "Please select an option for Item 3.",
    ),
    (
        "1706",
        "Please enter a valid TIN number on Item 5.",
        "Please enter a valid TIN number on Item 6A (the seller's TIN).",
    ),
    (
        "1706",
        "Please enter a valid Seller's RDO Code on Item 6.",
        "Please enter a valid Seller's RDO Code (under Not on the printed sheet).",
    ),
    (
        "1706",
        "Please enter the RDO Code on Item 14A.",
        "Please enter the RDO Code on Item 5.",
    ),
    (
        "1706",
        "Please select an option for Item 15.",
        "Please select an option for the Part IV Schedule 1 Classification.",
    ),
    (
        "1706",
        "Please specify the classification of the property on Item 15.",
        "Please specify the classification of the property in Part IV Schedule 1.",
    ),
    (
        "1706",
        "Item 16 Others holds at most 25 characters.",
        "The Others description (the older layout's item 16; the 2018 sheet prints no box for it, see Not on the printed sheet) holds at most 25 characters.",
    ),
    (
        "1706",
        "Please select an option for Item 17.",
        "Please select an option for Item 8.",
    ),
    (
        "1706",
        "Please select an option for Item 18.",
        "Please select an option for Item 9.",
    ),
    (
        "1706",
        "Please select an option for Item 19.",
        "Please select an option for Item 10.",
    ),
    (
        "1706",
        "Please specify tax relief you are availing for Item 20.",
        "Please specify tax relief you are availing for Item 11.",
    ),
    (
        "1706",
        "Please select an option for Item 21.",
        "Please select an option for Part IV Schedule 2.",
    ),
    (
        "1706",
        "Item 21 description holds at most 60 characters.",
        "Part IV Schedule 2 description holds at most 60 characters.",
    ),
    (
        "1706",
        "Item 28 takes the installment date as MM/DD/YYYY.",
        "Part IV Schedule 2 Item 7 takes the installment date as MM/DD/YYYY.",
    ),
    (
        "1706",
        "Please choose Determination of taxable base in Item 30",
        "Please choose Determination of taxable base in Part IV Schedule 4",
    ),
    (
        "1706",
        "Item 30E applies only when Items 17 and 18 are both Yes.",
        "Part IV Schedule 4 Item 1C applies only when Items 8 and 9 are both Yes.",
    ),
    (
        "1706",
        "Item 30F description holds at most 40 characters.",
        "Part IV Schedule 4 Item 1E description holds at most 40 characters.",
    ),
    (
        "1706",
        "Item 31 cannot be determined yet. Complete Items 29 and 30 for this transaction.",
        "Item 12 cannot be determined yet. Complete Part IV Schedules 1 and 4 for this transaction.",
    ),
    (
        "1706",
        "Item 33 applies only to an amended return.",
        "Item 15 applies only to an amended return.",
    ),
];

/// The page's wording of `message`: the printed item numbers where the
/// official program names its own (D6), else `None`.
pub(super) fn printed_message(kind: AgentFormKind, message: &str) -> Option<&'static str> {
    let code = kind.form_code();
    PRINTED_MESSAGES
        .iter()
        .find(|(form, official, _)| *form == code && *official == message)
        .map(|(_, _, printed)| *printed)
}

/// 1606 and 1706 item 11 No clears the relief chosen from its list.
fn removes_relief(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let code = kind.form_code();
    // Item 11 No, or the list's blank entry (1606 "", 1706 "0").
    if !edits.keys().any(|key| {
        key.starts_with(&format!("frm{code}:rdTreaty")) || *key == format!("frm{code}:selTreaty")
    }) {
        return None;
    }
    let held = match before["tax_relief"].as_str()? {
        "international_tax_treaty" => "International Tax Treaty",
        "special_law" => "Special Law",
        _ => return None,
    };
    (after["tax_relief"] == "no").then(|| {
        serde_json::json!({
            "message": format!("This clears the relief chosen at item 11 ({held}). Continue?"),
            "done": "Cleared item 11's relief",
            "rows": 1,
        })
    })
}

/// 1707 item 8 No clears what 8A held.
fn removes_1707(before: &Value, after: &Value, edits: &Map<String, Value>) -> Option<Value> {
    if !edits.contains_key("frm1707:rdoPg1I8TaxReliefNo") {
        return None;
    }
    let held = before["tax_relief"]
        .as_str()
        .filter(|t| !t.trim().is_empty())?;
    after["tax_relief"].is_null().then(|| {
        serde_json::json!({
            "message": format!("Answering No at item 8 clears item 8A (\"{held}\"). Continue?"),
            "done": "Cleared item 8A",
            "rows": 1,
        })
    })
}

/// When `edits` turned `before` into `after` by removing what the filer
/// typed, what the page asks first (`message`), what the Undo toast says
/// (`done`) and how many rows went; `None` when nothing typed went.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    if kind.form_code() == "1707" {
        return removes_1707(before, after, edits);
    }
    if matches!(kind.form_code(), "1606" | "1706") {
        return removes_relief(kind, before, after, edits);
    }
    if kind.form_code() != "2552"
        || !edits
            .keys()
            .any(|key| key.starts_with("frm2552:rdoPg1I11Transaction"))
    {
        return None;
    }
    // Item 11 picks another schedule: `set_transaction` clears every row
    // of the old one, so each row typed before goes, whatever the batch
    // types under the new schedule.
    if before["transaction"] == after["transaction"] {
        return None;
    }
    // A rate the filer types (Schedules 2 and 3) counts; Schedule 1's fixed
    // rate and every row's tax due are the form's.
    let typed_rate = before["transaction"] != "local_stock_exchange";
    let rows = before["schedule"].as_array().map_or(0, |rows| {
        rows.iter()
            .filter(|row| {
                row.as_object().is_some_and(|fields| {
                    fields.iter().any(|(field, value)| match value {
                        Value::String(text) => !text.trim().is_empty(),
                        Value::Number(n) => {
                            field != "tax_due"
                                && (field != "tax_rate" || typed_rate)
                                && n.as_f64().is_some_and(|n| n != 0.0)
                        }
                        _ => false,
                    })
                })
            })
            .count()
    });
    if rows == 0 {
        return None;
    }
    let schedule = match before["transaction"].as_str() {
        Some("local_stock_exchange") => "Schedule 1",
        Some("primary_offering") => "Schedule 2",
        _ => "Schedule 3",
    };
    let plural = if rows == 1 { "" } else { "s" };
    Some(serde_json::json!({
        "message": format!(
            "Changing item 11 removes the {rows} Part V {schedule} row{plural} you typed. Continue?"
        ),
        "done": format!("Removed {rows} {schedule} row{plural}"),
        "rows": rows,
    }))
}
