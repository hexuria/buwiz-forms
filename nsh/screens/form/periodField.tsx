// The PR 3 controls: PeriodField (one MM/YYYY box with a month picker for a host one_periods pair) and
// RangeField (two date controls in one row for a host date_ranges pair). Both are dumb about the draft:
// every act goes through the session (edit/pickPeriodMonth/edit on the end's key), locks read the part
// boxes' own BoxState, and a refusal sentence arrives on the period's month box / the To end's box.
import type { Accessor } from "nsh";
import { effect, signal, untracked, View } from "nsh";
import { isoDate, MONTHS_LONG } from "../../lib/dates";
import { DateField, DateTouchField, MonthPicker, Text, TextField } from "../../ui";
import type { DateRange, OneDate, OnePeriod, RangeEnd } from "./schema";
import type { FormSession } from "./session";
import { parsePeriod, periodLong, periodText, periodYear4 } from "./periodRules";

export interface PeriodFieldProps {
  s: FormSession;
  period: OnePeriod;
  /** The row's label, for the box's accessible name. */
  label: string;
  narrow: Accessor<boolean>;
  /** The refusal sentence the row resolved for the period's month box ("" none). */
  message: string;
  invalid: boolean;
}

/** "December 2023" for a "12/2023" text ("" when it is no period). */
function longPeriod(text: string): string {
  const p = parsePeriod(text);
  return p ? `${MONTHS_LONG[p.m - 1]} ${p.yDigits <= 2 ? 2000 + p.y : p.y}` : "";
}

/**
 * One month+year pair as a single MM/YYYY field: typing takes 12/2023, 12-2023, 122023 or 12 2023; leaving
 * the box shows "December 2023"; the calendar button opens the month grid. A locked part draws as plain
 * text — a locked year still offers the month picker beside it, a locked month and year the long text.
 */
export function PeriodField(props: PeriodFieldProps) {
  const s = props.s;
  const period = props.period;
  const key = period.periodKey;
  const box = s.box(key);
  const monthBox = s.box(period.month.k);
  const yearBox = s.box(period.year.k);
  const monthLocked = () => monthBox.locked();
  const yearLocked = () => yearBox.locked();
  const focused = signal(false);
  const text = signal(untracked(() => box.value()));
  const long = (v: string) => longPeriod(v) || v;
  // The box's text follows the draft (a pick, a host write, a refused-then-fixed edit), like DateField.
  effect(() => {
    const v = box.value();
    const now = untracked(text);
    const a = parsePeriod(now);
    const b = parsePeriod(v);
    const same = a && b ? a.m === b.m && (a.yDigits <= 2 ? 2000 + a.y : a.y) === (b.yDigits <= 2 ? 2000 + b.y : b.y) : now === v;
    if (!same) text.set(untracked(focused) ? v : long(v));
  });
  const monthNum = () => {
    const p = parsePeriod(box.value() || periodText(String(monthBox.value() || ""), String(yearBox.value() || "")));
    return p ? p.m : 0;
  };
  const yearText = () => {
    const y = periodYear4(String(yearBox.value() || ""));
    return Number.isInteger(y) && y >= 1900 ? String(y) : String(yearBox.value() || "");
  };
  const pickerYear = () => {
    const typed = parsePeriod(box.value());
    const typedYear = typed ? periodYear4(String(typed.y)) : Number.NaN;
    if (Number.isInteger(typedYear) && typedYear >= 1900 && typedYear <= 9999) return typedYear;
    const storedYear = periodYear4(String(yearBox.value() || ""));
    return Number.isInteger(storedYear) && storedYear >= 1900 && storedYear <= 9999 ? storedYear : new Date().getFullYear();
  };
  const calendarOnly = () => s.calendarYearEnd({ month: period.month, day: null, year: period.year });
  const picker = (id: string) => (
    <MonthPicker
      month={monthNum()}
      year={pickerYear()}
      // A locked year leaves the stepper nothing to reach; a calendar year leaves the grid December only.
      minYear={yearLocked() ? s.periodYear(period) : undefined}
      maxYear={yearLocked() ? s.periodYear(period) : undefined}
      allowed={calendarOnly() ? (m) => m === 12 : undefined}
      label={`Pick the month for ${props.label}`}
      testID={id}
      disabled={box.locked() || !s.header().editable}
      onPick={(m, y) => s.pickPeriodMonth(period, m, y)}
    />
  );
  // Locked month and year: the period as plain text with the lock mark.
  if (monthLocked() && yearLocked()) {
    const shown = periodLong(String(monthBox.value() || ""), String(yearBox.value() || "")) || box.value();
    return (
      <View class="gap-1">
        <TextField accessibilityLabel={props.label} locked value={shown} onChange={() => {}} width={200} testID={`fp-${key}`} />
        <If when={props.message}>
          <Text variant="meta" tone="danger">{props.message}</Text>
        </If>
      </View>
    );
  }
  // A locked year: the month alone is picked (typed 1-12 or the grid), the year shows as plain text.
  if (yearLocked() && !monthLocked()) {
    return (
      <View class="flex-row flex-wrap items-end gap-x-2 gap-y-1">
        <View class="gap-1">
          <TextField
            accessibilityLabel={`${props.label} month`}
            value={String(monthNum() || "")}
            placeholder="MM"
            width={props.narrow() ? undefined : 72}
            tabular
            error={props.invalid || undefined}
            onChange={(v) => {
              const m = Number(v.replace(/\D/g, "").slice(0, 2));
              if (m >= 1 && m <= 12) s.pickPeriodMonth(period, m);
            }}
            onFocus={() => s.focus(key)}
            onBlur={() => s.blur(key)}
            testID={`fp-${key}`}
          />
        </View>
        {picker(`${key}-calendar`)}
        <Text variant="body" tone="fg2" class="pb-2 tabular-nums">{yearText()}</Text>
        <If when={props.message}>
          <Text variant="meta" tone="danger" class="w-full">{props.message}</Text>
        </If>
      </View>
    );
  }
  // Open (or a locked month with an open year): the whole MM/YYYY field with its month grid.
  return (
    <View class="gap-1">
      <View class="flex-row items-center gap-1">
        <View class="min-w-0 flex-1">
          <TextField
            accessibilityLabel={props.label}
            value={text()}
            error={props.invalid || undefined}
            placeholder="MM/YYYY"
            width={props.narrow() ? undefined : 200}
            tabular
            trailing={picker(`${key}-calendar`)}
            onChange={(raw) => {
              text.set(raw);
              s.edit(key, raw);
            }}
            onFocus={() => {
              focused.set(true);
              const p = parsePeriod(text());
              if (p) text.set(`${String(p.m).padStart(2, "0")}/${p.yDigits <= 2 ? 2000 + p.y : p.y}`);
              s.focus(key);
            }}
            onBlur={() => {
              focused.set(false);
              const l = longPeriod(text());
              if (l) text.set(l);
              s.blur(key);
            }}
            testID={`fp-${key}`}
          />
        </View>
      </View>
      <If when={props.message}>
        <Text variant="meta" tone="danger">{props.message}</Text>
      </If>
    </View>
  );
}

export interface RangeFieldProps {
  s: FormSession;
  range: DateRange;
  /** The item's label, for the ends' accessible names. */
  label: string;
  narrow: Accessor<boolean>;
}

/** One from/to end as a date control: a one-date triple's box, or the single box (a text box edits as a date). */
function RangeEndControl(props: { s: FormSession; end: RangeEnd; label: string; narrow: Accessor<boolean>; min?: string; max?: string; start: string }) {
  const s = props.s;
  const end = props.end;
  const key = end.editKey;
  const box = s.box(key);
  if (end.date) return <RangeOneDate s={s} date={end.date} label={props.label} narrow={props.narrow} min={props.min} max={props.max} start={props.start} />;
  if (props.narrow()) {
    return (
      <DateTouchField
        testID={`fp-${key}`}
        label={props.label}
        value={box.value()}
        error={box.invalid()}
        start={props.start}
        min={props.min}
        max={props.max}
        onChange={(t) => s.edit(key, t)}
        onFocus={() => s.focus(key)}
        onBlur={() => s.blur(key)}
      />
    );
  }
  return (
    <DateField
      testID={`fp-${key}`}
      accessibilityLabel={props.label}
      value={box.value()}
      error={box.invalid()}
      min={props.min}
      max={props.max}
      calendarStart={props.start || undefined}
      onChange={(t) => s.edit(key, t)}
      onFocus={() => s.focus(key)}
      onBlur={() => s.blur(key)}
      onPick={(iso) => { s.edit(key, iso); s.blur(key); }}
      inputRef={(ref) => s.registerFocus(key, () => ref.focus())}
    />
  );
}

function RangeOneDate(props: { s: FormSession; date: OneDate; label: string; narrow: Accessor<boolean>; min?: string; max?: string; start: string }) {
  const s = props.s;
  const date = props.date;
  const key = date.oneKey;
  const box = s.box(key);
  if (props.narrow()) {
    return (
      <DateTouchField
        testID={`fp-${key}`}
        label={props.label}
        value={box.value()}
        error={box.invalid()}
        start={props.start}
        min={props.min}
        max={props.max}
        validate={(v) => s.oneDateProblem(date, v)}
        onChange={(t) => s.edit(key, t)}
        onFocus={() => s.focus(key)}
        onBlur={() => s.blur(key)}
      />
    );
  }
  return (
    <DateField
      testID={`fp-${key}`}
      accessibilityLabel={props.label}
      value={box.value()}
      error={box.invalid()}
      min={props.min}
      max={props.max}
      onChange={(t) => s.edit(key, t)}
      onFocus={() => s.focus(key)}
      onBlur={() => s.blur(key)}
      onPick={(iso) => { s.edit(key, iso); s.blur(key); }}
      inputRef={(ref) => s.registerFocus(key, () => ref.focus())}
    />
  );
}

/**
 * A from/to pair as one row: two date controls with a plain "to" between. Each keeps its own validation
 * sentence; the To end goes red with "the end date is before the start date" while the order is wrong, and
 * each calendar is bounded by the other end. Under 600 pt the ends wrap to two full-width touch rows.
 */
export function RangeField(props: RangeFieldProps) {
  const s = props.s;
  const range = props.range;
  const fromDay = () => s.rangeEndDay(range.from);
  const toDay = () => s.rangeEndDay(range.to);
  const end = (e: RangeEnd, which: "From" | "To") => (
    <View class="min-w-0 gap-1" style={props.narrow() ? undefined : { flex: "1 1 140px" }}>
      <If when={props.narrow()}>
        <Text variant="meta" tone="fg2">{which}</Text>
      </If>
      <RangeEndControl
        s={s}
        end={e}
        label={`${props.label} ${which}`}
        narrow={props.narrow}
        min={which === "To" && !range.derived ? fromDay() || undefined : undefined}
        max={which === "From" && !range.derived ? toDay() || undefined : undefined}
        start={(which === "To" ? toDay() : fromDay()) || (which === "To" ? fromDay() : toDay()) || isoDate(new Date())}
      />
    </View>
  );
  return (
    <View class="flex-row flex-wrap items-end gap-x-3 gap-y-2">
      {end(range.from, "From")}
      <Text variant="body" tone="fg3" class="pb-2">to</Text>
      {end(range.to, "To")}
    </View>
  );
}
