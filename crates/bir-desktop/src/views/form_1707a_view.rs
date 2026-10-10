//! Editor for BIR Form 1707-A (April 2021), Annual Capital Gains Tax Return
//! for Onerous Transfer of Shares of Stock Not Traded Through the Local Stock
//! Exchange. Rust owns every calculation, validation and the official submit
//! plaintext (`bir_core::forms::form_1707a`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1707a::{Form1707AAtc, Form1707ADraft, Form1707ARow};
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
    ("month", "Item 1 — Year-end month (fiscal)", "MM"),
    ("day", "Item 1 — Year-end day (fiscal)", "DD"),
    ("year", "Item 1 — Year ended", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("rdo", "Item 7 — RDO code", "e.g. 039"),
    ("name", "Item 8 — Taxpayer's name", ""),
    ("lob", "Line of business (profile)", ""),
    ("address", "Item 9 — Registered address", ""),
    ("zip", "Item 10 — Zip code", ""),
    ("contact", "Contact number", ""),
    ("email", "Item 11 — E-mail address", ""),
    ("relief", "Item 12A — Tax relief, specify", ""),
];

/// Amounts where blank is zero: (key, label).
const AMOUNTS: &[(&str, &str)] = &[
    (
        "less",
        "16B — Tax paid in return previously filed (amended only)",
    ),
    ("surcharge", "18A — Surcharge"),
    ("interest", "18B — Interest"),
    ("compromise", "18C — Compromise"),
];

/// Row inputs: (part, caption, gains only).
const ROW_FIELDS: [(&str, &str, bool); 5] = [
    ("date", "Date of transaction (MM/DD/YYYY)", false),
    ("corp", "Name of corporate stock", false),
    ("sell", "Selling price", false),
    ("cost", "Cost and expenses", false),
    ("paid", "Capital gains tax paid", true),
];

/// Rows offered per schedule: 1–4, then the official "More" list once a
/// schedule passes four rows.
const SCHEDULE_UI_ROWS: usize = 10;

fn row_key(schedule: u8, row: usize, part: &str) -> String {
    format!("s{schedule}_{row}_{part}")
}

pub struct Form1707AView {
    draft: Form1707ADraft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
}

impl EventEmitter<QueueableFormEvent> for Form1707AView {}

impl Form1707AView {
    fn text_value(d: &Form1707ADraft, key: &str) -> String {
        match key {
            "month" => format!("{:02}", d.year_end_month),
            "day" => format!("{:02}", d.year_end_day),
            "year" => d.year_end_year.to_string(),
            "sheets" => d.number_of_attached_sheets.to_string(),
            "rdo" => d.rdo_code.clone(),
            "name" => d.taxpayer_name.clone(),
            "lob" => d.line_of_business.clone(),
            "address" => d.registered_address.clone(),
            "zip" => d.zip_code.clone(),
            "contact" => d.contact_number.clone(),
            "email" => d.email.clone(),
            "relief" => d.tax_relief.clone().unwrap_or_default(),
            "less" => money_text(d.tax_paid_previous),
            "surcharge" => money_text(d.surcharge),
            "interest" => money_text(d.interest),
            "compromise" => money_text(d.compromise),
            _ => {
                for (schedule, rows) in [(1u8, &d.gains), (2u8, &d.losses)] {
                    for index in 0..SCHEDULE_UI_ROWS {
                        let row = rows.get(index).cloned().unwrap_or_default();
                        for (part, value) in [
                            ("date", row.date.clone()),
                            ("corp", row.corporation.clone()),
                            ("sell", money_text(row.selling_price)),
                            ("cost", money_text(row.cost)),
                            ("paid", money_text(row.tax_paid)),
                        ] {
                            if key == row_key(schedule, index, part) {
                                return value;
                            }
                        }
                    }
                }
                String::new()
            }
        }
    }

    fn input_specs(d: &Form1707ADraft) -> Vec<InputSpec> {
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(k, _, p)| (k.to_string(), p.to_string()))
            .collect();
        keys.extend(
            AMOUNTS
                .iter()
                .map(|(k, _)| (k.to_string(), "0.00".to_string())),
        );
        for schedule in [1u8, 2u8] {
            for index in 0..SCHEDULE_UI_ROWS {
                for (part, _, gains_only) in ROW_FIELDS {
                    if gains_only && schedule == 2 {
                        continue;
                    }
                    keys.push((row_key(schedule, index, part), String::new()));
                }
            }
        }
        keys.into_iter()
            .map(|(key, placeholder)| {
                let value = Self::text_value(d, &key);
                (key, placeholder, value)
            })
            .collect()
    }

    fn label(key: &str) -> &'static str {
        TEXT_INPUTS
            .iter()
            .map(|(k, l, _)| (*k, *l))
            .chain(AMOUNTS.iter().copied())
            .find(|(k, _)| *k == key)
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

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let individual = d.atc == Some(Form1707AAtc::Individual);
        let basis = choice_row(
            "Item 1 — For the",
            vec![
                choice("1707a_calendar", "Calendar year", !d.fiscal, !editable).on_click(
                    cx.listener(|this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_fiscal(false))
                    }),
                ),
                choice(
                    "1707a_fiscal",
                    "Fiscal year",
                    d.fiscal,
                    !editable || individual,
                )
                .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| d.set_fiscal(true))
                })),
            ],
        );
        let amended = choice_row(
            "Item 2 — Amended return?",
            vec![
                choice("1707a_amended_yes", "Yes", d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(true))
                    },
                )),
                choice("1707a_amended_no", "No", !d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(false))
                    },
                )),
            ],
        );
        let atc = choice_row(
            "Item 4 — ATC",
            vec![
                choice(
                    "1707a_atc_ii030",
                    "II030 — Individual",
                    individual,
                    !editable,
                )
                .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| d.set_atc(Form1707AAtc::Individual))
                })),
                choice(
                    "1707a_atc_ic110",
                    "IC110 — Corporation",
                    d.atc == Some(Form1707AAtc::Corporation),
                    !editable,
                )
                .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| d.set_atc(Form1707AAtc::Corporation))
                })),
            ],
        );
        let relief = choice_row(
            "Item 12 — Availing of tax relief under Special Law or International Tax Treaty?",
            vec![
                choice("1707a_relief_yes", "Yes", d.tax_relief.is_some(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| {
                            if d.tax_relief.is_none() {
                                d.tax_relief = Some(String::new());
                            }
                        })
                    }),
                ),
                choice("1707a_relief_no", "No", d.tax_relief.is_none(), !editable).on_click(
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
            basis,
            self.f("month", layout, !d.fiscal),
            self.f("day", layout, !d.fiscal),
            self.f("year", layout, false),
            amended,
            self.f("sheets", layout, false),
            atc,
            tin,
            self.f("rdo", layout, false),
            self.f("name", layout, false),
            self.f("lob", layout, false),
            self.f("address", layout, false),
            self.f("zip", layout, false),
            self.f("contact", layout, false),
            self.f("email", layout, false),
            relief,
        ];
        if d.tax_relief.is_some() {
            children.push(self.f("relief", layout, false));
        }
        section(
            "Return period and background information",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_schedule(&self, schedule: u8, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let rows = if schedule == 1 { &d.gains } else { &d.losses };
        let mut children = Vec::new();
        for index in 0..SCHEDULE_UI_ROWS {
            let mut cells: Vec<AnyElement> = ROW_FIELDS
                .iter()
                .filter(|(_, _, gains_only)| !gains_only || schedule == 1)
                .map(|(part, caption, _)| {
                    field(
                        &self.kit,
                        &row_key(schedule, index, part),
                        &format!("{} — {caption}", index + 1),
                        layout,
                        disabled,
                    )
                })
                .collect();
            let result = rows.get(index).map(|r| r.gain_or_loss).unwrap_or(0.0);
            cells.push(computed(
                &format!(
                    "{} — Capital {}",
                    index + 1,
                    if schedule == 1 { "gain" } else { "loss" }
                ),
                result,
                cx,
            ));
            children.push(grid(layout, cells));
        }
        let totals = if schedule == 1 {
            vec![
                computed("Total selling price", d.s1_total_selling_price, cx),
                computed("Total cost", d.s1_total_cost, cx),
                computed("Total capital gains (Item 12)", d.s1_total_gains, cx),
                computed("Total tax paid (Item 16A)", d.s1_total_tax_paid, cx),
            ]
        } else {
            vec![
                computed("Total selling price", d.s2_total_selling_price, cx),
                computed("Total cost", d.s2_total_cost, cx),
                computed("Total capital loss (Item 13)", d.s2_total_loss, cx),
            ]
        };
        children.push(grid(layout, totals));
        section(
            if schedule == 1 {
                "Schedule 1 — Capital gains"
            } else {
                "Schedule 2 — Capital losses"
            },
            children,
            cx,
        )
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let children = vec![
            computed("12 — Total capital gains", d.s1_total_gains, cx),
            computed("13 — Total capital loss", d.s2_total_loss, cx),
            computed("14 — Net capital gain (loss)", d.net_capital_gain, cx),
            computed("15 — Tax due (15%)", d.tax_due, cx),
            computed(
                "16A — Tax paid on prior returns this year",
                d.s1_total_tax_paid,
                cx,
            ),
            self.f("less", layout, !d.is_amended),
            computed("16C — Total tax paid", d.total_tax_paid, cx),
            computed("17 — Tax payable (overpayment)", d.tax_payable, cx),
            self.f("surcharge", layout, false),
            self.f("interest", layout, false),
            self.f("compromise", layout, false),
            computed("18D — Total penalties", d.total_penalties, cx),
            computed(
                "19 — Total amount payable (overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        section(
            "Part II — Computation of tax (Items 12–19)",
            vec![grid(layout, children)],
            cx,
        )
    }
}

impl KitView for Form1707AView {
    type Draft = Form1707ADraft;
    const SLUG: &'static str = "1707a";
    const CODE: &'static str = "1707A";

    fn kit(&self) -> &EditorState {
        &self.kit
    }
    fn kit_mut(&mut self) -> &mut EditorState {
        &mut self.kit
    }
    fn draft(&self) -> &Form1707ADraft {
        &self.draft
    }
    fn draft_mut(&mut self) -> &mut Form1707ADraft {
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
        if d.fiscal {
            if let Some(v) = r.whole::<u8>("month", "Item 1 month", 99) {
                d.year_end_month = v;
            }
            if let Some(v) = r.whole::<u8>("day", "Item 1 day", 99) {
                d.year_end_day = v;
            }
        }
        if let Some(v) = r.whole::<u16>("year", "Item 1 year", 9999) {
            d.year_end_year = v;
        }
        if let Some(v) = r.whole::<u16>("sheets", "Item 3", 999) {
            d.number_of_attached_sheets = v;
        }
        d.rdo_code = r.trimmed("rdo").to_uppercase();
        d.taxpayer_name = r.text("name");
        d.line_of_business = r.text("lob");
        d.registered_address = r.text("address");
        d.zip_code = r.trimmed("zip");
        d.contact_number = r.trimmed("contact");
        d.email = r.trimmed("email");
        if d.tax_relief.is_some() {
            d.tax_relief = Some(r.text("relief"));
        }
        for (key, label) in AMOUNTS {
            if let Some(value) = r.amount(key, label) {
                match *key {
                    "less" => d.tax_paid_previous = value,
                    "surcharge" => d.surcharge = value,
                    "interest" => d.interest = value,
                    _ => d.compromise = value,
                }
            }
        }
        for schedule in [1u8, 2u8] {
            let mut rows = Vec::new();
            for index in 0..SCHEDULE_UI_ROWS {
                let label = format!("Schedule {schedule} row {}", index + 1);
                let selling = r
                    .amount(&row_key(schedule, index, "sell"), &label)
                    .unwrap_or(0.0);
                let cost = r
                    .amount(&row_key(schedule, index, "cost"), &label)
                    .unwrap_or(0.0);
                let tax_paid = if schedule == 1 {
                    r.amount(&row_key(schedule, index, "paid"), &label)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                rows.push(Form1707ARow {
                    date: r.trimmed(&row_key(schedule, index, "date")),
                    corporation: r.text(&row_key(schedule, index, "corp")),
                    selling_price: selling,
                    cost,
                    gain_or_loss: 0.0,
                    tax_paid,
                });
            }
            while rows.last().is_some_and(|row| {
                row.date.is_empty()
                    && row.corporation.trim().is_empty()
                    && row.selling_price == 0.0
                    && row.cost == 0.0
                    && row.tax_paid == 0.0
            }) {
                rows.pop();
            }
            if schedule == 1 {
                d.gains = rows;
            } else {
                d.losses = rows;
            }
        }
        r.errors
    }
}

impl QueueableFormView for Form1707AView {
    type Draft = Form1707ADraft;

    fn new(
        draft: Form1707ADraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let kit = EditorState::new(Self::input_specs(&draft), errors, window, cx);
        Self { draft, db, kit }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1707ADraft {
        Form1707ADraft::new_from_profile(profile, year, u32::from(period.max(1)))
    }
}

impl FormViewTrait for Form1707AView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1707-A"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Capital Gains Tax Return (Shares of Stock Not Traded Through the Local Stock Exchange)"
    }
    fn form_version(&self) -> &'static str {
        "April 2021 (ENCS)"
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
            Some("1707A payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::cancel(self, window, cx);
    }
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        kit::preview(self, "1707a-2021", cx);
    }
}

impl Render for Form1707AView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let sections = vec![
            self.render_header_items(layout, cx),
            self.render_schedule(1, layout, cx),
            self.render_schedule(2, layout, cx),
            self.render_computation(layout, cx),
        ];
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{AMOUNTS, Form1707AView, TEXT_INPUTS};

    #[test]
    fn every_input_has_a_label() {
        for (key, _, _) in TEXT_INPUTS {
            assert!(!Form1707AView::label(key).is_empty(), "{key}");
        }
        for (key, _) in AMOUNTS {
            assert!(!Form1707AView::label(key).is_empty(), "{key}");
        }
    }
}
