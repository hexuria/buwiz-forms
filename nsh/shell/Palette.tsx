// The command palette (Cmd/Ctrl+K): a search box over grouped rows. Up and Down move, Enter runs, Esc closes. Mounted only
// while it is open, so its key bindings live exactly that long.
import { keymap, Scroll, View } from "nsh";
import { Avatar, Dialog, EmptyRow, Icon, Kbd, NavRow, TextField, Text } from "../ui";
import { app } from "./app";
import { indexPalette, movePalette, paletteItems, paletteRow, runPaletteItem, type PaletteItem } from "./paletteItems";

/** The list as drawn rows: a heading where the group changes, then the item with its index. */
type Row = { kind: "group"; id: string; label: string } | { kind: "item"; id: string; index: number; item: PaletteItem };

function rows(): Row[] {
  const out: Row[] = [];
  let group = "";
  paletteItems().forEach((item, index) => {
    if (item.group !== group) {
      group = item.group;
      out.push({ kind: "group", id: `g:${group}`, label: group });
    }
    out.push({ kind: "item", id: `i${index}:${item.group}|${item.label}|${item.hint ?? ""}`, index, item });
  });
  return out;
}

/** A group heading: the one place the design keeps small capitals (old chrome.js renderPalette): 11/600 fg3, 10 above, 4 below, 10 in. */
function PaletteHeading(props: { label: string }) {
  return (
    <View class="px-2.5 pt-2.5 pb-1">
      <Text variant="cap" tone="fg3" class="font-semibold">
        {props.label.toUpperCase()}
      </Text>
    </View>
  );
}

export function Palette() {
  keymap.bind({ down: () => movePalette(1), up: () => movePalette(-1) });
  // The forms index is built as the palette opens, so the first typed letter filters at once like the rest.
  indexPalette();
  return (
    <Dialog testID="palette" open={true} onOpenChange={(open) => !open && app.closeOverlay()} top width={560}>
      {/* The old header: 48 tall, the icon and the Esc key beside a bare input (no box, so no ring while it has the keys). */}
      <View class="h-12 flex-row items-center gap-2.5 border-b border-border px-4">
        <Icon name="search" tone="fg3" />
        <View class="min-w-0 flex-1">
          <TextField
            testID="palette-input"
            bare
            accessibilityLabel="Search"
            placeholder="Search taxpayers, forms and pages"
            value={app.paletteQuery()}
            onChange={app.setPaletteQuery}
            autoFocus
            onSubmit={() => runPaletteItem(paletteRow())}
          />
        </View>
        <Kbd>Esc</Kbd>
      </View>
      <Scroll testID="palette-list" class="shrink" style={{ maxHeight: 420 }} contentClass="gap-0.5 p-3">
        <If when={paletteItems().length === 0}>
          <EmptyRow text="No matches. Try a form code like 2550Q." testID="palette-empty" />
        </If>
        <Each of={rows()} key={(r) => r.id}>
          {(r) => (
            <If when={r().kind === "group"} else={<ItemRow row={r()} />}>
              <PaletteHeading label={(r() as { label: string }).label} />
            </If>
          )}
        </Each>
      </Scroll>
    </Dialog>
  );
}

function ItemRow(props: { row: Row }) {
  const item = () => (props.row as { item: PaletteItem }).item;
  const index = () => (props.row as { index: number }).index;
  return (
    <NavRow
      testID={`palette-item-${index()}`}
      leading={
        item().avatar ? <Avatar name={item().avatar?.name ?? ""} size={20} hueKey={item().avatar?.root} /> : <Icon name={item().icon ?? "file-text"} />
      }
      label={item().label}
      size="list"
      current={index() === paletteRow()}
      trailing={item().hint ? <Text variant="meta" tone="fg3" class="tabular-nums">{item().hint}</Text> : undefined}
      onPress={() => runPaletteItem(index())}
    />
  );
}
