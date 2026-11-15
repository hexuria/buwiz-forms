// Parts the Calendar shares between its wide and narrow layouts: the chip and the dot a deadline draws in a day cell (the cell
// itself is the kit's PressCell), the taxpayers as choices, and the width of the content column.
import { View } from "nsh";
import type { DeadlineRow } from "../../bir";
import { app, contentColumn, store, today } from "../../shell";
import { Avatar, type ChipTone, formatTin, initials, Line, type MultiComboOption, type Tone } from "../../ui";
import { toneOf } from "./rows";

/** The width of the content column: the main pane less the frame's padding, at most the page column less it too (old renderCalendar's width). */
export function contentWidth(): number {
  return contentColumn("page", app.layout.mainWidth());
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
  const tone = () => toneOf(props.d, today());
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
