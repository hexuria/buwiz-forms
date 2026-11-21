// Rows and surfaces: ListGroup and ListRow, NameCell, EmptyRow, SettingRow, Card, PageHeader, Tabs (underlined).
// Rows inside a group are divided by hairlines: each row draws a top border except the first (`first:border-t-0`).
import type { JSX } from "nsh";
import { View } from "nsh";
import { Button, Tab, Tabs as BaseTabs } from "nsh/base";
import { cx, TOUCH } from "./look";
import { Line, PageTitle, Text } from "./text";

/** A bordered list (radius 12) whose rows are divided by hairlines. */
export function ListGroup(props: { children?: JSX.Element; testID?: string }) {
  return (
    <View testID={props.testID} class="w-full overflow-hidden rounded-(--radius-card) border border-border">
      {props.children}
    </View>
  );
}

const ROW = "w-full flex-row items-center gap-3 border-t border-border px-4 py-2 first:border-t-0";

/** A list row: at least 52 tall, 8 by 16 padding, items centred 12 apart; with `onPress` it is a button (hover fill). */
export function ListRow(props: { children?: JSX.Element; onPress?: () => void; accessibilityLabel?: string; testID?: string }) {
  return (
    <If
      when={props.onPress}
      else={
        <View testID={props.testID} class={ROW} style={{ minHeight: 52 }}>
          {props.children}
        </View>
      }
    >
      <Button
        testID={props.testID}
        accessibilityLabel={props.accessibilityLabel}
        class={cx(ROW, "enabled:hover:bg-hover active:bg-selected focus-visible:bg-hover")}
        style={{ minHeight: 52 }}
        onPress={props.onPress}
      >
        {props.children}
      </Button>
    </If>
  );
}

/** A two-line name: 13/500 title over a 12 fg3 line, both truncating; flex 1 unless `width` is given. */
export function NameCell(props: { title: string; sub?: string | null; dim?: boolean; width?: number }) {
  return (
    <View class={cx("min-w-0", props.width === undefined && "flex-1")} style={{ width: props.width }}>
      <Line variant="control" tone={props.dim ? "fg3" : "fg"}>{props.title}</Line>
      <If when={props.sub}>
        <Line variant="meta" tone="fg3">{props.sub as string}</Line>
      </If>
    </View>
  );
}

/** Centred empty-state text inside a list, with an optional action under it. */
export function EmptyRow(props: { text: string; action?: JSX.Element; testID?: string }) {
  return (
    <View testID={props.testID} class="items-center gap-3 px-4 py-10">
      <Text tone="fg3" class="text-center">{props.text}</Text>
      {props.action}
    </View>
  );
}

/** A settings row: title and hint on the left, the control on the right; hairline above unless first. */
export function SettingRow(props: { title: string; hint?: string; children?: JSX.Element; testID?: string }) {
  return (
    <View testID={props.testID} class="w-full flex-row items-center gap-4 border-t border-border py-3.5 first:border-t-0">
      <View class="min-w-0 flex-1">
        <Text variant="control">{props.title}</Text>
        <If when={props.hint}>
          <Text variant="meta" tone="fg3">{props.hint}</Text>
        </If>
      </View>
      {props.children}
    </View>
  );
}

/** A bordered card on the surface (radius 12, 16 in) that stacks SettingRows. */
export function Card(props: { children?: JSX.Element; testID?: string; class?: string }) {
  return (
    <View testID={props.testID} class={cx("rounded-(--radius-card) border border-border bg-card px-4 py-1", props.class)}>
      {props.children}
    </View>
  );
}

/** Props of PageHeader. */
export interface PageHeaderProps {
  title: string;
  /** A leading avatar (40). */
  avatar?: JSX.Element;
  /** A line under the title (a MetaLine). */
  meta?: JSX.Element;
  /** A sentence under the title, 13 fg2. */
  lede?: string;
  /** Trailing actions (AppButtons); they wrap under the title when the pane is narrow. */
  actions?: JSX.Element;
}

/** A page's header: optional avatar, a 20/600 title, a meta line or a lede, and trailing actions. */
export function PageHeader(props: PageHeaderProps) {
  return (
    <View class="w-full flex-row flex-wrap items-center gap-3.5">
      {props.avatar}
      <View class="flex-1" style={{ minWidth: 180 }}>
        <PageTitle>{props.title}</PageTitle>
        <If when={props.meta}>
          <View class="mt-0.5">{props.meta}</View>
        </If>
        <If when={props.lede}>
          <Text tone="fg2" class="mt-1.5">{props.lede}</Text>
        </If>
      </View>
      <If when={props.actions}>
        <View class="shrink-0 flex-row gap-2">{props.actions}</View>
      </If>
    </View>
  );
}

/** One tab of Tabs. */
export interface TabItem {
  value: string;
  label: string;
  /** A count in a pill after the label. */
  count?: string | number | null;
}

/**
 * Underlined tabs (Profile, Forms, Templates): 36 tall (44 on touch), a 2-point underline under the current one.
 * `fill`: the tabs share the row's width, each its count under its label, so five fit a 390 phone without scrolling.
 */
export function Tabs(props: { tabs: TabItem[]; value: string; onChange: (value: string) => void; testID?: string; fill?: boolean }) {
  return (
    <BaseTabs
      testID={props.testID}
      class="mt-5 mb-6 w-full flex-row gap-1 border-b border-border"
      indicatorClass="bottom-0 h-0.5 bg-foreground"
    >
      <Each of={props.tabs} key={(t) => t.value}>
        {(t) => (
          <Tab
            selected={t().value === props.value}
            onPress={() => props.onChange(t().value)}
            accessibilityLabel={t().label}
            class={cx(
              "group items-center rounded-t-(--radius-ctl) hover:bg-hover focus-visible:bg-hover",
              props.fill ? "grow justify-center gap-0.5 px-1" : "flex-row gap-1.5 px-2.5",
              // The underline is drawn under the tab: a fill over the chosen tab would hide it (hover and focus beat selected:, so say so).
              "selected:hover:bg-transparent selected:focus-visible:bg-transparent",
              props.fill ? "h-12" : TOUCH ? "h-11" : "h-9",
            )}
          >
            <Text variant="control" tone="fg2" lines={props.fill ? 1 : undefined} class="group-selected:text-foreground">{t().label}</Text>
            <If when={t().count !== undefined && t().count !== null && t().count !== ""}>
              <View class="rounded-full bg-hover px-1.5">
                <Text variant="cap" tone="fg2" class="tabular-nums">{String(t().count)}</Text>
              </View>
            </If>
          </Tab>
        )}
      </Each>
    </BaseTabs>
  );
}
