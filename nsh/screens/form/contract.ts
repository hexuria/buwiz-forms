// The contract between the form page (B1: screens/form/*) and the field controls (B2: screens/form/controls/*).
// Keep it small and stable: B2 builds against exactly these types; changes go through B1.
//
// CONTROLS ARE DUMB. A control never calls the door (nsh/bir), never reads the session, never imports anything
// from screens/form except this file. It draws what its props say, reads the accessors INSIDE JSX or closures (the
// reactive rule: never copy a read into a body const), and reports the user's acts through the callbacks. Everything
// else (staging, the 300 ms flush, validation, autosave, which boxes are locked, radio group rules, date parsing,
// "touched" errors) is the session's job. One box = its own signals, so an edit re-runs only the rows it changed.
//
// Shared helpers for controls (masks, date text, money text) live in nsh/ui (applyMask, formatMoney, shownDate,
// parseTypedDate, formatMDY, DATE_ERROR ...): import them from "../../../ui", never re-implement them here.
import type { Accessor, JSX } from "nsh";

// ---------------------------------------------------------------- what controls/index.tsx exports (B2)
//
// The page imports exactly these three from "./controls" (B2 owns that folder):
//   export function FieldControl(props: FieldControlProps): JSX.Element;      // text money pct int email phone date sel chk
//   export function RadioGroupControl(props: RadioGroupProps): JSX.Element;   // an item's radios
//   export function ScheduleControl(props: ScheduleProps): JSX.Element;       // a schedule table, rows in a drawer/sheet
// Testids: a box "fp-<key>", a radio "fp-<key>", a schedule "fp-table-<first key>", its row "fp-row-<first key>-<n>".
/** The shape controls/index.tsx exports (type check only). */
export interface ControlsModule {
  FieldControl(props: FieldControlProps): JSX.Element;
  RadioGroupControl(props: RadioGroupProps): JSX.Element;
  ScheduleControl(props: ScheduleProps): JSX.Element;
}

// ---------------------------------------------------------------- the schema (forms/forms-schema.json, via bir.screen_form_schema)

/** A box's kind. "radio" and "chk" are tick boxes; every other kind takes typing or a pick. */
export type FieldType = "text" | "money" | "num" | "radio" | "chk" | "date" | "sel" | "pct" | "int" | "email" | "phone";

/** One box of an item. Flags are 1 when set (the schema writes 1, not true). */
export interface SchemaField {
  /** The rules-corpus key the value is stored under ("frm2551Qv2018:txtTIN1", "txtATCAmt1"). */
  k: string;
  /** The box's own label ("TIN1", "Fiscal start month: 1"). */
  l: string;
  t: FieldType;
  /** Computed by the form: never typed. */
  c?: 1;
  /** Required. */
  r?: 1;
  /** Max length of the typed text. */
  m?: number;
  /** Listed choices of a "sel" ("03 - March": code, then label). The host's options (BoxState.options) win. */
  o?: string[];
  /** Filled from the tax profile by the extractor's guess (the host's ScreenFormOpened.profile is the truth). */
  a?: 1;
  /** A peso box over a whole-number field: no centavos. */
  w?: 1;
  /** A radio's one-answer group (host). */
  grp?: string;
  /** How many boxes the radio's group has (host). */
  rg?: number;
  /** A pooled address line: drawn once by the pool's first line. */
  pool?: { keys: string[] };
}

/** A column of a schedule table. "hidden" columns are not drawn. */
export interface SchemaColumn {
  l: string;
  t: FieldType | "hidden";
  c?: 1;
  w?: 1;
  o?: string[];
}

/** One numbered item (row) of a part: its boxes (f), or a schedule table (rows, cols, keys[row][col]). */
export interface SchemaItem {
  /** The printed item number ("13", "20A", ""). */
  no: string;
  l: string;
  /** The page of the paper form it prints on. */
  p: number;
  f?: SchemaField[];
  rows?: number;
  cols?: SchemaColumn[];
  keys?: string[][];
  /** A printed item with no box: what the paper prints there. */
  note?: string;
  /** Month, day and year boxes shown as one date box. */
  one_date?: boolean | 1;
  /** Registered month/day/year keys, in screen order. */
  one_dates?: [string, string, string][];
  /** Registered month/year keys, in screen order: the pair is one MM/YYYY field ("<month key>#period"). */
  one_periods?: [string, string][];
  /** The year keys of one_periods pairs whose box stores two digits (20YY, a two-digit select). */
  one_period_two_digit?: string[];
  /** Registered from/to anchors: a whole-date box's key, or a one-date's month key (its triple is the end). */
  date_ranges?: [string, string][];
  /** To anchors copied from From by the form, not independent range ends. */
  derived_to?: string[];
  /** A hint offering another item's amount for this item's box ("Use it"). */
  from?: { k: string; to: string; l: string };
  /** A schedule's preprinted row captions. */
  row_labels?: { caption: string; rows: string[] };
  numbered?: boolean;
  start?: number;
  calendar?: boolean | 1;
  typed_tin?: boolean | 1;
}

export interface SchemaPart {
  /** "Part I · Background Information", or "Page 1" for the untitled first part. */
  n: string;
  items: SchemaItem[];
}

/** A form as the page draws it. */
export interface FormSchema {
  /** Form code ("2551Q"). */
  c: string;
  id?: string;
  /** Title. */
  t: string;
  rev: string;
  freq: string;
  pages: number;
  /** Number of fields. */
  n: number;
  /** Layout quality 0..1. Kept in the data; no screen shows it. */
  q: number;
  parts: SchemaPart[];
}

// ---------------------------------------------------------------- what a control gets

/** A pick in a list: the value the host takes, the label shown. */
export interface ChoiceOption {
  value: string;
  label: string;
  selected?: boolean;
  /** The choice takes no pick (a locked box); it still shows, dimmed and unpressable. */
  disabled?: boolean;
}

/** One box's live state. Every accessor is its own signal: reading one subscribes to that box only. */
export interface BoxState {
  /** The text the box shows, ready to draw: an amount grouped ("10,000.00"), a date long ("Dec 1, 2020"), a tick "true",
   *  a choice its code ("03"). While the user types it is what they typed; the session reformats it on blur. */
  value: Accessor<string>;
  /** A note under the box that is nothing to fix (a comma read as the decimal point), drawn in the note style; "" for none. */
  note?: Accessor<string>;
  /** The box takes no typing: computed, locked by the host, a profile box not opened with Edit, read-only return, a
   *  penalty not opened, or turned off by another answer. A locked box draws as a read-only box (no Input). */
  locked: Accessor<boolean>;
  /** What a locked BLANK box shows in its place, in the filer's words ("Needs the ATC", "Set by the period"); "" none. */
  hint: Accessor<string>;
  /** Another answer turns this box off: draw it faded (opacity ~0.45). The tag ("Needs item 13 = Yes") is drawn by the
   *  page under the label, not by the control. "" when on. G1: the page does not render a blank gated box at all —
   *  this accessor matters only for a shut box that still draws (a kept value, a message, a to-do stop). */
  off: Accessor<string>;
  /** A "sel" box's choices (already "Choose…"-free; the control adds its own empty choice). Empty for other kinds. */
  options: Accessor<ChoiceOption[]>;
  /** The box holds an error the user should see now (touched-only rule applied): a red border. */
  invalid: Accessor<boolean>;
  /** The box's own sentence (a refused edit or a one-box validation error), "" none (B3/B4). */
  message?: Accessor<string>;
  /** The refusal the box carries, wherever it is drawn: `message` is blanked for the kinds whose sentence
   *  prints at the row (sel, chk, radio, date), so anything that needs to KNOW about the refusal reads this. */
  refusal?: Accessor<string>;
  /** A computed or penalty amount: a locked blank money box shows "0.00", never an empty box. */
  zeroWhenBlank: boolean;
}

/** Props of one box's control (text, money, pct, int, email, phone, date, sel, chk). Radio groups: RadioGroupProps. */
export interface FieldControlProps {
  field: SchemaField;
  /** The accessible label (the field's label, or "<column>, row <n>" in a schedule). */
  label: string;
  /** The testID the control's Input/Select/Checkbox carries: "fp-<k>" (tests select "id:fp-<k>"). */
  testID: string;
  box: BoxState;
  /** Pixel width the page gives (money 230, pct 88, date 140, short text 88, sel 260, text 320), capped to the room. */
  width: number;
  /** Phone layout: full-width, at least 44 px tall targets. */
  narrow: Accessor<boolean>;
  /** Typing or a pick. Text-like controls call it per keystroke with the text as shown (the session stages it and
   *  flushes 300 ms after the last edit); sel and chk call it once per pick ("true"/"false" for chk). A date control
   *  passes the text as typed: the session turns "12012020" into the draft's MM/DD/YYYY. */
  onEdit(text: string): void;
  /** The box gained focus (the session tracks the focused box for the profile Edit look). */
  onFocus(): void;
  /** The box lost focus: the session marks it touched (its error may show now) and flushes. */
  onBlur(): void;
  /** Register the focusable view so a pill step can put the keyboard in this box. Call with the Input's ref. */
  focusRef?(focus: () => void): void;
  /** Register how to take the keyboard out of this box (Queue leaves a focused amount box first), with the Input's ref; null when the box goes away. */
  blurRef?(blur: (() => void) | null): void;
  /** Register how an amount box leaves at once, in this turn, without writing to its native box (quit and the page going leave it first); null when the box goes away. */
  leaveRef?(leave: (() => void) | null): void;
  /** Whether a zero amount in this box is stored content (a schedule row that exists), rather than a host zero shown for a row nothing stores yet. */
  amountStored?(): boolean;
  /** An amount box was left with text that is not an amount (`typed`) and shows `kept`, the return's amount, again. The session says so under it. */
  onLeftUnreadable?(typed: string, kept: string): void;
  /** An amount box was left with one comma read as its decimal point (`typed` "1500,50", `shown` "1,500.50"). The session says so under it. */
  onCommaRead?(typed: string, shown: string): void;
  /** The filer typed in an amount box but came back to the amount it showed, and a value set from outside meanwhile won on leaving: it shows `shown`. The session says so under it. */
  onOutsideKept?(typed: string, shown: string): void;
}

/** One answer of a radio group. */
export interface RadioChoice {
  /** The radio's key ("frm2551Qv2018:taxRate1"); its value is "true" when ticked. */
  key: string;
  label: string;
  testID: string;
}

/** A one-answer group drawn as the paper form's tick boxes. */
export interface RadioGroupProps {
  /** The item's label, for the group's accessibility label. */
  label: string;
  testID: string;
  choices: RadioChoice[];
  /** The ticked radio's key, or null. Its own signal. */
  selected: Accessor<string | null>;
  /** Whether this radio is ticked (reactive). An item can draw radios of two groups (1701MS item 11: the taxpayer's and
   * the spouse's), so each choice reads its own; without it, a choice is ticked when it is selected(). */
  checked?(key: string): boolean;
  /** The whole group takes no pick. */
  locked: Accessor<boolean>;
  /** One choice is locked (others may not be). */
  choiceLocked(key: string): boolean;
  hint: Accessor<string>;
  off: Accessor<string>;
  narrow: Accessor<boolean>;
  /** The width the group may use, in points, for the one-line fit of a segmented bar (a stacked bar when the labels
   *  cannot hold it). Unset: the control guesses from `narrow`. */
  room?: Accessor<number>;
  /** The group may be cleared back to unanswered (optional and the session unticks it): a segmented bar of three or
   *  more shows a Clear, a dropdown a "None" entry, a search list its clear row. */
  clearable?: Accessor<boolean>;
  /** A pick. The session applies the group rules (untick in groups of 3+, pairs keep one answer, grp siblings). */
  onPick(key: string): void;
  focusRef?(focus: () => void): void;
}

/** One cell of a schedule row (a FieldControlProps for its column; a "radio"/"chk" column uses onEdit "true"/"false"). */
export type ScheduleCellProps = FieldControlProps;

/** A schedule table (item.rows): rows are listed compactly; a row opens in a drawer (wide) or sheet (narrow). */
export interface ScheduleProps {
  item: SchemaItem;
  testID: string;
  narrow: Accessor<boolean>;
  /** The drawn columns (hidden ones left out), with their index in item.cols. */
  columns: { index: number; col: SchemaColumn }[];
  /** How a row is named: its number, or its preprinted caption. */
  rowName(row: number): string;
  /** A row's one-line summary for the list ("RENT · 12,000.00"), "" when the row is empty. Its own signal per row. */
  summary(row: number): Accessor<string>;
  /** Rows the list shows: every row holding something, plus the first empty one ("Add row" opens the next empty one). */
  shownRows: Accessor<number[]>;
  /** The cell control's props for row r, column index c (index into item.cols). */
  cell(row: number, col: number): ScheduleCellProps;
  /** The row open in the drawer, or null. The page owns it; the control renders the drawer from it. */
  openRow: Accessor<number | null>;
  onOpenRow(row: number | null): void;
  /** Remove the row at once; rows below move up intact and Undo restores it. */
  onDeleteRow(row: number): void;
  /** The whole table takes no typing (read-only return). */
  locked: Accessor<boolean>;
}
