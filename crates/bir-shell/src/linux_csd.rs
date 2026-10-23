//! Linux client-side window decorations.
//!
//! The shell asks gpui for client decorations. gpui forwards that to
//! xdg-decoration on Wayland and to `_MOTIF_WM_HINTS` plus
//! `_GTK_FRAME_EXTENTS` on X11, then reports what the compositor actually
//! granted through `Window::window_decorations`.
//!
//! Linux does not turn `window_control_area` hitboxes into actions: both the
//! Wayland and X11 backends leave `on_hit_test_window_control` empty. Move,
//! resize, minimize, and zoom are explicit calls. `window_border` draws the
//! resize bands and sets the client inset the compositor uses to keep those
//! bands outside the window geometry. Caption clicks call `minimize_window`
//! and `zoom_window` (`zoom_window` toggles maximized and restored).
//!
//! When the compositor answers with server-side decorations — GNOME forcing
//! them, or an X11 session with no compositor — [`caption_policy`] is `None`.
//! The window manager's title bar is the only one, and this module draws no
//! second set of buttons.

use gpui_kit::{Decorations, WindowControls};

#[cfg(target_os = "linux")]
use gpui_base::InteractiveElementExt as _;
#[cfg(target_os = "linux")]
use gpui_component::{Icon, Sizable as _};
#[cfg(target_os = "linux")]
use gpui_kit::{
    App, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window,
    WindowControlArea, div, prelude::*, px,
};

/// Width of one caption button. Matches the shell chrome's `captionButton`.
pub(crate) const CAPTION_BUTTON_WIDTH: f32 = 46.0;

/// Which caption buttons to draw. `None` means the compositor owns the title
/// bar, so drawing ours would stack a second set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CaptionPolicy {
    pub minimize: bool,
    pub maximize: bool,
    pub close: bool,
}

pub(crate) fn caption_policy(
    decorations: Decorations,
    controls: WindowControls,
) -> Option<CaptionPolicy> {
    match decorations {
        Decorations::Server => None,
        Decorations::Client { .. } => Some(CaptionPolicy {
            minimize: controls.minimize,
            maximize: controls.maximize,
            // Close is always ours. A tiling compositor can decline minimize
            // and maximize and still needs a way to dismiss the window.
            close: true,
        }),
    }
}

pub(crate) fn caption_width(policy: Option<CaptionPolicy>) -> f32 {
    let Some(policy) = policy else {
        return 0.0;
    };
    let count = u8::from(policy.minimize) + u8::from(policy.maximize) + u8::from(policy.close);
    f32::from(count) * CAPTION_BUTTON_WIDTH
}

/// Element id for the zoom button. Maximized windows restore; the others maximize.
pub(crate) fn zoom_button_id(maximized: bool) -> &'static str {
    if maximized {
        "caption-restore"
    } else {
        "caption-maximize"
    }
}

// Drawing, dragging, and the client border are Linux-only. The module itself
// is also built for tests on other platforms so the decoration policy can be
// checked there; these items have no callers outside Linux, so they stay out
// of those builds instead of being allowed as dead code.
/// Shell caption glyphs. These are the same stroke icons as the rest of the
/// chrome (`app/icons`). They are embedded so the buttons do not depend on the
/// script asset directory, which does not contain the component icon catalog.
#[cfg(target_os = "linux")]
const MINIMIZE_SVG: &[u8] = include_bytes!("../../../app/icons/minus.svg");
#[cfg(target_os = "linux")]
const MAXIMIZE_SVG: &[u8] = include_bytes!("../../../app/icons/square.svg");
#[cfg(target_os = "linux")]
const CLOSE_SVG: &[u8] = include_bytes!("../../../app/icons/x.svg");
/// Two overlapping squares: the restore glyph. The chrome has no separate file
/// for this one.
#[cfg(target_os = "linux")]
const RESTORE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="7" y="7" rx="2"/><path d="M17 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h2"/></svg>"#;

#[cfg(target_os = "linux")]
fn zoom_icon(maximized: bool) -> &'static [u8] {
    if maximized { RESTORE_SVG } else { MAXIMIZE_SVG }
}

/// Colors for the caption buttons. Close uses the destructive pair on hover.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub(crate) struct CaptionColors {
    pub foreground: Hsla,
    pub hover: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
}

/// Caption buttons for the right end of a client-decorated title row.
///
/// `on_close` is the main window's hide-to-tray, or `Window::remove_window`
/// for an auxiliary window such as print preview.
#[cfg(target_os = "linux")]
pub(crate) fn caption_buttons(
    window: &Window,
    colors: CaptionColors,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let mut row = div()
        .id("window-controls")
        .flex()
        .flex_row()
        .items_center()
        .h_full()
        .flex_shrink_0();
    let Some(policy) = caption_policy(window.window_decorations(), window.window_controls()) else {
        return row;
    };
    if policy.minimize {
        row = row.child(caption_button(
            "caption-minimize",
            MINIMIZE_SVG,
            false,
            colors,
            |window, _| window.minimize_window(),
        ));
    }
    if policy.maximize {
        let maximized = window.is_maximized();
        row = row.child(caption_button(
            zoom_button_id(maximized),
            zoom_icon(maximized),
            false,
            colors,
            |window, _| window.zoom_window(),
        ));
    }
    if policy.close {
        row = row.child(caption_button(
            "caption-close",
            CLOSE_SVG,
            true,
            colors,
            on_close,
        ));
    }
    row
}

#[cfg(target_os = "linux")]
fn caption_button(
    id: &'static str,
    icon: &'static [u8],
    close: bool,
    colors: CaptionColors,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let CaptionColors {
        foreground,
        hover,
        destructive,
        destructive_foreground,
    } = colors;
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(CAPTION_BUTTON_WIDTH))
        .h_full()
        .text_color(foreground)
        .hover(move |style| {
            if close {
                style.bg(destructive).text_color(destructive_foreground)
            } else {
                style.bg(hover).text_color(foreground)
            }
        })
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            // Keep the press from also arming a drag on a parent row.
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_double_click(|_, _, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx);
        })
        .child(Icon::default().data(icon).small().text_color(foreground))
}

#[cfg(target_os = "linux")]
struct DragLatch {
    armed: bool,
}

/// Empty space that drags the window and toggles maximize on double-click.
///
/// A right-click opens the compositor's window menu when it offered one.
/// Server-side decorations still accept the drag: the window manager's title
/// bar is above this row, and moving from the gap does not add a second bar.
#[cfg(target_os = "linux")]
pub(crate) fn drag_spacer(window: &mut Window, cx: &mut App) -> impl IntoElement {
    with_drag(
        "menu-drag-region",
        div().id("menu-drag-region").h_full().flex_1().min_w(px(0.)),
        window,
        cx,
    )
}

/// The same drag gesture on a bar that already has an id, such as print preview.
#[cfg(target_os = "linux")]
pub(crate) fn arm_drag(
    key: &'static str,
    el: gpui_kit::Stateful<gpui_kit::Div>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    with_drag(key, el, window, cx)
}

#[cfg(target_os = "linux")]
fn with_drag(
    key: &'static str,
    el: gpui_kit::Stateful<gpui_kit::Div>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let state = window.use_keyed_state(key, cx, |_, _| DragLatch { armed: false });
    el.window_control_area(WindowControlArea::Drag)
        .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
            state.armed = false;
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, _| {
                state.armed = true;
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, _| {
                state.armed = false;
            }),
        )
        .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
            if state.armed {
                state.armed = false;
                window.start_window_move();
            }
        }))
        .on_double_click(window.listener_for(&state, |state, _, window, _| {
            // A double-click is also a press. Drop the drag latch first so the
            // maximize configure cannot be read as the start of a move.
            state.armed = false;
            window.zoom_window();
        }))
        .on_mouse_down(MouseButton::Right, |event, window, cx| {
            cx.stop_propagation();
            if !matches!(window.window_decorations(), Decorations::Client { .. }) {
                return;
            }
            if !window.window_controls().window_menu {
                return;
            }
            window.show_window_menu(event.position);
        })
}

/// Resize edges and the shadow inset. Under server-side decorations
/// `window_border` renders the child unchanged.
#[cfg(target_os = "linux")]
pub(crate) fn wrap_client_border(child: impl IntoElement) -> impl IntoElement {
    gpui_component::window_border().child(child)
}

/// Re-render `view` when the compositor changes decoration mode, so caption
/// buttons disappear instead of doubling a server title bar.
#[cfg(target_os = "linux")]
pub(crate) fn watch_decorations<T: 'static>(
    view: &gpui_kit::Entity<T>,
    window: &mut Window,
    cx: &mut App,
) {
    view.update(cx, |_, cx| {
        cx.observe_window_appearance(window, |_: &mut T, _, cx| cx.notify())
            .detach();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::Tiling;

    fn client() -> Decorations {
        Decorations::Client {
            tiling: Tiling::default(),
        }
    }

    fn controls(minimize: bool, maximize: bool) -> WindowControls {
        WindowControls {
            fullscreen: true,
            maximize,
            minimize,
            window_menu: true,
        }
    }

    #[test]
    fn server_decorations_draw_no_caption_buttons() {
        assert!(caption_policy(Decorations::Server, controls(true, true)).is_none());
        assert_eq!(
            caption_width(caption_policy(Decorations::Server, controls(true, true))),
            0.0
        );
    }

    #[test]
    fn client_decorations_follow_what_the_compositor_can_do() {
        let policy = caption_policy(client(), controls(false, true)).unwrap();
        assert!(!policy.minimize);
        assert!(policy.maximize);
        assert!(policy.close);
        assert_eq!(caption_width(Some(policy)), CAPTION_BUTTON_WIDTH * 2.0);
    }

    #[test]
    fn client_decorations_still_offer_close_when_nothing_else_is_supported() {
        let policy = caption_policy(client(), controls(false, false)).unwrap();
        assert!(!policy.minimize);
        assert!(!policy.maximize);
        assert!(policy.close);
        assert_eq!(caption_width(Some(policy)), CAPTION_BUTTON_WIDTH);
    }

    #[test]
    fn zoom_button_names_restore_only_while_maximized() {
        assert_eq!(zoom_button_id(false), "caption-maximize");
        assert_eq!(zoom_button_id(true), "caption-restore");
    }
}
