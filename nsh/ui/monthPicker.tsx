// MonthPicker: the kit's month+year picker for a MM/YYYY field (PR 3). A calendar-icon trigger opens a
// 3x4 month grid under a previous/next year stepper; the held month is marked, and min/max years and an
// allowed-month rule bound the picks. A popover on desktop, a sheet on touch — the same split DateField uses.
import { signal, View } from "nsh";
import { MONTHS } from "../lib/dates";
import { AppButton, IconButton } from "./buttons";
import { Text } from "./text";
import { TOUCH } from "./look";
import { Popover, Sheet } from "./layers";

/** Props of MonthPicker. */
export interface MonthPickerProps {
  /** The held month, 1-12 (0 or out of range: none held). */
  month: number;
  /** The year the grid opens on (the held period's, else today's). */
  year: number;
  /** The first and last year the stepper may reach (unset: free). */
  minYear?: number;
  maxYear?: number;
  /** Whether a month may be picked (unset: every month). */
  allowed?: (month: number) => boolean;
  /** What screen readers call the trigger and the grid. */
  label: string;
  disabled?: boolean;
  /** The trigger's testID ("<field>-calendar"). */
  testID?: string;
  /** A month was picked, with the year the grid stood on. */
  onPick: (month: number, year: number) => void;
}

/** The month grid itself: a year stepper over a 3x4 grid of month buttons. */
function MonthGrid(props: Omit<MonthPickerProps, "year"> & { year: () => number; setYear: (y: number) => void; onDone?: () => void }) {
  const can = (dir: -1 | 1) => {
    const next = props.year() + dir;
    if (props.minYear !== undefined && next < props.minYear) return false;
    if (props.maxYear !== undefined && next > props.maxYear) return false;
    return true;
  };
  const pick = (m: number) => {
    props.onPick(m, props.year());
    props.onDone?.();
  };
  return (
    <View class="gap-2">
      <View class="flex-row items-center justify-between gap-1">
        <IconButton
          icon="chevron-left"
          tooltip="Previous year"
          testID={props.testID ? `${props.testID}-prev` : undefined}
          disabled={!can(-1)}
          onPress={() => props.setYear(props.year() - 1)}
        />
        <Text variant="control" class="font-semibold tabular-nums" testID={props.testID ? `${props.testID}-year` : undefined}>{String(props.year())}</Text>
        <IconButton
          icon="chevron-right"
          tooltip="Next year"
          testID={props.testID ? `${props.testID}-next` : undefined}
          disabled={!can(1)}
          onPress={() => props.setYear(props.year() + 1)}
        />
      </View>
      <View class="flex-row flex-wrap" style={{ gap: 4 }}>
        <Each of={MONTHS}>
          {(name, i) => {
            const m = () => i() + 1;
            return (
              <View style={{ flexGrow: 1, flexBasis: "30%", minWidth: 64 }}>
                <AppButton
                  label={name()}
                  size="sm"
                  block
                  selected={props.month === m()}
                  disabled={props.allowed ? !props.allowed(m()) : false}
                  accessibilityLabel={`${name()} ${props.year()}`}
                  testID={props.testID ? `${props.testID}-m${m()}` : undefined}
                  onPress={() => pick(m())}
                />
              </View>
            );
          }}
        </Each>
      </View>
      <If when={props.onDone}>
        <View class="flex-row justify-end">
          <AppButton label="Done" variant="ghost" size="sm" testID={props.testID ? `${props.testID}-done` : undefined} onPress={() => props.onDone?.()} />
        </View>
      </If>
    </View>
  );
}

/**
 * A month picker for a MM/YYYY field: the calendar-icon trigger opens the grid (a popover on desktop, a sheet on
 * touch). Picking a month calls onPick with it and the grid's year; the year steps within min/max.
 */
export function MonthPicker(props: MonthPickerProps) {
  const open = signal(false);
  const validatedYear = (value: number) => value >= 1900 && value <= 9999 ? value : new Date().getFullYear();
  const year = signal(validatedYear(props.year));
  const resetYear = () => year.set(validatedYear(props.year));
  const grid = (
    <MonthGrid
      month={props.month}
      minYear={props.minYear}
      maxYear={props.maxYear}
      allowed={props.allowed}
      label={props.label}
      disabled={props.disabled}
      testID={props.testID}
      onPick={(m, y) => props.onPick(m, y)}
      year={() => year()}
      setYear={(y) => {
        if (props.minYear !== undefined && y < props.minYear) return;
        if (props.maxYear !== undefined && y > props.maxYear) return;
        year.set(y);
      }}
      onDone={TOUCH ? () => open.set(false) : undefined}
    />
  );
  const trigger = (
    <IconButton
      icon="calendar"
      tooltip={props.label}
      testID={props.testID}
      disabled={props.disabled}
      pressed={open()}
      onPress={TOUCH ? () => { resetYear(); open.set(true); } : undefined}
    />
  );
  return (
    <If
      when={TOUCH}
      else={
        <Popover trigger={trigger} open={open()} onOpenChange={(o) => { if (o) resetYear(); open.set(o); }} placement="bottom" align="end" testID={props.testID ? `${props.testID}-pop` : undefined}>
          {grid}
        </Popover>
      }
    >
      {trigger}
      <Sheet open={open()} onOpenChange={(o) => open.set(o)} title={props.label} testID={props.testID ? `${props.testID}-sheet` : undefined}>
        {grid}
      </Sheet>
    </If>
  );
}
