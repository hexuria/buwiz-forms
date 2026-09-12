//! UI-thread mailbox drain. The TCP thread never touches GPUI entities.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use gpui_agent::authorize_request;
use gpui_agent::handle_request;
use gpui_agent::mailbox::MailboxRequest;
use gpui_agent::protocol::{Op, PlatformKind, Response};

use crate::agent::host::BirAgentHost;
use crate::agent::ids;
use crate::agent::keybindings;
use crate::agent::scrolled_shot::{self, ScrolledShotJob};
use crate::app::{ActiveView, AppState, ProfileTargetAction};
use crate::global_actions::{CreateProfile, OpenCommandPalette};
use chrono::Datelike;

pub(crate) struct PendingKeybindingFire {
    posted: MailboxRequest,
    binding: String,
}

pub fn apply_agent(app: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) {
    let Some(mailbox) = app.agent_mailbox.clone() else {
        return;
    };
    app.agent_request_backlog.extend(mailbox.take());
    if app.scrolled_job.is_some() {
        app.advance_scrolled_job(window, cx);
        return;
    }
    if app.pending_keybinding.is_some() {
        return;
    }
    while let Some(posted) = app.agent_request_backlog.pop_front() {
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
        if let Err(resp) = authorize_request(&posted.request, None, None) {
            crate::agent::request_log::emit_request_log(&posted.request, resp.as_ref());
            posted.reply(*resp);
            cx.notify();
            continue;
        }
        if posted.request.op.is_virtual_input() {
            let response = Response::err(
                &posted.request.id,
                gpui_agent::virtual_unavailable(
                    "bir-desktop ships semantic delivery first; \
                     virtual in-window events are not wired (no per-widget painted bounds). \
                     Protocol is unchanged; this host does not synthesize OS HID",
                ),
            );
            crate::agent::request_log::emit_request_log(&posted.request, &response);
            posted.reply(response);
            cx.notify();
            continue;
        }
        if matches!(posted.request.op, Op::Keybinding { .. }) {
            if app.start_keybinding_fire(posted, window, cx) {
                break;
            }
            continue;
        }
        if let Op::Screenshot { mode, .. } = &posted.request.op {
            if mode.is_scrolled() {
                let (path, target, max_height_px) = match &posted.request.op {
                    Op::Screenshot {
                        path,
                        target,
                        max_height_px,
                        ..
                    } => (path.clone(), target.clone(), *max_height_px),
                    _ => unreachable!(),
                };
                app.start_scrolled_job(posted, path, target, max_height_px, window, cx);
                return;
            }
            let path = match &posted.request.op {
                Op::Screenshot { path, .. } => path.clone(),
                _ => unreachable!(),
            };
            let response = match screenshot_this_window(window, path.as_deref()) {
                Ok(result) => {
                    let mut resp = Response::ok(&posted.request.id);
                    resp.result = result.value;
                    resp
                }
                Err(error) => Response::err(&posted.request.id, error),
            };
            crate::agent::request_log::emit_request_log(&posted.request, &response);
            posted.reply(response);
            cx.notify();
            continue;
        }

        let mut host = snapshot_host(app, cx);
        let response = handle_request(&mut host, posted.request.clone(), None, None);
        if mutating && response.ok {
            apply_host(host, app, window, cx);
        }
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
    host.set_cron_tab(app.cron_tasks_view.read(cx).agent_active_tab());
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

    if let Some(list) = app.db.lock().ok().and_then(|db| db.list_profiles().ok()) {
        app.profiles = list;
    }

    if host.active_view() != app.active_view {
        apply_navigation(host.active_view(), &host, app, window, cx);
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
            // Only when this request edited a field. The snapshot is read out
            // of this same view when the request starts, so writing it back
            // unconditionally is a round trip that is a no-op *only* if
            // nothing else touched the view in between — and a navigation in
            // the same request does exactly that, loading the taxpayer into
            // the editor. `form.open` followed by the profile manager pushed
            // the pre-navigation copy, which was empty, over every field:
            // the editor came up blank, the unsaved-changes banner appeared,
            // and Save Changes would have written the blanks to the profile.
            if host.editor_touched() && host.editor_snapshot().save_message.is_none() {
                view.agent_apply_editor(&host.editor_snapshot(), window, cx);
            }
        });
    }

    if host.active_view() == ActiveView::CronTasks {
        app.cron_tasks_view.update(cx, |view, cx| {
            view.agent_set_tab(host.cron_tab(), cx);
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

/// The taxpayer the app must adopt before `target` can be rendered.
///
/// A form view reads `AppState::active_profile_tin`, not the agent host's
/// selection, and `handle_file_form` silently does nothing without it. The
/// other views select their own taxpayer on the way in (`ProfileManager`
/// edits it, `Dashboard` shows it), so only a form needs this.
///
/// `form.open`/`filing.start` with `tin=` selects the taxpayer and opens the
/// form in one invoke; the drain used to apply the navigation before the
/// selection, so the form found no profile and the app stayed where it was.
/// A second, identical invoke worked, because by then the selection had
/// landed — which is what made it look intermittent.
fn profile_to_adopt(
    target: ActiveView,
    host_tin: Option<&str>,
    app_tin: Option<&str>,
) -> Option<String> {
    if ids::form_chrome(target).is_none() {
        return None;
    }
    let tin = host_tin?;
    (app_tin != Some(tin)).then(|| tin.to_string())
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
            if let Some(tin) =
                profile_to_adopt(form, host.selected_tin(), app.active_profile_tin.as_deref())
                && let Some(profile) = app
                    .profiles
                    .iter()
                    .find(|profile| profile.tin.full() == tin)
                    .cloned()
            {
                app.select_profile(profile, ProfileTargetAction::UnlockOnly, window, cx);
            }
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

pub(crate) fn gpui_scroll_metrics(
    handle: &ScrollHandle,
    target: &str,
) -> Result<gpui_agent::ScrollMetrics, String> {
    let bounds = handle.bounds();
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if h < 1.0 {
        return Err(gpui_agent::scroll_unavailable(format!(
            "{target} viewport is not painted yet"
        )));
    }
    let max_y = f32::from(handle.max_offset().y);
    let offset_y = (-f32::from(handle.offset().y)).max(0.0);
    Ok(gpui_agent::ScrollMetrics {
        viewport: gpui_agent::Bounds {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            w,
            h,
        },
        content_height: h + max_y.max(0.0),
        offset_y,
    })
}

fn set_gpui_scroll_offset(handle: &ScrollHandle, y: f32) {
    handle.set_offset(point(px(0.0), px(-y)));
}

/// The window's **content** size, in logical pixels. `crop_window_png` maps
/// `ScrollHandle` bounds — which are content-relative — onto a `screencapture`
/// of the whole window, and works out the title bar from the difference between
/// the PNG height and this value. `window.bounds()` is the frame, title bar
/// included (32 pt on current macOS), which zeroed that difference and cropped
/// every tile 32 pt too high: a band of chrome at each seam and 32 pt of content
/// lost per tile.
fn window_size(window: &Window) -> (f32, f32) {
    let size = window.viewport_size();
    (f32::from(size.width), f32::from(size.height))
}

#[cfg(target_os = "macos")]
struct ScrolledCaptureWindow {
    window_id: u32,
    size: (f32, f32),
}

fn reply_mailbox_err(posted: MailboxRequest, err: impl Into<String>) {
    let err = err.into();
    let id = posted.request.id.clone();
    crate::agent::request_log::emit_request_log(&posted.request, &Response::err(&id, err.clone()));
    posted.reply(Response::err(id, err));
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
        super::macos_capture::capture_window_to_client_path(id, path)
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
    /// Action body (`on_action` / global `App::on_action`). Records the
    /// listener result and completes a pending fire immediately so a
    /// deferred `finish_keybinding_fire` cannot miss it. The intercept still
    /// only `dispatch_action`s — this is the handler, not a dual-write.
    pub(crate) fn note_keybinding_fired(&mut self, id: &str, scope: &str) {
        self.last_keybinding_result = Some((
            id.to_string(),
            Ok(gpui_agent::DispatchResult::json(
                keybindings::keybinding_result_json(id, scope),
            )),
        ));
        self.finish_keybinding_fire(None);
    }

    fn start_keybinding_fire(
        &mut self,
        posted: MailboxRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let catalog = keybindings::catalog();
        let focused = window.is_window_active();
        let entry = match gpui_agent::authorize_keybinding_op(&posted.request.op, &catalog, focused)
        {
            Ok(entry) => entry.clone(),
            Err(error) => {
                reply_mailbox_err(posted, error);
                cx.notify();
                return false;
            }
        };
        let activate = match &posted.request.op {
            Op::Keybinding { activate, .. } => *activate,
            _ => false,
        };
        let Some(action) = keybindings::action_for_binding(&entry.id) else {
            reply_mailbox_err(posted, format!("unknown binding `{}`", entry.id));
            cx.notify();
            return false;
        };
        if activate {
            window.activate_window();
            self.focus_handle.focus(window, cx);
        }

        // Record pending *before* dispatch so a sync global listener can see it.
        // Window::dispatch_action always `cx.defer`s; finish is queued after that.
        // Do not `App::dispatch_action` from `render` (this drain): the window is
        // already taken, so that path no-ops. Window defer + App::on_action
        // (see `register_global_actions`) is the same contract as gpui-agent todo.
        self.last_keybinding_result = None;
        self.pending_keybinding = Some(PendingKeybindingFire {
            posted,
            binding: entry.id.clone(),
        });
        window.dispatch_action(action, cx);
        cx.defer_in(window, |this, _window, cx| {
            this.finish_keybinding_fire(Some(cx));
        });
        true
    }

    fn finish_keybinding_fire(&mut self, cx: Option<&mut Context<Self>>) {
        let Some(pending) = self.pending_keybinding.take() else {
            return;
        };
        let listener = match self.last_keybinding_result.take() {
            Some((id, result)) if id == pending.binding => Some(result),
            _ => None,
        };
        let response = match gpui_agent::complete_keybinding_action(listener) {
            Ok(result) => {
                let mut resp = Response::ok(&pending.posted.request.id);
                resp.result = result.value;
                resp
            }
            Err(error) => Response::err(&pending.posted.request.id, error),
        };
        crate::agent::request_log::emit_request_log(&pending.posted.request, &response);
        pending.posted.reply(response);
        if let Some(cx) = cx {
            cx.notify();
        }
    }

    fn start_scrolled_job(
        &mut self,
        posted: MailboxRequest,
        path: Option<String>,
        target: Option<String>,
        max_height_px: Option<u32>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let spec = gpui_agent::ScreenshotSpec {
            path: path.as_deref(),
            mode: gpui_agent::ScreenshotMode::Scrolled,
            target: target.as_deref(),
            max_height_px,
        };
        if let Err(err) = spec.validate_request() {
            reply_mailbox_err(posted, err);
            return;
        }
        let path = match gpui_agent::require_screenshot_path(spec.path) {
            Ok(path) => path.to_string(),
            Err(err) => {
                reply_mailbox_err(posted, err);
                return;
            }
        };
        if let Err(err) = gpui_agent::confine_screenshot_path(&path) {
            reply_mailbox_err(posted, err);
            return;
        }
        let target = match spec.scrolled_target() {
            Ok(t) => t.to_string(),
            Err(err) => {
                reply_mailbox_err(posted, err);
                return;
            }
        };
        if !scrolled_shot::known_scroll_target(&target) {
            reply_mailbox_err(
                posted,
                gpui_agent::scroll_unavailable(format!(
                    "unknown scroll target `{target}` (want {} or {})",
                    ids::FORM_1601C_SCROLL,
                    ids::PRINT_PREVIEW_SCROLL
                )),
            );
            return;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
            let _ = cx;
            reply_mailbox_err(
                posted,
                gpui_agent::screenshot_unavailable(
                    "scrolled screenshot is macOS-only (`screencapture -l` tiles). \
                     This OS has no production GPUI framebuffer export (`Window::render_to_image` is \
                     test-support only). Headless stays screenshot_unavailable.",
                ),
            );
        }
        #[cfg(target_os = "macos")]
        {
            let capture = match self.scrolled_capture_window(&target, window, cx) {
                Ok(capture) => capture,
                Err(err) => {
                    reply_mailbox_err(posted, err);
                    return;
                }
            };
            let original = self
                .scroll_metrics_for_target(&target, cx)
                .map(|m| m.offset_y)
                .unwrap_or(0.0);
            self.scrolled_job = Some(ScrolledShotJob {
                reply: posted,
                dest_client: path,
                target,
                original_offset: original,
                tiles: Vec::new(),
                next: 0,
                captured: Vec::new(),
                metrics: None,
                window_size: capture.size,
                window_id: capture.window_id,
                phase: scrolled_shot::ScrolledPhase::WaitMetrics,
                frames_waited: 0,
                awaiting_paint: false,
            });
            cx.notify();
        }
    }

    fn restore_scroll_offset(&mut self, target: &str, y: f32, cx: &mut Context<Self>) {
        self.set_scroll_offset_y(target, y, cx);
    }

    fn finish_scrolled_job(
        &mut self,
        job: ScrolledShotJob,
        response: Response,
        cx: &mut Context<Self>,
    ) {
        self.restore_scroll_offset(&job.target, job.original_offset, cx);
        crate::agent::request_log::emit_request_log(&job.reply.request, &response);
        job.reply.reply(response);
        cx.notify();
    }

    fn advance_scrolled_job(&mut self, window: &Window, cx: &mut Context<Self>) {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
            if let Some(job) = self.scrolled_job.take() {
                let id = job.reply.request.id.clone();
                self.finish_scrolled_job(
                    job,
                    Response::err(
                        id,
                        gpui_agent::screenshot_unavailable("scrolled screenshot is macOS-only"),
                    ),
                    cx,
                );
            }
        }
        #[cfg(target_os = "macos")]
        {
            self.advance_scrolled_job_macos(window, cx);
        }
    }

    #[cfg(target_os = "macos")]
    fn advance_scrolled_job_macos(&mut self, window: &Window, cx: &mut Context<Self>) {
        use gpui_agent::{
            MAX_SCROLLED_PNG_BYTES, confine_screenshot_path, crop_window_png, encode_png_rgba,
            plan_scroll_tiles, scrolled_dispatch_result, stitch_tiles_vertically,
        };

        let mut job = match self.scrolled_job.take() {
            Some(job) => job,
            None => return,
        };
        job.frames_waited += 1;
        if let Ok(capture) = self.scrolled_capture_window(&job.target, window, cx) {
            job.window_size = capture.size;
            job.window_id = capture.window_id;
        }

        match job.phase {
            scrolled_shot::ScrolledPhase::WaitMetrics => {
                match self.scroll_metrics_for_target(&job.target, cx) {
                    Ok(metrics) => {
                        let cap = match &job.reply.request.op {
                            Op::Screenshot { max_height_px, .. } => {
                                max_height_px.unwrap_or(gpui_agent::DEFAULT_MAX_HEIGHT_PX)
                            }
                            _ => gpui_agent::DEFAULT_MAX_HEIGHT_PX,
                        };
                        let tiles = match plan_scroll_tiles(&metrics, cap) {
                            Ok(tiles) => tiles,
                            Err(err) => {
                                let id = job.reply.request.id.clone();
                                self.finish_scrolled_job(job, Response::err(id, err), cx);
                                return;
                            }
                        };
                        job.original_offset = metrics.offset_y;
                        job.metrics = Some(metrics);
                        job.tiles = tiles;
                        job.next = 0;
                        if job.tiles.is_empty() {
                            let id = job.reply.request.id.clone();
                            self.finish_scrolled_job(
                                job,
                                Response::err(id, "scrolled screenshot produced no tiles"),
                                cx,
                            );
                            return;
                        }
                        self.set_scroll_offset_y(&job.target, job.tiles[0].offset_y, cx);
                        job.phase = scrolled_shot::ScrolledPhase::WaitPaint;
                        job.awaiting_paint = true;
                        self.scrolled_job = Some(job);
                        cx.notify();
                    }
                    Err(_) if job.frames_waited < scrolled_shot::METRICS_WAIT_FRAMES => {
                        self.scrolled_job = Some(job);
                        cx.notify();
                    }
                    Err(err) => {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(job, Response::err(id, err), cx);
                    }
                }
            }
            scrolled_shot::ScrolledPhase::WaitPaint => {
                if job.awaiting_paint {
                    job.awaiting_paint = false;
                    self.scrolled_job = Some(job);
                    cx.notify();
                    return;
                }
                let spec = job.tiles[job.next];
                let metrics = match job.metrics {
                    Some(m) => m,
                    None => {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(
                            job,
                            Response::err(
                                id,
                                gpui_agent::scroll_unavailable("missing scroll metrics"),
                            ),
                            cx,
                        );
                        return;
                    }
                };
                let png = match super::macos_capture::capture_window_png_bytes(job.window_id) {
                    Ok(png) => png,
                    Err(err) => {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(job, Response::err(id, err), cx);
                        return;
                    }
                };
                let slice = match crop_window_png(
                    &png,
                    job.window_size.0,
                    job.window_size.1,
                    metrics.viewport,
                    spec.skip_top_px,
                    spec.take_height_px,
                ) {
                    Ok(slice) => slice,
                    Err(err) => {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(job, Response::err(id, err), cx);
                        return;
                    }
                };
                job.captured.push(slice);
                job.next += 1;
                if job.next >= job.tiles.len() {
                    let dest_client = job.dest_client.clone();
                    let target = job.target.clone();
                    let content_h = metrics.content_height.max(metrics.viewport.h);
                    let vh = metrics.viewport.h;
                    let tile_count = job.tiles.len();
                    let stitched = match stitch_tiles_vertically(&job.captured) {
                        Ok(img) => img,
                        Err(err) => {
                            let id = job.reply.request.id.clone();
                            self.finish_scrolled_job(job, Response::err(id, err), cx);
                            return;
                        }
                    };
                    let bytes = match encode_png_rgba(&stitched) {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            let id = job.reply.request.id.clone();
                            self.finish_scrolled_job(job, Response::err(id, err), cx);
                            return;
                        }
                    };
                    if bytes.len() > MAX_SCROLLED_PNG_BYTES {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(
                            job,
                            Response::err(
                                id,
                                format!(
                                    "stitched png exceeds {MAX_SCROLLED_PNG_BYTES} bytes ({})",
                                    bytes.len()
                                ),
                            ),
                            cx,
                        );
                        return;
                    }
                    let dest = match confine_screenshot_path(&dest_client) {
                        Ok(dest) => dest,
                        Err(err) => {
                            let id = job.reply.request.id.clone();
                            self.finish_scrolled_job(job, Response::err(id, err), cx);
                            return;
                        }
                    };
                    if let Err(err) = gpui_agent::atomic_write_png(&dest, &bytes) {
                        let id = job.reply.request.id.clone();
                        self.finish_scrolled_job(job, Response::err(id, err), cx);
                        return;
                    }
                    let dest_str = dest.to_string_lossy().into_owned();
                    let result =
                        scrolled_dispatch_result(&dest_str, &target, content_h, vh, tile_count);
                    let id = job.reply.request.id.clone();
                    let mut resp = Response::ok(id);
                    resp.result = result.value;
                    self.finish_scrolled_job(job, resp, cx);
                    return;
                }
                self.set_scroll_offset_y(&job.target, job.tiles[job.next].offset_y, cx);
                job.awaiting_paint = true;
                self.scrolled_job = Some(job);
                cx.notify();
            }
        }
    }

    fn scroll_metrics_for_target(
        &mut self,
        target: &str,
        cx: &mut Context<Self>,
    ) -> Result<gpui_agent::ScrollMetrics, String> {
        if target == ids::FORM_1601C_SCROLL {
            let Some(view) = &self.form_1601c_view else {
                return Err(gpui_agent::scroll_unavailable(
                    "form-1601c-scroll is not painted (open 1601-C first)",
                ));
            };
            return gpui_scroll_metrics(&view.read(cx).agent_scroll_handle(), target);
        }
        if target == ids::PRINT_PREVIEW_SCROLL {
            return self.print_preview_metrics(cx);
        }
        Err(gpui_agent::scroll_unavailable(format!(
            "unknown scroll target `{target}`"
        )))
    }

    fn print_preview_metrics(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<gpui_agent::ScrollMetrics, String> {
        let handle = crate::views::frozen_html_preview::live_preview_window().ok_or_else(|| {
            gpui_agent::scroll_unavailable(
                "print-preview-scroll is not painted (open form.print first)",
            )
        })?;
        handle
            .update(cx, |view, _window, cx| {
                view.request_js_metrics(cx);
                let gpui = gpui_scroll_metrics(view.scroll_handle(), ids::PRINT_PREVIEW_SCROLL);
                let js = view.js_scroll_metrics();
                match (gpui, js) {
                    (Ok(gpui), Some(js)) if js.content_height > gpui.content_height + 1.0 => {
                        Ok(gpui_agent::ScrollMetrics {
                            viewport: gpui.viewport,
                            content_height: js.content_height.max(js.viewport_height).max(1.0),
                            offset_y: js.offset_y.max(0.0),
                        })
                    }
                    // The GPUI scroller around the WebView is exactly as tall
                    // as its child, so its metrics say "nothing to scroll".
                    // The document height is only known to the WebView; keep
                    // the job in `WaitMetrics` until that answer has arrived.
                    (Ok(_), None) => Err(gpui_agent::scroll_unavailable(
                        "print-preview-scroll: waiting for the WebView's scroll metrics",
                    )),
                    (Ok(metrics), _) => Ok(metrics),
                    (Err(_), Some(js)) if js.viewport_height >= 1.0 => {
                        let bounds = view.scroll_handle().bounds();
                        Ok(gpui_agent::ScrollMetrics {
                            viewport: gpui_agent::Bounds {
                                x: f32::from(bounds.origin.x),
                                y: f32::from(bounds.origin.y),
                                w: f32::from(bounds.size.width).max(1.0),
                                h: js.viewport_height,
                            },
                            content_height: js.content_height.max(js.viewport_height),
                            offset_y: js.offset_y.max(0.0),
                        })
                    }
                    (Err(err), _) => Err(err),
                }
            })
            .map_err(|err| {
                gpui_agent::scroll_unavailable(format!("print-preview-scroll window: {err}"))
            })?
    }

    fn set_scroll_offset_y(&mut self, target: &str, y: f32, cx: &mut Context<Self>) {
        if target == ids::FORM_1601C_SCROLL {
            if let Some(view) = &self.form_1601c_view {
                set_gpui_scroll_offset(&view.read(cx).agent_scroll_handle(), y);
                view.update(cx, |_view, cx| cx.notify());
            }
            return;
        }
        if target == ids::PRINT_PREVIEW_SCROLL
            && let Some(handle) = crate::views::frozen_html_preview::live_preview_window()
        {
            let _ = handle.update(cx, |view, _window, cx| {
                view.set_scroll_offset_y(y, cx);
            });
        }
    }

    #[cfg(target_os = "macos")]
    fn scrolled_capture_window(
        &mut self,
        target: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<ScrolledCaptureWindow, String> {
        if target == ids::PRINT_PREVIEW_SCROLL {
            let handle =
                crate::views::frozen_html_preview::live_preview_window().ok_or_else(|| {
                    gpui_agent::scroll_unavailable(
                        "print-preview-scroll is not painted (open form.print first)",
                    )
                })?;
            return handle
                .update(cx, |_view, preview_window, _cx| {
                    Ok(ScrolledCaptureWindow {
                        window_id: super::macos_window::cgwindow_id(preview_window)?,
                        size: window_size(preview_window),
                    })
                })
                .map_err(|err| {
                    gpui_agent::scroll_unavailable(format!("print-preview-scroll window: {err}"))
                })?;
        }
        Ok(ScrolledCaptureWindow {
            window_id: super::macos_window::cgwindow_id(window)?,
            size: window_size(window),
        })
    }

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

#[cfg(test)]
mod tests {
    use super::{ActiveView, profile_to_adopt};

    const TIN: &str = "00000000000000";

    #[test]
    fn a_form_navigation_adopts_the_hosts_taxpayer_once() {
        assert_eq!(
            profile_to_adopt(ActiveView::Form1601C, Some(TIN), None),
            Some(TIN.to_string()),
            "form.open with tin= must select the taxpayer before the form opens"
        );
        assert_eq!(
            profile_to_adopt(ActiveView::Form1601C, Some(TIN), Some(TIN)),
            None,
            "already the app's profile: nothing to adopt"
        );
        assert_eq!(
            profile_to_adopt(ActiveView::Form1601C, Some(TIN), Some("99999999999999")),
            Some(TIN.to_string()),
            "a different taxpayer is adopted"
        );
        assert_eq!(
            profile_to_adopt(ActiveView::Form1601C, None, None),
            None,
            "no host selection: the form view reports the missing profile itself"
        );
    }

    #[test]
    fn other_views_select_their_own_taxpayer() {
        for view in [
            ActiveView::Dashboard,
            ActiveView::ProfileManager,
            ActiveView::GlobalDashboard,
            ActiveView::Settings,
            ActiveView::Notifications,
        ] {
            assert_eq!(
                profile_to_adopt(view, Some(TIN), None),
                None,
                "{view:?} handles its own selection"
            );
        }
    }
}
