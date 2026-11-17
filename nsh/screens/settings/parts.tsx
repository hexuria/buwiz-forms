// Small parts the Settings sections share: a label over a card, a row whose controls sit under its words (a PIN, its buttons),
// and a note row (a path, a sentence). The kit's SettingRow is the row with one control at its right.
import type { JSX } from "nsh";
import { View } from "nsh";
import { Text } from "../../ui";

/** A small fg3 label over what it names (a card, a block). */
export function Labeled(props: { label?: string; testID?: string; children?: JSX.Element }) {
  return (
    <View testID={props.testID} class="w-full gap-2">
      <If when={props.label}>
        <Text variant="metaStrong" tone="fg3">{props.label as string}</Text>
      </If>
      {props.children}
    </View>
  );
}

/** A settings row for a control too wide for the row's right (PIN cells and buttons): title and hint, the controls under them. */
export function StackRow(props: { title: string; hint?: string; testID?: string; children?: JSX.Element }) {
  return (
    <View testID={props.testID} class="w-full gap-3 border-t border-border py-3.5 first:border-t-0">
      <View class="min-w-0">
        <Text variant="control">{props.title}</Text>
        <If when={props.hint}>
          <Text variant="meta" tone="fg3">{props.hint as string}</Text>
        </If>
      </View>
      <View class="flex-row flex-wrap items-center gap-2">{props.children}</View>
    </View>
  );
}

/** A row that tells something: a title and a sentence under it (a folder, how closing works). */
export function NoteRow(props: { title: string; note: string; testID?: string }) {
  return (
    <View testID={props.testID} class="w-full gap-1 border-t border-border py-3.5 first:border-t-0">
      <Text variant="control">{props.title}</Text>
      <Text variant="meta" tone="fg3">{props.note}</Text>
    </View>
  );
}
