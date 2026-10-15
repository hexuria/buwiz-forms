// The design system for the shell redesign: tokens and wrapper components.
//
// Source of truth: DESIGN_SYSTEM.md in hexuria/buwiz-ui-redesign (a working
// copy sits in docs/ui-redesign/). Screens build from these wrappers and
// never set a raw size or a literal color: every number below is a token,
// and every color has a light and a dark value.
//
// Wrappers are plain functions returning elements. Each one takes a stable
// `id` first; that id is the element id the gpui-agent tooling addresses, so
// it must not depend on list position or render order.

import { div, svg } from "gpui-kit";
import { Input, Popup, OtpInput, OtpState } from "gpui-base";

// ---------------------------------------------------------------- tokens

/** @typedef {Record<string, import("gpui-kit").Color>} Palette */

/**
 * Color tokens, light and dark (DESIGN_SYSTEM.md "Tokens · Color").
 * @type {{ light: Palette, dark: Palette }}
 */
export const PALETTES = {
  light: {
    bg: "#f4f4f6",
    surface: "#ffffff",
    raised: "#ffffff",
    hover: "#ebebee",
    selected: "#e2e2e7",
    border: "#e2e2e6",
    borderStrong: "#cdcdd3",
    fg: "#18181b",
    fg2: "#54545d",
    fg3: "#71717a",
    primary: "#18181b",
    primaryFg: "#ffffff",
    accent: "#2557d6",
    success: "#157a3c",
    warning: "#a5520a",
    danger: "#c0262d",
    // Soft tints: 10–14 % alpha of the same hue.
    accentSoft: "#2557d61a",
    successSoft: "#157a3c1a",
    warningSoft: "#c46e0a21",
    dangerSoft: "#c0262d1a",
    scrim: "#18181b4d",
    // Switch knob and the one system value the spec allows (caption close hover).
    knob: "#ffffff",
    captionClose: "#c42b1c",
    captionCloseFg: "#ffffff",
  },
  dark: {
    bg: "#0e0e10",
    surface: "#161619",
    raised: "#232328",
    hover: "#1f1f23",
    selected: "#29292f",
    border: "#25252a",
    borderStrong: "#38383f",
    fg: "#f3f3f5",
    fg2: "#a7a7b1",
    fg3: "#8b8b95",
    primary: "#f3f3f5",
    primaryFg: "#111113",
    accent: "#7ea2ff",
    success: "#5bd68a",
    warning: "#f3b44a",
    danger: "#ff7b7b",
    accentSoft: "#7ea2ff24",
    successSoft: "#5bd68a1f",
    warningSoft: "#f3b44a21",
    dangerSoft: "#ff7b7b1f",
    scrim: "#0000008c",
    knob: "#ffffff",
    captionClose: "#c42b1c",
    captionCloseFg: "#ffffff",
  },
};

/** Avatar tones per theme: saturation, background lightness, text lightness. */
const AVATAR_TONE = {
  light: { s: 55, bg: 90, fg: 30 },
  dark: { s: 40, bg: 26, fg: 84 },
};

/** Type scale (DESIGN_SYSTEM.md "Tokens · Type"). */
export const TYPE = {
  page: { size: 20, weight: 600 },
  title: { size: 16, weight: 600 },
  emph: { size: 14, weight: 600 },
  body: { size: 13, weight: 400 },
  control: { size: 13, weight: 500 },
  meta: { size: 12, weight: 400 },
  metaStrong: { size: 12, weight: 500 },
  cap: { size: 11, weight: 500 },
  num: { size: 12, weight: 600 },
};

/** The 4 px spacing grid. Index by step: SPACE[2] === 8. */
export const SPACE = { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 20, 6: 24, 7: 28, 8: 32 };

/** Radius scale. */
export const RADIUS = { ctl: 6, row: 8, card: 12, dialog: 14, full: 9999 };

/** Fixed sizes from "Size, space, radius". */
export const SIZE = {
  titleBar: 40,
  sidebar: 248,
  panel: 320,
  mainInset: 8,
  contentMax: 880,
  icon: 16,
  iconChip: 12,
  iconSmall: 14,
  control: 32,
  controlSm: 28,
  pill: 30,
  chip: 20,
  segment: 24,
  listRow: 52,
  disclosureHead: 44,
  captionButton: 46,
  trafficLight: 12,
  validationLine: 18,
};

/** Responsive breakpoints. */
export const BREAK = {
  overlays: 860, // sidebar and panel become overlays below this
  compactPill: 600, // pill shows avatar + branch code, back/forward hidden
  narrowContent: 640, // content padding 20 × 16, one-column field grids
  formNav: 760, // part navigator becomes a chip row
};

/**
 * The current palette, resolved from the live theme.
 * @returns {Palette}
 */
export function tokens(cx) {
  return cx.theme().is_dark ? PALETTES.dark : PALETTES.light;
}

/**
 * Map the design tokens onto gpui-base's semantic roles, so base components
 * (inputs, checkboxes, scrollbars, tooltips) draw in the same palette.
 */
export function baseColors(appearance) {
  const t = appearance === "dark" ? PALETTES.dark : PALETTES.light;
  return {
    background: t.bg,
    foreground: t.fg,
    surface: t.surface,
    surface_foreground: t.fg,
    primary: t.primary,
    primary_foreground: t.primaryFg,
    secondary: t.hover,
    secondary_foreground: t.fg,
    muted: t.hover,
    muted_foreground: t.fg2,
    accent: t.selected,
    accent_foreground: t.fg,
    destructive: t.danger,
    destructive_foreground: t.primaryFg,
    border: t.border,
    input: t.borderStrong,
    ring: t.accent,
    selection: t.accentSoft,
  };
}

// ---------------------------------------------------------------- text

/** A run of text in one type token. `color` is a palette key, default fg. */
export function Text(cx, style, content, color) {
  const t = tokens(cx);
  const s = TYPE[style] || TYPE.body;
  return div()
    .text_size(s.size)
    .font_weight(s.weight)
    .text_color(t[color || "fg"] || t.fg)
    .child(content == null ? "" : String(content));
}

/** One-line text that truncates with an ellipsis inside its box. */
export function Line(cx, style, content, color) {
  return Text(cx, style, content, color).min_w(0).truncate();
}

/**
 * TINs, branch codes and form codes: the UI face (never monospace) in the
 * `num` token. GPUI exposes no `tnum` feature to scripts yet, so digits use
 * the face's default figures.
 */
export function Code(cx, content, color) {
  return Text(cx, "num", content, color).whitespace_nowrap();
}

/** The page title, one per view. */
export function PageTitle(cx, content) {
  return Text(cx, "page", content);
}

// ---------------------------------------------------------------- icon

/** Lucide icon at a design size: 16 everywhere, 12 in chips, 14 in pills/segments. */
export function Icon(cx, name, opts = {}) {
  const t = tokens(cx);
  const size = opts.size || SIZE.icon;
  return svg(`icons/${name}.svg`)
    .flex_none()
    .size(size)
    .text_color(t[opts.color || "fg2"] || t.fg2);
}

// ---------------------------------------------------------------- buttons

/**
 * IconButton: 28 × 28, radius 6, 16 px icon in fg-2 at rest, `selected`
 * fill when pressed. A tooltip is required.
 */
export function IconButton(cx, id, icon, opts) {
  const t = tokens(cx);
  if (!opts || !opts.tooltip) throw new Error(`IconButton ${id} needs a tooltip`);
  const disabled = Boolean(opts.disabled);
  const btn = div()
    .id(id)
    .relative()
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .size(SIZE.controlSm)
    .rounded(RADIUS.ctl)
    .tooltip(opts.tooltip)
    .accessibility_label(opts.tooltip)
    .child(Icon(cx, icon, { color: opts.pressed ? "fg" : disabled ? "fg3" : "fg2" }));
  if (opts.pressed) btn.bg(t.selected);
  if (disabled) {
    btn.opacity(0.5).disabled(true);
  } else {
    btn.cursor_pointer().hover((s) => s.bg(opts.pressed ? t.selected : t.hover));
    if (opts.onClick) btn.on_click((e, cx) => opts.onClick(e, cx));
  }
  if (opts.dot) {
    btn.child(
      div()
        .absolute()
        .top(5)
        .right(5)
        .size(6)
        .rounded(RADIUS.full)
        .bg(t.accent),
    );
  }
  return btn;
}

/**
 * AppButton: 32 (md) or 28 (sm) tall, padding 12 or 10, label 13/500,
 * leading icon 16, gap 6, radius 6. Variants: primary (inverted ink),
 * secondary (bordered surface), ghost, danger (soft tint).
 */
export function AppButton(cx, id, label, opts = {}) {
  const t = tokens(cx);
  const variant = opts.variant || "secondary";
  const sm = opts.size === "sm";
  const disabled = Boolean(opts.disabled);
  const fg =
    variant === "primary" ? "primaryFg" : variant === "danger" ? "danger" : variant === "ghost" ? "fg2" : "fg";
  const btn = div()
    .id(id)
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .gap(6)
    .h(sm ? SIZE.controlSm : SIZE.control)
    .px(sm ? 10 : 12)
    .rounded(RADIUS.ctl)
    .border(1)
    .border_color(variant === "secondary" ? t.borderStrong : "#00000000")
    .whitespace_nowrap()
    .accessibility_label(opts.ariaLabel || label);
  if (variant === "primary") btn.bg(t.primary);
  else if (variant === "secondary") btn.bg(t.surface);
  else if (variant === "danger") btn.bg(t.dangerSoft);
  if (opts.leading) btn.child(opts.leading);
  else if (opts.icon) btn.child(Icon(cx, opts.icon, { color: fg }));
  btn.child(Text(cx, "control", label, fg));
  if (opts.trailing) btn.child(opts.trailing);
  if (disabled) {
    btn.opacity(0.5).disabled(true);
  } else {
    btn.cursor_pointer();
    if (variant === "primary") btn.hover((s) => s.opacity(0.88));
    else if (variant === "secondary") btn.hover((s) => s.bg(t.hover));
    else if (variant === "ghost") btn.hover((s) => s.bg(t.hover));
    else btn.hover((s) => s.opacity(0.9));
    if (opts.onClick) btn.on_click((e, cx) => opts.onClick(e, cx));
  }
  if (opts.tooltip) btn.tooltip(opts.tooltip);
  return btn;
}

// ---------------------------------------------------------------- nav

/**
 * NavRow: 32 tall, padding 0 8, gap 10, radius 8. `leading` is an icon name
 * or an element (an Avatar, a Code). Optional count, trailing element, kbd.
 */
export function NavRow(cx, id, leading, label, opts = {}) {
  const t = tokens(cx);
  const current = Boolean(opts.current);
  const row = div()
    .id(id)
    .flex()
    .flex_row()
    .items_center()
    .gap(10)
    .w_full()
    .h(SIZE.control)
    .flex_none()
    .px(SPACE[2])
    .rounded(RADIUS.row)
    .cursor_pointer()
    .accessibility_label(label);
  if (current) row.bg(t.selected);
  else row.hover((s) => s.bg(t.hover));
  if (leading) {
    row.child(typeof leading === "string" ? Icon(cx, leading, { color: current ? "fg" : "fg2" }) : leading);
  }
  row.child(Line(cx, "control", label, current ? "fg" : "fg2").flex_1());
  if (opts.count !== undefined && opts.count !== null && opts.count !== "") {
    row.child(Text(cx, "meta", opts.count, "fg3").flex_none());
  }
  if (opts.kbd) row.child(Kbd(cx, opts.kbd));
  if (opts.trailing) row.child(opts.trailing);
  if (opts.tooltip) row.tooltip(opts.tooltip);
  if (opts.onClick) row.on_click((e, cx) => opts.onClick(e, cx));
  return row;
}

/** Group label inside sidebars and panels: 12/500 fg-3, sentence case. */
export function GroupLabel(cx, label) {
  return div().pt(SPACE[4]).pb(SPACE[1]).px(SPACE[2]).child(Text(cx, "metaStrong", label, "fg3"));
}

/** Keyboard hint: 11/500 fg-3 in a hairline box, 18 tall. */
export function Kbd(cx, label) {
  const t = tokens(cx);
  return div()
    .flex()
    .flex_none()
    .items_center()
    .h(18)
    .px(SPACE[1])
    .rounded(4)
    .border(1)
    .border_color(t.border)
    .child(Text(cx, "cap", label, "fg3"));
}

// ---------------------------------------------------------------- avatar

function hslToHex(h, s, l) {
  s /= 100;
  l /= 100;
  const k = (n) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  const hex = (x) =>
    Math.round(x * 255)
      .toString(16)
      .padStart(2, "0");
  return /** @type {import("gpui-kit").Color} */ (`#${hex(f(0))}${hex(f(8))}${hex(f(4))}`);
}

/** A stable hue from a taxpayer id (TIN root) or a name. */
export function hueFor(key) {
  const s = String(key || "");
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

/** Initials: the first and last capitalised words. */
export function initials(name) {
  const words = String(name || "")
    .split(/\s+/)
    .filter((w) => /^[A-Z]/.test(w));
  if (!words.length) return "?";
  return words[0][0] + (words.length > 1 ? words[words.length - 1][0] : "");
}

const AVATAR_FONT = { 20: 9, 28: 11, 40: 14, 56: 19 };

/** Avatar: 20 (rows, pill), 28 (account), 40 (page header), 56 (panel). */
export function Avatar(cx, name, size, hueKey) {
  const tone = cx.theme().is_dark ? AVATAR_TONE.dark : AVATAR_TONE.light;
  const hue = hueFor(hueKey || name);
  return div()
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .size(size)
    .rounded(RADIUS.full)
    .bg(hslToHex(hue, tone.s, tone.bg))
    .child(
      div()
        .text_size(AVATAR_FONT[size] || Math.round(size * 0.36))
        .font_weight(600)
        .text_color(hslToHex(hue, tone.s, tone.fg))
        .child(initials(name)),
    );
}

/** A 28 px numbered/icon circle (welcome steps, empty panel). */
export function StepCircle(cx, content, size = 28) {
  const t = tokens(cx);
  return div()
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .size(size)
    .rounded(RADIUS.full)
    .bg(t.hover)
    .child(typeof content === "string" ? Text(cx, "metaStrong", content, "fg2").font_weight(600) : content);
}

// ---------------------------------------------------------------- context pill

/**
 * ContextPill: 30 tall, full radius, avatar 20, name 13/600, TIN 12, chevron
 * 14 pointing toward where the panel is. `compact` shows avatar + branch code.
 */
export function ContextPill(cx, id, opts) {
  const t = tokens(cx);
  const pill = div()
    .id(id)
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .gap(SPACE[2])
    .h(SIZE.pill)
    .max_w(460)
    .min_w(0)
    .pl(opts.empty ? 10 : SPACE[1])
    .pr(10)
    .rounded(RADIUS.full)
    .border(1)
    .border_color(opts.open ? t.borderStrong : t.border)
    .bg(opts.open ? t.selected : t.surface)
    .cursor_pointer()
    .hover((s) => s.border_color(t.borderStrong))
    .accessibility_label(opts.empty ? "Add a taxpayer" : `Tax profile: ${opts.name}`)
    .on_click((e, cx) => opts.onClick(e, cx));
  if (opts.empty) {
    pill.child(Icon(cx, "plus", { size: SIZE.iconSmall, color: "fg3" }));
    pill.child(Line(cx, "control", "Add a taxpayer").font_weight(600));
  } else {
    pill.child(Avatar(cx, opts.name, 20, opts.hueKey));
    if (opts.compact) {
      pill.child(Text(cx, "meta", opts.branchCode, "fg2"));
    } else {
      pill.child(Line(cx, "control", opts.name).font_weight(600));
      pill.child(Text(cx, "meta", opts.tin, "fg2").whitespace_nowrap().flex_none());
    }
  }
  pill.child(Icon(cx, opts.open ? "chevron-left" : "chevron-right", { size: SIZE.iconSmall, color: "fg3" }));
  return pill;
}

// ---------------------------------------------------------------- chips

/** Chip: 20 tall, 11/500, soft tint, optional 12 px icon. tone: neutral|accent|success|warning|danger. */
export function Chip(cx, label, tone = "neutral", icon) {
  const t = tokens(cx);
  const fg = tone === "neutral" ? "fg2" : tone;
  const bg = tone === "neutral" ? t.hover : t[`${tone}Soft`];
  const chip = div()
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .gap(SPACE[1])
    .h(SIZE.chip)
    .px(SPACE[2])
    .rounded(RADIUS.full)
    .bg(bg)
    .whitespace_nowrap();
  if (icon) chip.child(Icon(cx, icon, { size: SIZE.iconChip, color: fg }));
  return chip.child(Text(cx, "cap", label, fg));
}

/**
 * StatusChip: NotStarted, Draft, Filed, DueSoon(days), Overdue(days).
 * `status` is one of "not_started" | "draft" | "filed" | "due_soon" | "overdue"
 * | "queued".
 */
export function StatusChip(cx, status, days) {
  switch (status) {
    case "draft":
      return Chip(cx, "Draft", "accent");
    case "queued":
      return Chip(cx, "Queued", "accent");
    case "filed":
      return Chip(cx, "Filed", "success", "check");
    case "due_soon":
      return Chip(cx, days === 0 ? "Due today" : `In ${days} day${days === 1 ? "" : "s"}`, "warning");
    case "overdue":
      return Chip(cx, `${Math.abs(days)} day${Math.abs(days) === 1 ? "" : "s"} overdue`, "danger");
    default:
      return Chip(cx, "Not started");
  }
}

// ---------------------------------------------------------------- segmented

/**
 * Segmented: 2 px track padding on `hover`, 24 px segments, 12/500, the
 * selected segment on `raised`. options: [{value, label, icon?}].
 */
export function Segmented(cx, id, options, selected, onChange, opts = {}) {
  const t = tokens(cx);
  const track = div()
    .id(id)
    .flex()
    .flex_row()
    .flex_none()
    .gap(2)
    .p(2)
    .rounded(RADIUS.row)
    .bg(t.hover)
    .accessibility_label(opts.label || id);
  for (const option of options) {
    const on = option.value === selected;
    const seg = div()
      .id(`${id}-${option.value}`)
      .flex()
      .flex_row()
      .items_center()
      .gap(6)
      .h(SIZE.segment)
      .px(10)
      .rounded(RADIUS.ctl)
      .cursor_pointer()
      .accessibility_label(String(option.label));
    if (on) seg.bg(t.raised).shadow_xs();
    else seg.hover((s) => s.text_color(t.fg));
    if (option.icon) seg.child(Icon(cx, option.icon, { size: SIZE.iconSmall, color: on ? "fg" : "fg2" }));
    seg.child(Text(cx, "metaStrong", option.label, on ? "fg" : "fg2"));
    seg.on_click((_e, cx) => onChange(option.value, cx));
    track.child(seg);
  }
  return track;
}

// ---------------------------------------------------------------- switch / checkbox

/** Switch: 32 × 18 track, accent when on, 14 px white knob. */
export function Switch(cx, id, checked, onChange, label) {
  const t = tokens(cx);
  return div()
    .id(id)
    .relative()
    .flex_none()
    .w(32)
    .h(18)
    .rounded(RADIUS.full)
    .bg(checked ? t.accent : t.borderStrong)
    .cursor_pointer()
    .accessibility_label(label || id)
    .on_click((_e, cx) => onChange(!checked, cx))
    .child(
      div()
        .absolute()
        .top(2)
        .left(checked ? 16 : 2)
        .size(14)
        .rounded(RADIUS.full)
        .bg(t.knob)
        .shadow_xs(),
    );
}

/** Checkbox: 16 × 16, radius 4, primary fill with a 12 px check when on. */
export function Checkbox(cx, id, checked, onChange, label) {
  const t = tokens(cx);
  const box = div()
    .id(id)
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .size(16)
    .rounded(4)
    .border(1)
    .border_color(checked ? t.primary : t.borderStrong)
    .bg(checked ? t.primary : t.surface)
    .cursor_pointer()
    .accessibility_label(label || id)
    .on_click((_e, cx) => {
      cx.stop_propagation();
      onChange(!checked, cx);
    });
  if (checked) box.child(Icon(cx, "check", { size: SIZE.iconChip, color: "primaryFg" }));
  return box;
}

/** A checkbox with its label to the right, the whole thing clickable. */
export function CheckboxLabel(cx, id, checked, onChange, label) {
  return div()
    .id(`${id}-row`)
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .cursor_pointer()
    .on_click((_e, cx) => onChange(!checked, cx))
    .child(Checkbox(cx, id, checked, onChange, label))
    .child(Text(cx, "body", label));
}

// ---------------------------------------------------------------- inputs

/** TextField: 32 tall, padding 0 10, radius 8, border-strong; danger border on error. */
export function TextField(cx, state, opts = {}) {
  const t = tokens(cx);
  const el = Input.new(state)
    .h(SIZE.control)
    .px(10)
    .rounded(RADIUS.row)
    .border(1)
    .border_color(opts.error ? t.danger : t.borderStrong)
    .bg(opts.disabled ? t.hover : t.surface)
    .text_size(TYPE.body.size)
    .text_color(opts.disabled ? t.fg2 : t.fg);
  if (opts.width) el.w(opts.width);
  else el.w_full();
  if (opts.disabled) el.disabled(true);
  if (opts.id) el.id(opts.id);
  if (opts.label) el.accessibility_label(opts.label);
  return el;
}

/**
 * Dropdown: a 32 px select-looking trigger and an anchored list (Popup).
 * The open state belongs to the caller: `open`, `onOpenChange(open, cx)`.
 */
export function Dropdown(cx, id, opts) {
  const t = tokens(cx);
  const current = opts.options.find((o) => o.value === opts.value);
  const trigger = div()
    .id(`${id}-trigger`)
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .h(SIZE.control)
    .pl(10)
    .pr(SPACE[2])
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.borderStrong)
    .bg(t.surface)
    .cursor_pointer()
    .accessibility_label(opts.label || id)
    .on_click((_e, cx) => opts.onOpenChange(!opts.open, cx))
    .child(Line(cx, "body", current ? current.label : opts.placeholder || "Choose").flex_1())
    .child(Icon(cx, "chevron-down", { size: SIZE.iconSmall, color: "fg3" }));
  if (opts.width) trigger.w(opts.width);
  const popup = Popup.new(id, trigger).anchor("top_left");
  if (opts.open) {
    const list = div()
      .id(`${id}-list`)
      .flex()
      .flex_col()
      .gap(2)
      .mt(SPACE[1])
      .p(SPACE[1])
      .min_w(opts.width || 120)
      .max_h(280)
      .overflow_y_scroll()
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.raised)
      .shadow_lg()
      .on_mouse_down_out((_e, cx) => opts.onOpenChange(false, cx));
    for (const option of opts.options) {
      const on = option.value === opts.value;
      list.child(
        div()
          .id(`${id}-opt-${option.value}`)
          .flex()
          .items_center()
          .h(SIZE.controlSm)
          .flex_shrink(0)
          .px(SPACE[2])
          .rounded(RADIUS.ctl)
          .cursor_pointer()
          .when(on, (el) => el.bg(t.selected))
          .hover((s) => s.bg(t.hover))
          .on_click((_e, cx) => {
            opts.onChange(option.value, cx);
            opts.onOpenChange(false, cx);
          })
          .child(Text(cx, "body", option.label)),
      );
    }
    popup.content(list);
  }
  return popup;
}

// ---------------------------------------------------------------- field

/**
 * Field: label 12 fg-2 over a 13 px value; in edit mode the caller passes an
 * `input` element (a TextField) instead. `inherited` shows the value in fg-2
 * with a "Head office" chip; `inheritSwitch` (edit mode) replaces the chip.
 */
export function Field(cx, label, opts = {}) {
  const t = tokens(cx);
  const head = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .min_h(20)
    .mb(SPACE[1])
    .child(Text(cx, "meta", label, "fg2"));
  if (opts.inheritSwitch) {
    head.child(div().flex_1()).child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(6)
        .child(Text(cx, "cap", "Same as head office", "fg3"))
        .child(opts.inheritSwitch),
    );
  } else if (opts.inherited) {
    head.child(div().flex_1()).child(Chip(cx, "Head office", "neutral", "git-branch"));
  }
  const body = div().min_w(0);
  if (opts.input) {
    body.child(opts.input);
    if (opts.error) body.child(Text(cx, "meta", opts.error, "danger").mt(SPACE[1]));
  } else {
    const empty = opts.value === undefined || opts.value === null || opts.value === "";
    body.child(
      Text(cx, "body", empty ? "Not set" : opts.value, empty ? "fg3" : opts.inherited ? "fg2" : "fg").whitespace_normal(),
    );
  }
  const field = div().flex().flex_col().min_w(0).child(head).child(body);
  if (opts.width) field.w(opts.width);
  return field;
}

/**
 * A two-column field grid with 16 × 32 gaps (one column when `width` is
 * under 640 − padding). Cells get pixel widths computed from `width`, per
 * the layout lesson: no flex-grown text cells.
 */
export function FieldGrid(cx, width, cells) {
  const oneCol = width < 420;
  const colW = oneCol ? width : Math.floor((width - 32) / 2);
  const grid = div().flex().flex_row().flex_wrap().gap_x(32).gap_y(16).w(width);
  for (const cell of cells) {
    grid.child(div().w(cell.full || oneCol ? width : colW).min_w(0).child(cell.el));
  }
  return grid;
}

// ---------------------------------------------------------------- disclosure

/** DisclosureGroup: one bordered group (radius 12), hairlines between sections. */
export function DisclosureGroup(cx, id, sections) {
  const t = tokens(cx);
  const group = div()
    .id(id)
    .flex()
    .flex_col()
    .w_full()
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .overflow_hidden();
  sections.forEach((section, i) => {
    if (i > 0) section.border_t(1).border_color(t.border);
    group.child(section);
  });
  return group;
}

/**
 * Disclosure: 44 px header (16 px chevron that turns down when open, title
 * 13/600, meta 12 fg-3, trailing ghost Edit while open), body padded
 * 4 / 20 / 20 / 36 so content lines up with the title text.
 */
export function Disclosure(cx, id, opts) {
  const head = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .pr(SPACE[3])
    .child(
      div()
        .id(`${id}-toggle`)
        .flex()
        .flex_row()
        .flex_1()
        .min_w(0)
        .items_center()
        .gap(SPACE[2])
        .h(SIZE.disclosureHead)
        .pl(SPACE[3])
        .pr(SPACE[2])
        .cursor_pointer()
        .accessibility_label(opts.title)
        .on_click((e, cx) => opts.onToggle(e, cx))
        .child(Icon(cx, opts.open ? "chevron-down" : "chevron-right", { color: "fg3" }))
        .child(Text(cx, "control", opts.title).font_weight(600).flex_none())
        .child(Line(cx, "meta", opts.meta || "", "fg3").flex_1()),
    );
  if (opts.open && opts.onEdit && !opts.editing) {
    head.child(AppButton(cx, `${id}-edit`, "Edit", { variant: "ghost", size: "sm", icon: "pencil", onClick: opts.onEdit }));
  }
  const section = div().flex().flex_col().w_full().child(head);
  if (opts.open) {
    section.child(div().pt(SPACE[1]).pr(SPACE[5]).pb(SPACE[5]).pl(36).child(opts.body));
  }
  return section;
}

/** Cancel (ghost) and a primary save at the bottom right of a section. */
export function FormActions(cx, id, onCancel, onSave, saveLabel = "Save changes") {
  return div()
    .flex()
    .flex_row()
    .justify_end()
    .gap(SPACE[2])
    .mt(SPACE[5])
    .child(AppButton(cx, `${id}-cancel`, "Cancel", { variant: "ghost", size: "sm", onClick: onCancel }))
    .child(AppButton(cx, `${id}-save`, saveLabel, { variant: "primary", size: "sm", onClick: onSave }));
}

// ---------------------------------------------------------------- lists

/** A bordered list (radius 12) whose rows are divided by hairlines. */
export function ListGroup(cx, id, rows) {
  const t = tokens(cx);
  const list = div()
    .id(id)
    .flex()
    .flex_col()
    .w_full()
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .overflow_hidden();
  rows.forEach((row, i) => {
    if (i > 0) row.border_t(1).border_color(t.border);
    list.child(row);
  });
  return list;
}

/** A list row: at least 52 tall, padding 8 × 16, items centred, gap 12. */
export function ListRow(cx, id, opts = {}) {
  const t = tokens(cx);
  const row = div()
    .id(id)
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[3])
    .min_h(SIZE.listRow)
    .py(SPACE[2])
    .px(SPACE[4])
    .w_full();
  if (opts.onClick) {
    row.cursor_pointer().hover((s) => s.bg(t.hover)).on_click((e, cx) => opts.onClick(e, cx));
  }
  return row;
}

/** Two-line name cell: 13/500 title over a 12 fg-3 line, both truncating. */
export function NameCell(cx, title, sub, opts = {}) {
  const cell = div()
    .flex()
    .flex_col()
    .min_w(0)
    .child(Line(cx, "control", title, opts.dim ? "fg3" : "fg"));
  if (sub) cell.child(Line(cx, "meta", sub, "fg3"));
  if (opts.width) cell.w(opts.width);
  else cell.flex_1();
  return cell;
}

/** Centred empty-state text inside a list, with an optional action. */
export function EmptyRow(cx, id, text, action) {
  const el = div()
    .id(id)
    .flex()
    .flex_col()
    .items_center()
    .gap(SPACE[3])
    .py(40)
    .px(SPACE[4])
    .child(Text(cx, "body", text, "fg3").text_center());
  if (action) el.child(action);
  return el;
}

/** A settings-style row: title + hint on the left, a control on the right. */
export function SettingRow(cx, id, title, hint, control) {
  return div()
    .id(id)
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[4])
    .py(14)
    .child(NameCell(cx, title, hint))
    .child(control || div());
}

/** A bordered card that stacks SettingRows with hairlines between them. */
export function Card(cx, id, rows) {
  const t = tokens(cx);
  const card = div().id(id).flex().flex_col().px(SPACE[4]).py(SPACE[1]).rounded(RADIUS.card).border(1).border_color(t.border);
  rows.forEach((row, i) => {
    if (i > 0) row.border_t(1).border_color(t.border);
    card.child(row);
  });
  return card;
}

/** Section heading: 16/600 with the spec's 32 / 12 margins. */
export function SectionTitle(cx, label, first) {
  return Text(cx, "title", label).mt(first ? 0 : SPACE[8]).mb(SPACE[3]);
}

/** Banner on accent-soft: radius 12, padding 12 × 16 (Add year band). */
export function Banner(cx, id, children) {
  const t = tokens(cx);
  const band = div()
    .id(id)
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[3])
    .py(SPACE[3])
    .px(SPACE[4])
    .mb(SPACE[4])
    .rounded(RADIUS.card)
    .bg(t.accentSoft);
  for (const c of children) band.child(c);
  return band;
}

// ---------------------------------------------------------------- page

/**
 * Page header: optional leading avatar, a 20/600 title, a lede or meta line,
 * and trailing actions.
 */
export function PageHeader(cx, opts) {
  const text = div().flex().flex_col().flex_1().min_w(0).child(PageTitle(cx, opts.title));
  if (opts.meta) text.child(div().mt(2).child(opts.meta));
  if (opts.lede) text.child(Text(cx, "body", opts.lede, "fg2").mt(6).whitespace_normal());
  const row = div().flex().flex_row().items_center().gap(14).w_full();
  if (opts.avatar) row.child(opts.avatar);
  row.child(text);
  if (opts.actions && opts.actions.length) {
    const actions = div().flex().flex_row().flex_none().gap(SPACE[2]);
    for (const a of opts.actions) actions.child(a);
    row.child(actions);
  }
  return row;
}

/** Meta line items separated by 3 px dots. */
export function MetaLine(cx, parts) {
  const t = tokens(cx);
  const row = div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]);
  parts.filter(Boolean).forEach((p, i) => {
    if (i > 0) row.child(div().size(3).rounded(RADIUS.full).bg(t.fg3));
    row.child(typeof p === "string" ? Text(cx, "meta", p, "fg2") : p);
  });
  return row;
}

/**
 * Underlined tabs (Profile, Forms, Templates): 36 tall, 2 px bottom border on
 * the current tab, optional count pill.
 */
export function Tabs(cx, id, tabs, current, onSelect) {
  const t = tokens(cx);
  const bar = div()
    .id(id)
    .flex()
    .flex_row()
    .gap(SPACE[1])
    .w_full()
    .mt(SPACE[5])
    .mb(SPACE[6])
    .border_b(1)
    .border_color(t.border);
  for (const tab of tabs) {
    const on = tab.value === current;
    const el = div()
      .id(`${id}-${tab.value}`)
      .flex()
      .flex_row()
      .items_center()
      .gap(6)
      .h(36)
      .px(10)
      .border_b(2)
      .border_color(on ? t.fg : "#00000000")
      .cursor_pointer()
      .accessibility_label(tab.label)
      .on_click((_e, cx) => onSelect(tab.value, cx))
      .child(Text(cx, "control", tab.label, on ? "fg" : "fg2"));
    if (tab.count !== undefined && tab.count !== null && tab.count !== "") {
      el.child(div().px(6).rounded(RADIUS.full).bg(t.hover).child(Text(cx, "cap", tab.count, "fg2")));
    }
    bar.child(el);
  }
  return bar;
}

// ---------------------------------------------------------------- panels

/**
 * SidePanel: 320 wide on `bg`, a 40 px header with a 14/600 title and at
 * most one ghost action, no close button (the trigger closes it). The body
 * scrolls vertically only.
 */
export function SidePanel(cx, id, opts) {
  const t = tokens(cx);
  const panel = div()
    .id(id)
    .flex()
    .flex_col()
    .flex_none()
    .w(SIZE.panel)
    .h_full()
    .min_h(0)
    .bg(t.bg);
  if (opts.title) {
    const head = div()
      .flex()
      .flex_row()
      .flex_none()
      .items_center()
      .gap(SPACE[2])
      .h(SIZE.titleBar)
      .pl(SPACE[5])
      .pr(SPACE[3])
      .child(Text(cx, "emph", opts.title).flex_1());
    if (opts.action) head.child(opts.action);
    panel.child(head);
  }
  // Padding sits on the content, not on the scroll box, exactly as in the
  // sidebar: a scroll box's own padding counts toward its scroll range, and
  // the panels scrolled when their content fit (Notifications with two rows).
  panel.child(
    div()
      .id(`${id}-scroll`)
      .flex_1()
      .min_h(0)
      .overflow_y_scroll()
      .overflow_x_hidden()
      .child(
        div()
          .flex()
          .flex_col()
          .gap(2)
          .w(SIZE.panel)
          .px(SPACE[3])
          .pb(SPACE[3])
          .pt(opts.title ? 0 : SPACE[1])
          .child(opts.body),
      ),
  );
  return panel;
}

/** Bottom-centre toast on `primary`, 20 px above the window edge. */
export function Toast(cx, message, icon = "circle-check") {
  const t = tokens(cx);
  return div()
    .id("toast")
    .absolute()
    .bottom(20)
    .left(0)
    .right(0)
    .flex()
    .flex_row()
    .justify_center()
    .child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(10)
        .max_w(560)
        .py(10)
        .px(14)
        .rounded(RADIUS.card)
        .bg(t.primary)
        .shadow_lg()
        .child(Icon(cx, icon, { color: "primaryFg" }))
        .child(Text(cx, "control", message, "primaryFg").flex_1().min_w(0).whitespace_normal()),
    );
}

/** A full-window scrim with a dialog card (radius 14) near the top. */
export function Overlay(cx, id, card, onDismiss) {
  const t = tokens(cx);
  return div()
    .id(id)
    .absolute()
    .top(0)
    .left(0)
    .right(0)
    .bottom(0)
    .flex()
    .flex_col()
    .items_center()
    .pt(64)
    .px(SPACE[4])
    .pb(SPACE[4])
    .bg(t.scrim)
    .on_mouse_down("left", (_e, cx) => onDismiss(cx))
    .child(
      div()
        .id(`${id}-card`)
        .flex()
        .flex_col()
        .w_full()
        .max_w(560)
        .max_h_full()
        .rounded(RADIUS.dialog)
        .border(1)
        .border_color(t.border)
        .bg(t.surface)
        .shadow_xl()
        .overflow_hidden()
        .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
        .child(card),
    );
}

/** Hairline used between list rows and disclosure sections only. */
export function Hairline(cx) {
  return div().w_full().h(1).flex_none().bg(tokens(cx).border);
}

/** PIN / one-time-code cells: 32 px boxes, radius 8, accent when active. */
export function PinCells(cx, state, id) {
  const t = tokens(cx);
  return OtpInput.new(state)
    .id(id)
    .flex()
    .gap(6)
    .cell_style((cell) =>
      cell
        .size(SIZE.control)
        .flex()
        .items_center()
        .justify_center()
        .border(1)
        .border_color(t.borderStrong)
        .bg(t.surface)
        .rounded(RADIUS.row),
    )
    .cell_active_style((cell) => cell.border_color(t.accent))
    .caret_style((caret) => caret.w(2).h(16).bg(t.fg));
}

/**
 * Digits typed in printed-form groups: 3 · 3 · 3 for a TIN, 5 for a branch
 * code. Each group is an `OtpState`; a full group hands the keyboard to the
 * next one, so typing runs straight through (scripts cannot focus a text
 * Input, but they can focus a code). Digits past a group's length, as from a
 * paste, flow on into the groups after it.
 */
export class DigitEntry {
  constructor(sizes, value = "") {
    this.sizes = sizes;
    /** Called after any edit, for screens that react to the digits. @type {((cx: any) => void) | null} */
    this.onChange = null;
    this.groups = sizes.map((n) => OtpState.new(n));
    this.set(value);
    this.groups.forEach((g, i) => {
      g.on("complete", () => {
        const next = this.groups[i + 1];
        if (next && next.value().length < next.len()) next.focus();
      });
      g.on("change", (_e, cx) => {
        const v = g.value().replace(/\D/g, "");
        if (v.length > g.len()) this.set(this.value().slice(0, this.offset(i)) + v + this.rest(i + 1));
        if (this.onChange) this.onChange(cx);
      });
    });
  }

  /** Index of the first digit of group `i`. */
  offset(i) {
    return this.sizes.slice(0, i).reduce((a, b) => a + b, 0);
  }

  /** The digits of groups `i` onward. */
  rest(i) {
    return this.groups.slice(i).map((g) => g.value()).join("");
  }

  value() {
    return this.groups.map((g) => g.value().replace(/\D/g, "").slice(0, g.len())).join("");
  }

  isComplete() {
    return this.value().length === this.sizes.reduce((a, b) => a + b, 0);
  }

  set(digits) {
    const d = String(digits || "").replace(/\D/g, "");
    let at = 0;
    this.groups.forEach((g, i) => {
      g.set_value(d.slice(at, at + this.sizes[i]));
      at += this.sizes[i];
    });
  }

  /** Keyboard onto the first group that is not full. */
  focus() {
    (this.groups.find((g) => g.value().length < g.len()) || this.groups[0]).focus();
  }
}

/** The cells of a `DigitEntry`, groups separated by a dash like the printed TIN. */
export function DigitCells(cx, entry, id, opts = {}) {
  const t = tokens(cx);
  const row = div().id(id).flex().flex_row().flex_wrap().items_center().gap(6);
  entry.groups.forEach((g, i) => {
    if (i) row.child(Text(cx, "body", "-", "fg3"));
    row.child(
      OtpInput.new(g)
        .id(`${id}-${i}`)
        .flex()
        .gap(3)
        .cell_style((cell) =>
          cell
            .w(opts.cell || 24)
            .h(SIZE.control)
            .flex()
            .items_center()
            .justify_center()
            .border(1)
            .border_color(opts.error ? t.danger : t.borderStrong)
            .bg(t.surface)
            .rounded(RADIUS.ctl),
        )
        .cell_active_style((cell) => cell.border_color(t.accent))
        .caret_style((caret) => caret.w(2).h(14).bg(t.fg)),
    );
  });
  return row;
}
