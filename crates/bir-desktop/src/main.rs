//! BIR Desktop Application — GPUI-powered tax filing interface.

use gpui::*;
use std::ops::Deref;
use gpui_component::*;

mod app;
mod views;

fn main() {
    gpui_platform::application()
        .run(move |cx| {
            gpui_component::init(cx);
            
            cx.spawn(async move |cx| {
                let options = WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("BIR eForms".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| app::AppState::new(cx));
                    cx.new(|cx| {
                        Root::new(view, window, cx)
                            .bg(cx.theme().background)
                    })
                }).expect("Failed to open window");
            }).detach();
        });
}
