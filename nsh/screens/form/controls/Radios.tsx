// A one-answer group drawn as the paper form's tick boxes (the old page's radioGroup): square boxes with a check, so a
// ticked one can be pressed again. Every press is reported as a pick of that key; the session applies the group's rules
// (the ticked box of a group of three or more unticks, a pair keeps one answer, siblings of the same host group untick).
import type { ViewRef } from "nsh";
import { View } from "nsh";
import { CheckboxLabel, Icon, Text, TOUCH } from "../../../ui";
import { cx } from "./model";
import { read, type RadioInput } from "./types";

/** Labels longer than this stack the choices one under another, shorter ones sit side by side (the old rule). */
const STACK_FROM = 24;

/** One group of tick boxes: wrapped in a row when its labels are short, a column when long or narrow. */
export function RadioGroupControl(props: RadioInput) {
  const stacked = () => props.narrow() || props.choices.some((c) => c.label.length > STACK_FROM);
  const why = () => read(props.reason, "");
  // A pill step puts the keys in the ticked box, else the first one that takes a pick.
  const boxes = new Map<string, ViewRef>();
  props.focusRef?.(() => {
    const free = props.choices.find((c) => !props.locked() && !props.choiceLocked(c.key));
    (boxes.get(props.selected() ?? "") ?? (free ? boxes.get(free.key) : undefined))?.focus();
  });
  return (
    <View
      testID={props.testID}
      accessibilityLabel={props.label}
      class="min-w-0 gap-1"
      style={{ opacity: props.off() ? 0.45 : 1 }}
    >
      <View class={cx(stacked() ? "gap-y-1" : "flex-row flex-wrap gap-x-6 gap-y-2", TOUCH && "gap-y-0")}>
        <Each of={props.choices} key={(c) => c.key}>
          {(c) => (
            <CheckboxLabel
              testID={c().testID}
              label={c().label}
              focusRef={(ref) => boxes.set(c().key, ref)}
              checked={props.checked ? props.checked(c().key) : props.selected() === c().key}
              disabled={props.locked() || props.choiceLocked(c().key)}
              onChange={() => props.onPick(c().key)}
            />
          )}
        </Each>
      </View>
      <If when={props.hint() || why()}>
        <View class="flex-row items-center gap-1.5">
          <Icon name="lock" size={12} tone="fg3" />
          <Text variant="meta" tone="fg3" class="min-w-0 flex-1">{props.hint() || why()}</Text>
        </View>
      </If>
    </View>
  );
}
