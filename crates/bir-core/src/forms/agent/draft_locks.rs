//! The draft-dependent box locks (decision P4-19, part 2).
//!
//! [`super::locks`] is the base every writer shares: computed, identity,
//! profile, occurrence-alias and unmapped boxes, read from the pinned
//! bindings. Over it the form page's open (bir-shell `opened_json`) lays the
//! locks that follow the draft as it stands. They live here, so the page and
//! the desktop assistant read one implementation: the page draws them and
//! [`super::fill`] refuses them.
//!
//! [`draft_locks`] applies nine layers, in this order, each named after the
//! page's row module it came from (the last after the rule it carries):
//!
//! 1. [`dst_pt_vat`] (PR 4): 2000's 15B and 15C, computed; 2553's
//!    classification boxes 14A-18A, its printed rows' ATC and rate, and its
//!    quarter boxes (the return's own period).
//! 2. [`annual_corporate`] (PR 8): the derived boxes 1702RT, 1707A and
//!    1702MX compute, said to be computed; 1702RT's folded (Add more...)
//!    items and a shut item 42, 40 or 57 ([`annual_corporate::live_locks`]);
//!    1702EX's rows BIR's program keeps shut.
//! 3. [`as_needed`] (PR 9): 2552's computed cells.
//! 4. [`penalties`]: every box of a computed penalty group the filer has not
//!    edited, with why; a column still on the amounts it was stored with
//!    says so instead ([`income_q`], [`annual_individual`], [`excise`]).
//! 5. [`income_q`] (PR 5): 1701Q's spouse penalties, while column B is not
//!    on the return, are taken off again: the page fades them with what
//!    opens them, so no penalty lock stands beside that.
//! 6. [`item_13`]: 2551Q's Item 13 on a later quarter whose filed Q1 this
//!    app holds, locked to that Q1's answer.
//! 7. [`item16_2551q`]: 2551Q Item 16 is shut unless Amended Return is Yes
//!    without the 8% option.
//! 8. [`installment_1606`]: 1606's installment-sale boxes, shut unless
//!    Item 20 is an installment sale.
//! 9. [`previous_payment`]: the previous-payment box of an unamended
//!    return, shut with its item's amended-return reason.
//!
//! [`live_locks`] are the locks a page fill reports back (`relocks`), so a
//! box shuts or opens as soon as the answer that decides it is typed.
//!
//! The page's own UX around these stays in bir-shell: the penalties Edit
//! (`unlocked`), the notes under a locked box, the notices and the waits.
//! The page writers ([`super::fill_as_user`],
//! [`super::fill_profile_box_as_user`], [`super::write_period_box`],
//! [`super::fill_host_owned`]) never consult this layer: the page governs
//! those boxes itself (Edit penalties opens a group). The assistant has no
//! Edit penalties, so an unedited computed penalty group stays locked to it,
//! as only a typed penalty overrides the computed one.

use serde_json::{Map, Value};

use super::AgentFormKind;

/// Lay the draft-dependent locks over `locks`, the base map
/// ([`super::locks`]), the way the page's open does: the nine layers of this
/// module's note, in that order, with their own reasons. A layer may replace
/// a base reason with an exact one (2000's 15B, which the probe calls
/// derived, is computed), and the last one takes 1701Q's spouse penalty
/// locks off again.
pub fn draft_locks(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
    layers(kind, draft, locks, None);
}

/// [`draft_locks`]' verdict on `key` alone: the reason the page locks `key`
/// with when the base map gives it `base` (its entry there, if any), or
/// `None` where the page shows it open. Every layer decides a key from that
/// key's own entry and the draft, never from another key's, so this is the
/// entry [`draft_locks`] leaves for `key` over the whole base map; only the
/// penalty group `key` belongs to is evaluated (each group's check runs the
/// form's compute).
pub fn draft_lock(
    kind: AgentFormKind,
    draft: &Value,
    key: &str,
    base: Option<&str>,
) -> Option<String> {
    let mut locks = Map::new();
    if let Some(reason) = base {
        locks.insert(key.to_string(), Value::from(reason));
    }
    layers(kind, draft, &mut locks, Some(key));
    locks.get(key).and_then(Value::as_str).map(str::to_string)
}

/// The nine layers; `only` limits the penalty layer to that key's group.
fn layers(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>, only: Option<&str>) {
    dst_pt_vat::relock(kind, draft, locks);
    annual_corporate::relock(kind, draft, locks);
    // PR 9's forms' computed boxes the probe called derived.
    as_needed::relock(kind, locks);
    // A computed penalty is locked until Edit penalties opens its group; a
    // return with no due date computes none, and its boxes stay open.
    penalties::relock(kind, draft, locks, only);
    // 1701Q's spouse penalties while column B is not on the return: faded
    // with what opens them, not locked (PR 5).
    income_q::relock(kind, draft, locks);
    // 2551Q's Item 13 on a later quarter whose filed Q1 is in this app.
    item_13::relock(kind, draft, locks);
    // 2551Q Item 16 follows Amended Return and the 8% option.
    item16_2551q::relock(kind, draft, locks);
    // 1606's installment-sale boxes follow Item 20.
    installment_1606::relock(kind, draft, locks);
    // Previous payment applies only to an amended return.
    previous_payment::relock(kind, draft, locks);
    schedule1_1601fq::relock(kind, draft, locks);
}

/// The locks a fill leaves, each key with its reason or `null` (open), for
/// the page to apply at once (`screen_form_fill`'s `relocks`): 1702RT's
/// folded (Add more...) items and items 42, 40 and 57 (PR 8), an excise,
/// 1800 or 1801 return's penalty groups (PR 11), and the amended-return
/// amount gates whose answers can change in the fill. Empty for every other
/// form.
pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let mut relocks = annual_corporate::live_locks(kind, draft);
    relocks.extend(excise::live_locks(kind, draft));
    relocks.extend(item16_2551q::live_locks(kind, draft));
    relocks.extend(installment_1606::live_locks(kind, draft));
    relocks.extend(previous_payment::live_locks(kind, draft));
    relocks.extend(schedule1_1601fq::live_locks(kind, draft));
    relocks
}

fn typed<T: serde::de::DeserializeOwned>(draft: &Value) -> Result<T, String> {
    serde_json::from_value(draft.clone()).map_err(|e| e.to_string())
}

pub mod dst_pt_vat {
    //! PR 4's 2000 and 2553 locks. bir-shell `rows_dst_pt_vat` keeps the
    //! cells' writes, notices and waits, and reads the printed rows here.
    //!
    //! 2553 rows 14 to 17: the July 1999 sheet preprints each row's
    //! classification, ATC and rate (html-frozen/2553-1999/index.html), and
    //! the print fills only their amounts, so a new entry on rows 14-17 is
    //! that row's printed entry and the row's ATC and preprinted rate are
    //! shut. A stored row that holds another entry (the official popup lets
    //! any row hold any entry) keeps its ATC and rate open, so the filer can
    //! clear it or set the printed one. Every classification box follows the
    //! ATC chosen in its row. 2000's 15B and 15C are totals of Schedules 2
    //! and 3 (`Form2000Draft::recompute`).

    use serde_json::{Map, Value, json};

    use crate::forms::agent::{AgentFormKind, Lock};
    use crate::forms::form_2553::{
        FORM_2553_ATC_OPTIONS, Form2553AtcOption, Form2553AtcRow, Form2553Draft,
    };

    /// The 2553 rows the sheet preprints, each with its printed entry of the
    /// official popup (`FORM_2553_ATC_OPTIONS` index) and its printed caption.
    pub const PT_PRINTED_ROWS: [(usize, &str, &str, &str); 4] = [
        (
            0,
            "Philippine Amusement and Gaming Corp. (Pagcor)",
            "OT010",
            "5%",
        ),
        (2, "Clark Development Corporations", "OT011", "5%"),
        (
            3,
            "Special/Regular Economic/Free Port Zone Enterprises",
            "OT011",
            "5%",
        ),
        (1, "Others", "OT012", ""),
    ];
    /// Rows 14-18.
    pub const PT_ROWS: usize = 5;
    /// 2553's first ATC item.
    pub const PT_FIRST_ITEM: usize = 14;

    /// The lock reason of a 2553 row 14-17 ATC (B) or 5% rate (D): the sheet
    /// preprints them next to the row's amounts.
    pub const PRINTED_LOCK: &str =
        "printed on the form: a new entry on this row is the row's printed entry";

    /// The lock reason of a 2553 classification box (column A): the official
    /// popup writes it with the row's ATC (`getATCCode`), so it follows the
    /// ATC chosen in the row's B box.
    pub const CLASSIFICATION_LOCK: &str = "follows the ATC chosen in this row's ATC box (column B)";

    fn pt_option(index: usize) -> &'static Form2553AtcOption {
        &FORM_2553_ATC_OPTIONS[index]
    }

    /// The printed option of a 2553 row 14-17.
    pub fn printed_option(row: usize) -> Option<&'static Form2553AtcOption> {
        PT_PRINTED_ROWS
            .get(row)
            .map(|(index, _, _, _)| pt_option(*index))
    }

    /// Whether a stored 2553 row holds the popup entry `option`.
    pub fn same_entry(row: &Form2553AtcRow, option: &Form2553AtcOption) -> bool {
        row.atc_code == option.code && row.description == option.description
    }

    /// A stored row 14-17's entry, when it is another than the one the sheet
    /// prints there: its ATC and rate stay open.
    pub fn misplaced_entry(form: &Form2553Draft, row: usize) -> Option<&Form2553AtcRow> {
        let printed = printed_option(row)?;
        let entry = form.schedule.get(row).filter(|entry| !entry.is_empty())?;
        (!same_entry(entry, printed)).then_some(entry)
    }

    /// The boxes this layer shuts, and why, over the probed locks: 2553's
    /// classification boxes (14A-18A), its printed rows' ATC and rate, its
    /// quarter boxes (the return's own period), and 2000's 15B and 15C,
    /// which `recompute` totals from Schedules 2 and 3.
    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        match kind.form_code() {
            "2000" => {
                for key in ["frm2000:txtTax15B", "frm2000:txtTax15C"] {
                    locks.insert(key.into(), json!("computed by BIR's own formulas"));
                }
            }
            "2553" => {
                for item in PT_FIRST_ITEM..PT_FIRST_ITEM + PT_ROWS {
                    locks.insert(format!("frm2553:txt{item}A"), json!(CLASSIFICATION_LOCK));
                }
                // Rows 14-17: a new entry's ATC and preprinted rate are the
                // sheet's; a stored row holding another entry stays open so
                // the filer can clear it (bir-shell's notices say why).
                let form = super::typed::<Form2553Draft>(draft).ok();
                for (row, printed) in
                    (0..PT_PRINTED_ROWS.len()).filter_map(|r| Some((r, printed_option(r)?)))
                {
                    if form
                        .as_ref()
                        .is_some_and(|form| misplaced_entry(form, row).is_some())
                    {
                        continue;
                    }
                    let item = PT_FIRST_ITEM + row;
                    locks.insert(format!("frm2553:txt{item}B"), json!(PRINTED_LOCK));
                    if !printed.rate_is_editable {
                        locks.insert(format!("frm2553:txt{item}D"), json!(PRINTED_LOCK));
                    }
                }
                for quarter in 1..=4 {
                    locks.insert(
                        format!("frm2553:optQtr:_{quarter}"),
                        json!(Lock::Identity.reason()),
                    );
                }
            }
            _ => {}
        }
    }
}

pub mod annual_corporate {
    //! PR 8's locks on the annual corporate income tax returns (1702RT,
    //! 1702MX, 1702EX) and 1707A. bir-shell `rows_annual_corporate` keeps
    //! the boxes the host writes, the notes, notices and waits, and reads
    //! the keys and reasons here.
    //!
    //! - 1702RT item 42 MCIT Due takes an amount only while MCIT applies
    //!   (the form's own `subject_to_mcit`) and, from 2020, item 39 is above
    //!   zero; before 2020 the form computes it, 2% of item 33 (maintainer
    //!   decision: pre-2020 MCIT is 2% of gross), and items 40 and 57 with
    //!   it (maintainer decision: pre-2020 1702RT computed like BIR). Its
    //!   (Add more...) items read OTHERS and the subtotal once their table
    //!   holds two or more rows: they are then the form's.
    //! - Computed boxes the probe left as "derived" say they are computed.
    //! - 1702EX's "others" rows the official page keeps shut at load
    //!   (maintainer decision: 1702EX shut rows locked like 1702MX).

    use serde_json::{Map, Value, json};

    use crate::forms::agent::AgentFormKind;
    use crate::forms::form_1702rt::{
        FORM_1702RT_MODALS, Form1702RTDraft, Form1702RTModal, Form1702RTMoreRows,
    };
    use crate::forms::form_1702rt_official::MCIT_ENTERED_FROM;

    /// The official key prefixes of 1702RT, 1702MX, 1702EX and 1707A.
    pub const RT: &str = "frm1702RT:";
    pub const MX: &str = "frm1702MX:";
    pub const EX: &str = "frm1702EX:";
    pub const A: &str = "frm1707Av2021:";

    /// The lock of a box the form computes (the page's note: "Computed by
    /// the form.").
    const COMPUTED: &str = "computed by BIR's own formulas";

    /// 1702RT item 42 MCIT Due.
    pub const RT_MCIT_DUE: &str = "frm1702RT:txtPg2Pt4I42MinimumCorporate";

    /// Item 42's lock while MCIT does not apply.
    pub const RT_MCIT_REFUSAL: &str = "Item 42 applies once MCIT does: from the fourth taxable year after the year of incorporation (item 10).";

    /// Item 42's lock on a return for a year before 2020 while MCIT applies
    /// (calculations.json 1702rt-item42-mcit: "Before 2020 and when IC 055 is
    /// selected, MCIT is 2% of gross"): form_1702rt_official.rs recompute
    /// computes it (maintainer decision (pre-2020 MCIT is 2% of gross)).
    pub const RT_MCIT_BEFORE_2020: &str =
        "For years before 2020, MCIT is 2% of gross income (item 33), which the form computes.";

    /// 1702RT item 40 (the income tax rate) and Part V item 57.
    pub const RT_RATE: &str = "frm1702RT:Pg2Pt4I40IncomeTaxRate";
    pub const RT_ITEM_57: &str = "frm1702RT:txtPg2Pt5I57SpecialAllowable";

    /// Item 40's lock on a return for a year before 2020 (maintainer decision
    /// (pre-2020 1702RT computed like BIR),
    /// calculations.json 1702rt-item41-rate-tax: "before 2020 it uses 30%").
    pub const RT_RATE_BEFORE_2020: &str =
        "For years before 2020, the income tax rate is 30%, which the form uses.";

    /// Item 57's lock on a return for a year before 2020 (maintainer decision
    /// (pre-2020 1702RT computed like BIR),
    /// calculations.json 1702rt-item57-special-tax: "Only before 2020, Item 57
    /// = Item 35 times income-tax rate / 100").
    pub const RT_57_BEFORE_2020: &str =
        "For years before 2020, item 57 is item 35 times the 30% rate, which the form computes.";

    /// Item 42's lock from 2020 while net taxable income is zero or less
    /// (calculations.json 1702rt-item42-mcit: "2020+ leaves the enabled value
    /// user-controlled except nonpositive net forces zero").
    pub const RT_MCIT_NO_NET: &str =
        "From 2020, MCIT is zero while net taxable income (item 39) is zero or less.";

    /// The lock of a 1702RT (Add more...) item once its table folds.
    pub const RT_FOLDED_LOCK: &str =
        "computed from its (Add more...) rows: OTHERS and their subtotal";

    /// The boxes of a 1702RT (Add more...) item: its description, Schedule
    /// II's legal basis, and its amount.
    pub fn rt_folded_columns(modal: Form1702RTModal) -> Vec<String> {
        let columns: &[&str] = if modal.has_legal_basis() {
            &["C1", "C2", "C3"]
        } else {
            &["C1", "C2"]
        };
        columns
            .iter()
            .map(|column| format!("{RT}{}{column}", modal.row_name()))
            .collect()
    }

    /// The (Add more...) item a 1702RT key is a box of.
    pub fn rt_folded_item(key: &str) -> Option<Form1702RTModal> {
        FORM_1702RT_MODALS
            .into_iter()
            .find(|modal| rt_folded_columns(*modal).iter().any(|k| k == key))
    }

    /// A 1702RT draft's (Add more...) rows.
    pub fn rt_more_rows(draft: &Value) -> Form1702RTMoreRows {
        serde_json::from_value(draft["more_rows"].clone()).unwrap_or_default()
    }

    /// Maintainer decision (1702EX shut rows locked like 1702MX) (2026-10-19):
    /// the 1702EX "others" rows the official page keeps shut at load
    /// (rules/forms/1702ex-v2018c/fixtures/
    /// runtime-control-inventory-v796.json records each `disabled: true`):
    /// item 49 Other Tax Credits/Payments, Schedule 1 items 17e-17h and
    /// Schedule 2 rows 2-3, each with its description (and legal basis).
    /// They are locked the way 1702MX's others rows are, their amounts
    /// computed as fields.json marks them. A draft stored with values there
    /// still files them as stored (form_1702ex.rs is unchanged).
    pub const EX_ATTACHED_ROWS: [(&str, &[&str]); 7] = [
        (
            "Item 49",
            &[
                "txtPg2Pt4I49OtherTaxCrPmtDesc",
                "txtPg2Pt4I49OtherTaxCrPmtAmt",
            ],
        ),
        (
            "Schedule 1 item 17e",
            &["txtPg3Pt6I17OthersEDesc", "txtPg3Pt6I17OthersEAmt"],
        ),
        (
            "Schedule 1 item 17f",
            &["txtPg3Pt6I17OthersFDesc", "txtPg3Pt6I17OthersFAmt"],
        ),
        (
            "Schedule 1 item 17g",
            &["txtPg3Pt6I17OthersGDesc", "txtPg3Pt6I17OthersGAmt"],
        ),
        (
            "Schedule 1 item 17h",
            &["txtPg3Pt6I17OthersHDesc", "txtPg3Pt6I17OthersHAmt"],
        ),
        (
            "Schedule 2 row 2",
            &[
                "txtPg3Pt6S2I2Description",
                "txtPg3Pt6S2I2LegalBasis",
                "txtPg3Pt6S2I2Amount",
            ],
        ),
        (
            "Schedule 2 row 3",
            &[
                "txtPg3Pt6S2I3Description",
                "txtPg3Pt6S2I3LegalBasis",
                "txtPg3Pt6S2I3Amount",
            ],
        ),
    ];

    /// The lock and refusal of an [`EX_ATTACHED_ROWS`] box, naming the
    /// schedule to fill on the form page.
    pub const EX_ATTACHED_LOCK_ITEM_49: &str =
        "filled from the Part IV Item 49 attached schedule; BIR's program keeps this row shut";
    pub const EX_ATTACHED_LOCK_SCHED_1: &str = "filled from the Schedule 1 Item 17i (more...) attached schedule; BIR's program keeps this row shut";
    pub const EX_ATTACHED_LOCK_SCHED_2: &str = "filled from the Schedule 2 Item 4 (more...) attached schedule; BIR's program keeps this row shut";

    /// The `(more...)` summary rows of Schedule 1 item 17i and Schedule 2
    /// item 4 (`rules/forms/1702ex-v2018c/fixtures/runtime-control-inventory-v796.json`:
    /// `txtPg3Pt6I17OthersIDesc`/`Amt`, `txtPg3Pt6S2I4Description`/`LegalBasis`/`Amount`).
    /// BIR's program keeps all five shut as popup/attached-schedule-owned
    /// controls; locking them here ensures both the form page and `form.fill`
    /// refuse them by naming the attached schedule (including `LegalBasis`,
    /// which is not computed, and overriding the generic `COMPUTED` lock on
    /// the folded description/amount boxes).
    const EX_ATTACHED_MORE_ROWS: [(&str, &[&str]); 2] = [
        (
            "Schedule 1 item 17i",
            &["txtPg3Pt6I17OthersIDesc", "txtPg3Pt6I17OthersIAmt"],
        ),
        (
            "Schedule 2 item 4",
            &[
                "txtPg3Pt6S2I4Description",
                "txtPg3Pt6S2I4LegalBasis",
                "txtPg3Pt6S2I4Amount",
            ],
        ),
    ];

    /// The schedule-specific lock/refusal for an [`EX_ATTACHED_ROWS`] or
    /// [`EX_ATTACHED_MORE_ROWS`] box, by official key (`frm1702EX:...`).
    pub fn ex_attached_lock(key: &str) -> Option<&'static str> {
        let stem = key.strip_prefix(EX)?;
        EX_ATTACHED_ROWS
            .iter()
            .chain(EX_ATTACHED_MORE_ROWS.iter())
            .find_map(|(label, stems)| {
                stems.contains(&stem).then(|| {
                    if *label == "Item 49" {
                        EX_ATTACHED_LOCK_ITEM_49
                    } else if label.starts_with("Schedule 1") {
                        EX_ATTACHED_LOCK_SCHED_1
                    } else {
                        EX_ATTACHED_LOCK_SCHED_2
                    }
                })
            })
    }

    /// Every [`EX_ATTACHED_ROWS`] box, by official key.
    pub fn ex_attached_keys() -> impl Iterator<Item = String> {
        EX_ATTACHED_ROWS
            .iter()
            .flat_map(|(_, stems)| stems.iter())
            .map(|stem| format!("{EX}{stem}"))
    }

    fn ex_attached_more_keys() -> impl Iterator<Item = String> {
        EX_ATTACHED_MORE_ROWS
            .iter()
            .flat_map(|(_, stems)| stems.iter())
            .map(|stem| format!("{EX}{stem}"))
    }

    /// 1702MX's shut attached-schedule rows (maintainer decision P2-3:
    /// `rules/forms/1702mx-v2018c/fixtures/runtime-control-inventory-v796.json`
    /// L3377–3592 for Schedule 5 items 17d–17i, L3725–3755 for Schedule 6
    /// rows 2–4; `calculation-function-inventory-v796.json` `computeToMainScD`,
    /// `computeToMainScE`). Each is locked with a message naming the schedule.
    pub const MX_ATTACHED_LOCK_SCHED_5: &str = "filled from the Schedule 5 Item 17d-17i attached schedule; BIR's program keeps this row shut";
    pub const MX_ATTACHED_LOCK_SCHED_6: &str = "filled from the Schedule 6 (rows 2-4 / more...) attached schedule; BIR's program keeps this row shut";

    const MX_ATTACHED_SCHED_5_LETTERS: [char; 6] = ['d', 'e', 'f', 'g', 'h', 'i'];
    const MX_ATTACHED_SCHED_6_STEMS: [&str; 9] = [
        "txtPg6Sc6I2CA",
        "txtPg3Sc6I2CB",
        "txtPg6Sc6I2CC",
        "txtPg6Sc6I3CA",
        "txtPg4Sc6I3CB",
        "txtPg6Sc6I3CC",
        "txtPg3Sc6I4CA",
        "txtPg3Sc6I4CB",
        "txtPg3Sc6I4CC",
    ];

    /// The schedule-specific lock/refusal for a 1702MX shut attached-schedule
    /// box, by official key (`frm1702MX:...`).
    pub fn mx_attached_lock(key: &str) -> Option<&'static str> {
        let stem = key.strip_prefix(MX)?;
        if let Some(rest) = stem.strip_prefix("txtPg3Sc5Itm17") {
            let mut chars = rest.chars();
            if let (Some(letter), Some(col), None) = (chars.next(), chars.next(), chars.next())
                && MX_ATTACHED_SCHED_5_LETTERS.contains(&letter)
                && matches!(col, 'A' | 'B' | 'C')
            {
                return Some(MX_ATTACHED_LOCK_SCHED_5);
            }
        }
        if MX_ATTACHED_SCHED_6_STEMS.contains(&stem) {
            return Some(MX_ATTACHED_LOCK_SCHED_6);
        }
        None
    }

    /// Every 1702MX shut attached-schedule box, by official key.
    pub fn mx_attached_keys() -> impl Iterator<Item = String> {
        MX_ATTACHED_SCHED_5_LETTERS
            .into_iter()
            .flat_map(|letter| {
                ['A', 'B', 'C']
                    .into_iter()
                    .map(move |col| format!("{MX}txtPg3Sc5Itm17{letter}{col}"))
            })
            .chain(
                MX_ATTACHED_SCHED_6_STEMS
                    .into_iter()
                    .map(|stem| format!("{MX}{stem}")),
            )
    }

    /// Why 1702RT item 42 is shut, if it is: the official computeP2Pt4I42
    /// (calculations.json 1702rt-item42-mcit) leaves it to the filer only
    /// from 2020, while MCIT applies (item 10's four years) and item 39 is
    /// above zero. Before 2020 the form computes it while MCIT applies
    /// (maintainer decision (pre-2020 MCIT is 2% of gross)), and clears it
    /// otherwise, as from 2020.
    pub fn rt_item_42_lock(draft: &Value) -> Option<&'static str> {
        let typed: Form1702RTDraft = serde_json::from_value(draft.clone()).ok()?;
        if !typed.subject_to_mcit() {
            Some(RT_MCIT_REFUSAL)
        } else if typed.taxable_year < MCIT_ENTERED_FROM {
            Some(RT_MCIT_BEFORE_2020)
        } else if typed.part_iv.item_39_net_taxable_income_or_loss.0 <= 0 {
            Some(RT_MCIT_NO_NET)
        } else {
            None
        }
    }

    /// Item 40's or item 57's lock, if `key` is one and the return is for a
    /// year before 2020 (maintainer decision (pre-2020 1702RT computed like
    /// BIR)): the form computes both then.
    pub fn rt_before_2020_lock(draft: &Value, key: &str) -> Option<&'static str> {
        let reason = match key {
            RT_RATE => RT_RATE_BEFORE_2020,
            RT_ITEM_57 => RT_57_BEFORE_2020,
            _ => return None,
        };
        let year = draft["taxable_year"].as_u64()?;
        (year < u64::from(MCIT_ENTERED_FROM)).then_some(reason)
    }

    /// 1707A's printed Schedule 1 and 2 row 4 boxes and the continuation
    /// rows' column E: what fills each (form_1707a.rs main_row and
    /// recompute).
    fn a_row4_and_column_e() -> Vec<String> {
        let mut keys: Vec<String> = [
            "txtS1C1DateOfTransactionI4",
            "txtS1C2NameOfCorporateStockI4",
            "txtS1C3SellingPriceI4",
            "txtS1C4CostI4",
            "txtS1C5CapitalGainsI4",
            "txtS1C6TaxPaidI4",
            "txtS2C1DateOfTransactionI4",
            "txtS2C2NameOfCorporateStockI4",
            "txtS2C3SellingPriceI4",
            "txtS2C4CostI4",
            "txtS2C5CapitalLossI4",
        ]
        .iter()
        .map(|stem| format!("{A}{stem}"))
        .collect();
        for row in 1..=3 {
            keys.push(format!("{A}txtS1C5CapitalGainsI{row}"));
            keys.push(format!("{A}txtS2C5CapitalLossI{row}"));
        }
        keys.extend((1..=11).map(|row| format!("{A}txtS1C5CapitalGains_{row}")));
        keys.extend((1..=9).map(|row| format!("{A}txtS2C5CapitalLoss_{row}")));
        keys
    }

    /// 1702EX's last fixed line of each "(more...)" pop-up: Schedule 1 item
    /// 17i, Schedule 2 item 4 and Schedule 3 items 3, 6 and 8. Each shows the
    /// pop-up's first row, or OTHERS with the pop-up subtotal once it holds
    /// two or more (form_1702ex.rs to_field_map). `(description, amount)`.
    pub const EX_FOLDED_LINES: [(&str, &str); 5] = [
        ("txtPg3Pt6I17OthersIDesc", "txtPg3Pt6I17OthersIAmt"),
        ("txtPg3Pt6S2I4Description", "txtPg3Pt6S2I4Amount"),
        (
            "txtPg3Pt6S3I3NonDeductExpenseOtherIncomeDesc",
            "txtPg3Pt6S3I3NonDeductExpenseOtherIncomeAmt",
        ),
        (
            "txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
            "txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxAmt",
        ),
        (
            "txtPg3Pt6S3I8SpecialDeductDesc",
            "txtPg3Pt6S3I8SpecialDeductAmt",
        ),
    ];

    /// The forms this layer serves.
    const FORMS: [&str; 4] = ["1702RT", "1702MX", "1702EX", "1707A"];

    /// The locks that follow the draft as it is filled, each key with its
    /// reason or `null` (open): 1702RT's (Add more...) items, shut while
    /// their table holds two or more rows, item 42 (see [`rt_item_42_lock`])
    /// and items 40 and 57 (see [`rt_before_2020_lock`]). The page's open
    /// applies them with the other locks; its fill returns them, so a box
    /// shuts or opens as soon as the answer that decides it is typed.
    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if kind.form_code() != "1702RT" {
            return locks;
        }
        let more = rt_more_rows(draft);
        for modal in FORM_1702RT_MODALS {
            let lock = if more.is_folded(modal) {
                json!(RT_FOLDED_LOCK)
            } else {
                Value::Null
            };
            for key in rt_folded_columns(modal) {
                locks.insert(key, lock.clone());
            }
        }
        locks.insert(
            RT_MCIT_DUE.to_string(),
            rt_item_42_lock(draft).map_or(Value::Null, |reason| json!(reason)),
        );
        for key in [RT_RATE, RT_ITEM_57] {
            locks.insert(
                key.to_string(),
                rt_before_2020_lock(draft, key).map_or(Value::Null, |reason| json!(reason)),
            );
        }
        locks
    }

    /// The lock the probe gives a box whose value follows other draft fields
    /// and never echoes one of them.
    const DERIVED: &str = "cannot be filled here: derived by the app";

    /// The forms every derived box of which, the TIN's parts aside, the form
    /// computes (see [`relock`]).
    const DERIVED_IS_COMPUTED: [&str; 2] = ["1702RT", "1707A"];

    /// 1702MX's derived boxes the form computes: the MCIT ATC (from the
    /// incorporation date), Schedule 2 item 15's tax due and the Schedule 7.1
    /// and 8.1 item 7 years (this year's loss). Its other derived boxes (the
    /// tax rates, which the draft keeps as a `PercentInput`) are the filer's
    /// entries the probe could not bind; they keep their reason.
    const MX_COMPUTED: [&str; 5] = [
        "frm1702MX:chkPg1I5ATCR1",
        "frm1702MX:txtPg2Sc2It15B",
        "frm1702MX:txtPg2Sc2It15C",
        "frm1702MX:txtPg3IShed7_7Year",
        "frm1702MX:txtPg4IShed8_7Year",
    ];

    /// Locks that say what the form does with a box the probe could only
    /// call derived or constant: the form computes it. On 1702RT and 1707A
    /// every derived box but the TIN's parts (the return's identity, drawn
    /// as one TIN) is a total, a rate or a carried amount the form's own
    /// recompute writes; the boxes the filer types that recompute only
    /// clears are bound (probe.rs GATED_ENTRY).
    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        let code = kind.form_code();
        if !FORMS.contains(&code) {
            return;
        }
        // 1702RT: a folded (Add more...) item and a shut item 42.
        for (key, lock) in live_locks(kind, draft) {
            if !lock.is_null() {
                locks.insert(key, lock);
            }
        }
        let identity = |key: &str| key.contains("TIN") || key.contains("Tin");
        let derived: Vec<String> = locks
            .iter()
            .filter(|(key, reason)| {
                let derived = reason.as_str().is_some_and(|r| r.starts_with(DERIVED));
                derived
                    && ((DERIVED_IS_COMPUTED.contains(&code) && !identity(key))
                        || MX_COMPUTED.contains(&key.as_str()))
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in derived {
            locks.insert(key, json!(COMPUTED));
        }
        let computed: Vec<String> = match code {
            // Schedule IIIA item 4 is this year's loss: its year and amount
            // follow Schedule III item 3, its columns B to D stay at zero
            // (form_1702rt_official.rs recompute).
            "1702RT" => (1..=6)
                .map(|column| format!("{RT}txtPg4Sc3AI4C{column}"))
                .collect(),
            "1702EX" => EX_FOLDED_LINES
                .iter()
                .flat_map(|(description, amount)| [description, amount])
                .map(|stem| format!("{EX}{stem}"))
                .collect(),
            "1707A" => a_row4_and_column_e(),
            _ => Vec::new(),
        };
        for key in computed {
            if locks.contains_key(&key) {
                locks.insert(key, json!(COMPUTED));
            }
        }
        // 1702EX's rows BIR's program keeps shut, each box (P4-8), including
        // the Schedule 1 item 17i and Schedule 2 item 4 (more...) boxes.
        if code == "1702EX" {
            for key in ex_attached_keys().chain(ex_attached_more_keys()) {
                if let Some(reason) = ex_attached_lock(&key) {
                    locks.insert(key, json!(reason));
                }
            }
        }
        // 1702MX's rows BIR's program keeps shut (P2-3): Schedule 5 17d-17i
        // and Schedule 6 rows 2-4 name the attached schedule to fill.
        if code == "1702MX" {
            for key in mx_attached_keys() {
                if let Some(reason) = mx_attached_lock(&key) {
                    locks.insert(key, json!(reason));
                }
            }
        }
    }
}

pub mod as_needed {
    //! PR 9's 2552 locks (bir-shell `rows_as_needed` keeps the rest).

    use serde_json::{Map, Value};

    use crate::forms::agent::AgentFormKind;

    /// The boxes this layer shuts, and why, over the probed locks: 2552's
    /// add-more tax due (h) cells and Schedule 1's fixed rate (g), which
    /// form_2552.rs `recompute` computes (the probe saw them change with the
    /// item 11 answer and called them derived), and the add-more subtotals.
    pub fn relock(kind: AgentFormKind, locks: &mut Map<String, Value>) {
        if kind.form_code() != "2552" {
            return;
        }
        let computed = Value::from("computed by BIR's own formulas");
        for n in 1..=9 {
            for schedule in 1..=3 {
                locks.insert(
                    format!("frm2552:txtPg2Pt5Sch{schedule}_{n}Col8"),
                    computed.clone(),
                );
            }
            locks.insert(format!("frm2552:txtPg2Pt5Sch1_{n}Col7"), computed.clone());
        }
        for schedule in 1..=3 {
            locks.insert(format!("Pg2Pt5Sch{schedule}SubTotal"), computed.clone());
        }
    }
}

pub mod penalties {
    //! Penalties (rule 8): computed, but the filer can override them. A form
    //! on the shared penalty model (`crate::forms::penalty_items`) binds its
    //! Surcharge, Interest and Compromise boxes to the draft's per-component
    //! overrides (`<party>/penalty_overrides/surcharge`, ...). The three
    //! boxes of a party are one group: while no override is set the group is
    //! locked and shows the computed amounts; the page's one Edit opens all
    //! three, and an override files instead. 0605 (decided 2026-10-17) has no
    //! due date to compute a penalty from, so its boxes are open.
    //!
    //! The groups are the page's: pinned in `data/agent-locks.json`
    //! (`penalty_groups`) from the page schema's own boxes by bir-shell's
    //! `agent_locks_table` test, the way the host-written keys are. bir-shell
    //! `penalties` keeps the page's Edit, its group JSON and the kept flag.

    use serde::{Deserialize, Serialize};
    use serde_json::{Map, Value, json};

    use crate::forms::agent::{AgentFormKind, Binding};

    /// Why a computed penalty box is locked on the page.
    pub const LOCK: &str =
        "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to";

    /// Why a penalty group that is not part of the return (a spouse's column
    /// with no spouse) is locked: amounts entered there would not file.
    pub const NOT_ON_RETURN: &str = "computed; not part of this return as it stands";

    /// Why 2551Q's Items 20-22 are locked on an 8% return: the official page
    /// zeroes and disables them (`taxRateOption()`), so they take no amount.
    pub const EIGHT_PERCENT: &str = "nil on this return: Item 13 is the 8% income tax rate, which the official form files with no penalties";

    /// Why a 2551Q penalty box on an 8% return still shows an amount: a draft
    /// saved with the old manual switch keeps what it filed
    /// (`adopt_legacy_manual_penalties`), and a blank clears it.
    pub const KEPT_ON_EIGHT_PERCENT: &str = "Kept from an earlier version of this return. On the 8% rate the official form files no penalties; clear it to follow the official form.";

    /// Whether `draft` is a 2551Q return on Item 13 = 8%, whose Items 20-22
    /// take no amount.
    pub fn eight_percent(kind: AgentFormKind, draft: &Value) -> bool {
        kind == AgentFormKind::Form2551Q && draft["item_13_election"] == "eight_percent"
    }

    /// On a 2551Q 8% return saved with the old manual switch, the draft path
    /// of the non-zero amount `key`'s component still files (`/surcharge`,
    /// ...).
    pub fn kept_amount_path(kind: AgentFormKind, draft: &Value, key: &str) -> Option<String> {
        if !eight_percent(kind, draft) || draft["auto_compute_penalties"] != false {
            return None;
        }
        let (_, component) = override_of(kind, key)?;
        let path = format!("/{component}");
        draft
            .pointer(&path)
            .and_then(Value::as_f64)
            .filter(|amount| *amount != 0.0)
            .map(|_| path)
    }

    /// Why a penalty group that is not part of the return is locked.
    fn off_reason(kind: AgentFormKind, draft: &Value) -> &'static str {
        if eight_percent(kind, draft) {
            EIGHT_PERCENT
        } else {
            NOT_ON_RETURN
        }
    }

    /// Why `key` of a group is locked while no amount was entered there: a
    /// kept 8% amount, the computed amount, or not on the return.
    pub fn lock_reason(
        kind: AgentFormKind,
        group: &Group,
        draft: &Value,
        key: &str,
    ) -> &'static str {
        if kept_amount_path(kind, draft, key).is_some() {
            KEPT_ON_EIGHT_PERCENT
        } else if applies(kind, group, draft) {
            LOCK
        } else {
            off_reason(kind, draft)
        }
    }

    /// A party's three penalty components, as its overrides name them.
    pub const COMPONENTS: [&str; 3] = ["surcharge", "interest", "compromise"];

    /// One party's Surcharge, Interest and Compromise boxes.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Group {
        /// The draft path of the party's penalty state (`""` or `/spouse`).
        pub prefix: String,
        /// Its boxes on the page (a component may repeat on another page).
        pub keys: Vec<String>,
    }

    /// The override a box writes: `(party prefix, component)`.
    pub fn override_of(kind: AgentFormKind, key: &str) -> Option<(String, &'static str)> {
        let Some(Binding::Amount { path, .. }) = kind
            .definition()
            .bindings
            .get(key)
            .map(|bound| &bound.binding)
        else {
            return None;
        };
        let (prefix, component) = path.rsplit_once("/penalty_overrides/")?;
        let component = COMPONENTS.into_iter().find(|c| *c == component)?;
        Some((prefix.to_string(), component))
    }

    /// The form's penalty groups on the page (pinned): a party whose three
    /// overrides all have a box the filer types on the page, the filer's own
    /// first. A computed box bound to an override too (1700's spouse Total
    /// Penalties) is not one of them: it shows the total.
    pub fn groups(kind: AgentFormKind) -> Vec<Group> {
        crate::forms::agent::agent_locks(kind.package())
            .penalty_groups
            .clone()
    }

    /// Forms whose penalties the filer types, with no computed amount to
    /// lock: 0605 only (decided 2026-10-17; it has no due date to compute
    /// from).
    const OPEN_PENALTY_FORMS: [&str; 1] = ["0605"];

    /// Whether `kind`'s penalty groups are computed, locked until Edit opens
    /// them; false for 0605, whose penalty boxes are open.
    pub fn computes(kind: AgentFormKind) -> bool {
        !OPEN_PENALTY_FORMS.contains(&kind.form_code())
    }

    /// The groups locked until Edit opens them: every group, or none on 0605.
    pub fn computed_groups(kind: AgentFormKind) -> Vec<Group> {
        if computes(kind) {
            groups(kind)
        } else {
            Vec::new()
        }
    }

    /// Whether the filer has entered any of the group's amounts.
    pub fn edited(group: &Group, draft: &Value) -> bool {
        COMPONENTS.iter().any(|component| {
            draft
                .pointer(&format!("{}/penalty_overrides/{component}", group.prefix))
                .is_some_and(|value| !value.is_null())
        })
    }

    /// The draft path of a penalty box's override.
    pub fn path_of(kind: AgentFormKind, key: &str) -> Option<String> {
        override_of(kind, key)
            .map(|(prefix, component)| format!("{prefix}/penalty_overrides/{component}"))
    }

    /// Whether the group is part of the return as it stands: an amount
    /// entered there survives the form's computation. A spouse's column on a
    /// return with no spouse does not (the model leaves it to the form's own
    /// rules, which clear it), so it gets no Edit.
    pub fn applies(kind: AgentFormKind, group: &Group, draft: &Value) -> bool {
        if edited(group, draft) {
            return true;
        }
        let mut copy = draft.clone();
        // A 1701 or 1701Q column stored before the penalty model has no state
        // for the trial's overrides to land in; give the copy one first
        // ([`super::annual_individual::make_room`]), or the trial reads a
        // kept column as off the return. The draft itself is not touched.
        if let Some(key) = group.keys.first() {
            super::annual_individual::make_room(kind, &mut copy, key);
        }
        for component in COMPONENTS {
            let path = format!("{}/penalty_overrides/{component}", group.prefix);
            crate::forms::agent::set_draft_path(&mut copy, &path, json!(1.0));
        }
        crate::forms::agent::compute(kind, &mut copy).is_ok() && edited(group, &copy)
    }

    /// The penalty layer: every box of a computed group the filer has not
    /// edited, with [`lock_reason`]. Where that reason is [`LOCK`], a 1701Q
    /// (PR 5) or 1701 (PR 7) column, or an excise, 1800 or 1801 return
    /// stored before its penalties ran from the typed date (PR 11, P4-4),
    /// still on the filer's typed amounts says so instead. `only` limits the
    /// layer to that one box.
    pub(super) fn relock(
        kind: AgentFormKind,
        draft: &Value,
        locks: &mut Map<String, Value>,
        only: Option<&str>,
    ) {
        for group in computed_groups(kind) {
            if !edited(&group, draft) {
                for key in &group.keys {
                    if only.is_some_and(|only| only != key.as_str()) {
                        continue;
                    }
                    let reason = lock_reason(kind, &group, draft, key);
                    let reason = super::income_q::penalty_lock(kind, draft, key)
                        .or_else(|| super::annual_individual::penalty_lock(kind, draft, key))
                        .or_else(|| super::excise::penalty_lock(kind, draft, key))
                        .filter(|_| reason == LOCK)
                        .unwrap_or(reason);
                    locks.insert(key.clone(), json!(reason));
                }
            }
        }
    }
}

pub mod income_q {
    //! PR 5's 1701Q penalty locks (bir-shell `rows_income_q` keeps the
    //! column's join, the Civil Status and the gates).

    use serde_json::{Map, Value};

    use crate::forms::agent::AgentFormKind;
    use crate::forms::form_1701q::Form1701QDraft;

    /// The lock of a 1701Q penalty column that still holds the filer's own
    /// amounts (a draft stored, or a save file imported, before the penalty
    /// model: penalty_model_version 0); "computed from the due date" would
    /// be wrong there.
    pub const KEPT_AS_ENTERED: &str =
        "kept as entered before penalties were computed; choose Edit penalties to change them.";

    fn is_1701q(kind: AgentFormKind) -> bool {
        kind.form_code() == "1701Q"
    }

    /// The penalty declaration and stored state version of a 1701Q penalty
    /// box.
    pub fn penalty_column(kind: AgentFormKind, draft: &Value, key: &str) -> Option<(usize, u8)> {
        if !is_1701q(kind) {
            return None;
        }
        let form: Form1701QDraft = super::typed(draft).ok()?;
        Form1701QDraft::penalty_declarations()
            .iter()
            .enumerate()
            .find(|(_, items)| items.controls.contains(&key))
            .map(|(n, items)| (n, (items.state)(&form).penalty_model_version))
    }

    /// [`KEPT_AS_ENTERED`] for a penalty box of a column still on its typed
    /// amounts, `None` for every other box.
    pub fn penalty_lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
        penalty_column(kind, draft, key)
            .filter(|(_, version)| *version < crate::forms::penalty_items::PENALTY_MODEL_VERSION)
            .map(|_| KEPT_AS_ENTERED)
    }

    /// The spouse's penalty boxes (64B-66B) while their column is not part
    /// of the return: the page fades them with what opens them (its gates or
    /// the form's own clearing), so no penalty lock stands beside the tag.
    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        if !is_1701q(kind) {
            return;
        }
        let Ok(form) = super::typed::<Form1701QDraft>(draft) else {
            return;
        };
        let spouse = &Form1701QDraft::penalty_declarations()[1];
        if !(spouse.applies)(&form) {
            for key in spouse.controls {
                locks.remove(key);
            }
        }
    }
}

pub mod annual_individual {
    //! PR 7's 1701 penalty locks, and the room a 1701 or 1701Q column with
    //! no penalty state gets (bir-shell `rows_annual_individual` keeps the
    //! column's join and the rest).

    use serde_json::Value;

    use crate::forms::agent::AgentFormKind;
    use crate::forms::form_1701::Form1701Draft;

    /// The lock of a 1701 penalty column that still holds the filer's own
    /// amounts (a draft stored, or a save file imported, before the penalty
    /// model: penalty_model_version 0); "computed from the due date" would
    /// be wrong there.
    pub const KEPT_AS_ENTERED: &str =
        "kept as entered before penalties were computed; choose Edit penalties to change them.";

    /// The penalty declaration and stored state version of a 1701 penalty
    /// box.
    pub fn penalty_column(kind: AgentFormKind, draft: &Value, key: &str) -> Option<(usize, u8)> {
        if kind.form_code() != "1701" {
            return None;
        }
        let form: Form1701Draft = serde_json::from_value(draft.clone()).ok()?;
        Form1701Draft::penalty_declarations()
            .iter()
            .enumerate()
            .find(|(_, items)| items.controls.contains(&key))
            .map(|(n, items)| (n, (items.state)(&form).penalty_model_version))
    }

    /// [`KEPT_AS_ENTERED`] for a penalty box of a column still on its typed
    /// amounts, `None` for every other box.
    pub fn penalty_lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
        penalty_column(kind, draft, key)
            .filter(|(_, version)| *version < crate::forms::penalty_items::PENALTY_MODEL_VERSION)
            .map(|_| KEPT_AS_ENTERED)
    }

    /// A 1701 or 1701Q draft stored, or a save file imported, before the
    /// penalty model has no `penalty` / `spouse_penalty` object, so an
    /// amount typed after Edit penalties has no override to land in (round
    /// 4: a keyless 1701Q's typed 64A was refused). Give the column its
    /// (default, version 0) state first: the typed amount becomes its
    /// override and the column joins the model (form_1701_official.rs
    /// `official_recompute`; for 1701Q, form_1701q_official.rs's
    /// legacy-column join in its recompute, and penalty_items.rs
    /// `adopt_legacy_manual_penalties`). [`super::penalties::applies`] uses
    /// it on its trial copy too, so a kept keyless column reads as on the
    /// return; opening writes nothing.
    pub fn make_room(kind: AgentFormKind, draft: &mut Value, key: &str) {
        if matches!(kind.form_code(), "1701" | "1701Q") {
            give_state(kind, draft, key);
        }
    }

    /// Give `key`'s penalty column a default state when it has none.
    fn give_state(kind: AgentFormKind, draft: &mut Value, key: &str) {
        let Some(path) = super::penalties::path_of(kind, key) else {
            return;
        };
        let Some(slot) = path.trim_start_matches('/').split('/').next() else {
            return;
        };
        if let Some(object) = draft.as_object_mut()
            && object.get(slot).is_none_or(Value::is_null)
            && let Ok(state) =
                serde_json::to_value(crate::forms::penalty_items::PenaltyState::default())
        {
            object.insert(slot.to_string(), state);
        }
    }
}

pub mod excise {
    //! PR 11's penalty locks on the excise returns (2200A, 2200AN, 2200C,
    //! 2200M, 2200P, 2200S, 2200T), and on 1800 and 1801 (P4-4). Their model
    //! measures penalties from Item 1 (`PenaltyItems::typed_due_date`;
    //! maintainer decision D8, excise penalties run from the typed date;
    //! maintainer decision, 1800/1801 penalties run from the due date). A
    //! draft stored before that keeps what it holds: its penalty boxes are
    //! "kept as entered before penalties were computed" until the filer
    //! chooses Edit penalties. bir-shell `rows_excise` keeps the join, the
    //! gates and the printed messages.

    use serde::de::DeserializeOwned;
    use serde_json::{Map, Value, json};

    use crate::forms::agent::AgentFormKind;
    use crate::forms::queueable::{QueueableForm, QueueableKind};
    use crate::forms::{FilingStatus, form_1800, form_1801};
    use crate::forms::{form_2200a, form_2200an, form_2200c, form_2200m};
    use crate::forms::{form_2200p, form_2200s, form_2200t};

    /// The lock of an excise return's penalty box while the draft predates
    /// decision D8: the amounts are the ones it was stored with.
    pub const KEPT_AS_ENTERED: &str =
        "kept as entered before penalties were computed; choose Edit penalties to change them.";

    /// The excise return `kind` is, if it is one.
    pub fn excise(kind: AgentFormKind) -> Option<QueueableKind> {
        match kind {
            AgentFormKind::Queueable(
                queueable @ (QueueableKind::Form2200A
                | QueueableKind::Form2200AN
                | QueueableKind::Form2200C
                | QueueableKind::Form2200M
                | QueueableKind::Form2200P
                | QueueableKind::Form2200S
                | QueueableKind::Form2200T),
            ) => Some(queueable),
            _ => None,
        }
    }

    /// The return `kind` is if its penalties run from a date the filer
    /// types: an excise return (maintainer decision (excise penalties run
    /// from the typed date)) or 1800 and 1801 (maintainer decision
    /// (1800/1801 penalties run from the due date)).
    pub fn typed_due(kind: AgentFormKind) -> Option<QueueableKind> {
        match kind {
            AgentFormKind::Queueable(
                queueable @ (QueueableKind::Form1800 | QueueableKind::Form1801),
            ) => Some(queueable),
            _ => excise(kind),
        }
    }

    /// Whether `draft` is a Draft whose penalties are measured from a typed
    /// date it is not yet on (stored before D8).
    #[inline(never)]
    fn stored_before_typed_due_date<F: QueueableForm + DeserializeOwned>(draft: &Value) -> bool {
        let Ok(form) = serde_json::from_value::<F>(draft.clone()) else {
            return false;
        };
        matches!(form.lifecycle().status, FilingStatus::Draft)
            && F::penalty_items()
                .iter()
                .any(|items| items.typed_due_date.is_some() && !items.on_typed_due_date(&form))
    }

    /// The penalty controls of the form (`PenaltyItems::controls`).
    #[inline(never)]
    fn penalty_controls<F: QueueableForm>() -> Vec<&'static str> {
        F::penalty_items()
            .iter()
            .flat_map(|items| items.controls)
            .collect()
    }

    /// Whether `draft`, a return of `kind` (see [`typed_due`]), was stored
    /// before its penalties ran from the typed date.
    pub fn before_d8(kind: QueueableKind, draft: &Value) -> bool {
        match kind {
            QueueableKind::Form1800 => {
                stored_before_typed_due_date::<form_1800::Form1800Draft>(draft)
            }
            QueueableKind::Form1801 => {
                stored_before_typed_due_date::<form_1801::Form1801Draft>(draft)
            }
            QueueableKind::Form2200A => {
                stored_before_typed_due_date::<form_2200a::Form2200ADraft>(draft)
            }
            QueueableKind::Form2200AN => {
                stored_before_typed_due_date::<form_2200an::Form2200ANDraft>(draft)
            }
            QueueableKind::Form2200C => {
                stored_before_typed_due_date::<form_2200c::Form2200CDraft>(draft)
            }
            QueueableKind::Form2200M => {
                stored_before_typed_due_date::<form_2200m::Form2200MDraft>(draft)
            }
            QueueableKind::Form2200P => {
                stored_before_typed_due_date::<form_2200p::Form2200PDraft>(draft)
            }
            QueueableKind::Form2200S => {
                stored_before_typed_due_date::<form_2200s::Form2200SDraft>(draft)
            }
            QueueableKind::Form2200T => {
                stored_before_typed_due_date::<form_2200t::Form2200TDraft>(draft)
            }
            _ => false,
        }
    }

    fn controls(kind: QueueableKind) -> Vec<&'static str> {
        match kind {
            QueueableKind::Form1800 => penalty_controls::<form_1800::Form1800Draft>(),
            QueueableKind::Form1801 => penalty_controls::<form_1801::Form1801Draft>(),
            QueueableKind::Form2200A => penalty_controls::<form_2200a::Form2200ADraft>(),
            QueueableKind::Form2200AN => penalty_controls::<form_2200an::Form2200ANDraft>(),
            QueueableKind::Form2200C => penalty_controls::<form_2200c::Form2200CDraft>(),
            QueueableKind::Form2200M => penalty_controls::<form_2200m::Form2200MDraft>(),
            QueueableKind::Form2200P => penalty_controls::<form_2200p::Form2200PDraft>(),
            QueueableKind::Form2200S => penalty_controls::<form_2200s::Form2200SDraft>(),
            QueueableKind::Form2200T => penalty_controls::<form_2200t::Form2200TDraft>(),
            _ => Vec::new(),
        }
    }

    /// [`KEPT_AS_ENTERED`] for a penalty box of an excise return stored
    /// before maintainer decision (excise penalties run from the typed date),
    /// or of a 1800 or 1801 return stored before maintainer decision
    /// (1800/1801 penalties run from the due date); `None` for every other
    /// box and form.
    pub fn penalty_lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
        let kind = typed_due(kind)?;
        (controls(kind).contains(&key) && before_d8(kind, draft)).then_some(KEPT_AS_ENTERED)
    }

    /// The penalty locks of an excise, 1800 or 1801 return as a fill leaves
    /// them, for the page to apply at once (`relocks`): a group still
    /// computed is locked as computed, one kept as entered says so, an
    /// edited one is open (`null`). Empty for every other form.
    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if typed_due(kind).is_none() {
            return locks;
        }
        for group in super::penalties::groups(kind) {
            let edited = super::penalties::edited(&group, draft);
            for key in &group.keys {
                let lock = if edited {
                    Value::Null
                } else if !super::penalties::applies(kind, &group, draft) {
                    json!(super::penalties::NOT_ON_RETURN)
                } else {
                    json!(penalty_lock(kind, draft, key).unwrap_or(super::penalties::LOCK))
                };
                locks.insert(key.clone(), lock);
            }
        }
        locks
    }
}

pub mod item16_2551q {
    //! 2551Q Item 16 (tax paid in a previously filed return) is shut unless
    //! Amended Return is Yes without Item 13's 8% option. The official page
    //! keeps the box disabled and zeroed in those states
    //! (`rules/forms/2551q-v2018/validations.json` rules
    //! `2551q-amended-toggle` and `2551q-tax-rate-eight-percent`).

    use serde_json::{Map, Value, json};

    use crate::forms::agent::AgentFormKind;

    pub const KEY: &str = "frm2551Qv2018:txt16";
    pub const AMENDED_LOCK: &str = "Item 16 applies only to an amended return.";
    pub const EIGHT_PERCENT_LOCK: &str =
        "Item 16 is nil on this return: Item 13 is the 8% income tax rate.";
    pub const CLEARED_AMENDED_NOTE: &str =
        "Item 16 was cleared: it applies only to an amended return.";
    pub const CLEARED_EIGHT_PERCENT_NOTE: &str =
        "Item 16 was cleared: Item 13 is the 8% income tax rate.";

    /// The reason Item 16 is shut on `draft`, or `None` while it is open.
    pub fn lock_reason(draft: &Value) -> Option<&'static str> {
        if draft["item_13_election"] == "eight_percent" {
            Some(EIGHT_PERCENT_LOCK)
        } else if draft["is_amended"] != true {
            Some(AMENDED_LOCK)
        } else {
            None
        }
    }

    /// Whether `value` is the zero amount a shut Item 16 accepts as a no-op.
    pub fn zero_noop(value: &Value) -> bool {
        value.as_f64() == Some(0.0)
            || value
                .as_str()
                .is_some_and(|text| crate::forms::agent::parse_amount(text) == Some(0.0))
    }

    /// The clearing note for a transition into the closed state, by cause.
    pub fn cleared_note(draft: &Value) -> Option<&'static str> {
        if draft["item_13_election"] == "eight_percent" {
            Some(CLEARED_EIGHT_PERCENT_NOTE)
        } else if draft["is_amended"] != true {
            Some(CLEARED_AMENDED_NOTE)
        } else {
            None
        }
    }

    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        if kind != AgentFormKind::Form2551Q {
            return;
        }
        match lock_reason(draft) {
            Some(reason) => {
                locks.insert(KEY.to_string(), json!(reason));
            }
            None => {
                locks.remove(KEY);
            }
        }
    }

    /// Item 16's lock as a fill leaves it (`null` opens it), so the page
    /// shuts or opens the box as soon as Amended Return or Item 13 changes.
    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if kind == AgentFormKind::Form2551Q {
            locks.insert(
                KEY.to_string(),
                lock_reason(draft).map_or(Value::Null, |reason| json!(reason)),
            );
        }
        locks
    }
}

pub mod installment_1606 {
    //! 1606 Items 21 to 26 are installment-sale entries. The official page
    //! opens them only for Item 20's installment-sale choice
    //! (`rules/forms/1606-v2018/fixtures/validation-function-inventory-v796.json`,
    //! `enableInstallment`: L3230-L3266) and keeps them disabled at load
    //! (`runtime-control-inventory-v796.json`).
    //!
    //! Item 21 is a copy of Item 28A's gross selling price, so its amount is
    //! never a separate stored installment value; the lock stops writes into
    //! the copy while it is shut. Items 22 to 26 are installment-only model
    //! amounts and are cleared by `Form1606Draft::recompute` while shut.

    use serde_json::{Map, Value, json};

    use crate::forms::agent::AgentFormKind;

    pub const KEYS: [&str; 6] = [
        "frm1606:txtSelling",
        "frm1606:txtCost",
        "frm1606:txtMortgage",
        "frm1606:txtTotalP",
        "frm1606:txtAmount",
        "frm1606:txtTotalN",
    ];
    pub const LOCK: &str = "Items 21 to 26 apply only to an installment sale.";
    pub const LOCK_NOTE: &str = "Items 21 to 26 apply only to an installment sale. Leave them blank unless Item 20 is Installment Sale.";
    pub const CLEARED_NOTE: &str =
        "Items 21 to 26 were cleared: they apply only to an installment sale.";
    pub const AMOUNT_FIELDS: [&str; 5] = [
        "cost_and_expenses",
        "mortgage_assumed",
        "initial_year_payments",
        "installment_this_month",
        "number_of_installments",
    ];

    /// Whether installment-only amounts lie dormant on a draft whose gate is shut.
    pub fn dormant_amounts(draft: &Value) -> bool {
        lock_reason(draft).is_some()
            && AMOUNT_FIELDS
                .iter()
                .any(|field| draft[field].as_f64().unwrap_or(0.0).abs() >= 0.005)
    }

    /// The reason the boxes are shut on `draft`, or `None` for an installment sale.
    pub fn lock_reason(draft: &Value) -> Option<&'static str> {
        if draft["transaction"] == "installment_sale" {
            None
        } else {
            Some(LOCK)
        }
    }

    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        if kind.form_code() != "1606" {
            return;
        }
        match lock_reason(draft) {
            Some(reason) => {
                for key in KEYS {
                    locks.insert(key.to_string(), json!(reason));
                }
            }
            None => {
                for key in KEYS {
                    locks.remove(key);
                }
            }
        }
    }

    /// The boxes' locks as a fill leaves them (`null` opens them), so the
    /// page follows Item 20 as soon as its choice changes.
    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if kind.form_code() == "1606" {
            for key in KEYS {
                locks.insert(
                    key.to_string(),
                    lock_reason(draft).map_or(Value::Null, |reason| json!(reason)),
                );
            }
        }
        locks
    }
}

pub mod previous_payment {
    //! Previous-payment boxes the official pages open only for an amended
    //! return. One table serves the forms whose in-repository runtime-control
    //! inventory records the box disabled at load; 0619E, 0619F, 2550M and
    //! 1602Q are deliberately absent because their captured pages leave the
    //! surveyed box enabled on a new return.

    use serde_json::{Map, Value, json};

    use crate::forms::agent::AgentFormKind;

    pub struct Spec {
        pub form_code: &'static str,
        pub key: &'static str,
        pub item: &'static str,
        pub path: &'static str,
        pub yes_key: &'static str,
        pub no_key: &'static str,
        pub lock: &'static str,
        pub cleared: &'static str,
    }

    pub const SPECS: [Spec; 6] = [
        Spec {
            form_code: "1600PT",
            key: "frm1600PT:txtPg1P2I19TaxRemittedInRtrn",
            item: "19",
            path: "tax_remitted_previous",
            yes_key: "frm1600PT:rdoPg1I2AmendedYes",
            no_key: "frm1600PT:rdoPg1I2AmendedNo",
            lock: "Item 19 applies only to an amended return.",
            cleared: "Item 19 was cleared: it applies only to an amended return.",
        },
        Spec {
            form_code: "1600VT",
            key: "frm1600VT:txtPg1P2I19TaxRemittedInRtrn",
            item: "19",
            path: "tax_remitted_previous",
            yes_key: "frm1600VT:rdoPg1I2AmendedYes",
            no_key: "frm1600VT:rdoPg1I2AmendedNo",
            lock: "Item 19 applies only to an amended return.",
            cleared: "Item 19 was cleared: it applies only to an amended return.",
        },
        Spec {
            form_code: "1600WP",
            key: "frm1600WP:txtTax13",
            item: "13",
            path: "tax_remitted_previous",
            yes_key: "frm1600WP:AmendedReturn_1",
            no_key: "frm1600WP:AmendedReturn_2",
            lock: "Item 13 applies only to an amended return.",
            cleared: "Item 13 was cleared: it applies only to an amended return.",
        },
        Spec {
            form_code: "1603Q",
            key: "frm1603Q:txtTax15",
            item: "15",
            path: "tax_remitted_previous",
            yes_key: "frm1603Q:AmendedRtn_1",
            no_key: "frm1603Q:AmendedRtn_2",
            lock: "Item 15 applies only to an amended return.",
            cleared: "Item 15 was cleared: it applies only to an amended return.",
        },
        Spec {
            form_code: "1606",
            key: "frm1606:txtLess",
            item: "33",
            path: "tax_remitted_previous",
            yes_key: "frm1606:j_id217:_1",
            no_key: "frm1606:j_id217:_2",
            lock: "Item 33 applies only to an amended return.",
            cleared: "Item 33 was cleared: it applies only to an amended return.",
        },
        Spec {
            form_code: "1706",
            key: "frm1706:txtLess",
            item: "33",
            path: "tax_paid_previous",
            yes_key: "frm1706:j_id217:_1",
            no_key: "frm1706:j_id217:_2",
            lock: "Item 33 applies only to an amended return.",
            cleared: "Item 33 was cleared: it applies only to an amended return.",
        },
    ];

    pub fn spec(kind: AgentFormKind) -> Option<&'static Spec> {
        SPECS.iter().find(|spec| spec.form_code == kind.form_code())
    }

    pub fn lock_text(spec: &Spec) -> &'static str {
        spec.lock
    }

    pub fn cleared_text(spec: &Spec) -> &'static str {
        spec.cleared
    }

    /// The reason the box is shut on `draft`, or `None` for Amended Yes.
    pub fn lock_reason(spec: &Spec, draft: &Value) -> Option<&'static str> {
        (draft["is_amended"] != true).then_some(spec.lock)
    }

    /// A zero write is the box's already-empty state, so it is accepted as a no-op.
    pub fn zero_write(value: &Value) -> bool {
        value.as_f64() == Some(0.0)
            || value
                .as_str()
                .is_some_and(|text| crate::forms::agent::parse_amount(text) == Some(0.0))
    }

    /// The stable key this gate's pending notice uses.
    pub fn gate_key(spec: &Spec) -> String {
        format!(
            "{}_item{}",
            spec.form_code.to_lowercase(),
            spec.item.to_lowercase()
        )
    }

    /// A filed original whose box was shut must not revive its dormant amount
    /// when an amendment turns the copy's Amended answer to Yes.
    pub fn clear_dormant_amendment_copy(
        kind: AgentFormKind,
        original: &Value,
        amended: &mut Value,
    ) {
        let Some(spec) = spec(kind) else {
            return;
        };
        if dormant_amount(spec, original)
            && let Value::Object(object) = amended
        {
            object.insert(spec.path.to_string(), json!(0.0));
        }
    }

    /// Whether a closed draft still holds a dormant previous-payment amount.
    pub fn dormant_amount(spec: &Spec, draft: &Value) -> bool {
        lock_reason(spec, draft).is_some()
            && draft[spec.path].as_f64().unwrap_or(0.0).abs() >= 0.005
    }

    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        let Some(spec) = spec(kind) else {
            return;
        };
        match lock_reason(spec, draft) {
            Some(reason) => {
                locks.insert(spec.key.to_string(), json!(reason));
            }
            None => {
                locks.remove(spec.key);
            }
        }
    }

    /// The box's lock as a fill leaves it (`null` opens it), so the page
    /// follows Amended Return as soon as its choice changes.
    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if let Some(spec) = spec(kind) {
            locks.insert(
                spec.key.to_string(),
                lock_reason(spec, draft).map_or(Value::Null, |reason| json!(reason)),
            );
        }
        locks
    }
}

pub mod item_13 {
    //! 2551Q Item 13, the income-tax-rate election, is answered once a year,
    //! on the initial quarter. A later quarter (Q2-Q4) of a TIN whose Q1 of
    //! the same taxable year this app holds as filed (Submitted, Confirmed or
    //! Paid) takes that Q1's answer, and its Item 13 boxes are locked to it.
    //! Without one here (Q1 filed in the original eBIRForms before
    //! migrating, or not filed yet) Item 13 stays the filer's.
    //!
    //! The fact comes from the database when the return is opened
    //! ([`refresh`]): the draft keeps it as `filed_q1_item_13`
    //! ([`Form2551QDraft::adopt_filed_q1_item_13`]), and the lock reads it
    //! there, so the page's open, its fill and the assistant judge one value.

    use serde_json::{Map, Value};

    use crate::db::{Database, DbError};
    use crate::forms::agent::AgentFormKind;
    use crate::forms::form_2551q::Form2551QDraft;

    /// The Item 13 group box and its radios, including Not applicable.
    pub const KEYS: [&str; 4] = [
        "frm2551Qv2018:taxRate",
        "frm2551Qv2018:taxRate1",
        "frm2551Qv2018:taxRate2",
        "frm2551Qv2018:taxRateNA",
    ];

    /// The start of every Item 13 lock reason (the filer's words).
    pub const REASON_START: &str = "Set on your filed Q1 return for ";

    /// Record on `draft` the Item 13 answer of its TIN's filed Q1 of the
    /// same taxable year that `db` holds (none clears it). A Q1 draft, or a
    /// return that left Draft, is not changed.
    pub fn refresh(db: &Database, draft: &mut Form2551QDraft) -> Result<(), DbError> {
        if draft.quarter == 1 || !draft.is_editable() {
            return Ok(());
        }
        let filed = db.filed_2551q_q1_item_13(&draft.tin, draft.taxable_year)?;
        draft.adopt_filed_q1_item_13(filed);
        Ok(())
    }

    /// Why Item 13 of `draft` is locked: a later quarter holding its filed
    /// Q1's answer.
    fn reason(kind: AgentFormKind, draft: &Value) -> Option<String> {
        if kind != AgentFormKind::Form2551Q || draft["quarter"].as_u64() == Some(1) {
            return None;
        }
        let filed = draft["filed_q1_item_13"].as_str()?;
        if filed == "unanswered" {
            return None;
        }
        let year = draft["taxable_year"].as_u64()?;
        Some(format!(
            "{REASON_START}{year}. Item 13 is answered once a year, on the initial quarter."
        ))
    }

    /// The lock on `key` when it is an Item 13 box of a locked return.
    pub fn lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<String> {
        if !KEYS.contains(&key) {
            return None;
        }
        reason(kind, draft)
    }

    /// Lock the Item 13 boxes of a later quarter holding its filed Q1's
    /// answer.
    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        if let Some(reason) = reason(kind, draft) {
            for key in KEYS {
                locks.insert(key.to_string(), Value::from(reason.clone()));
            }
        }
    }
}

/// 1601FQ Schedule 1 while Item 13 is not Yes. The official page clears and
/// disables the schedule (`disableSelTreaty`) and tells the filer there is no
/// need to fill it up while they are not availing of tax relief.
pub mod schedule1_1601fq {
    use serde_json::{Map, Value, json};

    use super::AgentFormKind;

    pub const ROWS: usize = 5;
    pub const STEMS: [&str; 6] = [
        "drpTreatyCode",
        "drpATCCode",
        "txtNatIncPayment",
        "txtAmtIncomePay",
        "txtRate",
        "txtReqWithheld",
    ];
    const EDITABLE_STEMS: [&str; 4] = ["drpTreatyCode", "drpATCCode", "txtAmtIncomePay", "txtRate"];
    const COMPUTED_STEMS: [&str; 2] = ["txtNatIncPayment", "txtReqWithheld"];
    const COMPUTED_LOCK: &str = "computed by BIR's own formulas";
    pub const LOCK: &str = "You are not availing of tax relief under Special Law or International Tax Treaty. There is no need to fill up schedule 1.";
    pub const CLEARED_NOTE: &str = "Schedule 1 was cleared: it applies only when item 13 is Yes.";

    fn stem_keys(stems: &[&str]) -> Vec<String> {
        (0..ROWS)
            .flat_map(|row| stems.iter().map(move |stem| format!("{stem}{row}")))
            .collect()
    }

    pub fn keys() -> Vec<String> {
        stem_keys(&STEMS)
    }

    pub fn is_key(key: &str) -> bool {
        STEMS.iter().any(|stem| {
            key.strip_prefix(stem)
                .and_then(|row| row.parse::<usize>().ok())
                .is_some_and(|row| row < ROWS)
        })
    }

    pub fn lock_reason(draft: &Value) -> Option<&'static str> {
        if draft["tax_relief"].is_null() {
            Some(LOCK)
        } else {
            None
        }
    }

    fn typed_row(row: &Value) -> bool {
        row["treaty_code"] != "NA"
            || row["atc_code"] != "NA"
            || row["income_payment"].as_f64().unwrap_or(0.0).abs() >= 0.005
            || row["tax_rate"].as_f64().unwrap_or(0.0).abs() >= 0.005
    }

    pub fn typed_rows(draft: &Value) -> bool {
        draft["schedule1"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(typed_row))
    }

    pub fn dormant_rows(draft: &Value) -> bool {
        lock_reason(draft).is_some() && typed_rows(draft)
    }

    /// A row's stable identity. Rows written by this build carry
    /// `instance_id`; a pre-identity draft is seeded once in its stored
    /// order, after which the id travels with the row object through a
    /// Delete row compaction.
    pub fn row_id(row: &Value, legacy_index: usize) -> String {
        row["instance_id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("row-{:020}", legacy_index + 1))
    }

    /// The current vector position of a stable row id in `draft`.
    pub fn row_index(draft: &Value, id: &str) -> Option<usize> {
        draft["schedule1"]
            .as_array()?
            .iter()
            .position(|row| row["instance_id"].as_str() == Some(id))
    }

    /// The six keys of the row currently at `row`.
    pub fn keys_for_row(row: usize) -> Vec<String> {
        STEMS.iter().map(|stem| format!("{stem}{row}")).collect()
    }

    #[derive(Debug)]
    pub struct RowClearance {
        pub id: String,
        pub gate: String,
        pub text: String,
    }

    /// Every typed Schedule 1 row a closing Item 13 gate cleared, keyed by
    /// the row's stable id rather than its transient vector index.
    pub fn row_clearances(kind: AgentFormKind, before: &Value, after: &Value) -> Vec<RowClearance> {
        if kind.form_code() != "1601FQ" || lock_reason(after).is_none() {
            return Vec::new();
        }
        let empty_after = |id: &str| {
            after["schedule1"]
                .as_array()
                .and_then(|rows| {
                    rows.iter()
                        .find(|row| row["instance_id"].as_str() == Some(id))
                })
                .is_none_or(|row| !typed_row(row))
        };
        before["schedule1"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, row)| typed_row(row))
            .filter_map(|(index, row)| {
                let id = row_id(row, index);
                if !empty_after(&id) {
                    return None;
                }
                let gate = format!("1601fq_schedule1:{id}");
                let text = format!(
                    "Schedule 1 row {id} was cleared: it applies only when item 13 is Yes."
                );
                Some(RowClearance { id, gate, text })
            })
            .collect()
    }

    /// The helper interface's owned `(gate, sentence)` pairs.
    pub fn clearances(kind: AgentFormKind, before: &Value, after: &Value) -> Vec<(String, String)> {
        row_clearances(kind, before, after)
            .into_iter()
            .map(|row| (row.gate, row.text))
            .collect()
    }

    pub fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
        if kind.form_code() != "1601FQ" {
            return;
        }
        match lock_reason(draft) {
            Some(reason) => {
                for key in keys() {
                    locks.insert(key, json!(reason));
                }
            }
            None => {
                for key in keys() {
                    locks.remove(&key);
                }
            }
        }
    }

    pub fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
        let mut locks = Map::new();
        if kind.form_code() != "1601FQ" {
            return locks;
        }
        if let Some(reason) = lock_reason(draft) {
            for key in keys() {
                locks.insert(key, json!(reason));
            }
        } else {
            // Opening Item 13 unlocks only the row's writable cells. Its two
            // ATC-derived columns stay computed, as the base lock map draws
            // them on a return that opens with Item 13 already Yes.
            for key in stem_keys(&EDITABLE_STEMS) {
                locks.insert(key, Value::Null);
            }
            for key in stem_keys(&COMPUTED_STEMS) {
                locks.insert(key, json!(COMPUTED_LOCK));
            }
        }
        locks
    }
}

#[cfg(test)]
mod tests {
    //! The assistant's half of the draft-dependent layer: [`super::super::fill`]
    //! (nothing lifted) refuses what the page's open locks, with the page's
    //! reason, judged on the draft as it stands when the key is written;
    //! the page's own writers are not held to it.

    use serde::Serialize;
    use serde_json::{Map, Value, json};

    use super::{annual_corporate, draft_lock, draft_locks, dst_pt_vat, penalties};
    use crate::forms::agent::{self, AgentFormKind, Lock};

    /// A fictitious corporate or individual filer, incorporated (or started)
    /// in 2010.
    fn profile(code: &str, taxpayer_type: &str) -> crate::profile::TaxpayerProfile {
        serde_json::from_value(json!({
            "id": null,
            "full_name": "Example Store",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "store@example.test",
            "default_form_type": code,
            "taxpayer_type": taxpayer_type,
            "business_start_date": "2010-01-04",
        }))
        .unwrap()
    }

    /// `draft` as JSON, recomputed as the page leaves it.
    fn computed(kind: AgentFormKind, draft: impl Serialize) -> Value {
        let mut json = serde_json::to_value(draft).unwrap();
        agent::compute(kind, &mut json).unwrap();
        json
    }

    fn kind(code: &str) -> AgentFormKind {
        AgentFormKind::from_code(code).unwrap()
    }

    /// The page's open locks a box this layer locks for the same reason the
    /// assistant is refused with.
    fn page_lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<String> {
        let mut locks = Map::new();
        draft_locks(kind, draft, &mut locks);
        locks.get(key).and_then(Value::as_str).map(str::to_string)
    }

    /// An unedited computed penalty is refused to the assistant with the
    /// page's penalty reason; once its group holds a typed amount (the
    /// page's Edit penalties, through the page's own writer) the same write
    /// goes through.
    #[test]
    fn an_unedited_computed_penalty_is_refused_until_its_group_is_edited() {
        use crate::forms::form_0619e::Form0619EDraft;
        let kind = kind("0619E");
        let profile = profile("0619E", "Corporation");
        let mut draft = computed(kind, Form0619EDraft::new_from_profile(&profile, 2025, 6));
        for key in [
            "frm0619E:txtTax17A",
            "frm0619E:txtTax17B",
            "frm0619E:txtTax17C",
        ] {
            let refused =
                agent::fill(kind, &mut draft.clone(), key, &json!("100"), None).unwrap_err();
            assert_eq!(refused, format!("`{key}` {}", penalties::LOCK));
            assert_eq!(
                page_lock(kind, &draft, key).as_deref(),
                Some(penalties::LOCK)
            );
        }
        // The page's Edit: an amount the filer types in the group opens it.
        agent::fill_as_user(kind, &mut draft, "frm0619E:txtTax17B", &json!("5")).unwrap();
        assert_eq!(page_lock(kind, &draft, "frm0619E:txtTax17A"), None);
        agent::fill(kind, &mut draft, "frm0619E:txtTax17A", &json!("100"), None).unwrap();
        assert_eq!(
            draft["penalty_overrides"]["surcharge"].as_f64(),
            Some(100.0)
        );
    }

    /// 2000's 15B and 15C are totals of Schedules 2 and 3: the probe called
    /// them derived (the base map's "cannot be filled here: derived by the
    /// app"), and the page says what they are, computed. The assistant is
    /// refused with the page's reason.
    #[test]
    fn form_2000_items_15b_and_15c_are_refused_as_computed() {
        use crate::forms::form_2000::Form2000Draft;
        let kind = kind("2000");
        let profile = profile("2000", "Corporation");
        let draft = computed(kind, Form2000Draft::new_from_profile(&profile, 2025, 6));
        for key in ["frm2000:txtTax15B", "frm2000:txtTax15C"] {
            let refused =
                agent::fill(kind, &mut draft.clone(), key, &json!("100"), None).unwrap_err();
            assert_eq!(refused, format!("`{key}` computed by BIR's own formulas"));
        }
    }

    /// 2553's quarter boxes pick which return this is, and each row's
    /// classification follows the ATC chosen in its row: refused with the
    /// page's reasons.
    #[test]
    fn a_2553_quarter_box_and_classification_are_refused() {
        use crate::forms::form_2553::Form2553Draft;
        let kind = kind("2553");
        let profile = profile("2553", "Corporation");
        let draft = computed(kind, Form2553Draft::new_from_profile(&profile, 2025, 2));
        for quarter in 1..=4 {
            let key = format!("frm2553:optQtr:_{quarter}");
            let refused =
                agent::fill(kind, &mut draft.clone(), &key, &json!("true"), None).unwrap_err();
            assert_eq!(refused, format!("`{key}` {}", Lock::Identity.reason()));
        }
        for item in 14..=18 {
            let key = format!("frm2553:txt{item}A");
            let refused =
                agent::fill(kind, &mut draft.clone(), &key, &json!("X"), None).unwrap_err();
            assert_eq!(
                refused,
                format!("`{key}` {}", dst_pt_vat::CLASSIFICATION_LOCK)
            );
        }
    }

    /// 1702RT before 2020: the form computes item 42 (2% of gross income)
    /// and items 40 and 57, so the assistant is refused all three with the
    /// page's reasons; from 2020, with net taxable income above zero,
    /// item 42 is the filer's again.
    #[test]
    fn item_42_of_a_1702rt_before_2020_is_refused() {
        use crate::forms::form_1702rt::Form1702RTDraft;
        let kind = kind("1702RT");
        let profile = profile("1702RT", "Corporation");
        let draft = computed(kind, Form1702RTDraft::new_from_profile(&profile, 2019, 12));
        for (key, reason) in [
            (
                annual_corporate::RT_MCIT_DUE,
                annual_corporate::RT_MCIT_BEFORE_2020,
            ),
            (
                annual_corporate::RT_RATE,
                annual_corporate::RT_RATE_BEFORE_2020,
            ),
            (
                annual_corporate::RT_ITEM_57,
                annual_corporate::RT_57_BEFORE_2020,
            ),
        ] {
            let refused =
                agent::fill(kind, &mut draft.clone(), key, &json!("20000"), None).unwrap_err();
            assert_eq!(refused, format!("`{key}` {reason}"));
        }
        let mut later = computed(kind, Form1702RTDraft::new_from_profile(&profile, 2025, 12));
        agent::fill(
            kind,
            &mut later,
            "frm1702RT:txtPg2Pt4I27Sales",
            &json!("1000000"),
            None,
        )
        .unwrap();
        agent::compute(kind, &mut later).unwrap();
        agent::fill(
            kind,
            &mut later,
            annual_corporate::RT_MCIT_DUE,
            &json!("20000"),
            None,
        )
        .unwrap();
    }

    /// 1701Q's spouse penalties while column B is not on the return: the
    /// page shows them open (faded with what opens them, not locked), and the
    /// assistant is not refused them; the filer's own computed column stays
    /// refused.
    #[test]
    fn spouse_penalties_of_a_1701q_with_column_b_off_are_not_refused() {
        use crate::forms::form_1701q::Form1701QDraft;
        let kind = kind("1701Q");
        let profile = profile("1701Q", "Individual");
        let draft = computed(kind, Form1701QDraft::new_from_profile(&profile, 2025, 2));
        assert_eq!(draft["has_spouse"], false, "column B is off");
        for key in ["frm1701q:txt64B", "frm1701q:txt65B", "frm1701q:txt66B"] {
            assert_eq!(
                page_lock(kind, &draft, key),
                None,
                "{key} is open on the page"
            );
            agent::fill(kind, &mut draft.clone(), key, &json!("100"), None)
                .unwrap_or_else(|error| panic!("{key}: {error}"));
        }
        let refused = agent::fill(
            kind,
            &mut draft.clone(),
            "frm1701q:txt64A",
            &json!("100"),
            None,
        )
        .unwrap_err();
        assert_eq!(refused, format!("`frm1701q:txt64A` {}", penalties::LOCK));
    }

    /// Judged on the draft as it stands when the key is written: on a 1702RT
    /// whose item 17i table holds two rows, its own boxes are the form's
    /// (OTHERS and the subtotal); written before the second row, they were
    /// the filer's.
    #[test]
    fn a_lock_is_judged_on_the_draft_as_the_writes_leave_it() {
        use crate::forms::form_1702rt::Form1702RTDraft;
        let kind = kind("1702RT");
        let profile = profile("1702RT", "Corporation");
        let fresh = computed(kind, Form1702RTDraft::new_from_profile(&profile, 2025, 12));
        let rows = [
            ("frm1702RT:txtPg3Sc1I17i.1C1", "Training"),
            ("frm1702RT:txtPg3Sc1I17i.1C2", "100"),
            ("frm1702RT:txtPg3Sc1I17i.2C1", "Seminars"),
            ("frm1702RT:txtPg3Sc1I17i.2C2", "200"),
        ];
        let item = "frm1702RT:txtPg3Sc1I17iC1";
        // The item first: open.
        let mut draft = fresh.clone();
        agent::fill(kind, &mut draft, item, &json!("Fees"), None).unwrap();
        for (key, value) in rows {
            agent::fill(kind, &mut draft, key, &json!(value), None).unwrap();
        }
        // The rows first: the second one folds the table, and the item is
        // refused on the draft they leave.
        let mut draft = fresh;
        for (key, value) in rows {
            agent::fill(kind, &mut draft, key, &json!(value), None).unwrap();
        }
        let refused = agent::fill(kind, &mut draft, item, &json!("Fees"), None).unwrap_err();
        assert_eq!(
            refused,
            format!("`{item}` {}", annual_corporate::RT_FOLDED_LOCK)
        );
    }

    /// The page's own writers never consult the draft layer: the page's UX
    /// governs those boxes (its Edit penalties opens a group).
    #[test]
    fn the_page_writers_are_not_held_to_the_draft_layer() {
        use crate::forms::form_0619e::Form0619EDraft;
        let kind = kind("0619E");
        let profile = profile("0619E", "Corporation");
        let draft = computed(kind, Form0619EDraft::new_from_profile(&profile, 2025, 6));
        let key = "frm0619E:txtTax17A";
        assert_eq!(
            page_lock(kind, &draft, key).as_deref(),
            Some(penalties::LOCK)
        );
        agent::fill_as_user(kind, &mut draft.clone(), key, &json!("100")).unwrap();
        agent::fill_profile_box_as_user(kind, &mut draft.clone(), key, &json!("100")).unwrap();
        agent::fill_host_owned(kind, &mut draft.clone(), key, &json!("100")).unwrap();
    }

    /// A box the base map draws a lock for keeps its refusal where the draft
    /// layer leaves the lock as it is, and one the page draws no lock for
    /// (here 2553's host-written printed ATC, which the layer shuts) keeps
    /// the assistant's own refusal: only a lock the layer decides reads the
    /// layer's reason. Asked alone, each key is the open's entry.
    #[test]
    fn only_a_lock_the_layer_decides_reads_its_reason() {
        use crate::forms::form_2553::Form2553Draft;
        let kind = kind("2553");
        let profile = profile("2553", "Corporation");
        let draft = computed(kind, Form2553Draft::new_from_profile(&profile, 2025, 2));
        let key = "frm2553:txt14B";
        assert_eq!(
            page_lock(kind, &draft, key).as_deref(),
            Some(dst_pt_vat::PRINTED_LOCK)
        );
        let refused =
            agent::fill(kind, &mut draft.clone(), key, &json!("OT010"), None).unwrap_err();
        assert!(
            refused.contains("written through the form's own editors"),
            "{refused}"
        );
        assert_eq!(
            draft_lock(kind, &draft, key, None).as_deref(),
            Some(dst_pt_vat::PRINTED_LOCK)
        );
        assert_eq!(
            draft_lock(
                kind,
                &draft,
                "frm2553:optQtr:_1",
                Some("cannot be filled here: x")
            )
            .as_deref(),
            Some(Lock::Identity.reason())
        );
        assert_eq!(
            draft_lock(kind, &draft, "frm2553:txt22A", None).as_deref(),
            Some(penalties::LOCK)
        );
        assert_eq!(draft_lock(kind, &draft, "frm2553:txt19A", None), None);
    }

    /// 2551Q Item 13 on a later quarter: locked to the filed Q1's election
    /// when this app holds that Q1 (same TIN, same taxable year), open
    /// otherwise.
    mod item_13_q1 {
        use serde_json::Value;

        use super::{draft_lock, kind, page_lock, profile};
        use crate::db::Database;
        use crate::forms::FilingStatus;
        use crate::forms::agent::draft_locks::item_13;
        use crate::forms::form_2551q::{Form2551QDraft, Item13Election};

        fn draft(year: u16, quarter: u8) -> Form2551QDraft {
            Form2551QDraft::new_from_profile(&profile("2551Qv2018", "Individual"), year, quarter)
        }

        /// Store a Q1 of `year` for `tin` with `status` and Item 13 `election`.
        fn store_q1(
            db: &Database,
            tin: &str,
            year: u16,
            status: FilingStatus,
            election: Item13Election,
        ) {
            store_q1_sent(db, tin, year, status, election, |_| {});
        }

        /// [`store_q1`] with the send's facts as `sent` sets them (a dry run, a relay receipt).
        fn store_q1_sent(
            db: &Database,
            tin: &str,
            year: u16,
            status: FilingStatus,
            election: Item13Election,
            sent: impl FnOnce(&mut Form2551QDraft),
        ) {
            let mut q1 = draft(year, 1);
            q1.tin = tin.to_string();
            q1.item_13_election = election;
            sent(&mut q1);
            let id = db.save_2551q_draft(&q1).unwrap();
            q1.id = Some(id);
            q1.status = status;
            let status = serde_json::to_value(&q1.status).unwrap();
            db.conn
                .execute(
                    "UPDATE form_drafts SET status = ?1, data_json = ?2 WHERE id = ?3",
                    rusqlite::params![
                        status.as_str().unwrap(),
                        serde_json::to_string(&q1).unwrap(),
                        id
                    ],
                )
                .unwrap();
        }

        /// The draft as the page opens it from `db`.
        fn opened(db: &Database, mut draft: Form2551QDraft) -> Value {
            item_13::refresh(db, &mut draft).unwrap();
            let mut json = serde_json::to_value(draft).unwrap();
            crate::forms::agent::compute(kind("2551Q"), &mut json).unwrap();
            json
        }

        fn tin() -> String {
            draft(2026, 3).tin
        }

        #[test]
        fn a_filed_q1_locks_a_later_quarters_item_13_to_its_election() {
            for status in [
                FilingStatus::Submitted,
                FilingStatus::Confirmed,
                FilingStatus::Paid,
            ] {
                let db = Database::open_in_memory_for_tests().unwrap();
                store_q1(
                    &db,
                    &tin(),
                    2026,
                    status.clone(),
                    Item13Election::EightPercent,
                );
                let mut q3 = draft(2026, 3);
                q3.item_13_election = Item13Election::Graduated;
                let q3 = opened(&db, q3);
                assert_eq!(q3["item_13_election"], "eight_percent", "{status:?}");
                for key in item_13::KEYS {
                    let lock = page_lock(kind("2551Q"), &q3, key)
                        .unwrap_or_else(|| panic!("{key} is locked on {status:?}"));
                    assert!(
                        lock.starts_with("Set on your filed Q1 return for 2026"),
                        "{lock}"
                    );
                    assert_eq!(
                        draft_lock(kind("2551Q"), &q3, key, None).as_deref(),
                        Some(lock.as_str())
                    );
                    assert_eq!(item_13::lock(kind("2551Q"), &q3, key), Some(lock));
                }
                // The locked answer validates on the later quarter.
                let typed: Form2551QDraft = serde_json::from_value(q3).unwrap();
                assert!(
                    crate::forms::FormValidator::validate(&typed)
                        .iter()
                        .all(|(field, _)| field != "item_13_election"),
                    "{status:?}"
                );
            }
        }

        #[test]
        fn a_q1_only_simulated_leaves_item_13_open_and_a_relay_sent_one_locks_it() {
            // The worker's dry run sent nothing: the later quarter keeps its own answer.
            let db = Database::open_in_memory_for_tests().unwrap();
            store_q1_sent(
                &db,
                &tin(),
                2026,
                FilingStatus::Submitted,
                Item13Election::EightPercent,
                |q1| q1.submitted_dry_run = true,
            );
            let mut q3 = draft(2026, 3);
            q3.item_13_election = Item13Election::Graduated;
            let q3 = opened(&db, q3);
            assert_eq!(q3["item_13_election"], "graduated");
            for key in item_13::KEYS {
                assert_eq!(page_lock(kind("2551Q"), &q3, key), None, "{key}");
            }
            // Sent through the relay with a receipt (no dry run): it reached the BIR and locks.
            let db = Database::open_in_memory_for_tests().unwrap();
            store_q1_sent(
                &db,
                &tin(),
                2026,
                FilingStatus::Submitted,
                Item13Election::EightPercent,
                |q1| q1.relay_receipt = Some("a.b.c".into()),
            );
            let mut q3 = draft(2026, 3);
            q3.item_13_election = Item13Election::Graduated;
            let q3 = opened(&db, q3);
            assert_eq!(q3["item_13_election"], "eight_percent");
            for key in item_13::KEYS {
                assert!(page_lock(kind("2551Q"), &q3, key).is_some(), "{key}");
            }
            // Confirmed and Paid lock even if a flag was left on them.
            for status in [FilingStatus::Confirmed, FilingStatus::Paid] {
                let db = Database::open_in_memory_for_tests().unwrap();
                store_q1_sent(
                    &db,
                    &tin(),
                    2026,
                    status.clone(),
                    Item13Election::EightPercent,
                    |q1| q1.submitted_dry_run = true,
                );
                let q3 = opened(&db, draft(2026, 3));
                assert_eq!(q3["item_13_election"], "eight_percent", "{status:?}");
            }
        }

        #[test]
        fn no_filed_q1_leaves_item_13_open() {
            let db = Database::open_in_memory_for_tests().unwrap();
            let mut q3 = draft(2026, 3);
            q3.item_13_election = Item13Election::Graduated;
            let q3 = opened(&db, q3);
            assert_eq!(q3["item_13_election"], "graduated");
            for key in item_13::KEYS {
                assert_eq!(page_lock(kind("2551Q"), &q3, key), None, "{key}");
            }
        }

        #[test]
        fn a_q1_that_is_only_a_draft_or_queued_leaves_item_13_open() {
            for status in [FilingStatus::Draft, FilingStatus::Queued] {
                let db = Database::open_in_memory_for_tests().unwrap();
                store_q1(
                    &db,
                    &tin(),
                    2026,
                    status.clone(),
                    Item13Election::EightPercent,
                );
                let mut q3 = draft(2026, 3);
                q3.item_13_election = Item13Election::Graduated;
                let q3 = opened(&db, q3);
                assert_eq!(q3["item_13_election"], "graduated", "{status:?}");
                for key in item_13::KEYS {
                    assert_eq!(page_lock(kind("2551Q"), &q3, key), None, "{status:?} {key}");
                }
            }
        }

        #[test]
        fn a_filed_q1_of_another_tin_or_year_leaves_item_13_open() {
            let db = Database::open_in_memory_for_tests().unwrap();
            store_q1(
                &db,
                "99988877700000",
                2026,
                FilingStatus::Submitted,
                Item13Election::EightPercent,
            );
            store_q1(
                &db,
                &tin(),
                2025,
                FilingStatus::Submitted,
                Item13Election::EightPercent,
            );
            let mut q3 = draft(2026, 3);
            q3.item_13_election = Item13Election::Graduated;
            let q3 = opened(&db, q3);
            assert_eq!(q3["item_13_election"], "graduated");
            for key in item_13::KEYS {
                assert_eq!(page_lock(kind("2551Q"), &q3, key), None, "{key}");
            }
        }

        #[test]
        fn the_q1_draft_itself_is_never_locked() {
            let db = Database::open_in_memory_for_tests().unwrap();
            store_q1(
                &db,
                &tin(),
                2026,
                FilingStatus::Submitted,
                Item13Election::EightPercent,
            );
            // An editable Q1 of the same year (an amendment's copy).
            let mut q1 = draft(2026, 1);
            q1.item_13_election = Item13Election::Graduated;
            let q1 = opened(&db, q1);
            assert_eq!(q1["item_13_election"], "graduated");
            for key in item_13::KEYS {
                assert_eq!(page_lock(kind("2551Q"), &q1, key), None, "{key}");
            }
        }
    }

    #[test]
    fn deleting_an_earlier_1601fq_schedule_row_keeps_the_notice_on_its_stable_row() {
        use super::schedule1_1601fq as gate;
        let row = |id: &str, amount: f64| {
            json!({
                "instance_id": id,
                "treaty_code": "NA",
                "atc_code": "NA",
                "income_payment": amount,
                "tax_rate": 10.0,
                "tax_withheld": amount,
            })
        };
        let first = row("row-00000000000000000001", 11.0);
        let second = row("row-00000000000000000002", 22.0);
        let mut before = json!({
            "tax_relief": "special_rate",
            "schedule1": [first, second],
        });
        // Delete row removes the first backing object; the second row moves
        // to index 0 but keeps the stable identity stored on its object.
        before["schedule1"].as_array_mut().unwrap().remove(0);
        assert_eq!(
            before["schedule1"][0]["instance_id"],
            "row-00000000000000000002"
        );
        assert_eq!(
            gate::row_index(&before, "row-00000000000000000002"),
            Some(0)
        );

        let mut after = before.clone();
        after["tax_relief"] = Value::Null;
        after["schedule1"] = json!([]);
        let clearances = gate::row_clearances(kind("1601FQ"), &before, &after);
        assert_eq!(clearances.len(), 1, "{clearances:?}");
        assert_eq!(clearances[0].id, "row-00000000000000000002");
        assert_eq!(
            clearances[0].gate,
            "1601fq_schedule1:row-00000000000000000002"
        );
        assert!(clearances[0].text.contains("row-00000000000000000002"));
    }
}

pub mod pending_notices {
    //! Pending gate-clearing notices, stored beside the once-note seen flags.
    //!
    //! A gate uses this by adding its dormant-before/cleared-after test to
    //! [`clearance`]; [`super::super::save_draft`] then records the notice in
    //! the same save operation that recomputes the draft, whoever initiated
    //! that save. The key is `notice_pending:<gate>:<draft id>` and the value
    //! carries the note text plus a unique clearing event. The page offers it
    //! until that event is recorded seen (after an offered open, or when the
    //! page's own inline clearing is saved); recording also removes the
    //! pending key, so a later import or edit that newly clears the same gate
    //! creates a fresh event and is said again.

    use serde_json::{Value, json};

    use crate::db::Database;
    use crate::forms::agent::AgentFormKind;

    pub const PENDING_PREFIX: &str = "notice_pending:";
    pub const SEEN_PREFIX: &str = "notice_seen:";

    fn pending_key(gate: &str, draft_id: i64) -> String {
        format!("{PENDING_PREFIX}{gate}:{draft_id}")
    }

    fn seen_key(gate: &str, draft_id: i64, event: &str) -> String {
        format!("{SEEN_PREFIX}{gate}:{draft_id}:{event}")
    }

    /// Parse a pending key into its gate and draft id.
    pub fn parse_pending_key(key: &str) -> Option<(String, i64)> {
        let rest = key.strip_prefix(PENDING_PREFIX)?;
        let (gate, id) = rest.rsplit_once(':')?;
        Some((gate.to_string(), id.parse().ok()?))
    }

    /// The key that records one clearing event seen.
    pub fn seen_key_for(gate: &str, draft_id: i64, event: &str) -> String {
        seen_key(gate, draft_id, event)
    }

    /// The clearings `save_draft` just made: every registered gate that
    /// turned a dormant value into its closed zero/blank state. Add later
    /// gates here; save then records them without each caller knowing which
    /// path opened or imported the return. Gate and text are owned strings so
    /// a row-keyed gate (1601FQ Schedule 1, for example) can name its row.
    pub fn clearance(kind: AgentFormKind, before: &Value, after: &Value) -> Vec<(String, String)> {
        collect_clearances(
            kind,
            before,
            after,
            &[
                item16_2551q_clearance,
                installment_1606_clearance,
                previous_payment_clearance,
                schedule1_1601fq_clearance,
            ],
        )
    }

    type ClearanceCheck = fn(AgentFormKind, &Value, &Value) -> Vec<(String, String)>;

    fn collect_clearances(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
        checks: &[ClearanceCheck],
    ) -> Vec<(String, String)> {
        checks
            .iter()
            .flat_map(|check| check(kind, before, after))
            .collect()
    }

    fn schedule1_1601fq_clearance(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
    ) -> Vec<(String, String)> {
        super::schedule1_1601fq::clearances(kind, before, after)
    }

    fn item16_2551q_clearance(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
    ) -> Vec<(String, String)> {
        if kind == AgentFormKind::Form2551Q
            && before["tax_paid_previous"].as_f64().unwrap_or(0.0).abs() >= 0.005
            && after["tax_paid_previous"].as_f64().unwrap_or(0.0).abs() < 0.005
            && let Some(text) = super::item16_2551q::cleared_note(after)
        {
            return vec![("2551q_item16".to_string(), text.to_string())];
        }
        Vec::new()
    }

    fn installment_1606_clearance(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
    ) -> Vec<(String, String)> {
        if kind.form_code() == "1606"
            && super::installment_1606::AMOUNT_FIELDS
                .iter()
                .any(|field| before[field].as_f64().unwrap_or(0.0).abs() >= 0.005)
            && super::installment_1606::AMOUNT_FIELDS
                .iter()
                .all(|field| after[field].as_f64().unwrap_or(0.0).abs() < 0.005)
            && super::installment_1606::lock_reason(after).is_some()
        {
            return vec![(
                "1606_installment".to_string(),
                super::installment_1606::CLEARED_NOTE.to_string(),
            )];
        }
        Vec::new()
    }

    fn previous_payment_clearance(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
    ) -> Vec<(String, String)> {
        let Some(spec) = super::previous_payment::spec(kind) else {
            return Vec::new();
        };
        if before[spec.path].as_f64().unwrap_or(0.0).abs() >= 0.005
            && after[spec.path].as_f64().unwrap_or(0.0).abs() < 0.005
            && (super::previous_payment::lock_reason(spec, after).is_some()
                || super::previous_payment::lock_reason(spec, before).is_some())
        {
            return vec![(
                super::previous_payment::gate_key(spec),
                spec.cleared.to_string(),
            )];
        }
        Vec::new()
    }

    fn event_name() -> String {
        let now = chrono::Utc::now();
        now.timestamp_nanos_opt()
            .unwrap_or_else(|| now.timestamp_millis())
            .to_string()
    }

    /// The settings rows one save writes for its clearings. One event names
    /// this save across every gate it cleared; a later clearing gets a later
    /// event, so an old seen key cannot acknowledge it.
    pub(crate) fn settings_writes(
        kind: AgentFormKind,
        before: &Value,
        after: &Value,
        draft_id: i64,
    ) -> Vec<(String, String)> {
        let clearances = clearance(kind, before, after);
        if clearances.is_empty() {
            return Vec::new();
        }
        let event = event_name();
        clearances
            .into_iter()
            .map(|(gate, text)| {
                let value = json!({"text": text, "event": event}).to_string();
                (pending_key(&gate, draft_id), value)
            })
            .collect()
    }

    #[derive(Debug, Clone)]
    pub struct PendingNotice {
        pub gate: String,
        pub event: String,
        pub text: String,
        pub seen_key: String,
    }

    /// Pending clearings of one draft, unreadable entries skipped with a
    /// warning so a bad app-state value never stops the return opening.
    pub fn for_draft(db: &Database, draft_id: i64) -> Vec<PendingNotice> {
        let rows = match db.settings_with_prefix(PENDING_PREFIX) {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!("reading pending gate notices failed: {error}");
                return Vec::new();
            }
        };
        rows.into_iter()
            .filter_map(|(key, stored)| {
                let rest = key.strip_prefix(PENDING_PREFIX)?;
                let (gate, id) = rest.rsplit_once(':')?;
                if id.parse::<i64>().ok()? != draft_id {
                    return None;
                }
                let value: Value = match serde_json::from_str(&stored) {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!("{key}: pending gate notice is not JSON: {error}");
                        return None;
                    }
                };
                let event = value["event"].as_str()?.to_string();
                let text = value["text"].as_str()?.to_string();
                Some(PendingNotice {
                    seen_key: seen_key(gate, draft_id, &event),
                    gate: gate.to_string(),
                    event,
                    text,
                })
            })
            .collect()
    }

    /// The pending key and event a seen key acknowledges, for
    /// `screen_form_notes_seen`.
    pub fn pending_event_for_seen(seen: &str) -> Option<(String, String)> {
        let rest = seen.strip_prefix(SEEN_PREFIX)?;
        let mut parts = rest.rsplitn(3, ':');
        let event = parts.next()?;
        let id = parts.next()?.parse::<i64>().ok()?;
        let gate = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        Some((pending_key(gate, id), event.to_string()))
    }

    #[cfg(test)]
    mod tests {
        use serde_json::{Value, json};

        use super::{clearance, collect_clearances};
        use crate::forms::agent::AgentFormKind;

        #[test]
        fn clearance_collects_every_gate_as_owned_row_keyed_strings() {
            fn first(_: AgentFormKind, _: &Value, _: &Value) -> Vec<(String, String)> {
                vec![("first".to_string(), "first cleared".to_string())]
            }
            fn second(_: AgentFormKind, _: &Value, _: &Value) -> Vec<(String, String)> {
                vec![("schedule1:row:7".to_string(), "second cleared".to_string())]
            }
            let draft = json!({});
            let both =
                collect_clearances(AgentFormKind::Form2551Q, &draft, &draft, &[first, second]);
            assert_eq!(
                both,
                vec![
                    ("first".to_string(), "first cleared".to_string()),
                    ("schedule1:row:7".to_string(), "second cleared".to_string()),
                ]
            );

            let before = json!({"tax_paid_previous": 42.0});
            let after = json!({
                "tax_paid_previous": 0.0,
                "is_amended": false,
                "item_13_election": "graduated",
            });
            let clearances: Vec<(String, String)> =
                clearance(AgentFormKind::Form2551Q, &before, &after);
            assert_eq!(clearances.len(), 1);
            assert_eq!(clearances[0].0, "2551q_item16");

            let seen = super::seen_key("schedule1:row:7", 9, "E1");
            assert_eq!(
                super::pending_event_for_seen(&seen),
                Some((super::pending_key("schedule1:row:7", 9), "E1".to_string()))
            );
        }

        #[test]
        fn previous_payment_clearance_covers_active_close_and_dormant_original() {
            use crate::forms::agent::draft_locks::previous_payment;
            use crate::forms::queueable::QueueableKind;

            let kind = AgentFormKind::Queueable(QueueableKind::Form1600PT);
            let spec = previous_payment::spec(kind).unwrap();
            let mut yes_with_amount = json!({"is_amended": true});
            yes_with_amount[spec.path] = json!(42.0);
            let mut no_without_amount = json!({"is_amended": false});
            no_without_amount[spec.path] = json!(0.0);
            let active = clearance(kind, &yes_with_amount, &no_without_amount);
            assert_eq!(
                active,
                vec![("1600pt_item19".to_string(), spec.cleared.to_string())]
            );

            let mut dormant_original = json!({"is_amended": false});
            dormant_original[spec.path] = json!(42.0);
            let mut amended_cleared = json!({"is_amended": true});
            amended_cleared[spec.path] = json!(0.0);
            let original = clearance(kind, &dormant_original, &amended_cleared);
            assert_eq!(
                original,
                vec![("1600pt_item19".to_string(), spec.cleared.to_string())]
            );

            let manual_clear_while_open = clearance(kind, &yes_with_amount, &amended_cleared);
            assert_eq!(manual_clear_while_open, Vec::new());
        }
    }
}
