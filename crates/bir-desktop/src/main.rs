//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use gpui_component::*;

mod actions;
mod app;
mod auth_overlays;
mod components;
pub mod events;
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

    // Initialize structured logging for debug builds.
    // Controlled via RUST_LOG env var, e.g.: RUST_LOG=bir_desktop=debug,bir_print=debug
    #[cfg(debug_assertions)]
    {
        use tracing_subscriber::EnvFilter;
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "bir_desktop=debug,bir_print=debug,bir_core=info"
                    .parse()
                    .unwrap()
            }))
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .init();
        tracing::info!("🔍 Tracing initialized (debug build)");
    }

    let assets_dir = crate::platform::find_resource_dir("assets");
    gpui_platform::application()
        .with_assets(Assets { base: assets_dir })
        .run(move |cx| {
            gpui_component::init(cx);
            crate::platform::bind_global_keys(cx);

            let bounds =
                gpui::Bounds::centered(None, gpui::size(gpui::px(1024.0), gpui::px(768.0)), cx);

            cx.spawn(async move |cx| {
                let (db, profiles) = cx.background_executor().spawn(async move {
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
                    (std::sync::Arc::new(std::sync::Mutex::new(db)), profiles)
                }).await;

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
                    window.on_window_should_close(cx, |_, cx| {
                        cx.quit();
                        true
                    });

                    let view = cx.new(|cx| app::AppState::new(db, profiles, window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                });
            })
            .detach();
        });
}
