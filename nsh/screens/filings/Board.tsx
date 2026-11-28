// The wide board: one column per status group, each with its title, a count and a card per return. Paid sits in Confirmed.
import { computed, View } from "nsh";
import type { ReturnRow } from "../../bir";
import { Chip, Text } from "../../ui";
import { type Column, columnRows, COLUMNS, type FormIndex } from "./model";
import { ReturnCard } from "./ReturnCard";

/** Props of Board. */
export interface BoardProps {
  /** The returns the filters keep. */
  rows: ReturnRow[];
  index: FormIndex;
  onOpen: (row: ReturnRow) => void;
  onCancel: (row: ReturnRow) => void;
}

function BoardColumn(props: BoardProps & { column: Column }) {
  const list = computed(() => columnRows(props.rows, props.column));
  return (
    <View
      testID={`filings-col-${props.column.id}`}
      class="min-w-0 flex-1 basis-0 gap-2 rounded-(--radius-card) border border-border bg-background p-2.5"
    >
      <View class="flex-row items-center gap-1.5 px-0.5">
        <Text variant="control" class="font-semibold">{props.column.title}</Text>
        <Chip label={String(list().length)} />
      </View>
      <If
        when={list().length > 0}
        else={<Text variant="meta" tone="fg3" class="px-0.5 py-2">Nothing here.</Text>}
      >
        <Each of={list()} key={(r) => r.id}>
          {(r) => <ReturnCard row={r()} index={props.index} column={props.column.title} onOpen={props.onOpen} onCancel={props.onCancel} />}
        </Each>
      </If>
    </View>
  );
}

/** Four columns side by side, as tall as their cards; the page scrolls. */
export function Board(props: BoardProps) {
  return (
    <View testID="filings-board" class="mt-4 w-full flex-row items-start gap-3">
      <Each of={COLUMNS} key={(c) => c.id}>
        {(c) => <BoardColumn column={c()} rows={props.rows} index={props.index} onOpen={props.onOpen} onCancel={props.onCancel} />}
      </Each>
    </View>
  );
}
