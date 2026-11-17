// The Calendar from 600 points up (old renderCalendar): the page header, the filter bar, then the List (deadlines grouped by
// due month, or by covered quarter under Quarterly) or the Month (a six-week grid and the chosen day's deadlines beside it).
import { computed, View } from "nsh";
import type { DeadlineRow } from "../../bir";
import { MONTHS, MONTHS_LONG } from "../../lib/dates";
import { app, store, today } from "../../shell";
import { AppButton, EmptyRow, IconButton, Line, ListGroup, MultiCombo, PageHeader, PressCell, Segmented, Text, TOUCH } from "../../ui";
import { DayRow, DueRow } from "./DueRow";
import { calendarState } from "./filters";
import { contentWidth, MiniChip, taxpayerOptions } from "./parts";
import {
  byWhoAndHowOften, calendarRows, type Cell, countText, FREQ_OPTIONS, type Freq, gridCells, gridRange, groupRows, inMonthCount, longDay,
  plural, QUARTER_MONTHS, rowKey, WEEKDAYS,
} from "./rows";

const FRESH_TEXT = "Deadlines show here once a taxpayer has forms for a tax year.";

// ---------------------------------------------------------------- the filter bar

/** The tax year, stepped inside the years the app has. */
function YearStepper() {
  const bounds = () => store.yearBounds();
  return (
    <View
      testID="calendar-year"
      accessibilityLabel={`Tax year ${calendarState.year()}`}
      class={`shrink-0 flex-row items-center rounded-(--radius-row) border border-border-strong bg-card px-0.5 ${TOUCH ? "min-h-11" : "h-8"}`}
    >
      <IconButton
        testID="calendar-year-prev"
        icon="chevron-left"
        tooltip="Previous year"
        disabled={calendarState.year() <= bounds().min}
        onPress={() => calendarState.stepYear(-1, bounds().min, bounds().max)}
      />
      <Text variant="control" class="px-1.5 text-center font-semibold tabular-nums" testID="calendar-year-text">{String(calendarState.year())}</Text>
      <IconButton
        testID="calendar-year-next"
        icon="chevron-right"
        tooltip="Next year"
        disabled={calendarState.year() >= bounds().max}
        onPress={() => calendarState.stepYear(1, bounds().min, bounds().max)}
      />
    </View>
  );
}

/** The taxpayers, how often, the year (List only) and the List | Month switch. */
function FilterBar() {
  return (
    <View testID="calendar-filters" class="mt-6 w-full flex-row flex-wrap items-center gap-2">
      <MultiCombo
        testID="calendar-taxpayers"
        all="All taxpayers"
        nouns="taxpayers"
        icon="users"
        placeholder="Find a taxpayer"
        options={taxpayerOptions()}
        selected={calendarState.taxpayers()}
        onChange={(selected) => calendarState.setTaxpayers(selected)}
      />
      <View>
        <Segmented
          testID="calendar-freq"
          label="How often"
          options={FREQ_OPTIONS}
          value={calendarState.freq()}
          onChange={(v) => calendarState.setFreq(v as Freq)}
        />
      </View>
      <If when={calendarState.view() === "list"}>
        <YearStepper />
      </If>
      <View class="ml-auto">
        <Segmented
          testID="calendar-view"
          label="View"
          options={[
            { value: "list", label: "List", icon: "list" },
            { value: "month", label: "Month", icon: "calendar" },
          ]}
          value={calendarState.view()}
          onChange={(v) => calendarState.setView(v === "month" ? "month" : "list")}
        />
      </View>
    </View>
  );
}

// ---------------------------------------------------------------- the List

const MONTH_CHIPS = [{ value: "0", label: "All months" }, ...MONTHS.map((m, i) => ({ value: String(i + 1), label: m }))];
const QUARTER_CHIPS = [{ value: "0", label: "All quarters" }, ...[1, 2, 3, 4].map((q) => ({ value: String(q), label: `Q${q}` }))];

/** Month chips under Monthly (the month a return is due), quarter chips under Quarterly (the quarter it covers). */
function ChipRow() {
  return (
    <>
      <If when={calendarState.freq() === "monthly"}>
        <View class="mt-2.5 w-full flex-row flex-wrap items-center gap-2">
          <View class="min-w-0 shrink">
            <Segmented
              testID="calendar-month"
              label="Month due"
              wrap
              options={MONTH_CHIPS}
              value={String(calendarState.month())}
              onChange={(v) => calendarState.setMonth(Number(v))}
            />
          </View>
          <If when={calendarState.month()}>
            <Text variant="meta" tone="fg3">{`Forms due in ${MONTHS_LONG[calendarState.month() - 1]}`}</Text>
          </If>
        </View>
      </If>
      <If when={calendarState.freq() === "quarterly"}>
        <View class="mt-2.5 w-full flex-row flex-wrap items-center gap-2">
          <Segmented
            testID="calendar-quarter"
            label="Quarter covered"
            options={QUARTER_CHIPS}
            value={String(calendarState.quarter())}
            onChange={(v) => calendarState.setQuarter(Number(v))}
          />
          <If when={calendarState.quarter()}>
            <Text variant="meta" tone="fg3">{`Returns for ${QUARTER_MONTHS[calendarState.quarter() - 1]}`}</Text>
          </If>
        </View>
      </If>
    </>
  );
}

function ListView() {
  // Every deadline the List can show for a year, in one read: due from January 1 of the year (a month chip) through the end
  // of the next year (the year's December, fourth-quarter and annual returns fall due then).
  const all = computed(() => store.deadlines(`${calendarState.year()}-01-01`, `${calendarState.year() + 1}-12-31`));
  const rows = computed(() => calendarRows(calendarState.filters(), all()));
  const groups = computed(() => groupRows(calendarState.filters(), rows()));
  const compact = () => contentWidth() < 560;
  const clearable = () => all().length > 0 || calendarState.active();
  return (
    <View testID="calendar-list" class="w-full">
      <ChipRow />
      <View testID="calendar-count" class="mt-4">
        <Text variant="meta" tone="fg2">{countText(calendarState.filters(), rows())}</Text>
      </View>
      <If
        when={rows().length > 0}
        else={
          <View class="mt-4">
            <ListGroup testID="calendar-empty">
              <EmptyRow
                testID="calendar-empty-row"
                text={clearable() ? "No deadlines match these filters." : FRESH_TEXT}
                action={
                  clearable() ? <AppButton testID="calendar-clear" label="Clear filters" size="sm" onPress={() => calendarState.clear()} /> : undefined
                }
              />
            </ListGroup>
          </View>
        }
      >
        <Each of={groups()} key={(g) => g.label}>
          {(g) => (
            <>
              <View class={`w-full flex-row flex-wrap items-center gap-2 mb-3 ${g().label === groups()[0]?.label ? "mt-5" : "mt-8"}`}>
                <Text variant="title">{g().title}</Text>
                <If when={g().caption}>
                  <Text variant="meta" tone="fg3">{g().caption as string}</Text>
                </If>
              </View>
              <ListGroup testID={`calendar-group-${g().label.replace(" ", "-")}`}>
                <Each of={g().rows} key={rowKey}>
                  {(d) => <DueRow d={d()} compact={compact()} />}
                </Each>
              </ListGroup>
            </>
          )}
        </Each>
      </If>
    </View>
  );
}

// ---------------------------------------------------------------- the Month

/** One day of the grid: its number (a ring on today), up to two chips and "+N more"; only a count when the pane is narrow. */
function MonthCell(props: { c: () => Cell; compact: boolean }) {
  const c = props.c;
  return (
    <PressCell
      testID={`calendar-day-${c().iso}`}
      label={c().label}
      selected={calendarState.day() === c().iso}
      onPress={() => calendarState.setDay(c().iso)}
      style={{ minHeight: props.compact ? 52 : 78 }}
      class={`gap-[3px] ${props.compact ? "p-1" : "p-1.5"} ${c().col > 0 ? "border-l border-border" : ""} ${c().row > 0 ? "border-t border-border" : ""} ${c().inMonth ? "" : "opacity-40"}`}
    >
      <View
        testID={c().isToday ? "calendar-today-ring" : undefined}
        class={`size-5.5 shrink-0 items-center justify-center rounded-full ${c().isToday ? "bg-accent-blue" : ""}`}
      >
        <Text variant="metaStrong" tone={c().isToday ? "primaryFg" : "fg2"} class="tabular-nums">{String(c().day)}</Text>
      </View>
      <If
        when={!props.compact}
        else={
          <If when={c().items.length > 0}>
            <Text variant="cap" tone="fg2">{String(c().items.length)}</Text>
          </If>
        }
      >
        <Each of={c().items.slice(0, 2)} key={rowKey}>
          {(d) => <MiniChip d={d()} />}
        </Each>
        <If when={c().items.length > 2}>
          <Text variant="cap" tone="fg3" class="pl-1">{`+${c().items.length - 2} more`}</Text>
        </If>
      </If>
    </PressCell>
  );
}

function MonthView() {
  const y = () => calendarState.gridYear();
  const m = () => calendarState.gridMonth();
  // Six weeks always, so the grid keeps its height from month to month.
  const rows = computed(() => {
    const [from, to] = gridRange(y(), m());
    return byWhoAndHowOften({ tps: calendarState.taxpayers(), freq: calendarState.freq() }, store.deadlines(from, to));
  });
  const cells = computed(() => gridCells(y(), m(), rows(), today()));
  const dayItems = (): DeadlineRow[] => cells().find((c) => c.iso === calendarState.day())?.items ?? [];
  const compact = () => contentWidth() < 560;
  const side = () => contentWidth() >= 760;
  // The day panel is 280 wide beside the grid, 320 where the column is wide enough to keep the grid's cells as they are.
  const panel = () => (contentWidth() >= 840 ? 320 : 280);
  return (
    <View testID="calendar-month-view" class="w-full">
      <View class="mt-5 w-full flex-row flex-wrap items-center gap-1">
        <IconButton testID="calendar-prev-month" icon="chevron-left" tooltip="Previous month" onPress={() => calendarState.stepMonth(-1)} />
        <View testID="calendar-month-title" style={{ minWidth: 150 }}>
          <Text variant="title" class="text-center">{`${MONTHS_LONG[m()]} ${y()}`}</Text>
        </View>
        <IconButton testID="calendar-next-month" icon="chevron-right" tooltip="Next month" onPress={() => calendarState.stepMonth(1)} />
        <AppButton testID="calendar-today" label="Today" variant="ghost" size="sm" onPress={() => calendarState.showToday()} />
        <View testID="calendar-month-count" class="ml-auto">
          <Text variant="meta" tone="fg3">{`${plural(inMonthCount(y(), m(), rows()), "deadline")} this month`}</Text>
        </View>
      </View>
      <View class={`mt-3 w-full gap-4 ${side() ? "flex-row items-start" : ""}`}>
        <View
          testID="calendar-grid"
          accessibilityLabel={`${MONTHS_LONG[m()]} ${y()}`}
          class={`min-w-0 overflow-hidden rounded-(--radius-card) border border-border ${side() ? "flex-1" : "w-full"}`}
        >
          <View class="grid grid-cols-7 border-b border-border bg-background">
            <Each of={WEEKDAYS} key={(w) => w}>
              {(w) => (
                <View class="min-w-0 px-2 py-1.5">
                  <Line variant="cap" tone="fg3" class="font-semibold">{w().slice(0, 3).toUpperCase()}</Line>
                </View>
              )}
            </Each>
          </View>
          <View class="grid grid-cols-7">
            <Each of={cells()} key={(c) => c.iso}>
              {(c) => <MonthCell c={c} compact={compact()} />}
            </Each>
          </View>
        </View>
        <View testID="calendar-day" class={side() ? "shrink-0" : "w-full"} style={{ width: side() ? panel() : undefined }}>
          <If
            when={calendarState.day()}
            else={
              <>
                <Text variant="emph" class="mb-2">{`${MONTHS_LONG[m()]} ${y()}`}</Text>
                <ListGroup testID="calendar-day-list">
                  <EmptyRow testID="calendar-day-none" text="Pick a day to see what is due." />
                </ListGroup>
              </>
            }
          >
            <Text variant="emph" class="mb-2">{longDay(calendarState.day() as string)}</Text>
            <ListGroup testID="calendar-day-list">
              <If when={dayItems().length > 0} else={<EmptyRow testID="calendar-day-empty" text="Nothing due this day." />}>
                <Each of={dayItems()} key={rowKey}>
                  {(d) => <DayRow d={d()} />}
                </Each>
              </If>
            </ListGroup>
          </If>
        </View>
      </View>
    </View>
  );
}

// ---------------------------------------------------------------- the page

/** The Calendar for a window 600 points wide or more. */
export function WideCalendar() {
  return (
    <View testID="calendar" class="w-full">
      <PageHeader title="Calendar" lede="Every deadline for the forms your taxpayers file, from the BIR tax calendar." />
      <FilterBar />
      <If
        when={!app.isFresh()}
        else={
          <View class="mt-6">
            <ListGroup testID="calendar-empty">
              <EmptyRow testID="calendar-empty-row" text={FRESH_TEXT} />
            </ListGroup>
          </View>
        }
      >
        <If when={calendarState.view() === "month"} else={<ListView />}>
          <MonthView />
        </If>
      </If>
    </View>
  );
}
