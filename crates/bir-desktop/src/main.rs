//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use gpui_component::*;

mod app;
mod components;
pub mod events;
mod platform;
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
            ToggleFullScreen
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
    let current_exe = std::env::current_exe().unwrap_or_default();
    let is_app_bundle = current_exe.to_string_lossy().contains("Contents/MacOS");
    let assets_dir = if is_app_bundle {
        current_exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources/assets")
    } else {
        // Fallback for `cargo run`
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    };
    gpui_platform::application()
        .with_assets(Assets { base: assets_dir })
        .run(move |cx| {
            gpui_component::init(cx);
            crate::platform::bind_global_keys(cx);

            let bounds =
                gpui::Bounds::centered(None, gpui::size(gpui::px(1024.0), gpui::px(768.0)), cx);

            cx.spawn(async move |cx| {
                let options = WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("e-BIRForms".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Maximized(bounds)),
                    window_min_size: Some(gpui::size(gpui::px(620.0), gpui::px(500.0))),
                    ..Default::default()
                };

                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| app::AppState::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
