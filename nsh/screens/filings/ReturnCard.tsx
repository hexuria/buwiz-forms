// One return as a card: "2551Q Q3 2026", whose it is, the branch when it is not the head office, a send that needs a word (held,
// retrying, needs attention, refused: a chip and the host's line), its date, and Cancel queue on a queued one. The whole card opens the return; the column (or tab) says the status, so the card repeats it only where
// the two differ (Paid under Confirmed, a dry run's "Dry run" under Submitted, or every card on the narrow list's All tab).
import { View } from "nsh";
import type { ReturnRow } from "../../bir";
import { statusChipName, statusWord } from "../../lib/statusWord";
import { AppButton, Chip, Line, PressCard, StatusChip, Text } from "../../ui";
import { cardLabel, dateLine, type FormIndex, namedBranch, returnTitle, sendChip } from "./model";

/** Props of ReturnCard. */
export interface ReturnCardProps {
  row: ReturnRow;
  index: FormIndex;
  /** The title of the column or tab the card sits in; null where every card shows its status. */
  column: string | null;
  onOpen: (row: ReturnRow) => void;
  onCancel: (row: ReturnRow) => void;
}

/** A return as a card. */
export function ReturnCard(props: ReturnCardProps) {
  return (
    <PressCard testID={`return-${props.row.id}`} label={cardLabel(props.row)} size="row" class="w-full gap-1" onPress={() => props.onOpen(props.row)}>
      <View testID={`return-${props.row.id}-head`} class="w-full flex-row items-center gap-1.5">
        <Line variant="emph" class="flex-1">{returnTitle(props.index, props.row)}</Line>
        <If when={props.column !== statusWord(props.row)}>
          <View testID={`return-${props.row.id}-status`} class="shrink-0">
            <StatusChip status={statusChipName(props.row)} />
          </View>
        </If>
      </View>
      <Line variant="meta" tone="fg2">{props.row.taxpayer_name}</Line>
      <If when={namedBranch(props.row)}>
        <Line variant="meta" tone="fg3">{namedBranch(props.row)}</Line>
      </If>
      <If when={sendChip(props.row)}>
        {(chip) => (
          <View testID={`return-${props.row.id}-send`} class="w-full flex-row items-center gap-1.5">
            <Chip testID={`return-${props.row.id}-send-chip`} label={chip().label} tone={chip().tone} />
            <Line variant="meta" tone="fg2" class="flex-1">{chip().line}</Line>
          </View>
        )}
      </If>
      <View testID={`return-${props.row.id}-meta`} class="w-full flex-row flex-wrap items-center gap-x-1.5 gap-y-0.5">
        <Text variant="cap" tone="fg3" class="shrink-0">{dateLine(props.row)}</Text>
        <If when={props.row.status === "Queued"}>
          <View class="ml-auto shrink-0">
            <AppButton
              testID={`return-${props.row.id}-cancel`}
              label="Cancel queue"
              variant="ghost"
              size="sm"
              onPress={() => props.onCancel(props.row)}
            />
          </View>
        </If>
      </View>
    </PressCard>
  );
}
