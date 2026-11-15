// Parts the Calendar shares between its wide and narrow layouts: the pressable day cell, the chip and the dot a deadline
// draws in a cell, the taxpayers as choices, and the width of the content column.
import type { JSX } from "nsh";
import { View } from "nsh";
import { Button } from "nsh/base";
import type { DeadlineRow } from "../../bir";
import { app, store } from "../../shell";
import { BREAK, SIZE } from "../../theme/tokens";
import { Avatar, type ChipTone, formatTin, initials, Line, type MultiComboOption, type Tone } from "../../ui";
import { toneOf } from "./rows";

/** The width of the content column: the main pane less the frame's padding, at most the page column (old renderCalendar's width). */
export function contentWidth(): number {
  const w = app.layout.mainWidth();
  return Math.min(SIZE.contentMax, w - (w < BREAK.narrowContent ? 32 : 64));
}

/** Every taxpayer as a choice: avatar, name, TIN. */
export function taxpayerOptions(): MultiComboOption[] {
  return store.taxpayers().map((tp) => ({
    value: tp.tin_root,
    label: tp.name,
    sub: tp.tin_display || formatTin(tp.tin_root),
    lead: () => <Avatar name={tp.name} size={20} hueKey={tp.tin_root} />,
  }));
}

/** Props of DayCell. */
export interface DayCellProps {
  testID: string;
  /** What screen readers say: "November 15, 2 deadlines". */
  label: string;
  /** The chosen day: the selected fill. */
  selected: boolean;
  onPress: () => void;
  /** Layout and look: padding, borders. */
  class?: string;
  /** The least height, in points. */
  minHeight?: number;
  children?: JSX.Element;
}

/** A day of the grid: a pressable cell (hover fill, the selected fill while chosen). The kit has no grid cell, so this one lives with the Calendar. */
export function DayCell(props: DayCellProps) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      selected={props.selected}
      onPress={props.onPress}
      style={{ minHeight: props.minHeight }}
      class={`min-w-0 enabled:hover:bg-hover active:bg-selected selected:bg-selected selected:hover:bg-selected selected:focus-visible:bg-selected focus-visible:bg-hover ${props.class ?? ""}`}
    >
      {props.children}
    </Button>
  );
}

const CHIP_FILL: Record<ChipTone, string> = {
  neutral: "bg-hover",
  accent: "bg-accent-blue-soft",
  success: "bg-success-soft",
  warning: "bg-warning-soft",
  danger: "bg-danger-soft",
};
const CHIP_INK: Record<ChipTone, Tone> = { neutral: "fg2", accent: "accent", success: "success", warning: "warning", danger: "danger" };
const DOT_FILL: Record<ChipTone, string> = {
  neutral: "bg-subtle-foreground",
  accent: "bg-accent-blue",
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
};

/** A deadline in a grid cell: 18 tall, "<code> <initials>" in the tone of its state (late, soon, filed, in progress). */
export function MiniChip(props: { d: DeadlineRow }) {
  const tone = () => toneOf(props.d);
  return (
    <View class={`h-4.5 min-w-0 flex-row items-center overflow-hidden rounded-[5px] px-1.5 ${CHIP_FILL[tone()]}`}>
      <Line variant="cap" tone={CHIP_INK[tone()]} class="font-semibold">{`${props.d.form_code} ${initials(props.d.taxpayer_name)}`}</Line>
    </View>
  );
}

/** A 6-point dot in a tone (the narrow grid's mark of a due day). */
export function Dot(props: { tone: ChipTone; testID?: string }) {
  return <View testID={props.testID} class={`size-1.5 rounded-full ${DOT_FILL[props.tone]}`} />;
}
