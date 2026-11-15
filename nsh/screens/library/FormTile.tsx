// A form card of the Form library (old app/shell/library.js formCard): the code, the title in two lines, and chips for the
// frequency, the pages and the fields, plus "Draft layout" where the layout is still a draft (quality under 0.8).
// Kit gap: ui/ has no pressable tile, so the card is one nsh/base Button styled from tokens, as shell/parts.tsx does for its
// own parts. It moves into ui/ when the kit gets a tile.
import type { Accessor } from "nsh";
import { View } from "nsh";
import { Button } from "nsh/base";
import type { ScreenFormIndexEntry } from "../../bir";
import { Chip, Text } from "../../ui";

/** Props of FormTile. */
export interface FormTileProps {
  /** The form's index entry (an accessor, as `Each` gives it: read it inside JSX or a closure). */
  entry: Accessor<ScreenFormIndexEntry>;
  /** Pressed: open the form as a sample. */
  onOpen: () => void;
}

/** One form card; its testID is "library-<code>" and its label "Form <code>: <title>". */
export function FormTile(props: FormTileProps) {
  return (
    <Button
      testID={`library-${props.entry().c}`}
      accessibilityLabel={`Form ${props.entry().c}: ${props.entry().t}`}
      class="min-w-0 flex-col items-stretch justify-start gap-2 rounded-(--radius-card) border border-border bg-card px-4 py-3.5 enabled:hover:border-border-strong enabled:hover:shadow-md focus-visible:ring-2 focus-visible:ring-accent-blue"
      onPress={props.onOpen}
    >
      <View class="flex-row items-center gap-2">
        <Text variant="title">{props.entry().c}</Text>
        <View class="flex-1" />
        <If when={props.entry().q < 0.8}>
          <Chip label="Draft layout" tone="warning" />
        </If>
      </View>
      <Text variant="control" lines={2} class="h-9 leading-[18px]">{props.entry().t}</Text>
      <View class="flex-row flex-wrap gap-1.5">
        <Chip label={props.entry().freq} />
        <Chip label={`${props.entry().pages}p`} />
        <Chip label={`${props.entry().n} fields`} />
      </View>
    </Button>
  );
}
