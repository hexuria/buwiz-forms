// Buttons and navigation rows, each on nsh/base's Button (presses, keyboard focus, disabled, selected, the role).
import type { JSX } from "nsh";
import { View } from "nsh";
import { Button } from "nsh/base";
import type { IconName } from "./icons";
import { CTL, CTL_SM, cx, FOCUS_RING, T_MIN_H, T_SIZE, TOUCH, type Tone } from "./look";
import { Tooltip } from "./layers";
import { Icon, Kbd, Line, Text } from "./text";

/** A button's look: primary (inverted ink), secondary (bordered surface), ghost (quiet), danger (soft tint). */
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

const VARIANT: Record<ButtonVariant, string> = {
  primary: "border-transparent bg-primary enabled:hover:opacity-90 active:opacity-80",
  secondary: "border-border-strong bg-card enabled:hover:bg-hover active:bg-selected selected:bg-selected",
  ghost: "border-transparent enabled:hover:bg-hover active:bg-selected selected:bg-selected",
  danger: "border-transparent bg-danger-soft enabled:hover:opacity-90 active:opacity-80",
};

const INK: Record<ButtonVariant, Tone> = { primary: "primaryFg", secondary: "fg", ghost: "fg2", danger: "danger" };

/** Props of AppButton. */
export interface AppButtonProps {
  label: string;
  /** Default secondary. */
  variant?: ButtonVariant;
  /** md: 32 tall, sm: 28 (both 44 on touch). */
  size?: "md" | "sm";
  /** Fills the container's width. */
  block?: boolean;
  /** A line under the label, inside the button: a dialog's choice and what it does. */
  hint?: string;
  /** A leading icon. */
  icon?: IconName;
  /** A leading element instead of the icon (an Avatar). */
  leading?: JSX.Element;
  /** An element after the label (a Kbd, a count). */
  trailing?: JSX.Element;
  disabled?: boolean;
  /** Pressed-in look while what it opens is open (secondary and ghost). */
  selected?: boolean;
  /** A tooltip; a label that says it all needs none. */
  tooltip?: string;
  /** What screen readers say, when the label is not enough. */
  accessibilityLabel?: string;
  testID?: string;
  onPress?: () => void;
}

/** The app's button: 13/500 label, optional icon, radius 6. */
export function AppButton(props: AppButtonProps) {
  const variant = () => props.variant ?? "secondary";
  const sm = () => props.size === "sm";
  const cls = () =>
    cx(
      "rounded-(--radius-ctl) border disabled:opacity-50",
      FOCUS_RING,
      VARIANT[variant()],
      props.block && "w-full",
      props.hint
        ? "flex-col items-start justify-center gap-0.5 px-3 py-2"
        : cx("flex-row items-center justify-center gap-1.5", sm() ? cx(CTL_SM, TOUCH ? "px-3" : "px-2.5") : cx(CTL, TOUCH ? "px-4" : "px-3")),
    );
  return (
    <Button
      testID={props.testID}
      class={cls()}
      disabled={props.disabled}
      selected={props.selected}
      accessibilityLabel={props.accessibilityLabel ?? props.label}
      tooltip={props.tooltip ? <Tooltip label={props.tooltip} /> : undefined}
      onPress={props.onPress}
    >
      <If
        when={props.hint}
        else={
          <>
            {props.leading ?? (props.icon ? <Icon name={props.icon} tone={INK[variant()]} /> : null)}
            <Line variant="control" tone={INK[variant()]}>{props.label}</Line>
            {props.trailing}
          </>
        }
      >
        <Text variant="control" tone={INK[variant()]}>{props.label}</Text>
        <Text variant="meta" tone={variant() === "primary" ? "primaryFg" : variant() === "danger" ? "danger" : "fg2"}>{props.hint}</Text>
      </If>
    </Button>
  );
}

/** Props of IconButton. */
export interface IconButtonProps {
  icon: IconName;
  /** Required: what it does, shown on hover and focus and said by screen readers. */
  tooltip: string;
  /** A shortcut shown after the tooltip ("⌘B"). */
  shortcut?: string;
  /** On (what it toggles is shown): the selected fill. */
  pressed?: boolean;
  disabled?: boolean;
  /** A 6-point accent dot at the top right (something new). */
  dot?: boolean;
  testID?: string;
  onPress?: () => void;
  /** A press went down on it, and that press ended (released, moved away or cancelled): SearchPick's chevron. */
  onPressDown?: () => void;
  onPressUp?: () => void;
}

/** A 28-point icon-only button (44 on touch): 16-point icon in fg2, the selected fill while pressed. */
export function IconButton(props: IconButtonProps) {
  return (
    <Button
      testID={props.testID}
      class={cx(
        "items-center justify-center rounded-(--radius-ctl) enabled:hover:bg-hover active:bg-selected selected:bg-selected disabled:opacity-50",
        FOCUS_RING,
        TOUCH ? T_SIZE : "size-7",
      )}
      selected={props.pressed}
      disabled={props.disabled}
      accessibilityLabel={props.tooltip}
      tooltip={<Tooltip label={props.tooltip} shortcut={props.shortcut} />}
      onPress={props.onPress}
      onPressDown={props.onPressDown}
      onPressUp={props.onPressUp}
    >
      <Icon name={props.icon} tone={props.pressed ? "fg" : props.disabled ? "fg3" : "fg2"} />
      <If when={props.dot}>
        <View class={cx("absolute size-1.5 rounded-full bg-accent-blue", TOUCH ? "top-2.5 right-2.5" : "top-1 right-1")} />
      </If>
    </Button>
  );
}

/** Props of NavRow. */
export interface NavRowProps {
  label: string;
  /** An icon name, or an element (an Avatar, a Code). */
  leading?: IconName | JSX.Element;
  /** The current destination: the selected fill, ink in fg. */
  current?: boolean;
  /** A count after the label, in fg3. */
  count?: string | number | null;
  /** A shortcut hint. */
  kbd?: string;
  trailing?: JSX.Element;
  tooltip?: string;
  testID?: string;
  /**
   * default: 32 tall, label 13/500 (the sidebar, the panels, Settings' sections). list: the rows of a list in a dialog (the command
   * palette, Start a return): at least 36 tall, 6 by 10 inside, label 13/400, as the old dialogs drew them. recent: the sidebar's Recent
   * rows: the label is 12/500 with the letters a little closer and may take a second line, so "2551Q Q3 2026 · Juana Santos" shows
   * whole in the 190 points beside the icon (the old row cut it at the SF font's width; Inter is wider).
   */
  size?: "default" | "list" | "recent";
  onPress?: () => void;
}

/** A sidebar or panel row: 32 tall (44 on touch), radius 8, the current one on the selected fill. */
export function NavRow(props: NavRowProps) {
  const list = () => props.size === "list";
  return (
    <Button
      testID={props.testID}
      class={cx(
        "group w-full flex-row items-center gap-2.5 rounded-(--radius-row) enabled:hover:bg-hover active:bg-selected selected:bg-selected",
        FOCUS_RING,
        list() ? (TOUCH ? cx(T_MIN_H, "px-2.5") : "min-h-9 px-2.5 py-1.5") : props.size === "recent" ? "min-h-8 px-2 py-1" : cx("px-2", CTL),
      )}
      selected={props.current}
      accessibilityLabel={props.label}
      tooltip={props.tooltip ? <Tooltip label={props.tooltip} /> : undefined}
      onPress={props.onPress}
    >
      {typeof props.leading === "string" ? (
        <Icon name={props.leading as IconName} class="group-selected:tint-foreground" />
      ) : (
        props.leading
      )}
      <If
        when={props.size === "recent"}
        else={
          <Line variant={list() ? "body" : "control"} tone={list() ? "fg" : "fg2"} class={list() ? "flex-1" : "flex-1 group-selected:text-foreground"}>
            {props.label}
          </Line>
        }
      >
        <Text variant="metaStrong" tone="fg2" lines={2} class="min-w-0 flex-1 tracking-tight group-selected:text-foreground">{props.label}</Text>
      </If>
      <If when={props.count !== undefined && props.count !== null && props.count !== ""}>
        <Text variant="meta" tone="fg3" class="tabular-nums">{String(props.count)}</Text>
      </If>
      <If when={props.kbd}>
        <Kbd>{props.kbd}</Kbd>
      </If>
      {props.trailing}
    </Button>
  );
}
