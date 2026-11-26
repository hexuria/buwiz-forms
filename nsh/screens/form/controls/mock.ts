// Stand-ins for what the form session gives the controls: a box's signals, a field's props, a schedule's props. For the
// gallery and the tests only (nothing in the page uses this file). They follow the session's own rules where a control relies
// on them: typing keeps the text as typed, leaving an amount groups it, leaving a date writes it long, a pick commits at once.
import type { Accessor, Signal } from "nsh";
import { signal, windowSize } from "nsh";
import { formatLongDate, formatMoney, parseTypedDate, TOUCH } from "../../../ui";
import { BREAK } from "../../../theme/tokens";
import type { BoxState, ChoiceOption, RadioGroupProps, ScheduleProps, SchemaField, SchemaItem } from "../contract";
import type { FieldInput } from "./types";

/** What a box was told, per key: the last text it reported, how many blurs and focuses it reported. */
export function reports() {
  const edits = signal<Record<string, string>>({});
  const blurs = signal<Record<string, number>>({});
  const focuses = signal<Record<string, number>>({});
  return {
    edits,
    blurs,
    focuses,
    edit: (k: string, text: string) => edits.set({ ...edits(), [k]: text }),
    blur: (k: string) => blurs.set({ ...blurs(), [k]: (blurs()[k] ?? 0) + 1 }),
    focus: (k: string) => focuses.set({ ...focuses(), [k]: (focuses()[k] ?? 0) + 1 }),
  };
}
export type Reports = ReturnType<typeof reports>;

/** The phone layout, as the page decides it: every touch screen, and a desktop window under 600 points. */
export const narrowNow: Accessor<boolean> = () => TOUCH || windowSize().width < BREAK.phone;

/** A box's state as its own signals. */
export interface MockBox {
  state: BoxState;
  value: Signal<string>;
  locked: Signal<boolean>;
  invalid: Signal<boolean>;
}

/** What a test sets on a box. */
export interface BoxInit {
  value?: string;
  locked?: boolean;
  hint?: string;
  off?: string;
  options?: ChoiceOption[];
  invalid?: boolean;
  zeroWhenBlank?: boolean;
}

/** One box's state. */
export function mockBox(init: BoxInit = {}): MockBox {
  const value = signal(init.value ?? "");
  const locked = signal(init.locked ?? false);
  const invalid = signal(init.invalid ?? false);
  const hint = signal(init.hint ?? "");
  const off = signal(init.off ?? "");
  const options = signal(init.options ?? []);
  return { value, locked, invalid, state: { value, locked, hint, off, options, invalid, zeroWhenBlank: !!init.zeroWhenBlank } };
}

/** What the session shows for a box after typing in it ends. */
function shownAfterBlur(f: SchemaField, raw: string): string {
  if (f.t === "money") return formatMoney(raw, !!f.w);
  if (f.t === "date") {
    const d = parseTypedDate(raw);
    return d ? formatLongDate(d) : raw;
  }
  return raw;
}

/** The props of one box, reporting into `log`. */
export function mockField(log: Reports, f: SchemaField, init: BoxInit = {}, over: Partial<FieldInput> = {}): FieldInput & { mock: MockBox } {
  const mock = mockBox(init);
  return {
    field: f,
    label: f.l,
    testID: `fp-${f.k}`,
    box: mock.state,
    width: 230,
    narrow: narrowNow,
    onEdit: (text) => {
      log.edit(f.k, text);
      mock.value.set(text);
    },
    onFocus: () => log.focus(f.k),
    onBlur: () => {
      log.blur(f.k);
      if (!mock.locked()) mock.value.set(shownAfterBlur(f, mock.value()));
    },
    mock,
    ...over,
  };
}

/** A radio group of the given choices, one ticked at most; a pick reports and ticks (a pair keeps one answer, a group of three or more unticks). `required` and `room` stand in for the page's optionality and row width. */
export function mockRadios(log: Reports, name: string, labels: string[], init: { selected?: string | null; locked?: boolean; hint?: string; required?: boolean; room?: number } = {}): RadioGroupProps & { selectedSignal: Signal<string | null> } {
  const keys = labels.map((_l, i) => `${name}${i + 1}`);
  const selected = signal<string | null>(init.selected ?? null);
  const locked = signal(init.locked ?? false);
  return {
    label: name,
    testID: `fp-radio-${name}`,
    choices: keys.map((key, i) => ({ key, label: labels[i], testID: `fp-${key}` })),
    selected,
    selectedSignal: selected,
    locked,
    choiceLocked: () => false,
    hint: () => (locked() && selected() === null ? init.hint ?? "" : ""),
    off: () => "",
    narrow: narrowNow,
    room: init.room === undefined ? undefined : () => init.room ?? 0,
    clearable: () => keys.length >= 3 && !init.required && !locked(),
    onPick: (key) => {
      log.edit(key, selected() === key && keys.length >= 3 ? "false" : "true");
      selected.set(selected() === key && keys.length >= 3 ? null : key);
    },
  };
}

/** A schedule table's props (the page's scheduleProps, over local signals): every row of its item, a row opens in the drawer. */
export function mockSchedule(log: Reports, item: SchemaItem, init: Record<string, string> = {}): ScheduleProps & { cleared: Accessor<number[]>; mock: Record<string, MockBox> } {
  const keys = item.keys ?? [];
  const cols = item.cols ?? [];
  const columns = cols.map((col, index) => ({ index, col })).filter((c) => c.col.t !== "hidden");
  const boxes: Record<string, MockBox> = {};
  for (const row of keys) for (const k of row) boxes[k] = mockBox({ value: init[k] ?? "" });
  const openRow = signal<number | null>(null);
  const cleared = signal<number[]>([]);
  const rowName = (r: number) => String((item.start ?? 1) + r);
  const summary = (r: number) => () =>
    columns
      .map((c) => {
        const v = boxes[keys[r][c.index]].value();
        if (!v || v === "0.00" || v === "false") return "";
        return v === "true" ? c.col.l : v;
      })
      .filter(Boolean)
      .join(" · ");
  const filled = (r: number) => summary(r)() !== "";
  return {
    item,
    testID: `fp-table-${keys[0]?.[0] ?? "x"}`,
    narrow: narrowNow,
    columns,
    rowName,
    summary,
    shownRows: () => {
      const out: number[] = [];
      let empty = false;
      keys.forEach((_row, r) => {
        if (filled(r)) out.push(r);
        else if (!empty) {
          empty = true;
          out.push(r);
        }
      });
      return out;
    },
    cell: (r, c) => {
      const col = cols[c];
      const k = keys[r][c];
      const f: SchemaField = { k, l: col.l, t: col.t === "hidden" ? "text" : col.t, c: col.c, w: col.w, o: col.o };
      const mock = boxes[k];
      return {
        field: f,
        label: `${col.l}, row ${rowName(r)}`,
        testID: `fp-${k}`,
        box: mock.state,
        width: 230,
        narrow: narrowNow,
        onEdit: (text) => {
          log.edit(k, text);
          mock.value.set(text);
        },
        onFocus: () => log.focus(k),
        onBlur: () => {
          log.blur(k);
          mock.value.set(shownAfterBlur(f, mock.value()));
        },
      };
    },
    openRow,
    onOpenRow: (r) => openRow.set(r),
    onClearRow: (r) => {
      for (const k of keys[r]) boxes[k].value.set("");
      cleared.set([...cleared(), r]);
    },
    locked: () => false,
    cleared,
    mock: boxes,
  };
}
