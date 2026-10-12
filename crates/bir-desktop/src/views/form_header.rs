//! Shared return-editor header: the `[Validate] [Submit] [⋯]` row, its ⋯
//! menu and the status box under the Draft→Queued→Submitted→Confirmed→Paid
//! stepper (owner decisions D1B–D8A).
//!
//! # Adopting it in a form view
//!
//! 1. Keep a [`FormHeaderState`] in the view (it owns the menu, the status
//!    box's dismiss state and the "View what was sent" snapshot). Call
//!    [`FormHeaderState::ensure_sent_snapshot`] in `render` and
//!    [`FormHeaderState::load_sent_snapshot`] after a revert.
//! 2. Track "validated at the current entries" (a flag cleared on every
//!    edit, set by the view's Validate).
//! 3. Each render, build a [`FormHeaderProps`] (status, editor safety,
//!    validated flag, blocking-error count, first error, the form's official
//!    success text, plus any other status message the view has) and a
//!    [`FormHeaderActions`] of plain `fn` callbacks.
//! 4. Put [`render_header_buttons`] on the right of the toolbar (next to
//!    "← Back") and [`render_status_box`] directly under
//!    `render_status_pipeline`. Show no other status text in the toolbar.
//! 5. `on_validate` must run the form's checks and set the validated flag;
//!    call [`FormHeaderState::reset_dismissal`] from it (the component also
//!    does when its own button is used). `on_submit` is the view's queue
//!    action (`mark_submitted`); it toasts only (D5C). `on_revert` should go
//!    through [`revert_for_amendment`] and, when a snapshot was taken, show
//!    [`AMENDED_REVERT_MESSAGE`].
//!
//! Visibility comes from [`header_visibility`], Submit's state from
//! [`submit_enabled`] and the box text from [`status_text`]; all three are
//! pure and unit-tested.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use bir_core::db::{AmendmentSnapshot, Database, DbError, RevertedForAmendment};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingPeriod, FilingStatus};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::*;

/// `settings` key for the "Separate Validate button" switch (D2A).
pub const SEPARATE_VALIDATE_SETTING: &str = "separate_validate_button";

/// Status box text after a Submitted/Confirmed return is reverted (D8A).
pub const AMENDED_REVERT_MESSAGE: &str =
    "Reverted to Draft as an amended return. A read-only copy of what BIR received is kept.";

/// Shown while Submit waits for Validate (separate Validate button on).
pub const CLICK_VALIDATE_MESSAGE: &str = "Click Validate to check this return before submitting.";

/// Status box text for a form that can't be queued (issue #87), e.g.
/// `queue_unavailable_message("2200-A")`. Shared by the 2200-A/AN/M/P views.
pub fn queue_unavailable_message(form_label: &str) -> String {
    format!(
        "In-app submission isn't available for {form_label} in this build. \
         Print this return and file it through eFPS or your RDO."
    )
}

/// The "Why?" disclosure under [`queue_unavailable_message`].
pub const QUEUE_UNAVAILABLE_WHY: &str = "BIR's own eBIRForms 7.9.6.2.1 has no upload folder for this form version, so returns can't be sent from this app yet.";

/// True when `text` came from [`queue_unavailable_message`].
pub fn is_queue_unavailable_message(text: &str) -> bool {
    text.starts_with("In-app submission isn't available for ")
        && text.ends_with(" in this build. Print this return and file it through eFPS or your RDO.")
}

/// Fallback when a form has no official success alert.
pub const DEFAULT_SUCCESS_MESSAGE: &str = "Validation successful.";

/// Status box text after the assistant brought a validated draft here
/// (`form.review_in_app`). Only the user's own Submit click files it.
pub const AGENT_REVIEW_MESSAGE: &str =
    "Prepared by the assistant. Review every box, then click Submit yourself.";

// 0 = not loaded, 1 = off, 2 = on.
static SEPARATE_VALIDATE: AtomicU8 = AtomicU8::new(0);

/// The "Separate Validate button" setting; On unless stored as "false".
pub fn separate_validate_button(db: &Arc<Mutex<Database>>) -> bool {
    match SEPARATE_VALIDATE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = db
                .lock()
                .ok()
                .and_then(|db| db.get_setting(SEPARATE_VALIDATE_SETTING).ok().flatten())
                .as_deref()
                != Some("false");
            SEPARATE_VALIDATE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// Store the "Separate Validate button" setting (Settings view).
pub fn set_separate_validate_button(db: &Arc<Mutex<Database>>, on: bool) {
    if let Ok(db) = db.lock() {
        let _ = db.set_setting(SEPARATE_VALIDATE_SETTING, if on { "true" } else { "false" });
    }
    SEPARATE_VALIDATE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

/// Which header controls show (D6A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderVisibility {
    pub validate: bool,
    pub submit: bool,
    pub revert: bool,
    pub save: bool,
    pub print: bool,
}

/// D6A matrix. `editor_safe` is false for a locked or unsafe editor, which
/// hides Validate and Submit.
pub fn header_visibility(status: &FilingStatus, editor_safe: bool) -> HeaderVisibility {
    let draft = matches!(status, FilingStatus::Draft);
    HeaderVisibility {
        validate: draft && editor_safe,
        submit: draft && editor_safe,
        revert: can_user_revert(status),
        save: draft,
        print: true,
    }
}

/// Issue #87: a form whose capability record says it can't be queued
/// never shows Submit (Validate stays). The code is the uppercased slug;
/// unknown codes keep Submit as before.
pub fn submit_hidden_for_slug(slug: &str) -> bool {
    bir_core::forms::find_form_capability(&slug.to_ascii_uppercase())
        .is_some_and(|record| !record.can_queue())
}

/// [`header_visibility`] for a form, with Submit hidden per
/// [`submit_hidden_for_slug`].
pub fn form_header_visibility(
    slug: &str,
    status: &FilingStatus,
    editor_safe: bool,
) -> HeaderVisibility {
    let mut visible = header_visibility(status, editor_safe);
    if submit_hidden_for_slug(slug) {
        visible.submit = false;
    }
    visible
}

/// D2A: with a separate Validate button, Submit needs a passing Validate at
/// the current entries; without it, Submit validates first itself.
pub fn submit_enabled(separate_validate: bool, validated: bool, blocking_errors: usize) -> bool {
    !separate_validate || (validated && blocking_errors == 0)
}

/// The status box text (D3A). `message` is any other status the view has
/// and wins; otherwise a Draft shows why Submit is off or the success text.
pub fn status_text(props: &FormHeaderProps<'_>, separate_validate: bool) -> Option<String> {
    if let Some(message) = &props.message {
        return Some(message.clone());
    }
    if !header_visibility(&props.status, props.editor_safe).submit {
        return None;
    }
    if props.blocking_errors > 0 {
        let first = props
            .first_error
            .clone()
            .unwrap_or_else(|| "This return needs review.".to_string());
        let more = props.blocking_errors - 1;
        return Some(if more > 0 {
            format!("{first} (+{more} more under Needs review)")
        } else {
            first
        });
    }
    if props.validated {
        return Some(props.success_text.to_string());
    }
    separate_validate.then(|| CLICK_VALIDATE_MESSAGE.to_string())
}

/// Whether the status box renders at all: only with non-blank text or an
/// open "what was sent" view, so no empty band sits under the stepper.
pub fn status_box_visible(text: Option<&str>, showing_sent: bool) -> bool {
    showing_sent || text.is_some_and(|t| !t.trim().is_empty())
}

/// Header state a view keeps: the ⋯ menu, the status box's dismiss state
/// and the latest snapshot of what BIR received.
#[derive(Default)]
pub struct FormHeaderState {
    menu_open: bool,
    dismissed: Option<String>,
    show_sent: bool,
    show_why: bool,
    sent: std::cell::OnceCell<Option<AmendmentSnapshot>>,
    /// Set by the assistant's `form.review_in_app`: the status box says so
    /// and Submit is highlighted until the user validates or submits.
    agent_review: bool,
}

impl FormHeaderState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the status box again (each Validate, D4A).
    pub fn reset_dismissal(&mut self) {
        self.dismissed = None;
    }

    pub fn close_menu(&mut self) {
        self.menu_open = false;
    }

    /// Mark the return as prepared by the assistant for the user's review.
    pub fn mark_agent_review(&mut self) {
        self.agent_review = true;
        self.dismissed = None;
    }

    pub fn agent_review(&self) -> bool {
        self.agent_review
    }

    /// Load the newest amendment snapshot of this return once (call from
    /// `render`; later calls are free).
    pub fn ensure_sent_snapshot<F: QueueableForm>(&self, db: &Arc<Mutex<Database>>, draft: &F) {
        self.sent.get_or_init(|| latest_snapshot(db, draft));
    }

    /// Reload the snapshot (after a revert took a new one).
    pub fn load_sent_snapshot<F: QueueableForm>(&mut self, db: &Arc<Mutex<Database>>, draft: &F) {
        self.sent = std::cell::OnceCell::from(latest_snapshot(db, draft));
    }

    /// [`Self::ensure_sent_snapshot`] for forms with their own storage
    /// (1601C, 2551Q): the return is named by form code, TIN, year and period.
    pub fn ensure_sent_snapshot_for(
        &self,
        db: &Arc<Mutex<Database>>,
        form_code: &str,
        tin: &str,
        taxable_year: u16,
        period: &FilingPeriod,
    ) {
        self.sent
            .get_or_init(|| latest_snapshot_for(db, form_code, tin, taxable_year, period));
    }

    /// [`Self::load_sent_snapshot`] for forms with their own storage.
    pub fn load_sent_snapshot_for(
        &mut self,
        db: &Arc<Mutex<Database>>,
        form_code: &str,
        tin: &str,
        taxable_year: u16,
        period: &FilingPeriod,
    ) {
        self.sent = std::cell::OnceCell::from(latest_snapshot_for(
            db,
            form_code,
            tin,
            taxable_year,
            period,
        ));
    }

    pub fn has_sent_snapshot(&self) -> bool {
        self.sent.get().is_some_and(Option::is_some)
    }
}

fn latest_snapshot<F: QueueableForm>(
    db: &Arc<Mutex<Database>>,
    draft: &F,
) -> Option<AmendmentSnapshot> {
    latest_snapshot_for(
        db,
        F::FORM_CODE,
        draft.tin(),
        draft.taxable_year(),
        &draft.filing_period(),
    )
}

fn latest_snapshot_for(
    db: &Arc<Mutex<Database>>,
    form_code: &str,
    tin: &str,
    taxable_year: u16,
    period: &FilingPeriod,
) -> Option<AmendmentSnapshot> {
    let period_key = period.to_period_key();
    db.lock().ok().and_then(|db| {
        db.list_amendment_snapshots(form_code, tin, taxable_year, &period_key)
            .ok()
            .and_then(|mut list| list.pop())
    })
}

/// D7A: revert through `Database::revert_queueable_for_amendment`.
pub fn revert_for_amendment<F: QueueableForm>(
    db: &Arc<Mutex<Database>>,
    draft: &F,
) -> Result<RevertedForAmendment<F>, String> {
    let guard = db.lock().map_err(|error| error.to_string())?;
    guard
        .revert_queueable_for_amendment::<F>(
            draft.tin(),
            draft.taxable_year(),
            period_column(&draft.filing_period()),
        )
        .map_err(|error: DbError| error.to_string())
}

/// What the view reports to the header each render.
#[derive(Clone)]
pub struct FormHeaderProps<'a> {
    /// Lowercase element-id prefix, e.g. `"2550q"`.
    pub slug: &'a str,
    pub status: FilingStatus,
    /// False for a locked or unsafe editor.
    pub editor_safe: bool,
    pub validated: bool,
    pub blocking_errors: usize,
    pub first_error: Option<String>,
    /// The form's official success alert, or [`DEFAULT_SUCCESS_MESSAGE`].
    pub success_text: &'a str,
    /// Any other status message (save result, lock reason, revert note…).
    pub message: Option<String>,
    pub separate_validate: bool,
}

/// A view action.
pub type Action<V> = fn(&mut V, &mut Window, &mut Context<V>);

/// The view's callbacks.
pub struct FormHeaderActions<V: 'static> {
    pub state: fn(&mut V) -> &mut FormHeaderState,
    pub state_ref: fn(&V) -> &FormHeaderState,
    /// Blocking errors right now (read after `on_validate` when Submit
    /// validates by itself).
    pub blocking_errors: fn(&V) -> usize,
    pub on_validate: Action<V>,
    pub on_submit: Action<V>,
    pub on_save: Action<V>,
    pub on_print: Action<V>,
    pub on_revert: Action<V>,
    /// Dev Mode "Simulate BIR confirmation" (debug builds, Submitted only).
    pub on_simulate: Option<Action<V>>,
}

impl<V: 'static> Clone for FormHeaderActions<V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V: 'static> Copy for FormHeaderActions<V> {}

fn id(slug: &str, suffix: &str) -> ElementId {
    ElementId::Name(format!("{slug}_{suffix}").into())
}

fn validate_then<V: 'static>(
    actions: FormHeaderActions<V>,
    this: &mut V,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    (actions.on_validate)(this, window, cx);
    let state = (actions.state)(this);
    state.reset_dismissal();
    // The user's own Validate takes over from the assistant's hand-over.
    state.agent_review = false;
    cx.notify();
}

/// `form.review_in_app`: run the page's own Validate (as its button does),
/// then mark the header so the status box says the assistant prepared it
/// and Submit is highlighted. Nothing is queued or submitted here.
pub fn agent_review<V: 'static>(
    this: &mut V,
    actions: FormHeaderActions<V>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    (actions.on_validate)(this, window, cx);
    (actions.state)(this).mark_agent_review();
    cx.notify();
}

/// The status box text while the assistant's hand-over stands: only on a
/// Draft that validates; otherwise the usual text (errors win).
pub fn agent_review_text(props: &FormHeaderProps<'_>, agent_review: bool) -> Option<String> {
    (agent_review
        && props.message.is_none()
        && props.blocking_errors == 0
        && header_visibility(&props.status, props.editor_safe).submit)
        .then(|| AGENT_REVIEW_MESSAGE.to_string())
}

/// The right side of the toolbar: `[Validate] [Submit] [⋯]` and the menu.
pub fn render_header_buttons<V: 'static>(
    this: &V,
    props: &FormHeaderProps<'_>,
    actions: FormHeaderActions<V>,
    phone: bool,
    cx: &mut Context<V>,
) -> AnyElement {
    let visible = form_header_visibility(props.slug, &props.status, props.editor_safe);
    let state = (actions.state_ref)(this);
    let separate = props.separate_validate;
    let enabled = submit_enabled(separate, props.validated, props.blocking_errors);
    let highlight = enabled && agent_review_text(props, state.agent_review).is_some();
    let ring = cx.theme().ring;
    let simulate = actions
        .on_simulate
        .filter(|_| matches!(props.status, FilingStatus::Submitted));

    // Same row look as the dashboard profile ⋯ menu (`header_menu_item`):
    // left-aligned text, px_3/py_2 padding, rounded_md, muted hover.
    let hover_bg = cx.theme().muted;
    let menu_item = move |key: &str, text: &str| {
        div()
            .id(id(props.slug, key))
            .w_full()
            .px_3()
            .py_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .child(text.to_string())
    };
    let mut items: Vec<AnyElement> = Vec::new();
    if visible.save {
        items.push(
            menu_item("save", "Save draft")
                .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                    (actions.state)(this).close_menu();
                    (actions.on_save)(this, window, cx);
                }))
                .into_any_element(),
        );
    }
    if visible.print {
        items.push(
            menu_item("preview", "Print preview")
                .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                    (actions.state)(this).close_menu();
                    (actions.on_print)(this, window, cx);
                }))
                .into_any_element(),
        );
    }
    if visible.revert {
        items.push(
            menu_item("revert", "Revert to Draft")
                .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                    (actions.state)(this).close_menu();
                    (actions.on_revert)(this, window, cx);
                    (actions.state)(this).reset_dismissal();
                    cx.notify();
                }))
                .into_any_element(),
        );
    }
    if state.has_sent_snapshot() {
        items.push(
            menu_item("view_sent", "View what was sent")
                .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                    let state = (actions.state)(this);
                    state.close_menu();
                    state.show_sent = true;
                    cx.notify();
                }))
                .into_any_element(),
        );
    }
    #[cfg(not(debug_assertions))]
    let _ = simulate;
    #[cfg(debug_assertions)]
    if let Some(simulate) = simulate {
        items.push(
            crate::dev_mode::simulate_confirmation_button(id(
                props.slug,
                "dev_simulate_confirmation",
            ))
            .w_full()
            .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                (actions.state)(this).close_menu();
                simulate(this, window, cx);
            }))
            .into_any_element(),
        );
    }

    let menu_open = state.menu_open;
    let menu = div()
        .relative()
        .child(
            Button::new(id(props.slug, "more"))
                .label("⋯")
                .outline()
                .when(phone, |b| b.small())
                .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                    let state = (actions.state)(this);
                    state.menu_open = !state.menu_open;
                    cx.notify();
                })),
        )
        .when(menu_open, |d| {
            d.child(deferred(
                div()
                    .occlude()
                    .absolute()
                    .top_full()
                    .right_0()
                    .mt_1p5()
                    .w(px(240.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_lg()
                    .rounded_xl()
                    .bg(cx.theme().popover)
                    .text_color(cx.theme().popover_foreground)
                    .flex()
                    .flex_col()
                    .p_1()
                    .children(items),
            ))
        });

    div()
        .flex()
        .flex_none()
        .items_center()
        .when(phone, |d| d.gap_1())
        .when(!phone, |d| d.gap_2())
        .when(visible.validate && separate, |row| {
            row.child(
                Button::new(id(props.slug, "validate"))
                    .label("Validate")
                    .outline()
                    .when(phone, |b| b.small())
                    .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                        validate_then(actions, this, window, cx)
                    })),
            )
        })
        .when(visible.submit, |row| {
            let submit = Button::new(id(props.slug, "submit"))
                .label("Submit")
                .primary()
                .when(phone, |b| b.small())
                .disabled(!enabled)
                .on_click(cx.listener(move |this: &mut V, _, window, cx| {
                    (actions.state)(this).agent_review = false;
                    if !separate {
                        // Validation always runs before submit (D2A).
                        validate_then(actions, this, window, cx);
                        if (actions.blocking_errors)(this) > 0 {
                            return;
                        }
                    }
                    (actions.on_submit)(this, window, cx);
                }));
            // The assistant's hand-over: a ring around Submit, which the
            // user still has to click themselves.
            row.child(
                div()
                    .id(id(props.slug, "submit_highlight"))
                    .rounded_md()
                    .when(highlight, |d| d.p_0p5().border_2().border_color(ring))
                    .child(submit),
            )
        })
        .child(menu)
        .into_any_element()
}

/// The status box under the stepper, plus the read-only "what was sent"
/// view when open. `None` when there is nothing to show.
pub fn render_status_box<V: 'static>(
    this: &V,
    props: &FormHeaderProps<'_>,
    actions: FormHeaderActions<V>,
    cx: &mut Context<V>,
) -> Option<AnyElement> {
    let state = (actions.state_ref)(this);
    let text = agent_review_text(props, state.agent_review)
        .or_else(|| status_text(props, props.separate_validate))
        .filter(|text| state.dismissed.as_deref() != Some(text.as_str()))
        .filter(|text| status_box_visible(Some(text), false));
    let sent = state
        .show_sent
        .then(|| state.sent.get().and_then(Option::as_ref))
        .flatten()
        .map(|snapshot| {
            snapshot
                .submitted_xml
                .clone()
                .unwrap_or_else(|| "The XML BIR received is not available for this copy.".into())
        });
    if !status_box_visible(text.as_deref(), sent.is_some()) {
        return None;
    }
    let is_error = props.message.is_none() && props.blocking_errors > 0;
    let mut col = div().flex().flex_col().gap_2().w_full().min_w_0();
    if let Some(text) = text {
        let dismissed_text = text.clone();
        col = col.child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .w_full()
                .p_3()
                .rounded_md()
                .text_sm()
                .when(is_error, |d| {
                    d.border_1()
                        .border_color(cx.theme().danger)
                        .bg(cx.theme().danger.opacity(0.08))
                })
                .when(!is_error, |d| d.bg(cx.theme().muted.opacity(0.6)))
                .child({
                    let why = is_queue_unavailable_message(&text);
                    let show_why = state.show_why;
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(text.clone())
                        .when(why, |d| {
                            d.child(
                                div().child(
                                    Button::new(id(props.slug, "status_why"))
                                        .label(if show_why { "Why? ▾" } else { "Why? ▸" })
                                        .ghost()
                                        .xsmall()
                                        .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                                            let st = (actions.state)(this);
                                            st.show_why = !st.show_why;
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when(show_why, |d| {
                                d.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(QUEUE_UNAVAILABLE_WHY),
                                )
                            })
                        })
                })
                .child(
                    Button::new(id(props.slug, "status_dismiss"))
                        .label("×")
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                            (actions.state)(this).dismissed = Some(dismissed_text.clone());
                            cx.notify();
                        })),
                ),
        );
    }
    if let Some(xml) = sent {
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_sm()
                                .child("What was sent (read-only)"),
                        )
                        .child(
                            Button::new(id(props.slug, "view_sent_close"))
                                .label("Close")
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                                    (actions.state)(this).show_sent = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .id(id(props.slug, "view_sent_scroll"))
                        .max_h(px(320.))
                        .overflow_scroll()
                        .font_family("monospace")
                        .text_xs()
                        .child(xml),
                ),
        );
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::{AGENT_REVIEW_MESSAGE, FormHeaderState, agent_review_text};
    use super::{
        AMENDED_REVERT_MESSAGE, CLICK_VALIDATE_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderProps,
        HeaderVisibility, form_header_visibility, header_visibility, status_box_visible,
        status_text, submit_enabled, submit_hidden_for_slug,
    };
    use super::{is_queue_unavailable_message, queue_unavailable_message};

    #[test]
    fn agent_review_note_shows_only_on_a_clean_draft() {
        let mut state = FormHeaderState::new();
        assert!(!state.agent_review());
        state.mark_agent_review();
        assert!(state.agent_review());
        let mut clean = props(FilingStatus::Draft);
        clean.validated = true;
        assert_eq!(
            agent_review_text(&clean, true).as_deref(),
            Some(AGENT_REVIEW_MESSAGE)
        );
        assert_eq!(agent_review_text(&clean, false), None);
        let mut errors = clean.clone();
        errors.blocking_errors = 1;
        assert_eq!(agent_review_text(&errors, true), None);
        // Never on a return that is no longer a Draft.
        assert_eq!(agent_review_text(&props(FilingStatus::Queued), true), None);
    }

    #[test]
    fn queue_unavailable_text_names_the_form_and_next_step() {
        let text = queue_unavailable_message("2200-AN");
        assert_eq!(
            text,
            "In-app submission isn't available for 2200-AN in this build. Print this return and file it through eFPS or your RDO."
        );
        assert!(is_queue_unavailable_message(&text));
        assert!(!is_queue_unavailable_message("Saved."));
    }
    use bir_core::forms::FilingStatus;

    fn props(status: FilingStatus) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "t",
            status,
            editor_safe: true,
            validated: false,
            blocking_errors: 0,
            first_error: None,
            success_text: DEFAULT_SUCCESS_MESSAGE,
            message: None,
            separate_validate: true,
        }
    }

    fn row(v: HeaderVisibility) -> [bool; 5] {
        [v.validate, v.submit, v.revert, v.save, v.print]
    }

    #[test]
    fn draft_shows_validate_submit_save_print() {
        assert_eq!(
            row(header_visibility(&FilingStatus::Draft, true)),
            [true, true, false, true, true]
        );
    }

    #[test]
    fn queued_shows_revert_and_print() {
        assert_eq!(
            row(header_visibility(&FilingStatus::Queued, true)),
            [false, false, true, false, true]
        );
    }

    #[test]
    fn submitted_shows_revert_and_print() {
        assert_eq!(
            row(header_visibility(&FilingStatus::Submitted, true)),
            [false, false, true, false, true]
        );
    }

    #[test]
    fn confirmed_shows_revert_and_print() {
        assert_eq!(
            row(header_visibility(&FilingStatus::Confirmed, true)),
            [false, false, true, false, true]
        );
    }

    #[test]
    fn paid_shows_print_only() {
        assert_eq!(
            row(header_visibility(&FilingStatus::Paid, true)),
            [false, false, false, false, true]
        );
    }

    #[test]
    fn unsafe_editor_hides_validate_and_submit() {
        let v = header_visibility(&FilingStatus::Draft, false);
        assert!(!v.validate && !v.submit && v.save && v.print);
    }

    #[test]
    fn unqueueable_forms_hide_submit_only() {
        assert!(submit_hidden_for_slug("2200a"));
        assert!(submit_hidden_for_slug("2200an"));
        assert!(!submit_hidden_for_slug("2200c"));
        assert!(!submit_hidden_for_slug("t"));
        let a = form_header_visibility("2200a", &FilingStatus::Draft, true);
        assert!(a.validate && !a.submit && a.save && a.print);
        assert_eq!(
            row(form_header_visibility("2200c", &FilingStatus::Draft, true)),
            row(header_visibility(&FilingStatus::Draft, true))
        );
        assert_eq!(
            row(form_header_visibility("t", &FilingStatus::Draft, true)),
            row(header_visibility(&FilingStatus::Draft, true))
        );
    }

    #[test]
    fn submit_waits_for_validate_only_when_separate() {
        assert!(!submit_enabled(true, false, 0));
        assert!(!submit_enabled(true, true, 2));
        assert!(submit_enabled(true, true, 0));
        assert!(submit_enabled(false, false, 3));
    }

    #[test]
    fn status_box_text_by_state() {
        let mut p = props(FilingStatus::Draft);
        assert_eq!(
            status_text(&p, true).as_deref(),
            Some(CLICK_VALIDATE_MESSAGE)
        );
        assert_eq!(status_text(&p, false), None);
        p.validated = true;
        assert_eq!(
            status_text(&p, true).as_deref(),
            Some(DEFAULT_SUCCESS_MESSAGE)
        );
        p.blocking_errors = 3;
        p.first_error = Some("Bad TIN.".into());
        assert_eq!(
            status_text(&p, true).as_deref(),
            Some("Bad TIN. (+2 more under Needs review)")
        );
        p.blocking_errors = 1;
        assert_eq!(status_text(&p, true).as_deref(), Some("Bad TIN."));
        p.message = Some(AMENDED_REVERT_MESSAGE.into());
        assert_eq!(
            status_text(&p, true).as_deref(),
            Some(AMENDED_REVERT_MESSAGE)
        );
        let q = props(FilingStatus::Queued);
        assert_eq!(status_text(&q, true), None);
    }

    #[test]
    fn status_box_collapses_without_a_message() {
        assert!(!status_box_visible(None, false));
        assert!(!status_box_visible(Some(""), false));
        assert!(!status_box_visible(Some("   "), false));
        assert!(status_box_visible(Some("Validation successful."), false));
        assert!(status_box_visible(None, true));
    }
}
