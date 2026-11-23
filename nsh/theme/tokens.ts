// The non-color design tokens, ported from the old app (the old shell's ui.js: TYPE, SPACE, RADIUS, SIZE, BREAK).
// Colors are not here: they live in theme.json and screens read them as classes (bg-card, text-muted-foreground)
// or token("..."). Never write a raw color in a screen.
//
// Where a token is also a Tailwind class (nsh.json "tailwind" and theme.json), the class is the way to use it:
//   TYPE   -> text-page text-title text-emph text-body text-meta text-cap (size and line height), with font-medium etc.
//   SPACE  -> Tailwind's own 4 pt scale: p-1 is 4, p-2 is 8 ... p-8 is 32 (SPACE[n] === n * 4)
//   RADIUS -> rounded-(--radius-ctl) rounded-(--radius-row) rounded-(--radius-card) rounded-(--radius-dialog), rounded-full
//   BREAK  -> the variants wide: content: partnav: docked: (min-width, "from this width up")
// The numbers below are for style objects, layout math and tests, where a class cannot go (SIZE has no classes).

/** Type scale: points and weight. `cls` is the same thing as classes; weights 400/500/600 are font-normal/-medium/-semibold. */
export const TYPE = {
  page: { size: 20, weight: 600, cls: "text-page font-semibold" },
  title: { size: 16, weight: 600, cls: "text-title font-semibold" },
  emph: { size: 14, weight: 600, cls: "text-emph font-semibold" },
  body: { size: 13, weight: 400, cls: "text-body font-normal" },
  control: { size: 13, weight: 500, cls: "text-body font-medium" },
  meta: { size: 12, weight: 400, cls: "text-meta font-normal" },
  metaStrong: { size: 12, weight: 500, cls: "text-meta font-medium" },
  cap: { size: 11, weight: 500, cls: "text-cap font-medium" },
  num: { size: 12, weight: 600, cls: "text-meta font-semibold tabular-nums" },
} as const;

/** The 4 pt spacing grid. Index by step: SPACE[2] === 8. Same as Tailwind's p-2, gap-2, ... */
export const SPACE = { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 20, 6: 24, 7: 28, 8: 32 } as const;

/** Radius scale, in points. Classes: rounded-(--radius-ctl) etc. (theme.json), `rounded-full` for pills. */
export const RADIUS = { ctl: 6, row: 8, card: 12, dialog: 14, full: 9999 } as const;

/** Fixed sizes, in points. */
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
} as const;

/**
 * Responsive breakpoints, in points of window width (the same numbers as the old app).
 * Tailwind variants (nsh.json "tailwind" "breakpoints") mean "from this width up": wide: 600, content: 640, partnav: 760, docked: 860.
 * Read the live width with windowSize().width when logic, not a class, must branch.
 */
export const BREAK = {
  /** Under it the sidebar and the panel are overlays (docked: from here up they dock). */
  overlays: 860,
  /** Under it the pill shows avatar and branch code only, back/forward are hidden. */
  compactPill: 600,
  /** Under it content padding is 20 x 16 and field grids have one column. */
  narrowContent: 640,
  /** Under it the part navigator becomes a chip row. */
  formNav: 760,
  /** Under it: the narrow (phone) layout, with the bottom tab bar. */
  phone: 600,
} as const;
