// Filings: every return, saved or filed, on a board with one column per status group (wide), or as a list under status tabs
// (narrow, under 720 points of page width). Above it a search box, four multi-select filters (taxpayers, forms, periods,
// statuses), Clear filters and a count; narrow, the four filters sit in a Filters sheet. A card is its return and opens it;
// the one button a card keeps is Cancel queue. Ported from the old shell's filings.js; the narrow layout is new (plan, UI decisions).
//
// Reads: the two cached lists Home also makes, bir.list_returns("filed") and ("saved"), through the store. Writes: bir.cancel_queued.
// Under them, "Filed in BIR eBIRForms" (LegacyGroup): what the import brought in from the old program, for the taxpayer in view.
// A Home tile arrives with params { statuses, updatedMonth } (the old FilingsPreset's names): they replace every filter, once.
import type { JSX } from "nsh";
import { computed, signal, untracked, View } from "nsh";
import type { ReturnRow } from "../../bir";
import { app, cancelQueue, openReturn, store } from "../../shell";
import { BREAK } from "../../theme/tokens";
import { AppButton, Chip, EmptyRow, Icon, ListGroup, PageHeader, Sheet, Tabs, Text, TextField } from "../../ui";
import { Board } from "./Board";
import { FilterCombos } from "./FilterCombos";
import { filters } from "./filters";
import { LegacyGroup } from "./LegacyGroup";
import { COLUMNS, type FormIndex, filterRows, monthLabel, newestFirst, plural, presetOf } from "./model";
import { NarrowList } from "./NarrowList";

/** Page width (the main surface less its padding) from which the board shows; under it, the list with tabs. */
export const BOARD_MIN = 720;

/** "Changed in Nov 2026", the month a Home tile asked for: a press removes it. */
function MonthChip() {
  return (
    <AppButton
      testID="filings-month-chip"
      label={`Changed in ${monthLabel(filters.updatedMonth() ?? "")}`}
      accessibilityLabel={`Changed in ${monthLabel(filters.updatedMonth() ?? "")}. Remove`}
      size="sm"
      trailing={<Icon name="x" size={14} tone="fg3" />}
      onPress={filters.clearMonth}
    />
  );
}

/** The search box. */
function Search(props: { width?: number }) {
  return (
    <TextField
      testID="filings-search"
      width={props.width}
      value={filters.query()}
      onChange={filters.setQuery}
      placeholder="Search returns"
      accessibilityLabel="Search returns"
    />
  );
}

function ClearFilters() {
  return <AppButton testID="filings-clear" label="Clear filters" variant="ghost" size="sm" icon="x" onPress={filters.clear} />;
}

/** Nothing saved or filed yet. */
function Empty() {
  return (
    <View class="mt-4">
      <ListGroup testID="filings-list">
        <EmptyRow testID="filings-empty" text="Saved and filed returns show here." />
      </ListGroup>
    </View>
  );
}

/** Returns exist, the search and filters keep none. */
function NoMatch() {
  return (
    <View class="mt-4">
      <ListGroup testID="filings-list">
        <EmptyRow
          testID="filings-empty"
          text="No returns match these filters."
          action={<AppButton testID="filings-clear-empty" label="Clear filters" size="sm" onPress={filters.clear} />}
        />
      </ListGroup>
    </View>
  );
}

/** The wide filter bar: search, the four combos, the month chip, Clear filters and the count. */
function WideBar(props: { all: ReturnRow[]; index: FormIndex; count: number }) {
  return (
    <View testID="filings-filters" class="mt-6 w-full flex-row flex-wrap items-center gap-2">
      <Search width={200} />
      <FilterCombos all={props.all} index={props.index} />
      <If when={filters.updatedMonth() !== null}>
        <MonthChip />
      </If>
      <If when={filters.active()}>
        <ClearFilters />
      </If>
      <View testID="filings-count" class="ml-auto">
        <Text variant="meta" tone="fg2">{plural(props.count, "return")}</Text>
      </View>
    </View>
  );
}

/** The narrow bar: search and a Filters button that opens the sheet; the month chip stays in view. */
function NarrowBar(props: { onFilters: () => void }) {
  return (
    <View testID="filings-filters" class="mt-4 w-full gap-2">
      <View class="w-full flex-row items-center gap-2">
        <View class="min-w-0 flex-1">
          <Search />
        </View>
        <AppButton
          testID="filings-filters-open"
          label="Filters"
          trailing={
            <If when={filters.narrowed() > 0}>
              <Chip label={String(filters.narrowed())} tone="accent" />
            </If>
          }
          onPress={props.onFilters}
        />
      </View>
      <If when={filters.updatedMonth() !== null}>
        <View class="flex-row">
          <MonthChip />
        </View>
      </If>
    </View>
  );
}

/** The returns of the saved and filed lists, the filters and the two actions of a card. */
export function Filings(): JSX.Element {
  // A Home tile's preset, once per navigation: Back to this page hands over the same params object and applies nothing.
  const params = untracked(() => app.params());
  const preset = presetOf(params);
  if (preset) filters.applyPreset(preset, params);

  const all = computed<ReturnRow[]>(() => (app.isFresh() ? [] : newestFirst([...store.returns("filed"), ...store.returns("saved")])));
  // The form library's index names each form's frequency and title; it is not read while there is nothing to name.
  const index = computed<FormIndex>(() => (all().length ? store.screenIndex() : []));
  const rows = computed(() => filterRows(all(), index(), filters.values()));
  // The frame pads the page 16 each side under 640 points of main surface, 32 from there (Shell.tsx ViewFrame).
  const pad = () => (app.layout.mainWidth() < BREAK.narrowContent ? 16 : 32);
  const column = () => Math.max(200, app.layout.mainWidth() - pad() * 2);
  const wide = () => column() >= BOARD_MIN;
  const sheetOpen = signal(false);

  const open = (r: ReturnRow) =>
    openReturn({ tin: r.tin, code: r.form_code, year: r.taxable_year, period: r.period, ...(r.return_key != null ? { returnKey: r.return_key } : {}) });
  // The card's own press opens the return; this button takes its press (only the innermost pressable responds).
  const cancel = (r: ReturnRow) => void cancelQueue(r.tin, r.form_code, r.taxable_year, r.period || null);

  const tabs = computed(() => [
    { value: "all", label: "All", count: rows().length },
    ...COLUMNS.map((c) => ({ value: c.id, label: c.title, count: rows().filter((r) => c.statuses.includes(r.status)).length })),
  ]);
  const shownCount = () => {
    const c = COLUMNS.find((x) => x.id === filters.tab());
    return c ? rows().filter((r) => c.statuses.includes(r.status)).length : rows().length;
  };

  return (
    <View testID="filings" class="w-full">
      <PageHeader title="Filings" lede="Every return, from draft to confirmed. Filed returns stay exactly as submitted." />
      <If when={wide()} else={<NarrowBar onFilters={() => sheetOpen.set(true)} />}>
        <WideBar all={all()} index={index()} count={rows().length} />
      </If>
      <If when={all().length > 0} else={<Empty />}>
        <If when={wide()}>
          <If when={rows().length > 0} else={<NoMatch />}>
            <Board rows={rows()} index={index()} onOpen={open} onCancel={cancel} />
          </If>
        </If>
        <If when={!wide()}>
          {/* Five tabs with counts side by side are wider than a phone: each count goes under its label, so all five fit. */}
          <Tabs testID="filings-tabs" fill tabs={tabs()} value={filters.tab()} onChange={filters.setTab} />
          <View class="mb-3 w-full flex-row items-center justify-between">
            <View testID="filings-count">
              <Text variant="meta" tone="fg2">{plural(shownCount(), "return")}</Text>
            </View>
            <If when={filters.active()}>
              <ClearFilters />
            </If>
          </View>
          <If when={rows().length > 0} else={<NoMatch />}>
            <NarrowList rows={rows()} index={index()} onOpen={open} onCancel={cancel} />
          </If>
        </If>
      </If>
      {/* What was filed in the old program is not one of this app's returns: no status, no card, nothing filters it. It shows while nothing is narrowed. */}
      <If when={!filters.active() && (wide() || filters.tab() === "all")}>
        <LegacyGroup wide={wide()} />
      </If>
      <If when={!wide()}>
        <Sheet
          testID="filings-sheet"
          open={sheetOpen()}
          onOpenChange={(o) => sheetOpen.set(o)}
          title="Filters"
          width={360}
          footer={
            <>
              <If when={filters.active()}>
                <AppButton testID="filings-sheet-clear" label="Clear filters" onPress={filters.clear} />
              </If>
              <AppButton testID="filings-sheet-done" label={`Show ${plural(rows().length, "return")}`} variant="primary" onPress={() => sheetOpen.set(false)} />
            </>
          }
        >
          <FilterCombos all={all()} index={index()} stacked />
        </Sheet>
      </If>
    </View>
  );
}
