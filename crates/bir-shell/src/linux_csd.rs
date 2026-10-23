//! Linux client-side window decorations.
//!
//! The shell asks gpui for [`WindowDecorations::Client`]. gpui forwards that
//! to xdg-decoration on Wayland and to `_MOTIF_WM_HINTS` plus
//! `_GTK_FRAME_EXTENTS` on X11, then reports what the compositor actually
//! granted through [`Window::window_decorations`].
//!
//! Linux does not turn `window_control_area` hitboxes into actions: both the
//! Wayland and X11 backends leave `on_hit_test_window_control` empty. Move,
//! resize, minimize, and zoom are explicit calls. [`window_border`] draws the
//! resize bands and sets the client inset the compositor uses to keep those
//! bands outside the window geometry. Caption clicks call `minimize_window`
//! and `zoom_window` (`zoom_window` toggles maximized and restored).
//!
//! When the compositor answers with server-side decorations — GNOME forcing
//! them, or an X11 session with no compositor — [`caption_policy`] is `None`.
//! The window manager's title bar is the only one, and this module draws no
//! second set of buttons.

use gpui_base::InteractiveElementExt as _;
use gpui_component::{Icon, IconName, Sizable as _};
use gpui_kit::{
    App, Decorations, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Window, WindowControlArea, WindowControls, div, prelude::*, px,
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

fn zoom_icon(maximized: bool) -> IconName {
    if maximized {
        IconName::WindowRestore
    } else {
        IconName::WindowMaximize
    }
}

/// Colors for the caption buttons. Close uses the destructive pair on hover.
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
            IconName::WindowMinimize,
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
            IconName::WindowClose,
            true,
            colors,
            on_close,
        ));
    }
    row
}

fn caption_button(
    id: &'static str,
    icon: IconName,
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
        .child(Icon::new(icon).small().text_color(foreground))
}

struct DragLatch {
    armed: bool,
}

/// Empty space that drags the window and toggles maximize on double-click.
///
/// A right-click opens the compositor's window menu when it offered one.
/// Server-side decorations still accept the drag: the window manager's title
/// bar is above this row, and moving from the gap does not add a second bar.
pub(crate) fn drag_spacer(window: &mut Window, cx: &mut App) -> impl IntoElement {
    with_drag(
        "menu-drag-region",
        div().id("menu-drag-region").h_full().flex_1().min_w(px(0.)),
        window,
        cx,
    )
}

/// The same drag gesture on a bar that already has an id, such as print preview.
pub(crate) fn arm_drag(
    key: &'static str,
    el: gpui_kit::Stateful<gpui_kit::Div>,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    with_drag(key, el, window, cx)
}

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
pub(crate) fn wrap_client_border(child: impl IntoElement) -> impl IntoElement {
    gpui_component::window_border().child(child)
}

/// Re-render `view` when the compositor changes decoration mode, so caption
/// buttons disappear instead of doubling a server title bar.
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
