//! Scroll areas a script can move (I-3).
//!
//! A plain scroll area keeps its offset where scripts cannot read or set it,
//! so the form page could open a section but never bring a row into view.
//! This module gives the page three things, all keyed by name:
//!
//! - `ScrollArea`: a vertical scroll area that owns a [`ScrollHandle`]. It
//!   grows to fill its parent (`flex_1`, `h_full`, no minimum size) the way
//!   the page's `#fp-scroll` and `#main-scroll` did, and hides sideways
//!   overflow.
//! - `ScrollMark`: a zero-size marker placed at the top of a row. At each
//!   prepaint it records where it sits inside its area's content, so a later
//!   scroll does not make the record stale.
//! - `scroll_reveal(area, mark, margin)`: moves the area so the marked row
//!   sits `margin` px below the area's top, clamped to what the area can
//!   scroll. The script asks the window to redraw afterwards.
//!
//! GPUI comes through `gpui_kit`, as everywhere in the host, so the same code
//! builds on upstream GPUI and on GPUI Fast.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement as _, ParentElement as _, Pixels,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, point, px,
};
use gpui_shell::{ComponentArgs, HostArguments, HostError, HostModule, HostResult, HostValue};

thread_local! {
    /// Each named area's handle, kept for as long as the app runs (a handful
    /// of names, so nothing to evict).
    static AREAS: RefCell<HashMap<String, ScrollHandle>> = RefCell::new(HashMap::new());
    /// Each named mark: its area and its top in that area's content (px).
    static MARKS: RefCell<HashMap<String, (String, f32)>> = RefCell::new(HashMap::new());
}

/// TypeScript for the exports below; `bir_module::module` appends it.
pub const DECLARATIONS: &str = r#"
/**
 * A vertical scroll area whose position `scroll_reveal` can move. It fills
 * its parent (`flex_1`, `h_full`, `min_w(0)`, `min_h(0)`) and hides sideways
 * overflow. `props.name` names it for `scroll_reveal` and `scroll_offset`
 * (default: the id).
 */
export const ScrollArea: { new(id: string, props: { name?: string }): NativeElement };
/**
 * A zero-size marker for the top of a row inside the area `props.area`. Place
 * it as an absolute child at the row's top (the row `relative()`).
 */
export const ScrollMark: { new(id: string, props: { name: string; area: string }): NativeElement };
/**
 * Move area `area` so mark `mark` sits `margin` px below its top (clamped).
 * False when either has not been painted yet (nothing moves). Call
 * `window.refresh()` afterwards so the new position is drawn.
 */
export function scroll_reveal(area: string, mark: string, margin: number): boolean;
/** How far area `area` is scrolled down (px), or null when it is not painted yet. */
export function scroll_offset(area: string): number | null;
/** Forget mark `mark` until it is painted again (before a step changes the layout). */
export function scroll_forget(mark: string): null;
"#;

/// Registers the scroll exports on the app's `bir` module.
pub fn register(module: HostModule) -> HostModule {
    module
        .component("ScrollArea", scroll_area)
        .component("ScrollMark", scroll_mark)
        .function("scroll_reveal", scroll_reveal)
        .function("scroll_offset", scroll_offset)
        .function("scroll_forget", scroll_forget)
}

fn area_handle(name: &str) -> ScrollHandle {
    AREAS.with(|areas| {
        areas
            .borrow_mut()
            .entry(name.to_owned())
            .or_default()
            .clone()
    })
}

fn known_area(name: &str) -> Option<ScrollHandle> {
    AREAS.with(|areas| areas.borrow().get(name).cloned())
}

fn prop<'a>(args: &'a ComponentArgs<'_>, key: &str) -> Option<&'a str> {
    args.props().get(key).and_then(HostValue::as_str)
}

fn scroll_area(mut args: ComponentArgs<'_>, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let name = prop(&args, "name").unwrap_or(args.id()).to_owned();
    let handle = area_handle(&name);
    div()
        .id(args.id().to_owned())
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .overflow_y_scroll()
        .overflow_x_hidden()
        .track_scroll(&handle)
        .children(args.take_children())
        .into_any_element()
}

fn scroll_mark(args: ComponentArgs<'_>, _window: &mut Window, _cx: &mut App) -> AnyElement {
    let name = prop(&args, "name").unwrap_or(args.id()).to_owned();
    let area = prop(&args, "area").unwrap_or_default().to_owned();
    canvas(
        move |bounds: Bounds<Pixels>, _window, _cx| record_mark(name, area, bounds),
        |_, (), _, _| {},
    )
    .absolute()
    .top(px(0.))
    .left(px(0.))
    .size(px(0.))
    .into_any_element()
}

/// At prepaint the area has already set this frame's bounds and offset on its
/// handle, and the mark is drawn at `area top + offset + content top`.
fn record_mark(name: String, area: String, bounds: Bounds<Pixels>) {
    let Some(handle) = known_area(&area) else {
        return;
    };
    let top = content_top(
        f32::from(bounds.top()),
        f32::from(handle.bounds().top()),
        f32::from(handle.offset().y),
    );
    MARKS.with(|marks| marks.borrow_mut().insert(name, (area, top)));
}

/// Where a mark drawn at window y `mark` sits in the content of an area whose
/// top is at window y `area` and that is scrolled to `offset` (zero or less).
fn content_top(mark: f32, area: f32, offset: f32) -> f32 {
    mark - area - offset
}

/// The offset (zero or less) that puts content y `top` `margin` px below the
/// area's top, kept within what the area can scroll (`max`, zero or more).
fn reveal_offset(top: f32, margin: f32, max: f32) -> f32 {
    let wanted = (top - margin).clamp(0., max.max(0.));
    if wanted == 0. { 0. } else { -wanted }
}

fn scroll_reveal(args: &HostArguments) -> HostResult {
    let area = args.string(0)?;
    let mark = args.string(1)?;
    let margin = args.number(2)? as f32;
    if !margin.is_finite() {
        return Err(HostError::new("scroll_reveal: margin must be a number"));
    }
    let Some(handle) = known_area(area) else {
        return Ok(HostValue::Bool(false));
    };
    let Some((_, top)) =
        MARKS.with(|marks| marks.borrow().get(mark).filter(|(a, _)| a == area).cloned())
    else {
        return Ok(HostValue::Bool(false));
    };
    let y = reveal_offset(top, margin, f32::from(handle.max_offset().y));
    handle.set_offset(point(handle.offset().x, px(y)));
    Ok(HostValue::Bool(true))
}

fn scroll_offset(args: &HostArguments) -> HostResult {
    let area = args.string(0)?;
    Ok(match known_area(area) {
        Some(handle) => HostValue::Number(-f64::from(f32::from(handle.offset().y))),
        None => HostValue::Null,
    })
}

fn scroll_forget(args: &HostArguments) -> HostResult {
    let mark = args.string(0)?;
    MARKS.with(|marks| marks.borrow_mut().remove(mark));
    Ok(HostValue::Null)
}

#[cfg(test)]
mod tests {
    use super::{content_top, reveal_offset};

    #[test]
    fn a_mark_keeps_its_content_position_whatever_the_scroll() {
        // Area top at 100; a row 640 px into the content.
        assert_eq!(content_top(740., 100., 0.), 640.);
        assert_eq!(content_top(440., 100., -300.), 640.);
    }

    #[test]
    fn reveal_puts_the_row_margin_below_the_top() {
        assert_eq!(reveal_offset(640., 24., 2000.), -616.);
    }

    #[test]
    fn reveal_clamps_at_the_top_and_the_bottom() {
        // A row near the top cannot scroll above the content's start.
        assert_eq!(reveal_offset(10., 24., 2000.), 0.);
        // A row near the end stops where the area stops.
        assert_eq!(reveal_offset(1900., 24., 1500.), -1500.);
        // An area that does not scroll stays put.
        assert_eq!(reveal_offset(400., 24., 0.), 0.);
        assert_eq!(reveal_offset(400., 24., -5.), 0.);
    }
}
