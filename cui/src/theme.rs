//! Theme: the non-color tokens and the palette, ported from `nsh/theme/tokens.ts` and
//! `nsh/theme/theme.json` (shadcn cssVars, light + dark).
//!
//! CUI's own theme already adapts to light/dark and the accent; the app reads [`palette`] for the
//! roles CUI has no token for (the muted/subtle text steps, the blue accent, the soft tints, the
//! selected-row fill, primary) and picks by [`cui::Ui::theme_mode`]. Widget colors that CUI *does*
//! theme (background, surface, border, danger, ...) are the `Color::BACKGROUND`-style tokens, so the
//! stock components stay native.

/// Type scale: points and weight (was `TYPE` in tokens.ts). `cls` has no meaning here; the size and
/// weight are what a `ui.text_with` call takes.
pub mod r#type {
    /// 20/600 — the page title.
    pub const PAGE: (f32, u16) = (20.0, 600);
    /// 16/600 — a card or section title.
    pub const TITLE: (f32, u16) = (16.0, 600);
    /// 14/600 — emphasised body text.
    pub const EMPH: (f32, u16) = (14.0, 600);
    /// 13/400 — body text.
    pub const BODY: (f32, u16) = (13.0, 400);
    /// 13/500 — control labels.
    pub const CONTROL: (f32, u16) = (13.0, 500);
    /// 12/400 — meta text.
    pub const META: (f32, u16) = (12.0, 400);
    /// 12/500 — emphasised meta text.
    pub const META_STRONG: (f32, u16) = (12.0, 500);
    /// 11/500 — captions, badges.
    pub const CAP: (f32, u16) = (11.0, 500);
    /// 12/600 tabular — numbers, TINs, amounts.
    pub const NUM: (f32, u16) = (12.0, 600);
}

/// The 4 pt spacing grid (`SPACE`): `space(2)` is 8.
pub fn space(step: u8) -> f32 {
    f32::from(step) * 4.0
}

/// Radius scale, in points (`RADIUS`).
pub mod radius {
    pub const CTL: f32 = 6.0;
    pub const ROW: f32 = 8.0;
    pub const CARD: f32 = 12.0;
    pub const DIALOG: f32 = 14.0;
    pub const FULL: f32 = 9999.0;
}

/// Fixed sizes, in points (`SIZE`).
pub mod size {
    pub const TITLE_BAR: f32 = 40.0;
    pub const SIDEBAR: f32 = 248.0;
    pub const PANEL: f32 = 320.0;
    pub const MAIN_INSET: f32 = 8.0;
    pub const CONTENT_MAX: f32 = 880.0;
    pub const ICON: f32 = 16.0;
    pub const ICON_CHIP: f32 = 12.0;
    pub const ICON_SMALL: f32 = 14.0;
    pub const CONTROL: f32 = 32.0;
    pub const CONTROL_SM: f32 = 28.0;
    pub const PILL: f32 = 30.0;
    pub const CHIP: f32 = 20.0;
    pub const SEGMENT: f32 = 24.0;
    pub const LIST_ROW: f32 = 52.0;
    pub const DISCLOSURE_HEAD: f32 = 44.0;
    pub const CAPTION_BUTTON: f32 = 46.0;
    pub const TRAFFIC_LIGHT: f32 = 12.0;
    pub const VALIDATION_LINE: f32 = 18.0;
}

/// Responsive breakpoints, in points of window width (`BREAK`).
pub mod breakpoints {
    /// Under it the sidebar and the panel are overlays (docked: from here up they dock).
    pub const OVERLAYS: f32 = 860.0;
    /// Under it the pill shows avatar and branch code only.
    pub const COMPACT_PILL: f32 = 600.0;
    /// Under it content padding is 20 x 16 and field grids have one column.
    pub const NARROW_CONTENT: f32 = 640.0;
    /// Under it the part navigator becomes a chip row.
    pub const FORM_NAV: f32 = 760.0;
    /// Under it: the narrow (phone) layout, with the bottom tab bar.
    pub const PHONE: f32 = 600.0;
}

use cui::Color;

/// The palette roles that have no CUI token, per theme. Colors with a `var(--…)` alias in theme.json
/// are already resolved to their values here.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// `background` — the page behind everything (CUI `BACKGROUND` is set from this too).
    pub background: Color,
    /// `card` — the surface cards sit on.
    pub card: Color,
    /// `raised` (= popover) — menus, dialogs, the panel.
    pub raised: Color,
    /// `hover` (= muted/secondary) — hover fills.
    pub hover: Color,
    /// `selected` — the selected-row/accent fill (NOT the blue accent).
    pub selected: Color,
    /// `foreground`.
    pub foreground: Color,
    /// `muted-foreground` — secondary text.
    pub muted_foreground: Color,
    /// `subtle-foreground` — tertiary text (4.5:1 on background/card/raised/hover/selected).
    pub subtle_foreground: Color,
    /// `primary` — the near-black/near-white button and strong fills.
    pub primary: Color,
    /// `primary-foreground` — text on `primary`.
    pub primary_foreground: Color,
    /// `border`.
    pub border: Color,
    /// `border-strong` (= input) — control borders.
    pub border_strong: Color,
    /// `accent-blue` (= ring) — links, focus, the blue accent. NOT `accent`.
    pub accent_blue: Color,
    /// `accent-blue-soft` — tinted blue fills at ~10%.
    pub accent_blue_soft: Color,
    /// `success` / `warning` / `danger` (+ `-soft` tints at ~10-13%).
    pub success: Color,
    pub success_soft: Color,
    pub warning: Color,
    pub warning_soft: Color,
    pub danger: Color,
    pub danger_soft: Color,
    /// `scrim` — the overlay behind dialogs and the mobile sheet.
    pub scrim: Color,
    /// `knob` — switch thumbs.
    pub knob: Color,
    /// `caption-close` / `-foreground` — the faux-XP caption close button.
    pub caption_close: Color,
    pub caption_close_foreground: Color,
}

const LIGHT: Palette = Palette {
    background: Color::hex(0xf4f4f6),
    card: Color::hex(0xffffff),
    raised: Color::hex(0xffffff),
    hover: Color::hex(0xebebee),
    selected: Color::hex(0xe2e2e7),
    foreground: Color::hex(0x18181b),
    muted_foreground: Color::hex(0x54545d),
    subtle_foreground: Color::hex(0x64646d),
    primary: Color::hex(0x18181b),
    primary_foreground: Color::hex(0xffffff),
    border: Color::hex(0xe2e2e6),
    border_strong: Color::hex(0xcdcdd3),
    accent_blue: Color::hex(0x2557d6),
    accent_blue_soft: Color::rgba(0x25, 0x57, 0xd6, 0x1a),
    success: Color::hex(0x157a3c),
    success_soft: Color::rgba(0x15, 0x7a, 0x3c, 0x1a),
    warning: Color::hex(0xa5520a),
    warning_soft: Color::rgba(0xc4, 0x6e, 0x0a, 0x21),
    danger: Color::hex(0xc0262d),
    danger_soft: Color::rgba(0xc0, 0x26, 0x2d, 0x1a),
    scrim: Color::rgba(0x18, 0x18, 0x1b, 0x4d),
    knob: Color::hex(0xffffff),
    caption_close: Color::hex(0xc42b1c),
    caption_close_foreground: Color::hex(0xffffff),
};

const DARK: Palette = Palette {
    background: Color::hex(0x0e0e10),
    card: Color::hex(0x161619),
    raised: Color::hex(0x232328),
    hover: Color::hex(0x1f1f23),
    selected: Color::hex(0x29292f),
    foreground: Color::hex(0xf3f3f5),
    muted_foreground: Color::hex(0xa7a7b1),
    subtle_foreground: Color::hex(0x91919b),
    primary: Color::hex(0xf3f3f5),
    primary_foreground: Color::hex(0x111113),
    border: Color::hex(0x25252a),
    border_strong: Color::hex(0x38383f),
    accent_blue: Color::hex(0x7ea2ff),
    accent_blue_soft: Color::rgba(0x7e, 0xa2, 0xff, 0x24),
    success: Color::hex(0x5bd68a),
    success_soft: Color::rgba(0x5b, 0xd6, 0x8a, 0x1f),
    warning: Color::hex(0xf3b44a),
    warning_soft: Color::rgba(0xf3, 0xb4, 0x4a, 0x21),
    danger: Color::hex(0xff7b7b),
    danger_soft: Color::rgba(0xff, 0x7b, 0x7b, 0x1f),
    scrim: Color::rgba(0x00, 0x00, 0x00, 0x8c),
    knob: Color::hex(0xffffff),
    caption_close: Color::hex(0xc42b1c),
    caption_close_foreground: Color::hex(0xffffff),
};

/// The app palette for a theme mode.
pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

/// The theme preference the app keeps (`redesign` settings' `theme` key, was `theme.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}

impl ThemePreference {
    /// `light`, `dark` or `system`; anything else is `system` (the stored rule).
    pub fn parse(value: Option<&str>) -> ThemePreference {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("light") => ThemePreference::Light,
            Some("dark") => ThemePreference::Dark,
            _ => ThemePreference::System,
        }
    }

    /// The preference after one toggle of the Settings switch: light -> dark -> system -> light.
    pub fn next(self) -> ThemePreference {
        match self {
            ThemePreference::Light => ThemePreference::Dark,
            ThemePreference::Dark => ThemePreference::System,
            ThemePreference::System => ThemePreference::Light,
        }
    }

    /// The mode to draw, given whether the OS is in dark mode: `System` follows the OS.
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            ThemePreference::Light => false,
            ThemePreference::Dark => true,
            ThemePreference::System => system_dark,
        }
    }

    /// The spelling the `theme_preference` setting is stored in ("System", "Dark", "Light").
    pub fn stored(self) -> &'static str {
        match self {
            ThemePreference::Light => "Light",
            ThemePreference::Dark => "Dark",
            ThemePreference::System => "System",
        }
    }
}

/// The theme from a dev start route ("tp=...;theme=dark;view=library"), or `None` when it names none.
pub fn theme_preference_from_start(raw: Option<&str>) -> Option<ThemePreference> {
    for part in raw.unwrap_or("").split(';') {
        let mut it = part.splitn(2, '=');
        let key = it.next().unwrap_or("").trim();
        let value = it.next().unwrap_or("").trim();
        if key == "theme" && !value.is_empty() {
            return Some(ThemePreference::parse(Some(value)));
        }
    }
    None
}
