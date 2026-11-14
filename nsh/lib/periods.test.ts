import { describe, expect, test } from "nsh/test";
import { frequencyOf, periodLabel, periodOptions } from "./periods";
import type { FormIndexEntry } from "./periods";

const INDEX: FormIndexEntry[] = [
  { c: "1601C", freq: "Monthly" },
  { c: "2550Q", freq: "Quarterly" },
  { c: "1701", freq: "Annual" },
  { c: "1800", freq: "As needed" },
];

// The clock for these tests: mid-March 2026.
const MARCH_2026 = new Date(2026, 2, 15, 10, 0);

describe("frequencyOf", () => {
  test("a listed form has the frequency it is listed with", () => {
    expect(frequencyOf(INDEX, "1601C")).toBe("Monthly");
    expect(frequencyOf(INDEX, "2550Q")).toBe("Quarterly");
    expect(frequencyOf(INDEX, "1800")).toBe("As needed");
  });
  test("a form the index does not list is Annual", () => {
    expect(frequencyOf(INDEX, "9999")).toBe("Annual");
    expect(frequencyOf([], "1601C")).toBe("Annual");
  });
});

describe("periodOptions", () => {
  test("monthly: the months up to the current one, newest first", () => {
    expect(periodOptions("1601C", "Monthly", 2026, MARCH_2026)).toEqual([
      { value: 3, label: "March 2026" },
      { value: 2, label: "February 2026" },
      { value: 1, label: "January 2026" },
    ]);
  });
  test("monthly: a past year offers all twelve, December first", () => {
    const options = periodOptions("1601C", "Monthly", 2025, MARCH_2026);
    expect(options.length).toBe(12);
    expect(options[0]).toEqual({ value: 12, label: "December 2025" });
    expect(options[11]).toEqual({ value: 1, label: "January 2025" });
  });
  test("monthly: a later year offers only January", () => {
    expect(periodOptions("1601C", "Monthly", 2027, MARCH_2026)).toEqual([{ value: 1, label: "January 2027" }]);
  });
  test("monthly: on the last day of the year the current year offers all twelve (year rollover)", () => {
    const options = periodOptions("1601C", "Monthly", 2026, new Date(2026, 11, 31, 9, 0));
    expect(options.length).toBe(12);
    expect(options[0]).toEqual({ value: 12, label: "December 2026" });
  });
  test("quarterly: the quarter a month falls in is the last one offered (quarter boundary)", () => {
    expect(periodOptions("2550Q", "Quarterly", 2026, MARCH_2026)).toEqual([{ value: 1, label: "Q1 2026" }]);
    expect(periodOptions("2550Q", "Quarterly", 2026, new Date(2026, 5, 30))).toEqual([
      { value: 2, label: "Q2 2026" },
      { value: 1, label: "Q1 2026" },
    ]);
    expect(periodOptions("2550Q", "Quarterly", 2026, new Date(2026, 6, 1))).toEqual([
      { value: 3, label: "Q3 2026" },
      { value: 2, label: "Q2 2026" },
      { value: 1, label: "Q1 2026" },
    ]);
  });
  test("quarterly: a past year offers four quarters, Q4 first; a later year only Q1", () => {
    expect(periodOptions("2550Q", "Quarterly", 2025, MARCH_2026).map((o) => o.label)).toEqual([
      "Q4 2025",
      "Q3 2025",
      "Q2 2025",
      "Q1 2025",
    ]);
    expect(periodOptions("2550Q", "Quarterly", 2027, MARCH_2026)).toEqual([{ value: 1, label: "Q1 2027" }]);
  });
  test("as needed: one open slot; annual: TY (period 0)", () => {
    expect(periodOptions("1800", "As needed", 2026, MARCH_2026)).toEqual([{ value: 1, label: "2026, first return" }]);
    expect(periodOptions("1701", "Annual", 2026, MARCH_2026)).toEqual([{ value: 0, label: "TY 2026" }]);
  });
});

describe("periodLabel", () => {
  test("a monthly period names its month, short or long", () => {
    expect(periodLabel("Monthly", 2026, 9, true)).toBe("Sep 2026");
    expect(periodLabel("Monthly", 2026, 9)).toBe("September 2026");
  });
  test("the first and last months of a monthly form; anything outside 1 to 12 is TY", () => {
    expect(periodLabel("Monthly", 2026, 1)).toBe("January 2026");
    expect(periodLabel("Monthly", 2026, 12, true)).toBe("Dec 2026");
    expect(periodLabel("Monthly", 2026, 0)).toBe("TY 2026");
    expect(periodLabel("Monthly", 2026, 13)).toBe("TY 2026");
  });
  test("quarterly periods 1 to 4; anything outside is TY", () => {
    expect(periodLabel("Quarterly", 2026, 1)).toBe("Q1 2026");
    expect(periodLabel("Quarterly", 2026, 4)).toBe("Q4 2026");
    expect(periodLabel("Quarterly", 2026, 0)).toBe("TY 2026");
    expect(periodLabel("Quarterly", 2026, 5)).toBe("TY 2026");
  });
  test("as needed: the slot number, and slot 0 shows as 1 (the old period || 1)", () => {
    expect(periodLabel("As needed", 2026, 1)).toBe("2026 #1");
    expect(periodLabel("As needed", 2026, 3)).toBe("2026 #3");
    expect(periodLabel("As needed", 2026, 0)).toBe("2026 #1");
  });
  test("annual returns are TY whatever the period", () => {
    expect(periodLabel("Annual", 2026, 0)).toBe("TY 2026");
    expect(periodLabel("Annual", 2026, 5)).toBe("TY 2026");
  });
});
