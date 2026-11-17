// ShortcutRecorder: a box that records the keys a person presses, as macOS System Settings does. NSH has no raw key-down event in TSX
// (an Input's onKeyPress gives only the key, no modifiers), so while the box is recording it binds every chord a person could mean
// with nsh/keymap (each subset of ctrl, alt, shift and cmd with a to z, 0 to 9 and f1 to f12, plus escape, backspace and delete; tab only
// to end the recording as the focus moves) and takes the one that fires. One keymap.bind with a record sends the platform its table
// once; the keystrokes the platform keeps (Cmd+Q ...) are found once by halving (chords.ts bindAvailable) and left out, so they still do
// what the system does. Everything is unbound when recording ends, and when the part goes away.
import type { ViewRef } from "nsh";
import { cleanup, keymap, Overlay, signal, View } from "nsh";
import { Button } from "nsh/base";
import { allChords, bindAvailable, CANCEL_KEY, CLEAR_KEYS, LEAVE_KEYS } from "./chords";
import { Tooltip } from "./layers";
import { CTL, cx, FOCUS_RING, TOUCH } from "./look";
import { Icon, Line, Text } from "./text";

type Heard = (e: { keystroke: string }) => void;

/** The keystrokes the platform let the first recording bind (the set never changes while the app runs): later ones bind them in one call. */
let offered: string[] | null = null;

/**
 * Binds every chord, escape, backspace and delete to `heard` (which gets the keystroke as written here, lower case, modifiers in the
 * order ctrl, alt, shift, cmd) and returns the way to unbind them all. The keystrokes the platform keeps are not bound.
 */
export function listenForChords(heard: Heard): () => void {
  const group = (list: string[]) => {
    const record: Record<string, Heard> = {};
    for (const key of list) record[key] = heard;
    // "always": a recording takes the key in a text box too, before the box sees it.
    return keymap.bind(record, { when: "always" });
  };
  // Tab moves the focus on as it always does, and ends the recording (a keyboard user who tabs away has left the box).
  const leave: Record<string, Heard> = {};
  for (const key of LEAVE_KEYS) leave[key] = heard;
  const unbindLeave = keymap.bind(leave, { when: "always", passThrough: true });
  if (offered) {
    const unbindChords = group(offered);
    return () => {
      unbindChords();
      unbindLeave();
    };
  }
  const done = bindAvailable([...allChords(), CANCEL_KEY, ...CLEAR_KEYS], group);
  offered = done.bound;
  return () => {
    done.unbind();
    unbindLeave();
  };
}

/** What the box says while a chord that is not acceptable was pressed and no words were given. */
const NOT_ACCEPTED = "That shortcut cannot be used.";

/** Props of ShortcutRecorder. */
export interface ShortcutRecorderProps {
  /** The shortcut as nsh/keymap names it ("ctrl+alt+e", modifiers in the order ctrl, alt, shift, cmd), or "" for none. Controlled. */
  value: string;
  /** A chord was recorded, or "" when the person removed the shortcut (the clear button, Delete or Backspace while recording). */
  onChange: (keystroke: string) => void;
  /** What the shortcut is, for screen readers ("Global show hotkey"). */
  label: string;
  /** Which keystrokes may be taken; the others keep the box recording and show `unacceptable`. Default: all of them. */
  accepts?: (keystroke: string) => boolean;
  /** The words shown while a chord `accepts` refused ("Add Control, Option or Command."). */
  unacceptable?: string;
  /** A problem with the shortcut as it stands, under the box in the danger tone ("Another app is using this shortcut."). */
  error?: string;
  /** Told when recording starts and when it ends, however it ends (a chord, Escape, a press elsewhere, the part going away). */
  onRecordingChange?: (recording: boolean) => void;
  /** The box's width in points; unset, its container's. */
  width?: number;
  disabled?: boolean;
  /** Names the box (the one that is pressed to record); `<testID>-clear` the remove button, `<testID>-hint` the words under the box while recording. */
  testID?: string;
}

/**
 * A 32-point box (44 on touch) that shows the shortcut as the platform writes it (keymap.label: "⌃⌥E", "Ctrl+Alt+E") with a small remove
 * button, or "None" and "Click to record". A press or Enter or Space on it records: it takes the accent border, reads "Type shortcut", and a
 * bubble under it (right edges together) says "Esc to cancel · Delete to remove". The next chord ends the recording and goes to onChange;
 * Escape, Tab or a press anywhere else cancels it.
 */
export function ShortcutRecorder(props: ShortcutRecorderProps) {
  const recording = signal(false);
  const note = signal("");
  let frame: ViewRef | undefined;
  let unbind: (() => void) | null = null;

  /** The value as the platform shows it; empty for none. */
  const shown = (): string => {
    const value = props.value;
    if (!value) return "";
    try {
      return keymap.label(value);
    } catch {
      return value;
    }
  };
  const words = (): string => (recording() ? "Type shortcut" : shown() || "None");

  /** Stops listening. True when it was recording. */
  const stop = (): boolean => {
    const off = unbind;
    unbind = null;
    off?.();
    if (!recording()) return false;
    recording.set(false);
    note.set("");
    return true;
  };
  /** Ends the recording; `change` (when given) goes to onChange before the end is told, so a screen that holds something off while recording sees the new value first. */
  const finish = (change?: string): void => {
    const was = stop();
    if (change !== undefined) props.onChange(change);
    if (was) props.onRecordingChange?.(false);
  };
  const heard: Heard = (e) => {
    // A key pressed in the same moment as the one that ended the recording is already queued for a binding that is gone now.
    if (!recording()) return;
    const key = e.keystroke;
    if (key === CANCEL_KEY || LEAVE_KEYS.includes(key)) return finish();
    if (CLEAR_KEYS.includes(key)) return finish(props.value ? "" : undefined);
    if (props.accepts && !props.accepts(key)) {
      note.set(props.unacceptable ?? NOT_ACCEPTED);
      return;
    }
    finish(key);
  };
  const start = (): void => {
    if (recording() || props.disabled) return;
    note.set("");
    try {
      unbind = listenForChords(heard);
    } catch {
      return; // no keyboard shortcuts here: stay as it was
    }
    recording.set(true);
    props.onRecordingChange?.(true);
  };
  cleanup(() => finish());

  const id = (suffix: string) => (props.testID ? `${props.testID}-${suffix}` : undefined);
  const clearable = () => !recording() && !!props.value && !props.disabled;
  const frameClass = () =>
    cx(
      "flex-row rounded-(--radius-row) border bg-card",
      CTL,
      recording() ? "border-accent-blue ring-2 ring-accent-blue-soft" : props.error ? "border-danger" : "border-border-strong focus-within:border-accent-blue",
      props.disabled ? "opacity-50" : !recording() && "hover:bg-hover",
    );

  return (
    <View class="min-w-0 gap-1" style={{ width: props.width }}>
      <View ref={(r: ViewRef) => (frame = r)} class={frameClass()}>
        <Button
          testID={props.testID}
          class={cx("min-w-0 flex-1 flex-row items-center gap-2 rounded-(--radius-row) pr-2 pl-2.5", FOCUS_RING)}
          disabled={props.disabled}
          selected={recording()}
          accessibilityLabel={`${props.label}: ${words()}`}
          onPress={start}
        >
          <If when={recording() || shown()} else={<Text tone="fg3" class="shrink-0">None</Text>}>
            <Line tone={recording() ? "fg2" : "fg"} class="flex-1">{words()}</Line>
          </If>
          <If when={!recording() && !shown()}>
            <Line variant="meta" tone="fg3" class="flex-1 text-right">Click to record</Line>
          </If>
        </Button>
        <If when={clearable()}>
          <Button
            testID={id("clear")}
            class={cx(
              "mr-1 shrink-0 items-center justify-center self-center rounded-(--radius-ctl) enabled:hover:bg-selected active:bg-selected",
              FOCUS_RING,
              TOUCH ? "size-9" : "size-6",
            )}
            accessibilityLabel="Remove shortcut"
            tooltip={<Tooltip label="Remove shortcut" placement="top" />}
            onPress={() => props.onChange("")}
          >
            <Icon name="x" size={14} tone="fg3" />
          </Button>
        </If>
      </View>
      <If when={props.error && !recording()}>
        <Text testID={id("error")} variant="meta" tone="danger">{props.error}</Text>
      </If>
      <If when={recording()}>
        <Overlay
          anchor={frame}
          placement="bottom"
          align="end"
          offset={4}
          margin={8}
          onPressOutside={() => finish()}
          class="max-w-64 gap-0.5 rounded-(--radius-ctl) border border-border bg-raised px-2 py-1 shadow-md"
        >
          <If when={note()}>
            <Text testID={id("note")} variant="meta" tone="danger">{note()}</Text>
          </If>
          <Text testID={id("hint")} variant="meta" tone="fg3">Esc to cancel · Delete to remove</Text>
        </Overlay>
      </If>
    </View>
  );
}
