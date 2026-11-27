// One box of the form: the control for its kind (text, money, pct, int, email, phone, date, sel, chk), built only from
// nsh/ui. Dumb: it draws what its props say and reports typing (onEdit), focus and blur; the session decides the rest.
//
// What it reports (the same strings the old page sent to screen_form_fill, which sent what was typed and showed the amounts
// grouped only for the eye): money the plain number ("1250000.50", no separators, no peso sign); pct as typed; int digits;
// a TIN its digits (no dashes); a date MM/DD/YYYY once the text reads as a date, else the text; sel the code; chk
// "true"/"false". Locked, computed and disabled boxes draw as read-only boxes (no Input) and report nothing.
import type { ViewRef } from "nsh";
import { cleanup, View } from "nsh";
import {
  CheckboxLabel, DateField, Dropdown, Icon, IntInput, Line, MoneyInput, moneyPlain, PercentInput, T_H, T_MIN_H, Text, TextField, TinInput, Tooltip, TOUCH,
} from "../../../ui";
import { SearchPick } from "./Pick";
import { cx, isOn, listedOptions, mdy, readBoxPad, readText, SEARCH_FROM, selectModel, tinKind, tinSegment } from "./model";
import { probing, registerMeasure } from "../layoutProbe";
import { type FieldInput, read } from "./types";

const OFF_OPACITY = 0.45;

/** The box takes no typing: the host locks it, the form computes it, or the page has switched it off. */
function shut(p: FieldInput): boolean {
  return p.box.locked() || !!p.field.c || !!read(p.disabled, false);
}

/** Why a locked box is locked: the page's words, else what an auto-filled box means. */
function reasonOf(p: FieldInput): string {
  const given = read(p.reason, "");
  if (given) return given;
  return p.field.a && p.box.locked() && !p.field.c ? "From tax profile" : "";
}

/** The accessible name of a box: its label, and that it is required. */
function nameOf(p: FieldInput): string {
  return p.field.r ? `${p.label}, required` : p.label;
}

/** The width a box has: the page's on a wide layout, the whole room on a narrow one. */
function widthOf(p: FieldInput): number | undefined {
  return p.narrow() ? undefined : p.width;
}

/** A cell of the phone TIN grid: centred tabular digits, and a locked one carries no lock of its own (the row's "From the tax profile" line says it once). */
function cellOf(p: FieldInput): boolean {
  return !!p.tinCell;
}

/** A box that takes no typing, as a read-only box: computed (flat, in the ink of a result), locked (a lock, fainter), disabled (dimmed). */
function ReadBox(props: { p: FieldInput }) {
  const p = props.p;
  const computed = () => !!p.field.c;
  const dimmed = () => !!read(p.disabled, false) && !p.box.locked() && !computed();
  const text = () => readText(p);
  const hint = () => (text() ? "" : p.box.hint());
  const right = () => !hint() && (p.field.t === "money" || p.field.t === "pct");
  const figures = () => p.field.t === "money" || p.field.t === "num" || p.field.t === "pct" || p.field.t === "int" || p.field.t === "date" || cellOf(p);
  const why = () => reasonOf(p);
  // The narrow boxes of a TIN (58, 76 wide) have no room for a lock, and neither has a phone grid's cell: they read centred.
  const small = () => (!p.narrow() && p.width < 100) || cellOf(p);
  // A hint is never cut mid-word (F9-4: "Shows once the period is set" in an 88-wide year box read "Shows o…"): while it
  // shows, the box grows to the hint, up to the row's width, where the hint wraps.
  const grows = () => Boolean(hint());
  return (
    <View
      testID={p.testID}
      accessibilityLabel={`${nameOf(p)}: ${text() || hint() || "empty"}${why() ? ", " + why().toLowerCase() : ""}`}
      tooltip={why() ? <Tooltip label={why()} /> : undefined}
      class={cx(
        "min-w-0 flex-row items-center gap-1.5 rounded-(--radius-row) border",
        readBoxPad(cellOf(p)) === 4 ? "px-1" : "px-2.5",
        TOUCH ? T_MIN_H : grows() ? "min-h-8 py-1" : "h-8",
        grows() && "max-w-full",
        computed() ? "border-transparent bg-hover" : "border-border bg-hover",
        dimmed() && "opacity-60",
        p.narrow() && "w-full",
      )}
      style={grows() ? { minWidth: widthOf(p) } : { width: widthOf(p) }}
    >
      <If when={p.field.t === "money" && text()}>
        <Text variant="meta" tone="fg3">₱</Text>
      </If>
      <If
        when={grows()}
        else={
          <Line
            tone={hint() ? "fg3" : computed() ? "fg" : "fg2"}
            class={cx("flex-1", right() && "text-right", small() && "text-center", figures() && "tabular-nums", computed() && !hint() && "font-semibold")}
          >
            {text() || hint()}
          </Line>
        }
      >
        <Text testID={p.testID ? `${p.testID}-hint` : undefined} tone="fg3" class="min-w-0 shrink">
          {hint()}
        </Text>
      </If>
      <If when={p.field.t === "pct" && text()}>
        <Text variant="meta" tone="fg3">%</Text>
      </If>
      <If when={!computed() && !dimmed() && !small()}>
        <Icon name="lock" size={14} tone="fg3" />
      </If>
    </View>
  );
}

/** A tick box with its words beside it; a pick reports once ("true" or "false"). */
function TickBox(props: { p: FieldInput }) {
  const p = props.p;
  return (
    <CheckboxLabel
      testID={p.testID}
      label={p.label}
      focusRef={(ref) => giveFocus(p, ref)}
      checked={isOn(p.box.value())}
      disabled={shut(p)}
      hint={shut(p) && !isOn(p.box.value()) ? p.box.hint() : undefined}
      onChange={(on) => {
        if (shut(p)) return;
        p.onFocus();
        p.onEdit(on ? "true" : "false");
        p.onBlur();
      }}
    />
  );
}

/** A choice: a dropdown for a few, a search list for many. A pick reports once, the code. */
function Choice(props: { p: FieldInput }) {
  const p = props.p;
  const model = () => selectModel(listedOptions(p.field, p.box.options()), p.box.value());
  const pick = (v: string) => {
    p.onFocus();
    p.onEdit(v);
    p.onBlur();
  };
  return (
    <If
      when={model().options.length > SEARCH_FROM}
      else={
        // The kit's select root is a few points taller than its trigger: a box of the trigger's own height keeps neighbours aligned.
        <View class={cx("min-w-0", TOUCH ? T_H : "h-8", p.narrow() && "w-full")} style={{ width: widthOf(p) }}>
          <Dropdown
            testID={p.testID}
            label={nameOf(p)}
            options={model().options}
            value={model().shown || null}
            placeholder="Choose…"
            width={widthOf(p)}
            error={p.box.invalid() || Boolean(p.box.message?.())}
            focusRef={(ref) => giveFocus(p, ref)}
            onChange={pick}
          />
        </View>
      }
    >
      {/* SearchPick is a popover and a sheet side by side: in the box's own column the closed sheet took a 4-point gap and
          the trigger sat higher than its neighbours (a multi-control row aligns at the bottom). One box of the trigger's height. */}
      <View class={cx("min-w-0", TOUCH ? T_H : "h-8", p.narrow() && "w-full")} style={{ width: widthOf(p) }}>
        <SearchPick
          testID={p.testID}
          label={p.label}
          options={model().options}
          value={model().shown}
          width={widthOf(p)}
          error={p.box.invalid() || Boolean(p.box.message?.())}
          focusRef={(ref) => giveFocus(p, ref)}
          onPick={pick}
        />
      </View>
    </If>
  );
}

/** A date typed as text (12012020, 12/01/2020, Dec 1 2020 all read) or picked from the calendar. */
function DateBox(props: { p: FieldInput }) {
  const p = props.p;
  // The kit's DateField prints "Enter a valid date..." under its box; the page prints the same sentence on the item's own
  // message line (and the box's red border), so the box is clipped to its own height and the sentence shows once.
  return (
    <View class={cx("min-w-0 overflow-hidden", TOUCH ? T_H : "h-8", p.narrow() && "w-full")} style={{ width: widthOf(p) }}>
      <DateField
        testID={p.testID}
        accessibilityLabel={nameOf(p)}
        value={mdy(p.box.value())}
        error={p.box.message?.() || p.box.invalid()}
        placeholder={p.placeholder}
        onChange={(text) => p.onEdit(text)}
        onFocus={() => p.onFocus()}
        onBlur={() => p.onBlur()}
        inputRef={(ref) => giveFocus(p, ref)}
      />
    </View>
  );
}

/** The ref a box gives the page (an input's, a select trigger's, a checkbox's), so a pill step can put the keys in it. */
function giveFocus(p: FieldInput, ref: ViewRef) {
  p.focusRef?.(() => ref.focus());
  p.blurRef?.(() => ref.blur());
}

/** The typing boxes: an amount, a percent, a whole number, a TIN, an email, a phone, plain text. */
function TypedBox(props: { p: FieldInput }) {
  const p = props.p;
  const kind = () => p.field.t;
  const tin = () => tinKind(p.field);
  const countOnly = () => kind() === "int" && tin() === null && !tinSegment(p.field) && !/code/i.test(`${p.field.k} ${p.field.l}`);
  return (
    <If
      when={kind() === "money" || kind() === "num"}
      else={
        <If
          when={kind() === "pct"}
          else={
            <If
              when={kind() === "int" || tin() === "part" || (cellOf(p) && tin() === null && tinSegment(p.field))}
              else={
                <If
                  when={tin() === "full"}
                  else={
                    <TextField
                      testID={p.testID}
                      accessibilityLabel={nameOf(p)}
                      value={p.box.value()}
                      onChange={(text) => p.onEdit(text)}
                      onFocus={() => p.onFocus()}
                      onBlur={() => p.onBlur()}
                      width={widthOf(p)}
                      error={p.box.message?.() || p.box.invalid()}
                      maxLength={p.field.m}
                      placeholder={p.placeholder}
                      keyboardType={kind() === "email" ? "email-address" : kind() === "phone" ? "phone-pad" : undefined}
                      autoCapitalize={kind() === "email" ? "none" : undefined}
                      inputRef={(ref) => giveFocus(p, ref)}
                    />
                  }
                >
                  <TinInput
                    testID={p.testID}
                    accessibilityLabel={nameOf(p)}
                    value={p.box.value()}
                    onChange={(digits) => p.onEdit(digits)}
                    onFocus={() => p.onFocus()}
                    onBlur={() => p.onBlur()}
                    width={widthOf(p)}
                    align={cellOf(p) ? "center" : undefined}
                    error={p.box.message?.() || p.box.invalid()}
                    inputRef={(ref) => giveFocus(p, ref)}
                  />
                </If>
              }
            >
              <IntInput
                testID={p.testID}
                accessibilityLabel={nameOf(p)}
                value={p.box.value()}
                onChange={(digits) => p.onEdit(digits)}
                onFocus={() => p.onFocus()}
                onBlur={() => p.onBlur()}
                width={widthOf(p)}
                align={cellOf(p) ? "center" : undefined}
                error={p.box.message?.() || p.box.invalid()}
                maxDigits={p.field.m}
                zeroEmpty={countOnly()}
                placeholder={p.placeholder}
                inputRef={(ref) => giveFocus(p, ref)}
              />
            </If>
          }
        >
          <PercentInput
            testID={p.testID}
            accessibilityLabel={nameOf(p)}
            value={p.box.value()}
            onChange={(text) => p.onEdit(text)}
            onFocus={() => p.onFocus()}
            onBlur={() => p.onBlur()}
            width={widthOf(p)}
            error={p.box.message?.() || p.box.invalid()}
            placeholder={p.placeholder}
            inputRef={(ref) => giveFocus(p, ref)}
          />
        </If>
      }
    >
      <MoneyInput
        testID={p.testID}
        accessibilityLabel={nameOf(p)}
        value={moneyPlain(p.box.value())}
        onChange={(plain) => p.onEdit(plain)}
        onFocus={() => p.onFocus()}
        onBlur={() => p.onBlur()}
        amountStored={p.amountStored}
        onLeftUnreadable={(typed, kept) => p.onLeftUnreadable?.(typed, kept)}
        onCommaRead={(typed, shown) => p.onCommaRead?.(typed, shown)}
        hint={p.box.note?.() || undefined}
        onOutsideKept={(typed, shown) => p.onOutsideKept?.(typed, shown)}
        leaveRef={(leave) => p.leaveRef?.(leave)}
        width={widthOf(p)}
        error={p.box.message?.() || p.box.invalid()}
        whole={!!p.field.w}
        prefix={kind() === "num" ? "" : undefined}
        negative
        placeholder={p.placeholder}
        trailing={p.trailing}
        inputRef={(ref) => {
          giveFocus(p, ref);
          if (probing() && ref?.measure) registerMeasure(`input:${p.testID}`, () => ref.measure());
        }}
      />
    </If>
  );
}

/**
 * The control of one box, by its kind. Locked, computed and disabled boxes are read-only boxes; a box another answer turns
 * off is drawn faded. Wide: the box has the width the page gives. Narrow: the whole room, with the kit's touch height.
 */
export function FieldControl(props: FieldInput) {
  cleanup(() => {
    props.blurRef?.(null);
    props.leaveRef?.(null);
  });
  return (
    <View class="min-w-0 gap-1" style={{ opacity: props.box.off() ? OFF_OPACITY : 1 }}>
      <If
        when={props.field.t === "chk"}
        else={
          <If
            when={shut(props)}
            else={
              <If when={props.field.t === "sel"} else={<If when={props.field.t === "date"} else={<TypedBox p={props} />}><DateBox p={props} /></If>}>
                <Choice p={props} />
              </If>
            }
          >
            <ReadBox p={props} />
          </If>
        }
      >
        <TickBox p={props} />
      </If>
    </View>
  );
}
