//! Painted bounds of a few landmark elements, recorded during layout.
//!
//! The agent's semantic tree carried no geometry, so `assert --in-viewport`
//! could only answer "bounds are zero". A zero-size `canvas` pinned to the
//! corners of the sidebar and of the content column writes their painted
//! bounds here each frame; the drain copies them into the tree.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Bounds, IntoElement, Pixels, Styled, canvas};

pub(crate) type BoundsCell = Rc<Cell<Option<Bounds<Pixels>>>>;

#[derive(Clone, Default)]
pub(crate) struct LayoutProbe {
    pub sidebar: BoundsCell,
    pub page: BoundsCell,
}

/// An invisible element that fills its (positioned) parent and records the
/// parent's bounds on every prepaint. It paints nothing and takes no space.
pub(crate) fn bounds_probe(cell: BoundsCell) -> impl IntoElement {
    canvas(
        move |bounds, _window, _cx| cell.set(Some(bounds)),
        |_bounds, _state, _window, _cx| {},
    )
    .absolute()
    .inset_0()
}
