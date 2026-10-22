//! Help menu for Check for Updates and About, until application menus land.
//!
//! Draft PR #179 (`feat/shell-app-menus`) adds the real menu bar in
//! `app_menu.rs` and calls `cx.set_menus` itself. This module does not create
//! that file, so the two branches do not collide on a whole new module.
//!
//! On rebase onto #179:
//! 1. Delete [`install`]'s `set_menus` call. Leaving it would replace #179's
//!    menu bar with this Help menu alone.
//! 2. Add `CheckForUpdates` to `app_menu.rs`'s `actions!` list and register
//!    `cx.on_action(|_: &CheckForUpdates, _| crate::updater::check_now())`.
//! 3. Inside `help_menu`, before Contact Support…, push
//!    `MenuItem::action("Check for Updates…", CheckForUpdates)`.
//!    About eBIRForms already lives in that file (the app menu on macOS, Help
//!    on Linux and Windows). Drop [`AboutApplication`] here when that lands.
//! 4. `host.rs`'s `run` also gains a call from #179; keep one `updater::start()`
//!    after the database opens, next to whichever menu install remains.

use std::time::Duration;

use gpui_shell::gpui::{self, App, Menu, MenuItem};

gpui::actions!(bir_shell_updater, [CheckForUpdates, AboutApplication]);

/// Put Check for Updates… and About eBIRForms in the menu bar.
pub fn install(cx: &mut App) {
    cx.on_action(|_: &CheckForUpdates, _cx: &mut App| {
        crate::updater::check_now();
    });
    cx.on_action(|_: &AboutApplication, _cx: &mut App| {
        show_about();
    });
    cx.set_menus(vec![Menu::new("Help").items(vec![
        MenuItem::action("Check for Updates…", CheckForUpdates),
        MenuItem::separator(),
        MenuItem::action("About eBIRForms", AboutApplication),
    ])]);
    // Test hook: the relaunched process is started with this set so a
    // screenshot can show the version without a menu click.
    if std::env::var_os("BIR_SHOW_ABOUT").is_some() {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(2));
            show_about();
        });
    }
}

fn show_about() {
    let body = format!(
        "eBIRForms\nVersion {}\n© 2026 Goldcoders Corp",
        env!("CARGO_PKG_VERSION")
    );
    // rfd's macOS dialog hops to the main thread itself. Spawning keeps a
    // menu click from nesting a modal run loop inside the action handler.
    std::thread::spawn(move || {
        let _ = rfd::MessageDialog::new()
            .set_title("About eBIRForms")
            .set_description(&body)
            .set_level(rfd::MessageLevel::Info)
            .show();
    });
}
