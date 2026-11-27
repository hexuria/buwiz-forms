import { cleanup, signal, untracked, View } from "nsh";
import { Button } from "nsh/base";
import { DATE_ERROR, formatIso, formatLongDate, formatMDY, isTwoDigitDate, parseTypedDate } from "../lib/dates";
import { AppButton } from "./buttons";
import { CalendarGrid, DateField } from "./fields";
import { Sheet } from "./layers";
import { cx, FOCUS_RING, T_MIN_H } from "./look";
import { Icon, Line } from "./text";

export interface DateTouchFieldProps {
  label: string;
  errorDescription?: string;
  value: string;
  onChange: (value: string) => void;
  start: string;
  min?: string;
  max?: string;
  error?: boolean;
  disabled?: boolean;
  testID: string;
  validate?: (value: string) => string;
  onFocus?: () => void;
  onBlur?: () => void;
  /**
   * Opens the date sheet once, just after the row is drawn: a link that came to set this date (a form's "Set the business start
   * date") lands in the sheet, as a wide window's link lands with the keys in the DateField. A short wait lets a sheet the row is in
   * finish coming up first.
   */
  autoOpen?: boolean;
  /** autoOpen opened the sheet: the caller stops asking for it, so a remount of the row (a resize between layouts) does not again. */
  onAutoOpened?: () => void;
}

/** How long autoOpen waits for a sheet around the row to come in, in milliseconds. */
const AUTO_OPEN_MS = 350;

/** A touch-sized row whose sheet holds a tentative date until Done or Clear. */
export function DateTouchField(props: DateTouchFieldProps) {
  const open = signal(false);
  const text = signal("");
  const tried = signal(false);
  const parsed = () => parseTypedDate(text());
  const problem = () => {
    if (props.validate) return props.validate(text());
    const date = parsed();
    if (!date && text().trim()) {
      return isTwoDigitDate(text()) ? "Type the four-digit year, for example 08/17/1988." : DATE_ERROR;
    }
    if (date && ((props.min && formatIso(date) < props.min) || (props.max && formatIso(date) > props.max))) return DATE_ERROR;
    return "";
  };
  const show = () => {
    const date = parseTypedDate(props.value);
    return date ? formatLongDate(date) : props.value;
  };
  const close = () => { open.set(false); props.onBlur?.(); };
  const openSheet = () => {
    const date = parseTypedDate(props.value);
    text.set(date ? formatMDY(date) : props.value);
    tried.set(false);
    props.onFocus?.();
    open.set(true);
  };
  if (untracked(() => props.autoOpen && !props.disabled)) {
    const timer = setTimeout(() => {
      openSheet();
      props.onAutoOpened?.();
    }, AUTO_OPEN_MS);
    cleanup(() => clearTimeout(timer));
  }
  const submit = () => {
    tried.set(true);
    if (problem()) return;
    props.onChange(parsed() ? formatMDY(parsed()!) : "");
    close();
  };
  return (
    <>
      <Button
        testID={props.testID}
        accessibilityLabel={`${props.label}, ${show() || "not set"}${props.errorDescription ? `, ${props.errorDescription}` : ""}`}
        disabled={props.disabled}
        class={cx(T_MIN_H, "w-full flex-row items-center gap-2 rounded-(--radius-row) border bg-card px-2.5", FOCUS_RING,
          props.error ? "border-danger" : "border-border-strong")}
        onPress={openSheet}
      >
        <Line tone={show() ? "fg" : "fg3"} class="flex-1 tabular-nums">{show() || "MM/DD/YYYY"}</Line>
        <Icon name="calendar" size={14} tone="fg3" />
      </Button>
      <Sheet
        open={open()}
        onOpenChange={(next) => { if (!next) close(); }}
        title={props.label}
        testID={`${props.testID}-sheet`}
        footer={
          <>
            <AppButton label="Clear" variant="ghost" testID={`${props.testID}-sheet-clear`} onPress={() => { props.onChange(""); close(); }} />
            <View class="flex-1">
              <AppButton label="Done" variant="primary" block testID={`${props.testID}-sheet-done`} onPress={submit} />
            </View>
          </>
        }
      >
        <DateField
          testID={`${props.testID}-sheet-input`}
          accessibilityLabel={props.label}
          value={text()}
          onChange={(value) => text.set(value)}
          error={tried() ? problem() : false}
          hideCalendar
          numericDisplay
        />
        <CalendarGrid
          value={parsed() ? formatIso(parsed()!) : null}
          start={props.start}
          min={props.min}
          max={props.max}
          onPick={(iso) => {
            const date = parseTypedDate(iso);
            if (date) { text.set(formatMDY(date)); tried.set(false); }
          }}
        />
      </Sheet>
    </>
  );
}
