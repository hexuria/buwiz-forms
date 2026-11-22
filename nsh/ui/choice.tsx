// Choice controls: Switch, Checkbox, CheckboxLabel, RadioGroup, Segmented, Dropdown (a select), ComboField
// (type-to-filter single choice for long lists: RDO, ZIP), MultiCombo (a multi-select filter in a popover).
import type { Accessor, JSX, ViewRef } from "nsh";
import { Scroll, signal, View } from "nsh";
import {
  Checkbox as BaseCheckbox, Combobox, Radio, RadioGroup as BaseRadioGroup, Select, type SelectEntry, Switch as BaseSwitch,
} from "nsh/base";
import { AppButton } from "./buttons";
import { TextField } from "./fields";
import { revealAndFocus } from "./focus";
import type { IconName } from "./icons";
import { Popover } from "./layers";
import { NameCell } from "./lists";
import { matchOptions, multiComboLabel, type MultiComboWords, normalizeSelection, type PickOption } from "./logic";
import { CTL, cx, FOCUS_RING, SMALL_HIT, TOUCH } from "./look";
import { Icon, Line, Text } from "./text";

// ---------------------------------------------------------------- switch

/** Props of Switch. */
export interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** What it turns on, for screen readers (and the visible label when `children` is none). */
  label: string;
  disabled?: boolean;
  /** A visible label after the track. */
  children?: JSX.Element;
  testID?: string;
}

/** An on/off switch: a 32 by 18 track, the accent when on, a 14-point knob. */
export function Switch(props: SwitchProps) {
  return (
    <BaseSwitch
      testID={props.testID}
      checked={props.checked}
      disabled={props.disabled}
      accessibilityLabel={props.label}
      hitSlop={SMALL_HIT}
      onChange={(e) => props.onChange(e.checked)}
      class={cx("flex-row items-center gap-2 rounded-full disabled:opacity-50", FOCUS_RING)}
      trackClass="h-4.5 w-8 justify-center rounded-full bg-border-strong px-0.5 checked:bg-accent-blue transition-colors duration-150"
      thumbClass="size-3.5 rounded-full bg-knob shadow-xs checked:translate-x-3.5 transition-transform duration-150"
    >
      {props.children}
    </BaseSwitch>
  );
}

// ---------------------------------------------------------------- checkbox

interface CheckboxBaseProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  indeterminate?: boolean;
  disabled?: boolean;
  testID?: string;
  class?: string;
  focusRef?: (ref: ViewRef) => void;
  children?: JSX.Element;
}

/** The one wrapper of nsh/base's Checkbox: a 16-point box, radius 4, the primary fill and a check when on. */
function CheckboxBase(props: CheckboxBaseProps) {
  return (
    <BaseCheckbox
      ref={(r: ViewRef) => props.focusRef?.(r)}
      testID={props.testID}
      checked={props.checked}
      indeterminate={props.indeterminate}
      disabled={props.disabled}
      accessibilityLabel={props.label}
      hitSlop={SMALL_HIT}
      onChange={(e) => props.onChange(e.checked)}
      class={cx("flex-row items-center gap-2 rounded-(--radius-ctl) disabled:opacity-50", FOCUS_RING, props.class)}
      indicatorClass="size-4 shrink-0 items-center justify-center rounded-sm border border-border-strong bg-card checked:border-primary checked:bg-primary indeterminate:border-primary indeterminate:bg-primary"
      indicator={
        <If when={props.checked || props.indeterminate}>
          <Icon name={props.indeterminate ? "minus" : "check"} size={12} tone="primaryFg" />
        </If>
      }
    >
      {props.children}
    </BaseCheckbox>
  );
}

/** Props of Checkbox. */
export interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** What it selects, for screen readers. */
  label: string;
  indeterminate?: boolean;
  disabled?: boolean;
  testID?: string;
  /** Receives the checkbox's ref: `focus()` gives it the keyboard focus (macOS, the web), where Space or Return toggles it. */
  focusRef?: (ref: ViewRef) => void;
}

/** A checkbox alone (its label is for screen readers): a table cell, a row's leading box. */
export function Checkbox(props: CheckboxProps) {
  return (
    <CheckboxBase
      checked={props.checked}
      onChange={props.onChange}
      label={props.label}
      indeterminate={props.indeterminate}
      disabled={props.disabled}
      testID={props.testID}
      focusRef={props.focusRef}
    />
  );
}

/** A checkbox with its label to the right, the whole of it pressable. */
export function CheckboxLabel(props: CheckboxProps & { hint?: string }) {
  return (
    <CheckboxBase
      checked={props.checked}
      onChange={props.onChange}
      label={props.label}
      indeterminate={props.indeterminate}
      disabled={props.disabled}
      testID={props.testID}
      focusRef={props.focusRef}
      class={TOUCH ? "min-h-11" : undefined}
    >
      <View class="min-w-0 shrink">
        <Text>{props.label}</Text>
        <If when={props.hint}>
          <Text variant="meta" tone="fg3">{props.hint}</Text>
        </If>
      </View>
    </CheckboxBase>
  );
}

// ---------------------------------------------------------------- radio group

/** One option of a RadioGroup. */
export interface RadioOption {
  value: string;
  label: string;
  /** A line under the label: what it means. */
  hint?: string;
  disabled?: boolean;
}

/** Props of RadioGroup. */
export interface RadioGroupProps {
  options: RadioOption[];
  /** The checked option's value; null for none yet. */
  value: string | null;
  onChange: (value: string) => void;
  /** Side by side instead of stacked. */
  row?: boolean;
  testID?: string;
}

/** One choice of a few: 16-point circles, the primary dot on the checked one; arrow keys move through the set. */
export function RadioGroup(props: RadioGroupProps) {
  return (
    <BaseRadioGroup testID={props.testID} class={props.row ? "flex-row flex-wrap gap-x-6 gap-y-2" : "gap-2"}>
      <Each of={props.options} key={(o) => o.value}>
        {(o) => (
          <Radio
            checked={o().value === props.value}
            disabled={o().disabled}
            accessibilityLabel={o().label}
            hitSlop={SMALL_HIT}
            onChange={() => props.onChange(o().value)}
            class={cx("group flex-row items-start gap-2 rounded-(--radius-ctl) disabled:opacity-50", FOCUS_RING, TOUCH && "min-h-11 items-center")}
          >
            <View class="mt-0.5 size-4 shrink-0 items-center justify-center rounded-full border border-border-strong bg-card group-checked:border-primary">
              <View class="size-2 rounded-full bg-primary opacity-0 group-checked:opacity-100" />
            </View>
            <View class="min-w-0 shrink">
              <Text>{o().label}</Text>
              <If when={o().hint}>
                <Text variant="meta" tone="fg3">{o().hint}</Text>
              </If>
            </View>
          </Radio>
        )}
      </Each>
    </BaseRadioGroup>
  );
}

// ---------------------------------------------------------------- segmented

/** One segment of Segmented. */
export interface SegmentOption {
  value: string;
  label: string;
  icon?: IconName;
  /** The segment's own testID (a form radio's "fp-<key>"). */
  testID?: string;
  /** A segment that takes no pick (one choice of the group locked); a value or an accessor of it. */
  disabled?: boolean | Accessor<boolean>;
}

/** Props of Segmented. */
export interface SegmentedProps {
  options: SegmentOption[];
  value: string;
  onChange: (value: string) => void;
  /** What the choice is about: the set's name for screen readers. */
  label: string;
  /** A long row (twelve months) wraps in a narrow window. */
  wrap?: boolean;
  /** Full-width segments in a column: the labels cannot hold one line (long labels, a narrow room). */
  stacked?: boolean;
  /** Reveals and focuses the selected choice (or the first when unset) on mount. */
  autoFocus?: boolean;
  /** Receives the selected choice's ref (or the first when unset). */
  focusRef?: (ref: ViewRef) => void;
  testID?: string;
}

/**
 * A segmented choice: a 2-point track on the hover fill, 24-point segments, the chosen one raised. It is a one-of set,
 * so it is nsh/base's RadioGroup of Radios (role radiogroup named by `label`, each segment a radio with its checked
 * state; the arrow keys move through the set and check as they go, Space checks the focused one) — not a ToggleGroup,
 * whose toolbar pattern only moves focus and presses on Space/Return. A checked segment reports nothing (a radio cannot
 * uncheck itself): picking another segment is the only pick, as a radio group's is.
 */
export function Segmented(props: SegmentedProps) {
  const off = (d: SegmentOption["disabled"]) => (typeof d === "function" ? d() : !!d);
  return (
    <BaseRadioGroup
      testID={props.testID}
      accessibilityLabel={props.label}
      class={cx(
        "gap-0.5 self-start rounded-(--radius-row) bg-hover p-0.5",
        props.stacked ? "w-full flex-col" : cx("shrink-0 flex-row", props.wrap && "flex-wrap"),
      )}
    >
      <Each of={props.options} key={(o) => o.value}>
        {(o) => (
          <Radio
            ref={(r: ViewRef) => {
              if (o().value === (props.value || props.options[0]?.value)) {
                props.focusRef?.(r);
                if (props.autoFocus) revealAndFocus(r);
              }
            }}
            testID={o().testID ?? (props.testID ? `${props.testID}-${o().value}` : undefined)}
            checked={o().value === props.value}
            disabled={off(o().disabled)}
            accessibilityLabel={o().label}
            hitSlop={TOUCH ? { top: 10, bottom: 10 } : 0}
            onChange={() => props.onChange(o().value)}
            class={cx(
              "group flex-row items-center gap-1.5 rounded-(--radius-ctl) px-2.5 checked:bg-raised checked:shadow-xs disabled:opacity-50",
              // A stacked segment grows with its words: a long choice wraps inside the bar instead of running off its edge.
              props.stacked ? cx("w-full py-1", TOUCH ? "min-h-11" : "min-h-8") : "h-6",
              FOCUS_RING,
            )}
          >
            <If when={o().icon}>
              <Icon name={o().icon as IconName} size={14} class="group-checked:tint-foreground" />
            </If>
            <Text variant="metaStrong" tone="fg2" class={cx("group-hover:text-foreground group-checked:text-foreground", props.stacked && "min-w-0 flex-1")}>{o().label}</Text>
          </Radio>
        )}
      </Each>
    </BaseRadioGroup>
  );
}

// ---------------------------------------------------------------- shared list looks

/** The popup (pointer) or sheet (touch) a list of choices is in. */
const LIST_POPUP = TOUCH
  ? "rounded-t-(--radius-dialog) bg-card px-2 pt-2 pb-8"
  : "rounded-(--radius-card) border border-border bg-raised p-1 shadow-lg";
/** A row of a list of choices: lit where the keys or the pointer are, the chosen one on the selected fill. */
const LIST_ITEM = cx(
  "justify-center rounded-(--radius-ctl) px-2 focus:bg-hover selected:bg-selected disabled:opacity-50",
  TOUCH ? "min-h-11" : "min-h-7",
);

/** A trigger's frame: a 32-point box (44 on touch) on the surface, the strong border, radius 8. */
function triggerClass(o: { open: boolean; error?: boolean; disabled?: boolean }) {
  return cx(
    "flex-row items-center gap-2 rounded-(--radius-row) border bg-card pr-2 pl-2.5",
    CTL,
    o.error ? "border-danger" : o.open ? "border-accent-blue" : "border-border-strong",
    o.disabled ? "opacity-50" : "hover:bg-hover",
    "focus:border-accent-blue",
  );
}

// ---------------------------------------------------------------- dropdown

/** One choice of a Dropdown. */
export interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
}

/** Props of Dropdown. */
export interface DropdownProps {
  options: DropdownOption[];
  value: string | null;
  onChange: (value: string) => void;
  /** What the trigger says while nothing is chosen (default "Choose"). */
  placeholder?: string;
  /** What the choice is about, for screen readers. */
  label: string;
  /** The trigger's width in points; unset, its container's. */
  width?: number;
  /** A search field over the rows. */
  searchable?: boolean;
  disabled?: boolean;
  error?: boolean;
  testID?: string;
  /** Receives the trigger's ref: `focus()` gives it the keyboard focus (macOS, the web), where Return opens the list. */
  focusRef?: (ref: ViewRef) => void;
}

/** A select: a 32-point trigger with a chevron, the list in a popup at it (a sheet on touch). */
export function Dropdown(props: DropdownProps) {
  const open = signal(false);
  const current = () => props.options.find((o) => o.value === props.value);
  return (
    <Select
      testID={props.testID}
      items={props.options.map((o) => ({ value: o.value, title: o.label, disabled: o.disabled }))}
      value={props.value}
      onChange={(e) => {
        if (e.value !== null) props.onChange(e.value);
      }}
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      disabled={props.disabled}
      searchable={props.searchable}
      searchPlaceholder="Search"
      searchClass={cx("mb-1 rounded-(--radius-ctl) border border-border-strong bg-card px-2 text-body text-foreground placeholder:text-subtle-foreground", CTL)}
      placeholder={props.placeholder ?? "Choose"}
      accessibilityLabel={props.label}
      offset={4}
      margin={8}
      popupClass={LIST_POPUP}
      backdropClass="bg-scrim"
      listClass="max-h-70"
      itemClass={LIST_ITEM}
      renderItem={(item: SelectEntry) => <Text testID={props.testID ? `${props.testID}-opt-${item.value}` : undefined}>{item.title}</Text>}
      empty={<Text variant="meta" tone="fg3" class="px-2 py-1.5">Nothing matches.</Text>}
      trigger={
        <View
          ref={(r: ViewRef) => props.focusRef?.(r)}
          testID={props.testID ? `${props.testID}-trigger` : undefined}
          class={triggerClass({ open: open(), error: props.error, disabled: props.disabled })}
          style={{ width: props.width }}
        >
          <Line tone={current() ? "fg" : "fg3"} class="flex-1">{current()?.label ?? props.placeholder ?? "Choose"}</Line>
          <Icon name="chevron-down" size={14} tone="fg3" />
        </View>
      }
    />
  );
}

// ---------------------------------------------------------------- combo field

/** Props of ComboField. */
export interface ComboFieldProps {
  /** The choices: a code and its label ("050 - South Makati"); thousands are fine. */
  options: PickOption[];
  /** The chosen code; null or "" for none. */
  value: string | null;
  onChange: (value: string | null) => void;
  /** Default "Type to search". */
  placeholder?: string;
  /** What the choice is about, for screen readers. */
  label: string;
  /** The list's width in points (default the field's). */
  width?: number;
  disabled?: boolean;
  error?: boolean;
  /** A clear button while a value is chosen. */
  cleanable?: boolean;
  testID?: string;
}

/**
 * Type to filter, pick one: the list keeps the options whose label holds the typed text (a code, a place name),
 * ignoring case (nsh/base's listbox filter: one run of text, crates/nsh-base/src/listbox.rs); the list is virtualized,
 * so thousands of rows are fine. It has no focus handle: the input inside is NSH's own (ComboboxProps has `inputTestID` and no
 * input ref), and the Combobox's ref is its root view, whose focus() puts the keys nowhere (tried on macOS, headless).
 */
export function ComboField(props: ComboFieldProps) {
  const open = signal(false);
  return (
    <Combobox
      testID={props.testID}
      inputTestID={props.testID ? `${props.testID}-input` : undefined}
      items={props.options.map((o) => ({ value: o.value, title: o.label }))}
      value={props.value ? [props.value] : []}
      onChange={(e) => props.onChange(e.value[0] ?? null)}
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      disabled={props.disabled}
      cleanable={props.cleanable}
      clearButton={<Icon name="x" size={14} tone="fg3" />}
      clearButtonAccessibilityLabel={`Clear ${props.label}`}
      clearButtonClass="size-6 items-center justify-center rounded-full hover:bg-hover"
      placeholder={props.placeholder ?? "Type to search"}
      accessibilityLabel={props.label}
      offset={4}
      margin={8}
      class={cx(triggerClass({ open: open(), error: props.error, disabled: props.disabled }), "pr-1 focus-within:border-accent-blue")}
      style={{ width: props.width }}
      inputClass="h-full min-w-0 flex-1 text-body text-foreground placeholder:text-subtle-foreground"
      triggerButton={
        <View class="size-6 items-center justify-center rounded-(--radius-ctl) active:bg-selected">
          <Icon name="chevron-down" size={14} tone="fg3" />
        </View>
      }
      popupClass={LIST_POPUP}
      backdropClass="bg-scrim"
      listClass="max-h-70"
      itemClass={cx(LIST_ITEM, "py-1")}
      renderItem={(item: SelectEntry) => <Text testID={props.testID ? `${props.testID}-opt-${item.value}` : undefined}>{item.title}</Text>}
      empty={<Text variant="meta" tone="fg3" class="px-2 py-1.5">No match. Try a code or a place name.</Text>}
    />
  );
}

// ---------------------------------------------------------------- multi combo

/** One choice of a MultiCombo: a leading element (an Avatar, a status dot) besides the label and second line. */
export interface MultiComboOption extends PickOption {
  lead?: () => JSX.Element;
}

/** Props of MultiCombo. */
export interface MultiComboProps extends MultiComboWords {
  options: MultiComboOption[];
  /** The page's selection; null means every value, including ones that appear later. */
  selected: ReadonlySet<string> | null;
  /** The new selection, null once every option is checked. */
  onChange: (selected: Set<string> | null) => void;
  /** The closed trigger's leading icon. */
  icon?: IconName;
  /** The search field's placeholder ("Find a form"). */
  placeholder?: string;
  /** The popover's width (default 340). */
  width?: number;
  testID?: string;
}

/** A multi-select filter: a summary trigger and a popover with search, Select all, Clear and a checkbox row per value. */
export function MultiCombo(props: MultiComboProps) {
  const open = signal(false);
  const search = signal("");
  const on = () => (props.selected === null ? props.options : props.options.filter((o) => props.selected!.has(o.value)));
  const narrowed = () => on().length !== props.options.length;
  const label = () => multiComboLabel(props, props.options, props.selected);
  const current = () => new Set(props.selected === null ? props.options.map((o) => o.value) : props.selected);
  const found = () => matchOptions(props.options, search());
  const choose = (next: Set<string>) => props.onChange(normalizeSelection(next, props.options));
  const toggle = (value: string) => {
    const next = current();
    if (next.has(value)) next.delete(value);
    else next.add(value);
    choose(next);
  };
  const selectAll = () => {
    const next = current();
    for (const o of found()) next.add(o.value);
    choose(next);
  };
  const clear = () => {
    const next = current();
    for (const o of found()) next.delete(o.value);
    choose(next);
  };
  const lead = () => {
    const one = on();
    if (one.length === 1 && narrowed() && one[0].lead) return one[0].lead();
    return props.icon ? <Icon name={props.icon} /> : null;
  };
  return (
    <Popover
      testID={props.testID}
      open={open()}
      onOpenChange={(o) => {
        open.set(o);
        if (!o) search.set("");
      }}
      width={props.width ?? 340}
      trigger={
        <View
          testID={props.testID ? `${props.testID}-trigger` : undefined}
          role="button"
          accessibilityLabel={`${props.all}: ${label()}`}
          class={cx(
            "min-w-0 shrink flex-row items-center gap-2 rounded-(--radius-row) border bg-card pr-2 pl-2.5 hover:bg-hover",
            CTL,
            narrowed() || open() ? "border-subtle-foreground" : "border-border-strong",
          )}
          style={{ maxWidth: 220 }}
        >
          {lead()}
          <Line>{label()}</Line>
          <Icon name="chevron-down" size={14} tone="fg3" />
        </View>
      }
    >
      <View class="flex-row items-center gap-1 border-b border-border pb-2">
        <View class="min-w-0 flex-1">
          <TextField
            size="sm"
            icon="search"
            value={search()}
            onChange={(v) => search.set(v)}
            placeholder={props.placeholder ?? "Search"}
            accessibilityLabel={props.placeholder ?? "Search"}
            testID={props.testID ? `${props.testID}-search` : undefined}
          />
        </View>
        <AppButton label="Select all" variant="ghost" size="sm" testID={props.testID ? `${props.testID}-all` : undefined} onPress={selectAll} />
        <AppButton label="Clear" variant="ghost" size="sm" testID={props.testID ? `${props.testID}-clear` : undefined} onPress={clear} />
      </View>
      <Scroll keyboardShouldPersistTaps="handled" class="max-h-75" contentClass="gap-0.5">
        <If when={found().length === 0}>
          <Text variant="meta" tone="fg3" class="p-2">Nothing matches.</Text>
        </If>
        <Each of={found()} key={(o) => o.value}>
          {(o) => (
            <CheckboxBase
              checked={props.selected === null || props.selected.has(o().value)}
              onChange={() => toggle(o().value)}
              label={o().label}
              testID={props.testID ? `${props.testID}-opt-${o().value}` : undefined}
              class={cx("px-2 enabled:hover:bg-selected", TOUCH ? "min-h-11" : o().sub ? "min-h-9" : "min-h-7")}
            >
              {o().lead?.()}
              <NameCell title={o().label} sub={o().sub} />
            </CheckboxBase>
          )}
        </Each>
      </Scroll>
    </Popover>
  );
}
