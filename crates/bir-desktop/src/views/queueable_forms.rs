//! Editors for forms on the generic submission path
//! (`bir_core::forms::queueable`).
//!
//! Each such form has one view implementing [`QueueableFormView`] and one
//! entry in [`FORM_VIEW_SPECS`] (written with [`form_view_spec!`]). The app,
//! navigation and agent chrome all read that table, so adding a form never
//! touches `app.rs`, `agent/ids.rs` or `agent/host.rs`.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::queueable::{QueueableForm, QueueableKind};
use bir_core::profile::TaxpayerProfile;
use gpui::*;

use crate::agent::ids::FormChrome;
use crate::app::{ActiveView, AppState};

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

pub struct FormViewSpec {
    pub kind: QueueableKind,
    pub page_id: &'static str,
    pub slug: &'static str,
    pub title: &'static str,
    pub chrome: FormChrome,
    pub open: OpenFormView,
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
        }
    };
}

pub const FORM_VIEW_SPECS: &[FormViewSpec] = &[
    form_view_spec!(
        Form2553,
        super::form_2553_view::Form2553View,
        "2553",
        "2553"
    ),
    form_view_spec!(
        Form2200M,
        super::form_2200m_view::Form2200MView,
        "2200M",
        "2200m"
    ),
    form_view_spec!(
        Form2200A,
        super::form_2200a_view::Form2200AView,
        "2200A",
        "2200a"
    ),
    form_view_spec!(
        Form2200T,
        super::form_2200t_view::Form2200TView,
        "2200T",
        "2200t"
    ),
];

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
    let draft = match db.lock() {
        Ok(guard) => V::load_draft(&guard, profile, year, period),
        Err(_) => V::new_draft(profile, year, period),
    };
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
        assert_eq!(
            seen.len(),
            bir_core::forms::queueable::QueueableKind::ALL.len(),
            "every queueable form needs a desktop view spec"
        );
    }
}
