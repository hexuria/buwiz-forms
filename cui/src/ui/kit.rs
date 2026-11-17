//! The widget kit: the app's controls wrapped once over `cui::Ui`, styled from `theme` tokens.
//! Immediate-mode: every call draws and returns whether it was pressed/changed.

use cui::{
    Align, Button, ButtonVariant, Card, Color, ControlSize, Input, Text as TextOpts, ThemeMode,
    Tone, Ui,
};

use crate::theme::{self, Palette};

/// The palette for the current theme mode.
pub fn palette(ui: &mut Ui) -> Palette {
    theme::palette(ui.theme_mode() == ThemeMode::Dark)
}

thread_local! {
    /// The frame's palette, set once before any screen draws: color lookups inside widget
    /// arguments cannot borrow `ui` mutably, so they read this instead.
    static FRAME_PALETTE: std::cell::RefCell<Option<Palette>> = const { std::cell::RefCell::new(None) };
}

/// Records the frame's palette (the app calls this first thing every frame).
pub fn set_frame_palette(p: Palette) {
    FRAME_PALETTE.with(|c| *c.borrow_mut() = Some(p));
}

/// The frame's palette; falls back to light when no frame set it yet (tests).
pub fn frame_palette() -> Palette {
    FRAME_PALETTE.with(|c| c.borrow().unwrap_or_else(|| theme::palette(false)))
}

/// A card: bordered, rounded, filled surface.
pub fn card(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.card(
        &Card {
            pad: theme::space(4),
            gap: theme::space(3),
            ..Default::default()
        },
        f,
    );
}

/// A card variant with explicit padding/gap and optional muted surface.
pub fn card_styled(
    ui: &mut Ui,
    pad: f32,
    gap: f32,
    muted: bool,
    selected: bool,
    f: impl FnOnce(&mut Ui),
) {
    ui.card(
        &Card {
            pad,
            gap,
            muted,
            selected,
            ..Default::default()
        },
        f,
    );
}

fn button_variant(
    ui: &mut Ui,
    label: &str,
    variant: ButtonVariant,
    size: ControlSize,
    enabled: bool,
    full: bool,
) -> bool {
    ui.button_with(
        label,
        &Button {
            variant,
            size,
            disabled: !enabled,
            full_width: full,
            ..Default::default()
        },
    )
}

/// A primary button (the accent fill).
pub fn primary_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Primary,
        ControlSize::Md,
        enabled,
        false,
    )
}

/// A full-width primary button.
pub fn primary_button_block(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Primary,
        ControlSize::Md,
        enabled,
        true,
    )
}

/// A default button.
pub fn button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Default,
        ControlSize::Md,
        enabled,
        false,
    )
}

/// A secondary button.
pub fn secondary_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Secondary,
        ControlSize::Md,
        enabled,
        false,
    )
}

/// A small ghost/secondary button.
pub fn ghost_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Ghost,
        ControlSize::Sm,
        enabled,
        false,
    )
}

/// Danger button.
pub fn danger_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    button_variant(
        ui,
        label,
        ButtonVariant::Danger,
        ControlSize::Md,
        enabled,
        false,
    )
}

/// A square icon-only button named the NSH way ("chevron-left", "x", "calendar").
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: Option<&str>) -> bool {
    ui.icon_button_with(
        crate::ui::icons::glyph(icon),
        &Button {
            tooltip,
            ..Default::default()
        },
    )
}

/// A small ghost button with a leading icon, named the NSH way.
pub fn icon_ghost_button(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> bool {
    ui.button_with(
        label,
        &Button {
            variant: ButtonVariant::Ghost,
            size: ControlSize::Sm,
            icon: Some(crate::ui::icons::glyph(icon)),
            disabled: !enabled,
            ..Default::default()
        },
    )
}

/// Draw an icon glyph at `size` points in the muted tone.
pub fn icon(ui: &mut Ui, name: &str, size: f32) {
    let p = palette(ui);
    ui.icon(crate::ui::icons::glyph(name), size, p.muted_foreground);
}

/// A labelled text input; returns true when the text changed this frame.
pub fn text_input(
    ui: &mut Ui,
    id: &str,
    value: &mut String,
    placeholder: &str,
    password: bool,
) -> bool {
    // cui-rs hands the C field `value.capacity()` bytes of buffer; a plain
    // clone has capacity == len, so typing past the current length is
    // truncated. Reserve headroom before each call.
    if value.capacity() < 256 {
        value.reserve(256);
    }
    ui.input_with(
        id,
        value,
        &Input {
            placeholder: Some(placeholder),
            password,
            ..Default::default()
        },
    )
    .changed()
}

/// A text input that never holds more than `max` characters, digits only when `digits` —
/// the NSH `maxLength`/`keyboardType` contract applied the frame the text changes.
pub fn text_input_limited(
    ui: &mut Ui,
    id: &str,
    value: &mut String,
    placeholder: &str,
    password: bool,
    max: usize,
    digits: bool,
) -> bool {
    let changed = text_input(ui, id, value, placeholder, password);
    let filtered: String = value
        .chars()
        .filter(|c| !digits || c.is_ascii_digit())
        .take(max)
        .collect();
    if filtered != *value {
        *value = filtered;
    }
    changed
}

/// A labelled select; returns true when the index changed this frame.
pub fn select(ui: &mut Ui, id: &str, items: &[&str], index: &mut i32) -> bool {
    ui.select_with(id, items, index, &Default::default())
}

/// A select with one icon glyph per item (resolved names, e.g. "sun").
pub fn select_with_icons(
    ui: &mut Ui,
    id: &str,
    items: &[&str],
    icons: &[&str],
    index: &mut i32,
) -> bool {
    let glyphs: Vec<&'static str> = icons.iter().map(|n| crate::ui::icons::glyph(n)).collect();
    ui.select_with(
        id,
        items,
        index,
        &cui::Select {
            icons: &glyphs,
            ..Default::default()
        },
    )
}

/// A switch with a label.
pub fn switch(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    ui.switch(label, value)
}

/// A checkbox with a label.
pub fn checkbox(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    ui.checkbox(label, value)
}

/// A section heading inside a card.
pub fn section_title(ui: &mut Ui, text: &str) {
    ui.secondary(text);
}

fn toned_text(ui: &mut Ui, text: &str, color: Color) {
    ui.text_with(
        text,
        &TextOpts {
            size: theme::r#type::META.0,
            color,
            ..Default::default()
        },
    );
}

/// Danger-coloured text.
pub fn danger_text(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    toned_text(ui, text, p.danger);
}

/// Success-coloured text.
pub fn success_text(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    toned_text(ui, text, p.success);
}

/// Warning-coloured text.
pub fn warning_text(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    toned_text(ui, text, p.warning);
}

/// Muted meta text.
pub fn meta_text(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    toned_text(ui, text, p.muted_foreground);
}

/// A status pill.
pub fn tone_badge(ui: &mut Ui, text: &str, tone: &str) {
    let tone = match tone {
        "success" => Tone::Success,
        "warning" => Tone::Warning,
        "danger" => Tone::Danger,
        "info" | "accent" => Tone::Info,
        _ => Tone::Neutral,
    };
    ui.badge_with(
        text,
        &cui::Badge {
            tone,
            ..Default::default()
        },
    );
}

/// Centred body text in a column.
pub fn centered(ui: &mut Ui, text: &str) {
    ui.column(
        &cui::Column {
            align: Align::Center,
            ..Default::default()
        },
        |ui| ui.text(text),
    );
}

// ---------------------------------------------------------------- pressable rows and cells
// (nsh/ui's ListRow, ListGroup, PressCell and StatusChip, which CUI has no widgets for)

thread_local! {
    /// Last frame's pressable rects, for the hover fill a row needs before it is drawn.
    static PRESS_RECTS: std::cell::RefCell<std::collections::HashMap<String, cui::Rect>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn rect_hit(r: cui::Rect, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

fn press_bg(ui: &mut Ui, id: &str, selected: bool) -> Color {
    let p = frame_palette();
    let mouse = ui.mouse_pos();
    let hovered = PRESS_RECTS.with(|m| {
        m.borrow()
            .get(id)
            .is_some_and(|r| rect_hit(*r, mouse.x, mouse.y))
    });
    if selected {
        p.selected
    } else if hovered {
        p.hover
    } else {
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }
}

fn remember_rect(id: &str, rect: cui::Rect) {
    PRESS_RECTS.with(|m| {
        m.borrow_mut().insert(id.to_owned(), rect);
    });
}

/// Point-in-rect for the manual hit-tests (a `PressCell` press, a strip day).
pub fn press_hit(r: cui::Rect, x: f32, y: f32) -> bool {
    rect_hit(r, x, y)
}

/// Whether the rect remembered for `id` last frame holds the point.
pub fn press_hovered(id: &str, x: f32, y: f32) -> bool {
    PRESS_RECTS.with(|m| m.borrow().get(id).is_some_and(|r| rect_hit(*r, x, y)))
}

/// Remember a pressable rect drawn this frame (custom hit-tested controls).
pub fn press_remember(id: &str, rect: cui::Rect) {
    remember_rect(id, rect);
}

/// A `ListRow` stand-in: a full-width row inside a `list_group` card with a hover fill and a
/// press anywhere on it. CUI has no pressable row, so the rect is hit-tested by hand.
pub fn pressable_row(ui: &mut Ui, id: &str, f: impl FnOnce(&mut Ui)) -> bool {
    let bg = press_bg(ui, id, false);
    let wid = ui.make_id(id);
    ui.row(
        &cui::Row {
            id: Some(id),
            width: cui::Size::FILL,
            pad_y: theme::space(2),
            pad_x: theme::space(2),
            gap: theme::space(3),
            bg,
            ..Default::default()
        },
        f,
    );
    let rect = ui.get_rect(wid);
    remember_rect(id, rect);
    let mouse = ui.mouse_pos();
    ui.mouse_clicked(cui::MouseButton::Left) && rect_hit(rect, mouse.x, mouse.y)
}

/// A `PressCell` stand-in for grid cells: a minimum height, a selected fill, a press anywhere.
/// `pad_x`/`pad_y`/`gap` are the cell's own padding and gap (the wide calendar's p-1.5/gap-3px,
/// the narrow one's py-1/gap-0.5).
pub fn press_cell(
    ui: &mut Ui,
    id: &str,
    min_h: f32,
    selected: bool,
    radius: f32,
    pad_x: f32,
    pad_y: f32,
    gap: f32,
    f: impl FnOnce(&mut Ui),
) -> bool {
    let bg = press_bg(ui, id, selected);
    let wid = ui.make_id(id);
    ui.box_(
        &cui::BoxOpts {
            id: Some(id),
            dir: cui::Dir::Column,
            width: cui::Size::FILL,
            min_height: min_h,
            pad_x,
            pad_y,
            gap,
            bg,
            radius,
            ..Default::default()
        },
        f,
    );
    let rect = ui.get_rect(wid);
    remember_rect(id, rect);
    let mouse = ui.mouse_pos();
    ui.mouse_clicked(cui::MouseButton::Left) && rect_hit(rect, mouse.x, mouse.y)
}

/// A `ListGroup`: one card, rows inside it.
pub fn list_group(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.card(
        &Card {
            pad: 0.0,
            gap: 0.0,
            ..Default::default()
        },
        |ui| {
            f(ui);
        },
    );
}

/// A separator between two rows of a `list_group`.
pub fn row_sep(ui: &mut Ui) {
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            ..Default::default()
        },
        |ui| {
            ui.separator();
        },
    );
}

/// The tone colors a chip, dot or mini-chip is drawn in (parts.tsx CHIP_FILL/CHIP_INK/DOT_FILL).
pub fn tone_color(tone: &str) -> Color {
    let p = frame_palette();
    match tone {
        "danger" => p.danger,
        "warning" => p.warning,
        "accent" => p.accent_blue,
        "success" => p.success,
        "neutral" => p.subtle_foreground,
        _ => p.muted_foreground,
    }
}

/// A chip's soft fill (CHIP_FILL).
pub fn chip_fill(tone: &str) -> Color {
    let p = frame_palette();
    match tone {
        "accent" => p.accent_blue_soft,
        "success" => p.success_soft,
        "warning" => p.warning_soft,
        "danger" => p.danger_soft,
        "neutral" => p.hover,
        _ => p.hover,
    }
}

/// A status chip as CUI's small badge (ui/status.tsx's StatusChip).
pub fn draw_chip(ui: &mut Ui, chip: &crate::ui::logic::StatusChip) {
    ui.badge_with(
        &chip.label,
        &cui::Badge {
            tone: match chip.tone {
                "accent" => Tone::Accent,
                "info" => Tone::Info,
                "success" => Tone::Success,
                "warning" => Tone::Warning,
                "danger" => Tone::Danger,
                _ => Tone::Neutral,
            },
            size: ControlSize::Sm,
            icon: if chip.check {
                Some(cui::ICON_CHECK)
            } else if chip.send {
                Some(cui::ICON_SEND)
            } else {
                None
            },
            ..Default::default()
        },
    );
}

/// A small button (NSH `size="sm"`).
pub fn small_button(ui: &mut Ui, label: &str, variant: ButtonVariant, icon: Option<&str>) -> bool {
    ui.button_with(
        label,
        &Button {
            variant,
            size: ControlSize::Sm,
            icon: icon.map(crate::ui::icons::glyph),
            ..Default::default()
        },
    )
}

/// A segmented control (ui/Segmented): a bordered track of small buttons, the current one
/// filled. Returns the pressed option's index.
pub fn segmented(
    ui: &mut Ui,
    id: &str,
    options: &[(&str, Option<&str>)],
    current: usize,
) -> Option<usize> {
    let _g = ui.push_id(id);
    let mut picked = None;
    // ui/choice.tsx Segmented: a bg-hover track, rounded row, p-0.5 gap-0.5; each segment an
    // h-6 control, the pressed one on the raised fill.
    ui.box_(
        &cui::BoxOpts {
            dir: cui::Dir::Row,
            gap: 2.0,
            pad: 2.0,
            bg: frame_palette().hover,
            radius: theme::radius::ROW,
            ..Default::default()
        },
        |ui| {
            for (i, (label, icon)) in options.iter().enumerate() {
                let variant = if i == current {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                };
                if small_button(ui, label, variant, *icon) {
                    picked = Some(i);
                }
            }
        },
    );
    picked
}

// ---------------------------------------------------------------- method-style wrappers

/// The kit's free functions as `ui.` methods, so screens read like the TSX did.
pub trait UiExt {
    fn code(&mut self, text: &str);
    fn primary_button(&mut self, label: &str, enabled: bool) -> bool;
    fn primary_button_block(&mut self, label: &str, enabled: bool) -> bool;
    fn default_button(&mut self, label: &str, enabled: bool) -> bool;
    fn secondary_button(&mut self, label: &str, enabled: bool) -> bool;
    fn ghost_button(&mut self, label: &str, enabled: bool) -> bool;
    fn danger_button(&mut self, label: &str, enabled: bool) -> bool;
    fn text_input(
        &mut self,
        id: &str,
        value: &mut String,
        placeholder: &str,
        password: bool,
    ) -> bool;
    fn text_input_limited(
        &mut self,
        id: &str,
        value: &mut String,
        placeholder: &str,
        password: bool,
        max: usize,
        digits: bool,
    ) -> bool;
    fn danger_text(&mut self, text: &str);
    fn success_text(&mut self, text: &str);
    fn warning_text(&mut self, text: &str);
    fn meta_text(&mut self, text: &str);
    fn tone_badge(&mut self, text: &str, tone: &str);
    fn section_title(&mut self, text: &str);
    fn centered(&mut self, text: &str);
    fn card_styled(
        &mut self,
        pad: f32,
        gap: f32,
        muted: bool,
        selected: bool,
        f: impl FnOnce(&mut Ui),
    );
}

impl UiExt for Ui<'_> {
    fn code(&mut self, text: &str) {
        let p = palette(self);
        self.text_with(
            text,
            &TextOpts {
                font: cui::font::MONO,
                size: theme::r#type::META.0,
                color: p.foreground,
                tabular: true,
                ..Default::default()
            },
        );
    }
    fn primary_button(&mut self, label: &str, enabled: bool) -> bool {
        primary_button(self, label, enabled)
    }
    fn primary_button_block(&mut self, label: &str, enabled: bool) -> bool {
        primary_button_block(self, label, enabled)
    }
    fn default_button(&mut self, label: &str, enabled: bool) -> bool {
        button(self, label, enabled)
    }
    fn secondary_button(&mut self, label: &str, enabled: bool) -> bool {
        secondary_button(self, label, enabled)
    }
    fn ghost_button(&mut self, label: &str, enabled: bool) -> bool {
        ghost_button(self, label, enabled)
    }
    fn danger_button(&mut self, label: &str, enabled: bool) -> bool {
        danger_button(self, label, enabled)
    }
    fn text_input(
        &mut self,
        id: &str,
        value: &mut String,
        placeholder: &str,
        password: bool,
    ) -> bool {
        text_input(self, id, value, placeholder, password)
    }
    fn text_input_limited(
        &mut self,
        id: &str,
        value: &mut String,
        placeholder: &str,
        password: bool,
        max: usize,
        digits: bool,
    ) -> bool {
        text_input_limited(self, id, value, placeholder, password, max, digits)
    }
    fn danger_text(&mut self, text: &str) {
        danger_text(self, text)
    }
    fn success_text(&mut self, text: &str) {
        success_text(self, text)
    }
    fn warning_text(&mut self, text: &str) {
        warning_text(self, text)
    }
    fn meta_text(&mut self, text: &str) {
        meta_text(self, text)
    }
    fn tone_badge(&mut self, text: &str, tone: &str) {
        tone_badge(self, text, tone)
    }
    fn section_title(&mut self, text: &str) {
        section_title(self, text)
    }
    fn centered(&mut self, text: &str) {
        centered(self, text)
    }
    fn card_styled(
        &mut self,
        pad: f32,
        gap: f32,
        muted: bool,
        selected: bool,
        f: impl FnOnce(&mut Ui),
    ) {
        card_styled(self, pad, gap, muted, selected, f)
    }
}
