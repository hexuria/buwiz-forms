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
pub mod events;
mod ipc;
mod platform;
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
            QuitApplication,
            HideApplication,
            HideOthers,
            CloseWindow,
            MinimizeWindow,
            ToggleFullScreen,
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
        if developer_mode {
            "bir_desktop=debug,bir_print=debug,bir_core=info"
                .parse()
                .unwrap()
        } else {
            "bir_desktop=error,bir_print=error,bir_core=error"
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

    crate::ipc::prevent_multiple_instances();

    let assets_dir = crate::platform::find_resource_dir("assets");
    gpui_platform::application()
        .with_assets(Assets { base: assets_dir })
        .run(move |cx| {
            crate::ipc::start_ipc_listener(cx);

            gpui_component::init(cx);
            crate::platform::bind_global_keys(cx);

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
                        let (db, recovered_backup) =
                            bir_core::db::Database::open_or_recreate(&db_path)
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

                    // Listen to tray events
                    let menu_channel = tray_icon::menu::MenuEvent::receiver();
                    let tray_channel = tray_icon::TrayIconEvent::receiver();

                    cx.spawn(async move |cx| {
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
                                    cx.update(|cx| {
                                        // Ensure the tray icon is dropped properly before quitting
                                        drop(tray);
                                        cx.quit();
                                    });
                                    break;
                                }
                            }

                            // We no longer bring the app to foreground on tray click.
                            // This allows native tray menus to open without side effects.
                            if let Ok(_event) = tray_channel.try_recv() {}
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(100))
                                .await;
                        }
                    })
                    .detach();

                    let view = cx.new(|cx| app::AppState::new(db, profiles, window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                });
            })
            .detach();
        });
}
