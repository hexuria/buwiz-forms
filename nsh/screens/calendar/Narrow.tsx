// The Calendar under 600 points: a month grid with a dot on each due day (danger when something is late), a month stepper, and
// under it the Upcoming list (late first) in the same rows as the List. A press on a day narrows the list to it; the taxpayer
// and how-often filters live in a sheet.
import { computed, signal, View } from "nsh";
import type { DeadlineRow } from "../../bir";
import { MONTHS_LONG } from "../../lib/dates";
import { app, store, today } from "../../shell";
import {
  AppButton, Chip, CheckboxLabel, EmptyRow, IconButton, Line, ListGroup, PressCell, Segmented, Sheet, Text, TextField,
} from "../../ui";
import { DueRow } from "./DueRow";
import { calendarState } from "./filters";
import { Dot, taxpayerOptions } from "./parts";
import {
  byWhoAndHowOften, type Cell, dayTone, FREQ_OPTIONS, type Freq, gridCells, gridRange, inMonthCount, longDay, plural, rowKey, upcomingRange,
  upcomingRows, WEEKDAYS,
} from "./rows";

const FRESH_TEXT = "Deadlines show here once a taxpayer has forms for a tax year.";

/** The two filters this layout has (taxpayers, how often): how many differ from their start. */
const narrowedCount = (): number => (calendarState.taxpayers() !== null ? 1 : 0) + (calendarState.freq() !== "all" ? 1 : 0);

// ---------------------------------------------------------------- the filter sheet

/** The taxpayers as checkboxes (with a search), and how often. Changes apply at once; Done closes the sheet. */
function FilterSheet(props: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const search = signal("");
  const options = computed(() => taxpayerOptions());
  const found = computed(() => {
    const words = search().trim().toLowerCase().split(/\s+/).filter(Boolean);
    return options().filter((o) => {
      const hay = `${o.label} ${o.sub ?? ""}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
  });
  const current = () => new Set(calendarState.taxpayers() ?? options().map((o) => o.value));
  /** A selection handed to the page: null once it holds every taxpayer (so one that appears later is in it too). */
  const choose = (next: Set<string>) => calendarState.setTaxpayers(options().every((o) => next.has(o.value)) ? null : next);
  const toggle = (value: string) => {
    const next = current();
    if (next.has(value)) next.delete(value);
    else next.add(value);
    choose(next);
  };
  const selectAll = () => {
    const next = current();
    for (const o of found()) next.add(o.value);
    choose(next);
  };
  const clear = () => {
    const next = current();
    for (const o of found()) next.delete(o.value);
    choose(next);
  };
  return (
    <Sheet
      testID="calendar-filter-sheet"
      open={props.open}
      onOpenChange={props.onOpenChange}
      title="Filters"
      footer={
        <>
          <AppButton testID="calendar-sheet-clear" label="Clear filters" variant="ghost" disabled={narrowedCount() === 0} onPress={() => calendarState.clear()} />
          <AppButton testID="calendar-sheet-done" label="Done" variant="primary" onPress={() => props.onOpenChange(false)} />
        </>
      }
    >
      <View class="gap-2">
        <Text variant="metaStrong" tone="fg2">How often</Text>
        <Segmented
          testID="calendar-freq"
          label="How often"
          options={FREQ_OPTIONS}
          value={calendarState.freq()}
          onChange={(v) => calendarState.setFreq(v as Freq)}
        />
      </View>
      <View class="gap-1">
        <View class="flex-row items-center gap-1">
          <Text variant="metaStrong" tone="fg2" class="flex-1">Taxpayers</Text>
          <AppButton testID="calendar-tp-all" label="Select all" variant="ghost" size="sm" onPress={selectAll} />
          <AppButton testID="calendar-tp-clear" label="Clear" variant="ghost" size="sm" onPress={clear} />
        </View>
        <TextField
          size="sm"
          icon="search"
          testID="calendar-tp-search"
          value={search()}
          onChange={(v) => search.set(v)}
          placeholder="Find a taxpayer"
          accessibilityLabel="Find a taxpayer"
        />
        <If when={found().length === 0}>
          <Text variant="meta" tone="fg3" class="p-2">Nothing matches.</Text>
        </If>
        <Each of={found()} key={(o) => o.value}>
          {(o) => (
            <CheckboxLabel
              testID={`calendar-tp-${o().value}`}
              label={o().label}
              hint={o().sub}
              checked={calendarState.taxpayers() === null || calendarState.taxpayers()!.has(o().value)}
              onChange={() => toggle(o().value)}
            />
          )}
        </Each>
      </View>
    </Sheet>
  );
}

// ---------------------------------------------------------------- the grid

/** One day: its number (a ring on today) and a dot when something is due (danger when something is late). */
function NarrowCell(props: { c: () => Cell }) {
  const c = props.c;
  const tone = () => dayTone(c().items, today());
  return (
    <PressCell
      testID={`calendar-day-${c().iso}`}
      label={`${c().label}${c().items.length > 0 && tone() === "danger" ? ", late" : ""}`}
      selected={calendarState.picked() === c().iso}
      onPress={() => calendarState.togglePicked(c().iso)}
      style={{ minHeight: 44 }}
      class={`items-center gap-0.5 rounded-(--radius-row) py-1 ${c().inMonth ? "" : "opacity-40"}`}
    >
      <View
        testID={c().isToday ? "calendar-today-ring" : undefined}
        class={`size-6 shrink-0 items-center justify-center rounded-full ${c().isToday ? "bg-accent-blue" : ""}`}
      >
        <Text variant="control" tone={c().isToday ? "primaryFg" : "fg"} class="tabular-nums">{String(c().day)}</Text>
      </View>
      <View class="h-1.5 items-center justify-center">
        <If when={c().items.length > 0}>
          <Dot testID={`calendar-dot-${c().iso}`} tone={tone()} />
        </If>
      </View>
    </PressCell>
  );
}

// ---------------------------------------------------------------- the page

/** The Calendar for a window under 600 points. */
export function NarrowCalendar() {
  const sheet = signal(false);
  const y = () => calendarState.gridYear();
  const m = () => calendarState.gridMonth();
  const who = () => ({ tps: calendarState.taxpayers(), freq: calendarState.freq() });
  // Six weeks of the shown month, so the grid keeps its height.
  const rows = computed(() => {
    const [from, to] = gridRange(y(), m());
    return byWhoAndHowOften(who(), store.deadlines(from, to));
  });
  const cells = computed(() => gridCells(y(), m(), rows(), today()));
  // What is still to do: late first, then what falls due in the next 60 days.
  const coming = computed(() => {
    const now = today();
    const [from, to] = upcomingRange(now);
    return upcomingRows(byWhoAndHowOften(who(), store.deadlines(from, to)), now);
  });
  const listed = (): DeadlineRow[] => {
    const day = calendarState.picked();
    return day ? (cells().find((c) => c.iso === day)?.items ?? []) : coming();
  };
  const emptyText = () =>
    calendarState.picked()
      ? "Nothing due this day."
      : narrowedCount() > 0
        ? "No deadlines match these filters."
        : "Nothing to do in the next 60 days.";
  return (
    <View testID="calendar-narrow" class="w-full gap-3">
      <If
        when={!app.isFresh()}
        else={
          <ListGroup testID="calendar-empty">
            <EmptyRow testID="calendar-empty-row" text={FRESH_TEXT} />
          </ListGroup>
        }
      >
        <View class="w-full flex-row flex-wrap items-center gap-1">
          <IconButton testID="calendar-prev-month" icon="chevron-left" tooltip="Previous month" onPress={() => calendarState.stepMonth(-1)} />
          <View testID="calendar-month-title" style={{ minWidth: 130 }}>
            <Text variant="title" class="text-center">{`${MONTHS_LONG[m()]} ${y()}`}</Text>
          </View>
          <IconButton testID="calendar-next-month" icon="chevron-right" tooltip="Next month" onPress={() => calendarState.stepMonth(1)} />
          <AppButton testID="calendar-today" label="Today" variant="ghost" size="sm" onPress={() => calendarState.showToday()} />
        </View>
        <View class="w-full flex-row items-center gap-2">
          <AppButton
            testID="calendar-filters-open"
            label="Filters"
            icon="users"
            selected={narrowedCount() > 0}
            trailing={
              <If when={narrowedCount() > 0}>
                <Chip testID="calendar-filters-count" label={String(narrowedCount())} tone="accent" />
              </If>
            }
            onPress={() => sheet.set(true)}
          />
          <View testID="calendar-month-count" class="ml-auto">
            <Text variant="meta" tone="fg3">{`${plural(inMonthCount(y(), m(), rows()), "deadline")} this month`}</Text>
          </View>
        </View>
        <View testID="calendar-grid" accessibilityLabel={`${MONTHS_LONG[m()]} ${y()}`} class="w-full overflow-hidden rounded-(--radius-card) border border-border bg-card p-1">
          <View class="grid grid-cols-7">
            <Each of={WEEKDAYS} key={(w) => w}>
              {(w) => (
                <View class="min-w-0 items-center py-1.5">
                  <Line variant="cap" tone="fg3" class="font-semibold">{w().slice(0, 3).toUpperCase()}</Line>
                </View>
              )}
            </Each>
          </View>
          <View class="grid grid-cols-7">
            <Each of={cells()} key={(c) => c.iso}>
              {(c) => <NarrowCell c={c} />}
            </Each>
          </View>
        </View>
        <View class="mt-2 w-full flex-row items-center gap-2">
          <Text testID="calendar-upcoming-title" variant="title" class="flex-1">
            {calendarState.picked() ? longDay(calendarState.picked() as string) : "Upcoming"}
          </Text>
          <If when={calendarState.picked()}>
            <AppButton testID="calendar-show-upcoming" label="Show upcoming" variant="ghost" size="sm" onPress={() => calendarState.clearPicked()} />
          </If>
        </View>
        <ListGroup testID="calendar-upcoming">
          <If
            when={listed().length > 0}
            else={
              <EmptyRow
                testID="calendar-upcoming-empty"
                text={emptyText()}
                action={
                  narrowedCount() > 0 && !calendarState.picked() ? (
                    <AppButton testID="calendar-clear" label="Clear filters" size="sm" onPress={() => calendarState.clear()} />
                  ) : undefined
                }
              />
            }
          >
            <Each of={listed()} key={rowKey}>
              {(d) => <DueRow d={d()} compact />}
            </Each>
          </If>
        </ListGroup>
        <FilterSheet open={sheet()} onOpenChange={(o) => sheet.set(o)} />
      </If>
    </View>
  );
}
