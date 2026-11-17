//! The command palette's items (port of `nsh/shell/paletteItems.ts`): Taxpayers, Branches, Go to,
//! Forms, Actions — narrowed by the typed text; digits match a TIN. Pure over the app state: the
//! frame draws it and runs the picked action.

use crate::state::{AppState, PanelName, ViewName};

/// The palette's group headings, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteGroup {
    Taxpayers,
    Branches,
    GoTo,
    Forms,
    Actions,
}

impl PaletteGroup {
    pub fn label(self) -> &'static str {
        match self {
            PaletteGroup::Taxpayers => "Taxpayers",
            PaletteGroup::Branches => "Branches",
            PaletteGroup::GoTo => "Go to",
            PaletteGroup::Forms => "Forms",
            PaletteGroup::Actions => "Actions",
        }
    }
}

/// What running a row does (the TS `run` closures, as data).
#[derive(Debug, Clone)]
pub enum PaletteAction {
    GoTo(ViewName),
    OpenTaxpayer(String),
    OpenBranch {
        root: String,
        code: String,
    },
    NewReturn(String),
    /// The open form's pill steps (‹ ›).
    Step(i32),
    ToggleSidebar,
    ToggleProfile,
    ToggleNotifs,
    CycleTheme,
}

/// One row of the palette.
#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub group: PaletteGroup,
    pub label: String,
    /// A TIN or a shortcut, shown at the end of the row.
    pub hint: Option<String>,
    pub icon: Option<&'static str>,
    /// A taxpayer's avatar (name and TIN root) instead of an icon.
    pub avatar: Option<(String, String)>,
    /// Kept whatever the typed text says (a TIN typed as digits).
    pub matched: bool,
    pub action: PaletteAction,
}

/// The pages "Go to" offers, in order (the dev views follow, where the dev environment is on).
const PAGES: [ViewName; 8] = [
    ViewName::Home,
    ViewName::Library,
    ViewName::Calendar,
    ViewName::Filings,
    ViewName::Profile,
    ViewName::Forms,
    ViewName::Templates,
    ViewName::Settings,
];

/// The rows for the typed text, in group order.
pub fn items(app: &AppState, form_can_step: bool) -> Vec<PaletteItem> {
    let q = app.palette_query.trim().to_lowercase();
    let fresh = app.is_fresh();
    let mut items: Vec<PaletteItem> = Vec::new();
    // A TIN typed as digits, with or without dashes: 10 or more digits name a branch.
    let digits: String = q
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    let by_tin = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    let current = app.taxpayer();
    for t in app.taxpayers() {
        if by_tin && digits.len() > 9 {
            for b in &t.branches {
                if !b.tin.starts_with(&digits) {
                    continue;
                }
                items.push(PaletteItem {
                    group: PaletteGroup::Taxpayers,
                    label: if t.branches.len() > 1 {
                        format!("{} · {}", t.name, b.name)
                    } else {
                        t.name.clone()
                    },
                    hint: Some(b.tin_display.clone()),
                    icon: None,
                    avatar: Some((t.name.clone(), t.tin_root.clone())),
                    matched: true,
                    action: PaletteAction::OpenBranch {
                        root: t.tin_root.clone(),
                        code: b.code.clone(),
                    },
                });
            }
            continue;
        }
        items.push(PaletteItem {
            group: PaletteGroup::Taxpayers,
            label: t.name.clone(),
            hint: Some(t.tin_display.clone()),
            icon: None,
            avatar: Some((t.name.clone(), t.tin_root.clone())),
            matched: by_tin && t.tin_root.starts_with(&digits),
            action: PaletteAction::OpenTaxpayer(t.tin_root.clone()),
        });
    }
    // The open taxpayer's branches are one step away.
    if let Some(t) = &current
        && t.branches.len() > 1
        && !by_tin
    {
        for b in &t.branches {
            items.push(PaletteItem {
                group: PaletteGroup::Branches,
                label: b.name.clone(),
                hint: Some(b.tin_display.clone()),
                icon: Some("git-branch"),
                avatar: None,
                matched: false,
                action: PaletteAction::OpenBranch {
                    root: t.tin_root.clone(),
                    code: b.code.clone(),
                },
            });
        }
    }
    let dev = crate::bir::door_status().test_environment;
    for view in PAGES.iter().copied().chain(
        ViewName::all()
            .into_iter()
            .filter(|v| !PAGES.contains(v) && v.meta().dev && dev),
    ) {
        let meta = view.meta();
        if fresh && meta.needs_taxpayer {
            continue;
        }
        items.push(PaletteItem {
            group: PaletteGroup::GoTo,
            label: meta.label.to_owned(),
            hint: None,
            icon: Some(meta.icon),
            avatar: None,
            matched: false,
            action: PaletteAction::GoTo(view),
        });
    }
    if !q.is_empty() {
        // The form index is built when the palette opens (the frame calls screen_index once);
        // a palette read without opening builds it here.
        for f in crate::state::screen_index() {
            items.push(PaletteItem {
                group: PaletteGroup::Forms,
                label: format!("{} · {}", f.c, f.t),
                hint: None,
                icon: Some("file-text"),
                avatar: None,
                matched: false,
                action: PaletteAction::NewReturn(f.c.clone()),
            });
        }
    }
    // The open form's pill steps (only while a form is open and shows its pill).
    if app.view() == ViewName::Form && !fresh && form_can_step {
        items.push(PaletteItem {
            group: PaletteGroup::Actions,
            label: "Next to do".to_owned(),
            hint: Some("Ctrl+→".to_owned()),
            icon: Some("chevron-right"),
            avatar: None,
            matched: false,
            action: PaletteAction::Step(1),
        });
        items.push(PaletteItem {
            group: PaletteGroup::Actions,
            label: "Previous to do".to_owned(),
            hint: Some("Ctrl+←".to_owned()),
            icon: Some("chevron-left"),
            avatar: None,
            matched: false,
            action: PaletteAction::Step(-1),
        });
    }
    items.push(PaletteItem {
        group: PaletteGroup::Actions,
        label: "Toggle sidebar".to_owned(),
        hint: Some("Ctrl+B".to_owned()),
        icon: Some("panel-left"),
        avatar: None,
        matched: false,
        action: PaletteAction::ToggleSidebar,
    });
    let profile_open = app.panel == Some(PanelName::Profile);
    items.push(PaletteItem {
        group: PaletteGroup::Actions,
        label: if profile_open {
            "Hide tax profile".to_owned()
        } else {
            "Show tax profile".to_owned()
        },
        hint: Some("Ctrl+Shift+B".to_owned()),
        icon: Some("panel-right"),
        avatar: None,
        matched: false,
        action: PaletteAction::ToggleProfile,
    });
    let notifs_open = app.panel == Some(PanelName::Notifs);
    items.push(PaletteItem {
        group: PaletteGroup::Actions,
        label: if notifs_open {
            "Hide notifications".to_owned()
        } else {
            "Show notifications".to_owned()
        },
        hint: Some("Ctrl+Shift+N".to_owned()),
        icon: Some("bell"),
        avatar: None,
        matched: false,
        action: PaletteAction::ToggleNotifs,
    });
    items.push(PaletteItem {
        group: PaletteGroup::Actions,
        label: "Change theme".to_owned(),
        hint: Some("Ctrl+T".to_owned()),
        icon: Some("moon"),
        avatar: None,
        matched: false,
        action: PaletteAction::CycleTheme,
    });
    if q.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|x| {
            x.matched
                || format!("{} {}", x.label, x.hint.clone().unwrap_or_default())
                    .to_lowercase()
                    .contains(&q)
        })
        .collect()
}

/// Runs a palette row: closes the palette and does what it says.
pub fn run(app: &mut AppState, item: PaletteAction) {
    app.close_overlay();
    match item {
        PaletteAction::GoTo(v) => app.go(v, Default::default()),
        PaletteAction::OpenTaxpayer(root) => app.open_taxpayer(&root),
        PaletteAction::OpenBranch { root, code } => app.open_branch(&root, &code),
        PaletteAction::NewReturn(code) => app.start_new_return(Some(&code)),
        PaletteAction::Step(dir) => crate::screens::form::step(dir),
        PaletteAction::ToggleSidebar => app.toggle_sidebar(),
        PaletteAction::ToggleProfile => app.toggle_panel(PanelName::Profile),
        PaletteAction::ToggleNotifs => app.toggle_panel(PanelName::Notifs),
        PaletteAction::CycleTheme => app.cycle_theme(),
    }
}
