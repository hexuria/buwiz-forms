// A choice from a long list (RDOs, ZIP codes, ATC codes, regions, cities), searched by every typed word in any order (the kit's
// ComboField and Select match one run of text only, nsh/base listbox.rs, and their filter cannot be replaced).
// - Phone layout (every touch screen, and a pointer window under 600 points, as the kit Sheet decides): a trigger that opens a bottom
//   sheet. The sheet's body does not scroll (Sheet `scroll={false}`): its search field stays pinned at the top and only the list under
//   it scrolls, a virtualised List sized to the room the sheet has above the keyboard (sheetBodyRoom).
// - Wide pointer window: a combobox. The field is the text box; typing filters a list anchored under it; Up/Down move the highlight,
//   Enter picks it, Escape closes and restores, Tab leaves; the chevron opens the whole list. Text that is not a choice is not taken
//   on leaving: the box goes back to the last pick and a line under it says so (resolveTyped in model.ts).
import type { Accessor, InputRef, ListRef, ViewRef } from "nsh";
import { announce, batch, effect, keymap, List, Overlay, platform, signal, untracked, View, windowSize } from "nsh";
import { ChoiceRow, Icon, IconButton, Line, phoneLayout, Popover, Sheet, sheetBodyRoom, T_MIN_H, T_PX, Text, TextField, TOUCH } from "../../../ui";
import type { ChoiceOption } from "../contract";
import { cx, matchWords, pickLabel, resolveTyped, searchKeyboardType } from "./model";

/** Props of SearchPick. */
export interface SearchPickProps {
  /**
   * "fp-<key>": the control's base view. Inside it: "<testID>-trigger" (the phone trigger; the chevron of the wide field),
   * "<testID>-search" (the sheet's search field; the wide field's text box), "<testID>-list", a row "<testID>-opt-<value>" (the
   * clear row "<testID>-opt-"), "<testID>-sheet" (the phone sheet) and "<testID>-note" (the wide field's refusal line).
   */
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
  /**
   * The phone keyboard the search opens with. "digits-first" for a list mostly searched by number but also by name (ZIP codes: a
   * filer who does not know the ZIP knows the city): iOS's numbers-and-punctuation keyboard, which opens on digits and reaches
   * letters with one key; Android's number keyboards have no letters, so there it is the ordinary keyboard. Never a digits-only
   * pad: every list here is searchable by name. Default: the ordinary keyboard.
   */
  searchKeyboard?: "default" | "digits-first";
  /** Fires when the list opens. */
  onOpen?: () => void;
  /** Receives the trigger's ref (phone) or the text box's (wide), so a pill step can put the keyboard on it. */
  focusRef?: (ref: ViewRef) => void;
}

const real = (p: SearchPickProps) => p.options.filter((o) => o.value !== "");
const currentOf = (p: SearchPickProps) => p.options.find((o) => o.value === p.value && o.value !== "");
const rowLabel = (o: ChoiceOption) => (o.disabled ? `${pickLabel(o)}, unavailable` : pickLabel(o));
const NOTHING = "Nothing matches. Try a code or other words.";


// ---------------------------------------------------------------- test seam

/** Where a box is in its root view (the sheet's modal): its top and bottom, in points. */
export interface FrameBox {
  top: number;
  bottom: number;
}

/** The phone sheet's search field, its list and the panel around them, with the window's height. */
export interface SheetFrames {
  testID: string;
  search: FrameBox;
  list: FrameBox;
  panel: FrameBox;
  window: number;
}

let frameWatch: ((f: SheetFrames) => void) | null = null;

/** Test seam: hears the sheet's frames as it opens and as its list scrolls (PickProbe.tsx prints them). Null stops. */
export function watchSheetFrames(fn: ((f: SheetFrames) => void) | null) {
  frameWatch = fn;
}

async function boxOf(ref: ViewRef | undefined): Promise<FrameBox | null> {
  const m = ref ? await ref.measure() : null;
  return m ? { top: m.pagePosition.y, bottom: m.pagePosition.y + m.bounds.size.height } : null;
}

// ---------------------------------------------------------------- phone: a sheet with a pinned search

/** One row's words: "050 — South Makati", dimmed when it takes no pick. */
function RowWords(props: { o: Accessor<ChoiceOption>; oneLine?: boolean }) {
  return (
    <If when={props.oneLine} else={<Text variant="control" lines={2} class={cx(props.o().disabled && "opacity-50")}>{pickLabel(props.o())}</Text>}>
      <Line variant="control" class={cx(props.o().disabled && "opacity-50")}>{pickLabel(props.o())}</Line>
    </If>
  );
}

/** The sheet's body: the search field, then the list in the room that is left. Mounted per opening (a fresh search, the list on the current choice). */
function SheetPanel(props: { p: SearchPickProps; choose: (value: string) => void }) {
  const p = props.p;
  const query = signal("");
  const rows = () => matchWords(real(p), query());
  const showClear = () => p.value !== "" && p.clearable !== false && query().trim() === "";
  // Read once as the panel mounts: the list opens with the current choice in view; later queries scroll from their own top.
  const start = untracked(() => {
    const at = rows().findIndex((o) => o.value === p.value);
    return at >= 0 ? at : undefined;
  });
  // The body's padding (16 a side) and gap (12) and the search field leave the list the rest of the sheet's room.
  const listHeight = () => Math.max(T_PX * 2, sheetBodyRoom() - 32 - 12 - T_PX);
  const refs: { panel?: ViewRef; search?: ViewRef; list?: ViewRef } = {};
  let list: ListRef | null = null;
  // Measured once every part is laid out (the panel's and the list's onLayout: a ref callback comes before its later siblings' refs
  // and before layout, so the old report from the search field's ref found no list and said nothing) and on every scroll of the list.
  // Only a test that registered watchSheetFrames before the sheet opened gets the handlers below: without one the panel and the list
  // carry no onLayout or scroll handlers at all (no events sent, no measure() calls, no signals written, so no extra renders).
  const watch = frameWatch;
  const watching = <T,>(fn: T): T | undefined => (watch ? fn : undefined);
  const report = () => {
    if (!watch || !refs.search || !refs.list || !refs.panel) return;
    void Promise.all([boxOf(refs.search), boxOf(refs.list), boxOf(refs.panel)]).then(([search, listBox, panel]) => {
      if (search && listBox && panel) watch({ testID: p.testID, search, list: listBox, panel, window: windowSize().height });
    });
  };
  const submit = () => {
    const open = rows().filter((o) => !o.disabled);
    if (open.length === 1) props.choose(open[0].value);
  };
  return (
    <View ref={(r: ViewRef) => (refs.panel = r)} class="min-h-0 gap-3" onLayout={watching(report)}>
      <TextField
        size="sm"
        icon="search"
        autoFocus
        value={query()}
        onChange={(v) => {
          query.set(v);
          list?.scrollToOffset({ offset: 0, animated: false });
        }}
        keyboardType={searchKeyboardType(p.searchKeyboard, platform.os)}
        autoCapitalize="none"
        returnKeyType="search"
        onSubmit={submit}
        placeholder="Search by code or words"
        accessibilityLabel={`Search ${p.label}`}
        testID={`${p.testID}-search`}
        inputRef={(r: InputRef) => (refs.search = r)}
      />
      <List
        ref={(r: ListRef) => {
          list = r;
          refs.list = r;
        }}
        testID={`${p.testID}-list`}
        each={rows()}
        keyExtractor={(o) => o.value}
        initialScrollIndex={start}
        estimatedItemSize={56}
        keyboardShouldPersistTaps="handled"
        style={{ height: listHeight() }}
        onLayout={watching(report)}
        onScroll={watching(report)}
        onScrollEnd={watching(report)}
        onScrollDragEnd={watching(report)}
        onMomentumEnd={watching(report)}
        header={() => (
          <If when={showClear()}>
            <ChoiceRow testID={`${p.testID}-opt-`} label="Clear the choice" onPress={() => props.choose("")}>
              <Text tone="fg3">Clear the choice</Text>
            </ChoiceRow>
          </If>
        )}
        empty={() => <Text variant="meta" tone="fg3" class="px-1 py-3">{NOTHING}</Text>}
      >
        {(item) => (
          // A recycled cell draws its new item anew (keyed by value), so a row's testID and check follow the item.
          <Each of={[item()]} key={(o) => o.value}>
            {(o) => (
              <ChoiceRow
                testID={`${p.testID}-opt-${o().value}`}
                label={rowLabel(o())}
                chosen={o().value === p.value}
                disabled={o().disabled}
                onPress={o().disabled ? undefined : () => props.choose(o().value)}
              >
                <RowWords o={o} />
              </ChoiceRow>
            )}
          </Each>
        )}
      </List>
    </View>
  );
}

/** The phone layout: a trigger like the kit's Dropdown, and the sheet. */
function SheetPick(props: { p: SearchPickProps }) {
  const p = props.p;
  const sheet = signal(false);
  const current = () => currentOf(p);
  const asked = (on: boolean) => {
    if (p.disabled) return;
    if (!on) return sheet.set(false);
    sheet.set(true);
    p.onOpen?.();
  };
  const choose = (value: string) => {
    sheet.set(false);
    p.onPick(value);
  };
  return (
    <>
      {/* A popover that is never open: its trigger is the kit's pressable (focus, Return, the press), routed to the sheet (AGENTS.md). */}
      <Popover
        open={false}
        onOpenChange={asked}
        trigger={
          <View
            ref={(r: ViewRef) => p.focusRef?.(r)}
            testID={`${p.testID}-trigger`}
            role="button"
            accessibilityLabel={`${p.label}: ${current() ? pickLabel(current()!) : "nothing chosen"}`}
            class={cx(
              "min-w-0 flex-row items-center gap-2 rounded-(--radius-row) border bg-card pr-2 pl-2.5",
              TOUCH ? T_MIN_H : "h-8",
              p.error ? "border-danger" : sheet() ? "border-accent-blue" : "border-border-strong",
              p.disabled ? "opacity-50" : "hover:bg-hover",
            )}
            style={{ width: p.width }}
          >
            <Line tone={current() ? "fg" : "fg3"} class="flex-1">{current() ? pickLabel(current()!) : "Choose…"}</Line>
            <Icon name="chevron-down" size={14} tone="fg3" />
          </View>
        }
      />
      <Sheet testID={`${p.testID}-sheet`} open={sheet()} onOpenChange={asked} title={p.label} scroll={false}>
        <If when={sheet()}>
          <SheetPanel p={p} choose={choose} />
        </If>
      </Sheet>
    </>
  );
}

// ---------------------------------------------------------------- wide: a combobox

/** A wide row is 32 points; the list shows eight before it scrolls. */
const ROW = 32;
const ROWS_SHOWN = 8;

/** The wide list, mounted as it opens: it starts with the lit row in view. */
function FieldList(props: {
  p: SearchPickProps;
  rows: Accessor<ChoiceOption[]>;
  lit: Accessor<string | null>;
  showClear: Accessor<boolean>;
  setLit: (value: string) => void;
  choose: (value: string) => void;
  list: (ref: ListRef) => void;
  seen: (first: number, last: number) => void;
}) {
  const p = props.p;
  const start = untracked(() => {
    const at = props.rows().findIndex((o) => o.value === props.lit());
    return at >= 0 ? at : undefined;
  });
  const height = () => Math.min(ROWS_SHOWN, props.rows().length + (props.showClear() ? 1 : 0)) * ROW;
  return (
    <List
      ref={props.list}
      testID={`${p.testID}-list`}
      each={props.rows()}
      keyExtractor={(o) => o.value}
      initialScrollIndex={start}
      estimatedItemSize={ROW}
      style={{ height: height() }}
      viewabilityConfig={{ itemVisibleThreshold: 1 }}
      onViewableItemsChange={(info) => {
        const at = info.viewableItems.map((t) => t.index);
        if (at.length) props.seen(Math.min(...at), Math.max(...at));
      }}
      header={() => (
        <If when={props.showClear()}>
          <ChoiceRow testID={`${p.testID}-opt-`} label="Clear the choice" onPress={() => props.choose("")}>
            <Text tone="fg3">Clear the choice</Text>
          </ChoiceRow>
        </If>
      )}
    >
      {(item) => (
        <Each of={[item()]} key={(o) => o.value}>
          {(o) => (
            <ChoiceRow
              testID={`${p.testID}-opt-${o().value}`}
              label={rowLabel(o())}
              chosen={o().value === p.value}
              lit={o().value === props.lit()}
              disabled={o().disabled}
              tooltip={pickLabel(o())}
              onHover={o().disabled ? undefined : () => props.setLit(o().value)}
              onPress={o().disabled ? undefined : () => props.choose(o().value)}
            >
              <RowWords o={o} oneLine />
            </ChoiceRow>
          )}
        </Each>
      )}
    </List>
  );
}

/** The wide layout: the field is the text box, the list drops under it. */
function FieldPick(props: { p: SearchPickProps }) {
  const p = props.p;
  const typed = signal("");
  /** The box holds text the person typed (not the current choice's words). */
  const dirty = signal(false);
  const open = signal(false);
  const lit = signal<string | null>(null);
  const note = signal("");
  const focused = signal(false);
  const anchor = signal<ViewRef | null>(null);
  const width = signal(0);
  let input: InputRef | null = null;
  let list: ListRef | null = null;
  let seen = { first: 0, last: -1 };
  let selectOnCaret = false;

  const current = () => currentOf(p);
  const shown = () => (dirty() ? typed() : current() ? pickLabel(current()!) : "");
  const query = () => (dirty() ? typed() : "");
  const rows = () => matchWords(real(p), query());
  const pickable = () => rows().filter((o) => !o.disabled);
  const showClear = () => p.value !== "" && p.clearable !== false && query().trim() === "";

  const restore = () =>
    batch(() => {
      dirty.set(false);
      typed.set("");
    });
  const close = () =>
    batch(() => {
      open.set(false);
      lit.set(null);
    });
  const showList = () => {
    if (p.disabled) return;
    const r = pickable();
    lit.set(r.some((o) => o.value === p.value) ? p.value : (r[0]?.value ?? null));
    if (!open()) {
      seen = { first: 0, last: -1 };
      open.set(true);
      p.onOpen?.();
    }
  };
  /** A row chosen (a press, Enter): it is the value, the list closes, the keys stay in the box. */
  const choose = (value: string) => {
    batch(() => {
      close();
      restore();
      note.set("");
    });
    p.onPick(value);
    input?.focus();
  };
  /** The keys leave (Tab, a press elsewhere, the focus going): the typed text is taken only when it names a choice. */
  const commit = () => {
    if (!dirty()) return close();
    const r = resolveTyped(p.options, typed(), p.value, p.clearable !== false);
    batch(() => {
      close();
      restore();
      note.set(r.kind === "revert" ? r.note : "");
    });
    if (r.kind === "pick") p.onPick(r.value);
    else if (r.kind === "clear") p.onPick("");
    else if (r.kind === "revert") announce(r.note);
  };
  const escape = () =>
    batch(() => {
      close();
      restore();
      note.set("");
    });
  const move = (step: 1 | -1) => {
    if (!open()) return showList();
    const r = pickable();
    if (!r.length) return;
    const i = r.findIndex((o) => o.value === lit());
    const next = r[i < 0 ? 0 : Math.max(0, Math.min(r.length - 1, i + step))];
    lit.set(next.value);
    const at = rows().indexOf(next);
    if (at < seen.first) list?.scrollToIndex({ index: at, viewPosition: 0, animated: false });
    else if (at > seen.last) list?.scrollToIndex({ index: at, viewPosition: 1, animated: false });
  };
  const enter = () => {
    const hit = lit();
    if (open() && hit !== null) return choose(hit);
    if (dirty()) return commit();
    showList();
  };

  // The keys a combobox takes while its box has the focus; Escape only while there is something to close or restore, so a dialog
  // around a quiet box still closes on it. Tab moves on as it always does (passThrough) after the text is settled.
  effect(() => {
    if (!focused()) return;
    const offs = [
      keymap.bind({ up: () => move(-1), down: () => move(1) }, { when: "always", repeat: true }),
      keymap.bind("enter", enter, { when: "always" }),
      keymap.bind({ tab: commit, "shift+tab": commit }, { when: "always", passThrough: true }),
    ];
    if (open() || dirty()) offs.push(keymap.bind("escape", escape, { when: "always" }));
    return () => {
      for (const off of offs) off();
    };
  });

  return (
    <View
      ref={(r: ViewRef) => anchor.set(r)}
      class="min-w-0"
      onLayout={(e) => width.set(e.bounds.size.width)}
    >
      <TextField
        testID={`${p.testID}-search`}
        accessibilityLabel={p.label}
        value={shown()}
        placeholder="Type a code or a name"
        disabled={p.disabled}
        error={p.error}
        autoCapitalize="none"
        width={p.width}
        inputRef={(r: InputRef) => {
          input = r;
          p.focusRef?.(r);
        }}
        onFocus={() => {
          focused.set(true);
          note.set("");
          // A click leaves its caret after the focus (AppKit selects all, then puts the caret): select the whole choice again, so
          // typing replaces it rather than adding to "050 — South Makati".
          selectOnCaret = !dirty();
          setTimeout(() => (selectOnCaret = false), 400);
        }}
        onSelectionChange={(e) => {
          if (!selectOnCaret || e.start !== e.end) return;
          selectOnCaret = false;
          input?.setSelection(0, shown().length);
        }}
        onBlur={() => {
          focused.set(false);
          // With the list open the press that took the focus may be a row's; the list's own press-outside settles the rest.
          if (!open()) commit();
        }}
        onChange={(v) => {
          selectOnCaret = false;
          batch(() => {
            typed.set(v);
            dirty.set(true);
            note.set("");
            const first = matchWords(real(p), v).find((o) => !o.disabled);
            lit.set(first?.value ?? null);
            if (!open()) {
              seen = { first: 0, last: -1 };
              open.set(true);
              p.onOpen?.();
            }
          });
          list?.scrollToOffset({ offset: 0, animated: false });
        }}
        trailing={
          <IconButton
            icon="chevron-down"
            tooltip={`Show every ${p.label}`}
            testID={`${p.testID}-trigger`}
            disabled={p.disabled}
            onPress={() => {
              if (open()) return commit();
              batch(() => {
                restore();
                note.set("");
                showList();
              });
              input?.focus();
              input?.setSelection(0, shown().length);
            }}
          />
        }
      />
      <If when={open() && anchor()}>
        <Overlay
          anchor={anchor()}
          placement="bottom"
          align="start"
          offset={4}
          margin={8}
          onPressOutside={commit}
          onCancel={escape}
        >
          <View class="rounded-(--radius-card) border border-border bg-raised p-1 shadow-lg" style={{ width: Math.max(width(), 380) }}>
            <If
              when={rows().length > 0 || showClear()}
              else={<Text variant="meta" tone="fg3" class="px-2 py-1.5">{NOTHING}</Text>}
            >
              <FieldList
                p={p}
                rows={rows}
                lit={() => lit()}
                showClear={showClear}
                setLit={(v) => lit.set(v)}
                choose={choose}
                list={(r) => (list = r)}
                seen={(first, last) => (seen = { first, last })}
              />
            </If>
          </View>
        </Overlay>
      </If>
      <If when={note() && !open() && anchor()}>
        <Overlay anchor={anchor()} placement="bottom" align="start" offset={4} margin={8} onPressOutside={() => note.set("")}>
          <View
            testID={`${p.testID}-note`}
            class="rounded-(--radius-ctl) border border-border bg-raised px-2 py-1 shadow-md"
            style={{ maxWidth: Math.max(width(), 320) }}
          >
            <Text variant="meta" tone="danger">{note()}</Text>
          </View>
        </Overlay>
      </If>
    </View>
  );
}

// ---------------------------------------------------------------- the control

/** A choice from a long list, searched by words: a sheet in the phone layout, a combobox in a wide pointer window. */
export function SearchPick(props: SearchPickProps) {
  return (
    <View testID={props.testID} class="min-w-0" style={{ width: props.width }}>
      <If when={phoneLayout()} else={<FieldPick p={props} />}>
        <SheetPick p={props} />
      </If>
    </View>
  );
}
