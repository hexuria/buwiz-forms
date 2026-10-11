//! Dev Mode panel. Compiled only into debug builds (`views/mod.rs`).
//!
//! Each control writes a `dev.` setting through `bir_core::dev_mode::save`.
//! When an environment variable overrides a switch, the control is disabled
//! and labelled "set by env: NAME". "Live BIR" is never offered here.

use bir_core::db::Database;
use bir_core::dev_mode::{self as dm, Resolved, Source, SubmissionMode, keys};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::switch::Switch;
use gpui_component::*;
use std::sync::{Arc, Mutex};

const FOLDER_HELP: &str = "Where the upload lands on your test server. Blank = a folder named after the form type, e.g. 2550Qv2024, like BIR's server.";
const HOST_KEY_HELP: &str =
    "Your test server's fingerprint. Uploads only go to a server that shows it.";
const ACCEPT_ANY_HELP: &str =
    "Skip the fingerprint check (BIR's own app does). Use only on a trusted network.";

pub enum DevModeEvent {
    /// A switch changed: re-render pills and pages that read it.
    Changed,
    /// Example alerts were seeded: reload the Notifications page.
    AlertsSeeded,
}

impl EventEmitter<DevModeEvent> for DevModeView {}

pub struct DevModeView {
    db: Arc<Mutex<Database>>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    username: Entity<InputState>,
    folder: Entity<InputState>,
    host_key: Entity<InputState>,
    password: Entity<InputState>,
    password_stored: bool,
    message: Option<(bool, String)>,
    /// The last export, for the "Open folder" button.
    last_export: Option<crate::export_target::SavedExport>,
    /// "Use a separate dev database" was switched on and awaits Continue.
    confirm_separate_db: bool,
    scroll: ScrollHandle,
}

impl DevModeView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fields = dm::test_server_fields();
        let input =
            |value: &str, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
                let value = value.to_string();
                let placeholder = placeholder.to_string();
                let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                state.update(cx, |state, cx| state.set_value(value, window, cx));
                state
            };
        let host = input(&fields.host.value, "127.0.0.1", window, cx);
        let port = input(&fields.port.value, "22", window, cx);
        let username = input(&fields.username.value, "lab", window, cx);
        let folder = input(&fields.folder.value, "form type (default)", window, cx);
        let host_key = input(
            &fields.host_key_sha256.value,
            "SHA-256 fingerprint",
            window,
            cx,
        );
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Stored in the OS keychain")
        });
        sync_separate_database_setting(&db);
        Self {
            db,
            host,
            port,
            username,
            folder,
            host_key,
            password,
            password_stored: dm::has_test_server_password(),
            message: None,
            last_export: None,
            confirm_separate_db: false,
            scroll: ScrollHandle::new(),
        }
    }

    fn save(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        let result = match self.db.lock() {
            Ok(db) => dm::save(&db, key, value).map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = result {
            self.message = Some((false, format!("Could not save {key}: {error}")));
        }
        cx.emit(DevModeEvent::Changed);
        cx.notify();
    }

    fn save_flag(&mut self, key: &str, value: bool, cx: &mut Context<Self>) {
        self.save(key, if value { "true" } else { "false" }, cx);
    }

    fn save_startup_flag(&mut self, key: &str, value: bool, cx: &mut Context<Self>) {
        let result = match self.db.lock() {
            Ok(db) => dm::save_startup_flag(&db, key, value).map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.message = Some(match result {
            Ok(()) => (true, "Saved. Takes effect on next start.".into()),
            Err(error) => (false, error),
        });
        cx.emit(DevModeEvent::Changed);
        cx.notify();
    }

    fn save_test_server(&mut self, cx: &mut Context<Self>) {
        let text = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().trim().to_string()
        };
        let port = text(&self.port, cx);
        if !port.is_empty() && port.parse::<u16>().is_err() {
            self.message = Some((
                false,
                format!("Port \"{port}\" is not a number up to 65535."),
            ));
            cx.notify();
            return;
        }
        let fields = dm::test_server_fields();
        let pairs = [
            (keys::SFTP_HOST, &self.host, fields.host.env),
            (keys::SFTP_PORT, &self.port, fields.port.env),
            (keys::SFTP_USERNAME, &self.username, fields.username.env),
            (keys::SFTP_FOLDER, &self.folder, fields.folder.env),
            (
                keys::SFTP_HOST_KEY_SHA256,
                &self.host_key,
                fields.host_key_sha256.env,
            ),
        ];
        let values: Vec<(&str, String)> = pairs
            .iter()
            .filter(|(_, _, env)| env.is_none())
            .map(|(key, input, _)| (*key, text(input, cx)))
            .collect();
        for (key, value) in values {
            self.save(key, &value, cx);
        }
        let password = self.password.read(cx).value().to_string();
        if !password.is_empty() && !fields.password_env {
            let result = dm::set_test_server_password(&password);
            match result {
                Ok(()) => {
                    self.password_stored = true;
                    self.message = Some((true, "Test server saved.".into()));
                }
                Err(error) => self.message = Some((false, error)),
            }
        } else {
            self.message = Some((true, "Test server saved.".into()));
        }
        cx.notify();
    }

    fn export_copy(&mut self, cx: &mut Context<Self>) {
        let source = crate::dev_mode::opened_database_path();
        let db = Arc::clone(&self.db);
        cx.spawn(async move |this, cx| {
            let result = crate::export_target::save_with_fallback(
                "Export a copy of the database",
                "bir_data-copy.db.zip",
                |destination| {
                    if let Ok(guard) = db.lock() {
                        let _ = guard.checkpoint();
                    }
                    crate::dev_mode::export_database_copy(&source, destination)
                },
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(saved) => {
                        this.message = Some((true, saved.message()));
                        this.last_export = Some(saved);
                    }
                    Err(error) => {
                        this.message = Some((false, error));
                        this.last_export = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The separate-database switch. Turning it on asks first.
    fn set_separate_database(&mut self, on: bool, cx: &mut Context<Self>) {
        if on {
            self.confirm_separate_db = true;
            cx.notify();
        } else {
            self.confirm_separate_db = false;
            self.save_startup_flag(keys::SEPARATE_DATABASE, false, cx);
        }
    }

    fn render_separate_db_confirm(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .mx_4()
            .my_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(crate::dev_mode::dev_orange())
            .bg(cx.theme().muted)
            .child(
                div()
                    .text_sm()
                    .child("Your profiles won't show until you switch this off again. Continue?"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("dev_separate_db_cancel")
                            .label("Cancel")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_separate_db = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("dev_separate_db_continue")
                            .label("Continue")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_separate_db = false;
                                this.save_startup_flag(keys::SEPARATE_DATABASE, true, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn seed_examples(&mut self, cx: &mut Context<Self>) {
        let result = match self.db.lock() {
            Ok(db) => db
                .seed_example_alerts(None)
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.message = Some(match result {
            Ok(()) => (true, "Example notifications added.".into()),
            Err(error) => (false, error),
        });
        cx.emit(DevModeEvent::AlertsSeeded);
        cx.notify();
    }
}

/// The startup-flags file decides the separate-database switch (it is what
/// the next start reads). Bring the `dev.separate_database` setting in line
/// with it when the two disagree.
fn sync_separate_database_setting(db: &Arc<Mutex<Database>>) {
    let resolved = dm::separate_database();
    if matches!(resolved.source, Source::Env(_)) {
        return;
    }
    let wanted = if resolved.value { "true" } else { "false" };
    let saved = dm::saved_value(keys::SEPARATE_DATABASE);
    if saved.as_deref().and_then(dm::parse_truthy) != Some(resolved.value)
        && let Ok(db) = db.lock()
        && let Err(error) = dm::save(&db, keys::SEPARATE_DATABASE, wanted)
    {
        tracing::warn!("could not sync {}: {error}", keys::SEPARATE_DATABASE);
    }
}

fn env_note(source: Source) -> Option<String> {
    match source {
        Source::Env(name) => Some(format!("set by env: {name}")),
        _ => None,
    }
}

impl DevModeView {
    fn row(
        &self,
        title: &str,
        description: &str,
        note: Option<String>,
        control: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .w_full()
            .justify_between()
            .items_center()
            .gap_4()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(description.to_string()),
                    )
                    .when_some(note, |col, note| {
                        col.child(
                            div()
                                .text_xs()
                                .text_color(crate::dev_mode::dev_orange())
                                .child(note),
                        )
                    }),
            )
            .child(control)
            .into_any_element()
    }

    fn flag_row(
        &self,
        id: &'static str,
        title: &str,
        description: &str,
        resolved: Resolved<bool>,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Read before the database opens, so they also go to the startup file.
        let startup = key == keys::VERBOSE_LOGS || key == keys::SEPARATE_DATABASE;
        let locked = matches!(resolved.source, Source::Env(_));
        let pending = key == keys::SEPARATE_DATABASE && self.confirm_separate_db;
        let switch = Switch::new(id)
            .checked(resolved.value || pending)
            .disabled(locked)
            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                if key == keys::SEPARATE_DATABASE {
                    this.set_separate_database(*checked, cx);
                } else if startup {
                    this.save_startup_flag(key, *checked, cx);
                } else {
                    this.save_flag(key, *checked, cx);
                }
            }));
        self.row(
            title,
            description,
            env_note(resolved.source),
            switch.into_any_element(),
            cx,
        )
    }

    fn heading(text: &str, cx: &Context<Self>) -> AnyElement {
        div()
            .px_4()
            .pt_6()
            .pb_2()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(cx.theme().muted_foreground)
            .child(text.to_string())
            .into_any_element()
    }

    fn submission_section(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let resolved = dm::submission();
        let live = dm::live_bir_env();
        let locked = live || matches!(resolved.source, Source::Env(_));
        let current = dm::panel_submission_mode();
        let mode_button = |id: &'static str, label: &'static str, mode: SubmissionMode| {
            let selected = current == Some(mode);
            Button::new(id)
                .label(label)
                .when(selected, |b| b.primary())
                .when(!selected, |b| b.outline())
                .disabled(locked)
        };
        let control = div()
            .flex()
            .gap_2()
            .child(
                mode_button("dev_submission_dry_run", "Dry run", SubmissionMode::DryRun).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.save(
                            keys::SUBMISSION_MODE,
                            SubmissionMode::DryRun.as_setting(),
                            cx,
                        )
                    }),
                ),
            )
            .child(
                mode_button(
                    "dev_submission_test_server",
                    "Test server",
                    SubmissionMode::TestServer,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.save(
                        keys::SUBMISSION_MODE,
                        SubmissionMode::TestServer.as_setting(),
                        cx,
                    )
                })),
            )
            .into_any_element();
        let note = if live {
            Some("LIVE BIR (env): BIR_SFTP_LIVE=1 was set at launch".to_string())
        } else {
            env_note(resolved.source)
        };
        let mut rows = vec![self.row(
            "Submission",
            "Where queued returns go. Live BIR is not offered here: it still needs BIR_SFTP_LIVE=1 at launch.",
            note,
            control,
            cx,
        )];

        // Test SFTP server fields: only while Submission is "Test server".
        if current != Some(SubmissionMode::TestServer) {
            return rows;
        }
        let fields = dm::test_server_fields();
        let muted = cx.theme().muted_foreground;
        let help_line = move |text: &'static str| div().text_xs().text_color(muted).child(text);
        let field_row = |label: &str,
                         input: &Entity<InputState>,
                         env: Option<&'static str>,
                         help: Option<&'static str>| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_sm().child(label.to_string()))
                .child(Input::new(input).disabled(env.is_some()))
                .when_some(help, |col, help| col.child(help_line(help)))
                .when_some(env, |col, name| {
                    col.child(
                        div()
                            .text_xs()
                            .text_color(crate::dev_mode::dev_orange())
                            .child(format!("set by env: {name}")),
                    )
                })
        };
        let accept = fields.accept_any_host_key.clone();
        let password_note = if fields.password_env {
            Some("set by env: BIR_SFTP_PASSWORD")
        } else if self.password_stored {
            Some("A password is stored in the OS keychain.")
        } else {
            None
        };
        rows.push(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Test SFTP server"),
                )
                .child(field_row("Host", &self.host, fields.host.env, None))
                .child(field_row("Port", &self.port, fields.port.env, None))
                .child(field_row(
                    "Username",
                    &self.username,
                    fields.username.env,
                    None,
                ))
                .child(field_row(
                    "Folder",
                    &self.folder,
                    fields.folder.env,
                    Some(FOLDER_HELP),
                ))
                .child(field_row(
                    "Host-key SHA-256",
                    &self.host_key,
                    fields.host_key_sha256.env,
                    Some(HOST_KEY_HELP),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child("Password"))
                        .child(
                            Input::new(&self.password)
                                .mask_toggle()
                                .disabled(fields.password_env),
                        )
                        .when_some(password_note, |col, note| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(note),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Switch::new("dev_sftp_accept_any")
                                        .checked(accept.value)
                                        .disabled(accept.env_name().is_some())
                                        .label("Accept any host key")
                                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                            this.save_flag(
                                                keys::SFTP_ACCEPT_ANY_HOST_KEY,
                                                *checked,
                                                cx,
                                            )
                                        })),
                                )
                                .when_some(env_note(accept.source), |row, note| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .text_color(crate::dev_mode::dev_orange())
                                            .child(note),
                                    )
                                }),
                        )
                        .child(help_line(ACCEPT_ANY_HELP)),
                )
                .child(
                    Button::new("dev_sftp_save")
                        .label("Save test server")
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.save_test_server(cx))),
                )
                .into_any_element(),
        );
        rows
    }

    fn database_section(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let path = crate::dev_mode::opened_database_path();
        let mut rows = vec![
            self.row(
                "Database",
                &path.display().to_string(),
                None,
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.last_export.is_some(), |row| {
                        row.child(
                            Button::new("dev_db_export_open")
                                .label("Open folder")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(saved) = &this.last_export
                                        && let Err(error) = crate::export_target::open_folder(saved)
                                    {
                                        this.message = Some((false, error));
                                        cx.notify();
                                    }
                                })),
                        )
                    })
                    .child(
                        Button::new("dev_db_export")
                            .label("Export a copy…")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.export_copy(cx))),
                    )
                    .into_any_element(),
                cx,
            ),
        ];
        rows.push(self.flag_row(
            "dev_separate_db",
            "Use a separate dev database",
            "Opens bir_data.dev.db instead of the normal database. Takes effect on next start.",
            dm::separate_database(),
            keys::SEPARATE_DATABASE,
            cx,
        ));
        if self.confirm_separate_db {
            rows.push(self.render_separate_db_confirm(cx));
        }
        rows
    }

    fn automation_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let compiled = cfg!(feature = "agent");
        let enabled = std::env::var("GPUI_AGENT").is_ok_and(|value| value.trim() == "1");
        let address = std::env::var("GPUI_AGENT_ADDR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "default loopback address".to_string());
        let status = if !compiled {
            "Not compiled in (build with --features agent)".to_string()
        } else if enabled {
            format!("On (GPUI_AGENT=1), address: {address}")
        } else {
            "Off (GPUI_AGENT is not 1)".to_string()
        };
        self.row(
            "Automation interface",
            "Read only. Set GPUI_AGENT / GPUI_AGENT_ADDR at launch.",
            None,
            div().text_sm().child(status).into_any_element(),
            cx,
        )
    }
}

impl Render for DevModeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let orange = crate::dev_mode::dev_orange();
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(orange)
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(orange)
                    .child("Dev Mode"),
            );

        let mut body: Vec<AnyElement> = Vec::new();
        if let Some((ok, message)) = self.message.clone() {
            body.push(
                div()
                    .m_4()
                    .p_3()
                    .rounded_md()
                    .text_sm()
                    .bg(cx.theme().muted)
                    .text_color(if ok {
                        cx.theme().foreground
                    } else {
                        cx.theme().danger
                    })
                    .child(message)
                    .into_any_element(),
            );
        }
        body.push(Self::heading("Submission", cx));
        body.extend(self.submission_section(cx));

        body.push(Self::heading("Validation and filing", cx));
        body.push(self.flag_row(
            "dev_relaxed_tin",
            "Relaxed TIN check",
            "Accept TINs that fail the official check digit.",
            dm::relaxed_tin(),
            keys::RELAXED_TIN,
            cx,
        ));
        body.push(self.flag_row(
            "dev_auto_confirm",
            "Auto-confirm",
            "Move a dry-run or test-server submission to Confirmed right after it reaches Submitted. Never applies to a live BIR send.",
            dm::auto_confirm(),
            keys::AUTO_CONFIRM,
            cx,
        ));
        body.push(self.flag_row(
            "dev_show_uncertified",
            "Show uncertified forms",
            "Let the dashboard open forms that are not yet certified.",
            bir_core::dev_mode::show_uncertified_forms(cfg!(feature = "dev-tools")),
            keys::SHOW_UNCERTIFIED_FORMS,
            cx,
        ));
        body.push(self.flag_row(
            "dev_form_diagnostics",
            "Developer diagnostics on forms",
            "2550Q candidate diagnostic, evidence note and raw identity controls.",
            bir_core::dev_mode::form_diagnostics(cfg!(feature = "dev-tools")),
            keys::FORM_DIAGNOSTICS,
            cx,
        ));

        body.push(Self::heading("Notifications and logs", cx));
        let alerts = dm::example_alerts();
        let alerts_locked = matches!(alerts.source, Source::Env(_));
        body.push(
            self.row(
                "Example notifications",
                "Seed one alert of each kind when the Notifications page opens.",
                env_note(alerts.source),
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("dev_seed_alerts")
                            .label("Seed examples now")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.seed_examples(cx))),
                    )
                    .child(
                        Switch::new("dev_example_alerts")
                            .checked(alerts.value)
                            .disabled(alerts_locked)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.save_flag(keys::EXAMPLE_ALERTS, *checked, cx)
                            })),
                    )
                    .into_any_element(),
                cx,
            ),
        );
        let verbose = dm::verbose_logs();
        let verbose_locked = matches!(verbose.source, Source::Env(_));
        body.push(
            self.row(
                "Verbose logs",
                "Debug-level logs with file and line. Takes effect on next start.",
                env_note(verbose.source),
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("dev_open_logs")
                            .label("Open log folder")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let dir = bir_core::log_files::log_dir();
                                let _ = std::fs::create_dir_all(&dir);
                                if let Err(error) = open::that_detached(&dir) {
                                    this.message = Some((
                                        false,
                                        format!("Could not open {}: {error}", dir.display()),
                                    ));
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Switch::new("dev_verbose_logs")
                            .checked(verbose.value)
                            .disabled(verbose_locked)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.save_startup_flag(keys::VERBOSE_LOGS, *checked, cx)
                            })),
                    )
                    .into_any_element(),
                cx,
            ),
        );

        body.push(Self::heading("Database", cx));
        body.extend(self.database_section(cx));

        body.push(Self::heading("Automation", cx));
        body.push(self.automation_section(cx));

        div()
            .id("dev_mode_panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .child(header)
            .child(
                div()
                    .id("dev_mode_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .w_full()
                            .max_w(px(900.))
                            .mx_auto()
                            .pb_8()
                            .children(body),
                    ),
            )
    }
}
