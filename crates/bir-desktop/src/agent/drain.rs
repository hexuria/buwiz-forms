//! UI-thread mailbox drain. The TCP thread never touches GPUI entities.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use gpui_agent::authorize_request;
use gpui_agent::handle_request;
use gpui_agent::protocol::{Op, PlatformKind, Response};

use crate::agent::host::BirAgentHost;
use crate::agent::ids;
use crate::app::{ActiveView, AppState, ProfileTargetAction};
use crate::global_actions::{CreateProfile, OpenCommandPalette};
use chrono::Datelike;

pub fn apply_agent(app: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) {
    let Some(mailbox) = app.agent_mailbox.clone() else {
        return;
    };
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
        // TCP spawn_mailbox already HMAC-authorizes (per-connection nonce is
        // not on MailboxRequest). Re-check protocol version so screenshot /
        // virtual intercepts that skip handle_request cannot skip v2. Pass
        // token=None into handle_request so HMAC does not fail without the
        // nonce; the TCP thread stamps hello.auth from the bind token.
        let response = if let Err(resp) = authorize_request(&posted.request, None, None) {
            *resp
        } else if posted.request.op.is_virtual_input() {
            Response::err(
                &posted.request.id,
                gpui_agent::virtual_unavailable(
                    "bir-desktop ships semantic delivery first; \
                     virtual in-window events are not wired (no per-widget painted bounds). \
                     Protocol is unchanged; this host does not synthesize OS HID",
                ),
            )
        } else if let Op::Screenshot { path } = &posted.request.op {
            match screenshot_this_window(window, path.as_deref()) {
                Ok(result) => {
                    let mut resp = Response::ok(&posted.request.id);
                    resp.result = result.value;
                    resp
                }
                Err(error) => Response::err(&posted.request.id, error),
            }
        } else {
            let mut host = snapshot_host(app, cx);
            let response = handle_request(&mut host, posted.request.clone(), None, None);
            if mutating && response.ok {
                apply_host(host, app, window, cx);
            }
            response
        };
        crate::agent::request_log::emit_request_log(&posted.request, &response);
        posted.reply(response);
        if shutdown {
            app.release_agent_listener();
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
    host.set_hide_tax_profiles(app.hide_tax_profiles);
    host.restore_view(app.active_view, app.active_profile_tin.clone());
    host.set_profile_tab(app.profile_manager.read(cx).agent_active_tab());
    host.replace_profiles(
        app.profiles
            .iter()
            .map(|profile| {
                (
                    profile.tin.full(),
                    profile.full_name.clone(),
                    profile.is_archived,
                )
            })
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
        // `saved` is a one-shot host flag set by form.save_draft / queue in this
        // request. Never derive it from draft.id — that made every mutating
        // invoke (including form.fields) re-trigger UI Draft-save on
        // Queued/Submitted returns and spam the toaster.
        host.replace_form_1601c_state(
            form.agent_draft().clone(),
            form.agent_validated(),
            false,
            form.agent_validation_errors(),
        );
    }
    if let Some(view) = &app.form_2551q_view {
        let form = view.read(cx);
        host.replace_form_2551q_state(
            form.agent_draft().clone(),
            form.agent_validated(),
            false,
            form.agent_validation_errors(),
        );
    }
    host.reconcile_open_forms_from_db();
    // Jobs and submissions are only read back out of the host by `jobs.list` /
    // `submissions.list` (which reload themselves) and by the Background Tasks
    // snapshot tree. Loading them for every request put `list_jobs` plus a full
    // submission history behind every `form.fields` poll, on the UI thread,
    // inside `render` — roughly half of each frame once a taxpayer had history.
    if app.active_view == ActiveView::CronTasks {
        let _ = host.reload_jobs_and_submissions();
    }
    host.mark_pending_admin(app.pending_admin_view);
    host.mark_pending_profile_auth(app.pending_profile.is_some());
    host.set_submit_confirmation_visible(app.agent_submit_confirmation_visible);
    host.set_palette_open(app.is_command_palette_open);
    host
}

fn apply_host(
    mut host: BirAgentHost,
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

    if let Some(tin) = host.selected_tin() {
        let already_selected = app.active_profile_tin.as_deref() == Some(tin);
        let edit = host.active_view() == ActiveView::ProfileManager;
        if (!already_selected || edit)
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
    }

    if host.active_view() == ActiveView::ProfileManager {
        app.profile_manager.update(cx, |view, cx| {
            view.agent_set_tab(host.profile_tab());
            if host.editor_snapshot().save_message.is_none() {
                view.agent_apply_editor(&host.editor_snapshot(), window, cx);
            }
        });
    }

    if host.active_view() == ActiveView::Dashboard {
        app.dashboard_view.update(cx, |view, cx| {
            view.agent_apply_filters(host.dashboard_forms(), host.dashboard_query(), window, cx);
        });
    }

    let print_requested = host.take_print_request();

    if host.active_view() == ActiveView::Form1601C
        && let Some(view) = &app.form_1601c_view
    {
        view.update(cx, |form, cx| {
            form.agent_apply_from_host(host.form_1601c_host_patch(), window, cx);
            if let Some(draft) = host.form_1601c_draft() {
                form.agent_sync_filing_snapshot(draft, cx);
            }
            form.agent_reload_filing_from_db(cx);
            if print_requested {
                form.agent_preview_pdf(window, cx);
            }
        });
    }

    if host.active_view() == ActiveView::Form2551Q
        && let Some(view) = &app.form_2551q_view
    {
        view.update(cx, |form, cx| {
            form.agent_apply_from_host(
                crate::views::form_2551q_view::Agent2551QHostPatch {
                    creditable_tax_withheld: host.form_2551q_creditable(),
                    other_tax_credit: host.form_2551q_other_credit(),
                    taxable_amount_0: host.form_2551q_taxable_0(),
                    save: host.form_2551q_saved(),
                    validate: host.form_2551q_validated(),
                },
                window,
                cx,
            );
            if let Some(draft) = host.form_2551q_draft() {
                form.agent_sync_filing_snapshot(draft, cx);
            }
            if print_requested {
                form.agent_preview_pdf(window, cx);
            }
        });
    }

    app.agent_submit_confirmation_visible = host.submit_confirmation_visible();

    if host.wants_palette() && !app.is_command_palette_open {
        app.handle_open_command_palette(&OpenCommandPalette, window, cx);
    }
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
            } else if let Some(tin) = host.selected_tin()
                && let Some(profile) = app
                    .profiles
                    .iter()
                    .find(|profile| profile.tin.full() == tin)
                    .cloned()
            {
                app.select_profile(profile, ProfileTargetAction::EditProfile, window, cx);
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
                let year = host
                    .form_1601c_draft()
                    .map(|draft| draft.taxable_year)
                    .or_else(|| host.form_2551q_draft().map(|draft| draft.taxable_year))
                    .unwrap_or_else(|| chrono::Local::now().year() as u16);
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

fn screenshot_this_window(
    window: &Window,
    path: Option<&str>,
) -> Result<gpui_agent::DispatchResult, String> {
    let path = gpui_agent::require_screenshot_path(path)?;
    let _dest = gpui_agent::confine_screenshot_path(path)?;
    #[cfg(target_os = "macos")]
    {
        let id = super::macos_window::cgwindow_id(window)?;
        gpui_agent::capture_window_via_screencapture(id, Some(path))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        let _ = path;
        Err(gpui_agent::screenshot_unavailable(
            "desktop PNG of the app window is macOS-only (`screencapture -l` of this window). \
             Linux/Windows have no production GPUI framebuffer export (`Window::render_to_image` is \
             test-support only). Headless stays screenshot_unavailable.",
        ))
    }
}

impl AppState {
    pub(crate) fn attach_agent(
        &mut self,
        mailbox: gpui_agent::mailbox::AgentMailbox,
        token: Option<String>,
        shutdown: Arc<AtomicBool>,
        cx: &mut Context<Self>,
    ) {
        self.agent_mailbox = Some(mailbox);
        self.agent_token = token;
        self.agent_shutdown = Some(shutdown);
        // Wake the UI thread only when the TCP thread actually posted work.
        // `apply_agent` runs inside `AppState::render`, so an unconditional
        // notify here re-laid out and repainted the whole window 60 times a
        // second for as long as the agent was attached (~50% of a core at idle
        // against ~1% without it), leaving no main-thread headroom for input.
        self.agent_refresh = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let alive = this.update(cx, |app, cx| {
                    if app
                        .agent_mailbox
                        .as_ref()
                        .is_some_and(|mailbox| !mailbox.is_empty())
                    {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    pub(crate) fn release_agent_listener(&self) {
        if let Some(flag) = &self.agent_shutdown {
            flag.store(true, Ordering::SeqCst);
        }
    }
}
