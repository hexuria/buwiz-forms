//! Main-window size and position.
//!
//! First start: about 90% of the primary display, centred. Later starts:
//! the last size and position, saved in `data_dir()/window-geometry` and
//! clamped to the current display so a window from a bigger or detached
//! monitor still opens fully visible.

use std::path::PathBuf;

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Share of the display the first window takes.
pub const FIRST_START_FRACTION: f32 = 0.9;
/// Smallest window (matches `window_min_size` in `gui.rs`).
pub const MIN_W: f32 = 620.;
pub const MIN_H: f32 = 500.;

const FILE_NAME: &str = "window-geometry";

/// About 90% of `display`, centred.
pub fn first_start(display: Rect) -> Rect {
    let w = (display.w * FIRST_START_FRACTION).round();
    let h = (display.h * FIRST_START_FRACTION).round();
    Rect {
        x: display.x + ((display.w - w) / 2.).round(),
        y: display.y + ((display.h - h) / 2.).round(),
        w,
        h,
    }
}

/// `saved` shrunk to fit `display` and moved fully onto it.
pub fn clamp_to_display(saved: Rect, display: Rect) -> Rect {
    let w = saved.w.max(MIN_W).min(display.w);
    let h = saved.h.max(MIN_H).min(display.h);
    let x = saved.x.clamp(display.x, display.x + display.w - w);
    let y = saved.y.clamp(display.y, display.y + display.h - h);
    Rect { x, y, w, h }
}

/// The bounds to open with.
pub fn initial(display: Rect, saved: Option<Rect>) -> Rect {
    match saved {
        Some(saved) => clamp_to_display(saved, display),
        None => first_start(display),
    }
}

pub fn parse(text: &str) -> Option<Rect> {
    let values: Vec<f32> = text
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match values.as_slice() {
        [x, y, w, h] if values.iter().all(|v| v.is_finite()) && *w > 0. && *h > 0. => Some(Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
        }),
        _ => None,
    }
}

pub fn format(rect: Rect) -> String {
    format!("{} {} {} {}\n", rect.x, rect.y, rect.w, rect.h)
}

fn path() -> PathBuf {
    bir_core::platform::data_dir().join(FILE_NAME)
}

pub fn load() -> Option<Rect> {
    parse(&std::fs::read_to_string(path()).ok()?)
}

pub fn save(rect: Rect) {
    let text = format(rect);
    let path = path();
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return;
    }
    if let Err(error) = std::fs::write(&path, text) {
        tracing::debug!("could not save window geometry: {error}");
    }
}

pub fn from_bounds(bounds: gpui::Bounds<gpui::Pixels>) -> Rect {
    Rect {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        w: f32::from(bounds.size.width),
        h: f32::from(bounds.size.height),
    }
}

pub fn to_bounds(rect: Rect) -> gpui::Bounds<gpui::Pixels> {
    gpui::Bounds::new(
        gpui::point(gpui::px(rect.x), gpui::px(rect.y)),
        gpui::size(gpui::px(rect.w), gpui::px(rect.h)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Rect = Rect {
        x: 0.,
        y: 0.,
        w: 1920.,
        h: 1080.,
    };

    #[test]
    fn first_start_is_ninety_percent_centred() {
        let r = first_start(DISPLAY);
        assert_eq!((r.w, r.h), (1728., 972.));
        assert_eq!((r.x, r.y), (96., 54.));
    }

    #[test]
    fn saved_inside_display_is_kept() {
        let saved = Rect {
            x: 100.,
            y: 50.,
            w: 1200.,
            h: 800.,
        };
        assert_eq!(initial(DISPLAY, Some(saved)), saved);
    }

    #[test]
    fn saved_too_big_is_shrunk_and_moved_on_screen() {
        let saved = Rect {
            x: 2500.,
            y: -300.,
            w: 2560.,
            h: 1440.,
        };
        assert_eq!(clamp_to_display(saved, DISPLAY), DISPLAY);
    }

    #[test]
    fn saved_off_screen_is_moved_back() {
        let saved = Rect {
            x: 1800.,
            y: 1000.,
            w: 800.,
            h: 600.,
        };
        let r = clamp_to_display(saved, DISPLAY);
        assert_eq!(
            r,
            Rect {
                x: 1120.,
                y: 480.,
                w: 800.,
                h: 600.
            }
        );
    }

    #[test]
    fn clamp_respects_display_origin_and_min_size() {
        let display = Rect {
            x: 1920.,
            y: 0.,
            w: 1280.,
            h: 1024.,
        };
        let saved = Rect {
            x: 0.,
            y: 0.,
            w: 100.,
            h: 100.,
        };
        assert_eq!(
            clamp_to_display(saved, display),
            Rect {
                x: 1920.,
                y: 0.,
                w: MIN_W,
                h: MIN_H
            }
        );
    }

    #[test]
    fn parse_round_trips_and_rejects_junk() {
        let r = Rect {
            x: 10.5,
            y: -4.,
            w: 900.,
            h: 700.,
        };
        assert_eq!(parse(&format(r)), Some(r));
        assert_eq!(parse("1 2 3"), None);
        assert_eq!(parse("1 2 0 4"), None);
        assert_eq!(parse("a b c d"), None);
    }
}
