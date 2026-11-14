// A schedule table (items with rows, cols and keys), NOT a sideways grid: one compact line per row that holds something (its
// number and the first meaningful cells), a press on a line opens that row in the kit Sheet (a drawer from the right when wide,
// a bottom sheet when narrow) with the row's boxes stacked, Delete row and Done. "Add row" opens the first empty row. Computed
// total rows (every drawn column computed) are not rows to open: they stay visible under the list.
import { View } from "nsh";
import { AppButton, Code, Icon, ListGroup, ListRow, Sheet, Text } from "../../../ui";
import type { FieldControlProps } from "../contract";
import { FieldControl } from "./FieldControl";
import { readText, rowParts, rowTitleOf } from "./model";
import type { ScheduleInput } from "./types";

/** The testID of a row's line: "fp-table-<first key>" becomes "fp-row-<first key>-<n>". */
function rowId(tableId: string, row: number): string {
  return `${tableId.replace(/^fp-table-/, "fp-row-")}-${row}`;
}

/** A table whose every drawn column is computed: its rows are totals, not something to type into. */
function isTotalTable(p: ScheduleInput): boolean {
  return p.columns.length > 0 && p.columns.every((c) => !!c.col.c);
}

/** One column of the open row: its words above, its control under them, across the room the drawer has. */
function DrawerCell(props: { p: ScheduleInput; row: number; column: ScheduleInput["columns"][number] }) {
  const base = props.p.cell(props.row, props.column.index);
  const tick = props.column.col.t === "chk" || props.column.col.t === "radio";
  // A tick box has no label above it: its own words (the column's) sit beside it.
  const cell: FieldControlProps = { ...base, narrow: () => true, label: tick ? props.column.col.l : base.label };
  return (
    <View class="gap-1" testID={`${props.p.testID}-cell-${props.column.index}`}>
      <If when={!tick}>
        <Text variant="meta" tone="fg2">{props.column.col.l}</Text>
      </If>
      <FieldControl {...cell} disabled={props.p.locked} />
    </View>
  );
}

/** The open row's boxes, stacked. */
function DrawerBody(props: { p: ScheduleInput; row: number }) {
  return (
    <View class="gap-4">
      <Each of={props.p.columns} key={(c) => c.index}>
        {(c) => <DrawerCell p={props.p} row={props.row} column={c()} />}
      </Each>
    </View>
  );
}

/** A schedule: the rows that hold something as lines, Add row, the totals, and the drawer a row opens in. */
export function ScheduleControl(props: ScheduleInput) {
  const total = (_row: number) => isTotalTable(props);
  // The first shown row that holds nothing is the one "Add row" opens; the lines are the rows that hold something.
  const empty = () => props.shownRows().find((r) => !total(r) && props.summary(r)() === "");
  const lines = () => props.shownRows().filter((r) => !total(r) && r !== empty());
  const totals = () => (props.item.keys ?? []).map((_row, r) => r).filter(total);
  const open = () => props.openRow();
  const openRows = () => (open() === null ? [] : [open() as number]);
  const titleOf = (r: number) => (props.rowTitle ? props.rowTitle(r) : rowTitleOf(props.item, r));
  const remove = () => {
    const r = open();
    if (r === null) return;
    props.onClearRow(r);
    props.onOpenRow(null);
  };
  return (
    <View testID={props.testID} class="min-w-0 gap-2">
      <If
        when={lines().length > 0}
        else={
          <Text variant="meta" tone="fg3" testID={`${props.testID}-none`}>No rows yet.</Text>
        }
      >
        <ListGroup testID={`${props.testID}-rows`}>
          <Each of={lines()} key={(r) => r}>
            {(r) => (
              <ListRow
                testID={rowId(props.testID, r())}
                accessibilityLabel={`Row ${rowParts(props.item, r()).n}, ${props.summary(r())()}. Open`}
                onPress={() => props.onOpenRow(r())}
              >
                <View class="h-5.5 min-w-7.5 items-center justify-center rounded-(--radius-ctl) bg-hover px-1.5">
                  <Code tone="fg2">{String(rowParts(props.item, r()).n)}</Code>
                </View>
                <View class="min-w-0 flex-1">
                  <If when={rowParts(props.item, r()).caption}>
                    <Text variant="meta" tone="fg3" lines={1}>{rowParts(props.item, r()).caption}</Text>
                  </If>
                  <Text lines={2}>{props.summary(r())()}</Text>
                </View>
                <Icon name="chevron-right" tone="fg3" />
              </ListRow>
            )}
          </Each>
        </ListGroup>
      </If>
      <If when={!props.locked() && empty() !== undefined}>
        <View class="flex-row">
          <AppButton
            testID={`${props.testID}-add`}
            icon="plus"
            label="Add row"
            size="sm"
            onPress={() => {
              const r = empty();
              if (r !== undefined) props.onOpenRow(r);
            }}
          />
        </View>
      </If>
      <Each of={totals()} key={(r) => r}>
        {(r) => (
          <View testID={`${props.testID}-total-${r()}`} class="gap-1 rounded-(--radius-row) border border-border bg-hover px-3 py-2.5">
            <Text variant="metaStrong" tone="fg2">{props.rowName(r())}</Text>
            <Each of={props.columns} key={(c) => c.index}>
              {(c) => (
                <View class="flex-row items-baseline justify-between gap-3">
                  <Text variant="meta" tone="fg2" class="min-w-0 flex-1">{c().col.l}</Text>
                  <Text class="font-semibold tabular-nums">{readText(props.cell(r(), c().index))}</Text>
                </View>
              )}
            </Each>
          </View>
        )}
      </Each>
      <Sheet
        testID={`${props.testID}-drawer`}
        open={open() !== null}
        onOpenChange={(o) => {
          if (!o) props.onOpenRow(null);
        }}
        title={open() === null ? "" : titleOf(open() as number)}
        footer={
          <>
            <AppButton
              testID={`${props.testID}-delete`}
              icon="trash"
              variant="danger"
              label="Delete row"
              disabled={props.locked() || open() === null || props.summary(open() as number)() === ""}
              onPress={remove}
            />
            <AppButton testID={`${props.testID}-done`} variant="primary" label="Done" onPress={() => props.onOpenRow(null)} />
          </>
        }
      >
        <Each of={openRows()} key={(r) => r}>
          {(r) => <DrawerBody p={props} row={r()} />}
        </Each>
      </Sheet>
    </View>
  );
}
