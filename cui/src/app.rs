//! The app: CUI window, screen routing, per-frame state.
//!
//! One `AppState` (the port of `nsh/shell/app.ts`) and one `SharedAuth` live for the process. Each
//! frame: `state.tick()` runs the polls, `auth::draw` may cover the whole window (gate, setup,
//! sign-in), else the shell draws — sidebar, main surface with the routed screen, toasts.

use std::sync::atomic::{AtomicBool, Ordering};

use cui::{ButtonVariant, Key, Mods, Ui};

use crate::desktop_host;
use crate::menu_model::{self, MenuAction, MenuEntry, Os};
use crate::screens::{self, auth};
use crate::state::{self, AppState, PanelName, ViewName};
use crate::ui::kit::{self, UiExt};

/// Whether the CUI window is up. The hooks ask before showing a panel: with no window there is no UI to ask.
static UI_RUNNING: AtomicBool = AtomicBool::new(false);

/// Whether the app's window is running (set by [`run`], cleared never: the process ends with the window).
pub fn ui_running() -> bool {
    UI_RUNNING.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------- shell chrome

/// The sidebar's entries: the routes a person navigates to, in order (routes.ts TABLE icons).
const NAV: &[(ViewName, &str, &str)] = &[
    (ViewName::Home, "Home", "house"),
    (ViewName::Library, "Form library", "file-text"),
    (ViewName::Calendar, "Calendar", "calendar"),
    (ViewName::Filings, "Filings", "history"),
    (ViewName::Settings, "Settings", "settings"),
];

fn draw_sidebar(ui: &mut Ui, app: &mut AppState) {
    ui.column(
        &cui::Column {
            width: cui::Size::px(app.sidebar_width),
            gap: 2.0,
            pad: crate::theme::space(3),
            ..Default::default()
        },
        |ui| {
            ui.heading("eBIRForms", 2);
            // taxpayer picker
            let taxpayers = app.taxpayers();
            if !taxpayers.is_empty() {
                ui.muted("Taxpayer");
                for tp in &taxpayers {
                    let branches = tp.branches.len();
                    let label = if branches > 1 {
                        format!("{} ({branches} branches)", tp.name)
                    } else {
                        tp.name.clone()
                    };
                    let active = app.taxpayer().is_some_and(|t| t.tin_root == tp.tin_root);
                    if ui.list_item_with(
                        &label,
                        &cui::ListItem {
                            icon: Some(crate::ui::icons::glyph("user")),
                            selected: active,
                            ..Default::default()
                        },
                    ) && !active
                    {
                        app.select_taxpayer(&tp.tin_root, false, None);
                    }
                }
                // branch picker when the taxpayer has several
                if let Some(tp) = app.taxpayer() {
                    if tp.branches.len() > 1 {
                        ui.muted("Branch");
                        for b in &tp.branches {
                            let active = app.branch().is_some_and(|x| x.tin == b.tin);
                            let label = format!("{} — {}", b.code, b.name);
                            if ui.list_item_with(
                                &label,
                                &cui::ListItem {
                                    icon: Some(crate::ui::icons::glyph("git-branch")),
                                    selected: active,
                                    ..Default::default()
                                },
                            ) && !active
                            {
                                app.select_branch(&b.code);
                            }
                        }
                    }
                }
                // year picker
                let years = app.years();
                if !years.is_empty() {
                    ui.muted("Tax year");
                    ui.row(&Default::default(), |ui| {
                        for y in years.iter().take(6) {
                            let active = app.year() == Some(*y);
                            if ui.ghost_button(&y.to_string(), true) && !active {
                                app.select_year(*y);
                            }
                        }
                    });
                }
            } else {
                ui.muted("No taxpayers yet.");
            }
            ui.gap(crate::theme::space(4));
            // nav
            let view = app.view();
            for &(v, label, icon) in NAV {
                let active = view == v;
                if ui.list_item_with(
                    label,
                    &cui::ListItem {
                        icon: Some(crate::ui::icons::glyph(icon)),
                        selected: active,
                        ..Default::default()
                    },
                ) && !active
                {
                    app.go(v, Default::default());
                }
            }
            // taxpayer pages when a taxpayer is selected
            if app.taxpayer().is_some() {
                ui.gap(crate::theme::space(4));
                ui.muted("This taxpayer");
                for (v, label, icon) in [
                    (ViewName::Profile, "Tax profile", "user"),
                    (ViewName::Forms, "Forms", "file-text"),
                    (ViewName::Templates, "Templates", "layout-template"),
                    (ViewName::NewBranch, "New branch", "git-branch"),
                ] {
                    if ui.list_item_with(
                        label,
                        &cui::ListItem {
                            icon: Some(crate::ui::icons::glyph(icon)),
                            ..Default::default()
                        },
                    ) {
                        app.go(v, Default::default());
                    }
                }
            }
        },
    );
}

/// The narrow layout's bottom tab bar: the four primary destinations (state::TABS).
fn draw_tab_bar(ui: &mut Ui, app: &mut AppState) {
    ui.row(
        &cui::Row {
            gap: 0.0,
            pad: crate::theme::space(1),
            ..Default::default()
        },
        |ui| {
            for &(v, label, icon) in state::TABS.iter() {
                let active = app.view() == v;
                if ui.button_with(
                    label,
                    &cui::Button {
                        icon: Some(crate::ui::icons::glyph(icon)),
                        variant: cui::ButtonVariant::Ghost,
                        ..Default::default()
                    },
                ) && !active
                {
                    app.go(v, Default::default());
                }
            }
        },
    );
}

// ---------------------------------------------------------------- menu bar

/// A menu shortcut ("mod+shift+b") as the menubar shows it ("Ctrl+Shift+B" / "⌘⇧B"-style text).
fn menu_shortcut_label(shortcut: &str, os: Os) -> String {
    let mac = os == Os::Macos;
    shortcut
        .split('+')
        .map(|word| {
            let w = word.trim().to_ascii_lowercase();
            match w.as_str() {
                "mod" => {
                    if mac {
                        "⌘".to_owned()
                    } else {
                        "Ctrl".to_owned()
                    }
                }
                "control" | "ctrl" => "⌃".to_owned(),
                "alt" | "option" | "opt" => {
                    if mac {
                        "⌥".to_owned()
                    } else {
                        "Alt".to_owned()
                    }
                }
                "shift" => {
                    if mac {
                        "⇧".to_owned()
                    } else {
                        "Shift".to_owned()
                    }
                }
                "platform" | "super" | "cmd" | "command" | "meta" | "win" => {
                    if mac {
                        "⌘".to_owned()
                    } else {
                        "Super".to_owned()
                    }
                }
                key => key.to_ascii_uppercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(if mac { "" } else { "+" })
}

/// The label a platform role shows as in the menu (the OS would name it; we name it plainly).
fn role_label(role: &str) -> &'static str {
    match role {
        "undo" => "Undo",
        "redo" => "Redo",
        "cut" => "Cut",
        "copy" => "Copy",
        "paste" => "Paste",
        "selectAll" => "Select All",
        "hideOthers" => "Hide Others",
        "showAll" => "Show All",
        "minimize" => "Minimize",
        "zoom" => "Zoom",
        "toggleFullScreen" => "Enter Full Screen",
        _ => "",
    }
}

/// The accelerator a role answers to (display only: CUI text fields already answer the edit
/// roles themselves; the menu documents them the way a native menu would).
fn role_shortcut(role: &str) -> Option<&'static str> {
    match role {
        "undo" => Some("mod+z"),
        "redo" => Some("mod+shift+z"),
        "cut" => Some("mod+x"),
        "copy" => Some("mod+c"),
        "paste" => Some("mod+v"),
        "selectAll" => Some("mod+a"),
        "hideOthers" => Some("mod+alt+h"),
        "minimize" => Some("mod+m"),
        _ => None,
    }
}

/// The quit item's accelerator as stored (Settings › Computer › Quit shortcut), default mod+q.
fn quit_shortcut_label(os: Os) -> String {
    let combo = crate::state::setting("quit_shortcut").unwrap_or_else(|| "mod+q".to_owned());
    menu_shortcut_label(&combo, os)
}

/// Draws one menu's entries; returns the action picked this frame, if any.
fn draw_menu_entries(ui: &mut Ui, entries: &[MenuEntry], os: Os, picked: &mut Option<MenuAction>) {
    for entry in entries {
        match entry {
            MenuEntry::Separator => ui.menu_separator(),
            MenuEntry::Submenu { .. } => {}
            MenuEntry::Item {
                label,
                role,
                shortcut,
                disabled,
                action,
            } => {
                let name = label
                    .clone()
                    .or_else(|| role.as_deref().map(|r| role_label(r).to_owned()))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                // The quit item's accelerator follows the stored (remappable) shortcut.
                let shown = if *action == Some(MenuAction::Quit) {
                    Some(quit_shortcut_label(os))
                } else {
                    shortcut
                        .as_deref()
                        .or_else(|| role.as_deref().and_then(role_shortcut))
                        .map(|s| menu_shortcut_label(s, os))
                };
                let item = cui::MenuItem {
                    shortcut: shown.as_deref(),
                    disabled: *disabled,
                    ..Default::default()
                };
                if ui.menu_item_with(&name, &item) {
                    if let Some(a) = action {
                        *picked = Some(*a);
                    }
                }
            }
        }
    }
}

/// The in-window application menu bar (CUI has no system menu bar): the app menu, File, Edit,
/// View, Window, Help from `menu_model`, plus Profiles. Every item dispatches through
/// `desktop_host::perform`, the same path a native menu would take.
fn draw_menubar(ui: &mut Ui, app: &mut AppState) {
    let os = desktop_host::desktop_os().unwrap_or(Os::Linux);
    let unlocked = app.chrome_visible();
    let taxpayers = app.taxpayers();
    let state = menu_model::MenuState {
        os,
        unlocked,
        has_taxpayer: !taxpayers.is_empty(),
        form_open: app.view() == ViewName::Form,
    };
    let menus = menu_model::build_menubar(&state);
    let p = crate::ui::kit::frame_palette();
    let mut picked: Option<MenuAction> = None;
    let mut picked_taxpayer: Option<String> = None;
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::px(30.0),
            gap: 2.0,
            pad_x: 8.0,
            align: cui::Align::Center,
            bg: p.background,
            border: 1.0,
            border_color: p.border,
            ..Default::default()
        },
        |ui| {
            for entry in &menus {
                let MenuEntry::Submenu { label, entries } = entry else {
                    continue;
                };
                let entries = entries.clone();
                let label = label.clone();
                ui.menu_with(
                    &label,
                    &cui::Menu {
                        variant: ButtonVariant::Ghost,
                        ..Default::default()
                    },
                    |ui| draw_menu_entries(ui, &entries, os, &mut picked),
                );
            }
            // Profiles: the taxpayers as a switcher, then New Tax Profile.
            if unlocked {
                let tps = taxpayers.clone();
                ui.menu_with(
                    "Profiles",
                    &cui::Menu {
                        variant: ButtonVariant::Ghost,
                        ..Default::default()
                    },
                    |ui| {
                        if tps.is_empty() {
                            ui.menu_item_with(
                                "No taxpayers yet",
                                &cui::MenuItem {
                                    disabled: true,
                                    ..Default::default()
                                },
                            );
                        }
                        for tp in &tps {
                            let name = if tp.tin_display.is_empty() {
                                tp.name.clone()
                            } else {
                                format!("{} — {}", tp.name, tp.tin_display)
                            };
                            let root = tp.tin_root.clone();
                            let current = app.taxpayer().is_some_and(|t| t.tin_root == tp.tin_root);
                            if ui.menu_item_with(
                                &name,
                                &cui::MenuItem {
                                    checked: current,
                                    ..Default::default()
                                },
                            ) {
                                picked_taxpayer = Some(root);
                            }
                        }
                        if !tps.is_empty() {
                            ui.menu_separator();
                        }
                        if ui.menu_item_with(
                            "New Tax Profile…",
                            &cui::MenuItem {
                                shortcut: Some(if os == Os::Macos { "⌘N" } else { "Ctrl+N" }),
                                ..Default::default()
                            },
                        ) {
                            picked = Some(MenuAction::NewTaxProfile);
                        }
                    },
                );
            }
        },
    );
    if let Some(root) = picked_taxpayer {
        app.open_taxpayer(&root);
    }
    if let Some(action) = picked {
        desktop_host::perform(app, action);
    }
}

/// The NSH title bar (shell/TitleBar.tsx + Narrow.tsx NarrowBar): 40pt, left controls, the
/// centred context pill that opens the tax-profile panel, right controls. Under 600pt the pill
/// compacts to avatar + branch code and back/forward hide; under 600 the narrow bar takes over.
fn draw_caption(ui: &mut Ui, app: &mut AppState) {
    if app.narrow() {
        draw_narrow_bar(ui, app);
        return;
    }
    // BREAK.compactPill: under 600 the pill shows avatar + branch code only.
    let compact = app.window_width < crate::state::BREAK_PHONE;
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::px(crate::theme::size::TITLE_BAR),
            gap: 4.0,
            pad_x: 10.0,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            // Left group (flex-1 basis-0): sidebar toggle, back/forward, then search and "+"
            // while the sidebar is hidden (or floats).
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::FILL,
                    dir: cui::Dir::Row,
                    align: cui::Align::Center,
                    gap: 4.0,
                    ..Default::default()
                },
                |ui| {
                    if ui.icon_button_with(
                        crate::ui::icons::glyph("panel-left"),
                        &cui::Button {
                            tooltip: Some(if app.sidebar_open {
                                "Hide sidebar"
                            } else {
                                "Show sidebar"
                            }),
                            ..Default::default()
                        },
                    ) {
                        app.toggle_sidebar();
                    }
                    if !compact {
                        if ui.icon_button_with(
                            crate::ui::icons::glyph("arrow-left"),
                            &cui::Button {
                                tooltip: Some("Back"),
                                disabled: !app.can_back(),
                                ..Default::default()
                            },
                        ) {
                            app.back();
                        }
                        if ui.icon_button_with(
                            crate::ui::icons::glyph("arrow-right"),
                            &cui::Button {
                                tooltip: Some("Forward"),
                                disabled: !app.can_forward(),
                                ..Default::default()
                            },
                        ) {
                            app.forward_one();
                        }
                    }
                    if !app.sidebar_open || app.overlay_mode() {
                        if ui.icon_button_with(
                            crate::ui::icons::glyph("search"),
                            &cui::Button {
                                tooltip: Some("Search"),
                                ..Default::default()
                            },
                        ) {
                            app.open_palette();
                        }
                        if ui.icon_button_with(
                            crate::ui::icons::glyph("plus"),
                            &cui::Button {
                                tooltip: Some("New return"),
                                ..Default::default()
                            },
                        ) {
                            app.start_new_return(None);
                        }
                    }
                },
            );
            // Under 860pt the sidebar/panel float as dialogs over the page: the pill and the
            // narrow avatar are custom hit tests, so they must not see the sheet's clicks.
            let modal = app.overlay_mode() && (app.sidebar_open || app.panel.is_some());
            // Centre: the context pill.
            if context_pill(ui, app, compact) && !modal {
                app.toggle_panel(PanelName::Profile);
            }
            // Right group (flex-1 basis-0, end): notifications, theme, panel.
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::FILL,
                    dir: cui::Dir::Row,
                    align: cui::Align::Center,
                    gap: 4.0,
                    ..Default::default()
                },
                |ui| {
                    ui.spacer();
                    if ui.icon_button_with(
                        crate::ui::icons::glyph("bell"),
                        &cui::Button {
                            tooltip: Some("Notifications"),
                            selected: app.panel == Some(PanelName::Notifs),
                            ..Default::default()
                        },
                    ) {
                        app.toggle_panel(PanelName::Notifs);
                    }
                    // The unread dot rides beside the bell (CUI has no overlay layer).
                    if app.unread_count() > 0 {
                        ui.box_(
                            &cui::BoxOpts {
                                width: cui::Size::px(6.0),
                                height: cui::Size::px(6.0),
                                bg: crate::ui::kit::frame_palette().accent_blue,
                                radius: crate::theme::radius::FULL,
                                ..Default::default()
                            },
                            |_| {},
                        );
                    }
                    let theme_icon = match app.theme_pref {
                        crate::theme::ThemePreference::Light => "sun",
                        crate::theme::ThemePreference::Dark => "moon",
                        crate::theme::ThemePreference::System => "monitor",
                    };
                    if ui.icon_button_with(
                        crate::ui::icons::glyph(theme_icon),
                        &cui::Button {
                            tooltip: Some("Theme: press to change"),
                            ..Default::default()
                        },
                    ) {
                        app.cycle_theme();
                    }
                    if ui.icon_button_with(
                        crate::ui::icons::glyph("panel-right"),
                        &cui::Button {
                            tooltip: Some(if app.panel.is_some() {
                                "Hide tax profile"
                            } else {
                                "Show tax profile"
                            }),
                            selected: app.panel.is_some(),
                            ..Default::default()
                        },
                    ) {
                        app.toggle_right_panel();
                    }
                },
            );
        },
    );
}

/// The title bar's context pill (ui/status.tsx ContextPill): 30pt, full radius, avatar 20,
/// name 13/600, TIN 12, a chevron toward the panel. Empty offers "Add a taxpayer". Press
/// toggles the profile panel. Returns true on press.
fn context_pill(ui: &mut Ui, app: &AppState, compact: bool) -> bool {
    let tp = app.taxpayer();
    let br = app.branch();
    let open = app.panel == Some(PanelName::Profile);
    let p = crate::ui::kit::frame_palette();
    let id = "context-pill";
    let mouse = ui.mouse_pos();
    let hovered = crate::ui::kit::press_hovered(id, mouse.x, mouse.y);
    let wid = ui.make_id(id);
    ui.box_(
        &cui::BoxOpts {
            id: Some(id),
            dir: cui::Dir::Row,
            align: cui::Align::Center,
            gap: 8.0,
            pad_x: if tp.is_some() { 4.0 } else { 10.0 },
            min_height: 30.0,
            max_width: if compact { 160.0 } else { 460.0 },
            bg: if open {
                p.selected
            } else if hovered {
                p.hover
            } else {
                p.card
            },
            border: 1.0,
            border_color: if open { p.subtle_foreground } else { p.border },
            radius: crate::theme::radius::FULL,
            ..Default::default()
        },
        |ui| {
            match &tp {
                None => {
                    ui.icon(crate::ui::icons::glyph("plus"), 14.0, p.subtle_foreground);
                    ui.text_with(
                        "Add a taxpayer",
                        &cui::Text {
                            size: crate::theme::r#type::CONTROL.0,
                            font: cui::font::MEDIUM,
                            ..Default::default()
                        },
                    );
                }
                Some(tp) => {
                    ui.avatar_with(
                        &tp.name,
                        &cui::Avatar {
                            size: 20.0,
                            ..Default::default()
                        },
                    );
                    if compact {
                        if let Some(br) = &br {
                            ui.text_with(
                                &br.code,
                                &cui::Text {
                                    size: crate::theme::r#type::META.0,
                                    color: p.muted_foreground,
                                    tabular: true,
                                    ..Default::default()
                                },
                            );
                        }
                    } else {
                        // The name takes the pill's spare width and ellipsizes.
                        ui.box_(
                            &cui::BoxOpts {
                                width: cui::Size::FILL,
                                max_width: 200.0,
                                clip: true,
                                ..Default::default()
                            },
                            |ui| {
                                ui.text_with(
                                    &tp.name,
                                    &cui::Text {
                                        size: crate::theme::r#type::CONTROL.0,
                                        font: cui::font::MEDIUM,
                                        nowrap: true,
                                        ..Default::default()
                                    },
                                );
                            },
                        );
                        let tin = br
                            .as_ref()
                            .map(|b| b.tin_display.as_str())
                            .unwrap_or(tp.tin_display.as_str());
                        ui.text_with(
                            tin,
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: p.muted_foreground,
                                tabular: true,
                                nowrap: true,
                                ..Default::default()
                            },
                        );
                    }
                    ui.icon(
                        crate::ui::icons::glyph(if open {
                            "chevron-left"
                        } else {
                            "chevron-right"
                        }),
                        14.0,
                        p.subtle_foreground,
                    );
                }
            }
        },
    );
    let rect = ui.get_rect(wid);
    crate::ui::kit::press_remember(id, rect);
    ui.mouse_clicked(cui::MouseButton::Left) && crate::ui::kit::press_hit(rect, mouse.x, mouse.y)
}

/// The narrow bar (Narrow.tsx NarrowBar): back when it applies, the page title, search,
/// notifications and the avatar that opens the profile panel.
fn draw_narrow_bar(ui: &mut Ui, app: &mut AppState) {
    let tp = app.taxpayer();
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::px(crate::theme::size::TITLE_BAR),
            gap: 4.0,
            pad_x: 8.0,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            let on_tab = state::TABS.iter().any(|(v, _, _)| *v == app.view());
            if app.can_back()
                && !on_tab
                && ui.icon_button_with(
                    crate::ui::icons::glyph("arrow-left"),
                    &cui::Button {
                        tooltip: Some("Back"),
                        ..Default::default()
                    },
                )
            {
                app.back();
            }
            ui.text_with(
                app.view().meta().label,
                &cui::Text {
                    size: crate::theme::r#type::EMPH.0,
                    font: cui::font::MEDIUM,
                    nowrap: true,
                    ..Default::default()
                },
            );
            ui.spacer();
            if ui.icon_button_with(
                crate::ui::icons::glyph("search"),
                &cui::Button {
                    tooltip: Some("Search"),
                    ..Default::default()
                },
            ) {
                app.open_palette();
            }
            if ui.icon_button_with(
                crate::ui::icons::glyph("bell"),
                &cui::Button {
                    tooltip: Some("Notifications"),
                    selected: app.panel == Some(PanelName::Notifs),
                    ..Default::default()
                },
            ) {
                app.toggle_panel(PanelName::Notifs);
            }
            if app.unread_count() > 0 {
                ui.box_(
                    &cui::BoxOpts {
                        width: cui::Size::px(6.0),
                        height: cui::Size::px(6.0),
                        bg: crate::ui::kit::frame_palette().accent_blue,
                        radius: crate::theme::radius::FULL,
                        ..Default::default()
                    },
                    |_| {},
                );
            }
            // The avatar button toggles the profile panel (or offers the account when empty).
            // Same modal guard as the wide pill: the floating sheet must not leak clicks.
            let modal = app.overlay_mode() && app.panel.is_some();
            let p = crate::ui::kit::frame_palette();
            let id = "nb-avatar";
            let mouse = ui.mouse_pos();
            let hovered = crate::ui::kit::press_hovered(id, mouse.x, mouse.y);
            let open = app.panel == Some(PanelName::Profile);
            let wid = ui.make_id(id);
            ui.box_(
                &cui::BoxOpts {
                    id: Some(id),
                    width: cui::Size::px(32.0),
                    height: cui::Size::px(32.0),
                    align: cui::Align::Center,
                    bg: if open {
                        p.selected
                    } else if hovered {
                        p.hover
                    } else {
                        p.background
                    },
                    radius: crate::theme::radius::FULL,
                    ..Default::default()
                },
                |ui| match &tp {
                    Some(tp) => {
                        ui.avatar_with(
                            &tp.name,
                            &cui::Avatar {
                                size: 28.0,
                                ..Default::default()
                            },
                        );
                    }
                    None => {
                        ui.icon(crate::ui::icons::glyph("user"), 16.0, p.muted_foreground);
                    }
                },
            );
            let rect = ui.get_rect(wid);
            crate::ui::kit::press_remember(id, rect);
            if ui.mouse_clicked(cui::MouseButton::Left)
                && crate::ui::kit::press_hit(rect, mouse.x, mouse.y)
                && !modal
            {
                app.toggle_panel(PanelName::Profile);
            }
        },
    );
}

/// The routed screen in the main surface: the frame's own scroll for page/wide/full, the view's
/// own for `self`.
fn draw_main(ui: &mut Ui, app: &mut AppState, auth: &auth::SharedAuth) {
    let layout = app.view().meta().layout;
    if layout == state::ViewLayout::Self_ {
        screens::draw(ui, app, auth);
        return;
    }
    ui.scroll(
        &cui::Scroll {
            id: Some("page-scroll"),
            ..Default::default()
        },
        |ui| {
            let main_w = app.main_width();
            let width = state::content_column(layout, main_w);
            // The surface padding is part of the cap (routes.ts contentBox): 32 a side and 28
            // on top (16/20 under 640), 56 below; the column is centred at its max width.
            let pad = if main_w < state::BREAK_NARROW_CONTENT {
                16.0
            } else {
                32.0
            };
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    align: cui::Align::Center,
                    pad_x: pad,
                    pad_top: if pad == 16.0 { 20.0 } else { 28.0 },
                    pad_bottom: 56.0,
                    ..Default::default()
                },
                |ui| {
                    ui.column(
                        &cui::Column {
                            width: cui::Size::px(width),
                            gap: crate::theme::space(4),
                            ..Default::default()
                        },
                        |ui| screens::draw(ui, app, auth),
                    );
                },
            );
        },
    );
}

/// Native CUI toasts (bottom-right overlay, auto-dismiss, action + timer) bridged to the app's
/// toast list: each `AppState` toast fires one `ui.toast_with`, its action press runs the
/// registered callback, and a toast the app drops is dismissed natively.
mod toast_bridge {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use cui::{Toast as CuiToast, ToastHandle, Tone, Ui};

    use crate::state::{AppState, Toast};

    thread_local! {
        /// app toast id -> the native toast's handle.
        static HANDLES: RefCell<HashMap<u64, cui::ToastHandle>> = RefCell::new(HashMap::new());
    }

    /// The toast's tone from its leading icon (NSH toasts are ink-coloured; we only tint the
    /// warning/danger ones so they read at a glance).
    fn tone_of(t: &Toast) -> Tone {
        match t.icon.as_deref() {
            Some("triangle-alert") | Some("circle-alert") => Tone::Warning,
            Some("trash") => Tone::Danger,
            _ => Tone::Neutral,
        }
    }

    pub fn sync(ui: &mut Ui, app: &mut AppState) {
        // Fire a native toast for every app toast not yet shown.
        HANDLES.with(|m| {
            let mut map = m.borrow_mut();
            for t in &app.toasts {
                if map.contains_key(&t.id) {
                    continue;
                }
                let handle = ui.toast_with(
                    &t.message,
                    &CuiToast {
                        tone: tone_of(t),
                        // NSH: 4 s plain, 5 s with an action (the draining bar).
                        duration: if t.action.is_some() { 5.0 } else { 4.0 },
                        action: t.action.as_ref().map(|a| a.label.as_str()),
                        show_timer: t.action.is_some(),
                        ..Default::default()
                    },
                );
                map.insert(t.id, handle);
            }
        });
        // App toasts that are gone take their native toast with them.
        HANDLES.with(|m| {
            let mut map = m.borrow_mut();
            let live: Vec<u64> = app.toasts.iter().map(|t| t.id).collect();
            let gone: Vec<(u64, ToastHandle)> = map
                .iter()
                .filter(|(id, _)| !live.contains(id))
                .map(|(id, h)| (*id, *h))
                .collect();
            for (_, h) in gone {
                ui.toast_dismiss(h);
            }
            map.retain(|id, _| live.contains(id));
        });
        // Action presses: run the app's callback and close both sides.
        let mut fired: Vec<u64> = Vec::new();
        HANDLES.with(|m| {
            let map = m.borrow();
            for (id, h) in map.iter() {
                if ui.toast_action_clicked(*h) {
                    fired.push(*id);
                }
            }
        });
        for id in fired {
            let action = app
                .toasts
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.action.clone());
            if let Some(a) = action {
                app.run_toast_action(a.index);
            }
            app.dismiss_toast(id);
        }
    }
}

/// Toasts along the bottom, newest last; a toast's action button resolves through toast_actions.
fn draw_toasts(ui: &mut Ui, app: &mut AppState) {
    toast_bridge::sync(ui, app);
}

/// One FactRow of the profile summary (shell/parts.tsx): an 84pt icon+label column, the value.
fn fact_row(ui: &mut Ui, icon: &str, label: &str, f: impl FnOnce(&mut Ui)) {
    let p = crate::ui::kit::frame_palette();
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: 12.0,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::px(84.0),
                    dir: cui::Dir::Row,
                    align: cui::Align::Center,
                    gap: 6.0,
                    ..Default::default()
                },
                |ui| {
                    ui.icon(crate::ui::icons::glyph(icon), 14.0, p.subtle_foreground);
                    ui.text_with(
                        label,
                        &cui::Text {
                            size: crate::theme::r#type::META.0,
                            color: p.subtle_foreground,
                            nowrap: true,
                            ..Default::default()
                        },
                    );
                },
            );
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::FILL,
                    ..Default::default()
                },
                f,
            );
        },
    );
}

/// A panel nav row (ui/buttons.tsx NavRow, default size): leading icon or element, the label,
/// an optional count, the selected label colour on the current destination, a trailing check
/// when `check` (the branch rows).
fn panel_row(
    ui: &mut Ui,
    id: &str,
    label: &str,
    icon: Option<&str>,
    count: Option<String>,
    current: bool,
    check: bool,
    leading: Option<&dyn Fn(&mut Ui)>,
) -> bool {
    let p = crate::ui::kit::frame_palette();
    kit::pressable_row(ui, id, |ui| {
        if let Some(f) = leading {
            f(ui);
        } else if let Some(ic) = icon {
            ui.icon(crate::ui::icons::glyph(ic), 16.0, p.muted_foreground);
        }
        ui.text_with(
            label,
            &cui::Text {
                size: crate::theme::r#type::CONTROL.0,
                color: if current {
                    p.foreground
                } else {
                    p.muted_foreground
                },
                nowrap: true,
                ..Default::default()
            },
        );
        ui.spacer();
        if let Some(n) = count {
            ui.text_with(
                &n,
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: p.subtle_foreground,
                    tabular: true,
                    ..Default::default()
                },
            );
        }
        if check && current {
            ui.icon(crate::ui::icons::glyph("check"), 14.0, p.subtle_foreground);
        }
    })
}

/// The empty profile panel (Panels.tsx EmptyProfile).
fn empty_profile(ui: &mut Ui, app: &mut AppState) {
    let p = crate::ui::kit::frame_palette();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            align: cui::Align::Center,
            gap: 8.0,
            pad_x: 8.0,
            pad_top: 32.0,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::px(56.0),
                    height: cui::Size::px(56.0),
                    align: cui::Align::Center,
                    bg: p.hover,
                    radius: crate::theme::radius::FULL,
                    ..Default::default()
                },
                |ui| {
                    ui.icon(crate::ui::icons::glyph("user"), 24.0, p.muted_foreground);
                },
            );
            ui.text_with(
                if app.taxpayers().is_empty() {
                    "No taxpayer yet"
                } else {
                    "No taxpayer open"
                },
                &cui::Text {
                    size: crate::theme::r#type::TITLE.0,
                    font: cui::font::MEDIUM,
                    ..Default::default()
                },
            );
            let lede = if app.taxpayers().is_empty() {
                "The taxpayer you file for shows here, with its branches, tax years, forms and templates."
            } else {
                "Choose one on Home or with search. The one you open shows here, with its branches, tax years, forms and templates."
            };
            ui.text_with(
                lede,
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: p.muted_foreground,
                    align: cui::TextAlign::Center,
                    ..Default::default()
                },
            );
            if app.can_add_taxpayer()
                && kit::primary_button(ui, "New taxpayer", true)
            {
                app.go(ViewName::NewTaxpayer, Default::default());
            }
        },
    );
}

/// The tax-profile summary (Panels.tsx ProfileSummary): avatar, name, TIN, the facts box and
/// "Edit in profile".
fn profile_summary(ui: &mut Ui, app: &mut AppState, tp: &crate::types::Taxpayer) {
    let p = crate::ui::kit::frame_palette();
    let br = app.branch();
    let year = app.year();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            align: cui::Align::Center,
            gap: 8.0,
            pad_x: 8.0,
            pad_top: 12.0,
            pad_bottom: 16.0,
            ..Default::default()
        },
        |ui| {
            ui.avatar_with(
                &tp.name,
                &cui::Avatar {
                    size: 56.0,
                    ..Default::default()
                },
            );
            ui.gap(4.0);
            ui.text_with(
                &tp.name,
                &cui::Text {
                    size: crate::theme::r#type::TITLE.0,
                    font: cui::font::MEDIUM,
                    nowrap: true,
                    ..Default::default()
                },
            );
            ui.text_with(
                br.as_ref()
                    .map(|b| b.tin_display.as_str())
                    .unwrap_or(tp.tin_display.as_str()),
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: p.muted_foreground,
                    tabular: true,
                    nowrap: true,
                    ..Default::default()
                },
            );
        },
    );
    // The facts box (border-y in NSH; separators read the same here).
    ui.separator();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 8.0,
            pad_x: 8.0,
            pad_y: 12.0,
            ..Default::default()
        },
        |ui| {
            fact_row(ui, "git-branch", "Branch", |ui| match &br {
                Some(b) => {
                    ui.text_with(
                        &format!("{} · {}", b.code, b.name),
                        &cui::Text {
                            size: crate::theme::r#type::BODY.0,
                            nowrap: true,
                            ..Default::default()
                        },
                    );
                }
                None => ui.muted("Not set"),
            });
            fact_row(ui, "map-pin", "Address", |ui| {
                let addr = br
                    .as_ref()
                    .and_then(|b| b.facts.registered_address.clone())
                    .filter(|s| !s.is_empty());
                match addr {
                    Some(a) => {
                        ui.text_with(
                            &a,
                            &cui::Text {
                                size: crate::theme::r#type::BODY.0,
                                ..Default::default()
                            },
                        );
                    }
                    None => ui.muted("Not set"),
                }
            });
            fact_row(ui, "building", "RDO", |ui| {
                let rdo = br
                    .as_ref()
                    .and_then(|b| b.facts.rdo_code.clone())
                    .filter(|s| !s.is_empty());
                match rdo {
                    Some(r) => {
                        ui.text_with(
                            &r,
                            &cui::Text {
                                size: crate::theme::r#type::BODY.0,
                                tabular: true,
                                ..Default::default()
                            },
                        );
                    }
                    None => ui.muted("Not set"),
                }
            });
            if let Some(y) = year {
                let chips = br
                    .as_ref()
                    .map(|b| crate::store::tax_types(Some(&b.tin), Some(y)))
                    .unwrap_or_default();
                fact_row(ui, "calendar", &y.to_string(), move |ui| {
                    if chips.is_empty() {
                        ui.muted("No forms yet");
                    } else {
                        ui.row(
                            &cui::Row {
                                width: cui::Size::FILL,
                                gap: 4.0,
                                wrap: true,
                                ..Default::default()
                            },
                            |ui| {
                                for c in &chips {
                                    kit::tone_badge(ui, &c.label, "neutral");
                                }
                            },
                        );
                    }
                });
                // Facts the year's forms read that are not set (Panels.tsx `missing`).
                let missing = br.as_ref().map(|b| {
                    let view = crate::lib::facts::not_set_view(&state::year_facts(&b.tin, y));
                    let ready = crate::lib::facts::readiness(&state::election_readiness(&b.tin, y));
                    view.map(|v| {
                        v.not_set
                            .iter()
                            .copied()
                            .filter(|f| {
                                !(*f == crate::lib::facts::FactName::IncomeTaxElection
                                    && ready.as_ref().and_then(|r| r.gap.as_deref())
                                        != Some("ready"))
                            })
                            .count()
                    })
                    .unwrap_or(0)
                });
                if let Some(n) = missing {
                    if n > 0 {
                        fact_row(ui, "triangle-alert", "Facts", move |ui| {
                            kit::tone_badge(ui, &format!("{n} not set"), "warning");
                        });
                    }
                }
            }
        },
    );
    ui.separator();
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            pad_x: 8.0,
            pad_top: 8.0,
            ..Default::default()
        },
        |ui| {
            if kit::small_button(
                ui,
                "Edit in profile",
                ButtonVariant::Secondary,
                Some("pencil"),
            ) {
                app.open_profile(None);
            }
        },
    );
}

/// The panel's lists (Panels.tsx ProfileLists): Open, Branch(es), Switch taxpayer.
fn profile_lists(ui: &mut Ui, app: &mut AppState, tp: &crate::types::Taxpayer) {
    let p = crate::ui::kit::frame_palette();
    let mut go: Option<ViewName> = None;
    let mut branch_pick: Option<String> = None;
    let mut tp_pick: Option<String> = None;
    ui.gap(4.0);
    // GroupLabel: a small caps muted heading with side padding.
    let label = |ui: &mut Ui, text: &str| {
        ui.text_with(
            text,
            &cui::Text {
                size: crate::theme::r#type::CAP.0,
                color: p.subtle_foreground,
                ..Default::default()
            },
        );
    };
    label(ui, "Open");
    for (v, text, icon) in [
        (ViewName::Profile, "Profile", "user"),
        (ViewName::Forms, "Forms", "file-text"),
        (ViewName::Templates, "Templates", "layout-template"),
    ] {
        let count = match v {
            ViewName::Forms => Some(app.year_forms().len().to_string()),
            ViewName::Templates => Some(app.templates().len().to_string()),
            _ => None,
        };
        if panel_row(
            ui,
            &format!("panel-open-{text}"),
            text,
            Some(icon),
            count,
            app.view() == v,
            false,
            None,
        ) {
            go = Some(v);
        }
    }
    label(
        ui,
        if tp.branches.len() > 1 {
            "Branches"
        } else {
            "Branch"
        },
    );
    for b in &tp.branches {
        let current = app.branch().is_some_and(|x| x.code == b.code);
        let id = format!("panel-branch-{}", b.code);
        let name = b.name.clone();
        let code = b.code.clone();
        // Leading element: the branch code in a fixed 44pt column (NSH Code w-11).
        let leading = move |ui: &mut Ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::px(44.0),
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        &code,
                        &cui::Text {
                            size: crate::theme::r#type::META.0,
                            color: p.muted_foreground,
                            tabular: true,
                            ..Default::default()
                        },
                    );
                },
            );
        };
        if panel_row(ui, &id, &name, None, None, current, true, Some(&leading)) {
            branch_pick = Some(b.code.clone());
        }
    }
    // "Add branch" row.
    if panel_row(
        ui,
        "panel-add-branch",
        "Add branch",
        Some("plus"),
        None,
        false,
        false,
        None,
    ) {
        go = Some(ViewName::NewBranch);
    }
    // Switch taxpayer (admin only, when others exist).
    if app.is_admin() {
        let others: Vec<crate::types::Taxpayer> = app
            .taxpayers()
            .into_iter()
            .filter(|x| x.tin_root != tp.tin_root)
            .collect();
        if !others.is_empty() {
            label(ui, "Switch taxpayer");
            for x in &others {
                let id = format!("panel-tp-{}", x.tin_root);
                let name = x.name.clone();
                let root = x.tin_root.clone();
                // NSH's count is the TIN root's first three digits.
                let prefix: String = x.tin_root.chars().take(3).collect();
                let leading = move |ui: &mut Ui| {
                    ui.avatar_with(
                        &name,
                        &cui::Avatar {
                            size: 20.0,
                            ..Default::default()
                        },
                    );
                };
                if panel_row(
                    ui,
                    &id,
                    &x.name.clone(),
                    None,
                    Some(prefix),
                    false,
                    false,
                    Some(&leading),
                ) {
                    tp_pick = Some(root.clone());
                }
            }
            if app.can_add_taxpayer()
                && panel_row(
                    ui,
                    "panel-new-tp",
                    "New taxpayer",
                    Some("plus"),
                    None,
                    false,
                    false,
                    None,
                )
            {
                go = Some(ViewName::NewTaxpayer);
            }
        }
    }
    if let Some(v) = go {
        app.go(v, Default::default());
    }
    if let Some(code) = branch_pick {
        app.select_branch(&code);
    }
    if let Some(root) = tp_pick {
        app.select_taxpayer(&root, false, None);
    }
}

/// The tax-profile panel body (Panels.tsx ProfileBody).
fn profile_panel(ui: &mut Ui, app: &mut AppState) {
    match app.taxpayer() {
        None => empty_profile(ui, app),
        Some(tp) => {
            profile_summary(ui, app, &tp);
            profile_lists(ui, app, &tp);
        }
    }
}

/// One notification row (Panels.tsx NotificationRow): tinted icon circle, title and sub,
/// the date and an unread dot.
fn notification_row(ui: &mut Ui, n: &crate::state::Notification) -> bool {
    let p = crate::ui::kit::frame_palette();
    let ok = n.tone == "success";
    kit::pressable_row(ui, &format!("notif-{}", n.id), |ui| {
        ui.box_(
            &cui::BoxOpts {
                width: cui::Size::px(28.0),
                height: cui::Size::px(28.0),
                align: cui::Align::Center,
                bg: if ok { p.success_soft } else { p.hover },
                radius: crate::theme::radius::FULL,
                ..Default::default()
            },
            |ui| {
                ui.icon(
                    crate::ui::icons::glyph(&n.icon),
                    14.0,
                    if ok { p.success } else { p.muted_foreground },
                );
            },
        );
        ui.column(
            &cui::Column {
                width: cui::Size::FILL,
                ..Default::default()
            },
            |ui| {
                ui.text(&n.title);
                ui.text_with(
                    &n.sub,
                    &cui::Text {
                        size: crate::theme::r#type::META.0,
                        color: p.subtle_foreground,
                        ..Default::default()
                    },
                );
            },
        );
        ui.column(
            &cui::Column {
                align: cui::Align::End,
                gap: 6.0,
                ..Default::default()
            },
            |ui| {
                ui.text_with(
                    &n.when,
                    &cui::Text {
                        size: crate::theme::r#type::CAP.0,
                        color: p.subtle_foreground,
                        ..Default::default()
                    },
                );
                if n.unread {
                    ui.box_(
                        &cui::BoxOpts {
                            width: cui::Size::px(6.0),
                            height: cui::Size::px(6.0),
                            bg: p.accent_blue,
                            radius: crate::theme::radius::FULL,
                            ..Default::default()
                        },
                        |_| {},
                    );
                }
            },
        );
    })
}

/// The notifications panel (Panels.tsx NotificationsBody + the SidePanel title row).
fn notifications_panel(ui: &mut Ui, app: &mut AppState) {
    // SidePanel's titled header: "Notifications" + Mark all read.
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::px(crate::theme::size::TITLE_BAR),
            gap: 8.0,
            pad_x: 20.0,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                "Notifications",
                &cui::Text {
                    size: crate::theme::r#type::EMPH.0,
                    font: cui::font::MEDIUM,
                    nowrap: true,
                    ..Default::default()
                },
            );
            ui.spacer();
            if ui.ghost_button("Mark all read", app.unread_count() > 0) {
                app.mark_all_read();
            }
        },
    );
    ui.gap(4.0);
    let mut open_id: Option<String> = None;
    let notifs = app.notifications();
    if notifs.is_empty() {
        ui.row(
            &cui::Row {
                width: cui::Size::FILL,
                pad_y: 24.0,
                pad_x: 8.0,
                ..Default::default()
            },
            |ui| {
                ui.muted("No notifications yet.");
            },
        );
    }
    for n in &notifs {
        if notification_row(ui, n) {
            open_id = Some(n.id.clone());
        }
    }
    if let Some(id) = open_id {
        app.open_notification(&id);
    }
}

/// The right panel (Panels.tsx RightPanel): the 320-point column — the tax-profile summary or
/// the notifications.
fn draw_panel(ui: &mut Ui, app: &mut AppState) {
    let Some(panel) = app.panel else {
        return;
    };
    ui.column(
        &cui::Column {
            width: cui::Size::px(crate::state::SIZE_PANEL),
            height: cui::Size::FILL,
            ..Default::default()
        },
        |ui| {
            ui.scroll(
                &cui::Scroll {
                    id: Some("panel-scroll"),
                    pad: 12.0,
                    ..Default::default()
                },
                |ui| match panel {
                    PanelName::Notifs => notifications_panel(ui, app),
                    PanelName::Profile => profile_panel(ui, app),
                },
            );
        },
    );
}

/// The command palette overlay: grouped rows over taxpayers, pages, forms and actions
/// (shell/paletteItems.ts + Palette.tsx).
fn draw_palette(ui: &mut Ui, app: &mut AppState) {
    if app.overlay != Some(state::OverlayName::Palette) {
        return;
    }
    let can_step = screens::form::can_step();
    let items = crate::palette::items(app, can_step);
    // The highlighted row, kept inside the list (paletteRow).
    if app.palette_index >= items.len() && !items.is_empty() {
        app.palette_index = items.len() - 1;
    }
    let mut picked: Option<crate::palette::PaletteAction> = None;
    let mut last_group: Option<crate::palette::PaletteGroup> = None;
    let mut open = true;
    // Palette.tsx: the list scrolls inside a 420pt max-height box, so the dialog keeps one
    // size instead of growing with the matches. Rows are ~34pt, a group heading ~20pt.
    let mut seen_group = None;
    let groups = items
        .iter()
        .filter(|it| {
            let fresh = seen_group != Some(it.group);
            seen_group = Some(it.group);
            fresh
        })
        .count();
    let list_h = (items.len() as f32 * 34.0 + groups as f32 * 20.0 + 24.0).clamp(56.0, 420.0);
    ui.dialog_with(
        "",
        &mut open,
        &cui::Dialog {
            width: 560.0,
            no_close_button: true,
            ..Default::default()
        },
        |ui| {
            let mut q = app.palette_query.clone();
            if q.capacity() < 256 {
                q.reserve(256);
            }
            if ui
                .input_with(
                    "palette",
                    &mut q,
                    &cui::Input {
                        placeholder: Some("Search taxpayers, forms and pages"),
                        icon: Some(crate::ui::icons::glyph("search")),
                        autofocus: true,
                        ..Default::default()
                    },
                )
                .changed()
            {
                app.set_palette_query(&q);
            }
            ui.scroll(
                &cui::Scroll {
                    id: Some("palette-list"),
                    height: cui::Size::px(list_h),
                    pad: 8.0,
                    gap: 2.0,
                    ..Default::default()
                },
                |ui| {
                    if items.is_empty() {
                        ui.muted("No matches. Try a form code like 2550Q.");
                    }
                    for (index, item) in items.iter().enumerate() {
                        if last_group != Some(item.group) {
                            last_group = Some(item.group);
                            ui.muted(&item.group.label().to_uppercase());
                        }
                        let current = index == app.palette_index;
                        let avatar = item.avatar.as_ref().map(|(name, _)| name.as_str());
                        if ui.list_item_with(
                            &item.label,
                            &cui::ListItem {
                                icon: item.icon.map(crate::ui::icons::glyph),
                                trailing: item.hint.as_deref(),
                                avatar,
                                selected: current,
                                ..Default::default()
                            },
                        ) {
                            picked = Some(item.action.clone());
                        }
                    }
                },
            );
        },
    );
    if !open {
        app.close_overlay();
    }
    if let Some(action) = picked {
        crate::palette::run(app, action);
    }
}

/// The start-a-return dialog (shell/NewReturn.tsx): the year's forms, a period, whether a
/// template exists, and the facts the form reads that are not set (named, never blocking).
fn draw_new_return(ui: &mut Ui, app: &mut AppState) {
    if app.overlay != Some(state::OverlayName::New) {
        return;
    }
    // Gather first: the picks borrow `app` mutably below.
    let codes = app.year_forms();
    let index = state::screen_index();
    let tp = app.taxpayer();
    let br = app.branch();
    let year = app.year();
    let code = app
        .new_return_code
        .clone()
        .filter(|c| codes.contains(c))
        .or_else(|| codes.first().cloned())
        .unwrap_or_default();
    let entry = index.iter().find(|f| f.c == code);
    let freq = entry
        .map(|f| crate::lib::periods::Frequency::parse(&f.freq))
        .unwrap_or(crate::lib::periods::Frequency::Annual);
    let now = chrono::Local::now();
    let year_i32 = year.unwrap_or_else(|| i64::from(chrono::Datelike::year(&now))) as i32;
    let period_opts = if code.is_empty() {
        Vec::new()
    } else {
        crate::lib::periods::period_options(&code, freq, year_i32, now)
    };
    let period = app
        .new_return_period
        .filter(|p| period_opts.iter().any(|o| o.value == *p))
        .or_else(|| period_opts.first().map(|o| o.value))
        .unwrap_or(0);
    let has_template = !code.is_empty() && app.templates().iter().any(|t| t.form_code == code);
    // A fact this form reads that is not set is named, never blocks.
    let not_set = match (&br, year) {
        (Some(b), Some(y)) => {
            let view = crate::lib::facts::not_set_view(&state::year_facts(&b.tin, y));
            let ready = crate::lib::facts::readiness(&state::election_readiness(&b.tin, y));
            crate::lib::facts::not_set_sentence(&crate::lib::facts::not_set_for_form(
                view.as_ref(),
                &code,
                ready.as_ref(),
            ))
        }
        _ => None,
    };

    let mut open = true;
    ui.dialog_with(
        "Start a return",
        &mut open,
        &cui::Dialog {
            width: 560.0,
            ..Default::default()
        },
        |ui| {
            {
                // "name ·" TIN-with-branch " branch name", as the old header.
                let who = format!(
                    "{} · {} {}",
                    tp.as_ref().map(|t| t.name.as_str()).unwrap_or(""),
                    br.as_ref().map(|b| b.tin_display.as_str()).unwrap_or(""),
                    br.as_ref().map(|b| b.name.as_str()).unwrap_or("")
                );
                ui.meta_text(&who);
                // The forms this branch files this year, one row each (code, title, frequency).
                ui.scroll(
                    &cui::Scroll {
                        id: Some("new-return-list"),
                        ..Default::default()
                    },
                    |ui| {
                        if codes.is_empty() {
                            ui.muted("This taxpayer files no forms this year.");
                        }
                        for c in &codes {
                            let e = index.iter().find(|f| &f.c == c);
                            let title = e.map(|f| f.t.as_str()).unwrap_or(c.as_str());
                            let freq_label = e.map(|f| f.freq.as_str()).unwrap_or("");
                            let picked = *c == code;
                            let label = if freq_label.is_empty() {
                                format!("{c}  {title}")
                            } else {
                                format!("{c}  {title}  {freq_label}")
                            };
                            if ui.list_item_with(
                                &label,
                                &cui::ListItem {
                                    selected: picked,
                                    ..Default::default()
                                },
                            ) {
                                app.pick_new_return_form(c);
                            }
                        }
                    },
                );
                if !period_opts.is_empty() {
                    ui.meta_text("Period");
                    for o in &period_opts {
                        if ui.list_item_with(
                            &o.label,
                            &cui::ListItem {
                                selected: o.value == period,
                                ..Default::default()
                            },
                        ) {
                            app.pick_new_return_period(o.value);
                        }
                    }
                }
                if has_template {
                    ui.tone_badge("Template", "success");
                } else {
                    ui.tone_badge("No template", "muted");
                }
                if let Some(sentence) = &not_set {
                    ui.warning_text(sentence);
                    if ui.ghost_button("Open the profile", true) {
                        app.close_overlay();
                        app.open_profile(None);
                    }
                }
                ui.row(
                    &cui::Row {
                        gap: crate::theme::space(8),
                        ..Default::default()
                    },
                    |ui| {
                        if ui.ghost_button("Cancel", true) {
                            app.close_overlay();
                        }
                        if ui.primary_button("Open form", !code.is_empty()) {
                            app.confirm_new_return();
                        }
                    },
                );
            }
        },
    );
    if !open {
        app.close_overlay();
    }
}

/// The menu's accelerators, as keys (menu_model.ts's shortcuts; CUI has no native menu to carry
/// them, so the frame hears them here). Filing commands act only while the chrome is up.
/// A stored-combo key word ("q", "f5", "1") as a CUI key.
fn combo_key(word: &str) -> Option<Key> {
    let w = word.trim().to_ascii_lowercase();
    if w.len() == 1 {
        let c = w.chars().next()?;
        const LETTERS: [Key; 26] = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        const DIGITS: [Key; 10] = [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        if c.is_ascii_lowercase() {
            return Some(LETTERS[(c as u8 - b'a') as usize]);
        }
        if c.is_ascii_digit() {
            return Some(DIGITS[(c as u8 - b'0') as usize]);
        }
    }
    match w.as_str() {
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => None,
    }
}

/// Whether the stored quit combo (`quit_shortcut` setting, default mod+q) fired this frame.
fn quit_shortcut_fired(ui: &mut Ui) -> bool {
    let combo = crate::state::setting("quit_shortcut")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "mod+q".to_owned());
    let tokens: Vec<&str> = combo
        .split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return false;
    }
    // The last token is the key; the rest are modifiers.
    let Some(key) = combo_key(tokens[tokens.len() - 1]) else {
        return false;
    };
    if !ui.key_pressed(key) {
        return false;
    }
    let mut want = Mods(0);
    for word in &tokens[..tokens.len() - 1] {
        let w = word.to_ascii_lowercase();
        let m = match w.as_str() {
            "mod" => Mods::PRIMARY,
            "control" | "ctrl" => Mods::CTRL,
            "alt" | "option" | "opt" => Mods::ALT,
            "shift" => Mods::SHIFT,
            "platform" | "super" | "cmd" | "command" | "meta" | "win" => Mods::SUPER,
            _ => Mods(0),
        };
        want = want.union(m);
    }
    // Require exactly the stored modifiers: an extra held modifier means a different chord.
    let held = ui.key_mods().0;
    held & want.0 == want.0 && held & !want.0 == 0
}

fn shortcuts(ui: &mut Ui, app: &mut AppState) {
    // The hotkey capture dialog owns the keyboard: nothing else may fire while it records.
    if screens::settings::hotkey_editing() {
        return;
    }
    let primary = Mods::PRIMARY;
    // Quit: the stored, remappable `quit_shortcut` combo (default mod+q — Cmd+Q quits on macOS).
    if quit_shortcut_fired(ui) {
        desktop_host::perform(app, crate::menu_model::MenuAction::Quit);
    }
    if app.overlay == Some(state::OverlayName::Palette) {
        // The palette's own keys: Up/Down move, Enter runs, Esc closes.
        let can_step = screens::form::can_step();
        let count = crate::palette::items(app, can_step).len();
        if ui.key_pressed(Key::Down) || ui.key_pressed(Key::Up) {
            let delta: isize = if ui.key_pressed(Key::Down) { 1 } else { -1 };
            if count > 0 {
                app.palette_index =
                    ((app.palette_index as isize + delta).rem_euclid(count as isize)) as usize;
            }
        }
        if ui.key_pressed(Key::Enter) {
            let can_step = screens::form::can_step();
            let items = crate::palette::items(app, can_step);
            if let Some(item) = items.get(app.palette_index) {
                let action = item.action.clone();
                crate::palette::run(app, action);
            }
        }
        if ui.key_pressed(Key::Escape) {
            app.close_overlay();
        }
        return;
    }
    if !app.chrome_visible() {
        return;
    }
    if ui.shortcut(primary, Key::K) {
        app.open_palette();
    } else if ui.shortcut(primary, Key::B) {
        desktop_host::perform(app, crate::menu_model::MenuAction::ToggleSidebar);
    } else if ui.shortcut(primary | Mods::SHIFT, Key::B) {
        desktop_host::perform(app, crate::menu_model::MenuAction::ToggleProfile);
    } else if ui.shortcut(primary | Mods::SHIFT, Key::N) {
        desktop_host::perform(app, crate::menu_model::MenuAction::ToggleNotifications);
    } else if ui.shortcut(primary, Key::T) {
        desktop_host::perform(app, crate::menu_model::MenuAction::ToggleTheme);
    } else if ui.shortcut(primary, Key::Comma) {
        desktop_host::perform(app, crate::menu_model::MenuAction::Settings);
    } else if ui.shortcut(primary, Key::N) {
        desktop_host::perform(app, crate::menu_model::MenuAction::NewTaxProfile);
    } else if app.view() == ViewName::Form
        && (ui.shortcut(primary, Key::Right) || ui.shortcut(primary, Key::Left))
    {
        let dir = if ui.key_down(Key::Right) { 1 } else { -1 };
        screens::form::step(dir);
    }
}

/// Draw whatever stands in front of the frame; true when it covered everything.
fn draw_auth_or_shell(ui: &mut Ui, app: &mut AppState, auth: &auth::SharedAuth) {
    if auth::draw(ui, app, auth) {
        return;
    }
    // chrome: menu bar (in-window; CUI has no system menu bar), caption, then sidebar + main + panel
    if app.chrome_visible() {
        draw_menubar(ui, app);
    }
    draw_caption(ui, app);
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: 0.0,
            ..Default::default()
        },
        |ui| {
            ui.row(
                &cui::Row {
                    gap: 0.0,
                    width: cui::Size::FILL,
                    height: cui::Size::FILL,
                    align: cui::Align::Stretch,
                    ..Default::default()
                },
                |ui| {
                    if app.sidebar_docked() {
                        draw_sidebar(ui, app);
                    }
                    // The main surface (Shell.tsx MainSurface): a bg-card rounded card with a
                    // hairline, inset 8 on the free sides — the sidebar and panel are plain
                    // bg-background and get their separation from this card's border.
                    let narrow = app.narrow();
                    ui.column(
                        &cui::Column {
                            width: cui::Size::FILL,
                            height: cui::Size::FILL,
                            pad_left: if narrow || app.sidebar_docked() {
                                0.0
                            } else {
                                8.0
                            },
                            pad_right: if narrow || app.panel_docked() {
                                0.0
                            } else {
                                8.0
                            },
                            pad_bottom: if narrow { 0.0 } else { 8.0 },
                            ..Default::default()
                        },
                        |ui| {
                            let p = crate::ui::kit::frame_palette();
                            ui.box_(
                                &cui::BoxOpts {
                                    width: cui::Size::FILL,
                                    height: cui::Size::FILL,
                                    bg: p.card,
                                    border: 1.0,
                                    border_color: p.border,
                                    radius: crate::theme::radius::CARD,
                                    clip: true,
                                    dir: cui::Dir::Column,
                                    ..Default::default()
                                },
                                |ui| draw_main(ui, app, auth),
                            );
                        },
                    );
                    if app.panel_docked() {
                        draw_panel(ui, app);
                    }
                },
            );
            // The narrow layout's bottom tab bar (state::TABS): Home, Forms, Filings, Calendar.
            if app.narrow() {
                draw_tab_bar(ui, app);
            }
        },
    );
    // Below 860 the sidebar and the panel float over the page (state.rs overlay_mode); CUI has no
    // absolute layer, so each floats as a modal dialog — the scrim is the dialog's own.
    if app.overlay_mode() && !app.narrow() && app.sidebar_open {
        let mut sheet_open = true;
        ui.dialog("eBIRForms", &mut sheet_open, |ui| draw_sidebar(ui, app));
        if !sheet_open {
            app.sidebar_open = false;
        }
    }
    if app.overlay_mode() && app.panel.is_some() {
        let mut sheet_open = true;
        ui.dialog("", &mut sheet_open, |ui| draw_panel(ui, app));
        if !sheet_open {
            app.panel = None;
        }
    }
    draw_toasts(ui, app);
    desktop_host::draw_update_banner(ui, app);
    draw_palette(ui, app);
    draw_new_return(ui, app);
}

/// Runs the CUI frame loop. Returns the process exit code.
pub fn run() -> i32 {
    let desc = cui::AppDesc {
        title: Some("eBIRForms"),
        width: 1440,
        height: 900,
        min_width: 420,
        min_height: 480,
        app_id: Some("dev.goldcoders.bir.cui"),
        follow_system_theme: true,
        ..Default::default()
    };
    let mut app = match cui::App::new(&desc) {
        Some(app) => app,
        None => {
            eprintln!("eBIRForms: could not open a window");
            return 1;
        }
    };
    UI_RUNNING.store(true, Ordering::SeqCst);

    let mut state = AppState::default();
    let auth = auth::shared();
    let start = std::env::var("BIR_SHELL_START").ok();
    state.boot(start);
    // An empty database opens on the seeded demo taxpayer (seed.rs), so first-run setup and an
    // empty Home never appear; a persistent BIR_DATABASE_PATH keeps all later entries. The seed
    // is idempotent: on an existing database it only tops up the demo Forms Set (seed.rs).
    if state.booted && !state.cloud_locked {
        crate::seed::run(&mut state);
    }

    // Session change: every screen's own state back to a first visit's.
    state.on_session_change(std::rc::Rc::new(std::cell::RefCell::new(|| {
        screens::home::reset_for_session();
        screens::library::reset_for_session();
        screens::calendar::reset_for_session();
        screens::filings::reset_for_session();
        screens::settings::reset_for_session();
        screens::form::reset_for_session();
    })));
    // Leaving a view: the form page flushes and saves what is typed.
    state.before_leave(std::rc::Rc::new(std::cell::RefCell::new(|| {
        screens::form::flush_on_leave();
    })));

    // The desktop host (Desktop.tsx): the updater driver, polled host commands, the About dialog.
    let mut last_view = state.view();
    while app.frame() {
        let size = app.size();
        state.window_width = size.x;
        state.tick();
        // Leaving Settings forgets its half-typed codes; a finished setup change signs out.
        if last_view == ViewName::Settings && state.view() != ViewName::Settings {
            screens::settings::leave(&mut state);
        }
        last_view = state.view();
        auth::AuthState::tick_flow(&auth, &mut state);
        desktop_host::tick(&mut state);
        {
            let mut ui = app.ui();
            // The theme preference (settings `theme_preference`): System leaves the OS follow
            // alone; Light/Dark pin the mode. set_theme no-ops when nothing changed.
            match state.theme_pref {
                crate::theme::ThemePreference::Light => ui.set_theme(cui::ThemeMode::Light),
                crate::theme::ThemePreference::Dark => ui.set_theme(cui::ThemeMode::Dark),
                crate::theme::ThemePreference::System => {}
            }
            crate::ui::kit::set_frame_palette(crate::ui::kit::palette(&mut ui));
            shortcuts(&mut ui, &mut state);
            draw_auth_or_shell(&mut ui, &mut state, &auth);
            if state.about_open {
                desktop_host::draw_about(&mut ui, &mut state);
            }
        }
        if state.quit_requested {
            app.quit();
        }
    }
    desktop_host::stop();
    crate::bir::shutdown();
    0
}
