// What the controls take beyond the contract (../contract.ts, B1's file, which this folder never edits): optional props, so the
// page's plain FieldControlProps / RadioGroupProps / ScheduleProps still fit. The gaps are listed in B2's report.
import type { Accessor } from "nsh";
import type { FieldControlProps, RadioGroupProps, ScheduleProps } from "../contract";

/** A value, or an accessor of it (the page may pass either). */
export type Maybe<T> = T | Accessor<T>;

/** The value of a Maybe; `fallback` when none was given. */
export function read<T>(value: Maybe<T> | undefined, fallback: T): T {
  if (value === undefined) return fallback;
  return typeof value === "function" ? (value as Accessor<T>)() : value;
}

/** Extras of one box, all optional. */
export interface FieldExtras {
  /** Why the box is locked ("From tax profile"): a tooltip on its lock and part of its accessible label. A locked auto-filled box (field.a) says "From tax profile" by itself. */
  reason?: Maybe<string>;
  /** The box takes nothing now and is drawn dimmed, with no lock (a section the page has switched off). */
  disabled?: Maybe<boolean>;
  /** Text shown in an empty box. */
  placeholder?: string;
  /** A cell of the phone TIN grid (PR 8): centred tabular digits, and a locked box shows no lock of its own — the row's "From the tax profile" line says it once. Only the TIN row's narrow layout passes it. */
  tinCell?: boolean;
}

/** What FieldControl takes: the contract's props and the extras. */
export type FieldInput = FieldControlProps & FieldExtras;

/** What RadioGroupControl takes. */
export type RadioInput = RadioGroupProps & {
  reason?: Maybe<string>;
  /** Choice keys another answer keeps shut (G1): those choices are not drawn. */
  hidden?: Maybe<readonly string[]>;
};

/** What ScheduleControl takes. */
export type ScheduleInput = ScheduleProps & {
  /** The drawer's title for a row ("Row 3"); default "Row <number>" (plus the row's caption). */
  rowTitle?: (row: number) => string;
};
