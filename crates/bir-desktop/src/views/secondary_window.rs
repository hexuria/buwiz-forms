//! Standard window shortcuts for windows other than the main one.
//!
//! `CloseWindow` / `MinimizeWindow` / `ZoomWindow` are bound globally
//! (`platform::bind_global_keys`) and sit in the app menu, but their handlers
//! lived only on `AppState`'s root, so in a print preview or a job log the
//! action had no target and Cmd+W / Cmd+M did nothing. Every secondary window
//! wraps its root with [`with_window_actions`]: close means *this* window,
//! not the app.

use gpui::{App, Context, Div, FocusHandle, InteractiveElement, Window};

use crate::global_actions::{CloseWindow, MinimizeWindow, ZoomWindow};

pub(crate) fn with_window_actions<V: 'static>(
    root: Div,
    focus_handle: &FocusHandle,
    key_context: &'static str,
    cx: &mut Context<V>,
) -> Div {
    root.key_context(key_context)
        .track_focus(focus_handle)
        .on_action(cx.listener(|_, _: &CloseWindow, window: &mut Window, _| {
            window.remove_window();
        }))
        .on_action(
            cx.listener(|_, _: &MinimizeWindow, window: &mut Window, _| {
                window.minimize_window();
            }),
        )
        .on_action(cx.listener(|_, _: &ZoomWindow, window: &mut Window, _| {
            window.zoom_window();
        }))
}

/// Give the window's root keyboard focus so the bindings resolve as soon
/// as it opens, before the user clicks anything.
pub(crate) fn focus_on_open(focus_handle: &FocusHandle, window: &mut Window, cx: &mut App) {
    if !focus_handle.is_focused(window) {
        window.focus(focus_handle, cx);
    }
}
