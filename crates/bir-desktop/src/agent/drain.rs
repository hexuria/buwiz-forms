//! UI-thread mailbox drain. The TCP thread never touches GPUI entities.

use gpui::*;
use gpui_agent::handle_request;
use gpui_agent::protocol::{Op, PlatformKind};

use crate::agent::host::BirAgentHost;
use crate::agent::ids;
use crate::app::{ActiveView, AppState, ProfileTargetAction};
use crate::global_actions::CreateProfile;
use crate::views::form_1601c_view::Agent1601CHostPatch;
use chrono::Datelike;

pub fn apply_agent(app: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) {
    let Some(mailbox) = app.agent_mailbox.clone() else {
        return;
    };
    // Clone so `handle_request` can take `Option<&str>` without holding `&app`
    // across the later `&mut app` apply. Never log this value.
    let expected_token = app.agent_token.clone();
    for posted in mailbox.take() {
        let shutdown = matches!(posted.request.op, Op::Shutdown);
        let mutating = matches!(
            posted.request.op,
            Op::Click { .. }
                | Op::Type { .. }
                | Op::SetValue { .. }
                | Op::Key { .. }
                | Op::Invoke { .. }
                | Op::Shutdown
        );
        let response = if posted.request.op.is_virtual_input() {
            gpui_agent::Response::err(
                &posted.request.id,
                gpui_agent::virtual_unavailable(
                    "bir-desktop ships semantic delivery first; \
                     virtual in-window events are not wired (no per-widget painted bounds). \
                     Protocol is unchanged; this host does not synthesize OS HID",
                ),
            )
        } else {
            let mut host = snapshot_host(app, cx);
            let response =
                handle_request(&mut host, posted.request.clone(), expected_token.as_deref());
            if mutating && response.ok {
                apply_host(host, app, window, cx);
            }
            response
        };
        posted.reply(response);
        if shutdown {
            cx.quit();
        }
        cx.notify();
    }
}

fn snapshot_host(app: &AppState, cx: &App) -> BirAgentHost {
    let admin_lock_enabled = if let Ok(db) = app.db.lock() {
        let pin = db.get_setting("app_lock_enabled").ok().flatten().as_deref() == Some("true");
        let totp = db.get_setting("app_totp_secret").ok().flatten().is_some();
        pin || totp
    } else {
        false
    };

    let mut host =
        BirAgentHost::new(PlatformKind::Desktop).with_database(std::sync::Arc::clone(&app.db));
    host.set_locked(app.is_locked);
    host.set_admin_lock_enabled(admin_lock_enabled);
    host.set_unsaved_compliance(
        app.profile_manager
            .read(cx)
            .has_unsaved_compliance_changes(),
    );
    host.set_profile_pins(app.enable_profile_pins);
    host.restore_view(app.active_view, app.active_profile_tin.clone());
    host.replace_profiles(
        app.profiles
            .iter()
            .map(|profile| (profile.tin.full(), profile.full_name.clone()))
            .collect(),
        app.active_profile_tin.clone(),
    );
    host.replace_editor(app.profile_manager.read(cx).agent_read_editor(cx));
    let dues = match app.active_view {
        ActiveView::Dashboard => app.dashboard_view.read(cx).agent_dues(),
        ActiveView::GlobalDashboard => app.global_dashboard_view.read(cx).agent_dues(),
        _ => Vec::new(),
    };
    host.replace_dues(dues);
    if let Some(view) = &app.form_1601c_view {
        let form = view.read(cx);
        host.replace_form_1601c_state(
            form.agent_draft().clone(),
            form.agent_validated(),
            form.agent_draft().id.is_some(),
            form.agent_validation_errors(),
        );
    }
    host.mark_pending_admin(app.pending_admin_view);
    host.mark_pending_profile_auth(app.pending_profile.is_some());
    host.set_submit_confirmation_visible(app.agent_submit_confirmation_visible);
    host
}

fn apply_host(
    host: BirAgentHost,
    app: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) {
    if app.is_locked {
        return;
    }

    if let Some(target) = host.pending_admin_view() {
        app.request_admin_access(target, window, cx);
        return;
    }

    if host.pending_profile_auth() {
        return;
    }

    if host.active_view() != app.active_view {
        apply_navigation(host.active_view(), &host, app, window, cx);
    }

    if let Some(list) = app.db.lock().ok().and_then(|db| db.list_profiles().ok()) {
        app.profiles = list;
    }

    if let Some(tin) = host.selected_tin()
        && app.active_profile_tin.as_deref() != Some(tin)
        && let Some(profile) = app
            .profiles
            .iter()
            .find(|profile| profile.tin.full() == tin)
            .cloned()
    {
        let action = match host.active_view() {
            ActiveView::ProfileManager => ProfileTargetAction::EditProfile,
            ActiveView::Dashboard => ProfileTargetAction::ViewDashboard,
            _ => ProfileTargetAction::UnlockOnly,
        };
        app.select_profile(profile, action, window, cx);
    }

    if host.active_view() == ActiveView::ProfileManager
        && host.editor_snapshot().save_message.is_none()
    {
        app.profile_manager.update(cx, |view, cx| {
            view.agent_apply_editor(&host.editor_snapshot(), window, cx);
        });
    }

    if host.active_view() == ActiveView::Form1601C
        && let Some(view) = &app.form_1601c_view
    {
        view.update(cx, |form, cx| {
            form.agent_apply_from_host(
                Agent1601CHostPatch {
                    tax_14: host.form_1601c_tax_14(),
                    tax_25: host.form_1601c_tax_25(),
                    sheets: host.form_1601c_sheets(),
                    save: host.form_1601c_saved(),
                    validate: host.form_1601c_validated(),
                },
                window,
                cx,
            );
        });
    }

    app.agent_submit_confirmation_visible = host.submit_confirmation_visible();
}

fn apply_navigation(
    target: ActiveView,
    host: &BirAgentHost,
    app: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) {
    match target {
        ActiveView::Settings | ActiveView::CronTasks | ActiveView::AdminCalendarDashboard => {
            app.request_admin_access(target, window, cx);
        }
        ActiveView::GlobalDashboard => {
            if app.block_unsaved_compliance_navigation(window, cx) {
                return;
            }
            app.active_view = ActiveView::GlobalDashboard;
            app.active_profile_tin = None;
            cx.notify();
        }
        ActiveView::ProfileManager => {
            if host.selected_tin().is_none() {
                app.handle_create_profile(&CreateProfile, window, cx);
            } else {
                if app.block_unsaved_compliance_navigation(window, cx) {
                    return;
                }
                app.active_view = ActiveView::ProfileManager;
                cx.notify();
            }
        }
        ActiveView::Dashboard => {
            if let Some(tin) = host.selected_tin()
                && let Some(profile) = app
                    .profiles
                    .iter()
                    .find(|profile| profile.tin.full() == tin)
                    .cloned()
            {
                app.select_profile(profile, ProfileTargetAction::ViewDashboard, window, cx);
            } else {
                app.active_view = ActiveView::Dashboard;
                cx.notify();
            }
        }
        ActiveView::Notifications => {
            if app.block_unsaved_compliance_navigation(window, cx) {
                return;
            }
            let tin = app.active_session_tin.clone();
            app.notifications_view.update(cx, |view, cx| {
                view.set_active_session_tin(tin, cx);
            });
            app.active_view = ActiveView::Notifications;
            cx.notify();
        }
        ActiveView::ImportExport => {
            if app.block_unsaved_compliance_navigation(window, cx) {
                return;
            }
            app.active_view = ActiveView::ImportExport;
            cx.notify();
        }
        form if ids::form_chrome(form).is_some() => {
            let chrome = ids::form_chrome(form).expect("form chrome");
            if host.selected_tin().is_some() {
                let year = chrono::Local::now().year() as u16;
                let period = host.form_period().unwrap_or(1);
                app.open_named_form(chrome.code, year, period, window, cx);
            } else {
                app.active_view = form;
                cx.notify();
            }
        }
        other => {
            if app.block_unsaved_compliance_navigation(window, cx) {
                return;
            }
            app.active_view = other;
            cx.notify();
        }
    }
}

impl AppState {
    pub(crate) fn attach_agent(
        &mut self,
        mailbox: gpui_agent::mailbox::AgentMailbox,
        token: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.agent_mailbox = Some(mailbox);
        self.agent_token = token;
        self.agent_refresh = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }
}
