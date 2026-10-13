//! The Surcharge / Interest / Compromise section every form with penalty
//! items shows (`bir_core::forms::penalty_items`).
//!
//! It shows what the engine computed and the inputs it used (due date,
//! filing date, payment date, tax still due, days late, taxpayer class,
//! compromise schedule), then each item as "Computed ₱X · Using ₱Y
//! (override)" with an override input (empty means "use computed") and a
//! "Use computed" reset, and an override note. A return with no deadline in
//! the app's calendar says so and takes overrides only.
//!
//! The section owns the override and note inputs ([`PenaltySection`]) and
//! emits [`PenaltySectionEvent::Changed`] on every edit; the form view reads
//! the overrides back with [`PenaltySection::overrides`] when it syncs, and
//! renders the section with [`render`] from its own draft each frame, so the
//! computed values shown are always the draft's.

use bir_core::forms::FilingStatus;
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::penalty_items::PenaltyItems;
use bir_core::forms::queueable::QueueableForm;
use bir_core::official_xml::{official_amount, parse_official_amount};
use bir_core::penalties::{
    ComputedPenalties, PenaltyComponent, PenaltyOverrides, PenaltyProfile, TaxpayerClass,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::views::form_controls::{messages_for, with_field_error};

/// Emitted whenever an override or the note changes.
pub enum PenaltySectionEvent {
    Changed,
}

/// What the section shows for one party's penalty items, read from the draft.
#[derive(Clone)]
pub struct PenaltyPartyView {
    pub party: Option<&'static str>,
    pub fields: [&'static str; 3],
    pub labels: [&'static str; 3],
    pub basic_tax_due_label: &'static str,
    pub schedule: PenaltyProfile,
    pub computed: Option<ComputedPenalties>,
    pub overrides: PenaltyOverrides,
    /// The items as the form currently files them.
    pub filed: [f64; 3],
    pub status: FilingStatus,
    pub applies: bool,
}

impl PenaltyPartyView {
    pub fn of<F>(items: &PenaltyItems<F>, draft: &F) -> Self {
        let state = (items.state)(draft);
        Self {
            party: items.party,
            fields: items.fields,
            labels: items.labels,
            basic_tax_due_label: items.basic_tax_due_label,
            schedule: items.schedule,
            computed: state.computed_penalties.clone(),
            overrides: state.penalty_overrides.clone(),
            filed: items.filed(draft),
            status: (items.status)(draft),
            applies: (items.applies)(draft),
        }
    }

    /// Every party of a queueable form, in declaration order.
    pub fn all<F: QueueableForm>(draft: &F) -> Vec<Self> {
        F::penalty_items()
            .iter()
            .map(|items| Self::of(items, draft))
            .collect()
    }

    /// Items 32-34 of a 1601-C.
    pub fn form_1601c(draft: &Form1601CDraft) -> Vec<Self> {
        vec![Self::of(&Form1601CDraft::penalty_items(), draft)]
    }
}

/// One party's override inputs and note.
struct PartyInputs {
    amounts: [Entity<InputState>; 3],
    note: Entity<InputState>,
}

pub struct PenaltySection {
    parties: Vec<PartyInputs>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PenaltySectionEvent> for PenaltySection {}

/// An override as its input shows it: whole pesos without decimals (every
/// form's amount boxes accept them), otherwise pesos and centavos.
fn amount_text(value: Option<f64>) -> String {
    value
        .map(|value| {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                format!("{value:.2}")
            }
        })
        .unwrap_or_default()
}

/// The text an override input shows for `component`: the override, or empty
/// ("use computed").
pub fn override_text(
    state: &bir_core::forms::penalty_items::PenaltyState,
    component: PenaltyComponent,
) -> String {
    amount_text(state.penalty_overrides.get(component))
}

impl PenaltySection {
    /// Inputs for each party's overrides (`parties` in declaration order).
    pub fn new(parties: &[PenaltyPartyView], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = Vec::new();
        let mut inputs = Vec::new();
        for party in parties {
            let amounts = PenaltyComponent::ALL.map(|component| {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("Use computed"));
                let text = amount_text(party.overrides.get(component));
                input.update(cx, |input, cx| input.set_value(text, window, cx));
                input
            });
            let note = cx.new(|cx| {
                InputState::new(window, cx).placeholder("e.g. RDO approval or abatement letter no.")
            });
            let text = party.overrides.note.clone();
            note.update(cx, |input, cx| input.set_value(text, window, cx));
            for input in amounts.iter().chain(std::iter::once(&note)) {
                subscriptions.push(cx.subscribe_in(
                    input,
                    window,
                    |_, _, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            cx.emit(PenaltySectionEvent::Changed);
                        }
                    },
                ));
            }
            inputs.push(PartyInputs { amounts, note });
        }
        Self {
            parties: inputs,
            _subscriptions: subscriptions,
        }
    }

    /// The section around amount inputs the form view already owns (and
    /// subscribes to), one `[surcharge, interest, compromise]` per party; the
    /// section adds the note inputs. The view shows each override in its
    /// input with [`override_text`] (empty means "use computed").
    pub fn wrap(
        parties: &[PenaltyPartyView],
        amount_inputs: Vec<[Entity<InputState>; 3]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let mut inputs = Vec::new();
        for (party, amounts) in parties.iter().zip(amount_inputs) {
            let note = cx.new(|cx| {
                InputState::new(window, cx).placeholder("e.g. RDO approval or abatement letter no.")
            });
            let text = party.overrides.note.clone();
            note.update(cx, |input, cx| input.set_value(text, window, cx));
            subscriptions.push(cx.subscribe_in(
                &note,
                window,
                |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.emit(PenaltySectionEvent::Changed);
                    }
                },
            ));
            inputs.push(PartyInputs { amounts, note });
        }
        Self {
            parties: inputs,
            _subscriptions: subscriptions,
        }
    }

    /// The overrides as entered (an empty box is "use computed"), one per
    /// party, or `(field, message)` for an amount that does not parse.
    pub fn overrides(
        &self,
        views: &[PenaltyPartyView],
        cx: &App,
    ) -> (Vec<PenaltyOverrides>, Vec<(String, String)>) {
        let mut errors = Vec::new();
        let overrides = self
            .parties
            .iter()
            .zip(views)
            .map(|(inputs, view)| {
                let mut overrides = PenaltyOverrides::default();
                for ((component, input), field) in PenaltyComponent::ALL
                    .into_iter()
                    .zip(&inputs.amounts)
                    .zip(view.fields)
                {
                    let text = input.read(cx).value().trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    match parse_official_amount(&text) {
                        Some(value) if value.is_finite() && value >= 0.0 => {
                            overrides.set(component, Some(value));
                        }
                        _ => errors.push((
                            field.to_string(),
                            format!(
                                "Enter an amount or leave the box empty to use the computed {}.",
                                component.label().to_lowercase()
                            ),
                        )),
                    }
                }
                overrides.note = inputs.note.read(cx).value().trim().to_string();
                overrides
            })
            .collect();
        (overrides, errors)
    }

    /// Show `views`' overrides in the inputs (after the draft was replaced).
    pub fn load(
        &mut self,
        views: &[PenaltyPartyView],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (inputs, view) in self.parties.iter().zip(views) {
            for (component, input) in PenaltyComponent::ALL.into_iter().zip(&inputs.amounts) {
                let text = amount_text(view.overrides.get(component));
                let current = input.read(cx).value().to_string();
                let unchanged = current.trim() == text
                    || (!text.is_empty()
                        && parse_official_amount(current.trim()) == view.overrides.get(component));
                if !unchanged {
                    input.update(cx, |input, cx| input.set_value(text, window, cx));
                }
            }
            if inputs.note.read(cx).value().trim() != view.overrides.note {
                let note = view.overrides.note.clone();
                inputs
                    .note
                    .update(cx, |input, cx| input.set_value(note, window, cx));
            }
        }
    }

    /// "Use computed": clear one override.
    fn reset(
        &mut self,
        party: usize,
        component: PenaltyComponent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(inputs) = self.parties.get(party) {
            let index = PenaltyComponent::ALL
                .iter()
                .position(|c| *c == component)
                .unwrap_or(0);
            inputs.amounts[index].update(cx, |input, cx| input.set_value("", window, cx));
            cx.emit(PenaltySectionEvent::Changed);
        }
    }
}

fn class_text(inputs: &bir_core::penalties::PenaltyInputs) -> String {
    if inputs.class_from_profile {
        let class = match inputs.taxpayer_class {
            TaxpayerClass::Micro => "Micro",
            TaxpayerClass::Small => "Small",
            TaxpayerClass::Medium => "Medium",
            TaxpayerClass::Large => "Large",
            TaxpayerClass::Regular => "Standard",
        };
        format!("Taxpayer class: {class} (from profile)")
    } else {
        "Taxpayer class: standard (non-micro/small) assumed, the profile has no EOPT classification"
            .to_string()
    }
}

fn schedule_text(schedule: PenaltyProfile) -> &'static str {
    match schedule {
        PenaltyProfile::Withholding => "Withholding compromise schedule (RMO 7-2015)",
        PenaltyProfile::StandardFiling => "Standard compromise schedule (RMO 7-2015)",
    }
}

/// The engine's inputs, shown above the items so a computed penalty is
/// never filed without the user seeing what it came from.
fn basis(view: &PenaltyPartyView, cx: &App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let frozen = !matches!(view.status, FilingStatus::Draft | FilingStatus::Queued);
    let Some(computed) = view.computed.as_ref() else {
        let text = if frozen {
            "Filed values: penalties are frozen as filed."
        } else {
            "No deadline for this return — enter any assessed penalties as overrides."
        };
        return div()
            .text_sm()
            .text_color(muted)
            .child(text)
            .into_any_element();
    };
    let inputs = &computed.inputs;
    let date = |date: chrono::NaiveDate| date.format("%b %-d, %Y").to_string();
    let headline = if computed.filed_on_time() {
        "Filed on time — no penalties".to_string()
    } else if inputs.basic_tax_due <= 0.0 {
        format!(
            "{} days late, but no tax is still due ({}), so nothing is computed. Enter any assessed compromise below.",
            inputs.days_late, view.basic_tax_due_label
        )
    } else {
        format!(
            "{} day{} late",
            inputs.days_late,
            if inputs.days_late == 1 { "" } else { "s" }
        )
    };
    let filing_label = if frozen {
        "Filing date"
    } else {
        "Filing date (today)"
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .bg(cx.theme().muted.opacity(0.3))
        .text_sm()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(headline))
        .child(div().text_color(muted).child(format!(
            "Due date {} · {} {} · Payment date {} · Tax still due ({}) ₱{}",
            date(inputs.due_date),
            filing_label,
            date(inputs.filing_date),
            date(inputs.payment_date),
            view.basic_tax_due_label,
            official_amount(inputs.basic_tax_due),
        )))
        .child(div().text_color(muted).child(format!(
            "{} · {}",
            class_text(inputs),
            schedule_text(view.schedule)
        )))
        .into_any_element()
}

/// The penalty section for `views` (one per party; parties whose items do
/// not apply are left out). `errors` are the page's `(field, message)`
/// pairs; those for an item's field show under it.
pub fn render(
    section: &Entity<PenaltySection>,
    views: &[PenaltyPartyView],
    errors: &[(String, String)],
    cx: &App,
) -> AnyElement {
    let several = views.iter().filter(|view| view.applies).count() > 1;
    let mut column = div().flex().flex_col().gap_3().w_full();
    for (index, view) in views.iter().enumerate() {
        if view.applies {
            column = column.child(render_party(section, index, view, several, errors, cx));
        }
    }
    if let Some(message) = messages_for(errors.iter(), &["computed_penalties"]) {
        column = column.child(crate::views::form_controls::field_error(message));
    }
    column.into_any_element()
}

/// One party's items (`index` in declaration order), for a page that lays
/// each party out in its own column. `named` shows the party's name.
pub fn render_party(
    section: &Entity<PenaltySection>,
    index: usize,
    view: &PenaltyPartyView,
    named: bool,
    errors: &[(String, String)],
    cx: &App,
) -> AnyElement {
    let state = section.read(cx);
    let Some(inputs) = state.parties.get(index) else {
        return div().into_any_element();
    };
    let editable = matches!(view.status, FilingStatus::Draft);
    let mut party = div().flex().flex_col().gap_2().w_full();
    if named && let Some(name) = view.party {
        party = party.child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(name),
        );
    }
    party = party.child(basis(view, cx));
    if editable {
        party = party.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("An amount in a box overrides that item; leave it empty to use the computed amount."),
        );
    }
    for (((component, label), input), field) in PenaltyComponent::ALL
        .into_iter()
        .zip(view.labels)
        .zip(&inputs.amounts)
        .zip(view.fields)
    {
        let overridden = view.overrides.get(component);
        let used = view.overrides.used(component, view.computed.as_ref());
        let computed_text = view
            .computed
            .as_ref()
            .map(|computed| format!("Computed ₱{}", official_amount(computed.get(component))))
            .unwrap_or_else(|| "Not computed".to_string());
        let used_text = if overridden.is_some() {
            format!("Using ₱{} (override)", official_amount(used))
        } else {
            format!("Using ₱{}", official_amount(used))
        };
        let reset = (editable && overridden.is_some()).then(|| {
            let section = section.clone();
            Button::new(SharedString::from(format!("{field}-use-computed-{index}")))
                .label("Use computed")
                .ghost()
                .on_click(move |_, window, cx| {
                    section.update(cx, |section, cx| {
                        section.reset(index, component, window, cx)
                    });
                })
        });
        let row = div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{computed_text} · {used_text}"))
                    .children(reset),
            )
            .child(Input::new(input).disabled(!editable));
        party = party.child(with_field_error(
            row.into_any_element(),
            messages_for(errors.iter(), &[field]),
        ));
    }
    party
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Override note / reference"),
                )
                .child(Input::new(&inputs.note).disabled(!editable)),
        )
        .into_any_element()
}

/// Copy the section's overrides into each party's state on `draft`, and
/// return any amount that did not parse. Call before the form recomputes.
pub fn apply_overrides<F>(
    section: &Entity<PenaltySection>,
    items: &[PenaltyItems<F>],
    draft: &mut F,
    cx: &App,
) -> Vec<(String, String)> {
    let views: Vec<PenaltyPartyView> = items
        .iter()
        .map(|items| PenaltyPartyView::of(items, draft))
        .collect();
    let (overrides, errors) = section.read(cx).overrides(&views, cx);
    for (items, overrides) in items.iter().zip(overrides) {
        (items.state_mut)(draft).penalty_overrides = overrides;
    }
    errors
}
