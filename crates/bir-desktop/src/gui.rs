//! Painted GPUI entry. Called from the `bir` binary.

#[cfg(feature = "agent")]
use crate::agent;
use crate::global_actions::*;
use crate::{app, ipc, platform};
use gpui::*;
use gpui_component::*;

use std::path::PathBuf;

#[cfg(feature = "dev-tools")]
const DEV_EXPORT_LIVE_DATABASE_FLAG: &str = "--dev-export-live-database";

#[cfg(feature = "dev-tools")]
fn parse_dev_export_destination<I>(arguments: I) -> Result<Option<PathBuf>, String>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(flag) = arguments.next() else {
        return Ok(None);
    };
    if flag != std::ffi::OsStr::new(DEV_EXPORT_LIVE_DATABASE_FLAG) {
        return Ok(None);
    }

    let destination = arguments.next().ok_or_else(|| {
        format!("{DEV_EXPORT_LIVE_DATABASE_FLAG} requires a destination ZIP path")
    })?;
    if arguments.next().is_some() {
        return Err(format!(
            "{DEV_EXPORT_LIVE_DATABASE_FLAG} accepts exactly one destination ZIP path"
        ));
    }

    Ok(Some(PathBuf::from(destination)))
}

#[cfg(feature = "dev-tools")]
fn export_live_database_for_diagnostics(destination: &std::path::Path) -> Result<(), String> {
    let source = bir_core::db::default_database_path();
    crate::dev_mode::export_database_copy(&source, destination)
}

#[cfg(feature = "dev-tools")]
fn run_dev_command_if_requested() -> Option<i32> {
    match parse_dev_export_destination(std::env::args_os()) {
        Ok(None) => None,
        Err(error) => {
            eprintln!("{error}");
            Some(2)
        }
        Ok(Some(destination)) => match export_live_database_for_diagnostics(&destination) {
            Ok(()) => {
                println!("Exported live database to {}", destination.display());
                Some(0)
            }
            Err(error) => {
                eprintln!("{error}");
                Some(1)
            }
        },
    }
}

struct Assets {
    base: PathBuf,
}

/// One file per UTC day (`ebirforms.YYYY-MM-DD.log`). Older days are pruned
/// by `bir_core::log_files::maintain` once the database — and with it the
/// user's retention setting — is open; see `run_gui`.
fn persistent_log_appender() -> Option<tracing_appender::rolling::RollingFileAppender> {
    let logs_dir = bir_core::log_files::log_dir();
    let _ = std::fs::create_dir_all(&logs_dir);
    tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(bir_core::log_files::LOG_FILE_PREFIX)
        .filename_suffix(bir_core::log_files::LOG_FILE_SUFFIX)
        .build(&logs_dir)
        .map_err(|error| eprintln!("log file appender unavailable: {error}"))
        .ok()
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(|err| err.into())
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        std::fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(gpui::SharedString::from)
                    })
                    .collect()
            })
            .map_err(|err| err.into())
    }
}

struct TrayUi {
    tray: tray_icon::TrayIcon,
    show_i: tray_icon::menu::MenuItem,
    hide_tray_i: tray_icon::menu::MenuItem,
    quit_i: tray_icon::menu::MenuItem,
}

fn try_create_tray() -> Option<TrayUi> {
    let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let tray_menu = tray_icon::menu::Menu::new();
        let show_i = tray_icon::menu::MenuItem::new("Show eBIRForms", true, None);
        let hide_tray_i = tray_icon::menu::MenuItem::new("Hide eBIRForms", true, None);
        let quit_i = tray_icon::menu::MenuItem::new("Quit", true, None);
        tray_menu
            .append_items(&[
                &show_i,
                &hide_tray_i,
                &tray_icon::menu::PredefinedMenuItem::separator(),
                &quit_i,
            ])
            .map_err(|error| error.to_string())?;

        let icon_data = include_bytes!("../../../assets/images/e_logo.png");
        let img = image::load_from_memory(icon_data)
            .map_err(|error| error.to_string())?
            .into_rgba8();
        let (width, height) = img.dimensions();
        let tray_icon = tray_icon::Icon::from_rgba(img.into_raw(), width, height)
            .map_err(|error| error.to_string())?;

        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("eBIRForms")
            .with_icon(tray_icon)
            .build()
            .map_err(|error| error.to_string())?;
        Ok::<TrayUi, String>(TrayUi {
            tray,
            show_i,
            hide_tray_i,
            quit_i,
        })
    }));
    match created {
        Ok(Ok(ui)) => Some(ui),
        Ok(Err(error)) => {
            tracing::warn!(%error, "system tray unavailable; continuing without it");
            None
        }
        Err(_) => {
            tracing::warn!("system tray panicked during create; continuing without it");
            None
        }
    }
}

pub fn run_gui() {
    dotenvy::dotenv().ok();

    #[cfg(feature = "dev-tools")]
    if let Some(exit_code) = run_dev_command_if_requested() {
        std::process::exit(exit_code);
    }

    ipc::prevent_multiple_instances();
    platform::enforce_single_instance();

    // `DEVELOPER_MODE=true` (any build), or the Dev Mode "Verbose logs"
    // switch (debug builds; read here, so it takes effect on next start).
    let developer_mode = std::env::var("DEVELOPER_MODE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
        || bir_core::dev_mode::verbose_logs().value;

    // Initialize structured logging. The native-output diagnostic intentionally
    // remains stdout-only because its external driver captures that stream and
    // must not depend on the mutable shared app-group log.
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // The binary crate is `bir` (bin target name); its modules log under
        // that target, not `bir_desktop`, so both are listed to avoid silently
        // dropping app-level logs.
        if developer_mode {
            "bir=debug,bir_desktop=debug,bir_print=debug,bir_core=info"
                .parse()
                .unwrap()
        } else {
            "bir=info,bir_desktop=info,bir_print=error,bir_core=info"
                .parse()
                .unwrap()
        }
    });

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_file(developer_mode)
        .with_line_number(developer_mode);

    let file_layer = persistent_log_appender().map(|appender| {
        tracing_subscriber::fmt::layer()
            .with_writer(appender)
            .with_ansi(false)
            .with_target(true)
            .with_file(developer_mode)
            .with_line_number(developer_mode)
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    tracing::info!(
        "🔍 Tracing initialized (developer_mode: {})",
        developer_mode
    );

    // tray-icon's muda `Menu` panics on Linux unless GTK is initialized first.
    // Do this before GPUI opens the X11 client so GDK can attach a screen.
    #[cfg(target_os = "linux")]
    match gtk::init() {
        Ok(()) => tracing::info!("GTK initialized for system tray / WebKitGTK"),
        Err(error) => tracing::warn!(%error, "gtk::init failed; system tray may be unavailable"),
    }

    let assets_dir = platform::find_resource_dir("assets");
    let app = gpui_kit::application().with_assets(Assets { base: assets_dir });

    // When the user clicks the dock icon or re-launches via Alfred/Spotlight,
    // macOS fires applicationShouldHandleReopen. Restore the window.
    app.on_reopen(|_cx| {
        platform::show_in_dock();
    });

    app.run(move |cx| {
        gpui_kit::init(cx);
        platform::bind_global_keys(cx);

        ipc::start_ipc_listener(cx);
        #[cfg(target_os = "macos")]
        let macos_quit_router = platform::install_app_menu(cx);

        // About 90% of the primary display on first start, then the last
        // size and position, clamped to the display (`window_geometry`).
        let bounds = match cx.primary_display() {
            Some(display) => crate::window_geometry::to_bounds(crate::window_geometry::initial(
                crate::window_geometry::from_bounds(display.bounds()),
                crate::window_geometry::load(),
            )),
            None => gpui::Bounds::centered(None, gpui::size(gpui::px(1024.0), gpui::px(768.0)), cx),
        };

        cx.spawn(async move |cx| {
            let (db, profiles) = cx
                .background_executor()
                .spawn(async move {
                    let (db, db_path, recovered_backup) = match bir_core::db::open_live_database()
                    {
                        Ok(opened) => opened,
                        Err(error) => {
                            eprintln!("{error}");
                            if matches!(error, bir_core::db::DbError::LiveDatabaseInUse(_)) {
                                eprintln!(
                                    "Shut bir-headless first (`bir-headless shutdown`), then open painted bir. \
                                     Reopen is offline verification of the shared DB, not live sync."
                                );
                            }
                            std::process::exit(1);
                        }
                    };
                    // Dev Mode values (debug builds) before anything resolves them.
                    bir_core::dev_mode::load_saved(&db);
                    crate::dev_mode::set_opened_database_path(db_path.clone());
                    if let Some(backup_path) = recovered_backup {
                        eprintln!(
                            "Recovered unreadable database at {} by moving it to {}",
                            db_path.display(),
                            backup_path.display()
                        );
                    }

                    bir_core::reference::get_all_rdos();
                    bir_core::reference::get_all_zipcodes();
                    bir_core::reference::get_all_tax_types();
                    bir_core::reference::get_all_regions();

                    // Adopt the pre-rotation ebirforms.log and prune to the
                    // user's retention. The appender only rotates; this bounds.
                    let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), &db);

                    let profiles = db.list_profiles().unwrap_or_default();
                    let db_arc = std::sync::Arc::new(std::sync::Mutex::new(db));

                    (db_arc, profiles)
                })
                .await;

            // Log retention: re-check every six hours so a long-running
            // instance crosses midnight without growing past the setting.
            {
                let prune_db = db.clone();
                let executor = cx.background_executor().clone();
                cx.background_executor()
                    .spawn(async move {
                        loop {
                            executor
                                .timer(std::time::Duration::from_secs(6 * 60 * 60))
                                .await;
                            if let Ok(guard) = prune_db.lock() {
                                let _ = bir_core::log_files::maintain(
                                    &bir_core::log_files::log_dir(),
                                    &guard,
                                );
                            }
                        }
                    })
                    .detach();
            }

            // Phase 2: In-App Background Orchestrator
            let cron_db = db.clone();
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(async move {
                        bir_core::background_cron::start_cron_jobs(cron_db).await;
                    });
                } else {
                    eprintln!("Failed to initialize Tokio runtime for background tasks");
                }
            });

            // Phase 2b: Global Hotkey Listener (all platforms, except Mac App
            // Store). The hotkey manager is created and kept alive inside the
            // polling task below — a manager dropped here would immediately
            // unregister the hotkey. This DB handle lets that task re-read the
            // stored combo and re-register when the user changes it.
            #[cfg(not(feature = "mas_build"))]
            let hotkey_db = db.clone();

            #[cfg(feature = "agent")]
            let agent_session = agent::maybe_start();

            // Phase 3: System Tray Integration
            let tray_ui = try_create_tray();
            let (tray, show_i, hide_tray_i, quit_i) = match tray_ui {
                Some(ui) => (
                    Some(ui.tray),
                    Some(ui.show_i),
                    Some(ui.hide_tray_i),
                    Some(ui.quit_i),
                ),
                None => (None, None, None, None),
            };

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("e-BIRForms".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(gpui::size(gpui::px(620.0), gpui::px(500.0))),
                // X11 WM_CLASS / Wayland app_id; must match StartupWMClass in
                // assets/linux/ebirforms.desktop so docks group the window.
                app_id: Some("ebirforms".into()),
                ..Default::default()
            };

            let _ = cx.open_window(options, move |window, cx| {
                let view = cx.new(|cx| {
                    let mut state = app::AppState::new(db, profiles, window, cx);
                    #[cfg(feature = "agent")]
                    if let Some(session) = agent_session.clone() {
                        state.attach_agent(
                            session.mailbox,
                            session.token,
                            session.shutdown,
                            cx,
                        );
                        state.agent_discovery = session.discovery;
                    }
                    state
                });
                // Remember the size and position for the next start.
                view.update(cx, |_, cx| {
                    cx.observe_window_bounds(window, |_, window, _| {
                        let bounds = window.window_bounds().get_bounds();
                        crate::window_geometry::save(crate::window_geometry::from_bounds(bounds));
                    })
                    .detach();
                });
                cx.on_action({
                    let entity = view.downgrade();
                    move |_: &ToggleAppVisibility, cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.perform_toggle_app_visibility(cx);
                        });
                    }
                });
                #[cfg(feature = "agent")]
                if agent_session.is_some() {
                    crate::agent::keybindings::register_global_actions(
                        view.downgrade(),
                        window.window_handle(),
                        cx,
                    );
                }
                // Shared so both the tray Quit item and the Linux close button
                // can drop the tray icon before quitting.
                let tray = std::rc::Rc::new(std::cell::RefCell::new(tray));
                if platform::close_button_quits() {
                    // Linux: there is often no StatusNotifier tray host (GNOME,
                    // fluxbox) and no reliable way to hide a window (xdotool is
                    // optional and X11-only), so the close button quits through
                    // the same guarded path as tray Quit. The window stays open
                    // only when unsaved Profile compliance changes block quitting.
                    let entity = view.downgrade();
                    let close_tray = tray.clone();
                    // Ctrl+Q / Alt+F4 (QuitApplication) take the same path.
                    let hook_tray = tray.clone();
                    view.update(cx, |state, _| {
                        state.quit_hook = Some(std::rc::Rc::new(move || {
                            drop(hook_tray.borrow_mut().take());
                        }));
                    });
                    window.on_window_should_close(cx, move |window, cx| {
                        entity
                            .update(cx, |state, cx| {
                                state.request_application_quit(window, cx, || {
                                    drop(close_tray.borrow_mut().take());
                                })
                            })
                            .unwrap_or(false)
                    });
                } else {
                    window.on_window_should_close(cx, |_, _cx| {
                        // macOS/Windows: hide is not quit. Bind + live-DB owner lock
                        // stay held until tray Quit / `gpui-agent shutdown`
                        // (request_application_quit). Cmd+Q / the menu's Quit (macOS)
                        // and Alt+F4 (Windows) also hide rather than quit.
                        platform::hide_from_dock();
                        false // Prevent window destruction
                    });
                }
                let main_window = window.window_handle();
                let tray_app_state = view.clone();
                #[cfg(target_os = "macos")]
                macos_quit_router.bind(main_window, &view);
                #[cfg(target_os = "macos")]
                platform::register_settings_menu_action(
                    main_window,
                    view.clone(),
                    cx,
                );

                // Listen to tray events
                let menu_channel = tray_icon::menu::MenuEvent::receiver();
                let tray_channel = tray_icon::TrayIconEvent::receiver();

                cx.spawn(async move |cx| {
                    let tray = tray;

                    // Own the global hotkey manager here so it stays alive for
                    // the app's lifetime (dropping it unregisters the hotkey).
                    #[cfg(not(feature = "mas_build"))]
                    let hotkey_manager = match global_hotkey::GlobalHotKeyManager::new() {
                        Ok(manager) => Some(manager),
                        Err(error) => {
                            tracing::warn!(%error, "Failed to initialize global hotkey manager");
                            None
                        }
                    };
                    #[cfg(not(feature = "mas_build"))]
                    let mut registered_hotkey: Option<global_hotkey::hotkey::HotKey> = None;
                    #[cfg(not(feature = "mas_build"))]
                    let mut current_combo: Option<String> = None;
                    #[cfg(not(feature = "mas_build"))]
                    let mut hotkey_tick: u32 = 0;

                    loop {
                        if let Ok(event) = menu_channel.try_recv() {
                            if show_i.as_ref().is_some_and(|item| event.id == item.id()) {
                                cx.update(|cx| {
                                    platform::show_in_dock();
                                    cx.activate(true);
                                });
                            } else if hide_tray_i.as_ref().is_some_and(|item| event.id == item.id())
                            {
                                cx.update(|_cx| {
                                    platform::hide_from_dock();
                                });
                            } else if quit_i.as_ref().is_some_and(|item| event.id == item.id()) {
                                let should_stop = cx.update(|cx| {
                                    match main_window.update(cx, |_, window, cx| {
                                        tray_app_state.update(cx, |state, cx| {
                                            state.request_application_quit(window, cx, || {
                                                drop(tray.borrow_mut().take());
                                            })
                                        })
                                    }) {
                                        Ok(should_quit) => should_quit,
                                        Err(error) => {
                                            tracing::warn!(%error, "Could not route tray Quit through the application state");
                                            false
                                        }
                                    }
                                });
                                if should_stop {
                                    break;
                                }
                            }
                        }

                        // We no longer bring the app to foreground on tray click.
                        // This allows native tray menus to open without side effects.
                        if let Ok(_event) = tray_channel.try_recv() {}

                        // Poll global hotkey events (cross-platform toggle).
                        // The crate emits an event for both key press AND
                        // release; toggling on both would hide then instantly
                        // re-show the window. Act only on the press.
                        #[cfg(not(feature = "mas_build"))]
                        while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv()
                        {
                            if event.state == global_hotkey::HotKeyState::Pressed {
                                let _ = cx.update(|cx| {
                                    cx.dispatch_action(&ToggleAppVisibility);
                                });
                            }
                        }

                        // Re-register the global hotkey when the stored combo
                        // changes (checked immediately, then ~once per second)
                        // so Settings changes apply without a restart.
                        #[cfg(not(feature = "mas_build"))]
                        {
                            hotkey_tick = hotkey_tick.wrapping_add(1);
                            if hotkey_tick == 1 || hotkey_tick.is_multiple_of(10) {
                                let stored = hotkey_db
                                    .lock()
                                    .ok()
                                    .and_then(|db| {
                                        db.get_setting("global_hotkey_key").ok().flatten()
                                    })
                                    .filter(|value| !value.is_empty());
                                if stored != current_combo {
                                    if let (Some(manager), Some(old)) =
                                        (hotkey_manager.as_ref(), registered_hotkey.take())
                                    {
                                        let _ = manager.unregister(old);
                                    }
                                    if let (Some(manager), Some(combo)) =
                                        (hotkey_manager.as_ref(), stored.as_deref())
                                    {
                                        match platform::build_hotkey(combo) {
                                            Some(hotkey) => match manager.register(hotkey) {
                                                Ok(()) => {
                                                    registered_hotkey = Some(hotkey);
                                                    tracing::info!(
                                                        "Global hotkey registered: {combo}"
                                                    );
                                                }
                                                Err(error) => tracing::warn!(
                                                    %error,
                                                    "Failed to register global hotkey '{combo}'"
                                                ),
                                            },
                                            None => tracing::warn!(
                                                "Invalid global hotkey combo '{combo}'"
                                            ),
                                        }
                                    }
                                    current_combo = stored;
                                }
                            }
                        }

                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                    }
                })
                .detach();

                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            });
        })
        .detach();
    });
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn persistent_log_appender_writes_under_the_app_data_dir() {
        assert!(persistent_log_appender().is_some());
    }

    #[::core::prelude::v1::test]
    fn linux_desktop_entry_has_required_keys_and_is_packaged() {
        let entry = include_str!("../../../assets/linux/ebirforms.desktop");
        let mut lines = entry.lines();
        assert_eq!(lines.next(), Some("[Desktop Entry]"));
        let keys: std::collections::HashMap<&str, &str> =
            lines.filter_map(|l| l.split_once('=')).collect();
        assert_eq!(keys.get("Type"), Some(&"Application"));
        assert_eq!(keys.get("Name"), Some(&"eBIRForms"));
        assert_eq!(keys.get("Exec"), Some(&"ebirforms"));
        assert_eq!(keys.get("Icon"), Some(&"ebirforms"));
        assert_eq!(keys.get("StartupWMClass"), Some(&"ebirforms"));
        assert!(keys.get("Categories").is_some_and(|c| c.ends_with(';')));

        let cargo = include_str!("../Cargo.toml");
        assert!(cargo.contains(
            r#"["../../assets/linux/ebirforms.desktop", "usr/share/applications/ebirforms.desktop", "644"]"#
        ));
        assert!(cargo.contains("usr/share/icons/hicolor/"));
    }
}
