//! Editor for BIR Form 1801 (January 2018), Estate Tax Return. Rust owns
//! every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1801`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1801::{
    FORM_1801_SCHEDULE_ROWS, Form1801Business, Form1801Draft, Form1801Frequency,
    Form1801Particular, Form1801RealProperty, Form1801Shares, Form1801Split,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice, choice_row,
    computed, field, format_tin, grid, money_text, section, yes_no,
};
use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

/// Plain inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of death", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("rdo", "Item 6 — RDO code", "e.g. 039"),
    ("name", "Item 7 — ESTATE OF", ""),
    (
        "address",
        "Item 8 — Residence of decedent at the time of death",
        "",
    ),
    ("admin_name", "Item 10 — Executor / administrator", ""),
    ("admin_tin", "Item 11 — Executor TIN", "000-000-000"),
    ("admin_branch", "Item 11 — Executor branch code", "00000"),
    ("contact", "Item 12 — Contact number", ""),
    ("email", "Item 13 — Email address", ""),
    ("relief", "Item 14A — Tax relief, specify", ""),
    ("freq_other", "Item 15D — Other frequency", ""),
    (
        "inst_year",
        "Item 21 — Installment due on or before (year)",
        "YYYY",
    ),
    (
        "other_ded_desc",
        "37C — Other special deduction, specify",
        "",
    ),
    ("ord_other_desc", "Schedule 5 — Others, specify", ""),
];

/// Amounts where blank is zero: (key, label).
const AMOUNTS: &[(&str, &str)] = &[
    ("std_ded", "37A — Standard deduction"),
    ("fam_ded", "37B — Family home"),
    ("other_ded", "37C — Other special deduction"),
    ("foreign", "19A — Foreign estate tax paid"),
    (
        "prev",
        "19B — Tax paid in return previously filed (amended only)",
    ),
    ("installment", "21 — Portion allowed for installment"),
    ("surcharge", "23A — Surcharge"),
    ("interest", "23B — Interest"),
    ("compromise", "23C — Compromise"),
];

/// Schedule 5 rows: (key, label).
const ORDINARY: [(&str, &str); 7] = [
    ("estate", "Claims against the estate"),
    ("insolvent", "Claims against insolvent persons"),
    ("unpaid", "Unpaid mortgages, taxes and casualty losses"),
    ("losses", "Losses during settlement of the estate"),
    (
        "vanishing",
        "Property previously taxed (vanishing deduction)",
    ),
    ("transfer", "Transfer for public use"),
    ("others", "Others"),
];

const REAL: [(&str, &str); 8] = [
    ("oct", "OCT/TCT/CCT No."),
    ("td", "Tax declaration no."),
    ("loc", "Location"),
    ("lot", "Lot / improvement"),
    ("area", "Area"),
    (
        "class",
        "Classification (RR, CR, CL, GL, A, X, RC, CC, PS, GP, I, APD)",
    ),
    ("fmv", "FMV per TD"),
    ("zonal", "FMV per BIR (zonal value)"),
];
const SHARES: [(&str, &str); 5] = [
    ("corp", "Name of corporation"),
    ("listing", "Listed / Not Listed"),
    ("cert", "Stock certificate no."),
    ("shares", "No. of shares"),
    ("per", "FMV / book value per share"),
];

fn k(schedule: &str, row: usize, part: &str) -> String {
    format!("{schedule}_{row}_{part}")
}

fn split_text(split: &Form1801Split, part: &str) -> String {
    match part {
        "exc" => money_text(split.exclusive),
        _ => money_text(split.conjugal),
    }
}

pub struct Form1801View {
    draft: Form1801Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
}

impl EventEmitter<QueueableFormEvent> for Form1801View {}

impl Form1801View {
    fn ordinary_split(d: &Form1801Draft, key: &str) -> Form1801Split {
        let o = &d.ordinary_deductions;
        match key {
            "estate" => o.claims_against_estate.clone(),
            "insolvent" => o.claims_against_insolvent.clone(),
            "unpaid" => o.unpaid_mortgages_taxes_losses.clone(),
            "losses" => o.settlement_losses.clone(),
            "vanishing" => o.vanishing_deduction.clone(),
            "transfer" => o.transfer_for_public_use.clone(),
            _ => o.others.clone(),
        }
    }

    fn row_texts(d: &Form1801Draft) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for row in 0..FORM_1801_SCHEDULE_ROWS {
            for (schedule, rows) in [("s1", &d.real_properties), ("s1a", &d.family_homes)] {
                let r = rows.get(row).cloned().unwrap_or_default();
                for (part, value) in [
                    ("oct", r.title_number),
                    ("td", r.tax_declaration_number),
                    ("loc", r.location),
                    ("lot", r.lot_or_improvement),
                    ("area", r.area),
                    ("class", r.classification),
                    ("fmv", r.fmv_per_tax_declaration),
                    ("zonal", r.fmv_per_zonal_value),
                    ("exc", money_text(r.value.exclusive)),
                    ("conj", money_text(r.value.conjugal)),
                ] {
                    out.push((k(schedule, row, part), value));
                }
            }
            let s = d.shares.get(row).cloned().unwrap_or_default();
            for (part, value) in [
                ("corp", s.corporation),
                ("listing", s.listing),
                ("cert", s.certificate_number),
                ("shares", s.number_of_shares),
                ("per", s.value_per_share),
                ("exc", money_text(s.value.exclusive)),
                ("conj", money_text(s.value.conjugal)),
            ] {
                out.push((k("s2", row, part), value));
            }
            for (schedule, rows) in [("s2a", &d.other_personal), ("s3", &d.taxable_transfers)] {
                let p = rows.get(row).cloned().unwrap_or_default();
                out.push((k(schedule, row, "desc"), p.particulars));
                out.push((k(schedule, row, "exc"), money_text(p.value.exclusive)));
                out.push((k(schedule, row, "conj"), money_text(p.value.conjugal)));
            }
            let b = d.business_interests.get(row).cloned().unwrap_or_default();
            for (part, value) in [
                ("name", b.name),
                ("addr", b.address),
                ("rdo", b.rdo_code),
                ("exc", money_text(b.value.exclusive)),
                ("conj", money_text(b.value.conjugal)),
            ] {
                out.push((k("s4", row, part), value));
            }
        }
        for (key, _) in ORDINARY {
            let split = Self::ordinary_split(d, key);
            out.push((format!("s5_{key}_exc"), split_text(&split, "exc")));
            out.push((format!("s5_{key}_conj"), split_text(&split, "conj")));
        }
        out
    }

    fn text_value(d: &Form1801Draft, key: &str) -> String {
        match key {
            "month" => format!("{:02}", d.death_month),
            "day" => format!("{:02}", d.death_day),
            "year" => d.death_year.to_string(),
            "sheets" => d.number_of_attached_sheets.to_string(),
            "rdo" => d.rdo_code.clone(),
            "name" => d.estate_name.clone(),
            "address" => d.residence_address.clone(),
            "admin_name" => d.administrator_name.clone(),
            "admin_tin" => d.administrator_tin.clone(),
            "admin_branch" => d.administrator_branch_code.clone(),
            "contact" => d.contact_number.clone(),
            "email" => d.email.clone(),
            "relief" => d.tax_relief.clone().unwrap_or_default(),
            "freq_other" => d.installment_frequency_other.clone(),
            "inst_year" => d.installment_year.clone(),
            "other_ded_desc" => d.other_special_deduction_description.clone(),
            "ord_other_desc" => d.ordinary_deductions.others_description.clone(),
            "std_ded" => money_text(d.standard_deduction),
            "fam_ded" => money_text(d.family_home_deduction),
            "other_ded" => money_text(d.other_special_deduction),
            "foreign" => money_text(d.foreign_estate_tax),
            "prev" => money_text(d.tax_paid_previous),
            "installment" => money_text(d.installment_portion),
            "surcharge" => money_text(d.surcharge),
            "interest" => money_text(d.interest),
            "compromise" => money_text(d.compromise),
            _ => Self::row_texts(d)
                .into_iter()
                .find(|(key2, _)| key2 == key)
                .map(|(_, v)| v)
                .unwrap_or_default(),
        }
    }

    fn input_specs(d: &Form1801Draft) -> Vec<InputSpec> {
        let mut specs: Vec<InputSpec> = TEXT_INPUTS
            .iter()
            .map(|(key, _, p)| (key.to_string(), p.to_string(), Self::text_value(d, key)))
            .collect();
        for (key, _) in AMOUNTS {
            specs.push((key.to_string(), "0.00".into(), Self::text_value(d, key)));
        }
        for (key, value) in Self::row_texts(d) {
            specs.push((key, String::new(), value));
        }
        specs
    }

    fn label(key: &str) -> &'static str {
        TEXT_INPUTS
            .iter()
            .map(|(a, l, _)| (*a, *l))
            .chain(AMOUNTS.iter().copied())
            .find(|(a, _)| *a == key)
            .map(|(_, l)| l)
            .unwrap_or("")
    }

    fn f(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        field(
            &self.kit,
            key,
            Self::label(key),
            layout,
            disabled || !self.draft.lifecycle.is_editable(),
        )
    }

    fn cell(&self, key: String, caption: String, layout: Layout) -> AnyElement {
        field(
            &self.kit,
            &key,
            &caption,
            layout,
            !self.draft.lifecycle.is_editable(),
        )
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = choice_row(
            "Item 2 — Amended return?",
            vec![
                choice("1801_amended_yes", "Yes", d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(true))
                    },
                )),
                choice("1801_amended_no", "No", !d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(false))
                    },
                )),
            ],
        );
        let relief = choice_row(
            "Item 14 — Availing of tax relief under a Special Law / International Tax Treaty?",
            vec![
                choice("1801_relief_yes", "Yes", d.tax_relief.is_some(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| {
                            if d.tax_relief.is_none() {
                                d.tax_relief = Some(String::new());
                            }
                        })
                    }),
                ),
                choice("1801_relief_no", "No", d.tax_relief.is_none(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.tax_relief = None)
                    }),
                ),
            ],
        );
        let tin = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(kit::label("Item 5 — TIN (profile)"))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format_tin(&d.tin)),
            )
            .into_any_element();
        let mut children = vec![
            self.f("month", layout, false),
            self.f("day", layout, false),
            self.f("year", layout, false),
            amended,
            self.f("sheets", layout, false),
            kit::computed_text("Item 4 — ATC", "ES 010".to_string(), cx),
            tin,
            self.f("rdo", layout, false),
            self.f("name", layout, false),
            self.f("address", layout, false),
            yes_no::<Self>(
                "1801_nra",
                "Item 9 — Non-resident alien?",
                d.non_resident_alien,
                !editable,
                cx,
                |d, yes| d.non_resident_alien = Some(yes),
            ),
            self.f("admin_name", layout, false),
            self.f("admin_tin", layout, false),
            self.f("admin_branch", layout, false),
            self.f("contact", layout, false),
            self.f("email", layout, false),
            relief,
        ];
        if d.tax_relief.is_some() {
            children.push(self.f("relief", layout, false));
        }
        children.push(yes_no::<Self>(
            "1801_15a",
            "15A — Has an extension to file the return been granted?",
            Some(d.extension_to_file),
            !editable,
            cx,
            |d, yes| d.extension_to_file = yes,
        ));
        children.push(yes_no::<Self>(
            "1801_15b",
            "15B — Has the estate been settled judicially?",
            Some(d.settled_judicially),
            !editable,
            cx,
            |d, yes| d.settled_judicially = yes,
        ));
        children.push(yes_no::<Self>(
            "1801_15c",
            "15C — Has an extension to pay the tax been granted?",
            Some(d.extension_to_pay),
            !editable,
            cx,
            |d, yes| d.extension_to_pay = yes,
        ));
        children.push(yes_no::<Self>(
            "1801_15d",
            "15D — Has an installment payment been granted?",
            Some(d.installment_granted),
            !editable,
            cx,
            |d, yes| d.set_installment_granted(yes),
        ));
        if d.installment_granted {
            let freqs: Vec<_> = [
                (Form1801Frequency::Monthly, "Monthly"),
                (Form1801Frequency::Quarterly, "Quarterly"),
                (Form1801Frequency::SemiAnnual, "Semi-Annual"),
                (Form1801Frequency::Others, "Others"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (freq, caption))| {
                choice(
                    ("1801_freq", index),
                    caption,
                    d.installment_frequency == Some(freq),
                    !editable,
                )
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    kit::edit(this, cx, |d| d.installment_frequency = Some(freq))
                }))
            })
            .collect();
            children.push(choice_row("15D — Frequency of payment", freqs));
            if d.installment_frequency == Some(Form1801Frequency::Others) {
                children.push(self.f("freq_other", layout, false));
            }
        }
        section(
            "Part I — Taxpayer information",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children = Vec::new();
        let total = |name: &str| d.schedule_totals.get(name).cloned().unwrap_or_default();
        let totals = |name: &str, caption: &str| -> AnyElement {
            let t = total(name);
            kit::computed_text(
                caption,
                format!(
                    "{} / {}",
                    bir_core::official_xml::official_amount(t.exclusive),
                    bir_core::official_xml::official_amount(t.conjugal)
                ),
                cx,
            )
        };
        for (schedule, title, total_name) in [
            ("s1", "Schedule 1 — Real properties", "sched1"),
            ("s1a", "Schedule 1A — Family home", "sched1A"),
        ] {
            children.push(kit::label(title).into_any_element());
            for row in 0..FORM_1801_SCHEDULE_ROWS {
                let mut cells: Vec<AnyElement> = REAL
                    .iter()
                    .filter(|(part, _)| schedule == "s1" || *part != "lot")
                    .map(|(part, caption)| {
                        self.cell(
                            k(schedule, row, part),
                            format!("{} — {caption}", row + 1),
                            layout,
                        )
                    })
                    .collect();
                cells.push(self.cell(
                    k(schedule, row, "exc"),
                    format!("{} — Exclusive", row + 1),
                    layout,
                ));
                cells.push(self.cell(
                    k(schedule, row, "conj"),
                    format!("{} — Conjugal", row + 1),
                    layout,
                ));
                children.push(grid(layout, cells));
            }
            children.push(totals(total_name, "Total exclusive / conjugal"));
        }
        children.push(kit::label("Schedule 2 — Shares of stock").into_any_element());
        for row in 0..FORM_1801_SCHEDULE_ROWS {
            let mut cells: Vec<AnyElement> = SHARES
                .iter()
                .map(|(part, caption)| {
                    self.cell(
                        k("s2", row, part),
                        format!("{} — {caption}", row + 1),
                        layout,
                    )
                })
                .collect();
            cells.push(self.cell(
                k("s2", row, "exc"),
                format!("{} — Exclusive", row + 1),
                layout,
            ));
            cells.push(self.cell(
                k("s2", row, "conj"),
                format!("{} — Conjugal", row + 1),
                layout,
            ));
            children.push(grid(layout, cells));
        }
        children.push(totals("sched2", "Total exclusive / conjugal"));
        for (schedule, title, total_name) in [
            ("s2a", "Schedule 2A — Other personal properties", "sched2A"),
            ("s3", "Schedule 3 — Taxable transfers", "sched3"),
        ] {
            children.push(kit::label(title).into_any_element());
            for row in 0..FORM_1801_SCHEDULE_ROWS {
                children.push(grid(
                    layout,
                    vec![
                        self.cell(
                            k(schedule, row, "desc"),
                            format!("{} — Particulars", row + 1),
                            layout,
                        ),
                        self.cell(
                            k(schedule, row, "exc"),
                            format!("{} — Exclusive", row + 1),
                            layout,
                        ),
                        self.cell(
                            k(schedule, row, "conj"),
                            format!("{} — Conjugal", row + 1),
                            layout,
                        ),
                    ],
                ));
            }
            children.push(totals(total_name, "Total exclusive / conjugal"));
        }
        children.push(kit::label("Schedule 4 — Business interest").into_any_element());
        for row in 0..FORM_1801_SCHEDULE_ROWS {
            let cells = [
                ("name", "Trade / business name"),
                ("addr", "Registered address"),
                ("rdo", "RDO code"),
                ("exc", "Exclusive"),
                ("conj", "Conjugal"),
            ]
            .iter()
            .map(|(part, caption)| {
                self.cell(
                    k("s4", row, part),
                    format!("{} — {caption}", row + 1),
                    layout,
                )
            })
            .collect();
            children.push(grid(layout, cells));
        }
        children.push(totals("sched4", "Total exclusive / conjugal"));
        children.push(kit::label("Schedule 5 — Ordinary deductions").into_any_element());
        let mut ordinary = Vec::new();
        for (key, caption) in ORDINARY {
            ordinary.push(self.cell(
                format!("s5_{key}_exc"),
                format!("{caption} — exclusive"),
                layout,
            ));
            ordinary.push(self.cell(
                format!("s5_{key}_conj"),
                format!("{caption} — conjugal"),
                layout,
            ));
        }
        ordinary.push(self.f("ord_other_desc", layout, false));
        children.push(grid(layout, ordinary));
        children.push(totals("sched5", "Total exclusive / conjugal"));
        section("Part V — Schedules", children, cx)
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let cols = |caption: &str, c: bir_core::forms::form_1801::Form1801Columns| {
            kit::computed_text(
                caption,
                format!(
                    "{} / {} / {}",
                    bir_core::official_xml::official_amount(c.exclusive),
                    bir_core::official_xml::official_amount(c.conjugal),
                    bir_core::official_xml::official_amount(c.total)
                ),
                cx,
            )
        };
        let (payable, first, total) = d.payables();
        let shown = |v: f64| {
            if v.is_nan() {
                "NaN (refused)".to_string()
            } else {
                bir_core::official_xml::official_amount(v)
            }
        };
        let children = vec![
            cols("29 — Real properties (A / B / C)", d.real_property),
            cols("30 — Family home", d.family_home),
            cols("31 — Personal properties", d.personal_property),
            cols("32 — Taxable transfers", d.taxable_transfer),
            cols("33 — Business interest", d.business_interest),
            cols("34 — Gross estate", d.gross_estate),
            cols("35 — Ordinary deductions", d.ordinary_deduction),
            cols(
                "36 — Estate after ordinary deductions",
                d.estate_after_deductions,
            ),
            self.f("std_ded", layout, false),
            self.f("fam_ded", layout, false),
            self.f("other_ded_desc", layout, false),
            self.f("other_ded", layout, false),
            computed(
                "37D — Total special deductions",
                d.total_special_deductions,
                cx,
            ),
            computed("38 — Net estate", d.net_estate, cx),
            computed("39 — Share of surviving spouse", d.spouse_share, cx),
            computed("40 / 16 — Net taxable estate", d.net_taxable_estate, cx),
            kit::computed_text("18 — Estate tax due", shown(d.estate_tax_due()), cx),
            self.f("foreign", layout, false),
            self.f("prev", layout, !d.is_amended),
            computed("19C — Total credits", d.total_credits, cx),
            kit::computed_text("20 — Tax payable", shown(payable), cx),
            self.f("inst_year", layout, false),
            self.f("installment", layout, false),
            kit::computed_text("22 — Tax payable (1st installment)", shown(first), cx),
            self.f("surcharge", layout, false),
            self.f("interest", layout, false),
            self.f("compromise", layout, false),
            computed("23D — Total penalties", d.total_penalties, cx),
            kit::computed_text("24 — Total amount payable", shown(total), cx),
        ];
        section(
            "Part II / IV — Computation of tax",
            vec![grid(layout, children)],
            cx,
        )
    }
}

impl KitView for Form1801View {
    type Draft = Form1801Draft;
    const SLUG: &'static str = "1801";
    const CODE: &'static str = "1801";

    fn kit(&self) -> &EditorState {
        &self.kit
    }
    fn kit_mut(&mut self) -> &mut EditorState {
        &mut self.kit
    }
    fn draft(&self) -> &Form1801Draft {
        &self.draft
    }
    fn draft_mut(&mut self) -> &mut Form1801Draft {
        &mut self.draft
    }
    fn db(&self) -> Arc<Mutex<Database>> {
        Arc::clone(&self.db)
    }
    fn set_draft_id(&mut self, id: i64) {
        self.draft.id = Some(id);
    }
    fn initial_text(&self, key: &str) -> String {
        Self::text_value(&self.draft, key)
    }

    fn read_inputs(&mut self, cx: &mut Context<Self>) -> Vec<(String, String)> {
        let mut r = InputReader::new(&self.kit, cx);
        let d = &mut self.draft;
        if let Some(v) = r.whole::<u8>("month", "Item 1 month", 99) {
            d.death_month = v;
        }
        if let Some(v) = r.whole::<u8>("day", "Item 1 day", 99) {
            d.death_day = v;
        }
        if let Some(v) = r.whole::<u16>("year", "Item 1 year", 9999) {
            d.death_year = v;
        }
        if let Some(v) = r.whole::<u16>("sheets", "Item 3", 999) {
            d.number_of_attached_sheets = v;
        }
        d.rdo_code = r.trimmed("rdo").to_uppercase();
        d.estate_name = r.text("name");
        d.residence_address = r.text("address");
        d.administrator_name = r.text("admin_name");
        d.administrator_tin = r.trimmed("admin_tin");
        d.administrator_branch_code = r.trimmed("admin_branch");
        d.contact_number = r.trimmed("contact");
        d.email = r.trimmed("email");
        if d.tax_relief.is_some() {
            d.tax_relief = Some(r.text("relief"));
        }
        d.installment_frequency_other = r.text("freq_other");
        d.installment_year = r.trimmed("inst_year");
        d.other_special_deduction_description = r.text("other_ded_desc");
        d.ordinary_deductions.others_description = r.text("ord_other_desc");
        for (key, label) in AMOUNTS {
            if let Some(value) = r.amount(key, label) {
                match *key {
                    "std_ded" => d.standard_deduction = value,
                    "fam_ded" => d.family_home_deduction = value,
                    "other_ded" => d.other_special_deduction = value,
                    "foreign" => d.foreign_estate_tax = value,
                    "prev" => d.tax_paid_previous = value,
                    "installment" => d.installment_portion = value,
                    "surcharge" => d.surcharge = value,
                    "interest" => d.interest = value,
                    _ => d.compromise = value,
                }
            }
        }
        let split = |schedule: &str, row: usize, r: &mut InputReader<'_>| Form1801Split {
            exclusive: r
                .amount(&k(schedule, row, "exc"), "Exclusive")
                .unwrap_or(0.0),
            conjugal: r
                .amount(&k(schedule, row, "conj"), "Conjugal")
                .unwrap_or(0.0),
        };
        let mut real = Vec::new();
        let mut family = Vec::new();
        let mut shares = Vec::new();
        let mut other = Vec::new();
        let mut transfers = Vec::new();
        let mut business = Vec::new();
        for row in 0..FORM_1801_SCHEDULE_ROWS {
            for (schedule, list) in [("s1", &mut real), ("s1a", &mut family)] {
                list.push(Form1801RealProperty {
                    title_number: r.text(&k(schedule, row, "oct")),
                    tax_declaration_number: r.text(&k(schedule, row, "td")),
                    location: r.text(&k(schedule, row, "loc")),
                    lot_or_improvement: r.text(&k(schedule, row, "lot")),
                    area: r.text(&k(schedule, row, "area")),
                    classification: r.trimmed(&k(schedule, row, "class")).to_uppercase(),
                    fmv_per_tax_declaration: r.text(&k(schedule, row, "fmv")),
                    fmv_per_zonal_value: r.text(&k(schedule, row, "zonal")),
                    value: split(schedule, row, &mut r),
                });
            }
            shares.push(Form1801Shares {
                corporation: r.text(&k("s2", row, "corp")),
                listing: r.trimmed(&k("s2", row, "listing")),
                certificate_number: r.text(&k("s2", row, "cert")),
                number_of_shares: r.text(&k("s2", row, "shares")),
                value_per_share: r.text(&k("s2", row, "per")),
                value: split("s2", row, &mut r),
            });
            for (schedule, list) in [("s2a", &mut other), ("s3", &mut transfers)] {
                list.push(Form1801Particular {
                    particulars: r.text(&k(schedule, row, "desc")),
                    value: split(schedule, row, &mut r),
                });
            }
            business.push(Form1801Business {
                name: r.text(&k("s4", row, "name")),
                address: r.text(&k("s4", row, "addr")),
                rdo_code: r.trimmed(&k("s4", row, "rdo")),
                value: split("s4", row, &mut r),
            });
        }
        let empty_split = |s: &Form1801Split| s.exclusive == 0.0 && s.conjugal == 0.0;
        let blank_property = |x: &Form1801RealProperty| {
            empty_split(&x.value)
                && x.title_number.trim().is_empty()
                && x.location.trim().is_empty()
                && x.area.trim().is_empty()
                && x.classification.is_empty()
                && x.tax_declaration_number.trim().is_empty()
                && x.lot_or_improvement.trim().is_empty()
                && x.fmv_per_tax_declaration.trim().is_empty()
                && x.fmv_per_zonal_value.trim().is_empty()
        };
        for list in [&mut real, &mut family] {
            while list.last().is_some_and(blank_property) {
                list.pop();
            }
        }
        while shares.last().is_some_and(|x: &Form1801Shares| {
            empty_split(&x.value)
                && x.corporation.trim().is_empty()
                && x.listing.is_empty()
                && x.certificate_number.trim().is_empty()
                && x.number_of_shares.trim().is_empty()
                && x.value_per_share.trim().is_empty()
        }) {
            shares.pop();
        }
        for list in [&mut other, &mut transfers] {
            while list.last().is_some_and(|x: &Form1801Particular| {
                empty_split(&x.value) && x.particulars.trim().is_empty()
            }) {
                list.pop();
            }
        }
        while business.last().is_some_and(|x: &Form1801Business| {
            empty_split(&x.value)
                && x.name.trim().is_empty()
                && x.address.trim().is_empty()
                && (x.rdo_code.is_empty() || x.rdo_code == "000")
        }) {
            business.pop();
        }
        d.real_properties = real;
        d.family_homes = family;
        d.shares = shares;
        d.other_personal = other;
        d.taxable_transfers = transfers;
        d.business_interests = business;
        for (key, _) in ORDINARY {
            let value = Form1801Split {
                exclusive: r
                    .amount(&format!("s5_{key}_exc"), "Schedule 5")
                    .unwrap_or(0.0),
                conjugal: r
                    .amount(&format!("s5_{key}_conj"), "Schedule 5")
                    .unwrap_or(0.0),
            };
            let o = &mut d.ordinary_deductions;
            match key {
                "estate" => o.claims_against_estate = value,
                "insolvent" => o.claims_against_insolvent = value,
                "unpaid" => o.unpaid_mortgages_taxes_losses = value,
                "losses" => o.settlement_losses = value,
                "vanishing" => o.vanishing_deduction = value,
                "transfer" => o.transfer_for_public_use = value,
                _ => o.others = value,
            }
        }
        r.errors
    }
}

impl QueueableFormView for Form1801View {
    type Draft = Form1801Draft;

    fn new(
        draft: Form1801Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let kit = EditorState::new(Self::input_specs(&draft), errors, window, cx);
        Self { draft, db, kit }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1801Draft {
        Form1801Draft::new_from_profile(profile, year, u32::from(period.max(1)))
    }
}

impl FormViewTrait for Form1801View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1801"
    }
    fn form_subtitle(&self) -> &'static str {
        "Estate Tax Return"
    }
    fn form_version(&self) -> &'static str {
        "January 2018 (ENCS)"
    }
    fn current_status(&self) -> FilingStatus {
        self.draft.lifecycle.status.clone()
    }
    fn submitted_at(&self) -> Option<&str> {
        self.draft.lifecycle.submitted_at.as_deref()
    }
    fn confirmed_at(&self) -> Option<&str> {
        self.draft.lifecycle.confirmed_at.as_deref()
    }
    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::save(self, window, cx);
    }
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::queue(self, window, cx);
    }
    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.kit.status_message =
            Some("1801 payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::cancel(self, window, cx);
    }
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        kit::preview(self, "1801-2018", cx);
    }
}

impl Render for Form1801View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let sections = vec![
            self.render_part_one(layout, cx),
            self.render_schedules(layout, cx),
            self.render_computation(layout, cx),
        ];
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{AMOUNTS, Form1801View, TEXT_INPUTS};

    #[test]
    fn every_input_has_a_label() {
        for (key, _, _) in TEXT_INPUTS {
            assert!(!Form1801View::label(key).is_empty(), "{key}");
        }
        for (key, _) in AMOUNTS {
            assert!(!Form1801View::label(key).is_empty(), "{key}");
        }
    }
}
