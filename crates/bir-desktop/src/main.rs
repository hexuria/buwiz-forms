//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use gpui_component::*;

mod app;
mod components;
pub mod events;
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
            OpenGlobalDashboard
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
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    gpui_platform::application()
        .with_assets(Assets { base: assets_dir })
        .run(move |cx| {
            gpui_component::init(cx);
            #[cfg(target_os = "macos")]
            cx.bind_keys([
                KeyBinding::new("cmd-enter", SubmitCurrentForm, None),
                KeyBinding::new("cmd-b", ToggleSidebar, None),
                KeyBinding::new("cmd-shift-b", ToggleSidebarMini, None),
                KeyBinding::new("cmd-f", FocusSearch, None),
                KeyBinding::new("cmd-n", CreateProfile, None),
                KeyBinding::new("cmd-t", ToggleTheme, None),
                KeyBinding::new("cmd-shift-x", OpenCronTasks, None),
                KeyBinding::new("cmd-,", OpenSettings, None),
                KeyBinding::new("cmd-k", OpenCommandPalette, None),
                KeyBinding::new("f1", OpenGlobalDashboard, None),
            ]);

            #[cfg(not(target_os = "macos"))]
            cx.bind_keys([
                KeyBinding::new("ctrl-enter", SubmitCurrentForm, None),
                KeyBinding::new("ctrl-b", ToggleSidebar, None),
                KeyBinding::new("ctrl-shift-b", ToggleSidebarMini, None),
                KeyBinding::new("ctrl-f", FocusSearch, None),
                KeyBinding::new("ctrl-n", CreateProfile, None),
                KeyBinding::new("ctrl-t", ToggleTheme, None),
                KeyBinding::new("ctrl-shift-x", OpenCronTasks, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
                KeyBinding::new("ctrl-k", OpenCommandPalette, None),
                KeyBinding::new("f1", OpenGlobalDashboard, None),
            ]);

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
