//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use gpui_component::*;

mod app;
mod components;
mod views;

actions!(bir_desktop, [SubmitCurrentForm]);

fn main() {
    gpui_platform::application().run(move |cx| {
        gpui_component::init(cx);
        gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);
        cx.bind_keys([
            KeyBinding::new("cmd-enter", SubmitCurrentForm, None),
            KeyBinding::new("ctrl-enter", SubmitCurrentForm, None),
        ]);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("BIR eForms".into()),
                    ..Default::default()
                }),
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
