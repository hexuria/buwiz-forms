//! eBIRForms on CUI: the NSH (TSX) app's Rust brain behind the CUI immediate-mode UI.
//!
//! The process is set up before the window opens, in the order the NSH app's native-module
//! registration did it: the send guard and the hooks (`door::prepare_process`), then the
//! one-copy guard (`desktop::prepare_process`). Everything after that is the frame loop in
//! `app.rs`.

mod agent;
mod app;
mod auth;
mod bir;
mod desktop;
mod desktop_host;
mod door;
mod form;
mod guards;
mod hooks;
mod hotkey;
mod lib;
mod menu_model;
mod palette;
mod print;
mod screens;
mod seed;
mod state;
mod store;
mod theme;
mod types;
mod ui;
mod updater;

/// Set up the process once, before any window or thread of ours exists.
pub fn prepare() {
    door::prepare_process();
    desktop::prepare_process();
}

fn main() {
    prepare();
    std::process::exit(app::run());
}
