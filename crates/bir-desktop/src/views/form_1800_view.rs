//! Editor for BIR Form 1800 (January 2018), Donor's Tax Return. Rust owns
//! every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1800`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1800::{
    FORM_1800_DEDUCTIONS, FORM_1800_DONEES, FORM_1800_SCHEDULE_ROWS, Form1800Deduction,
    Form1800Donee, Form1800Draft, Form1800PersonalProperty, Form1800RealProperty,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice, choice_row,
    computed, field, format_tin, grid, money_text, section,
};
use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

/// Plain inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of donation", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("rdo", "Item 6 — RDO code", "e.g. 039"),
    ("name", "Item 7 — Donor's name", ""),
    ("reg_address", "Item 8 — Registered address", ""),
    ("zip", "Item 8A — Zip code", ""),
    ("res_address", "Item 9 — Residence address", ""),
    ("res_zip", "Item 9A — Zip code", ""),
    ("contact", "Item 10 — Contact number", ""),
    ("email", "Item 11 — Email address", ""),
    ("lob", "Line of business (profile)", ""),
    ("relief", "Item 13A — Tax relief, specify", ""),
];

/// Amounts where blank is zero: (key, label).
const AMOUNTS: &[(&str, &str)] = &[
    (
        "prior_net",
        "35 — Total prior net gifts during the calendar year",
    ),
    ("credit_a", "17A — Payments for prior gifts during the year"),
    ("credit_b", "17B — Foreign donor's tax paid"),
    (
        "credit_c",
        "17C — Tax paid in previously filed return (amended only)",
    ),
    ("surcharge", "19A — Surcharge"),
    ("interest", "19B — Interest"),
    ("compromise", "19C — Compromise"),
];

const LETTERS: [&str; 5] = ["A", "B", "C", "D", "E"];
const REAL_FIELDS: [(&str, &str); 8] = [
    ("oct", "OCT/TCT/CCT No."),
    ("td", "Tax declaration no."),
    ("loc", "Location"),
    ("lot", "Lot / improvement"),
    ("class", "Classification (RR, CR, CL, …)"),
    ("area", "Area"),
    ("ftd", "FMV per tax declaration"),
    ("zonal", "FMV per BIR (zonal value)"),
];

fn key(prefix: &str, row: usize, part: &str) -> String {
    format!("{prefix}{row}_{part}")
}

pub struct Form1800View {
    draft: Form1800Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
}

impl EventEmitter<QueueableFormEvent> for Form1800View {}

impl Form1800View {
    fn text_value(d: &Form1800Draft, k: &str) -> String {
        match k {
            "month" => format!("{:02}", d.donation_month),
            "day" => format!("{:02}", d.donation_day),
            "year" => d.donation_year.to_string(),
            "sheets" => d.number_of_attached_sheets.to_string(),
            "rdo" => d.rdo_code.clone(),
            "name" => d.donor_name.clone(),
            "reg_address" => d.registered_address.clone(),
            "zip" => d.zip_code.clone(),
            "res_address" => d.residence_address.clone(),
            "res_zip" => d.residence_zip_code.clone(),
            "contact" => d.contact_number.clone(),
            "email" => d.email.clone(),
            "lob" => d.line_of_business.clone(),
            "relief" => d.tax_relief.clone().unwrap_or_default(),
            "prior_net" => money_text(d.prior_net_gifts),
            "credit_a" => money_text(d.prior_gift_payments),
            "credit_b" => money_text(d.foreign_tax_paid),
            "credit_c" => money_text(d.tax_paid_previous),
            "surcharge" => money_text(d.surcharge),
            "interest" => money_text(d.interest),
            "compromise" => money_text(d.compromise),
            _ => {
                for row in 0..FORM_1800_DONEES {
                    let donee = d.donees.get(row).cloned().unwrap_or_default();
                    if k == key("donee", row, "name") {
                        return donee.name;
                    }
                    if k == key("donee", row, "tin") {
                        return donee.tin;
                    }
                }
                for row in 0..FORM_1800_SCHEDULE_ROWS {
                    let a = d.personal_properties.get(row).cloned().unwrap_or_default();
                    if k == key("pp", row, "desc") {
                        return a.particulars;
                    }
                    if k == key("pp", row, "fmv") {
                        return money_text(a.fair_market_value);
                    }
                    let b = d.real_properties.get(row).cloned().unwrap_or_default();
                    let texts = [
                        ("oct", b.title_number.clone()),
                        ("td", b.tax_declaration_number.clone()),
                        ("loc", b.location.clone()),
                        ("lot", b.lot_or_improvement.clone()),
                        ("class", b.classification.clone()),
                        ("area", b.area.clone()),
                        ("ftd", b.fmv_per_tax_declaration.clone()),
                        ("zonal", b.fmv_per_zonal_value.clone()),
                        ("fmv", money_text(b.fair_market_value)),
                    ];
                    for (part, value) in texts {
                        if k == key("rp", row, part) {
                            return value;
                        }
                    }
                }
                for row in 0..FORM_1800_DEDUCTIONS {
                    let ded = d.deductions.get(row).cloned().unwrap_or_default();
                    if k == key("ded", row, "title") {
                        return ded.title;
                    }
                    if k == key("ded", row, "amount") {
                        return money_text(ded.amount);
                    }
                }
                String::new()
            }
        }
    }

    fn input_specs(d: &Form1800Draft) -> Vec<InputSpec> {
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(k, _, p)| (k.to_string(), p.to_string()))
            .collect();
        keys.extend(
            AMOUNTS
                .iter()
                .map(|(k, _)| (k.to_string(), "0.00".to_string())),
        );
        for row in 0..FORM_1800_DONEES {
            keys.push((key("donee", row, "name"), String::new()));
            keys.push((key("donee", row, "tin"), "digits".to_string()));
        }
        for row in 0..FORM_1800_SCHEDULE_ROWS {
            keys.push((key("pp", row, "desc"), String::new()));
            keys.push((key("pp", row, "fmv"), "0.00".to_string()));
            for (part, _) in REAL_FIELDS {
                keys.push((key("rp", row, part), String::new()));
            }
            keys.push((key("rp", row, "fmv"), "0.00".to_string()));
        }
        for row in 0..FORM_1800_DEDUCTIONS {
            keys.push((key("ded", row, "title"), String::new()));
            keys.push((key("ded", row, "amount"), "0.00".to_string()));
        }
        keys.into_iter()
            .map(|(k, placeholder)| {
                let value = Self::text_value(d, &k);
                (k, placeholder, value)
            })
            .collect()
    }

    fn label(k: &str) -> &'static str {
        TEXT_INPUTS
            .iter()
            .map(|(a, l, _)| (*a, *l))
            .chain(AMOUNTS.iter().copied())
            .find(|(a, _)| *a == k)
            .map(|(_, l)| l)
            .unwrap_or("")
    }

    fn f(&self, k: &str, layout: Layout, disabled: bool) -> AnyElement {
        field(
            &self.kit,
            k,
            Self::label(k),
            layout,
            disabled || !self.draft.lifecycle.is_editable(),
        )
    }

    fn cell(&self, k: String, caption: String, layout: Layout) -> AnyElement {
        field(
            &self.kit,
            &k,
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
                choice("1800_amended_yes", "Yes", d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(true))
                    },
                )),
                choice("1800_amended_no", "No", !d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(false))
                    },
                )),
            ],
        );
        let relief = choice_row(
            "Item 13 — Availing of tax relief under a Special Law / International Tax Treaty?",
            vec![
                choice("1800_relief_yes", "Yes", d.tax_relief.is_some(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| {
                            if d.tax_relief.is_none() {
                                d.tax_relief = Some(String::new());
                            }
                        })
                    }),
                ),
                choice("1800_relief_no", "No", d.tax_relief.is_none(), !editable).on_click(
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
            .child(kit::label("Item 5 — Donor's TIN (profile)"))
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
            computed_atc(cx),
            tin,
            self.f("rdo", layout, false),
            self.f("name", layout, false),
            self.f("reg_address", layout, false),
            self.f("zip", layout, false),
            self.f("res_address", layout, false),
            self.f("res_zip", layout, false),
            self.f("contact", layout, false),
            self.f("email", layout, false),
            self.f("lob", layout, false),
            relief,
        ];
        if d.tax_relief.is_some() {
            children.push(self.f("relief", layout, false));
        }
        let donees: Vec<AnyElement> = (0..FORM_1800_DONEES)
            .flat_map(|row| {
                [
                    self.cell(
                        key("donee", row, "name"),
                        format!("12{} — Donee's name", LETTERS[row]),
                        layout,
                    ),
                    self.cell(
                        key("donee", row, "tin"),
                        format!("12{} — Donee's TIN", LETTERS[row]),
                        layout,
                    ),
                ]
            })
            .collect();
        section(
            "Part I — Taxpayer information",
            vec![
                grid(layout, children),
                kit::label("Item 12 — Donees").into_any_element(),
                grid(layout, donees),
            ],
            cx,
        )
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children =
            vec![kit::label("Schedule A — Donated personal property").into_any_element()];
        for row in 0..FORM_1800_SCHEDULE_ROWS {
            children.push(grid(
                layout,
                vec![
                    self.cell(
                        key("pp", row, "desc"),
                        format!("A{} — Particulars", row + 1),
                        layout,
                    ),
                    self.cell(
                        key("pp", row, "fmv"),
                        format!("A{} — Fair market value", row + 1),
                        layout,
                    ),
                ],
            ));
        }
        children.push(computed("Total (Item 25)", d.total_personal, cx));
        children.push(kit::label("Schedule B — Donated real property").into_any_element());
        for row in 0..FORM_1800_SCHEDULE_ROWS {
            let mut cells: Vec<AnyElement> = REAL_FIELDS
                .iter()
                .map(|(part, caption)| {
                    self.cell(
                        key("rp", row, part),
                        format!("B{} — {caption}", row + 1),
                        layout,
                    )
                })
                .collect();
            cells.push(self.cell(
                key("rp", row, "fmv"),
                format!("B{} — Fair market value (higher)", row + 1),
                layout,
            ));
            children.push(grid(layout, cells));
        }
        children.push(computed("Total (Item 26)", d.total_real, cx));
        section("Part V — Schedules", children, cx)
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut deductions = Vec::new();
        for row in 0..FORM_1800_DEDUCTIONS {
            deductions.push(self.cell(
                key("ded", row, "title"),
                format!("{} — Deduction", 28 + row),
                layout,
            ));
            deductions.push(self.cell(
                key("ded", row, "amount"),
                format!("{} — Amount", 28 + row),
                layout,
            ));
        }
        let children = vec![
            computed("27 — Total gifts in this return", d.total_gifts, cx),
            computed("33 — Total deductions allowed", d.total_deductions, cx),
            computed(
                "34 — Total net gifts in this return",
                d.net_gifts_this_return,
                cx,
            ),
            self.f("prior_net", layout, false),
            computed("36 — Total net gifts", d.total_net_gifts, cx),
            computed(
                "38 / 14 — Net gifts subject to tax",
                d.net_gifts_subject_to_tax,
                cx,
            ),
            computed("16 — Donor's tax due (6%)", d.tax_due, cx),
            self.f("credit_a", layout, false),
            self.f("credit_b", layout, false),
            self.f("credit_c", layout, !d.is_amended || !editable),
            computed("17D — Total tax credits/payments", d.total_credits, cx),
            computed("18 — Tax payable (overpayment)", d.tax_payable, cx),
            self.f("surcharge", layout, false),
            self.f("interest", layout, false),
            self.f("compromise", layout, false),
            computed("19D — Total penalties", d.total_penalties, cx),
            computed(
                "20 — Total amount payable (overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        section(
            "Part II / IV — Computation of tax",
            vec![
                kit::label("Items 28–32 — Deductions").into_any_element(),
                grid(layout, deductions),
                grid(layout, children),
            ],
            cx,
        )
    }
}

fn computed_atc<V: 'static>(cx: &Context<V>) -> AnyElement {
    kit::computed_text("Item 4 — ATC", "DN 010".to_string(), cx)
}

impl KitView for Form1800View {
    type Draft = Form1800Draft;
    const SLUG: &'static str = "1800";
    const CODE: &'static str = "1800";

    fn kit(&self) -> &EditorState {
        &self.kit
    }
    fn kit_mut(&mut self) -> &mut EditorState {
        &mut self.kit
    }
    fn draft(&self) -> &Form1800Draft {
        &self.draft
    }
    fn draft_mut(&mut self) -> &mut Form1800Draft {
        &mut self.draft
    }
    fn db(&self) -> Arc<Mutex<Database>> {
        Arc::clone(&self.db)
    }
    fn set_draft_id(&mut self, id: i64) {
        self.draft.id = Some(id);
    }
    fn initial_text(&self, k: &str) -> String {
        Self::text_value(&self.draft, k)
    }

    fn read_inputs(&mut self, cx: &mut Context<Self>) -> Vec<(String, String)> {
        let mut r = InputReader::new(&self.kit, cx);
        let d = &mut self.draft;
        if let Some(v) = r.whole::<u8>("month", "Item 1 month", 99) {
            d.donation_month = v;
        }
        if let Some(v) = r.whole::<u8>("day", "Item 1 day", 99) {
            d.donation_day = v;
        }
        if let Some(v) = r.whole::<u16>("year", "Item 1 year", 9999) {
            d.donation_year = v;
        }
        if let Some(v) = r.whole::<u16>("sheets", "Item 3", 999) {
            d.number_of_attached_sheets = v;
        }
        d.rdo_code = r.trimmed("rdo").to_uppercase();
        d.donor_name = r.text("name");
        d.registered_address = r.text("reg_address");
        d.zip_code = r.trimmed("zip");
        d.residence_address = r.text("res_address");
        d.residence_zip_code = r.trimmed("res_zip");
        d.contact_number = r.trimmed("contact");
        d.email = r.trimmed("email");
        d.line_of_business = r.text("lob");
        if d.tax_relief.is_some() {
            d.tax_relief = Some(r.text("relief"));
        }
        for (k, label) in AMOUNTS {
            if let Some(value) = r.amount(k, label) {
                match *k {
                    "prior_net" => d.prior_net_gifts = value,
                    "credit_a" => d.prior_gift_payments = value,
                    "credit_b" => d.foreign_tax_paid = value,
                    "credit_c" => d.tax_paid_previous = value,
                    "surcharge" => d.surcharge = value,
                    "interest" => d.interest = value,
                    _ => d.compromise = value,
                }
            }
        }
        let mut donees: Vec<Form1800Donee> = (0..FORM_1800_DONEES)
            .map(|row| Form1800Donee {
                name: r.text(&key("donee", row, "name")),
                tin: r.trimmed(&key("donee", row, "tin")),
            })
            .collect();
        while donees
            .last()
            .is_some_and(|x| x.name.trim().is_empty() && x.tin.is_empty())
        {
            donees.pop();
        }
        d.donees = donees;
        let mut personal = Vec::new();
        let mut real = Vec::new();
        for row in 0..FORM_1800_SCHEDULE_ROWS {
            let label = format!("Schedule row {}", row + 1);
            personal.push(Form1800PersonalProperty {
                particulars: r.text(&key("pp", row, "desc")),
                fair_market_value: r.amount(&key("pp", row, "fmv"), &label).unwrap_or(0.0),
            });
            real.push(Form1800RealProperty {
                title_number: r.text(&key("rp", row, "oct")),
                tax_declaration_number: r.text(&key("rp", row, "td")),
                location: r.text(&key("rp", row, "loc")),
                lot_or_improvement: r.text(&key("rp", row, "lot")),
                classification: r.text(&key("rp", row, "class")),
                area: r.text(&key("rp", row, "area")),
                fmv_per_tax_declaration: r.text(&key("rp", row, "ftd")),
                fmv_per_zonal_value: r.text(&key("rp", row, "zonal")),
                fair_market_value: r.amount(&key("rp", row, "fmv"), &label).unwrap_or(0.0),
            });
        }
        while personal
            .last()
            .is_some_and(|x| x.particulars.trim().is_empty() && x.fair_market_value == 0.0)
        {
            personal.pop();
        }
        while real.last().is_some_and(|x| {
            x.fair_market_value == 0.0
                && [
                    &x.title_number,
                    &x.tax_declaration_number,
                    &x.location,
                    &x.lot_or_improvement,
                    &x.classification,
                    &x.area,
                    &x.fmv_per_tax_declaration,
                    &x.fmv_per_zonal_value,
                ]
                .iter()
                .all(|s| s.trim().is_empty())
        }) {
            real.pop();
        }
        d.personal_properties = personal;
        d.real_properties = real;
        let mut deductions: Vec<Form1800Deduction> = (0..FORM_1800_DEDUCTIONS)
            .map(|row| Form1800Deduction {
                title: r.text(&key("ded", row, "title")),
                amount: r
                    .amount(&key("ded", row, "amount"), "Deduction")
                    .unwrap_or(0.0),
            })
            .collect();
        while deductions
            .last()
            .is_some_and(|x| x.title.trim().is_empty() && x.amount == 0.0)
        {
            deductions.pop();
        }
        d.deductions = deductions;
        r.errors
    }
}

impl QueueableFormView for Form1800View {
    type Draft = Form1800Draft;

    fn new(
        draft: Form1800Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let kit = EditorState::new(Self::input_specs(&draft), errors, window, cx);
        Self { draft, db, kit }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1800Draft {
        Form1800Draft::new_from_profile(profile, year, u32::from(period.max(1)))
    }
}

impl FormViewTrait for Form1800View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1800"
    }
    fn form_subtitle(&self) -> &'static str {
        "Donor's Tax Return"
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
            Some("1800 payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::cancel(self, window, cx);
    }
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        kit::preview(self, "1800-2018", cx);
    }
}

impl Render for Form1800View {
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
    use super::{AMOUNTS, Form1800View, TEXT_INPUTS};

    #[test]
    fn every_input_has_a_label() {
        for (k, _, _) in TEXT_INPUTS {
            assert!(!Form1800View::label(k).is_empty(), "{k}");
        }
        for (k, _) in AMOUNTS {
            assert!(!Form1800View::label(k).is_empty(), "{k}");
        }
    }
}
