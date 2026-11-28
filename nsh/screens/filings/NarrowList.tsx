// The narrow layout's list: the cards of the board as rows under status tabs (All, Draft, Queued, Submitted, Confirmed).
import { computed, View } from "nsh";
import type { ReturnRow } from "../../bir";
import { EmptyRow, ListGroup } from "../../ui";
import { filters } from "./filters";
import { COLUMNS, columnRows, type FormIndex } from "./model";
import { ReturnCard } from "./ReturnCard";

/** Props of NarrowList. */
export interface NarrowListProps {
  /** The returns the filters keep, all tabs. */
  rows: ReturnRow[];
  index: FormIndex;
  onOpen: (row: ReturnRow) => void;
  onCancel: (row: ReturnRow) => void;
}

/** The returns of the chosen tab, newest change first. */
export function NarrowList(props: NarrowListProps) {
  const column = () => COLUMNS.find((c) => c.id === filters.tab());
  const shown = computed(() => {
    const c = column();
    return c ? columnRows(props.rows, c) : props.rows;
  });
  return (
    <View testID="filings-rows" class="w-full gap-2">
      <If
        when={shown().length > 0}
        else={
          <ListGroup>
            <EmptyRow testID="filings-tab-empty" text="Nothing here." />
          </ListGroup>
        }
      >
        <Each of={shown()} key={(r) => r.id}>
          {(r) => <ReturnCard row={r()} index={props.index} column={column()?.title ?? null} onOpen={props.onOpen} onCancel={props.onCancel} />}
        </Each>
      </If>
    </View>
  );
}
