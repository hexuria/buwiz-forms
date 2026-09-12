//! Notifications — actionable application health events for the current profile.
//!
//! Conditions this app hit that the user can do something about: an expired
//! Google token, a profile row this build cannot read. Previously these only
//! reached the log file, so email polling could fail every 60 seconds for a week
//! with nothing visible anywhere in the UI.
//!
//! Scoped to the active taxpayer profile, plus application-wide alerts, which
//! are always shown — a broken connection affects the user whichever profile
//! happens to be selected.

use bir_core::db::{AlertAction, AlertSeverity, AppAlert, Database};
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};
use gpui_rsx::rsx;
use std::sync::{Arc, Mutex};

/// Raised when the user clicks an alert's action button.
///
/// The view does not navigate itself — it emits and lets the app shell route,
/// so this stays independent of how views are wired together.
pub enum NotificationsEvent {
    ReconnectGoogleAccount,
    OpenProfileManager,
    /// A toast or system banner for an alert was clicked: show this page.
    Open,
}

impl EventEmitter<NotificationsEvent> for NotificationsView {}

pub struct NotificationsView {
    db: Arc<Mutex<Database>>,
    /// `None` means no profile is selected; only application-wide alerts show.
    pub(crate) active_session_tin: Option<String>,
    alerts: Vec<AppAlert>,
    scroll_handle: ScrollHandle,
    /// Alert ids already shown as a toast / system banner. Seeded with what
    /// exists at startup so a launch does not replay history.
    announced: std::collections::HashSet<i64>,
}

impl NotificationsView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        // Alerts arrive from background work — the submission cron, the email
        // poll — not from user interaction, so nothing would otherwise prompt
        // a re-render. The cron posts `DatabaseChanged` after each job; the
        // timer is the fallback so the page cannot silently go stale.
        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        cx.subscribe_in(
            &bus,
            window,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, window, cx| {
                if matches!(event, crate::events::AppEvent::DatabaseChanged) {
                    this.refresh_and_announce(window, cx);
                }
            },
        )
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(15))
                    .await;
                if this
                    .update_in(cx, |view, window, cx| {
                        view.refresh_and_announce(window, cx);
                    })
                    .is_err()
                {
                    break; // view dropped
                }
            }
        })
        .detach();

        let mut view = Self {
            db,
            active_session_tin: None,
            alerts: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            announced: std::collections::HashSet::new(),
        };

        // Opt-in sample data for checking this page renders. Real alerts only
        // exist when something is broken, so verifying the layout otherwise
        // means waiting for a genuine failure — and an empty screen does not
        // distinguish "working" from "broken".
        //
        // Env-gated rather than automatic so ordinary debug runs stay clean,
        // and `debug_assertions` on the seeder means it cannot ship at all.
        #[cfg(debug_assertions)]
        if std::env::var_os("BIR_SEED_EXAMPLE_ALERTS").is_some()
            && let Ok(db) = view.db.lock()
        {
            let _ = db.seed_example_alerts(view.active_session_tin.as_deref());
        }

        view.reload();
        view.announced = view.alerts.iter().map(|alert| alert.id).collect();
        view
    }

    /// Re-read alerts and deliver the ones this session has not shown yet as
    /// an in-app toast plus a system notification.
    ///
    /// This is the painted app's desktop delivery. GPUI posts through
    /// `UNUserNotificationCenter` when the binary runs from a `.app` (identity,
    /// icon, permission prompt, click-to-open) and stays silent otherwise, so a
    /// bare `cargo run` still gets the toast. `bir-core` is not asked to post
    /// for this process; its path is for `bir-headless`.
    fn refresh_and_announce(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt;
        use gpui_component::notification::Notification;

        self.reload();
        let fresh: Vec<AppAlert> = self
            .alerts
            .iter()
            .filter(|alert| !self.announced.contains(&alert.id))
            .cloned()
            .collect();
        for alert in fresh {
            self.announced.insert(alert.id);
            let this = cx.entity().downgrade();
            let notification = match alert.severity {
                AlertSeverity::Error => Notification::error(alert.detail.clone()),
                AlertSeverity::Warning => Notification::warning(alert.detail.clone()),
                AlertSeverity::Info => Notification::info(alert.detail.clone()),
            }
            .title(alert.title.clone())
            .in_app_and_system()
            .on_click(move |_, _window, cx| {
                let _ = this.update(cx, |_, cx| cx.emit(NotificationsEvent::Open));
            });
            window.push_notification(notification, cx);
        }
        cx.notify();
    }

    /// Re-read alerts from the database.
    ///
    /// Deliberately swallows read failures into an empty list: this view exists
    /// to report problems, and it failing loudly because it could not read the
    /// problem table would be its own worst failure mode.
    pub fn reload(&mut self) {
        self.alerts = self
            .db
            .lock()
            .ok()
            .and_then(|db| {
                db.list_active_alerts(self.active_session_tin.as_deref())
                    .ok()
            })
            .unwrap_or_default();
    }

    pub fn set_active_session_tin(&mut self, tin: Option<String>, cx: &mut Context<Self>) {
        if self.active_session_tin != tin {
            self.active_session_tin = tin;
            self.reload();
            cx.notify();
        }
    }

    /// Number of unresolved alerts, for a sidebar badge.
    pub fn active_count(&self) -> usize {
        self.alerts.len()
    }

    fn dismiss(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Ok(db) = self.db.lock() {
            let _ = db.dismiss_alert(id);
        }
        self.reload();
        cx.notify();
    }

    fn severity_color(severity: AlertSeverity, cx: &Context<Self>) -> Hsla {
        match severity {
            AlertSeverity::Error => cx.theme().danger,
            AlertSeverity::Warning => cx.theme().warning,
            AlertSeverity::Info => cx.theme().muted_foreground,
        }
    }

    fn severity_label(severity: AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Error => "Error",
            AlertSeverity::Warning => "Warning",
            AlertSeverity::Info => "Info",
        }
    }

    /// "seen 412 times since 2026-07-22 09:14" — the count is the point.
    ///
    /// A condition seen once is a blip; one seen 400 times has been broken for
    /// days, and that difference is what tells the user whether to act now.
    fn occurrence_summary(alert: &AppAlert) -> String {
        if alert.occurrences <= 1 {
            format!("First seen {}", alert.first_seen_at)
        } else {
            format!(
                "Seen {} times since {}",
                alert.occurrences, alert.first_seen_at
            )
        }
    }

    fn render_alert(&self, alert: &AppAlert, cx: &mut Context<Self>) -> AnyElement {
        let accent = Self::severity_color(alert.severity, cx);
        let id = alert.id;
        let action = alert.action;

        let root = rsx! {
            <div
                flex
                flex_col
                gap_2
                p_4
                rounded_lg
                border_1
                border_color={cx.theme().border}
                bg={cx.theme().background}
                border_l_4
            >
                <div flex flex_row items_center justify_between gap_4>
                    <div flex flex_row items_center gap_2>
                        <div
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={accent}
                            px_2
                            py_1
                            rounded_md
                            bg={accent.opacity(0.12)}
                        >
                            {Self::severity_label(alert.severity)}
                        </div>
                        <div text_sm font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().foreground}>
                            {alert.title.clone()}
                        </div>
                    </div>
                    <div text_xs text_color={cx.theme().muted_foreground}>
                        {Self::occurrence_summary(alert)}
                    </div>
                </div>

                <div text_sm text_color={cx.theme().muted_foreground}>
                    {alert.detail.clone()}
                </div>

                <div flex flex_row items_center gap_2>
                    {...action.label().map(|label| {
                        Button::new(("alert_action", id as usize))
                            .primary()
                            .label(label)
                            .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                match action {
                                    AlertAction::ReconnectGoogleAccount => {
                                        cx.emit(NotificationsEvent::ReconnectGoogleAccount)
                                    }
                                    AlertAction::OpenProfileManager => {
                                        cx.emit(NotificationsEvent::OpenProfileManager)
                                    }
                                    AlertAction::None => {}
                                }
                            }))
                    })}
                    {Button::new(("alert_dismiss", id as usize))
                        .ghost()
                        .label("Dismiss")
                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                            this.dismiss(id, cx);
                        }))}
                </div>
            </div>
        };
        root.into_any_element()
    }

    fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let root = rsx! {
            <div flex flex_col items_center justify_center gap_2 p_12>
                <div text_lg font_weight={FontWeight::SEMIBOLD} text_color={cx.theme().foreground}>
                    {"Nothing needs your attention"}
                </div>
                <div text_sm text_color={cx.theme().muted_foreground}>
                    {"Problems the app can act on will appear here."}
                </div>
            </div>
        };
        root.into_any_element()
    }
}

impl Render for NotificationsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let alerts = self.alerts.clone();
        let subtitle = match (&self.active_session_tin, alerts.len()) {
            (_, 0) => "No open issues".to_string(),
            (Some(tin), n) => format!("{n} open for TIN {tin}, plus anything app-wide"),
            (None, n) => format!("{n} open"),
        };

        let body: Vec<AnyElement> = if alerts.is_empty() {
            vec![self.render_empty_state(cx)]
        } else {
            alerts
                .iter()
                .map(|alert| self.render_alert(alert, cx))
                .collect()
        };

        rsx! {
            <div flex flex_col size_full p_8 gap_6 bg={cx.theme().background}>
                <div flex flex_col gap_1>
                    <div text_3xl font_weight={FontWeight::BLACK} text_color={cx.theme().primary}>
                        {"Notifications"}
                    </div>
                    <div text_base text_color={cx.theme().muted_foreground}>
                        {subtitle}
                    </div>
                </div>

                <div
                    id="notifications_scroll"
                    flex
                    flex_col
                    gap_3
                    flex_1
                    min_h_0
                    overflow_y_scroll
                    track_scroll={&self.scroll_handle}
                >
                    {...body}
                </div>
            </div>
        }
    }
}
