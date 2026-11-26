// A searchable list for a long choice (ATC codes, RDOs, regions, cities): a trigger that looks like the kit's Dropdown, a
// search box that matches every typed word in any order (the kit's ComboField matches one run of text only), and the
// matches as rows. Wide: a popover at the trigger. In the phone layout (every touch screen, and a desktop window under 600 points, as the kit Sheet does): a bottom sheet.
import type { Accessor, ViewRef } from "nsh";
import { signal, View } from "nsh";
import { Icon, Line, ListGroup, ListRow, phoneLayout, Popover, Sheet, T_MIN_H, Text, TextField, TOUCH } from "../../../ui";
import type { ChoiceOption } from "../contract";
import { cx, matchWords, splitLabel } from "./model";

/** The most rows one search shows; a longer match list asks for more words. */
export const PICK_LIMIT = 60;

/** Props of SearchPick. */
export interface SearchPickProps {
  /** "fp-<key>": the base popover's id; the trigger's is "<testID>-trigger", the search "<testID>-search", a row "<testID>-opt-<value>". */
  testID: string;
  /** What the choice is about: the sheet's title and the screen readers' name. */
  label: string;
  /** Every choice, the empty one ("Choose…", value "") first. */
  options: ChoiceOption[];
  /** The chosen value ("" none). */
  value: string;
  onPick: (value: string) => void;
  width?: number;
  error?: boolean;
  disabled?: boolean;
  /** The "Clear the choice" row shows for a chosen value (default true); a required group passes false. */
  clearable?: boolean;
  /** Fires when the list opens. */
  onOpen?: () => void;
  /** Receives the trigger's ref, so a pill step can put the keyboard on it. */
  focusRef?: (ref: ViewRef) => void;
}

/** The search box and the rows, the same in the popover and in the sheet. */
function PickPanel(props: { p: SearchPickProps; query: Accessor<string>; setQuery: (q: string) => void; choose: (value: string) => void }) {
  const p = props.p;
  const real = () => p.options.filter((o) => o.value !== "");
  const found = () => matchWords(real(), props.query(), PICK_LIMIT);
  const total = () => matchWords(real(), props.query()).length;
  const showClear = () => p.value !== "" && p.clearable !== false && props.query().trim() === "";
  return (
    <View class="gap-2">
      <TextField
        size="sm"
        icon="search"
        autoFocus
        value={props.query()}
        onChange={(v) => props.setQuery(v)}
        placeholder="Search by code or words"
        accessibilityLabel={`Search ${p.label}`}
        testID={`${p.testID}-search`}
      />
      <ListGroup testID={`${p.testID}-list`}>
        <If when={showClear()}>
          <ListRow testID={`${p.testID}-opt-`} accessibilityLabel="Clear the choice" onPress={() => props.choose("")}>
            <Text tone="fg3" class="flex-1">Clear the choice</Text>
          </ListRow>
        </If>
        <Each of={found()} key={(o) => o.value}>
          {(o) => (
            <ListRow
              testID={`${p.testID}-opt-${o().value}`}
              accessibilityLabel={o().disabled ? `${o().label}, unavailable` : o().label}
              onPress={o().disabled ? undefined : () => props.choose(o().value)}
            >
              <View class={cx("min-w-0 flex-1", o().disabled && "opacity-50")}>
                <Text variant="control">{splitLabel(o().label)[0]}</Text>
                <If when={splitLabel(o().label)[1]}>
                  <Text variant="meta" tone="fg2">{splitLabel(o().label)[1]}</Text>
                </If>
              </View>
              <If when={o().value === p.value}>
                <Icon name="check" tone="fg" />
              </If>
            </ListRow>
          )}
        </Each>
      </ListGroup>
      <If when={found().length === 0}>
        <Text variant="meta" tone="fg3" class="px-1">Nothing matches. Try a code or other words.</Text>
      </If>
      <If when={total() > PICK_LIMIT}>
        <Text variant="meta" tone="fg3" class="px-1">{`Showing ${PICK_LIMIT} of ${total()}. Type more words to narrow it.`}</Text>
      </If>
    </View>
  );
}

/** A choice from a long list, searched by words. */
export function SearchPick(props: SearchPickProps) {
  const open = signal(false);
  const sheet = signal(false);
  const query = signal("");
  const small = () => phoneLayout();
  const current = () => props.options.find((o) => o.value === props.value && o.value !== "");
  const close = () => {
    open.set(false);
    sheet.set(false);
    query.set("");
  };
  const choose = (value: string) => {
    close();
    props.onPick(value);
  };
  const asked = (on: boolean) => {
    if (props.disabled) return;
    if (!on) return close();
    if (small()) sheet.set(true);
    else open.set(true);
    props.onOpen?.();
  };
  return (
    <>
      <Popover
        testID={props.testID}
        open={!small() && open()}
        onOpenChange={asked}
        width={Math.max(props.width ?? 0, 380)}
        trigger={
          <View
            ref={(r: ViewRef) => props.focusRef?.(r)}
            testID={`${props.testID}-trigger`}
            role="button"
            accessibilityLabel={`${props.label}: ${current()?.label ?? "nothing chosen"}`}
            class={cx(
              "min-w-0 flex-row items-center gap-2 rounded-(--radius-row) border bg-card pr-2 pl-2.5",
              TOUCH ? T_MIN_H : "h-8",
              props.error ? "border-danger" : open() || sheet() ? "border-accent-blue" : "border-border-strong",
              props.disabled ? "opacity-50" : "hover:bg-hover",
            )}
            style={{ width: props.width }}
          >
            <Line tone={current() ? "fg" : "fg3"} class="flex-1">{current()?.label ?? "Choose…"}</Line>
            <Icon name="chevron-down" size={14} tone="fg3" />
          </View>
        }
      >
        <PickPanel p={props} query={query} setQuery={(q) => query.set(q)} choose={choose} />
      </Popover>
      <Sheet testID={`${props.testID}-sheet`} open={small() && sheet()} onOpenChange={asked} title={props.label}>
        <PickPanel p={props} query={query} setQuery={(q) => query.set(q)} choose={choose} />
      </Sheet>
    </>
  );
}
