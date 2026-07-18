#![allow(unexpected_cfgs)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::let_unit_value)]
#![allow(unused)]
#![allow(clippy::redundant_pattern_matching)]
// Suppress the console window on Windows release builds.
// Debug builds retain the console for tracing output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use gpui_component::*;

mod actions;
mod app;
mod auth_overlays;
mod components;
mod cor_evidence;
mod cor_ocr;
pub mod events;
mod ipc;
mod platform;
mod quit_guard;
mod sidebar;
mod theme;
mod views;

pub mod global_actions {
    gpui::actions!(
        bir_desktop,
        [
            SubmitCurrentForm,
            ToggleSidebar,
            ToggleSidebarMini,
            FocusSearch,
            CreateProfile,
            ToggleTheme,
            OpenCronTasks,
            OpenSettings,
            OpenCommandPalette,
            OpenGlobalDashboard,
            AboutApplication,
            QuitApplication,
            HideApplication,
            HideOthers,
            ShowAllApplications,
            CloseWindow,
            MinimizeWindow,
            ZoomWindow,
            ToggleFullScreen,
            BringAllToFront,
            OpenSupportEmail,
            OpenCompanyWebsite,
            ZoomIn,
            ZoomOut,
            ResetZoom,
            ToggleEditMode,
            SaveLayout,
            EditorNewBox,
            EditorDuplicateBox,
            EditorDeleteBox,
            EditorRenameField,
            EditorSetCharCount,
            EditorFocusSearch,
            EditorEscape,
            EditorNextField,
            EditorPrevField,
            EditorSelectBox1,
            EditorSelectBox2,
            EditorSelectBox3,
            EditorSelectBox4,
            EditorSelectBox5,
            EditorSelectBox6,
            EditorSelectBox7,
            EditorSelectBox8,
            EditorSelectBox9,
            EditorSelectLastBox,
            EditorCycleType,
            EditorToggleDirection,
            EditorNudgeUp,
            EditorNudgeDown,
            EditorNudgeLeft,
            EditorNudgeRight,
            NextPage,
            PrevPage,
            OpacityIncrease,
            OpacityDecrease
        ]
    );
}
use global_actions::*;

use std::path::PathBuf;

#[cfg(feature = "dev-tools")]
const DEV_EXPORT_LIVE_DATABASE_FLAG: &str = "--dev-export-live-database";

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
const DEV_NATIVE_EVIDENCE_ENVELOPE_FLAG: &str = "--dev-native-evidence-envelope";

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

    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Destination must have a valid UTF-8 file name".to_string())?;
    let temporary = destination.with_file_name(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    if let Err(error) = bir_core::export_existing_database_zip(&source, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("Could not export database: {error}"));
    }

    if let Err(error) = std::fs::rename(&temporary, destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "Could not atomically install {}: {error}",
            destination.display()
        ));
    }

    Ok(())
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

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
fn parse_dev_native_evidence_envelope<I>(arguments: I) -> Result<Option<PathBuf>, String>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(flag) = arguments.next() else {
        return Ok(None);
    };
    if flag != std::ffi::OsStr::new(DEV_NATIVE_EVIDENCE_ENVELOPE_FLAG) {
        return Ok(None);
    }

    let envelope = arguments.next().ok_or_else(|| {
        format!("{DEV_NATIVE_EVIDENCE_ENVELOPE_FLAG} requires an envelope JSON path")
    })?;
    if arguments.next().is_some() {
        return Err(format!(
            "{DEV_NATIVE_EVIDENCE_ENVELOPE_FLAG} accepts exactly one envelope JSON path"
        ));
    }
    Ok(Some(PathBuf::from(envelope)))
}

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
fn load_dev_native_evidence_envelope(
    path: &std::path::Path,
) -> Result<bir_print::html::RenderEnvelopeV1, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Evidence envelope metadata could not be read: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Evidence envelope must be a regular non-symlink file".to_string());
    }
    if metadata.len() > 4 * 1024 * 1024 {
        return Err("Evidence envelope exceeds the 4 MiB diagnostic limit".to_string());
    }
    let first = std::fs::read(path)
        .map_err(|error| format!("Evidence envelope could not be read: {error}"))?;
    let second = std::fs::read(path)
        .map_err(|error| format!("Evidence envelope could not be read twice: {error}"))?;
    if first != second {
        return Err("Evidence envelope changed while it was being read".to_string());
    }
    let envelope: bir_print::html::RenderEnvelopeV1 = serde_json::from_slice(&first)
        .map_err(|error| format!("Evidence envelope JSON is invalid: {error}"))?;
    if envelope.schema_version != bir_print::html::RENDER_CONTRACT_VERSION {
        return Err(format!(
            "Evidence envelope schema {} is not supported",
            envelope.schema_version
        ));
    }
    if envelope.form.code != "2551Q" || envelope.form.version != "2018" {
        return Err(format!(
            "Evidence envelope must target exactly 2551Q:2018, got {}:{}",
            envelope.form.code, envelope.form.version
        ));
    }
    Ok(envelope)
}

struct Assets {
    base: PathBuf,
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

fn main() {
    dotenvy::dotenv().ok();

    #[cfg(all(feature = "dev-tools", target_os = "macos"))]
    let dev_native_evidence_envelope = match parse_dev_native_evidence_envelope(std::env::args_os())
    {
        Ok(Some(path)) => match load_dev_native_evidence_envelope(&path) {
            Ok(envelope) => Some(envelope),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        },
        Ok(None) => None,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    #[cfg(feature = "dev-tools")]
    if let Some(exit_code) = run_dev_command_if_requested() {
        std::process::exit(exit_code);
    }

    // ── Single-Instance Enforcement ──────────────────────────────────────────
    // Order matters: IPC runs first because it sends a SHOW command to bring
    // the existing window forward (visible feedback for the user). The Win32
    // Mutex is the hard fallback in case IPC fails (e.g. port file stale).
    #[cfg(all(feature = "dev-tools", target_os = "macos"))]
    let is_dev_native_evidence_exercise = dev_native_evidence_envelope.is_some();
    #[cfg(not(all(feature = "dev-tools", target_os = "macos")))]
    let is_dev_native_evidence_exercise = false;
    if !is_dev_native_evidence_exercise {
        crate::ipc::prevent_multiple_instances();
        crate::platform::enforce_single_instance();
    }

    let developer_mode = std::env::var("DEVELOPER_MODE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    // Initialize structured logging for both stdout and file
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let logs_dir = bir_core::platform::data_dir().join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);

    let file_appender = tracing_appender::rolling::never(&logs_dir, "ebirforms.log");

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

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_file(developer_mode)
        .with_line_number(developer_mode);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    tracing::info!(
        "🔍 Tracing initialized (developer_mode: {})",
        developer_mode
    );

    let assets_dir = crate::platform::find_resource_dir("assets");
    let app = gpui_platform::application().with_assets(Assets { base: assets_dir });

    // When the user clicks the dock icon or re-launches via Alfred/Spotlight,
    // macOS fires applicationShouldHandleReopen. Restore the window.
    app.on_reopen(|_cx| {
        crate::platform::show_in_dock();
    });

    app.run(move |cx| {
        gpui_component::init(cx);
        crate::platform::bind_global_keys(cx);

        #[cfg(all(feature = "dev-tools", target_os = "macos"))]
        if let Some(envelope) = dev_native_evidence_envelope {
            let prepared = match crate::views::html_form_preview::prepare_html_form_preview(&envelope)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    eprintln!("Could not prepare deterministic 2551Q evidence preview: {error}");
                    cx.quit();
                    return;
                }
            };
            let options = WindowOptions {
                // Keep the external diagnostic window on the primary display.
                // `WindowBounds::centered` has produced a stale, disconnected-
                // display origin on real macOS runs, leaving only one pixel of
                // the window reachable by Accessibility automation.
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    // GPUI's pinned macOS backend converts this y-coordinate
                    // into an AppKit content-rect origin without subtracting
                    // the window height. Supplying the intended top inset plus
                    // the 900 px height therefore places the visible top edge
                    // at 40 px instead of clamping the window off-screen.
                    origin: point(px(40.), px(940.)),
                    size: size(px(1200.), px(900.)),
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("2551Q HTML Form Preview - Native Evidence".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = cx.open_window(options, move |window, cx| {
                cx.new(|cx| {
                    crate::views::html_form_preview::HtmlFormPreviewView::new(
                        prepared, window, cx,
                    )
                })
            }) {
                eprintln!("Could not open deterministic 2551Q evidence preview: {error}");
                cx.quit();
            }
            return;
        }

        crate::ipc::start_ipc_listener(cx);
        #[cfg(target_os = "macos")]
        let macos_quit_router = crate::platform::install_app_menu(cx);

        let bounds =
            gpui::Bounds::centered(None, gpui::size(gpui::px(1024.0), gpui::px(768.0)), cx);

        cx.spawn(async move |cx| {
            let (db, profiles) = cx
                .background_executor()
                .spawn(async move {
                    let db_path = bir_core::db::default_database_path();
                    if let Some(parent) = db_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let legacy_db_path = std::env::current_dir()
                        .unwrap_or_default()
                        .join("bir_data.db");
                    if !db_path.exists()
                        && legacy_db_path.exists()
                        && legacy_db_path.metadata().map(|m| m.len()).unwrap_or(0) > 0
                        && legacy_db_path != db_path
                    {
                        let _ = std::fs::copy(&legacy_db_path, &db_path);
                    }
                    let (db, recovered_backup) = bir_core::db::Database::open_or_recreate(&db_path)
                        .expect("Failed to open database");
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

                    let profiles = db.list_profiles().unwrap_or_default();
                    let db_arc = std::sync::Arc::new(std::sync::Mutex::new(db));

                    (db_arc, profiles)
                })
                .await;

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

            // Phase 3: System Tray Integration
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
                .expect("Failed to append tray menu items");

            let icon_data = include_bytes!("../../../assets/images/e_logo.png");
            let img = image::load_from_memory(icon_data)
                .expect("Failed to load tray icon")
                .into_rgba8();
            let (width, height) = img.dimensions();
            let tray_icon = tray_icon::Icon::from_rgba(img.into_raw(), width, height)
                .expect("Failed to create tray icon");

            let tray = tray_icon::TrayIconBuilder::new()
                .with_menu(Box::new(tray_menu))
                .with_tooltip("eBIRForms")
                .with_icon(tray_icon)
                .build()
                .unwrap();

            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("e-BIRForms".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Maximized(bounds)),
                window_min_size: Some(gpui::size(gpui::px(620.0), gpui::px(500.0))),
                ..Default::default()
            };

            let _ = cx.open_window(options, move |window, cx| {
                window.on_window_should_close(cx, |_, _cx| {
                    // Phase 4: Window Close & macOS Dock Hijacking
                    crate::platform::hide_from_dock();
                    false // Prevent window destruction
                });

                let view = cx.new(|cx| app::AppState::new(db, profiles, window, cx));
                let main_window = window.window_handle();
                let tray_app_state = view.clone();
                #[cfg(target_os = "macos")]
                macos_quit_router.bind(main_window, &view);
                #[cfg(target_os = "macos")]
                crate::platform::register_settings_menu_action(
                    main_window,
                    view.clone(),
                    cx,
                );

                // Listen to tray events
                let menu_channel = tray_icon::menu::MenuEvent::receiver();
                let tray_channel = tray_icon::TrayIconEvent::receiver();

                cx.spawn(async move |cx| {
                    let mut tray = Some(tray);

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
                            if event.id == show_i.id() {
                                cx.update(|cx| {
                                    crate::platform::show_in_dock();
                                    cx.activate(true);
                                });
                            } else if event.id == hide_tray_i.id() {
                                cx.update(|_cx| {
                                    crate::platform::hide_from_dock();
                                });
                            } else if event.id == quit_i.id() {
                                let should_stop = cx.update(|cx| {
                                    match main_window.update(cx, |_, window, cx| {
                                        tray_app_state.update(cx, |state, cx| {
                                            state.request_application_quit(window, cx, || {
                                                drop(tray.take());
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
                                let _ = cx.update(|_cx| {
                                    crate::platform::toggle_app_visibility();
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
                                        match crate::platform::build_hotkey(combo) {
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
