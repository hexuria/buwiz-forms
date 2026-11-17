//! The desktop host (port of `nsh/shell/desktop/Desktop.tsx` + `actions.ts`): what makes the app a
//! desktop app — the updater driver tick, the About dialog, and `perform`, what a menu, tray or key
//! action does.
//!
//! What CUI cannot do yet is honest about it: there is no native application menu, no tray, no
//! window hide/focus or close-press callback and no global hotkey in cui-rs 0.1.3. The menu model,
//! hotkey machine and updater are ported and tested (`menu_model.rs`, `hotkey.rs`, `updater.rs`);
//! they get a consumer when platform code lands. What runs today: the updater command pump, the
//! About dialog, URL opening, and every `perform` action that maps onto `AppState`.

use std::cell::RefCell;

use cui::Ui;

use crate::menu_model::{self, MenuAction, Os};
use crate::state::{AppState, PanelName, ViewName};
use crate::ui::kit::UiExt;
use crate::updater::{self, Command as UpdateCommand};

/// The desktop platform this run is, or `None` where there are no desktop windows.
pub fn desktop_os() -> Option<Os> {
    // CUI only runs where it opened a window; the OS word decides the menu/hotkey policy.
    Some(match std::env::consts::OS {
        "macos" => Os::Macos,
        "windows" => Os::Windows,
        "linux" => Os::Linux,
        _ => return None,
    })
}

/// Everything the desktop host keeps between frames.
struct Host {
    /// The updater driver (updates.ts's `get()`), made on the first frame.
    driver: Option<updater::Updater>,
    /// The menu refresh counter last seen (menu_model rebuilds on change; no native menu yet).
    #[allow(dead_code)]
    seen_menu_revision: u32,
}

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host {
        driver: None,
        seen_menu_revision: 0,
    });
}

fn with_host<R>(f: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| f(&mut host.borrow_mut()))
}

/// The frame's desktop tick (DesktopHost's poll + updates.announce): reads what the host functions
/// queued, drives the updater, publishes its state, and says the print preview's honest answer.
pub fn tick(app: &mut AppState) {
    // A print preview was asked for on a device that cannot show one: say so, once each.
    for (_key, _title) in crate::bir::print_states().unavailable {
        app.toast(crate::print::UNAVAILABLE.to_owned(), None, None);
    }
    let poll = crate::bir::desktop_poll();
    with_host(|h| {
        if h.driver.is_none() {
            let auto = crate::bir::auto_download();
            let version = crate::store::system_info().app_version.clone();
            h.driver = Some(updater::Updater::new(
                Box::new(updater::UnconfiguredApi),
                auto,
                version,
            ));
        }
        if let Some(driver) = h.driver.as_mut() {
            driver.tick();
            for word in &poll.update_commands {
                if let Some(command) = UpdateCommand::parse(word) {
                    driver.handle(command);
                }
            }
            if driver.take_dirty() {
                let report = driver.report();
                let _ = crate::bir::publish_update(crate::desktop::UpdateReport {
                    configured: report.configured,
                    phase: report.phase.word().to_owned(),
                    version: report.version,
                    message: report.message,
                });
            }
        }
        // A second launch asked this window to come to the front: CUI has no focus API yet.
        let _ = poll.activate;
        // The menu refresh counter: no native menu to rebuild yet; the model is ready.
        h.seen_menu_revision = poll.menu_revision;
    });
}

/// Stops the updater's periodic check (the app is closing).
pub fn stop() {
    with_host(|h| {
        if let Some(driver) = h.driver.as_mut() {
            driver.stop();
        }
    });
}

/// "Check for Updates…" (updates.checkNow): the banner's phases answer through `update_status`; a
/// toast says it when there is no banner. Returns the sentence to toast, or `None`.
fn check_for_updates() -> Option<String> {
    with_host(|h| {
        let driver = h.driver.as_mut()?;
        if !driver.report().configured {
            return Some(updater::NOT_CONFIGURED.to_owned());
        }
        driver.handle(UpdateCommand::Check);
        let report = driver.report();
        match report.phase {
            updater::Phase::Ready | updater::Phase::Failed | updater::Phase::UpToDate => None,
            _ => Some(
                report
                    .message
                    .unwrap_or_else(|| "Checked for updates.".to_owned()),
            ),
        }
    })
}

/// Carries out one menu, tray or key action (actions.ts `perform`). Filing commands do nothing
/// while the chrome is hidden (the old dispatch_if_unlocked).
pub fn perform(app: &mut AppState, action: MenuAction) {
    let filing = app.chrome_visible();
    match action {
        MenuAction::About => app.about_open = true,
        MenuAction::Settings => {
            if filing {
                app.go(ViewName::Settings, Default::default());
            }
        }
        MenuAction::NewReturn => {
            if filing {
                app.start_new_return(None);
            }
        }
        MenuAction::NewTaxProfile => {
            if filing {
                app.go(ViewName::NewTaxpayer, Default::default());
            }
        }
        MenuAction::ToggleSidebar => {
            if filing {
                app.toggle_sidebar();
            }
        }
        MenuAction::ToggleProfile => {
            if filing {
                app.toggle_panel(PanelName::Profile);
            }
        }
        MenuAction::ToggleNotifications => {
            if filing {
                app.toggle_panel(PanelName::Notifs);
            }
        }
        MenuAction::ToggleTheme => {
            if filing {
                app.cycle_theme();
            }
        }
        MenuAction::Palette => {
            if filing {
                app.open_palette();
            }
        }
        MenuAction::SignOut => {
            if filing {
                app.log_out();
            }
        }
        MenuAction::CheckUpdates => {
            if filing {
                if let Some(note) = check_for_updates() {
                    app.toast(note, None, None);
                }
            }
        }
        MenuAction::Support => {
            if filing {
                cui::App::open_url(menu_model::SUPPORT_URL);
            }
        }
        MenuAction::Website => {
            if filing {
                cui::App::open_url(menu_model::WEBSITE_URL);
            }
        }
        // Hide-to-tray, show and bring-to-front need a window hide/focus API CUI does not have.
        MenuAction::Hide
        | MenuAction::CloseWindow
        | MenuAction::Show
        | MenuAction::BringAllToFront => {}
        // The quit accelerator quits on every platform (the user asked for Cmd+Q to end the
        // app, not hide to a tray CUI does not have anyway).
        MenuAction::Quit | MenuAction::QuitNow => app.quit(),
    }
}

/// What the update banner says for an update status (banner.ts `promptFor`).
pub struct Prompt {
    pub message: String,
    pub install: bool,
}

/// The banner for a status: a failed update says so (Later), a downloaded release offers Install
/// now and Later, a manual check that found nothing says "up to date" (Later). Every other phase,
/// and a build with no update source, shows nothing.
pub fn prompt_for(status: &bir_app_api::hooks::UpdateStatus) -> Option<Prompt> {
    match status.phase.as_str() {
        "failed" => Some(Prompt {
            message: status
                .message
                .clone()
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| updater::VERIFY_REJECTED.to_owned()),
            install: false,
        }),
        "ready" if status.version.is_some() => Some(Prompt {
            message: format!(
                "Version {} is ready.",
                status.version.as_deref().unwrap_or("")
            ),
            install: true,
        }),
        "up_to_date" if status.message.is_some() => Some(Prompt {
            message: status.message.clone().unwrap_or_default(),
            install: false,
        }),
        _ => None,
    }
}

/// What identifies the prompt on screen, so "Later" hides this one and a different one shows again.
pub fn prompt_key(status: &bir_app_api::hooks::UpdateStatus) -> String {
    format!(
        "{}|{}|{}",
        status.phase,
        status.version.as_deref().unwrap_or(""),
        status.message.as_deref().unwrap_or("")
    )
}

/// The update banner (UpdateSlot.tsx): a card when a downloaded release waits, an update failed,
/// or a check found nothing. With no update source configured the status is always idle.
pub fn draw_update_banner(ui: &mut Ui, app: &mut AppState) {
    let Some(status) = app.update_status.clone() else {
        return;
    };
    let key = prompt_key(&status);
    // "Later" hides that prompt only: when the status moves on the next one shows again.
    if !app.update_put_away.is_empty() && key != app.update_put_away {
        app.update_put_away.clear();
    }
    if key == app.update_put_away {
        return;
    }
    let Some(prompt) = prompt_for(&status) else {
        return;
    };
    let mut install = false;
    let mut later = false;
    ui.column(
        &cui::Column {
            align: cui::Align::End,
            ..Default::default()
        },
        |ui| {
            ui.card(
                &cui::Card {
                    width: cui::Size::px(420.0),
                    ..Default::default()
                },
                |ui| {
                    ui.row(&Default::default(), |ui| {
                        ui.text(&prompt.message);
                        ui.spacer();
                        if prompt.install && ui.primary_button("Install now", true) {
                            install = true;
                        }
                        if ui.ghost_button("Later", true) {
                            later = true;
                        }
                    });
                },
            );
        },
    );
    if later {
        app.update_put_away = key;
        let _ = crate::bir::call0("dismiss_update");
    }
    if install {
        let _ = crate::bir::call0("install_update");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_app_api::hooks::UpdateStatus;

    fn status(phase: &str, version: Option<&str>, message: Option<&str>) -> UpdateStatus {
        UpdateStatus {
            phase: phase.to_owned(),
            version: version.map(str::to_owned),
            message: message.map(str::to_owned),
            auto_download: true,
            current_version: "0.2.0",
        }
    }

    #[test]
    fn the_banner_words_match_the_old_prompt_for() {
        // failed says so (Later), ready offers Install now, up_to_date repeats the sentence.
        let failed = prompt_for(&status("failed", None, Some("offline"))).unwrap();
        assert_eq!(failed.message, "offline");
        assert!(!failed.install);
        // JS `||`: only an empty message falls back ("  " is truthy there too).
        let failed_blank = prompt_for(&status("failed", None, Some(""))).unwrap();
        assert_eq!(failed_blank.message, updater::VERIFY_REJECTED);
        let ready = prompt_for(&status("ready", Some("0.3.0"), None)).unwrap();
        assert_eq!(ready.message, "Version 0.3.0 is ready.");
        assert!(ready.install);
        let up_to_date = prompt_for(&status(
            "up_to_date",
            None,
            Some("eBIRForms 0.2.0 is up to date."),
        ))
        .unwrap();
        assert_eq!(up_to_date.message, "eBIRForms 0.2.0 is up to date.");
        assert!(!up_to_date.install);
        // Every other phase, and a build with no update source, shows nothing.
        for phase in ["idle", "checking", "downloading", "available", "installing"] {
            assert!(
                prompt_for(&status(phase, Some("0.3.0"), Some("x"))).is_none(),
                "{phase}"
            );
        }
        assert!(prompt_for(&status("ready", None, None)).is_none());
        assert!(prompt_for(&status("up_to_date", None, None)).is_none());
    }

    #[test]
    fn the_prompt_key_identifies_the_prompt() {
        let a = status("ready", Some("0.3.0"), None);
        assert_eq!(prompt_key(&a), "ready|0.3.0|");
        let b = status("ready", Some("0.4.0"), None);
        assert_ne!(prompt_key(&a), prompt_key(&b));
        let c = status("failed", None, Some("offline"));
        assert_eq!(prompt_key(&c), "failed||offline");
    }
}

/// The About dialog (Desktop.tsx's AboutDialog): title, version, copyright, OK.
pub fn draw_about(ui: &mut Ui, app: &mut AppState) {
    ui.dialog("About eBIRForms", &mut app.about_open, |ui| {
        ui.column(&Default::default(), |ui| {
            ui.heading("About eBIRForms", 3);
            ui.text(&format!(
                "Version {}",
                crate::store::system_info().app_version
            ));
            ui.muted(menu_model::COPYRIGHT);
            ui.row(&Default::default(), |ui| {
                ui.spacer();
                if ui.primary_button("OK", true) {
                    // The flag the dialog owns closes it; nothing else to do.
                }
            });
        });
    });
}
