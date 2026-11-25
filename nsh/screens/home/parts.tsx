// Home's parts: the five status tiles, the 30-day strip, a deadline row, a filed-return row, a taxpayer row, an as-needed row.
// Built from nsh/ui; the tile is the kit's PressCard and the strip's day its PressCell.
import { View } from "nsh";
import type { AsNeededForm, AsNeededReturn, DeadlineRow, ReturnRow, Taxpayer } from "../../bir";
import { nextText, progressText, type DashboardTaxpayerRow, type StripDay, type TileSpec } from "../../lib/dashboard";
import { daysUntil, MONTHS, shortDate } from "../../lib/dates";
import { countsAsFiled, isDryRun, isOpenDeadline, statusWord } from "../../lib/statusWord";
import { store, today } from "../../shell";
import { AppButton, Avatar, Code, Icon, type IconName, Line, ListRow, NameCell, PressCard, PressCell, StatusChip, Text } from "../../ui";
import { cancelQueued, openDeadline, openListed, openUndated, startAsNeeded, useAsTemplate } from "./actions";
import { deadlineKey, statusKind, whoLine } from "./rows";

/** The width of the code column (a form code or its blank). */
const CODE_W = 68;
/** The progress bar's width. */
const BAR = 120;

// ---------------------------------------------------------------- tiles

/** One of the five counts: icon and label, the number, a caption. A press picks it or opens Filings; `picked` is the filled look with an x. */
export function StatTile(props: { spec: TileSpec; width: number; picked: boolean; onPress: () => void }) {
  return (
    <PressCard
      testID={`home-tile-${props.spec.key}`}
      selected={props.picked}
      label={`${props.spec.label}: ${props.spec.n}`}
      onPress={props.onPress}
      align="start"
      class="shrink-0 gap-0.5"
      style={{ width: props.width }}
    >
      <View class="w-full flex-row items-center gap-1.5">
        <Icon name={props.spec.icon as IconName} tone={props.spec.tone} />
        <Text variant="meta" tone="fg2" class="min-w-0 flex-1">{props.spec.label}</Text>
        <If when={props.picked}>
          <Icon name="x" size={14} tone="fg3" />
        </If>
      </View>
      <Text variant="page" class="tabular-nums">{String(props.spec.n)}</Text>
      <Text variant="meta" tone="fg3" class="w-full min-w-0">{props.spec.sub}</Text>
    </PressCard>
  );
}

// ---------------------------------------------------------------- strip

/** The dot of one open deadline: accent for a draft (or a dry run: started, nothing sent), warning within a week, quiet after. */
function Dot(props: { d: DeadlineRow }) {
  const tone = () => (props.d.status === "Draft" || isDryRun(props.d) ? "bg-accent-blue" : daysUntil(props.d.due_date, today()) <= 7 ? "bg-warning" : "bg-subtle-foreground");
  return <View class={`size-2 shrink-0 rounded-full ${tone()}`} />;
}

/** One day of the strip: its date (every day when wide, every seventh when narrow) and a dot per open deadline (five at most). */
function StripDayButton(props: { day: StripDay; index: number; everyDay: boolean; picked: boolean; onPress: () => void }) {
  const label = () => {
    const n = props.day.rows.length;
    return `${props.index === 0 ? "Today, " : ""}${MONTHS[props.day.date.getMonth()]} ${props.day.date.getDate()}: ${n ? `${n} deadline${n === 1 ? "" : "s"}` : "nothing due"}`;
  };
  return (
    <PressCell
      testID={`home-strip-${props.day.iso}`}
      selected={props.picked}
      label={label()}
      tooltip={label()}
      onPress={props.onPress}
      tone="outline"
      class="h-full flex-1 flex-col-reverse items-center gap-0.75 rounded-(--radius-ctl) pb-2"
    >
      <If when={props.everyDay || props.index % 7 === 0} else={<View class="h-3" />}>
        <Text variant="cap" tone={props.index === 0 ? "fg" : "fg3"} class="tabular-nums whitespace-nowrap">{String(props.day.date.getDate())}</Text>
      </If>
      <Each of={(props.day.rows as DeadlineRow[]).slice(0, 5)} key={deadlineKey}>
        {(d) => <Dot d={d()} />}
      </Each>
    </PressCell>
  );
}

/** The next 30 days: a column per day with a dot per open deadline, and the first and last day under it. */
export function DeadlineStrip(props: { days: StripDay[]; everyDay: boolean; pickedDay: string | null; onPickDay: (iso: string) => void }) {
  const first = () => props.days[0].date;
  const last = () => props.days[props.days.length - 1].date;
  return (
    <View class="w-full">
      <View testID="home-strip" class="w-full flex-row items-end gap-0.75 rounded-(--radius-card) border border-border px-3 pt-2.5" style={{ height: 92 }}>
        <Each of={props.days} key={(d) => d.iso}>
          {(day, i) => (
            <StripDayButton day={day()} index={i()} everyDay={props.everyDay} picked={props.pickedDay === day().iso} onPress={() => props.onPickDay(day().iso)} />
          )}
        </Each>
      </View>
      <View class="w-full flex-row justify-between px-3 pt-1">
        <Text variant="cap" tone="fg3">{`Today, ${MONTHS[first().getMonth()]} ${first().getDate()}`}</Text>
        <Text variant="cap" tone="fg3">{`${MONTHS[last().getMonth()]} ${last().getDate()}`}</Text>
      </View>
    </View>
  );
}

// ---------------------------------------------------------------- rows

/** The code column of a row: a form code, 68 wide. */
function CodeCell(props: { code: string }) {
  return (
    <View class="shrink-0" style={{ width: CODE_W }}>
      <Code>{props.code}</Code>
    </View>
  );
}

/** When a deadline falls: an overdue or due-soon chip for an open one, else "Today", "N days ago" or "In N days". */
function DueWhen(props: { d: DeadlineRow }) {
  const days = () => daysUntil(props.d.due_date, today());
  const open = () => isOpenDeadline(props.d);
  return (
    <If
      when={open() && days() < 0}
      else={
        <If
          when={open() && days() <= 14}
          else={
            <Text variant="meta" tone="fg2">
              {days() === 0 ? "Today" : days() < 0 ? `${-days()} days ago` : `In ${days()} days`}
            </Text>
          }
        >
          <StatusChip status="due_soon" days={days()} />
        </If>
      }
    >
      <StatusChip status="overdue" days={days()} />
    </If>
  );
}

/** A deadline row: date box, code, "<code> · <period>" over the taxpayer, when (not when compact), status. A press opens the return. */
export function DueRow(props: { d: DeadlineRow; compact: boolean }) {
  const month = () => MONTHS[Number(props.d.due_date.split("-")[1]) - 1].toUpperCase();
  const day = () => String(Number(props.d.due_date.split("-")[2]));
  return (
    <ListRow
      testID={deadlineKey(props.d)}
      accessibilityLabel={`${props.d.form_code} ${props.d.period_label}, ${whoLine(props.d)}`}
      onPress={() => openDeadline(props.d)}
    >
      <View class="shrink-0 items-center" style={{ width: 44 }}>
        <Text variant="cap" tone="fg3" class="font-semibold">{month()}</Text>
        <Text variant="title" class="tabular-nums">{day()}</Text>
      </View>
      <If when={!props.compact}>
        <CodeCell code={props.d.form_code} />
      </If>
      <NameCell title={`${props.d.form_code} · ${props.d.period_label}`} sub={whoLine(props.d)} />
      <If when={!props.compact}>
        <View class="shrink-0 flex-row" style={{ width: 96 }}>
          <DueWhen d={props.d} />
        </View>
      </If>
      <View class="shrink-0 flex-row justify-end" style={{ width: 88 }}>
        <StatusChip status={statusKind(statusWord(props.d))} />
      </View>
    </ListRow>
  );
}

/** What a filed return offers: Cancel queue while queued, its status or Use as template, then Open. */
function FiledActions(props: { r: ReturnRow }) {
  // A dry run is not filed: it shows its "Dry run" chip, as a Draft shows its own, and no Use as template.
  const filed = () => countsAsFiled(props.r);
  const hasTemplate = () => store.templates(props.r.tin).some((t) => t.form_code === props.r.form_code);
  return (
    <>
      <If when={props.r.status === "Queued"}>
        <AppButton testID={`return-${props.r.id}-cancel`} label="Cancel queue" variant="ghost" size="sm" onPress={() => cancelQueued(props.r)} />
      </If>
      <If
        when={hasTemplate() || !filed()}
        else={<AppButton testID={`return-${props.r.id}-use-template`} label="Use as template" icon="layout-template" size="sm" onPress={() => useAsTemplate(props.r)} />}
      >
        <StatusChip status={statusKind(statusWord(props.r))} />
      </If>
      <AppButton testID={`return-${props.r.id}-open`} label="Open" variant="ghost" size="sm" onPress={() => openListed(props.r)} />
    </>
  );
}

/**
 * A filed (or saved) return: code, "<code> · <period>" over the taxpayer, its date, and the actions. Compact (under 560): the
 * code (already in the title) and the date go, and the actions sit under the name instead of squeezing it.
 */
export function FiledRow(props: { r: ReturnRow; compact: boolean }) {
  return (
    <ListRow testID={`return-${props.r.id}`}>
      <If
        when={props.compact}
        else={
          <>
            <CodeCell code={props.r.form_code} />
            <NameCell title={`${props.r.form_code} · ${props.r.period_label}`} sub={whoLine(props.r)} />
            <View class="shrink-0" style={{ width: 104 }}>
              <Text variant="meta" tone="fg2" lines={1}>{shortDate(props.r.updated_at)}</Text>
            </View>
            <View class="shrink-0 flex-row items-center justify-end gap-1" style={{ minWidth: 232 }}>
              <FiledActions r={props.r} />
            </View>
          </>
        }
      >
        <View class="min-w-0 flex-1 gap-1.5">
          <View class="w-full flex-row">
            <NameCell title={`${props.r.form_code} · ${props.r.period_label}`} sub={`${whoLine(props.r)} · ${shortDate(props.r.updated_at)}`} />
          </View>
          <View class="w-full flex-row flex-wrap items-center justify-end gap-1">
            <FiledActions r={props.r} />
          </View>
        </View>
      </If>
    </ListRow>
  );
}

/** One taxpayer: avatar, filed this year (a bar when wide), the next open deadline, and the button that opens it. */
export function TaxpayerRow(props: { row: DashboardTaxpayerRow; compact: boolean }) {
  const tp = () => props.row.taxpayer as Taxpayer;
  const next = () => props.row.next as DeadlineRow | null;
  const nextDays = () => (next() ? daysUntil(next()!.due_date, today()) : 0);
  const pct = () => (props.row.due ? props.row.done / props.row.due : 0);
  return (
    <ListRow testID={`home-tp-${tp().tin_root}`}>
      <Avatar name={tp().name} size={28} hueKey={tp().tin_root} />
      <If
        when={props.compact}
        else={
          <>
            <NameCell title={tp().name} sub={tp().tin_display} width={210} />
            <View class="min-w-0 flex-1 gap-1" style={{ minWidth: BAR }}>
              <View testID={`home-tp-${tp().tin_root}-bar`} class="h-1.5 overflow-hidden rounded-full bg-hover" style={{ width: BAR }}>
                <View class="h-full rounded-full bg-success" style={{ width: Math.round(BAR * pct()) }} />
              </View>
              <Line variant="meta" tone="fg3">{progressText(props.row)}</Line>
            </View>
            <View class="shrink-0 gap-0.5" style={{ width: 170 }}>
              <Line variant="meta" tone={next() ? "fg2" : "fg3"}>{nextText(props.row.next)}</Line>
              <If when={next()}>
                <View class="flex-row">
                  <If when={nextDays() <= 14} else={<Text variant="meta" tone="fg3">{`In ${nextDays()} days`}</Text>}>
                    <StatusChip status="due_soon" days={nextDays()} />
                  </If>
                </View>
              </If>
            </View>
          </>
        }
      >
        <View class="min-w-0 flex-1">
          <Line variant="control">{tp().name}</Line>
          <Line variant="meta" tone="fg3">{progressText(props.row)}</Line>
          <Line variant="meta" tone={next() ? "fg2" : "fg3"}>{nextText(props.row.next)}</Line>
        </View>
      </If>
      <If when={next()}>
        <AppButton
          testID={`home-tp-${tp().tin_root}-next`}
          label={next()?.status === "Draft" || isDryRun(next() ?? { status: "" }) ? "Open next" : "Start next"}
          size="sm"
          onPress={() => openDeadline(next()!)}
        />
      </If>
    </ListRow>
  );
}

/**
 * An as-needed form a branch files: code, "<code> · <name>" over the taxpayer and when it is due, and Start a return. Compact:
 * the code goes and the button sits under the name.
 */
export function AsNeededHead(props: { f: AsNeededForm; compact: boolean }) {
  const id = () => `as-needed-${props.f.tin}-${props.f.form_code}`;
  const start = () => <AppButton testID={`${id()}-start`} label="Start a return" icon="plus" size="sm" onPress={() => startAsNeeded(props.f)} />;
  return (
    <ListRow testID={id()}>
      <If
        when={props.compact}
        else={
          <>
            <CodeCell code={props.f.form_code} />
            <NameCell title={`${props.f.form_code} · ${props.f.form_name}`} sub={`${whoLine(props.f)} · ${props.f.due_rule}`} />
            {start()}
          </>
        }
      >
        <View class="min-w-0 flex-1 gap-1.5">
          <View class="w-full flex-row">
            <NameCell title={`${props.f.form_code} · ${props.f.form_name}`} sub={whoLine(props.f)} />
          </View>
          <View class="w-full flex-row justify-end">{start()}</View>
        </View>
      </If>
    </ListRow>
  );
}

/** A saved as-needed return that holds no trigger date yet: it has no deadline, so it follows its form's row with Open. */
export function AsNeededUndated(props: { f: AsNeededForm; r: AsNeededReturn; compact: boolean }) {
  const id = () => `as-needed-${props.f.tin}-${props.f.form_code}-${props.r.draft_id}`;
  return (
    <ListRow testID={id()}>
      <If when={!props.compact}>
        <View class="shrink-0" style={{ width: CODE_W }} />
      </If>
      <NameCell title={props.f.trigger ? `No ${props.f.trigger.toLowerCase()} date yet` : "Saved return"} sub={`Saved ${shortDate(props.r.updated_at)}`} />
      <View class="shrink-0 flex-row justify-end" style={{ width: props.compact ? undefined : 88 }}>
        <StatusChip status={statusKind(statusWord(props.r))} />
      </View>
      <AppButton testID={`${id()}-open`} label="Open" variant="ghost" size="sm" onPress={() => openUndated(props.f, props.r)} />
    </ListRow>
  );
}
