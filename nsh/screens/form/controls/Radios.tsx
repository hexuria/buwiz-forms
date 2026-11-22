// A one-answer group, drawn by how many choices it has: two to four as the kit's segmented bar (full-width segments in a
// column when the labels cannot hold one line in the room the row gives), five to twelve as a dropdown, more as a search
// pick. Every pick is reported as onPick(key); the session applies the group's rules (the ticked choice of a group of
// three or more unticks, a pair keeps one answer, siblings of the same host group untick). Clearing back to unanswered is
// offered only where the session unticks — an optional group of three or more: a Clear under the bar, a "None" entry in
// the dropdown, the search list's clear row.
import { View } from "nsh";
import { AppButton, Dropdown, Icon, Segmented, Text } from "../../../ui";
import { radioKind } from "./model";
import { SearchPick } from "./Pick";
import { read, type RadioInput } from "./types";

/** Points per character of a segment's label at the meta size: the one-line fit's estimate (no text measure exists). */
const CHAR_W = 7;
/** A segment's side padding and air, and the bar's own padding and gaps, in points. */
const SEG_AIR = 30;
const BAR_AIR = 14;
/** The room guessed when the page gave none (the gallery, a test): a phone's content, else a wide form column. */
const ROOM_NARROW = 320;
const ROOM_WIDE = 720;
/** The width of a dropdown or search trigger on a wide layout. */
const LIST_WIDTH = 320;

/** The width the one-line bar would need, estimated from its labels' lengths (the documented character rule). */
function barWidth(labels: string[]): number {
  return BAR_AIR + labels.reduce((w, l) => w + Math.max(1, l.length) * CHAR_W + SEG_AIR, 0);
}

/** One group of one-answer choices, by its size. */
export function RadioGroupControl(props: RadioInput) {
  // G1: a choice another answer keeps shut draws nothing (its key sits in `hidden`).
  const shut = () => read(props.hidden, [] as readonly string[]);
  const visible = () => props.choices.filter((c) => !shut().includes(c.key));
  const count = () => visible().length;
  const kind = () => radioKind(count());
  const locked = () => props.locked();
  const why = () => read(props.reason, "");
  const room = () => {
    const r = props.room?.() ?? 0;
    return r > 0 ? r : props.narrow() ? ROOM_NARROW : ROOM_WIDE;
  };
  // The bar stacks (full-width segments, one under another) when its labels cannot hold one line in its room.
  const stacked = () => kind() === "segmented" && barWidth(visible().map((c) => c.label)) > room();
  const clearable = () => props.clearable?.() ?? false;
  const value = () => props.selected() ?? "";
  const pick = (key: string) => props.onPick(key);
  // Unticking is the session's: re-picking the ticked choice of a group of three or more clears it.
  const clear = () => {
    const k = props.selected();
    if (k) props.onPick(k);
  };
  const showClear = () => kind() === "segmented" && clearable() && props.selected() !== null;
  const choiceOff = (key: string) => locked() || props.choiceLocked(key);
  const width = () => (props.narrow() ? undefined : LIST_WIDTH);
  return (
    <View class="min-w-0 gap-1" style={{ opacity: props.off() ? 0.45 : 1 }}>
      <If when={kind() === "segmented"}>
        <View testID={stacked() ? `${props.testID}-stacked` : undefined} class="min-w-0">
          <Segmented
            testID={props.testID}
            label={props.label}
            options={visible().map((c) => ({
              value: c.key,
              label: c.label,
              testID: c.testID,
              disabled: () => choiceOff(c.key),
            }))}
            value={value()}
            onChange={pick}
            stacked={stacked()}
            wrap={!props.narrow()}
            focusRef={(r) => props.focusRef?.(() => r.focus())}
          />
        </View>
        <If when={showClear()}>
          <AppButton variant="ghost" size="sm" label="Clear" testID={`${props.testID}-clear`} onPress={clear} />
        </If>
      </If>
      <If when={kind() === "dropdown"}>
        <View class="min-w-0" style={{ width: width() }}>
          <Dropdown
            testID={props.testID}
            label={props.label}
            options={[
              ...(clearable() ? [{ value: "", label: "None" }] : []),
              ...visible().map((c) => ({ value: c.key, label: c.label, disabled: props.choiceLocked(c.key) })),
            ]}
            value={props.selected()}
            placeholder="Choose…"
            width={width()}
            disabled={locked()}
            focusRef={(r) => props.focusRef?.(() => r.focus())}
            onChange={(v) => (v === "" ? clear() : pick(v))}
          />
        </View>
      </If>
      <If when={kind() === "search"}>
        <View class="min-w-0" style={{ width: width() }}>
          <SearchPick
            testID={props.testID}
            label={props.label}
            options={visible().map((c) => ({ value: c.key, label: c.label, disabled: props.choiceLocked(c.key) }))}
            value={value()}
            width={width()}
            disabled={locked()}
            clearable={clearable()}
            focusRef={(r) => props.focusRef?.(() => r.focus())}
            onPick={(v) => (v === "" ? clear() : pick(v))}
          />
        </View>
      </If>
      <If when={props.hint() || why()}>
        <View class="flex-row items-center gap-1.5">
          <Icon name="lock" size={12} tone="fg3" />
          <Text variant="meta" tone="fg3" class="min-w-0 flex-1">{props.hint() || why()}</Text>
        </View>
      </If>
    </View>
  );
}
