// Frame-only parts that the UI kit has no part for: the sidebar's search row and the narrow layout's tab button. Each is
// one nsh/base Button (presses, focus, selected, role) styled from tokens, like the kit's own NavRow.
import { View } from "nsh";
import { Button } from "nsh/base";
import { Icon, Kbd, Line, T_MIN_H, Text, TOUCH, type IconName } from "../ui";

const FOCUS_RING = "focus-visible:ring-2 focus-visible:ring-accent-blue";

/** The sidebar's search row: a bordered 32-point button with the search icon, "Search" and the shortcut. */
export function SearchRow(props: { shortcut: string; onPress: () => void }) {
  return (
    <Button
      testID="sb-search"
      accessibilityLabel="Search"
      class={`min-w-0 flex-1 flex-row items-center gap-2 rounded-(--radius-row) border border-border bg-card pr-2 pl-2.5 enabled:hover:border-border-strong ${FOCUS_RING} ${TOUCH ? T_MIN_H : "h-8"}`}
      onPress={props.onPress}
    >
      <Icon name="search" tone="fg3" />
      <Line tone="fg3" class="flex-1">Search</Line>
      <Kbd>{props.shortcut}</Kbd>
    </Button>
  );
}

/** One tab of the narrow layout's bottom bar: an icon over a label, the current one in the foreground ink. */
export function TabButton(props: { view: string; label: string; icon: IconName; current: boolean; onPress: () => void }) {
  return (
    <Button
      testID={`tab-${props.view}`}
      accessibilityLabel={props.label}
      selected={props.current}
      class={`group min-h-14 min-w-0 flex-1 items-center justify-center gap-0.5 py-1.5 enabled:hover:bg-hover active:bg-selected ${FOCUS_RING}`}
      onPress={props.onPress}
    >
      <Icon name={props.icon} tone="fg2" class="group-selected:tint-foreground" />
      <Text variant="cap" tone="fg2" class="group-selected:text-foreground group-selected:font-semibold">{props.label}</Text>
    </Button>
  );
}

/** A label, a value: one fact of the profile summary (icon and label on the left, 84 wide; the value takes the rest). */
export function FactRow(props: { icon: IconName; label: string; testID?: string; children?: import("nsh").JSX.Element }) {
  return (
    <View testID={props.testID} class="flex-row gap-3">
      <View class="w-21 shrink-0 flex-row items-center gap-1.5">
        <Icon name={props.icon} size={14} tone="fg3" />
        <Text variant="meta" tone="fg3">{props.label}</Text>
      </View>
      <View class="min-w-0 flex-1">{props.children}</View>
    </View>
  );
}
