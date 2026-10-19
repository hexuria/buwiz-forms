//! Editors for forms on the generic submission path
//! (`bir_core::forms::queueable`).
//!
//! Each such form has one view implementing [`QueueableFormView`] and one
//! entry in [`FORM_VIEW_SPECS`] (written with [`form_view_spec!`]). The app,
//! navigation and agent chrome all read that table, so adding a form never
//! touches `app.rs`, `agent/ids.rs` or `agent/host.rs`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::queueable::{QueueableForm, QueueableKind};
use bir_core::profile::TaxpayerProfile;
use gpui::*;
use serde_json::Value;

use crate::agent::ids::FormChrome;
use crate::app::{ActiveView, AppState};
use crate::views::form_header::FormHeaderActions;

/// Events every queueable form view emits.
pub enum QueueableFormEvent {
    BackToDashboard,
    Saved,
    /// (level, title, message)
    PushNotification(String, String, String),
}

/// A form editor on the generic submission path.
pub trait QueueableFormView: Render + EventEmitter<QueueableFormEvent> + Sized + 'static {
    type Draft: QueueableForm;

    fn new(
        draft: Self::Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self;

    /// A fresh draft for the dashboard's `period` (month, quarter, `0` for
    /// annual, or the open-ended key).
    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Self::Draft;

    /// The draft to open: the saved one for this period, else a new one.
    fn load_draft(db: &Database, profile: &TaxpayerProfile, year: u16, period: u8) -> Self::Draft {
        db.get_queueable_draft::<Self::Draft>(&profile.tin.full(), year, i64::from(period))
            .ok()
            .flatten()
            .unwrap_or_else(|| Self::new_draft(profile, year, period))
    }

    /// Frozen print sheet (`bir_print::frozen_html`) the page's Print
    /// preview fills.
    const PRINT_SLUG: &'static str;

    /// The field map the page's Print preview fills [`Self::PRINT_SLUG`] with.
    fn print_fields(draft: &Self::Draft) -> BTreeMap<String, String>;

    /// The draft as the page holds it (every edit is synced into it).
    fn agent_draft(&self) -> &Self::Draft;

    /// The page's header callbacks (its own Validate among them).
    fn header_actions() -> FormHeaderActions<Self>;
}

/// Builds the view for one dashboard request and subscribes the app to it.
pub type OpenFormView = fn(
    &TaxpayerProfile,
    u16,
    u8,
    Arc<Mutex<Database>>,
    &mut Window,
    &mut Context<AppState>,
) -> AnyView;

/// The page's draft for `(profile, year, period)` as JSON, exactly as
/// [`QueueableFormView::load_draft`] opens it.
pub type LoadFormDraft = fn(&Database, &TaxpayerProfile, u16, u8) -> Value;

/// The page's print sheet slug and its filled field map for a JSON draft.
pub type PrintFormDraft = fn(&Value) -> Result<(&'static str, BTreeMap<String, String>), String>;

/// Builds the view around a given JSON draft (the assistant's edits).
pub type OpenFormWithDraft = fn(
    &Value,
    Arc<Mutex<Database>>,
    &mut Window,
    &mut Context<AppState>,
) -> Result<AnyView, String>;

/// The open view's draft as JSON.
pub type ReadFormView = fn(&AnyView, &App) -> Option<Value>;

/// Run the page's own Validate and mark the header for the user's review.
pub type ReviewFormView = fn(&AnyView, &mut Window, &mut App);

pub struct FormViewSpec {
    pub kind: QueueableKind,
    pub page_id: &'static str,
    pub slug: &'static str,
    pub title: &'static str,
    pub chrome: FormChrome,
    pub open: OpenFormView,
    pub load: LoadFormDraft,
    pub print: PrintFormDraft,
    pub open_with: OpenFormWithDraft,
    pub read: ReadFormView,
    pub review: ReviewFormView,
}

/// One entry per form: `form_view_spec!(Variant, ViewType, "code", "slug")`.
/// Element ids are `<slug>_back`, `<slug>_save` and `<slug>_submit`.
#[macro_export]
macro_rules! form_view_spec {
    ($variant:ident, $view:ty, $code:literal, $slug:literal) => {
        $crate::views::queueable_forms::FormViewSpec {
            kind: bir_core::forms::queueable::QueueableKind::$variant,
            page_id: concat!("page-form-", $slug),
            slug: concat!("form-", $slug),
            title: concat!("Form ", $code),
            chrome: $crate::agent::ids::FormChrome {
                view: $crate::app::ActiveView::Queueable(
                    bir_core::forms::queueable::QueueableKind::$variant,
                ),
                code: $code,
                back: concat!($slug, "_back"),
                save: concat!($slug, "_save"),
                submit: concat!($slug, "_submit"),
            },
            open: $crate::views::queueable_forms::open_form_view::<$view>,
            load: $crate::views::queueable_forms::load_draft_json::<$view>,
            print: $crate::views::queueable_forms::print_draft_json::<$view>,
            open_with: $crate::views::queueable_forms::open_form_view_with::<$view>,
            read: $crate::views::queueable_forms::read_view_draft::<$view>,
            review: $crate::views::queueable_forms::review_view::<$view>,
        }
    };
}

/// One line per form: `Variant, view type, "CODE", "slug";`.
macro_rules! form_view_specs {
    ($($variant:ident, $view:ty, $code:literal, $slug:literal;)*) => {
        pub const FORM_VIEW_SPECS: &[FormViewSpec] = &[$(form_view_spec!($variant, $view, $code, $slug),)*];
    };
}

form_view_specs! {
    Form2553, super::form_2553_view::Form2553View, "2553", "2553";
    Form1604F, super::form_1604f_view::Form1604fView, "1604F", "1604f";
    Form1604C, super::form_1604c_view::Form1604cView, "1604C", "1604c";
    Form1604E, super::form_1604e_view::Form1604eView, "1604E", "1604e";
    Form1606, super::form_1606_view::Form1606View, "1606", "1606";
    Form1600VT, super::form_1600vt_view::Form1600VtView, "1600VT", "1600vt";
    Form1600PT, super::form_1600pt_view::Form1600PtView, "1600PT", "1600pt";
    Form1600WP, super::form_1600wp_view::Form1600WpView, "1600WP", "1600wp";
    Form1601EQ, super::form_1601eq_view::Form1601EqView, "1601EQ", "1601eq";
    Form1601FQ, super::form_1601fq_view::Form1601FqView, "1601FQ", "1601fq";
    Form1603Q, super::form_1603q_view::Form1603QView, "1603Q", "1603q";
    Form1602Q, super::form_1602q_view::Form1602QView, "1602Q", "1602q";
    Form2552, super::form_2552_view::Form2552View, "2552", "2552";
    Form2550M, super::form_2550m_view::Form2550MView, "2550M", "2550m";
    Form1706, super::form_1706_view::Form1706View, "1706", "1706";
    Form1707, super::form_1707_view::Form1707View, "1707", "1707";
    Form1707A, super::form_1707a_view::Form1707AView, "1707A", "1707a";
    Form1800, super::form_1800_view::Form1800View, "1800", "1800";
    Form1801, super::form_1801_view::Form1801View, "1801", "1801";
    Form2000, super::form_2000_view::Form2000View, "2000", "2000";
    Form2000OT, super::form_2000ot_view::Form2000OTView, "2000OT", "2000ot";
    Form2200S, super::form_2200s_view::Form2200SView, "2200S", "2200s";
    Form2200C, super::form_2200c_view::Form2200CView, "2200C", "2200c";
    Form2200AN, super::form_2200an_view::Form2200ANView, "2200AN", "2200an";
    Form2200M, super::form_2200m_view::Form2200MView, "2200M", "2200m";
    Form2200P, super::form_2200p_view::Form2200PView, "2200P", "2200p";
    Form2200A, super::form_2200a_view::Form2200AView, "2200A", "2200a";
    Form2200T, super::form_2200t_view::Form2200TView, "2200T", "2200t";
    Form1701MS, super::form_1701ms_view::Form1701MsView, "1701MS", "1701ms";
    Form1702Q, super::form_1702q_view::Form1702qView, "1702Q", "1702q";
    Form1701A, super::form_1701a_view::Form1701AView, "1701A", "1701a";
    Form1700, super::form_1700_view::Form1700View, "1700", "1700";
    Form1702EX, super::form_1702ex_view::Form1702ExView, "1702EX", "1702ex";
    Form0605, super::form_0605_view::Form0605View, "0605", "0605";
    Form0619E, super::form_0619e_view::Form0619EView, "0619E", "0619e";
    Form0619F, super::form_0619f_view::Form0619FView, "0619F", "0619f";
    Form1701Q, super::form_1701q_view::Form1701QView, "1701Q", "1701q";
    Form1702RT, super::form_1702rt_view::Form1702RTView, "1702RT", "1702rt";
    Form1701, super::form_1701_view::Form1701View, "1701", "1701";
    Form1702MX, super::form_1702mx_view::Form1702MXView, "1702MX", "1702mx";
}

pub fn spec_for_kind(kind: QueueableKind) -> Option<&'static FormViewSpec> {
    FORM_VIEW_SPECS.iter().find(|spec| spec.kind == kind)
}

pub fn spec_for_code(code: &str) -> Option<&'static FormViewSpec> {
    FORM_VIEW_SPECS.iter().find(|spec| spec.chrome.code == code)
}

pub fn spec_for_view(view: ActiveView) -> Option<&'static FormViewSpec> {
    match view {
        ActiveView::Queueable(kind) => spec_for_kind(kind),
        _ => None,
    }
}

pub fn spec_for_slug(slug: &str) -> Option<&'static FormViewSpec> {
    FORM_VIEW_SPECS
        .iter()
        .find(|spec| spec.slug == slug || spec.slug.strip_prefix("form-") == Some(slug))
}

pub fn open_form_view<V: QueueableFormView>(
    profile: &TaxpayerProfile,
    year: u16,
    period: u8,
    db: Arc<Mutex<Database>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyView {
    let (mut draft, deadline_overrides) = match db.lock() {
        Ok(guard) => (
            V::load_draft(&guard, profile, year, period),
            guard.get_deadline_overrides(),
        ),
        Err(_) => (V::new_draft(profile, year, period), Vec::new()),
    };
    prepare_penalties::<V>(&mut draft, profile, &deadline_overrides);
    mount_view::<V>(draft, db, window, cx)
}

/// Penalties are measured from the deadline the Calendar shows (deadline
/// overrides applied) with the profile's EOPT class; an open Draft shows
/// today's computed values. Filed returns keep what they were filed with.
fn prepare_penalties<V: QueueableFormView>(
    draft: &mut V::Draft,
    profile: &TaxpayerProfile,
    deadline_overrides: &[bir_core::calendar_rules::DeadlineOverride],
) {
    if !V::Draft::penalty_items().is_empty() {
        draft.apply_penalty_profile(profile, deadline_overrides);
        if draft.lifecycle().is_editable() {
            draft.compute();
        }
    }
}

/// The record a draft's penalty resolution stands on: its typed-state
/// fields and the Calendar period each penalty item resolves from — a
/// typed transaction date moves it (decision P5-9). Kept by the view so a
/// sync resolves the due date again when the record moved since the open
/// did, never leaving the earlier state's date.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TriggerSignature {
    entered: bool,
    parts: Vec<String>,
    periods: Vec<Option<bir_core::calendar_rules::DeadlinePeriod>>,
}

/// `draft`'s [`TriggerSignature`] right now.
pub(crate) fn trigger_signature<D: QueueableForm>(draft: &D) -> TriggerSignature {
    let lifecycle = draft.lifecycle();
    TriggerSignature {
        entered: lifecycle.trigger_date_entered,
        parts: lifecycle.trigger_parts_typed.clone(),
        periods: D::penalty_items()
            .iter()
            .map(|items| (items.deadline_period)(draft))
            .collect(),
    }
}

/// When the record `draft`'s penalties resolve from moved since `resolved`
/// names it, resolve them again from the profile and the Calendar as the
/// open's [`prepare_penalties`] did (decision P5-9), and remember the new
/// signature. A form without penalty items, or a database that cannot be
/// read, leaves the draft as it is.
pub(crate) fn resolve_moved_trigger<D: QueueableForm>(
    draft: &mut D,
    resolved: &mut Option<TriggerSignature>,
    db: &Arc<Mutex<Database>>,
) {
    if D::penalty_items().is_empty() || !draft.lifecycle().is_editable() {
        return;
    }
    let signature = trigger_signature(draft);
    if resolved.as_ref() == Some(&signature) {
        return;
    }
    let Some((profile, overrides)) = db.lock().ok().and_then(|guard| {
        guard
            .get_profile_by_tin(draft.tin())
            .ok()
            .flatten()
            .map(|profile| (profile, guard.get_deadline_overrides()))
    }) else {
        return;
    };
    draft.apply_penalty_profile(&profile, &overrides);
    *resolved = Some(signature);
}

pub fn load_draft_json<V: QueueableFormView>(
    db: &Database,
    profile: &TaxpayerProfile,
    year: u16,
    period: u8,
) -> Value {
    let mut draft = V::load_draft(db, profile, year, period);
    prepare_penalties::<V>(&mut draft, profile, &db.get_deadline_overrides());
    serde_json::to_value(draft).expect("a form draft serializes to JSON")
}

pub fn print_draft_json<V: QueueableFormView>(
    draft: &Value,
) -> Result<(&'static str, BTreeMap<String, String>), String> {
    let typed: V::Draft = serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
    Ok((V::PRINT_SLUG, V::print_fields(&typed)))
}

/// The sheet slug and filled field map a form page's Print preview uses,
/// for every form the assistant can open: the 1601C and 2551Q pages, the
/// bespoke 2550Q page and each [`FORM_VIEW_SPECS`] page.
pub(crate) fn print_sheet(
    kind: AgentFormKind,
    draft: &Value,
) -> Result<(&'static str, BTreeMap<String, String>), String> {
    match kind {
        AgentFormKind::Form1601C => {
            let typed: bir_core::forms::form_1601c::Form1601CDraft =
                serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
            Ok(("1601c-2018", typed.to_print_field_map()))
        }
        AgentFormKind::Form2551Q => {
            let typed: bir_core::forms::form_2551q::Form2551QDraft =
                serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
            Ok(("2551q-2018", typed.to_print_field_map()))
        }
        AgentFormKind::Queueable(QueueableKind::Form2550Q) => {
            let typed: bir_core::forms::form_2550q::Form2550QDraft =
                serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
            Ok((
                crate::views::form_2550q_view::PRINT_SLUG,
                crate::views::form_2550q_view::print_fields(&typed),
            ))
        }
        AgentFormKind::Queueable(queueable) => {
            let spec = spec_for_kind(queueable).ok_or("this form has no page")?;
            (spec.print)(draft)
        }
    }
}

pub fn open_form_view_with<V: QueueableFormView>(
    draft: &Value,
    db: Arc<Mutex<Database>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Result<AnyView, String> {
    let typed: V::Draft = serde_json::from_value(draft.clone()).map_err(|e| e.to_string())?;
    Ok(mount_view::<V>(typed, db, window, cx))
}

pub fn read_view_draft<V: QueueableFormView>(view: &AnyView, cx: &App) -> Option<Value> {
    let entity = view.clone().downcast::<V>().ok()?;
    serde_json::to_value(entity.read(cx).agent_draft()).ok()
}

pub fn review_view<V: QueueableFormView>(view: &AnyView, window: &mut Window, cx: &mut App) {
    if let Ok(entity) = view.clone().downcast::<V>() {
        entity.update(cx, |this, cx| {
            crate::views::form_header::agent_review(this, V::header_actions(), window, cx);
        });
    }
}

fn mount_view<V: QueueableFormView>(
    draft: V::Draft,
    db: Arc<Mutex<Database>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyView {
    let view = cx.new(|cx| V::new(draft, Arc::clone(&db), window, cx));
    cx.subscribe_in(
        &view,
        window,
        |app: &mut AppState, _entity, event: &QueueableFormEvent, window, cx| match event {
            QueueableFormEvent::BackToDashboard => {
                app.active_view = ActiveView::Dashboard;
                cx.notify();
            }
            QueueableFormEvent::Saved => cx.notify(),
            QueueableFormEvent::PushNotification(level, title, message) => {
                crate::app::push_notification(level, title, message, window, cx);
            }
        },
    )
    .detach();
    view.into()
}

#[cfg(test)]
mod tests {
    use super::{FORM_VIEW_SPECS, spec_for_code, spec_for_slug};
    use std::collections::HashSet;

    #[test]
    fn specs_are_unique_and_match_core() {
        let mut seen = HashSet::new();
        for spec in FORM_VIEW_SPECS {
            assert!(seen.insert(spec.kind), "duplicate view for {:?}", spec.kind);
            assert_eq!(spec.chrome.code, spec.kind.form_code());
            assert_eq!(
                spec_for_code(spec.chrome.code).map(|s| s.slug),
                Some(spec.slug)
            );
            assert_eq!(spec_for_slug(spec.slug).map(|s| s.kind), Some(spec.kind));
        }
        // Kinds on the generic path that keep a bespoke page (routed in
        // app.rs before the spec lookup) and so have no FORM_VIEW_SPECS row.
        const BESPOKE: &[&str] = &["2550Q"];
        for code in BESPOKE {
            assert!(
                bir_core::forms::queueable::QueueableKind::ALL
                    .iter()
                    .any(|k| k.form_code() == *code),
                "bespoke form {code} is not a queueable kind"
            );
            assert!(
                spec_for_code(code).is_none(),
                "bespoke form {code} must not also have a generic spec"
            );
        }
        assert_eq!(
            seen.len() + BESPOKE.len(),
            bir_core::forms::queueable::QueueableKind::ALL.len(),
            "every queueable form needs a desktop view spec or a bespoke page"
        );
    }
}
