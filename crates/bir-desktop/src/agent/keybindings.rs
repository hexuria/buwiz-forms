//! GPUI Actions the agent may fire via `Op::Keybinding`.
//!
//! Dispatch is Action-only (`Window::dispatch_action` / `App::dispatch_action`).
//! The mailbox intercept must not call these bodies as a fallback.

use gpui::{Action, Keystroke};

use crate::agent::ids;
use crate::global_actions::*;

pub fn catalog() -> Vec<gpui_agent::KeybindingInfo> {
    vec![
        gpui_agent::KeybindingInfo::new(
            ids::KEY_TOGGLE_SIDEBAR,
            ids::toggle_sidebar_chord(),
            gpui_agent::KeybindingScope::Focused,
            false,
        ),
        gpui_agent::KeybindingInfo::new(
            ids::KEY_MINIMIZE,
            ids::minimize_chord(),
            gpui_agent::KeybindingScope::Focused,
            false,
        ),
        gpui_agent::KeybindingInfo::new(
            ids::KEY_TOGGLE_VISIBILITY,
            ids::toggle_visibility_chord(),
            gpui_agent::KeybindingScope::Global,
            false,
        ),
        gpui_agent::KeybindingInfo::new(
            ids::KEY_QUIT,
            ids::quit_chord(),
            gpui_agent::KeybindingScope::Global,
            true,
        ),
    ]
}

pub fn action_for_binding(binding: &str) -> Option<Box<dyn Action>> {
    match binding {
        ids::KEY_TOGGLE_SIDEBAR => Some(Box::new(ToggleSidebar)),
        ids::KEY_MINIMIZE => Some(Box::new(MinimizeWindow)),
        ids::KEY_TOGGLE_VISIBILITY => Some(Box::new(ToggleAppVisibility)),
        ids::KEY_QUIT => Some(Box::new(QuitApplication)),
        _ => None,
    }
}

pub fn keybinding_result_json(id: &str, scope: &str) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": id,
        "scope": scope,
        "path": "gpui.action"
    });
    if gpui_agent::is_quit_binding(id) {
        value["dangerous"] = serde_json::Value::Bool(true);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_required_actions() {
        let cat = catalog();
        assert_eq!(cat[0].id, ids::KEY_TOGGLE_SIDEBAR);
        assert_eq!(cat[0].scope, gpui_agent::KeybindingScope::Focused);
        assert_eq!(cat[0].chord, ids::toggle_sidebar_chord());
        assert_eq!(cat[1].id, ids::KEY_MINIMIZE);
        assert_eq!(cat[2].id, ids::KEY_TOGGLE_VISIBILITY);
        assert_eq!(cat[2].scope, gpui_agent::KeybindingScope::Global);
        assert_eq!(cat[3].id, ids::KEY_QUIT);
        assert!(cat[3].dangerous);
        assert!(Keystroke::parse(ids::toggle_sidebar_chord()).is_ok());
        assert!(Keystroke::parse(ids::minimize_chord()).is_ok());
        assert!(Keystroke::parse(ids::toggle_visibility_chord()).is_ok());
        assert!(
            gpui_agent::keystroke_token("cmd-q").is_err(),
            "free-form key must still reject cmd-q"
        );
    }

    #[test]
    fn action_ids_map_to_gpui_actions() {
        assert_eq!(
            action_for_binding(ids::KEY_TOGGLE_SIDEBAR).unwrap().name(),
            ToggleSidebar.name()
        );
        assert_eq!(
            action_for_binding(ids::KEY_MINIMIZE).unwrap().name(),
            MinimizeWindow.name()
        );
        assert_eq!(
            action_for_binding(ids::KEY_TOGGLE_VISIBILITY)
                .unwrap()
                .name(),
            ToggleAppVisibility.name()
        );
        assert_eq!(
            action_for_binding(ids::KEY_QUIT).unwrap().name(),
            QuitApplication.name()
        );
        assert!(action_for_binding("nope").is_none());
    }
}
