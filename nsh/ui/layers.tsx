// Layers: Tooltip, Dialog (centred, or top-anchored for the palette), Sheet (bottom when narrow, side drawer when
// wide), Popover, Menu with MenuItem and MenuSeparator, toasts (toast, undoToast and the Toaster that shows them),
// SidePanel, Disclosure and DisclosureGroup. Each is nsh/base's own overlay (focus, Escape, dismissal, roles), styled.
import type { JSX } from "nsh";
import { cleanup, keyboardFrame, Overlay, safeArea, Scroll, signal, View, windowSize } from "nsh";
import {
  Button, Collapsible, Dialog as BaseDialog, Menu as BaseMenu, MenuItem as BaseMenuItem, MenuSeparator as BaseMenuSeparator,
  Popover as BasePopover, Sheet as BaseSheet, Toast, type ToastEntry, ToastManager, ToastStack, Tooltip as BaseTooltip,
} from "nsh/base";
import { SIZE } from "../theme/tokens";
import { AppButton, IconButton } from "./buttons";
import type { IconName } from "./icons";
import { cx, FOCUS_RING, phoneLayout, T_MIN_H, TOUCH } from "./look";
import { Icon, Line, Text } from "./text";

type Placement = "top" | "bottom" | "left" | "right";
type Align = "start" | "center" | "end";

// ---------------------------------------------------------------- tooltip

/**
 * A tooltip's bubble, given as an element's `tooltip` prop: `tooltip={<Tooltip label="Hide sidebar" shortcut="⌘B" />}`.
 * The raised surface with a hairline, 12-point text, the shortcut in fg3.
 */
export function Tooltip(props: { label: string; shortcut?: string; placement?: Placement }) {
  return (
    <BaseTooltip
      placement={props.placement}
      class="flex-row items-center gap-2 rounded-(--radius-ctl) border border-border bg-raised px-2 py-1 shadow-md"
    >
      <Text variant="meta">{props.label}</Text>
      <If when={props.shortcut}>
        <Text variant="meta" tone="fg3">{props.shortcut}</Text>
      </If>
    </BaseTooltip>
  );
}

// ---------------------------------------------------------------- dialog

/** Props of Dialog. */
export interface DialogProps {
  open: boolean;
  /** The user closed it (Escape, the scrim, back): set `open` from it. */
  onOpenChange: (open: boolean) => void;
  /** Width in points (default 560), never wider than the window less 16 a side. */
  width?: number;
  /** Anchored 64 from the top instead of centred: the command palette, whose results change length while typing. */
  top?: boolean;
  /** A press on the scrim closes it (default true). */
  closeOnBackdropPress?: boolean;
  testID?: string;
  children?: JSX.Element;
}

/** A scrim and a dialog card: radius 14, hairline, the surface, a large shadow. The content is the caller's. */
export function Dialog(props: DialogProps) {
  const width = () => Math.min(props.width ?? 560, windowSize().width - 32);
  return (
    <BaseDialog
      testID={props.testID}
      open={props.open}
      animation="fade"
      onOpenChange={(e) => props.onOpenChange(e.open)}
      closeOnBackdropPress={props.closeOnBackdropPress ?? true}
      backdropClass="bg-scrim"
      class={cx("overflow-hidden rounded-(--radius-dialog) border border-border bg-card shadow-xl", props.top ? "mx-auto mt-16" : "m-auto")}
      style={{ width: width(), maxHeight: windowSize().height - (props.top ? 80 : 32) }}
    >
      {props.children}
    </BaseDialog>
  );
}

// ---------------------------------------------------------------- sheet

/** Props of Sheet. */
export interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The header's title; the header also holds the close button. */
  title?: string;
  /** A wide window's drawer width (default 480), never wider than the window less 48. */
  width?: number;
  /** Fixed under the scrolling body: the drawer's actions. */
  footer?: JSX.Element;
  testID?: string;
  children?: JSX.Element;
}

/**
 * The tallest a bottom sheet may be: below the status bar, and above the software keyboard while it is up. The kit keeps a sheet
 * there by itself, but a maxHeight in popupStyle takes the place of that limit (the kit's sheet.md, "The software keyboard"), so
 * every sheet of ours that sets one sets this: without it a sheet with a field in it rose under the status bar with the keyboard.
 */
/** The widest a bottom sheet is: on a tablet it is a sheet centred at the bottom, not a wall across the screen. */
export const SHEET_MAX_WIDTH = 720;

/** The side margins that centre a bottom sheet no wider than SHEET_MAX_WIDTH (0 on a phone). */
export function sheetSides(): { marginLeft: number; marginRight: number } {
  const side = Math.max(0, Math.floor((windowSize().width - SHEET_MAX_WIDTH) / 2));
  return { marginLeft: side, marginRight: side };
}

export function sheetMaxHeight(): number {
  const keyboard = Math.max(0, keyboardFrame().height - safeArea().bottom);
  return Math.max(160, windowSize().height - safeArea().top - keyboard - 24);
}

/**
 * Supplementary work in a panel: a bottom sheet in the phone layout (every phone and tablet, and a desktop window under 600 points),
 * at most 720 wide and centred; a drawer at the right edge on a wider desktop window.
 */
export function Sheet(props: SheetProps) {
  const narrow = () => phoneLayout();
  return (
    <BaseSheet
      testID={props.testID}
      open={props.open}
      onOpenChange={(e) => props.onOpenChange(e.open)}
      side={narrow() ? "bottom" : "right"}
      closeOnSwipe={narrow()}
      backdropClass="bg-scrim"
      // The sheet pads by the insets where IT renders (env(), the sheet's own), not the page's: a sheet is a layer of its own.
      popupClass={
        narrow()
          ? "rounded-t-(--radius-dialog) border border-b-0 border-border bg-card pb-[env(safe-area-inset-bottom)]"
          : "border-l border-border bg-card pt-[env(safe-area-inset-top)]"
      }
      popupStyle={
        narrow()
          ? { maxHeight: Math.min(windowSize().height * 0.9, sheetMaxHeight()), ...sheetSides() }
          : { width: Math.min(props.width ?? 480, windowSize().width - 48) }
      }
    >
      <If when={narrow()}>
        <View class="mt-2 h-1 w-9 self-center rounded-full bg-border-strong" />
      </If>
      <View class="flex-row items-center gap-2 border-b border-border py-2 pr-2 pl-4">
        <Text variant="emph" class="flex-1" lines={1}>{props.title ?? ""}</Text>
        <IconButton icon="x" tooltip="Close" onPress={() => props.onOpenChange(false)} />
      </View>
      <Scroll keyboardShouldPersistTaps="handled" class="shrink" contentClass="gap-4 p-4">
        {props.children}
      </Scroll>
      <If when={props.footer}>
        <View class="flex-row justify-end gap-2 border-t border-border px-4 py-3">{props.footer}</View>
      </If>
    </BaseSheet>
  );
}

// ---------------------------------------------------------------- popover

/** Props of Popover. */
export interface PopoverProps {
  /** What opens it when pressed; it is placed against it. */
  trigger: JSX.Element;
  /** Controlled open state; left out, the popover keeps its own. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  placement?: Placement;
  align?: Align;
  /** Width in points; unset, as wide as its content. */
  width?: number;
  testID?: string;
  children?: JSX.Element;
}

/** Contextual content anchored to a trigger: the raised surface, radius 12, 12 in. */
export function Popover(props: PopoverProps) {
  return (
    <BasePopover
      testID={props.testID}
      trigger={props.trigger}
      open={props.open}
      onOpenChange={(e) => props.onOpenChange?.(e.open)}
      placement={props.placement}
      align={props.align ?? "start"}
      offset={4}
      margin={8}
      popupClass="gap-2 rounded-(--radius-card) border border-border bg-raised p-3 shadow-lg"
      popupStyle={{ width: props.width }}
    >
      {props.children}
    </BasePopover>
  );
}

// ---------------------------------------------------------------- menu

/** Props of Menu. */
export interface MenuProps {
  /** What opens it: an element, or a function of whether it is open (to keep the trigger pressed while it is). */
  trigger: JSX.Element | ((open: () => boolean) => JSX.Element);
  placement?: Placement;
  align?: Align;
  testID?: string;
  /** MenuItems and MenuSeparators. */
  children?: JSX.Element;
}

/** A compact list of actions: a popup at the trigger with a pointer, a sheet from the bottom on touch. */
export function Menu(props: MenuProps) {
  const open = signal(false);
  const trigger = typeof props.trigger === "function" ? (props.trigger as (open: () => boolean) => JSX.Element)(open) : props.trigger;
  return (
    <BaseMenu
      testID={props.testID}
      trigger={trigger}
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      placement={props.placement}
      align={props.align}
      offset={4}
      margin={8}
      backdropClass="bg-scrim"
      popupClass={
        TOUCH
          ? "gap-0.5 rounded-t-(--radius-dialog) bg-card px-2 pt-2 pb-8"
          : "min-w-44 gap-0.5 rounded-(--radius-card) border border-border bg-raised p-1 shadow-lg"
      }
    >
      {props.children}
    </BaseMenu>
  );
}

/** Props of MenuItem. */
export interface MenuItemProps {
  label: string;
  icon?: IconName;
  /** The shortcut shown at the end ("⌘N"); binding it is the page's. */
  shortcut?: string;
  /** A destructive action: the danger ink. */
  danger?: boolean;
  disabled?: boolean;
  /** A checkable item: a check while true. */
  checked?: boolean;
  testID?: string;
  onSelect?: () => void;
}

/** One action of a Menu: 28 tall (44 on touch), lit while the keys or the pointer are on it. */
export function MenuItem(props: MenuItemProps) {
  return (
    <BaseMenuItem
      testID={props.testID}
      disabled={props.disabled}
      checked={props.checked}
      onSelect={props.onSelect}
      accessibilityLabel={props.label}
      class={cx("flex-row items-center gap-2 rounded-(--radius-ctl) px-2 hover:bg-hover focus:bg-hover disabled:opacity-50", TOUCH ? T_MIN_H : "h-7")}
      indicatorClass={props.checked === undefined ? undefined : "size-4 items-center justify-center"}
      indicator={props.checked === undefined ? undefined : <If when={props.checked}><Icon name="check" size={14} tone="fg" /></If>}
    >
      <If when={props.icon}>
        <Icon name={props.icon as IconName} tone={props.danger ? "danger" : "fg2"} />
      </If>
      <Line tone={props.danger ? "danger" : "fg"} class="flex-1">{props.label}</Line>
      <If when={props.shortcut}>
        <Text variant="meta" tone="fg3">{props.shortcut}</Text>
      </If>
    </BaseMenuItem>
  );
}

/** A hairline between groups of menu items. */
export function MenuSeparator() {
  return <BaseMenuSeparator class="my-1 h-px bg-border" />;
}

// ---------------------------------------------------------------- toasts

/** What one toast shows. */
interface ToastContent {
  message: string;
  icon: IconName;
  /** A button (Undo) and what it does. */
  action?: { label: string; onPress: () => void };
  /** Milliseconds it is present; the countdown bar of a toast with an action runs over it. */
  timeout: number;
}

const TICK = 50;
const manager = new ToastManager();
const shown = signal<ToastEntry[]>([]);
const contents = signal<Record<string, ToastContent>>({});
/** Milliseconds left per toast with an action (its countdown bar), paused while the stack is expanded. */
const left = signal<Record<string, number>>({});
const expanded = signal(false);
let ticking: ReturnType<typeof setInterval> | undefined;
let count = 0;

function sync() {
  shown.set(manager.visible(3));
}

function tick() {
  if (!expanded()) {
    const next: Record<string, number> = {};
    for (const [id, ms] of Object.entries(left())) next[id] = Math.max(0, ms - TICK);
    left.set(next);
  }
  const advanced = manager.advance(expanded());
  if (advanced.removed.length) {
    const c = { ...contents() };
    const l = { ...left() };
    for (const gone of advanced.removed) {
      delete c[gone.id];
      delete l[gone.id];
    }
    contents.set(c);
    left.set(l);
  }
  if (advanced.changed) sync();
  if (manager.isEmpty() && ticking !== undefined) {
    clearInterval(ticking);
    ticking = undefined;
  }
}

/** Options of toast. */
export interface ToastOptions {
  /** The leading icon (default circle-check). */
  icon?: IconName;
  /** A button: its label and what it does (it also closes the toast). */
  action?: { label: string; onPress: () => void };
  /** Milliseconds it shows (default 4000; 5000 with an action). */
  timeout?: number;
  /** Replaces the toast with this id. */
  id?: string;
}

/** Shows a toast at the bottom centre on the primary ink; returns its id. Needs one Toaster mounted. */
export function toast(message: string, options?: ToastOptions): string {
  const id = options?.id ?? `toast-${++count}`;
  const timeout = options?.timeout ?? (options?.action ? 5000 : 4000);
  contents.set({ ...contents(), [id]: { message, icon: options?.icon ?? "circle-check", action: options?.action, timeout } });
  if (options?.action) left.set({ ...left(), [id]: timeout });
  manager.push(id, id, { timeout });
  sync();
  ticking ??= setInterval(tick, TICK);
  return id;
}

/** A toast with Undo and a bar counting down the time left to use it (default 5 s). */
export function undoToast(message: string, onUndo: () => void, timeout = 5000): string {
  return toast(message, { icon: "trash", action: { label: "Undo", onPress: onUndo }, timeout });
}

/** Closes a toast now (every toast without an id). */
export function dismissToast(id?: string) {
  if (id === undefined) manager.dismissAll();
  else manager.dismiss(id);
  sync();
  ticking ??= setInterval(tick, TICK);
}

function ToastCard(props: { entry: ToastEntry }) {
  const c = () => contents()[props.entry.id];
  const progress = () => {
    const t = c();
    return t ? Math.max(0, Math.min(1, (left()[props.entry.id] ?? t.timeout) / t.timeout)) : 0;
  };
  return (
    <Toast
      transitionStatus={props.entry.status}
      accessibilityLabel={c()?.message ?? ""}
      onDismissRequest={() => dismissToast(props.entry.id)}
      class="w-full overflow-hidden rounded-(--radius-card) bg-primary shadow-lg transition-all duration-200"
      startingClass="opacity-0 translate-y-4"
      endingClass="opacity-0 scale-95"
    >
      <View class="flex-row items-center gap-2.5 px-3.5 py-2.5">
        <Icon name={c()?.icon ?? "circle-check"} tone="primaryFg" />
        <Text variant="control" tone="primaryFg" class="min-w-0 flex-1">{c()?.message ?? ""}</Text>
        <If when={c()?.action}>
          <Button
            testID="toast-action"
            accessibilityLabel={c()?.action?.label ?? ""}
            class={cx("shrink-0 rounded-(--radius-ctl) border border-primary-foreground px-2 py-0.5 active:opacity-70", FOCUS_RING, TOUCH && cx(T_MIN_H, "justify-center"))}
            onPress={() => {
              c()?.action?.onPress();
              dismissToast(props.entry.id);
            }}
          >
            <Text variant="control" tone="primaryFg" class="font-semibold">{c()?.action?.label ?? ""}</Text>
          </Button>
        </If>
      </View>
      <If when={c()?.action}>
        <View class="h-0.75 bg-primary-foreground/60" style={{ width: `${Math.round(progress() * 100)}%` }} />
      </If>
    </Toast>
  );
}

/**
 * Where toasts show: mount it once (the frame), over everything. `bottom` lifts them off the window edge (default 20): a
 * screen with a bar along the bottom (the form page's pill: 56 + 16 + 12 = 84, the frame uses 96) passes the bar's height
 * plus a gap. `top` puts them at the top instead, that far under the window's edge (a phone's form page: toastPlace).
 * The layer takes no presses of its own (`pointerEvents: "box-none"`, style-reference.md): outside a toast's
 * card a press reaches what is under it, so the bar stays pressable; the card and its Undo button take theirs.
 */
export function Toaster(props: { bottom?: number; top?: number }) {
  cleanup(() => {
    if (ticking !== undefined) clearInterval(ticking);
    ticking = undefined;
  });
  const atTop = () => props.top !== undefined;
  return (
    <Overlay
      testID="toaster"
      class="inset-x-0 items-center px-4"
      classList={{ "top-0": atTop(), "bottom-0": !atTop() }}
      style={{
        pointerEvents: "box-none",
        paddingTop: atTop() ? (props.top ?? 0) + safeArea().top : 0,
        paddingBottom: atTop() ? 0 : (props.bottom ?? 20) + safeArea().bottom,
      }}
    >
      <ToastStack
        placement={atTop() ? "topCenter" : "bottomCenter"}
        class="w-full"
        style={{ maxWidth: 440 }}
        onExpandedChange={(e) => expanded.set(e.expanded)}
        onCancel={() => {
          const newest = shown().filter((t) => t.status !== "ending").at(-1);
          if (newest) dismissToast(newest.id);
        }}
      >
        <Each of={shown()} key={(t: ToastEntry) => t.id}>
          {(t) => <ToastCard entry={t()} />}
        </Each>
      </ToastStack>
    </Overlay>
  );
}

// ---------------------------------------------------------------- panels

/** Props of SidePanel. */
export interface SidePanelProps {
  /** A 40-point header with a 14/600 title; none without. */
  title?: string;
  /** At most one ghost action at the header's end. */
  action?: JSX.Element;
  /** Fill the container (the full-window overlay of a phone-width window) instead of 320 wide. */
  full?: boolean;
  testID?: string;
  children?: JSX.Element;
}

/** The profile and notifications panel: 320 wide on the page background; the body scrolls vertically only. */
export function SidePanel(props: SidePanelProps) {
  return (
    <View testID={props.testID} class={cx("h-full min-h-0 bg-background", props.full ? "w-full" : "shrink-0")} style={{ width: props.full ? undefined : SIZE.panel }}>
      <If when={props.title}>
        <View class="shrink-0 flex-row items-center gap-2 pr-3 pl-5" style={{ height: SIZE.titleBar }}>
          <Text variant="emph" class="flex-1" lines={1}>{props.title}</Text>
          {props.action}
        </View>
      </If>
      <Scroll testID={props.testID ? `${props.testID}-scroll` : undefined} keyboardShouldPersistTaps="handled" class="min-h-0 flex-1" contentClass={cx("gap-0.5 px-3 pb-3", !props.title && "pt-1")}>
        {props.children}
      </Scroll>
    </View>
  );
}

/** One bordered group (radius 12) of Disclosures, hairlines between them. */
export function DisclosureGroup(props: { children?: JSX.Element; testID?: string }) {
  return (
    <View testID={props.testID} class="w-full overflow-hidden rounded-(--radius-card) border border-border">
      {props.children}
    </View>
  );
}

/** Props of Disclosure. */
export interface DisclosureProps {
  title: string;
  /** 12 fg3 after the title (a summary of what is inside). */
  meta?: string;
  open: boolean;
  onToggle: () => void;
  /** While open and not editing: a ghost Edit at the header's end. */
  onEdit?: () => void;
  editing?: boolean;
  testID?: string;
  /** The body, shown while open, padded so it lines up with the title. */
  children?: JSX.Element;
}

/** A section that expands: a 44-point header (a chevron that turns down, title 13/600, meta), the body under it. */
export function Disclosure(props: DisclosureProps) {
  return (
    <Collapsible
      testID={props.testID}
      open={props.open}
      class="w-full border-t border-border first:border-t-0"
      content={<View class="pt-1 pr-5 pb-5 pl-9">{props.children}</View>}
    >
      <View class="flex-row items-center gap-2 pr-3">
        <Button
          testID={props.testID ? `${props.testID}-toggle` : undefined}
          accessibilityLabel={props.title}
          class={cx("min-w-0 flex-1 flex-row items-center gap-2 pr-2 pl-3 focus-visible:bg-hover", TOUCH ? T_MIN_H : "h-11")}
          onPress={props.onToggle}
        >
          <Icon name={props.open ? "chevron-down" : "chevron-right"} tone="fg3" />
          <Text variant="control" class="shrink-0 font-semibold">{props.title}</Text>
          <Line variant="meta" tone="fg3" class="flex-1">{props.meta ?? ""}</Line>
        </Button>
        <If when={props.open && props.onEdit && !props.editing}>
          <EditButton onPress={() => props.onEdit?.()} testID={props.testID ? `${props.testID}-edit` : undefined} />
        </If>
      </View>
    </Collapsible>
  );
}

/** The disclosure's Edit: a small ghost button with a pencil. */
function EditButton(props: { onPress: () => void; testID?: string }) {
  return <AppButton label="Edit" variant="ghost" size="sm" icon="pencil" testID={props.testID} onPress={props.onPress} />;
}
