//! Shared editor plumbing for the event-based returns on the generic
//! submission path (1706, 1707, 1707A, 1800, 1801): responsive layout,
//! labelled inputs, choice buttons, the toolbar, and the save / queue /
//! cancel / release flows over `bir_core::db::queueable`. Each form's view
//! supplies its fields and sections; Rust core owns every calculation,
//! validation and the official submit plaintext.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::QueueableFormEvent;

/// Width classes the page lays out for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Phone,
    Tablet,
    Desktop,
}

impl Layout {
    pub fn for_width(width: Pixels) -> Self {
        if width < px(700.) {
            Self::Phone
        } else if width < px(1100.) {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }
}

/// Accepts `1,234.56`, `1234.5`; blank is zero.
pub fn parse_amount(value: &str) -> Option<f64> {
    match parse_optional_amount(value) {
        Ok(value) => Some(value.unwrap_or(0.0)),
        Err(()) => None,
    }
}

/// Accepts `1,234.56`, `1234.5`; blank stays blank (`Ok(None)`).
pub fn parse_optional_amount(value: &str) -> Result<Option<f64>, ()> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Ok(None);
    }
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Some)
        .ok_or(())
}

/// Editor text for an amount: blank for zero.
pub fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

/// Editor text for an optional amount: blank stays blank.
pub fn optional_money_text(value: Option<f64>) -> String {
    value.map(official_amount).unwrap_or_default()
}

pub fn format_tin(tin: &str) -> String {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 9 {
        return tin.to_string();
    }
    let branch = digits.get(9..).unwrap_or("");
    format!(
        "{}-{}-{}-{:0>5}",
        &digits[0..3],
        &digits[3..6],
        &digits[6..9],
        branch
    )
}

/// One editor input: (key, placeholder, initial text).
pub type InputSpec = (String, String, String);

/// Editors, messages and flags every event-form view keeps.
pub struct EditorState {
    pub inputs: BTreeMap<String, Entity<InputState>>,
    pub validation_errors: Vec<(String, String)>,
    pub parse_errors: Vec<(String, String)>,
    pub status_message: Option<String>,
    pub release_claim_confirm_open: bool,
    pub scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EditorState {
    pub fn new<V: KitView>(
        specs: Vec<InputSpec>,
        validation_errors: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (key, placeholder, value) in specs {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut V, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        sync(this, cx);
                    }
                },
            ));
            inputs.insert(key, input);
        }
        Self {
            inputs,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn has_error(&self, key: &str) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| {
                field == key
                    || field.starts_with(&format!("{key}["))
                    || field.starts_with(&format!("{key}."))
            })
    }
}

/// Collects editor text into draft values, recording malformed numbers.
pub struct InputReader<'a> {
    state: &'a EditorState,
    cx: &'a App,
    pub errors: Vec<(String, String)>,
}

impl<'a> InputReader<'a> {
    pub fn new(state: &'a EditorState, cx: &'a App) -> Self {
        Self {
            state,
            cx,
            errors: Vec::new(),
        }
    }

    pub fn text(&self, key: &str) -> String {
        self.state.text(key, self.cx)
    }

    pub fn trimmed(&self, key: &str) -> String {
        self.text(key).trim().to_string()
    }

    /// An amount; blank is zero. `None` when malformed (reported).
    pub fn amount(&mut self, key: &str, label: &str) -> Option<f64> {
        let text = self.text(key);
        let parsed = parse_amount(&text);
        if parsed.is_none() {
            self.errors.push((
                key.to_string(),
                format!("{label}: \"{text}\" is not a number."),
            ));
        }
        parsed
    }

    /// An optional amount; blank is `Some(None)`. `None` when malformed.
    pub fn optional_amount(&mut self, key: &str, label: &str) -> Option<Option<f64>> {
        let text = self.text(key);
        match parse_optional_amount(&text) {
            Ok(value) => Some(value),
            Err(()) => {
                self.errors.push((
                    key.to_string(),
                    format!("{label}: \"{text}\" is not a number."),
                ));
                None
            }
        }
    }

    /// A whole number in `0..=max`; blank is zero.
    pub fn whole<T: TryFrom<u64> + Default>(
        &mut self,
        key: &str,
        label: &str,
        max: u64,
    ) -> Option<T> {
        let text = self.trimmed(key);
        if text.is_empty() {
            return Some(T::default());
        }
        match text.parse::<u64>() {
            Ok(value) if value <= max => T::try_from(value).ok(),
            _ => {
                self.errors.push((
                    key.to_string(),
                    format!("{label}: \"{text}\" is not a whole number up to {max}."),
                ));
                None
            }
        }
    }
}

/// A view built on this kit.
pub trait KitView:
    FormViewTrait + EventEmitter<QueueableFormEvent> + Render + Sized + 'static
{
    type Draft: QueueableForm;
    /// Lowercase code in element ids (`<slug>_save`), e.g. `"1707a"`.
    const SLUG: &'static str;
    /// Form code shown in messages, e.g. `"1707A"`.
    const CODE: &'static str;

    fn kit(&self) -> &EditorState;
    fn kit_mut(&mut self) -> &mut EditorState;
    fn draft(&self) -> &Self::Draft;
    fn draft_mut(&mut self) -> &mut Self::Draft;
    fn db(&self) -> Arc<Mutex<Database>>;
    fn set_draft_id(&mut self, id: i64);
    /// Copy every editor into the draft; returns the malformed numbers.
    fn read_inputs(&mut self, cx: &mut Context<Self>) -> Vec<(String, String)>;
    /// Editor text for `key` from the draft.
    fn initial_text(&self, key: &str) -> String;
}

fn element_id(slug: &str, suffix: &str) -> ElementId {
    ElementId::Name(format!("{slug}_{suffix}").into())
}

/// Read every editor into the draft, then recompute and validate.
pub fn sync<V: KitView>(this: &mut V, cx: &mut Context<V>) {
    if !this.draft().lifecycle().is_editable() {
        return;
    }
    let parse_errors = this.read_inputs(cx);
    let draft = this.draft_mut();
    draft.compute();
    draft.lifecycle_mut().updated_at = chrono::Utc::now().to_rfc3339();
    let errors = QueueableForm::validate(this.draft());
    let kit = this.kit_mut();
    kit.validation_errors = errors;
    kit.parse_errors = parse_errors;
    cx.notify();
}

/// Apply a choice to the draft, recompute and validate.
pub fn edit<V: KitView>(this: &mut V, cx: &mut Context<V>, change: impl FnOnce(&mut V::Draft)) {
    if !this.draft().lifecycle().is_editable() {
        return;
    }
    change(this.draft_mut());
    this.draft_mut().compute();
    let errors = QueueableForm::validate(this.draft());
    let kit = this.kit_mut();
    kit.validation_errors = errors;
    kit.status_message = None;
    cx.notify();
}

/// Apply a choice that may clear entered values, then reload every editor.
pub fn edit_and_reload<V: KitView>(
    this: &mut V,
    window: &mut Window,
    cx: &mut Context<V>,
    change: impl FnOnce(&mut V::Draft),
) {
    edit(this, cx, change);
    reload_inputs(this, window, cx);
}

/// Reset every editor's text from the draft.
pub fn reload_inputs<V: KitView>(this: &mut V, window: &mut Window, cx: &mut Context<V>) {
    let updates: Vec<(Entity<InputState>, String)> = this
        .kit()
        .inputs
        .iter()
        .map(|(key, input)| (input.clone(), this.initial_text(key)))
        .collect();
    for (input, value) in updates {
        input.update(cx, |state, cx| state.set_value(value, window, cx));
    }
}

fn notify<V: KitView>(
    kind: notification::NotificationType,
    message: String,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    window.push_notification(
        notification::Notification::new()
            .message(message)
            .with_type(kind)
            .autohide(true),
        cx,
    );
}

pub fn save<V: KitView>(this: &mut V, window: &mut Window, cx: &mut Context<V>) {
    sync(this, cx);
    if !this.kit().parse_errors.is_empty() {
        notify::<V>(
            notification::NotificationType::Error,
            "Fix the highlighted numbers before saving.".into(),
            window,
            cx,
        );
        return;
    }
    let result = match this.db().lock() {
        Ok(db) => db
            .save_queueable_draft(this.draft())
            .map_err(|e| e.to_string()),
        Err(error) => Err(error.to_string()),
    };
    match result {
        Ok(id) => {
            this.set_draft_id(id);
            notify::<V>(
                notification::NotificationType::Success,
                format!("{} draft saved.", V::CODE),
                window,
                cx,
            );
            cx.emit(QueueableFormEvent::Saved);
        }
        Err(error) => cx.emit(QueueableFormEvent::PushNotification(
            "error".into(),
            "Save failed".into(),
            error,
        )),
    }
}

pub fn queue<V: KitView>(this: &mut V, window: &mut Window, cx: &mut Context<V>) {
    let code = <V::Draft as QueueableForm>::FORM_CODE;
    if !can_queue_for_submission(code) {
        this.kit_mut().status_message = Some(format!(
            "{} is not enabled for in-app submission in this build.",
            V::CODE
        ));
        cx.notify();
        return;
    }
    if !this.draft().lifecycle().is_editable() {
        this.kit_mut().status_message =
            Some("This return is already queued or filed and cannot be queued again.".into());
        cx.notify();
        return;
    }
    sync(this, cx);
    if !this.kit().parse_errors.is_empty() || !this.kit().validation_errors.is_empty() {
        this.kit_mut().status_message =
            Some("Fix the items listed under Needs review before submitting.".into());
        cx.notify();
        return;
    }
    let before = this.draft().clone();
    if let Err(errors) = this.draft_mut().queue(QueueAuthSource::Gui) {
        let kit = this.kit_mut();
        kit.validation_errors = errors;
        kit.status_message =
            Some("Fix the items listed under Needs review before submitting.".into());
        cx.notify();
        return;
    }
    let saved = match this.db().lock() {
        Ok(db) => db
            .save_queued_queueable(this.draft())
            .map_err(|e| e.to_string()),
        Err(error) => Err(error.to_string()),
    };
    if let Err(error) = saved {
        *this.draft_mut() = before;
        this.kit_mut().status_message = Some(format!(
            "Could not queue Form {}. No submission was started: {error}",
            V::CODE
        ));
        cx.notify();
        return;
    }
    let filename = this.draft().submission_filename();
    this.kit_mut().status_message =
        Some(format!("Queued for background submission as {filename}."));
    notify::<V>(
        notification::NotificationType::Success,
        format!("Form {} queued.", V::CODE),
        window,
        cx,
    );
    cx.emit(QueueableFormEvent::Saved);
    bir_core::background_cron::wake();
    cx.notify();
}

pub fn cancel<V: KitView>(this: &mut V, window: &mut Window, cx: &mut Context<V>) {
    if !matches!(this.draft().lifecycle().status, FilingStatus::Queued) {
        this.kit_mut().status_message =
            Some("This return cannot be reverted after submission has started.".into());
        cx.notify();
        return;
    }
    if !this.draft().lifecycle().is_unclaimed() {
        let kit = this.kit_mut();
        kit.release_claim_confirm_open = true;
        kit.status_message = Some(
            "Submission was claimed. Confirm nothing reached BIR to return it to an editable Draft. This does not file.".into(),
        );
        cx.notify();
        return;
    }
    let queued = this.draft().clone();
    let canceled = match this.db().lock() {
        Ok(db) => db
            .cancel_queued_queueable(&queued)
            .map_err(|e| e.to_string()),
        Err(error) => Err(error.to_string()),
    };
    match canceled {
        Ok(draft) => {
            *this.draft_mut() = draft;
            let errors = QueueableForm::validate(this.draft());
            let kit = this.kit_mut();
            kit.status_message = None;
            kit.validation_errors = errors;
            cx.emit(QueueableFormEvent::Saved);
        }
        Err(error) => {
            if let Ok(db) = this.db().lock()
                && let Ok(Some(current)) = db.get_queueable_draft::<V::Draft>(
                    queued.tin(),
                    queued.taxable_year(),
                    period_column(&queued.filing_period()),
                )
            {
                *this.draft_mut() = current;
            }
            notify::<V>(
                notification::NotificationType::Warning,
                format!("The queued return was not canceled: {error}"),
                window,
                cx,
            );
        }
    }
    cx.notify();
}

pub fn release_claim<V: KitView>(this: &mut V, window: &mut Window, cx: &mut Context<V>) {
    let (tin, year, column) = {
        let draft = this.draft();
        (
            draft.tin().to_string(),
            draft.taxable_year(),
            period_column(&draft.filing_period()),
        )
    };
    let result = match this.db().lock() {
        Ok(db) => db
            .release_abandoned_claimed_queueable::<V::Draft>(
                &tin,
                year,
                column,
                ABANDONED_CLAIM_RELEASE_REASON,
            )
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    this.kit_mut().release_claim_confirm_open = false;
    match result {
        Ok(AbandonedClaimRelease::Released { draft, .. }) => {
            *this.draft_mut() = draft;
            this.kit_mut().status_message = None;
            cx.emit(QueueableFormEvent::Saved);
        }
        Ok(AbandonedClaimRelease::AlreadyClear { draft, .. }) => {
            if let Some(draft) = draft {
                *this.draft_mut() = draft;
            }
        }
        Err(error) => {
            notify::<V>(
                notification::NotificationType::Error,
                format!("Could not release the claim: {error}"),
                window,
                cx,
            );
        }
    }
    let errors = QueueableForm::validate(this.draft());
    this.kit_mut().validation_errors = errors;
    cx.notify();
}

/// Print preview from the frozen official HTML, when this build has it.
pub fn preview<V: KitView>(this: &mut V, frozen_slug: &str, cx: &mut Context<V>) {
    sync(this, cx);
    if !this.kit().parse_errors.is_empty() {
        cx.emit(QueueableFormEvent::PushNotification(
            "error".into(),
            "Print preview failed".into(),
            "Fix the highlighted numbers first. No filing state was changed.".into(),
        ));
        return;
    }
    let fields = this.draft().field_map();
    let title = format!("{} — Print Preview", V::CODE);
    match super::form_html_preview_launcher::launch_frozen_form_preview(
        frozen_slug,
        &fields,
        &title,
        cx,
    ) {
        Ok(kind) => cx.emit(QueueableFormEvent::PushNotification(
            "info".into(),
            "Print preview".into(),
            format!("{} No filing state was changed.", kind.status_message()),
        )),
        Err(error) => cx.emit(QueueableFormEvent::PushNotification(
            "error".into(),
            "Print preview failed".into(),
            format!("{error}. No filing state was changed."),
        )),
    }
}

// ── Rendering helpers ──

pub fn section<V: 'static>(title: &str, children: Vec<AnyElement>, cx: &Context<V>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded_lg()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .child(title.to_string()),
        )
        .children(children)
        .into_any_element()
}

pub fn label(text: &str) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(text.to_string())
}

/// A labelled input. On phones the label sits above the field.
pub fn field(
    state: &EditorState,
    key: &str,
    label_text: &str,
    layout: Layout,
    disabled: bool,
) -> AnyElement {
    let input = state
        .inputs
        .get(key)
        .expect("editor input registry is complete");
    let body = div().child(Input::new(input).disabled(disabled));
    let caption = label(label_text);
    let caption = if state.has_error(key) {
        caption.text_color(gpui::red())
    } else {
        caption
    };
    match layout {
        Layout::Phone => div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .child(caption)
            .child(body.w_full()),
        _ => div()
            .flex()
            .items_center()
            .gap_4()
            .w_full()
            .child(caption.w(relative(0.45)))
            .child(body.w(relative(0.55))),
    }
    .into_any_element()
}

/// Two columns on desktop, one otherwise.
pub fn grid(layout: Layout, children: Vec<AnyElement>) -> AnyElement {
    if layout == Layout::Desktop {
        let mut rows = div().flex().flex_col().gap_3().w_full();
        let mut iter = children.into_iter();
        while let Some(left) = iter.next() {
            let mut row = div()
                .flex()
                .gap_6()
                .w_full()
                .child(div().flex_1().min_w_0().child(left));
            row = match iter.next() {
                Some(right) => row.child(div().flex_1().min_w_0().child(right)),
                None => row.child(div().flex_1()),
            };
            rows = rows.child(row);
        }
        rows.into_any_element()
    } else {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .children(children)
            .into_any_element()
    }
}

/// A computed amount, read-only.
pub fn computed<V: 'static>(label_text: &str, value: f64, cx: &Context<V>) -> AnyElement {
    computed_text(label_text, official_amount(value), cx)
}

pub fn computed_text<V: 'static>(label_text: &str, value: String, cx: &Context<V>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .p_2()
        .bg(cx.theme().muted.opacity(0.5))
        .rounded_md()
        .child(label(label_text))
        .child(
            div()
                .text_right()
                .font_weight(FontWeight::BOLD)
                .child(value),
        )
        .into_any_element()
}

pub fn choice(
    id: impl Into<ElementId>,
    label_text: impl Into<SharedString>,
    selected: bool,
    disabled: bool,
) -> Button {
    let label_text: SharedString = label_text.into();
    let button = Button::new(id).label(if selected {
        format!("✓ {label_text}")
    } else {
        label_text.to_string()
    });
    let button = if selected {
        button.primary()
    } else {
        button.outline()
    };
    button.disabled(disabled)
}

/// A caption followed by a wrapping row of choice buttons.
pub fn choice_row(caption: &str, buttons: Vec<Button>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(label(caption))
        .child(div().flex().flex_wrap().gap_2().children(buttons))
        .into_any_element()
}

/// Yes / No buttons for an optional answer; `on_pick` gets the answer.
pub fn yes_no<V: KitView>(
    id: &'static str,
    caption: &str,
    answer: Option<bool>,
    disabled: bool,
    cx: &Context<V>,
    on_pick: fn(&mut V::Draft, bool),
) -> AnyElement {
    choice_row(
        caption,
        vec![
            choice((id, 1usize), "Yes", answer == Some(true), disabled).on_click(
                cx.listener(move |this: &mut V, _, _, cx| edit(this, cx, |d| on_pick(d, true))),
            ),
            choice((id, 0usize), "No", answer == Some(false), disabled).on_click(
                cx.listener(move |this: &mut V, _, _, cx| edit(this, cx, |d| on_pick(d, false))),
            ),
        ],
    )
}

/// The whole page: toolbar, header, status pipeline, sections and the
/// "Needs review" list.
pub fn render_page<V: KitView>(
    this: &V,
    sections: Vec<AnyElement>,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Div {
    let layout = Layout::for_width(window.viewport_size().width);
    let slug = V::SLUG;
    let kit = this.kit();
    let is_draft = this.draft().lifecycle().is_editable();
    let is_queued = matches!(this.draft().lifecycle().status, FilingStatus::Queued);
    let pad = match layout {
        Layout::Phone => px(12.),
        Layout::Tablet => px(20.),
        Layout::Desktop => px(32.),
    };
    let issues: Vec<AnyElement> = kit
        .parse_errors
        .iter()
        .chain(kit.validation_errors.iter())
        .take(30)
        .map(|(_, message)| {
            div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(message.clone())
                .into_any_element()
        })
        .collect();
    let can_submit = is_draft && kit.validation_errors.is_empty() && kit.parse_errors.is_empty();

    let toolbar = div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .px(pad)
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Button::new(element_id(slug, "back"))
                .label("← Back")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard))),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    Button::new(element_id(slug, "preview"))
                        .label("Print preview")
                        .outline()
                        .on_click(
                            cx.listener(|this: &mut V, _, window, cx| this.preview_pdf(window, cx)),
                        ),
                )
                .child(
                    Button::new(element_id(slug, "save"))
                        .label("Save draft")
                        .outline()
                        .disabled(!is_draft)
                        .on_click(
                            cx.listener(|this: &mut V, _, window, cx| this.save_draft(window, cx)),
                        ),
                )
                .when(is_queued, |row| {
                    row.child(
                        Button::new(element_id(slug, "cancel"))
                            .label("Cancel queue")
                            .outline()
                            .on_click(cx.listener(|this: &mut V, _, window, cx| {
                                this.revert_to_draft(window, cx)
                            })),
                    )
                })
                .child(
                    Button::new(element_id(slug, "submit"))
                        .label("Queue for submission")
                        .primary()
                        .disabled(!can_submit)
                        .on_click(cx.listener(|this: &mut V, _, window, cx| {
                            this.mark_submitted(window, cx)
                        })),
                ),
        );

    let mut body = div()
        .w_full()
        .max_w(px(1180.))
        .mx_auto()
        .flex()
        .flex_col()
        .gap_5()
        .when_some(kit.status_message.clone(), |col, message| {
            col.child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .child(message),
            )
        })
        .when(kit.release_claim_confirm_open, |col| {
            col.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new(element_id(slug, "release_confirm"))
                            .label("Nothing reached BIR — release")
                            .danger()
                            .on_click(cx.listener(|this: &mut V, _, window, cx| {
                                release_claim(this, window, cx)
                            })),
                    )
                    .child(
                        Button::new(element_id(slug, "release_cancel"))
                            .label("Keep queued")
                            .outline()
                            .on_click(cx.listener(|this: &mut V, _, _, cx| {
                                this.kit_mut().release_claim_confirm_open = false;
                                cx.notify();
                            })),
                    ),
            )
        })
        .children(sections);
    if !issues.is_empty() {
        body = body.child(section("Needs review", issues, cx));
    }

    div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .bg(cx.theme().background)
        .child(toolbar)
        .child(
            div()
                .px(pad)
                .py_4()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(this.render_header(cx))
                .when(layout != Layout::Phone, |d| {
                    d.child(div().mt_4().child(this.render_status_pipeline(cx)))
                }),
        )
        .child(
            div()
                .id(element_id(slug, "scroll"))
                .flex_1()
                .w_full()
                .overflow_y_scroll()
                .track_scroll(&kit.scroll_handle)
                .p(pad)
                .child(body),
        )
}

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount, parse_optional_amount};
    use gpui::px;

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn amounts_accept_official_formatting() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(parse_optional_amount(""), Ok(None));
        assert_eq!(parse_optional_amount("0"), Ok(Some(0.0)));
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }
}
