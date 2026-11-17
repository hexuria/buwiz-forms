// Pressable surfaces the lists and grids need: PressCard, a bordered card that is one press target (a form tile, a filing, a Home
// count), and PressCell, the borderless cell (a calendar day, a day of the Home strip). Both are one nsh/base Button styled from
// theme tokens: the hover, press and selected tones of the rest of the kit (hover border or fill, the selected fill on --selected,
// the accent focus ring). A button inside one takes its own press: only the innermost pressable view responds (NSH
// touch-and-gestures.md). Layout (width, gap, flex) comes in through `class` and `style`; the look is theirs.
import type { JSX } from "nsh";
import { Button } from "nsh/base";
import { Tooltip } from "./layers";
import { cx, FOCUS_RING } from "./look";

/** The two card sizes: a tile (radius 12, 16 by 14 inside: the library's cards, Home's counts) and a row card (radius 8, 12 by 10: a filing). */
const CARD_SIZE = {
  tile: "rounded-(--radius-card) px-4 py-3.5",
  row: "rounded-(--radius-row) px-3 py-2.5",
};

/** Props of PressCard. */
export interface PressCardProps {
  /** What screen readers say of the whole card. */
  label: string;
  testID?: string;
  onPress?: () => void;
  /** The chosen card (a Home count that filters): the selected fill and the strong border. */
  selected?: boolean;
  /** No press, no hover; dimmed. */
  disabled?: boolean;
  /** tile (default) or row. */
  size?: "tile" | "row";
  /** Where the content sits across the card: stretch (default) fills its width, start keeps its natural one. */
  align?: "stretch" | "start";
  /** A soft shadow under the card while hovered (the Form library's cards). */
  lift?: boolean;
  /** A tooltip. */
  tooltip?: string;
  /** Layout only: width, shrink, gap. */
  class?: string;
  /** A width or least height in points. */
  style?: { width?: number; minHeight?: number };
  children?: JSX.Element;
}

/** A bordered surface on the card fill that is one press target: the strong border on hover, the selected fill while pressed or chosen, the keyboard ring on focus. */
export function PressCard(props: PressCardProps) {
  const cls = () =>
    cx(
      "min-w-0 flex-col justify-start border border-border bg-card",
      props.align === "start" ? "items-start" : "items-stretch",
      CARD_SIZE[props.size ?? "tile"],
      "enabled:hover:border-border-strong active:bg-selected selected:border-border-strong selected:bg-selected selected:hover:bg-selected disabled:opacity-50",
      props.lift && "enabled:hover:shadow-md",
      FOCUS_RING,
      props.class,
    );
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      selected={props.selected}
      disabled={props.disabled}
      tooltip={props.tooltip ? <Tooltip label={props.tooltip} /> : undefined}
      onPress={props.onPress}
      class={cls()}
      style={props.style}
    >
      {props.children}
    </Button>
  );
}

/** How a PressCell answers the pointer: fill (the hover fill, the keyboard focus the same fill: a grid cell) or outline (a hover border, the focus ring: a day of a strip). */
const CELL_TONE = {
  fill: "enabled:hover:bg-hover active:bg-selected selected:bg-selected selected:hover:bg-selected selected:focus-visible:bg-selected focus-visible:bg-hover",
  outline: cx(
    "border border-transparent enabled:hover:border-border active:bg-selected selected:bg-selected selected:hover:bg-selected",
    FOCUS_RING,
  ),
};

/** Props of PressCell. */
export interface PressCellProps {
  /** What screen readers say ("November 15, 2 deadlines"). */
  label: string;
  testID?: string;
  onPress?: () => void;
  /** The chosen cell: the selected fill. */
  selected?: boolean;
  disabled?: boolean;
  /** fill (default) or outline: how it answers the pointer. */
  tone?: "fill" | "outline";
  /** A tooltip. */
  tooltip?: string;
  /** Layout and look: radius, padding, borders (a grid draws its own lines), flex. */
  class?: string;
  /** A width or least height in points. */
  style?: { width?: number; minHeight?: number };
  children?: JSX.Element;
}

/** A cell that is one press target and draws nothing at rest: a calendar day, a day of the Home strip. */
export function PressCell(props: PressCellProps) {
  const cls = () => cx("min-w-0 disabled:opacity-50", CELL_TONE[props.tone ?? "fill"], props.class);
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      selected={props.selected}
      disabled={props.disabled}
      tooltip={props.tooltip ? <Tooltip label={props.tooltip} /> : undefined}
      onPress={props.onPress}
      class={cls()}
      style={props.style}
    >
      {props.children}
    </Button>
  );
}
