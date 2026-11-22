// The form page's rows: one item (number chip, label and marks, its controls from ./controls, the reserved 18-point
// message line, fact cards, Item 13's card), and a part's "From tax profile" fold. Each row reads its own RowInfo
// signal and its boxes' signals only, so an edit re-runs the rows it changed and nothing else.
import type { Accessor, JSX, Signal, ViewRef } from "nsh";
import { signal, untracked, View } from "nsh";
import { AppButton, CalendarButton, Chip, Code, DateField, DateTouchField, Dropdown, Icon, type IconName, ListRow, Segmented, Switch, Text, TextField, Tooltip } from "../../ui";
import type { FieldControlProps, RadioGroupProps, ScheduleProps, SchemaField, SchemaItem } from "./contract";
import { FieldControl, RadioGroupControl, ScheduleControl } from "./controls";
import { choiceLabel, controlWidth, dateParts, dateRanges, emptyCell, isTinItem, needsText, oneDates, onePeriods, radioGroups, tinWeights, type DateRange, type OneDate } from "./schema";
import { rowHead, tinSegment } from "./controls/model";
import { PeriodField, RangeField } from "./periodField";
import type { AskCard, ElectionCard, FormSession, RowInfo } from "./session";
import { dateRange, periodDay, splitDay } from "./dateRules";
export { dateRange, splitDay } from "./dateRules";

export interface RowProps {
  s: FormSession;
  id: string;
  it: SchemaItem;
  narrow: Accessor<boolean>;
  /** The row's view, for a step to scroll to. */
  onRef: (id: string, ref: ViewRef) => void;
  /** The first and the last row of its part's list (the rows are recycled cells, so CSS first:/last: cannot tell). */
  first?: Accessor<boolean>;
  last?: Accessor<boolean>;
}

/** The props of one box's control. */
export function fieldProps(s: FormSession, f: SchemaField, narrow: Accessor<boolean>, label = f.l, width = controlWidth(f, 320)): FieldControlProps {
  return {
    field: f,
    label,
    testID: `fp-${f.k}`,
    box: s.box(f.k),
    width,
    narrow,
    onEdit: (text) => s.edit(f.k, text),
    onFocus: () => s.focus(f.k),
    onBlur: () => s.blur(f.k),
    focusRef: (focus) => s.registerFocus(f.k, focus),
  };
}

/** The props of one radio group — `radios` is one host one-answer group of the item (see radioGroups), `id` its row id
 *  (exported for tests). `room` is the row's width in points, for the segmented bar's one-line fit; `hidden` names the
 *  choice keys G1 keeps shut. */
export function radioProps(s: FormSession, id: string, it: SchemaItem, radios: SchemaField[], narrow: Accessor<boolean>, room?: Accessor<number>, hidden?: () => readonly string[]): RadioGroupProps {
  const keys = radios.map((f) => f.k);
  const boxes = keys.map((k) => s.box(k));
  // The way back to unanswered is the session's untick: a re-pick clears a group of three or more (rg >= 3) that is not the
  // period's own part. Only an optional group (no member required) shows it.
  const untickable = radios.some((f) => (f.rg || 0) >= 3) && !radios.some((f) => s.isPeriodBox(f.k));
  const optional = !radios.some((f) => f.r);
  return {
    label: it.l,
    testID: `fp-radio-${id}`,
    choices: radios.map((f) => ({ key: f.k, label: choiceLabel(f), testID: `fp-${f.k}` })),
    selected: s.radioSelected(keys),
    checked: (k) => s.box(k).value() === "true",
    locked: () => boxes.every((b) => b.locked()),
    choiceLocked: (k) => s.box(k).locked(),
    hint: () => (boxes.every((b) => b.locked()) && !boxes.some((b) => b.value() === "true") ? boxes.map((b) => b.hint()).find(Boolean) || "" : ""),
    // A shut choice draws nothing, so its reason must not fade the choices that do draw.
    off: () => boxes.filter((_, i) => !(hidden?.() ?? []).includes(keys[i])).map((b) => b.off()).find(Boolean) || "",
    narrow,
    room,
    clearable: () => untickable && optional,
    onPick: (k) => s.pickRadio(k),
    focusRef: (focus) => s.registerFocus(`radio:${id}`, focus),
  };
}

/** A group's caption when an item draws several one-answer groups: the short marker its choices share ("A.", "B."), else
 *  none — the choice labels carry it already. */
function groupCaption(fields: SchemaField[]): string {
  const labels = fields.map((f) => choiceLabel(f));
  const dot = (labels[0] ?? "").indexOf(".");
  if (dot < 0) return "";
  const prefix = (labels[0] as string).slice(0, dot + 1).trim();
  return prefix.length > 0 && prefix.length <= 4 && labels.every((l) => l.startsWith(prefix)) ? prefix : "";
}

/** The props of a schedule table (exported for tests). */
export function scheduleProps(s: FormSession, id: string, it: SchemaItem, narrow: Accessor<boolean>): ScheduleProps {
  const keys = it.keys || [];
  const cols = it.cols || [];
  const columns = cols.map((col, index) => ({ index, col })).filter((c) => c.col.t !== "hidden");
  const numbered = !it.row_labels || it.numbered !== false;
  const rowName = (r: number) => (numbered ? String((it.start ?? 1) + r) : it.row_labels?.rows[r] || String((it.start ?? 1) + r));
  const openRow = signal<number | null>(null);
  const filled = (r: number) => columns.some((c) => !emptyCell(s.box(keys[r][c.index]).value(), c.col.t));
  return {
    item: it,
    testID: `fp-table-${keys[0]?.[0] ?? id}`,
    narrow,
    columns,
    rowName,
    summary: (r) => () =>
      columns
        .map((c) => {
          const box = s.box(keys[r][c.index]);
          const v = box.value();
          if (emptyCell(v, c.col.t)) return "";
          if (c.col.t === "sel") return box.options().find((o) => o.value === v)?.label.split(" — ")[0] ?? v;
          return v === "true" ? c.col.l : v;
        })
        .filter(Boolean)
        .join(" · "),
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
      // A table's radio column (1700 Schedule 1's Taxpayer / Spouse) is a tick box in the row's drawer; ticking one
      // unticks the row's other radio columns, as a radio does.
      const radio = col.t === "radio";
      const f: SchemaField = { k, l: col.l, t: col.t === "hidden" ? "text" : radio ? "chk" : col.t, c: col.c, w: col.w, o: col.o };
      const base = fieldProps(s, f, narrow, `${col.l}, row ${rowName(r)}`, controlWidth(f, 320));
      if (!radio) return base;
      return {
        ...base,
        onEdit: (text) => {
          const edits: Record<string, string> = { [k]: text };
          if (text === "true") cols.forEach((other, j) => other.t === "radio" && j !== c && (edits[keys[r][j]] = "false"));
          s.choose(edits);
        },
      };
    },
    openRow: () => openRow(),
    onOpenRow: (r) => {
      if (r === null) s.touch(id);
      openRow.set(r);
    },
    onClearRow: (r) => {
      const edits = Object.fromEntries(keys[r].filter((k, c) => cols[c]?.t !== "hidden" && !untracked(() => s.box(k).locked())).map((k) => [k, ""]));
      s.choose(edits);
    },
    locked: () => !s.header().editable,
  };
}

/** What a row draws for its boxes: a schedule, a note, a fold of extra fields, one date, a TIN, or its boxes. */
function OneDateBox(props: { s: FormSession; date: OneDate; label: string; narrow: Accessor<boolean>; describedBy: Accessor<ViewRef | null>; errorDescription: Accessor<string> }) {
  const s = props.s;
  const date = props.date;
  const key = date.oneKey;
  const box = s.box(key);
  const label = props.label.replace(/\s*\(MM\/DD\/YYYY\)\s*$/i, "").trim();
  const range = () => dateRange(s, date);
  const start = () => {
    const base = splitDay(s, date) || periodDay(s);
    return range().min && base < range().min! ? range().min! : range().max && base > range().max! ? range().max! : base;
  };
  const field = { k: key, l: label, t: "date" as const };
  return (
    <If
      when={!box.locked()}
      else={<FieldControl {...fieldProps(s, field, props.narrow, label, 200)} />}
    >
      <View class="gap-1">
        <If
          when={props.narrow()}
          else={
            <View class="h-8 min-w-0 overflow-hidden" style={{ width: 200 }}>
              <DateField
                testID={`fp-${key}`}
                accessibilityLabel={label}
                accessibilityDescribedBy={props.describedBy()}
                value={box.value()}
                error={box.invalid()}
                calendarStart={start()}
                min={range().min}
                max={range().max}
                onChange={(text) => s.edit(key, text)}
                onFocus={() => s.focus(key)}
                onBlur={() => s.blur(key)}
                onPick={(iso) => { s.edit(key, iso); s.blur(key); }}
                inputRef={(ref) => s.registerFocus(key, () => ref.focus())}
              />
            </View>
          }
        >
          <DateTouchField
            testID={`fp-${key}`}
            label={label}
            errorDescription={props.errorDescription()}
            value={box.value()}
            error={box.invalid()}
            start={start()}
            min={range().min}
            max={range().max}
            validate={(value) => s.oneDateProblem(date, value)}
            onChange={(text) => s.edit(key, text)}
            onFocus={() => s.focus(key)}
            onBlur={() => s.blur(key)}
          />
        </If>
        <If when={box.hint()}>
          <Text testID={`fp-${key}-hint`} variant="meta" tone="fg3">{box.hint()}</Text>
        </If>
      </View>
    </If>
  );
}

function Controls(props: { s: FormSession; id: string; it: SchemaItem; narrow: Accessor<boolean>; info: Accessor<RowInfo>; describedBy: (date: OneDate) => ViewRef | null; errorDescription: (date: OneDate) => string }) {
  const s = props.s;
  const it = props.it;
  const fs = it.f || [];
  // G1: a box another answer keeps shut is not drawn (RowInfo.hiddenBoxes); a schedule's cells always draw.
  const hid = () => props.info().hiddenBoxes;
  const shut = (keys: string[]) => keys.length > 0 && keys.every((k) => hid().includes(k));
  // A range's drawn keys: each end's anchor plus its date parts.
  const endKeys = (e: DateRange["from"]): string[] =>
    e.date ? [e.date.month.k, e.date.day?.k, e.date.year.k].filter((k): k is string => Boolean(k)) : [e.anchor];
  const rangeKeys = (r: DateRange): string[] => [...endKeys(r.from), ...endKeys(r.to)];
  if (it.rows) return <ScheduleControl {...scheduleProps(s, props.id, it, props.narrow)} />;
  if (!fs.length && it.note) return <Text variant="meta" tone="fg3" testID={`fp-note-${props.id}`}>{it.note}</Text>;
  const dates = oneDates(it);
  const periods = onePeriods(it);
  const ranges = dateRanges(it);
  // The keys a period or a range draws itself: its boxes never render on their own.
  const grouped = new Set<string>([
    ...periods.flatMap((p) => [p.month.k, p.year.k]),
    ...ranges.flatMap((r) => [
      r.from.anchor, r.to.anchor,
      ...(r.from.date ? [r.from.date.month.k, r.from.date.day?.k, r.from.date.year.k] : []),
      ...(r.to.date ? [r.to.date.month.k, r.to.date.day?.k, r.to.date.year.k] : []),
    ]),
  ] as string[]);
  if (isTinItem(it)) {
    const weights = tinWeights(fs);
    return (
      <View class="gap-1.5">
        <If
          when={props.narrow()}
          else={
            <View class="flex-row flex-wrap items-center gap-1.5">
              {fs.map((f, j) => (
                <If when={!hid().includes(f.k)}>
                  <View class="flex-row items-center gap-1.5">
                    {j ? <Text testID={`fp-tin-sep-${props.id}-${j}`} tone="fg3">-</Text> : null}
                    {/* The 76 pt box is the 5-digit branch code's (the grid's 1.4 weight); a lone whole-TIN box keeps
                        its own width (14 digits do not fit 58 pt); every other box is 58. */}
                    <FieldControl {...fieldProps(s, f, () => false, f.l, weights[j] > 1 ? 76 : fs.length === 1 ? controlWidth(f, 320) : 58)} />
                  </View>
                </If>
              ))}
            </View>
          }
        >
          {/* Under 600 pt the boxes are one grid that fills the row: equal columns, the last 1.4 only for the
              5-digit branch code, 6 pt gaps, no dashes (PR 8). */}
          <View testID={`fp-tin-grid-${props.id}`} class="flex-row gap-1.5">
            {fs.map((f, j) => (
              <If when={!hid().includes(f.k)}>
                <View class="min-w-0" style={{ flexGrow: weights[j], flexBasis: 0 }}>
                  <FieldControl {...fieldProps(s, f, props.narrow, f.l)} tinCell={tinSegment(f)} />
                </View>
              </If>
            ))}
          </View>
        </If>
        {/* A TIN the tax profile fills says so once, under the boxes, on both widths (RowInfo.tinLine: every box
            locked); a typed TIN or one open for Edit gets no line, and the row's profile mark stays for those. */}
        <If when={props.info().tinLine}>
          <View testID={`fp-tin-from-${props.id}`} class="flex-row items-center gap-1.5">
            <Icon name="lock" size={14} tone="fg3" />
            <Text variant="meta" tone="fg3">From the tax profile</Text>
          </View>
        </If>
      </View>
    );
  }
  const radios = fs.filter((f) => f.t === "radio");
  const boxes = fs.filter((f) => f.t !== "radio");
  const groups = radioGroups(radios);
  const multi = fs.length > 1;
  // The row's width in points, for a segmented bar's one-line fit (a stacked bar when its labels cannot hold it).
  const rowW = signal(0);
  // An "Others (Specify)" box — the item's one plain box beside an Others choice — sits full width under the group.
  const othersPick = radios.some((f) => /others|specify/i.test(choiceLabel(f)));
  const standalone = boxes.filter(
    (f) => !grouped.has(f.k) && !dates.some((d) => [d.month.k, d.day?.k, d.year.k].includes(f.k)) && f.t !== "chk" && f.t !== "date",
  );
  const depBox = othersPick && standalone.length === 1 && (standalone[0].t === "text" || standalone[0].t === "int") ? standalone[0] : null;
  const depSub = multi && depBox !== null && depBox.l && depBox.l !== it.l && !/^\d+[A-Z]?$/.test(depBox.l);
  // A date printed in split boxes (Year Ended MM and YYYY) gets a calendar button beside them, as the old page: a day picked
  // fills the parts that are open. Not when the item has a date box of its own (that box carries its calendar).
  const split = dates.length || periods.length || ranges.length || fs.some((f) => f.t === "date") ? null : dateParts(fs);
  const splitParts = split ? [split.month, split.day, split.year].filter((f): f is SchemaField => f !== null) : [];
  const splitOpen = () => split !== null && s.header().editable && !splitParts.every((f) => f.c || s.box(f.k).locked()) && !splitParts.every((f) => hid().includes(f.k));
  // What the boxes hold now, as the calendar's own day: the popup opens on its month.
  const splitIso = () => (split ? splitDay(s, split) : "");
  // What the calendar may offer: nothing outside the year (and month) that is locked.
  const splitRange = () => (split ? dateRange(s, split) : {});
  const body = (
    <View class="flex-row flex-wrap items-end gap-x-4 gap-y-3" onLayout={(e) => rowW.set(e.bounds.size.width)}>
      {groups.map((g, i) => {
        // The first group keeps the item's own ids ("fp-radio-<id>", "radio:<id>"); further groups of a multi-group item
        // take a "-g<i>" suffix, with a caption when their choices share a marker ("A.", "B.").
        const gid = groups.length > 1 && i > 0 ? `${props.id}-g${i}` : props.id;
        const cap = groups.length > 1 ? groupCaption(g) : "";
        // G1: a group whose every choice is shut draws nothing (its caption too).
        return (
          <If when={g.some((f) => !hid().includes(f.k))}>
            <View class="min-w-0 gap-1" style={{ width: props.narrow() ? "100%" : undefined }}>
              {cap ? <Text variant="meta" tone="fg2">{cap}</Text> : null}
              <RadioGroupControl {...radioProps(s, gid, it, g, props.narrow, () => rowW(), () => hid())} hidden={() => hid()} />
            </View>
          </If>
        );
      })}
      {depBox ? (
        <If when={!hid().includes(depBox.k)}>
          <View class="min-w-0 gap-1" style={{ width: "100%" }}>
            {depSub ? <Text variant="meta" tone="fg2">{depBox.l}</Text> : null}
            <FieldControl {...fieldProps(s, depBox, props.narrow, depBox.l, depBox.t === "text" ? 480 : controlWidth(depBox, 480))} />
          </View>
        </If>
      ) : null}
      {periods.map((p) => {
        const message = props.info().boxMessages.find((entry) => entry.key === p.month.k || entry.key === p.year.k)?.sentence || "";
        return (
          <If when={!shut([p.month.k, p.year.k])}>
            <View class="min-w-0" style={{ width: props.narrow() ? "100%" : undefined }}>
              <PeriodField s={s} period={p} label={it.l} narrow={props.narrow} message={message} invalid={s.box(p.periodKey).invalid()} />
            </View>
          </If>
        );
      })}
      {ranges.map((r) => (
        <If when={!shut(rangeKeys(r))}>
          <View class="min-w-0" style={{ width: props.narrow() ? "100%" : undefined }}>
            <RangeField s={s} range={r} label={it.l} narrow={props.narrow} />
          </View>
        </If>
      ))}
      {boxes.map((f) => {
        if (depBox !== null && f.k === depBox.k) return null;
        if (grouped.has(f.k)) return null;
        const date = dates.find((candidate) => [candidate.month.k, candidate.day?.k, candidate.year.k].includes(f.k));
        if (date && date.month.k !== f.k) return null;
        if (date) return (
          <If when={!shut([date.month.k, date.day?.k, date.year.k].filter((k): k is string => Boolean(k)))}>
            <View class="min-w-0" style={{ width: props.narrow() ? "100%" : undefined }}>
              <OneDateBox s={s} date={date} label={it.l} narrow={props.narrow} describedBy={() => props.describedBy(date)} errorDescription={() => props.errorDescription(date)} />
            </View>
          </If>
        );
        const sub = multi && f.l && f.l !== it.l && !/^\d+[A-Z]?$/.test(f.l);
        return (
          <If when={!hid().includes(f.k)}>
            <View class="min-w-0 gap-1" style={{ width: props.narrow() ? "100%" : undefined }}>
              {sub && f.t !== "chk" ? <Text variant="meta" tone="fg2">{f.l}</Text> : null}
              <FieldControl {...fieldProps(s, f, props.narrow, f.t === "chk" ? f.l || it.l : f.l)} />
            </View>
          </If>
        );
      })}
      <If when={splitOpen()}>
        <CalendarButton id={`fp-date-${props.id}`} value={splitIso()} min={splitRange().min} max={splitRange().max} onPick={(iso) => s.pickDate(split!, iso)} />
      </If>
    </View>
  );
  if (it.l !== "Additional details") return body;
  // B10: the row's own moreOpen is the open state — the filer's flag or a shown message, fact card
  // or to-do step pointing inside — read live, so attention opens the fold and clears it again.
  const open = () => props.info().moreOpen;
  return (
    <View class="gap-2.5">
      <AppButton
        variant="ghost"
        size="sm"
        icon={open() ? "chevron-down" : "chevron-right"}
        label={`${fs.length} more field${fs.length > 1 ? "s" : ""} not yet placed on the paper layout`}
        testID={`fp-more-${props.id}`}
        onPress={() => s.toggleMore(props.id)}
      />
      <If when={open()}>{body}</If>
    </View>
  );
}

function Mark(props: { icon: IconName; tip: string; id: string }) {
  return (
    <View testID={`fp-mark-${props.icon}-${props.id}`} class="shrink-0 pt-0.5" accessibilityLabel={props.tip} tooltip={<Tooltip label={props.tip} />}>
      <Icon name={props.icon} size={14} tone="fg3" />
    </View>
  );
}

/** A tax fact the profile has not set, asked where the form uses it (F2: inline choices, one status chip). */
function FactCard(props: { s: FormSession; ask: Accessor<AskCard>; narrow: Accessor<boolean> }) {
  // Drawn again when its row comes back into view (the rows are recycled): it starts from what was typed.
  const text = signal(untracked(() => props.s.askTextOf(props.ask().fact)));
  const fact = () => props.ask().fact;
  // A choice's value is opaque (a string, a number, a boolean): the pick is its JSON text.
  const pickOf = (v: string) => props.ask().choices.find((c) => JSON.stringify(c.value) === v)?.value;
  const commit = (v: string) => {
    const c = pickOf(v);
    if (c !== undefined) props.s.pickAsk(fact(), c);
  };
  const segments = () => props.ask().choices.map((c) => ({ value: JSON.stringify(c.value), label: c.short || c.label }));
  return (
    <View testID={`fp-ask-${fact()}`} class="mt-2 gap-1 rounded-(--radius-row) border border-border-strong bg-card px-3 py-2.5 shadow-xs">
      <View class="flex-row items-center justify-between gap-2">
        <Text variant="control" class="min-w-0 flex-1 font-semibold">{props.ask().label}</Text>
        <Chip
          testID={`fp-ask-${fact()}-chip`}
          label={props.ask().done ? "Saved to profile" : "Missing in tax profile"}
          tone={props.ask().done ? "success" : "warning"}
        />
      </View>
      <If when={props.ask().control !== "choice" || props.ask().pick === null}>
        <Text variant="meta" tone="fg2">{props.ask().note}</Text>
      </If>
      <View class="mt-2">
        <If
          when={props.ask().control === "choice"}
          else={
            <View class="flex-row flex-wrap items-end gap-2">
              <If when={props.ask().control === "date"} else={
                <TextField
                  testID={`fp-ask-${fact()}-input`}
                  accessibilityLabel={props.ask().label}
                  value={text()}
                  width={props.narrow() ? undefined : 320}
                  onChange={(v) => {
                    text.set(v);
                    props.s.typeAsk(fact(), v);
                  }}
                />
              }>
                <DateField
                  testID={`fp-ask-${fact()}-input`}
                  accessibilityLabel={props.ask().label}
                  value={text()}
                  width={props.narrow() ? undefined : controlWidth({ t: "date" }, 320)}
                  onChange={(v) => {
                    text.set(v);
                    props.s.typeAsk(fact(), v);
                  }}
                />
              </If>
            </View>
          }
        >
          <If when={props.ask().choices.length > 4} else={
            <Segmented
              testID={`fp-ask-${fact()}-choices`}
              label={props.ask().label}
              options={segments()}
              value={props.ask().pick ?? ""}
              wrap={props.narrow()}
              onChange={commit}
            />
          }>
            <Dropdown
              testID={`fp-ask-${fact()}-choices`}
              label={props.ask().label}
              options={props.ask().choices.map((c) => ({ value: JSON.stringify(c.value), label: c.label }))}
              value={props.ask().pick}
              width={props.narrow() ? undefined : 320}
              onChange={commit}
            />
          </If>
        </If>
      </View>
      <If when={props.ask().control !== "choice"}>
        <View class="mt-3 flex-row justify-end">
          <AppButton
            testID={`fp-ask-${fact()}-save`}
            variant="primary"
            size="sm"
            label="Save"
            onPress={() => props.s.saveAsk(fact())}
          />
        </View>
      </If>
    </View>
  );
}

/** 2551Q Item 13's card: the year's income-tax election, or why the profile cannot take one. */
function ElectionCardView(props: { s: FormSession; card: Accessor<ElectionCard>; narrow: Accessor<boolean> }) {
  const ask = () => {
    const c = props.card();
    return c.kind === "ask" ? c : null;
  };
  const blocked = () => {
    const c = props.card();
    return c.kind === "blocked" ? c : null;
  };
  const choice = (value: "graduated" | "eight_percent", label: string, hint: string) => (
    <AppButton
      testID={`fp-election-${value}`}
      label={label}
      hint={hint}
      block={props.narrow()}
      selected={ask()?.pick === value}
      accessibilityLabel={ask()?.pick === value ? `${label}, chosen` : label}
      onPress={() => props.s.pickElection(value)}
    />
  );
  return (
    <View testID="fp-election" class="mt-2 gap-1 rounded-(--radius-row) border border-border-strong bg-card px-3 py-2.5 shadow-xs">
      <If when={ask()}>
        {(a) => (
          <View class="gap-1">
            <Text variant="control" class="font-semibold">{a().question}</Text>
            <Text variant="meta" tone="fg2">We save it on the tax profile, so you answer once a year.</Text>
            <View class="mt-3 flex-row flex-wrap gap-2">
              {choice("graduated", "Graduated rates", "Tax on net taxable income")}
              {choice("eight_percent", "8% flat rate", "8% of gross sales or receipts")}
            </View>
            <View class="mt-3 flex-row justify-end">
              <AppButton testID="fp-election-save" variant="primary" size="sm" label="Save to tax profile" disabled={!a().pick} onPress={() => props.s.recordElection()} />
            </View>
          </View>
        )}
      </If>
      <If when={blocked()}>
        {(b) => (
          <View class="gap-1">
            <View class="flex-row items-start gap-2">
              <Icon name="info" tone="fg2" />
              <Text variant="control" class="min-w-0 flex-1 font-semibold">{b().title}</Text>
            </View>
            <Text variant="meta" tone="fg2">{b().body}</Text>
            <If when={b().action}>
              {(action) => (
                <View class="mt-3 flex-row justify-end">
                  <AppButton testID={action().id} variant="primary" size="sm" label={action().label} onPress={() => props.s.electionAction(action().id)} />
                </View>
              )}
            </If>
          </View>
        )}
      </If>
    </View>
  );
}

function Saved(props: { text: string; testID: string }) {
  return (
    <View testID={props.testID} class="mt-1 flex-row items-center gap-1.5">
      <Icon name="circle-check" size={14} tone="success" />
      <Text variant="meta" tone="success" class="min-w-0 flex-1">{props.text}</Text>
    </View>
  );
}

/** One item of a part. */
export function ItemRow(props: RowProps) {
  const s = props.s;
  const id = props.id;
  const it = props.it;
  const info = s.row(id);
  const fs = it.f || [];
  const allCalc = !it.rows && fs.length > 0 && fs.every((x) => x.c);
  const req = fs.some((x) => x.r);
  const label = req ? `${it.l} *` : it.l;
  const messageRefs = new Map<string, Signal<ViewRef | null>>();
  for (const date of oneDates(it)) {
    for (const field of [date.month, date.day, date.year]) {
      if (field) messageRefs.set(field.k, signal<ViewRef | null>(null));
    }
  }
  const rowMessageRef = signal<ViewRef | null>(null);
  const boxMessage = (date: OneDate) => {
    const keys = [date.month.k, date.day?.k, date.year.k];
    return info().boxMessages.find((entry) => keys.includes(entry.key));
  };
  const describedBy = (date: OneDate) => {
    const message = boxMessage(date);
    return message ? messageRefs.get(message.key)?.() ?? null : info().message ? rowMessageRef() : null;
  };
  const errorDescription = (date: OneDate) => boxMessage(date)?.sentence || info().message;
  // G1: a row whose every drawn box another answer keeps shut is not drawn at all (its number and title too).
  return (
    <If when={!info().hidden}>
    <View
      ref={(r: ViewRef) => props.onRef(id, r)}
      testID={`fp-item-${id}`}
      class="flex-row gap-3 border-border pt-3.5 pr-5 pb-2 pl-3"
      classList={{
        "border-t": !props.first?.(),
        "rounded-b-(--radius-card)": Boolean(props.last?.()),
        "bg-background": allCalc && !info().stepHere,
        "bg-accent-blue-soft": info().stepHere,
        "pr-3": props.narrow(),
      }}
    >
      {/* Under 600 pt the number sits in the label's line instead of a column of its own, so the boxes below start
          at the number's left edge and take the row's whole width (a phone has none to spare for an indent). */}
      <If when={!props.narrow()}>
        <View class="shrink-0" style={{ width: 44 }}>
          {it.no ? <ItemNo no={it.no} id={id} /> : null}
        </View>
      </If>
      <View class="min-w-0 flex-1">
        <View testID={`fp-head-${id}`} class="mb-2.5 min-h-5.5 flex-row flex-wrap items-start" style={{ gap: rowHead(props.narrow()).gap }}>
          {it.no ? (
            <If when={props.narrow()}>
              <ItemNo no={it.no} id={id} />
            </If>
          ) : null}
          {/* On a phone the label keeps at least 160 pt: a button that would leave it less wraps under it (rowHead). */}
          <View class="flex-1" style={{ minWidth: rowHead(props.narrow()).labelMin ?? 0 }}>
            <Text variant="control" testID={`fp-label-${id}`}>{label}</Text>
          </View>
          {it.rows ? <Text variant="meta" tone="fg3" class="pt-0.5">{`${it.rows} rows`}</Text> : null}
          {/* A profile TIN's own line under its boxes says it once; the row mark stays for every other profile row. */}
          <If when={info().fromProfile && !info().tinLine}>
            <Mark icon="user" id={id} tip={info().fromProfile} />
          </If>
          {allCalc ? <Mark icon="lock" id={id} tip="Computed from other items" /> : null}
          <If when={info().kept === "template"}>
            <Mark icon="layout-template" id={id} tip="From your template" />
          </If>
          <If when={info().kept === "last"}>
            <Mark icon="history" id={id} tip="Same as last time" />
          </If>
          <If when={info().penalty}>
            <AppButton
              size="sm"
              variant="ghost"
              icon={info().penalty === "computed" ? "history" : "pencil"}
              label={info().penalty === "computed" ? "Use the computed amounts" : "Edit penalties"}
              testID={`fp-penalty-${id}`}
              onPress={() => (info().penalty === "computed" ? s.usePenaltyComputed(id) : s.openPenalties(id))}
            />
          </If>
          <If when={info().canEdit && !info().editing}>
            <AppButton size="sm" variant="ghost" icon="pencil" label="Edit" tooltip="Change it on this return only" accessibilityLabel={`Edit ${it.l} on this return`} testID={`fp-edit-${id}`} onPress={() => s.startProfileEdit(id)} />
          </If>
          <If when={info().stoppable && info().stepHere}>
            <Switch testID={`fp-stop-${id}`} label="Stop here" checked={info().isStop} onChange={(on) => s.setStop(id, on)}>
              <Text variant="meta" tone="fg2">Stop here</Text>
            </Switch>
          </If>
        </View>
        <Each of={info().needs}>
          {(why) => (
            <View class="mb-1.5" tooltip={<Tooltip label={why()} />}>
              <Text variant="meta" tone="fg3">{needsText(why())}</Text>
            </View>
          )}
        </Each>
        <Controls s={s} id={id} it={it} narrow={props.narrow} info={info} describedBy={describedBy} errorDescription={errorDescription} />
        <If when={info().profileAsk}>
          {(pa) => (
            <If when={pa().ok} else={<Text variant="meta" tone="fg3" testID={`fp-profile-ask-${id}`}>{pa().text}</Text>}>
              <Saved testID={`fp-profile-ask-${id}`} text={pa().text} />
            </If>
          )}
        </If>
        <If when={info().from}>
          <View testID={`fp-from-${id}`} class="mt-1 flex-row items-center gap-2">
            <Text variant="meta" tone="fg3" class="min-w-0 shrink">{info().from}</Text>
            <AppButton size="sm" variant="ghost" label="Use it" testID={`fp-from-use-${id}`} onPress={() => s.useFrom(id)} />
          </View>
        </If>
        <Each of={info().notes}>{(note) => <Text variant="meta" tone="fg3" class="mt-1">{note()}</Text>}</Each>
        <Each of={info().boxMessages} key={(b: { key: string; sentence: string }) => b.key}>
          {(b) => (
            <View ref={(ref: ViewRef) => messageRefs.get(b().key)?.set(ref)}>
              <Text variant="meta" tone="danger" testID={`fp-boxmsg-${b().key}`}>{b().sentence}</Text>
            </View>
          )}
        </Each>
        <If when={info().message}>
          <View ref={(ref: ViewRef) => { if (messageRefs.size) rowMessageRef.set(ref); }} class="mt-1">
            <Text variant="meta" tone="danger" testID={`fp-msg-${id}`}>{info().message}</Text>
          </View>
        </If>
        <Each of={info().asks} key={(a: AskCard) => a.fact}>
          {(ask) => <FactCard s={s} ask={ask} narrow={props.narrow} />}
        </Each>
        <If when={info().askSaved}>
          <Saved testID={`fp-ask-saved-${id}`} text={info().askSaved} />
        </If>
        <If when={info().election}>{(card) => <ElectionCardView s={s} card={card} narrow={props.narrow} />}</If>
        <If when={info().electionSaved}>
          <Saved testID="fp-election-saved" text={info().electionSaved} />
        </If>
        <Each of={info().checkable} key={(c: { token: string; label: string }) => c.token}>
          {(c) => (
            <View class="mt-1 flex-row">
              <AppButton size="sm" variant="ghost" label={c().label} testID={`fp-checked-${c().token}`} onPress={() => s.markChecked(c().token)} />
            </View>
          )}
        </Each>
      </View>
    </View>
    </If>
  );
}

/** An item's number in its small box. */
function ItemNo(props: { no: string; id: string }): JSX.Element {
  return (
    <View testID={`fp-no-${props.id}`} class="h-5.5 min-w-7.5 items-center justify-center self-start rounded-(--radius-ctl) bg-hover px-1.5">
      <Code tone="fg2">{props.no}</Code>
    </View>
  );
}

/** A part's "From tax profile · N filled boxes" fold: profile-filled items fold so what is left to type shows. */
export function ProfileFold(props: { s: FormSession; pi: number; first?: Accessor<boolean> }): JSX.Element {
  const part = props.s.part(props.pi);
  return (
    <If when={part().fold}>
      {(fold) => (
        <View class="border-border bg-background px-1 py-1" classList={{ "border-t": !props.first?.() }}>
          <AppButton
            variant="ghost"
            size="sm"
            testID={`fp-profile-${props.pi}`}
            icon={fold().open ? "chevron-down" : "chevron-right"}
            label={fold().label + (fold().flagged ? `  ·  ${fold().flagged} need${fold().flagged === 1 ? "s" : ""} attention` : "")}
            accessibilityLabel={`${fold().label}, ${fold().open ? "shown" : "folded"}`}
            onPress={() => props.s.toggleProfileFold(props.pi)}
            trailing={<Icon name="user" size={14} tone="fg3" />}
          />
        </View>
      )}
    </If>
  );
}
