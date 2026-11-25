// The rows of a deadline: DueRow (the List and the narrow Upcoming list, the same row as Home's: date box, code, name, when,
// status) and DayRow (the Month view's day panel: code, period over the taxpayer, one chip). A press opens the return.
import { View } from "nsh";
import type { DeadlineRow } from "../../bir";
import { daysUntil, MONTHS } from "../../lib/dates";
import { isDryRun, statusWord } from "../../lib/statusWord";
import { openReturn, today } from "../../shell";
import { Code, ListRow, NameCell, StatusChip, Text } from "../../ui";
import { isOpenStatus, kindOf, rowKey, whoLine } from "./rows";

/** Opens the return a deadline stands for: its taxpayer and branch become the open ones; a saved return opens exactly (two in one month stay two). */
export function openDeadline(d: DeadlineRow): void {
  openReturn({ tin: d.tin, code: d.form_code, year: d.taxable_year, period: d.period, returnKey: d.return_key });
}

/** When a row is due: "Today", "3 days ago", "In 5 days". */
function whenText(n: number): string {
  return n === 0 ? "Today" : n < 0 ? `${-n} days ago` : `In ${n} days`;
}

/** The chip a row ends in: overdue and due soon say how many days, the rest say where the return stands. */
function RowChip(props: { d: DeadlineRow }) {
  const n = () => daysUntil(props.d.due_date, today());
  const late = () => isOpenStatus(props.d) && n() < 0;
  const soon = () => isOpenStatus(props.d) && n() <= 14;
  return (
    <If when={soon()} else={<StatusChip status={kindOf(statusWord(props.d))} />}>
      <StatusChip status={late() ? "overdue" : "due_soon"} days={n()} />
    </If>
  );
}

/** Props of DueRow. */
export interface DueRowProps {
  d: DeadlineRow;
  /** Leaves out the "when" column (a pane under 560 points). */
  compact: boolean;
  testID?: string;
}

/**
 * A deadline row: date box, code, "<code> · <period>" over the taxpayer, when, status. Where the pane is narrow (`compact`) the
 * code column and the "when" column go: the title already starts with the code, and the one chip at the end says how late or
 * how soon a return is (else where it stands); a saved draft says so after the taxpayer when the chip says the time.
 */
export function DueRow(props: DueRowProps) {
  const n = () => daysUntil(props.d.due_date, today());
  const month = () => MONTHS[Number(props.d.due_date.split("-")[1]) - 1].toUpperCase();
  const day = () => String(Number(props.d.due_date.split("-")[2]));
  const open = () => isOpenStatus(props.d);
  const timed = () => open() && n() <= 14;
  // Where the one chip says the time, a started return says where it stands after the taxpayer: "· Draft", "· Dry run".
  const sub = () => (props.compact && timed() && props.d.status !== "Not started" ? `${whoLine(props.d)} · ${statusWord(props.d)}` : whoLine(props.d));
  return (
    <ListRow testID={props.testID ?? `calendar-row-${rowKey(props.d)}`} onPress={() => openDeadline(props.d)}>
      <View class="w-11 shrink-0 items-center">
        <Text variant="cap" tone="fg3" class="font-semibold">{month()}</Text>
        <Text variant="title" class="tabular-nums">{day()}</Text>
      </View>
      <If when={!props.compact}>
        <Code class="w-17 shrink-0">{props.d.form_code}</Code>
      </If>
      <NameCell title={`${props.d.form_code} · ${props.d.period_label}`} sub={sub()} />
      <If
        when={!props.compact}
        else={
          <View class="shrink-0">
            <RowChip d={props.d} />
          </View>
        }
      >
        <View class="w-24 shrink-0 flex-row">
          <If when={timed()} else={<Text variant="meta" tone="fg2">{whenText(n())}</Text>}>
            <RowChip d={props.d} />
          </If>
        </View>
        <View class="w-22 shrink-0 flex-row justify-end">
          <StatusChip status={kindOf(statusWord(props.d))} />
        </View>
      </If>
    </ListRow>
  );
}

/** A day-panel row's line under the period: the taxpayer, and "· Dry run" while its one chip says how late or soon it is. */
function dayRowSub(d: DeadlineRow): string {
  const n = daysUntil(d.due_date, today());
  return isDryRun(d) && n <= 14 ? `${whoLine(d)} · ${statusWord(d)}` : whoLine(d);
}

/** The Month view's day-panel row: code, period over the taxpayer, then when or status. */
export function DayRow(props: { d: DeadlineRow }) {
  return (
    <ListRow testID={`calendar-day-row-${rowKey(props.d)}`} onPress={() => openDeadline(props.d)}>
      <Code class="w-13 shrink-0">{props.d.form_code}</Code>
      <NameCell title={props.d.period_label} sub={dayRowSub(props.d)} />
      <View class="shrink-0">
        <RowChip d={props.d} />
      </View>
    </ListRow>
  );
}
