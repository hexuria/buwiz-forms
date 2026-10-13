//! Editor for BIR Form 1800 (January 2018), Donor's Tax Return. Rust owns
//! every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1800`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1800::{
    FORM_1800_DEDUCTIONS, FORM_1800_DONEES, FORM_1800_EXEMPT_GIFT, FORM_1800_SCHEDULE_ROWS,
    FORM_1800_TAX_RATE, Form1800Deduction, Form1800Donee, Form1800Draft, Form1800PersonalProperty,
    Form1800RealProperty,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable};

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice, choice_row,
    computed, field, format_tin, grid, money_text, section,
};
use super::form_controls::{messages_for, with_field_error};
use crate::components::form_engine::FormViewTrait;
use crate::components::penalty_section::{
    self, PenaltyPartyView, PenaltySection, PenaltySectionEvent,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::penalties::PenaltyComponent;

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

/// The row number in an editor key built by [`key`] (`"pp3_fmv"` is 3 for
/// prefix `"pp"`).
fn row_of(editor_key: &str, prefix: &str) -> Option<usize> {
    editor_key
        .strip_prefix(prefix)?
        .split('_')
        .next()?
        .parse()
        .ok()
}

/// Item 1 parts left blank (0) show as an empty box.
fn blank_zero<T: Into<u32>>(value: T, text: String) -> String {
    if value.into() == 0 {
        String::new()
    } else {
        text
    }
}

/// Every validation key whose message belongs under the editor `editor_key`:
/// its own key (malformed numbers) and the model's field name.
fn error_keys(editor_key: &str) -> Vec<String> {
    let mut keys = vec![editor_key.to_string()];
    let model: &[&str] = match editor_key {
        "month" | "day" | "year" => &["donation_date"],
        "sheets" => &["number_of_attached_sheets"],
        "rdo" => &["rdo_code"],
        "name" => &["donor_name"],
        "relief" => &["tax_relief"],
        "prior_net" => &["prior_net_gifts"],
        "credit_a" => &["prior_gift_payments"],
        "credit_b" => &["foreign_tax_paid"],
        "credit_c" => &["tax_paid_previous"],
        _ => &[],
    };
    keys.extend(model.iter().map(|k| k.to_string()));
    if let Some(row) = row_of(editor_key, "donee") {
        keys.push(format!("donees[{row}]"));
    } else if let Some(row) = row_of(editor_key, "pp").filter(|_| editor_key.ends_with("_fmv")) {
        keys.push(format!("personal_properties[{row}].fair_market_value"));
    } else if let Some(row) = row_of(editor_key, "rp").filter(|_| editor_key.ends_with("_fmv")) {
        keys.push(format!("real_properties[{row}].fair_market_value"));
    }
    keys
}

pub struct Form1800View {
    draft: Form1800Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
    /// The shared penalty section (computed values, overrides, note).
    penalty: Entity<PenaltySection>,
    _penalty_subscription: Subscription,
    /// Schedule A / B rows on screen: never fewer than five or than the
    /// draft holds ("Add" on the official page has no limit).
    personal_rows: usize,
    real_rows: usize,
    /// Submit was pressed with tax due of zero or less from Items 28-32 or
    /// the exemption; the confirm is open and nothing is queued yet.
    zero_tax_confirm_open: bool,
}

impl EventEmitter<QueueableFormEvent> for Form1800View {}

impl Form1800View {
    fn text_value(d: &Form1800Draft, k: &str) -> String {
        match k {
            "month" => blank_zero(d.donation_month, format!("{:02}", d.donation_month)),
            "day" => blank_zero(d.donation_day, format!("{:02}", d.donation_day)),
            "year" => blank_zero(d.donation_year, d.donation_year.to_string()),
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
            "surcharge" => penalty_section::override_text(&d.penalty, PenaltyComponent::Surcharge),
            "interest" => penalty_section::override_text(&d.penalty, PenaltyComponent::Interest),
            "compromise" => {
                penalty_section::override_text(&d.penalty, PenaltyComponent::Compromise)
            }
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
                for row in 0..d.personal_properties.len() {
                    let a = d.personal_properties[row].clone();
                    if k == key("pp", row, "desc") {
                        return a.particulars;
                    }
                    if k == key("pp", row, "fmv") {
                        return money_text(a.fair_market_value);
                    }
                }
                for row in 0..d.real_properties.len() {
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

    fn specs(&self) -> Vec<InputSpec> {
        Self::input_specs(&self.draft, self.personal_rows, self.real_rows)
    }

    fn input_specs(d: &Form1800Draft, personal_rows: usize, real_rows: usize) -> Vec<InputSpec> {
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
        for row in 0..personal_rows {
            keys.push((key("pp", row, "desc"), String::new()));
            keys.push((key("pp", row, "fmv"), "0.00".to_string()));
        }
        for row in 0..real_rows {
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

    /// Messages (malformed input and validation) for any of `keys`.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.kit
                .parse_errors
                .iter()
                .chain(self.kit.validation_errors.iter()),
            keys,
        )
    }

    /// `element` with the messages for `keys` directly under it.
    fn with_errors(&self, element: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(element, self.errors_for(keys))
    }

    /// An input with its own messages directly under it.
    fn with_input_errors(&self, input: AnyElement, k: &str) -> AnyElement {
        let keys = error_keys(k);
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.with_errors(input, &keys)
    }

    /// A read-only profile value with its messages under it, plus a pointer
    /// to Profile Settings when there are any.
    fn with_profile_note(
        &self,
        element: AnyElement,
        keys: &[&str],
        cx: &Context<Self>,
    ) -> AnyElement {
        let element = self.with_errors(element, keys);
        if self.errors_for(keys).is_none() {
            return element;
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(element)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("This comes from your taxpayer profile. Edit it in Profile Settings."),
            )
            .into_any_element()
    }

    fn f(&self, k: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = field(
            &self.kit,
            k,
            Self::label(k),
            layout,
            disabled || !self.draft.lifecycle.is_editable(),
        );
        self.with_input_errors(input, k)
    }

    fn cell(&self, k: String, caption: String, layout: Layout) -> AnyElement {
        let input = field(
            &self.kit,
            &k,
            &caption,
            layout,
            !self.draft.lifecycle.is_editable(),
        );
        self.with_input_errors(input, &k)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = choice_row(
            layout,
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
            layout,
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
        let tin = self.with_profile_note(tin, &["tin"], cx);
        let children = vec![
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
        ];
        let mut relief_rows = vec![relief];
        if d.tax_relief.is_some() {
            relief_rows.push(self.f("relief", layout, false));
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
                self.with_errors(
                    kit::label("Item 12 — Donees").into_any_element(),
                    &["donees"],
                ),
                grid(layout, donees),
                grid(layout, relief_rows),
            ],
            cx,
        )
    }

    /// Row caption with "Remove"; removing never drops another row.
    fn row_heading(&self, schedule: &'static str, row: usize, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        let remove = Button::new((SharedString::from(format!("1800_{schedule}_remove")), row))
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                kit::edit_rows(
                    this,
                    window,
                    cx,
                    |v| v.remove_row(schedule, row),
                    Self::specs,
                )
            }));
        let letter = if schedule == "a" { "A" } else { "B" };
        kit::row_heading(&format!("Row {letter}{}", row + 1), remove)
    }

    fn add_row(&self, schedule: &'static str, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        kit::add_row_button(SharedString::from(format!("1800_{schedule}_add")), disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                kit::edit_rows(
                    this,
                    window,
                    cx,
                    |v| {
                        if schedule == "a" {
                            v.personal_rows += 1;
                        } else {
                            v.real_rows += 1;
                        }
                    },
                    Self::specs,
                )
            }))
            .into_any_element()
    }

    fn remove_row(&mut self, schedule: &str, row: usize) {
        if schedule == "a" {
            if row < self.draft.personal_properties.len() {
                self.draft.personal_properties.remove(row);
            }
            self.personal_rows = self
                .personal_rows
                .saturating_sub(1)
                .max(FORM_1800_SCHEDULE_ROWS);
        } else {
            if row < self.draft.real_properties.len() {
                self.draft.real_properties.remove(row);
            }
            self.real_rows = self
                .real_rows
                .saturating_sub(1)
                .max(FORM_1800_SCHEDULE_ROWS);
        }
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children =
            vec![kit::label("Schedule A — Donated personal property").into_any_element()];
        for row in 0..self.personal_rows {
            children.push(self.row_heading("a", row, cx));
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
        children.push(self.add_row("a", cx));
        children.push(computed("Total (Item 25)", d.total_personal, cx));
        children.push(kit::label("Schedule B — Donated real property").into_any_element());
        for row in 0..self.real_rows {
            children.push(self.row_heading("b", row, cx));
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
        children.push(self.add_row("b", cx));
        children.push(computed("Total (Item 26)", d.total_real, cx));
        section("Part V — Schedules", children, cx)
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> Vec<AnyElement> {
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
        let part_four = vec![
            computed("25 — Personal properties", d.total_personal, cx),
            computed("26 — Real properties", d.total_real, cx),
            computed("27 — Total gifts in this return", d.total_gifts, cx),
        ];
        let part_four_rest = vec![
            computed("33 — Total deductions allowed", d.total_deductions, cx),
            computed(
                "34 — Total net gifts in this return",
                d.net_gifts_this_return,
                cx,
            ),
            self.f("prior_net", layout, false),
            computed("36 — Total net gifts", d.total_net_gifts, cx),
            computed("37 — Less: Exempt gift", FORM_1800_EXEMPT_GIFT, cx),
            self.with_errors(
                computed(
                    "38 — Total net gifts subject to tax",
                    d.net_gifts_subject_to_tax,
                    cx,
                ),
                &["net_gifts_subject_to_tax"],
            ),
        ];
        let children = vec![
            computed(
                "14 — Total net gifts subject to tax (from Item 38)",
                d.net_gifts_subject_to_tax,
                cx,
            ),
            kit::computed_text(
                "15 — Applicable donor's tax rate",
                format!("{:.0}%", FORM_1800_TAX_RATE * 100.0),
                cx,
            ),
            computed("16 — Total donor's tax due", d.tax_due, cx),
            self.f("credit_a", layout, false),
            self.f("credit_b", layout, false),
            self.f("credit_c", layout, !d.is_amended || !editable),
            computed("17D — Total tax credits/payments", d.total_credits, cx),
            computed("18 — Tax payable (overpayment)", d.tax_payable, cx),
            penalty_section::render(
                &self.penalty,
                &PenaltyPartyView::all(&self.draft),
                &self
                    .kit
                    .validation_errors
                    .iter()
                    .chain(&self.kit.parse_errors)
                    .cloned()
                    .collect::<Vec<_>>(),
                cx,
            ),
            computed("19D — Total penalties", d.total_penalties, cx),
            computed(
                "20 — Total amount payable (overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        vec![
            section(
                "Part II — Computation of tax",
                vec![grid(layout, children)],
                cx,
            ),
            section(
                "Part IV — Computation of tax (gifts and deductions)",
                vec![
                    grid(layout, part_four),
                    self.with_errors(
                        kit::label("Items 28–32 — Deductions").into_any_element(),
                        &["deductions"],
                    ),
                    entry_note(DEDUCTIONS_NOTE, cx),
                    grid(layout, deductions),
                    grid(layout, part_four_rest),
                    entry_note(EXEMPTION_NOTE, cx),
                ],
                cx,
            ),
        ]
    }
}

impl Form1800View {
    fn render_zero_tax_confirm(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(div().text_sm().child(ZERO_TAX_CONFIRM_TEXT))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("1800_zero_tax_submit")
                            .label("Submit anyway")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.zero_tax_confirm_open = false;
                                kit::queue(this, window, cx);
                            })),
                    )
                    .child(
                        Button::new("1800_zero_tax_cancel")
                            .label("Cancel")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zero_tax_confirm_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}

/// Owner-set confirm before Submit when tax due is zero or less because of
/// Items 28-32 or the exemption (not an official alert).
const ZERO_TAX_CONFIRM_TEXT: &str = "Tax due is ₱0 or less because of Items 28–32 / the ₱250,000 exemption. Check with your RDO before filing.";

/// Shown before any figure is typed: the official page cannot carry
/// Items 28-32 into Item 33, so such a return is filed with the RDO.
const DEDUCTIONS_NOTE: &str = "Items 28 to 32 are not carried into Item 33 by the official 1800 page. If you claim a deduction, file this return with your RDO.";
/// Shown before any figure is typed: the official page gives a negative
/// tax due when net gifts fall within the P250,000 exemption.
const EXEMPTION_NOTE: &str = "If total net gifts (Item 36) are within the P250,000 exemption, the official page reports a negative tax due. File such a return with your RDO.";

fn entry_note<V: 'static>(text: &'static str, cx: &Context<V>) -> AnyElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

fn computed_atc<V: 'static>(cx: &Context<V>) -> AnyElement {
    kit::computed_text("Item 4 — ATC", "DN 010".to_string(), cx)
}

impl KitView for Form1800View {
    type Draft = Form1800Draft;
    const SLUG: &'static str = "1800";
    const CODE: &'static str = "1800";
    const VALIDATION_SUCCESS: Option<&'static str> =
        Some("Validation successful. Click on 'Edit' if you wish to modify your entries.");

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
    fn print_field_map(&self) -> std::collections::BTreeMap<String, String> {
        self.draft.to_print_field_map()
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
        for row in 0..self.personal_rows {
            let label = format!("Schedule A row {}", row + 1);
            personal.push(Form1800PersonalProperty {
                particulars: r.text(&key("pp", row, "desc")),
                fair_market_value: r.amount(&key("pp", row, "fmv"), &label).unwrap_or(0.0),
            });
        }
        for row in 0..self.real_rows {
            let label = format!("Schedule B row {}", row + 1);
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
        let mut errors = r.errors;
        // An empty penalty box means "use computed"; any amount overrides it.
        errors.extend(penalty_section::apply_overrides(
            &self.penalty,
            &<Form1800Draft as bir_core::forms::queueable::QueueableForm>::penalty_items(),
            &mut self.draft,
            cx,
        ));
        errors
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
        let personal_rows = draft.personal_properties.len().max(FORM_1800_SCHEDULE_ROWS);
        let real_rows = draft.real_properties.len().max(FORM_1800_SCHEDULE_ROWS);
        let mut kit = EditorState::new(
            Self::input_specs(&draft, personal_rows, real_rows),
            errors,
            window,
            cx,
        );
        kit.error_keys = Some(error_keys);
        // The penalty boxes are overrides (empty means "use computed").
        let penalty_views = PenaltyPartyView::all(&draft);
        let penalty_inputs =
            vec![["surcharge", "interest", "compromise"].map(|key| kit.inputs[key].clone())];
        let penalty = cx.new(|cx| PenaltySection::wrap(&penalty_views, penalty_inputs, window, cx));
        let _penalty_subscription = cx.subscribe_in(
            &penalty,
            window,
            |this: &mut Self, _, _: &PenaltySectionEvent, _, cx| {
                super::event_form_kit::sync(this, cx);
            },
        );
        Self {
            draft,
            db,
            kit,
            penalty,
            _penalty_subscription,
            personal_rows,
            real_rows,
            zero_tax_confirm_open: false,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1800Draft {
        Form1800Draft::new_from_profile(profile, year, u32::from(period.max(1)))
    }

    /// The saved draft, with the read-only TIN re-read from the profile while
    /// it is still editable, so a TIN fixed in Profile Settings reaches it.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form1800Draft {
        let mut draft = db
            .get_queueable_draft::<Form1800Draft>(&profile.tin.full(), year, i64::from(period))
            .ok()
            .flatten()
            .unwrap_or_else(|| Self::new_draft(profile, year, period));
        if draft.lifecycle.is_editable() {
            draft.tin = profile.tin.full();
        }
        draft
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
    /// Submit: when tax due is zero or less because of Items 28-32 or the
    /// exemption, ask first; otherwise queue as the official page files it.
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.zero_tax_confirm_open = false;
        if self.draft.lifecycle.is_editable() {
            kit::sync(self, cx);
            if self.kit.parse_errors.is_empty()
                && self.kit.validation_errors.is_empty()
                && self.draft.zero_tax_due_needs_confirm()
            {
                self.zero_tax_confirm_open = true;
                cx.notify();
                return;
            }
        }
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
        let mut sections = Vec::new();
        if self.zero_tax_confirm_open && self.draft.lifecycle.is_editable() {
            sections.push(self.render_zero_tax_confirm(cx));
        }
        sections.push(self.render_part_one(layout, cx));
        sections.extend(self.render_computation(layout, cx));
        sections.push(self.render_schedules(layout, cx));
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{AMOUNTS, Form1800PersonalProperty, Form1800View, TEXT_INPUTS, error_keys, key};
    use bir_core::forms::form_1800::Form1800Draft;

    /// Each message the model records lands under the editor it names.
    #[test]
    fn model_error_keys_reach_their_editors() {
        assert!(error_keys("month").contains(&"donation_date".to_string()));
        assert!(error_keys("rdo").contains(&"rdo_code".to_string()));
        assert!(error_keys("name").contains(&"donor_name".to_string()));
        assert!(error_keys("donee3_tin").contains(&"donees[3]".to_string()));
        assert!(
            error_keys("pp7_fmv").contains(&"personal_properties[7].fair_market_value".to_string())
        );
        assert!(
            error_keys("rp2_fmv").contains(&"real_properties[2].fair_market_value".to_string())
        );
        assert_eq!(error_keys("pp7_desc"), ["pp7_desc"]);
    }

    /// Every stored row gets an editor holding its value, so read-back keeps
    /// rows past the first five.
    #[test]
    fn every_stored_row_has_an_editor() {
        let profile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Sample Taxpayer",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "789", "branch": "00000"},
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "Quezon City",
            "zip_code": "1100",
            "phone": "09170000000",
            "email": "sample@example.com",
            "default_form_type": "1800v2018",
            "taxpayer_type": "Individual",
            "withholds_compensation": false,
            "has_employees": false
        }))
        .expect("profile");
        let mut draft = Form1800Draft::new_from_profile(&profile, 2025, 1);
        draft.personal_properties = (0..9)
            .map(|k| Form1800PersonalProperty {
                particulars: format!("Item {k}"),
                fair_market_value: 0.0,
            })
            .collect();
        let specs = Form1800View::input_specs(&draft, 9, 5);
        let last = key("pp", 8, "desc");
        let spec = specs.iter().find(|(k, _, _)| *k == last).expect("row 9");
        assert_eq!(spec.2, "Item 8");
    }

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
