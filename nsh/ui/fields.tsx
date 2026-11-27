// Text entry: TextField (the one wrapper of nsh/base's Input), Field and FieldGrid, FormActions, PinCells (OtpInput),
// and the masked fields on TextField: TinInput, MoneyInput, PercentInput, IntInput, DateField. The masks' rules are in
// mask.ts (pure, tested); here they meet the input: the text is controlled, the caret is put back after a reformat, and
// the selection the input reported before each edit goes to the mask so the edit is read exactly (NSH: onKeyPress comes
// before the text changes, onSelectionChange has the selection; nsh/.nsh/types/nsh/types.d.ts InputHostProps).
import type { InputRef, JSX, ViewRef } from "nsh";
import { effect, signal, Text as NText, untracked, View } from "nsh";
import { Calendar, type CalendarEntry, DatePicker, Input, OtpInput, type OtpInputRef } from "nsh/base";
import { AppButton } from "./buttons";
import type { IconName } from "./icons";
import { cx, T_H, T_MIN_H, T_MIN_W, T_PX, T_SIZE, TOUCH } from "./look";
import {
  DATE_ERROR, digitsOnly, editBase, formatIso, formatLongDate, formatMDY, formatMoney, formatTinDigits, intMask, type Mask, moneyMask, moneyPlain,
  isZeroAmount, parseTypedDate, PERCENT_MASK, reconcile, sameAmount, shownMoney, TIN_LENGTH, TIN_MASK, type TextSelection,
} from "./mask";
import { Chip } from "./status";
import { Icon, Text } from "./text";

// ---------------------------------------------------------------- text field

type KeyboardType = "default" | "number-pad" | "decimal-pad" | "numeric" | "numbers-and-punctuation" | "email-address" | "phone-pad" | "url";

/** Props of TextField. */
export interface TextFieldProps {
  value: string;
  /** The text after each edit. */
  onChange: (value: string) => void;
  /** A visible label above the box (12 fg2); also what screen readers call it. */
  label?: string;
  /** What screen readers call it when there is no visible label. */
  accessibilityLabel?: string;
  accessibilityDescribedBy?: ViewRef | null;
  /** A line under the box, 12 fg3. */
  hint?: string;
  /** An error: true for the danger border alone, a string for the message under the box too. */
  error?: string | boolean;
  placeholder?: string;
  disabled?: boolean;
  /** Read-only from elsewhere (the tax profile, a computed amount): the hover fill, a lock, no editing. */
  locked?: boolean;
  /** Width in points; unset, its container's. */
  width?: number;
  /** md: 32 tall, sm: 28 (44 on touch either way). */
  size?: "md" | "sm";
  /**
   * No box of its own: no border, fill or padding, and no ring while it has the keys, 14-point text. For a search line that sits
   * in a row of its own (the command palette's, whose icon and Esc key are beside it); the row draws the frame.
   */
  bare?: boolean;
  /** The sign-in pages' field: 46 points tall (48 on Android), radius 10, a 13-point label and 14-point text. */
  large?: boolean;
  /** Right-aligned text (amounts, percents), centred (the phone TIN grid's digits). */
  align?: "left" | "right" | "center";
  /** Tabular figures (TINs, amounts, dates). */
  tabular?: boolean;
  /** A leading icon. */
  icon?: IconName;
  /** A unit before the text (₱). */
  prefix?: string;
  /** A unit after the text (%). */
  suffix?: string;
  /** Something at the end of the box (a calendar button). */
  trailing?: JSX.Element;
  /** Hidden text (passwords, PINs). */
  masked?: boolean;
  keyboardType?: KeyboardType;
  autoCapitalize?: "none" | "sentences" | "words" | "characters";
  autoComplete?: string;
  maxLength?: number;
  autoFocus?: boolean;
  returnKeyType?: "default" | "done" | "go" | "next" | "search" | "send";
  onFocus?: () => void;
  onBlur?: () => void;
  onSubmit?: () => void;
  /** The caret or selection moved (UTF-16 units). */
  onSelectionChange?: (e: { start: number; end: number }) => void;
  /** Before the text changes: the typed text, "Backspace" or "Enter". */
  onKeyPress?: (e: { key: string }) => void;
  /** The input's ref, to focus it or set its selection. */
  inputRef?: (ref: InputRef) => void;
  testID?: string;
}

/** A text box: 32 tall (44 on touch), radius 8, the strong border, the accent while it has the keys, danger on error. */
export function TextField(props: TextFieldProps) {
  const sm = () => props.size === "sm";
  const message = () => (typeof props.error === "string" && props.error ? props.error : "");
  const flat = () => !!props.locked || !!props.disabled;
  const messageRef = signal<ViewRef | null>(null);
  return (
    <View class="min-w-0 gap-1" style={{ width: props.width }}>
      <If when={props.label}>
        <Text variant={props.large ? "body" : "meta"} tone="fg2">{props.label}</Text>
      </If>
      <View
        class={cx(
          "flex-row items-center gap-1.5",
          props.bare
            ? TOUCH ? T_MIN_H : "h-8"
            : cx(
                props.large ? "rounded-[10px] border" : "rounded-(--radius-row) border",
                // A centred box is a cell of the phone TIN grid: 47 pt wide on a 320 pt phone, where 10 pt sides cut
                // the third digit. It keeps 4 pt, like the locked cell (controls/model.ts readBoxPad).
                props.align === "center" ? "px-1" : props.large ? "px-0" : "px-2.5",
                // The large field is one fixed height, so its input (h-full) fills it: a press anywhere in the box focuses it.
                props.large ? (T_PX > 46 ? "h-12" : "h-11.5") : TOUCH ? T_MIN_H : sm() ? "h-7" : "h-8",
                flat() ? "border-border bg-hover" : "bg-card",
                props.error ? "border-danger" : !flat() && "border-border-strong focus-within:border-accent-blue",
              ),
          props.disabled && "opacity-60",
        )}
      >
        <If when={props.icon}>
          <Icon name={props.icon as IconName} size={14} tone="fg3" />
        </If>
        <If when={props.prefix}>
          <Text variant="meta" tone="fg3">{props.prefix}</Text>
        </If>
        <Input
          ref={(r: InputRef) => props.inputRef?.(r)}
          testID={props.testID}
          value={props.value}
          onChange={(e) => props.onChange(e.value)}
          placeholder={props.placeholder}
          readonly={props.locked}
          disabled={props.disabled}
          masked={props.masked}
          keyboardType={props.keyboardType}
          autoCapitalize={props.autoCapitalize}
          autoCorrect={props.keyboardType ? false : undefined}
          autoComplete={props.autoComplete as never}
          maxLength={props.maxLength}
          autoFocus={props.autoFocus}
          returnKeyType={props.returnKeyType}
          accessibilityLabel={props.label ?? props.accessibilityLabel}
          accessibilityDescribedBy={props.accessibilityDescribedBy ?? (message() ? messageRef() : null)}
          onFocus={() => props.onFocus?.()}
          onBlur={() => props.onBlur?.()}
          onSubmit={() => props.onSubmit?.()}
          onSelectionChange={props.onSelectionChange}
          onKeyPress={props.onKeyPress}
          class={cx(
            "h-full min-w-0 flex-1 bg-transparent caret-accent-blue placeholder:text-subtle-foreground selection:bg-accent-blue-soft",
            props.bare || props.large ? "text-emph font-normal" : "text-body",
            // The large field's sides are the input's own, so the whole box takes the press.
            props.large && "px-3",
            flat() ? "text-muted-foreground" : "text-foreground",
            props.align === "right" && "text-right",
            props.align === "center" && "text-center",
            props.tabular && "tabular-nums",
          )}
        />
        <If when={props.suffix}>
          <Text variant="meta" tone="fg3">{props.suffix}</Text>
        </If>
        <If when={props.locked}>
          <Icon name="lock" size={14} tone="fg3" />
        </If>
        {props.trailing}
      </View>
      <If when={message()}>
        <View ref={(ref: ViewRef) => messageRef.set(ref)}>
          <Text variant="meta" tone="danger">{message()}</Text>
        </View>
      </If>
      <If when={!message() && props.hint}>
        <Text variant="meta" tone="fg3">{props.hint}</Text>
      </If>
    </View>
  );
}

// ---------------------------------------------------------------- masked fields

/** What the masked fields share with TextField (the value and its events are theirs). `inputRef` gives the box's InputRef (focus(), setSelection()). */
export type MaskedFieldProps = Omit<TextFieldProps, "value" | "onChange" | "keyboardType" | "masked" | "maxLength">;

interface MaskedProps {
  /** The caller's TextField props (read where used, so they stay live). */
  field: MaskedFieldProps;
  value: string;
  onChange: (value: string) => void;
  mask: Mask;
  /** The text shown for a value, while the box has the keys and while it does not. */
  shown: (value: string, focused: boolean) => string;
  /** The value of a shown text. */
  valueOf: (text: string) => string;
  /** Two values mean the same (no reformat from outside, no report on leaving the box). */
  same?: (a: string, b: string) => boolean;
  /** The text a box the filer edited settles on when left (the amount rounded to centavos); unset: the shown one. */
  settle?: (text: string) => string;
  keyboardType: KeyboardType;
  /**
   * A number box (money, percent, integer): a box of zero is selected as it takes the keys by a click, so the first digit
   * replaces it; the box's own focus() (the form's step) selects the whole text whatever it holds.
   */
  numeric?: boolean;
  /** Defaults where the caller sets none. */
  placeholder?: string;
  prefix?: string;
  suffix?: string;
  align?: "left" | "right" | "center";
}

/**
 * A click's caret follows the focus it gave (the same mouse press): within this many ms of the focus handler's end, which can be
 * a long turn on a heavy page in a debug build. A caret later than that is the user's own move (an arrow after Tab). A caret
 * the window wrongly takes for a click's only selects a zero amount whole, which is what typing there wants anyway.
 */
const CLICK_MS = 500;

/**
 * A TextField whose text goes through a mask on every edit, the caret kept beside the same digit. The box always shows
 * what was reported: an edit the mask refuses is taken back out of the native box, and leaving a box the filer edited
 * reports the value it settled on (a typed 1.255 shown as 1.26 is 1.26 in the draft too) when that is not the one last
 * reported. A box left unedited never reports: a stored value is shown, not changed.
 *
 * Every way the native text changes comes to `change` as one edit of the text the box showed: typing, Backspace or Delete over
 * a caret or a selection (Cmd+A and Backspace, a drag and Delete), Cut, a paste, a digit over a selection. NSH reports each as a
 * key press (the typed text, or "Backspace" for any deletion, Cut included) BEFORE the change, with the selection the edit
 * started from known from the last onSelectionChange; an emptied box reports "" like any other value.
 *
 * Where the caret goes as the box takes the keys (macOS, onFocus then onSelectionChange; a click gives the focus, AppKit
 * selects all, then the click's caret follows within the same mouse press; Tab gives the focus and the whole text stays selected):
 * a Tab keeps the whole text selected; a click puts the caret where it landed, but in a number box holding zero the whole text is
 * selected so the first digit replaces it (typing at the caret of "0.00" gave "1,000.00" for 100); a box focused by its own
 * focus() (the form's step, which NSH gives a caret at the end) is selected whole.
 */
function MaskedField(props: MaskedProps) {
  let input: InputRef | undefined;
  const focused = signal(false);
  const text = signal(props.shown(props.value ?? "", false));
  const same = (a: string, b: string) => (props.same ? props.same(a, b) : a === b);
  // The value last reported or last given: a report is made only when it changes. `edited`: the filer changed it since the
  // box took the keys.
  let last = props.value ?? "";
  let edited = false;
  // The selection the input last reported, and what the current edit started from (taken at its key press): that
  // selection, and whether the key typed a character (a paste or any edit without a key is judged as a paste).
  let selected: TextSelection | undefined;
  let editFrom: TextSelection | undefined;
  let editTyped = false;
  // The text the box last reported while it has the keys. Keys that come faster than the answers (a burst, an agent's
  // typing) are typed on this text, not on the answer: NSH drops an answer the box has typed past.
  let boxText: string | undefined;
  // The box took the keys by itself (not through its focus()): until this time (Date.now(), 0: never) a caret is the click's that
  // gave them, and a key press ends it. A click's caret can come twice (AppKit places it as the press goes down and again when
  // it ends), so every caret in the window counts, not only the first.
  let clickUntil = 0;
  // The page's focus() was called: the box is selected whole, there is no click to wait for.
  let byFocus = false;
  // What the page gets as the box's ref: in a number box its focus() selects the whole text (NSH puts the caret at the end).
  const handle = (r: InputRef): InputRef =>
    props.numeric
      ? {
          focus() {
            byFocus = true;
            r.focus();
            r.setSelection(0, untracked(text).length);
          },
          blur: () => r.blur(),
          measure: () => r.measure(),
          reveal: (options) => r.reveal(options),
          setSelection: (start, end) => r.setSelection(start, end),
          insertText: (t) => r.insertText(t),
          animateLayout: (transition) => r.animateLayout(transition),
        }
      : r;
  // A value set from outside (another box, a reload) shows; the field's own edits come back equal and change nothing.
  effect(() => {
    const value = props.value ?? "";
    last = value;
    const now = untracked(text);
    if (!same(props.valueOf(now), value)) {
      // The filer emptied this box, and an amount comes back in it (a form that cannot hold a blank amount shows the draft's
      // again): selected whole, so the digits typed next replace it instead of being appended to it.
      const refilled = props.numeric && now === "" && edited && untracked(focused);
      const shown = props.shown(value, untracked(focused));
      text.set(shown);
      if (refilled && shown !== "") input?.setSelection(0, shown.length);
    }
  });
  const change = (raw: string) => {
    const prev = untracked(text);
    const from = editFrom;
    const typed = editTyped;
    editFrom = undefined;
    editTyped = false;
    const [on, at] = editBase(prev, boxText, raw, from);
    const next = reconcile(on, raw, props.mask, undefined, at, typed);
    // An edit that changes nothing is taken back: what the box holds after it is the text shown.
    boxText = next.rejected || next.text === prev ? next.text : raw;
    // The native box holds `raw`. A signal that already holds the answer would send nothing: show `raw` first so the
    // answer is a change the box must take.
    if (next.text === prev && raw !== prev) text.set(raw);
    text.set(next.text);
    // The box puts the caret at the end of a text it takes, so a caret that belongs there is not sent: sent, it would
    // arrive after the keys typed since this edit and put the next ones inside the number.
    if (next.rejected || (next.text !== raw && next.caret !== next.text.length)) input?.setSelection(next.caret);
    if (next.rejected) return;
    const value = props.valueOf(next.text);
    if (value === last) return;
    last = value;
    edited = true;
    props.onChange(value);
  };
  return (
    <TextField
      value={text()}
      onChange={change}
      keyboardType={props.keyboardType}
      inputRef={(r) => {
        input = r;
        props.field.inputRef?.(handle(r));
      }}
      tabular
      label={props.field.label}
      accessibilityLabel={props.field.accessibilityLabel}
      hint={props.field.hint}
      error={props.field.error}
      placeholder={props.field.placeholder ?? props.placeholder}
      disabled={props.field.disabled}
      locked={props.field.locked}
      width={props.field.width}
      size={props.field.size}
      align={props.field.align ?? props.align}
      icon={props.field.icon}
      prefix={props.field.prefix ?? props.prefix}
      suffix={props.field.suffix ?? props.suffix}
      trailing={props.field.trailing}
      autoFocus={props.field.autoFocus}
      returnKeyType={props.field.returnKeyType}
      onSubmit={props.field.onSubmit}
      testID={props.field.testID}
      onSelectionChange={(e) => {
        selected = { start: e.start, end: e.end };
        if (e.start === e.end && clickUntil > 0 && Date.now() <= clickUntil && props.numeric) {
          const now = untracked(text);
          if (isZeroAmount(now)) input?.setSelection(0, now.length);
        }
        props.field.onSelectionChange?.(e);
      }}
      onKeyPress={(e) => {
        clickUntil = 0;
        editFrom = selected;
        editTyped = [...e.key].length === 1;
        props.field.onKeyPress?.(e);
      }}
      onFocus={() => {
        focused.set(true);
        boxText = undefined;
        edited = false;
        props.field.onFocus?.();
        clickUntil = byFocus ? 0 : Date.now() + CLICK_MS;
        byFocus = false;
      }}
      onBlur={() => {
        focused.set(false);
        clickUntil = 0;
        byFocus = false;
        editFrom = undefined;
        editTyped = false;
        const laidOut = props.shown(props.valueOf(text()), false);
        const shown = edited && props.settle ? props.settle(laidOut) : laidOut;
        edited = false;
        if (shown !== text()) text.set(shown);
        const value = props.valueOf(shown);
        if (!same(value, last)) {
          last = value;
          props.onChange(value);
        }
        props.field.onBlur?.();
      }}
    />
  );
}

/** Props of TinInput. */
export interface TinInputProps extends MaskedFieldProps {
  /** The digits, up to 14 (9 for the TIN, 5 for the branch code); no dashes. */
  value: string;
  onChange: (digits: string) => void;
}

/** A TIN typed as 000-000-000-00000: digits only, dashes put in while typing, leading zeros kept, tabular figures. */
export function TinInput(props: TinInputProps) {
  return (
    <MaskedField
      field={props}
      value={props.value}
      onChange={props.onChange}
      placeholder="000-000-000-00000"
      mask={TIN_MASK}
      shown={(v) => (/^[\d\s-]*$/.test(v) && digitsOnly(v).length <= TIN_LENGTH ? formatTinDigits(v) : v)}
      valueOf={(t) => digitsOnly(t).slice(0, TIN_LENGTH)}
      keyboardType="number-pad"
    />
  );
}

/** Props of MoneyInput. */
export interface MoneyInputProps extends MaskedFieldProps {
  /** The plain number as text: "1250000.50", "-250", "" for none. A negative value is shown negative whatever `negative` says. */
  value: string;
  /**
   * The plain number as text after each edit: digits, one "." and a leading "-", no separators or ₱ ("1250000.5", "-250",
   * ".5" while typing, "" for none; whatever else arrives is shown as it is and reported unchanged). Leaving the box
   * reports again only when its formatting changed the number (a typed 1.255 shown as 1.26 reports "1.26"; 1250000.5
   * shown as 1,250,000.50 reports nothing).
   */
  onChange: (plain: string) => void;
  /** Whole pesos: leaving the box shows "1,234" rather than "1,234.00". */
  whole?: boolean;
  /** What precedes the digits; "" for a bare number (an excise volume in metric tons, a count of shares). Default ₱. */
  prefix?: string;
  /** A minus may be typed or pasted; unset, a pasted negative is refused and a typed minus does nothing (a negative value that is there stays negative). */
  negative?: boolean;
}

/**
 * An amount: digits and one decimal point, thousands separators while typing, right-aligned in tabular figures, a ₱
 * before it; leaving the box writes two decimals (formatMoney, on the digits: decimal rounding half up, any length). The
 * value out is the plain numeric string, and the box never shows a number other than the one reported.
 */
export function MoneyInput(props: MoneyInputProps) {
  return (
    <MaskedField
      field={props}
      value={props.value}
      onChange={props.onChange}
      prefix={props.prefix ?? "₱"}
      align="right"
      mask={moneyMask({ negative: props.negative })}
      shown={(v, focused) => shownMoney(v, focused, props.whole)}
      settle={(t) => formatMoney(t, props.whole)}
      valueOf={(t) => moneyPlain(t)}
      same={sameAmount}
      keyboardType="decimal-pad"
      numeric
    />
  );
}

/** Props of PercentInput and IntInput. */
export interface NumberTextProps extends MaskedFieldProps {
  value: string;
  onChange: (value: string) => void;
}

/** A percent: digits and one decimal point, right-aligned in tabular figures, a % after it. Reports the text as typed ("12.5"). */
export function PercentInput(props: NumberTextProps) {
  return (
    <MaskedField
      field={props}
      value={props.value}
      onChange={props.onChange}
      suffix="%"
      align="right"
      mask={PERCENT_MASK}
      shown={(v) => v}
      valueOf={(t) => t}
      keyboardType="decimal-pad"
      numeric
    />
  );
}

/** A whole number or a code: digits only (leading zeros kept), at most `maxDigits`, tabular figures. Reports the digits ("0123"). */
export function IntInput(props: NumberTextProps & { maxDigits?: number }) {
  return (
    <MaskedField
      field={props}
      value={props.value}
      onChange={props.onChange}
      mask={intMask(props.maxDigits)}
      shown={(v) => v}
      valueOf={(t) => t}
      keyboardType="number-pad"
      numeric
    />
  );
}

// ---------------------------------------------------------------- date field

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/** A calendar item's content: chevrons, weekday and month names, the year, the day; its color is the item's (state classes). */
function CalendarItem(props: { item: CalendarEntry }) {
  const kind = props.item.kind;
  if (kind === "previous" || kind === "next") return <Icon name={kind === "previous" ? "chevron-left" : "chevron-right"} size={14} tone="fg2" />;
  if (kind === "weekday") return <NText class="text-cap font-medium text-subtle-foreground">{WEEKDAYS[props.item.value] ?? ""}</NText>;
  const label =
    kind === "monthToggle" || kind === "month" ? MONTHS[props.item.value - 1] ?? "" : String(props.item.value);
  const strong = kind === "monthToggle" || kind === "yearToggle";
  return <NText class={cx("text-body tabular-nums", strong ? "font-semibold" : props.item.today ? "font-semibold" : "font-normal")}>{label}</NText>;
}

/** Props of CalendarPicker (the kit's own: DateField and CalendarButton draw it). */
interface CalendarPickerProps {
  /** The picked day as an ISO date (yyyy-mm-dd), or null. */
  value: string | null;
  /** The initial month when there is no selected date. */
  start?: string;
  /** Controlled open state; unset, the picker keeps its own. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** A day was picked: its ISO date (yyyy-mm-dd). */
  onPick: (iso: string) => void;
  /** What screen readers call the calendar. */
  label: string;
  triggerID?: string;
  disabled?: boolean;
  /** The first and the last day that can be picked (ISO dates). */
  min?: string;
  max?: string;
}

/** An inline calendar for a date sheet. */
export function CalendarGrid(props: { value: string | null; start: string; min?: string; max?: string; onPick: (iso: string) => void }) {
  return (
    <Calendar
      value={props.value || props.start}
      disabledMatcher={props.min || props.max ? { before: props.min ?? null, after: props.max ?? null } : null}
      previousAccessibilityLabel="Previous month"
      nextAccessibilityLabel="Next month"
      headerClass="mb-2 flex-row items-center justify-between gap-1"
      rowClass="flex-row justify-between"
      itemClass={cx(
        "h-11 min-w-11 items-center justify-center rounded-(--radius-ctl) px-1 text-foreground hover:bg-hover focus:bg-hover data-outside:text-subtle-foreground disabled:opacity-40",
        props.value && "selected:bg-primary selected:hover:bg-primary selected:focus:bg-primary selected:text-primary-foreground",
      )}
      renderItem={(item: CalendarEntry) => <CalendarItem item={item} />}
      onChange={(event) => {
        const date = typeof event.value === "string" ? parseTypedDate(event.value.slice(0, 10)) : null;
        if (date) props.onPick(formatIso(date));
      }}
    />
  );
}

/** The calendar's icon button and its popup (a sheet on touch): the one DatePicker the kit draws, with its look. */
function CalendarPicker(props: CalendarPickerProps) {
  return (
    <DatePicker
      value={props.value || props.start || null}
      open={props.open}
      onOpenChange={(e) => props.onOpenChange?.(e.open)}
      onChange={(e) => {
        const v = typeof e.value === "string" ? parseTypedDate(e.value.slice(0, 10)) : null;
        if (v) props.onPick(formatIso(v));
      }}
      disabled={props.disabled}
      disabledMatcher={props.min || props.max ? { before: props.min ?? null, after: props.max ?? null } : null}
      accessibilityLabel={props.label}
      previousAccessibilityLabel="Previous month"
      nextAccessibilityLabel="Next month"
      offset={6}
      margin={8}
      popupClass={TOUCH ? "rounded-t-(--radius-dialog) bg-card p-4 pb-8" : "rounded-(--radius-card) border border-border bg-raised p-3 shadow-lg"}
      backdropClass="bg-scrim"
      headerClass="mb-2 flex-row items-center justify-between gap-1"
      renderItem={(item: CalendarEntry) => <CalendarItem item={item} />}
      rowClass="flex-row justify-between"
      itemClass={cx(
        "items-center justify-center rounded-(--radius-ctl) px-1 text-foreground hover:bg-hover focus:bg-hover data-outside:text-subtle-foreground disabled:opacity-40",
        props.value && "selected:bg-primary selected:hover:bg-primary selected:focus:bg-primary selected:text-primary-foreground",
        TOUCH ? cx(T_H, T_MIN_W) : "h-8 min-w-8",
      )}
      trigger={
        <View
          testID={props.triggerID}
          role="button"
          accessibilityLabel={props.label}
          class={cx("items-center justify-center rounded-(--radius-ctl) hover:bg-hover active:bg-selected", TOUCH ? cx("-mr-2", T_SIZE) : "-mr-1.5 size-6")}
        >
          <Icon name="calendar" size={14} tone="fg3" />
        </View>
      }
    />
  );
}

/** Props of CalendarButton. */
export interface CalendarButtonProps {
  /** The trigger's testID (and what a test presses). */
  id: string;
  /** The picked day as an ISO date (yyyy-mm-dd), or "" for none. */
  value: string;
  /** A day was picked: its ISO date (yyyy-mm-dd). */
  onPick: (iso: string) => void;
  disabled?: boolean;
  /** The first and the last day that can be picked (ISO dates). */
  min?: string;
  /** See min. */
  max?: string;
}

/**
 * A calendar button on its own: the icon that DateField carries in its box, for a date entered in split boxes (the form page's
 * Year Ended month and year). A press opens the popup (a sheet on touch); a day picked calls onPick with its ISO date.
 */
export function CalendarButton(props: CalendarButtonProps) {
  const open = signal(false);
  return (
    <CalendarPicker
      value={props.value ? props.value.slice(0, 10) : null}
      open={open()}
      onOpenChange={(o) => open.set(o)}
      onPick={props.onPick}
      label="Pick a date"
      triggerID={props.id}
      disabled={props.disabled}
      min={props.min}
      max={props.max}
    />
  );
}

/** Props of DateField. */
export interface DateFieldProps extends MaskedFieldProps {
  /** The stored text: MM/DD/YYYY once it reads as a date, else as typed. */
  value: string;
  /** MM/DD/YYYY as soon as the text reads as a date (12102020, 12-10-2020 and 12/10/2020 alike), else the text. */
  onChange: (value: string) => void;
  min?: string;
  max?: string;
  calendarStart?: string;
  hideCalendar?: boolean;
  numericDisplay?: boolean;
  onPick?: (iso: string) => void;
}

/**
 * A date typed as text or picked from a calendar: the draft gets MM/DD/YYYY as soon as the text reads as a date,
 * leaving the box shows it as "Dec 10, 2020", and a text that is no date shows its message only after leaving the box.
 */
export function DateField(props: DateFieldProps) {
  const focused = signal(false);
  const edited = signal(false);
  const open = signal(false);
  let box: InputRef | undefined;
  const long = (v: string) => {
    const d = parseTypedDate(v);
    return d ? props.numericDisplay ? formatMDY(d) : formatLongDate(d) : v;
  };
  const text = signal(long(props.value ?? ""));
  effect(() => {
    const v = props.value ?? "";
    const now = untracked(text);
    const d = parseTypedDate(now);
    if ((d ? formatMDY(d) : now) !== v) text.set(untracked(focused) ? v : long(v));
  });
  const iso = () => {
    const d = parseTypedDate(text());
    return d ? formatIso(d) : null;
  };
  const bad = () => !focused() && edited() && text().trim() !== "" && !parseTypedDate(text());
  return (
    <TextField
      label={props.label}
      accessibilityLabel={props.accessibilityLabel}
      accessibilityDescribedBy={props.accessibilityDescribedBy}
      hint={props.hint}
      placeholder={props.placeholder ?? "MM/DD/YYYY"}
      disabled={props.disabled}
      locked={props.locked}
      width={props.width}
      size={props.size}
      testID={props.testID}
      autoFocus={props.autoFocus}
      returnKeyType={props.returnKeyType}
      onSubmit={props.onSubmit}
      inputRef={(r) => {
        box = r;
        props.inputRef?.(r);
      }}
      tabular
      value={text()}
      error={props.error || (bad() ? DATE_ERROR : false)}
      keyboardType="numbers-and-punctuation"
      onChange={(raw) => {
        edited.set(true);
        text.set(raw);
        const d = parseTypedDate(raw);
        const answer = d ? formatMDY(d) : raw;
        // Eight digits (08171988) rewrite to MM/DD/YYYY at once, the caret at the end (pitfall 23:
        // the signal takes the raw text and then the answer in the same handler).
        if (d && /^\d{8}$/.test(raw.trim())) {
          text.set(answer);
          box?.setSelection(answer.length, answer.length);
        }
        props.onChange(answer);
      }}
      onFocus={() => {
        focused.set(true);
        // The long form becomes the numeric date while the box has the keys.
        const d = parseTypedDate(text());
        if (d) text.set(formatMDY(d));
        props.onFocus?.();
      }}
      onBlur={() => {
        focused.set(false);
        text.set(long(text()));
        props.onBlur?.();
      }}
      trailing={
        <If when={!props.hideCalendar && !props.locked && !props.disabled}>
          <CalendarPicker
            value={iso()}
            start={props.calendarStart}
            open={open()}
            onOpenChange={(o) => open.set(o)}
            label={`Pick ${props.label ?? props.accessibilityLabel ?? "a date"}`}
            triggerID={props.testID ? `${props.testID}-calendar` : undefined}
            min={props.min}
            max={props.max}
            onPick={(iso) => {
              const v = parseTypedDate(iso);
              if (!v) return;
              text.set(focused() || props.numericDisplay ? formatMDY(v) : formatLongDate(v));
              edited.set(true);
              if (props.onPick) props.onPick(iso);
              else props.onChange(formatMDY(v));
            }}
          />
        </If>
      }
    />
  );
}

// ---------------------------------------------------------------- PIN and codes

/** Props of PinCells. */
export interface PinCellsProps {
  /** 4 for a PIN, 6 for an authenticator code. */
  length: number;
  value: string;
  onChange: (value: string) => void;
  /** The last cell was filled: the code, to check. */
  onComplete?: (value: string) => void;
  /** Bullets instead of digits (a PIN). */
  masked?: boolean;
  error?: boolean;
  disabled?: boolean;
  /** What the code is for, for screen readers. */
  label: string;
  testID?: string;
  /** Receives the function that puts the keys in the cells (a screen hands them on from another box). */
  focusRef?: (focus: () => void) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  /**
   * The sign-in code page's large boxes: this many points wide, 56 tall, 23-point digits, 6 apart and 14 between the two
   * groups. Unset: the kit's boxes.
   */
  large?: { width: number };
}

/** PIN and one-time-code cells: 32-point boxes (44 on touch), radius 8, the accent on the cell that takes the next digit. */
export function PinCells(props: PinCellsProps) {
  return (
    <OtpInput
      testID={props.testID}
      ref={(r: OtpInputRef) => props.focusRef?.(() => r.focus())}
      onFocus={() => props.onFocus?.()}
      onBlur={() => props.onBlur?.()}
      length={props.length}
      value={props.value}
      onChange={(e) => props.onChange(digitsOnly(e.value))}
      onComplete={(e) => props.onComplete?.(digitsOnly(e.value))}
      masked={props.masked}
      disabled={props.disabled}
      groups={props.length === 6 ? 2 : 1}
      accessibilityLabel={`${props.label}, ${props.length} digits`}
      class={cx("flex-row disabled:opacity-50", props.large ? "gap-3.5" : "gap-3")}
      groupClass="flex-row gap-1.5"
      cellClass={cx(
        "items-center justify-center rounded-(--radius-row) border bg-card font-semibold tabular-nums text-foreground focus:border-accent-blue",
        props.error ? "border-danger" : "border-border-strong",
        props.large ? undefined : cx("text-emph", TOUCH ? "h-12 w-11" : "size-8"),
      )}
      cellStyle={props.large ? { width: props.large.width, height: 56, fontSize: 23 } : undefined}
      caretClass={props.large ? "h-6 w-0.5 rounded-full bg-foreground" : "h-4 w-0.5 rounded-full bg-foreground"}
    />
  );
}

/** Props of DigitCells. */
export interface DigitCellsProps {
  /** How many digits: 9 for a TIN, 5 for a branch code. */
  length: number;
  /** How many groups the cells are drawn in, a dash between them (3 for a TIN: 000-000-000; 1, the default, for a branch code). */
  groups?: number;
  /** The digits typed so far (no dashes). */
  value: string;
  onChange: (value: string) => void;
  /** The last cell was filled: the digits. */
  onComplete?: (value: string) => void;
  /** The cells lost the keys (a screen that checks a box when it is left). */
  onBlur?: () => void;
  error?: boolean;
  disabled?: boolean;
  /** What the digits are for, for screen readers. */
  label: string;
  testID?: string;
  /** Receives the function that puts the keys in the cells (the box before it hands over its last digit). */
  focusRef?: (focus: () => void) => void;
}

/** A cell of DigitCells, in points: the old app's 24 wide, 32 tall (28 by 44 on touch), 3 apart; groups sit `DIGIT_GROUP_GAP` apart with a dash. */
const DIGIT_CELL = TOUCH ? { w: 28, h: 44 } : { w: 24, h: 32 };
const DIGIT_CELL_GAP = 3;
const DIGIT_GROUP_GAP = 17;

/**
 * The digits of a TIN or a branch code as one cell each (the old DigitCells): one OtpInput of `length` cells, drawn in `groups`
 * groups with a dash between them, so 000-123-453 is nine cells in three groups. The last digit calls onComplete (the screen moves
 * the keys on, see focusRef). nsh/base's OtpInput draws the gap between its groups but not a mark in it: the dashes are texts laid
 * over those gaps (the groups' widths are fixed, so their places are known) that take no press.
 */
export function DigitCells(props: DigitCellsProps) {
  const groups = () => Math.max(1, props.groups ?? 1);
  const per = () => Math.ceil(props.length / groups());
  const groupWidth = () => per() * DIGIT_CELL.w + (per() - 1) * DIGIT_CELL_GAP;
  const dashes = () => Array.from({ length: groups() - 1 }, (_, i) => i);
  return (
    <View class="relative self-start">
      <OtpInput
        testID={props.testID}
        ref={(r: OtpInputRef) => props.focusRef?.(() => r.focus())}
        length={props.length}
        value={props.value}
        onChange={(e) => props.onChange(digitsOnly(e.value).slice(0, props.length))}
        onComplete={(e) => props.onComplete?.(digitsOnly(e.value).slice(0, props.length))}
        onBlur={() => props.onBlur?.()}
        disabled={props.disabled}
        groups={groups()}
        accessibilityLabel={`${props.label}, ${props.length} digits`}
        class="flex-row disabled:opacity-50"
        style={{ gap: DIGIT_GROUP_GAP }}
        groupClass="flex-row gap-0.75"
        cellClass={cx(
          "items-center justify-center rounded-(--radius-ctl) border bg-card text-emph font-semibold tabular-nums text-foreground focus:border-accent-blue",
          props.error ? "border-danger" : "border-border-strong",
        )}
        cellStyle={{ width: DIGIT_CELL.w, height: DIGIT_CELL.h }}
        caretClass="h-3.5 w-0.5 rounded-full bg-foreground"
      />
      <Each of={dashes()}>
        {(i) => (
          <View
            testID={props.testID ? `${props.testID}-dash-${i() + 1}` : undefined}
            class="absolute top-0 bottom-0 items-center justify-center"
            style={{ left: (i() + 1) * groupWidth() + i() * DIGIT_GROUP_GAP, width: DIGIT_GROUP_GAP, pointerEvents: "none" }}
          >
            <Text tone="fg3">-</Text>
          </View>
        )}
      </Each>
    </View>
  );
}

// ---------------------------------------------------------------- field layout

/** Props of Field. */
export interface FieldProps {
  label: string;
  /** Read mode: the value as text ("Not set" in fg3 when empty). */
  value?: string | null;
  /** Edit mode: the control (a TextField without its own label). */
  children?: JSX.Element;
  /** The message under the control. */
  error?: string;
  /** The value comes from the head office: in fg2, with a "Head office" chip. */
  inherited?: boolean;
  /** Edit mode: a Switch "Same as head office" in place of the chip. */
  inheritSwitch?: JSX.Element;
  width?: number;
}

/** A labelled value or control: label 12 fg2 over a 13-point value or the control. */
export function Field(props: FieldProps) {
  const empty = () => props.value === undefined || props.value === null || props.value === "";
  return (
    <View class="min-w-0" style={{ width: props.width }}>
      <View class="mb-1 min-h-5 flex-row items-center gap-2">
        <Text variant="meta" tone="fg2" class="flex-1">{props.label}</Text>
        <If when={props.inheritSwitch}>
          <View class="flex-row items-center gap-1.5">
            <Text variant="cap" tone="fg3">Same as head office</Text>
            {props.inheritSwitch}
          </View>
        </If>
        <If when={!props.inheritSwitch && props.inherited}>
          <Chip label="Head office" icon="git-branch" />
        </If>
      </View>
      <If
        when={props.children}
        else={<Text tone={empty() ? "fg3" : props.inherited ? "fg2" : "fg"}>{empty() ? "Not set" : String(props.value)}</Text>}
      >
        {props.children}
        <If when={props.error}>
          <Text variant="meta" tone="danger" class="mt-1">{props.error}</Text>
        </If>
      </If>
    </View>
  );
}

/**
 * Two columns of fields, 16 by 32 apart. A cell shares its row only while it stays at least 280 wide (the 9-digit TIN,
 * the longest placeholder); otherwise it wraps onto its own line. A `full` cell always takes a row.
 */
export function FieldGrid(props: { children?: JSX.Element }) {
  return <View class="w-full min-w-0 flex-row flex-wrap items-start gap-x-8 gap-y-4">{props.children}</View>;
}

/** One cell of a FieldGrid. */
export function FieldCell(props: { full?: boolean; children?: JSX.Element }) {
  return (
    <View class={props.full ? "w-full shrink-0" : "min-w-70 flex-1 basis-0"}>
      {props.children}
    </View>
  );
}

/** Cancel (ghost) and a primary save at the bottom right of a section. */
export function FormActions(props: { onCancel: () => void; onSave: () => void; saveLabel?: string; saveDisabled?: boolean; testID?: string }) {
  return (
    <View class="mt-5 flex-row justify-end gap-2">
      <AppButton label="Cancel" variant="ghost" size="sm" testID={props.testID ? `${props.testID}-cancel` : undefined} onPress={props.onCancel} />
      <AppButton
        label={props.saveLabel ?? "Save changes"}
        variant="primary"
        size="sm"
        disabled={props.saveDisabled}
        testID={props.testID ? `${props.testID}-save` : undefined}
        onPress={props.onSave}
      />
    </View>
  );
}
