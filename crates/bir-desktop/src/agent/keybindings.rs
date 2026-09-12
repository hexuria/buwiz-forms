//! GPUI Actions the agent may fire via `Op::Keybinding`.
//!
//! Dispatch is Action-only (`Window::dispatch_action` / `App::dispatch_action`).
//! The mailbox intercept must not call these bodies as a fallback.
//!
//! The painted window root is gpui-component `Root`, not `AppState`. Focused
//! `on_action` listeners on `AppState`'s div are skipped when dispatch lands
//! on the window root (no GPUI focus / `--activate` only OS-activates). Global
//! `App::on_action` handlers run in the bubble phase and call the same Action
//! bodies, which `note_keybinding_fired` / `complete_keybinding_action`.

use gpui::{Action, AnyWindowHandle, App, Keystroke, WeakEntity};

use crate::agent::ids;
use crate::app::AppState;
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

/// App-level listeners so Action fire still reaches `AppState` when the
/// window dispatch path is `Root` (not the `AppState` div). Same bodies as
/// the window `on_action` handlers — including `note_keybinding_fired`.
pub fn register_global_actions(
    app_state: WeakEntity<AppState>,
    main_window: AnyWindowHandle,
    cx: &mut App,
) {
    cx.on_action({
        let entity = app_state.clone();
        let window = main_window;
        move |action: &ToggleSidebar, cx| {
            let entity = entity.clone();
            let _ = window.update(cx, move |_, window, cx| {
                entity
                    .update(cx, |this, cx| {
                        this.handle_toggle_sidebar(action, window, cx);
                    })
                    .ok();
            });
        }
    });
    cx.on_action({
        let entity = app_state.clone();
        let window = main_window;
        move |action: &MinimizeWindow, cx| {
            let entity = entity.clone();
            let _ = window.update(cx, move |_, window, cx| {
                entity
                    .update(cx, |this, cx| {
                        this.handle_minimize_window(action, window, cx);
                    })
                    .ok();
            });
        }
    });
    cx.on_action({
        let entity = app_state.clone();
        move |_: &ToggleAppVisibility, cx| {
            let _ = entity.update(cx, |this, cx| {
                this.perform_toggle_app_visibility(cx);
            });
        }
    });
    cx.on_action({
        let entity = app_state;
        let window = main_window;
        move |action: &QuitApplication, cx| {
            let entity = entity.clone();
            let _ = window.update(cx, move |_, window, cx| {
                entity
                    .update(cx, |this, cx| {
                        this.handle_quit_application(action, window, cx);
                    })
                    .ok();
            });
        }
    });
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

    #[test]
    fn complete_keybinding_action_requires_listener() {
        let err = gpui_agent::complete_keybinding_action(None).unwrap_err();
        assert!(gpui_agent::is_keybinding_unavailable(&err), "{err}");
        assert!(err.contains("Action handler did not run"), "{err}");

        let ok =
            gpui_agent::complete_keybinding_action(Some(Ok(gpui_agent::DispatchResult::json(
                keybinding_result_json(ids::KEY_TOGGLE_SIDEBAR, "focused"),
            ))))
            .unwrap();
        let value = ok.value.expect("json result");
        assert_eq!(value["id"], ids::KEY_TOGGLE_SIDEBAR);
        assert_eq!(value["path"], "gpui.action");
    }

    #[test]
    fn intercept_does_not_green_a_no_op_action() {
        let err = gpui_agent::intercept_keybinding_action(|slot| {
            let _ = slot;
        })
        .unwrap_err();
        assert!(err.contains("Action handler did not run"), "{err}");

        let result = gpui_agent::intercept_keybinding_action(|slot| {
            *slot = Some(Ok(gpui_agent::DispatchResult::json(
                keybinding_result_json(ids::KEY_MINIMIZE, "focused"),
            )));
        })
        .unwrap();
        assert_eq!(result.value.unwrap()["id"], ids::KEY_MINIMIZE);
    }

    #[test]
    fn start_keybinding_fire_is_dispatch_only() {
        let src = include_str!("drain.rs");
        let start = src
            .find("fn start_keybinding_fire")
            .expect("start_keybinding_fire");
        let rest = &src[start..];
        let end = rest[1..]
            .find("\n    fn ")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            !body.contains("handle_toggle_sidebar"),
            "intercept must not dual-write ToggleSidebar:\n{body}"
        );
        assert!(
            !body.contains("handle_minimize_window"),
            "intercept must not dual-write MinimizeWindow:\n{body}"
        );
        assert!(
            !body.contains("perform_toggle_app_visibility"),
            "intercept must not dual-write ToggleAppVisibility:\n{body}"
        );
        assert!(
            !body.contains("handle_quit_application"),
            "intercept must not dual-write Quit:\n{body}"
        );
        assert!(
            body.contains("dispatch_action"),
            "start_keybinding_fire must dispatch_action:\n{body}"
        );
        assert!(
            !body.contains("cx.dispatch_action"),
            "render-time App::dispatch_action no-ops while the window is taken:\n{body}"
        );
        assert!(src.contains("complete_keybinding_action"));
        assert!(src.contains("note_keybinding_fired"));
        assert!(
            src.contains("register_global_actions")
                || include_str!("keybindings.rs").contains("register_global_actions")
        );
    }

    #[test]
    fn note_keybinding_fired_source_completes_pending() {
        let src = include_str!("drain.rs");
        let note = src
            .find("fn note_keybinding_fired")
            .expect("note_keybinding_fired");
        let rest = &src[note..];
        let end = rest[1..]
            .find("\n    fn ")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            body.contains("finish_keybinding_fire"),
            "ToggleSidebar / Minimize / visibility / quit must complete the pending fire from the Action body:\n{body}"
        );
    }
}
