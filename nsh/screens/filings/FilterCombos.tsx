// The four multi-select filters: taxpayers, forms, periods and statuses. Inline in the wide bar, stacked with labels in the
// narrow Filters sheet. A null selection means every value (including ones that appear later); the kit's MultiCombo reports null
// once every box is ticked. Options come from every return, not only the ones the filters keep, as the old combo boxes did.
import type { JSX } from "nsh";
import { computed, View } from "nsh";
import type { ReturnRow } from "../../bir";
import { formatTin } from "../../lib/format";
import { store } from "../../shell";
import { Avatar, MultiCombo, Text } from "../../ui";
import { filters } from "./filters";
import { isDryRun } from "../../lib/statusWord";
import { type DotTone, type FormIndex, periodKey, periodRank, plural, STATUS_TONE, STATUSES } from "./model";

const DOT: Record<DotTone, string> = {
  accent: "size-2 shrink-0 rounded-full bg-accent-blue",
  quiet: "size-2 shrink-0 rounded-full bg-subtle-foreground",
  success: "size-2 shrink-0 rounded-full bg-success",
};

/** A combo and, when stacked, its label above it. */
function Slot(props: { label?: string; children?: JSX.Element }) {
  return (
    <View class="min-w-0 shrink items-start gap-1">
      <If when={props.label}>
        <Text variant="meta" tone="fg2">{props.label as string}</Text>
      </If>
      {props.children}
    </View>
  );
}

/** Props of FilterCombos. */
export interface FilterCombosProps {
  /** Every return, unfiltered. */
  all: ReturnRow[];
  index: FormIndex;
  /** Labels above each combo (the Filters sheet); off, they sit in a wrapping row. */
  stacked?: boolean;
}

/** Taxpayers, forms, periods and statuses, bound to Filings' filter state. */
export function FilterCombos(props: FilterCombosProps) {
  const taxpayers = computed(() =>
    store.taxpayers().map((tp) => ({
      value: tp.tin_root,
      label: tp.name,
      sub: tp.tin_display || formatTin(tp.tin_root),
      lead: () => <Avatar name={tp.name} size={20} hueKey={tp.tin_root} />,
    })),
  );
  const forms = computed(() => {
    const titles = new Map(props.index.map((f) => [f.c, f.t ?? ""]));
    return [...new Set(props.all.map((r) => r.form_code))].sort().map((c) => ({ value: c, label: c, sub: titles.get(c) ?? "" }));
  });
  const periods = computed(() => {
    const keys = props.all.map((r) => periodKey(props.index, r));
    return [...new Set(keys)]
      .sort((a, b) => periodRank(b) - periodRank(a))
      .map((k) => ({ value: k, label: k, sub: plural(keys.filter((x) => x === k).length, "return") }));
  });
  const statuses = computed(() => {
    const options: { value: string; label: string; sub: string; lead: () => JSX.Element }[] = STATUSES.map((s) => ({
      value: s,
      label: s,
      sub: plural(props.all.filter((r) => r.status === s && !isDryRun(r)).length, "return"),
      lead: () => <View class={DOT[STATUS_TONE[s]]} />,
    }));
    const tests = props.all.filter((r) => isDryRun(r)).length;
    if (tests) options.push({ value: "Test sends", label: "Test sends", sub: plural(tests, "return"), lead: () => <View class={DOT.quiet} /> });
    return options;
  });
  const label = (text: string) => (props.stacked ? text : undefined);
  return (
    <View class={props.stacked ? "w-full gap-4" : "flex-row flex-wrap items-center gap-2"}>
      <Slot label={label("Taxpayers")}>
        <MultiCombo
          testID="filings-taxpayers"
          all="All taxpayers"
          nouns="taxpayers"
          icon="users"
          placeholder="Find a taxpayer"
          options={taxpayers()}
          selected={filters.tps()}
          onChange={filters.setTps}
        />
      </Slot>
      <Slot label={label("Forms")}>
        <MultiCombo
          testID="filings-forms"
          all="All forms"
          nouns="forms"
          icon="file-text"
          placeholder="Find a form"
          options={forms()}
          selected={filters.forms()}
          onChange={filters.setForms}
        />
      </Slot>
      <Slot label={label("Periods")}>
        <MultiCombo
          testID="filings-periods"
          all="All periods"
          nouns="periods"
          icon="calendar"
          placeholder="Find a period"
          width={320}
          options={periods()}
          selected={filters.periods()}
          onChange={filters.setPeriods}
        />
      </Slot>
      <Slot label={label("Statuses")}>
        <MultiCombo
          testID="filings-statuses"
          all="All statuses"
          nouns="statuses"
          icon="circle-check"
          placeholder="Find a status"
          width={320}
          options={statuses()}
          selected={filters.statuses()}
          onChange={filters.setStatuses}
        />
      </Slot>
    </View>
  );
}
