// The group "Filed in BIR eBIRForms", without a window: how a period of the old program is named and ordered, which lines the
// returns make, and what an amendment and an e-mailed copy say. Rows are generated (the demo taxpayer's TIN, a Cebu branch).
import { expect, test } from "nsh/test";
import type { LegacyFiledReturn } from "../../bir";
import {
  amendedText, catalogFrequency, importedText, LEGACY_TITLE, legacyLines, legacyPeriodLabel, legacyRank, parseLegacyPeriod,
} from "./legacy";

const CATALOG = [
  { code: "1601C", frequency: "Monthly" },
  { code: "2551Q", frequency: "Quarterly" },
  { code: "1701", frequency: "Annual" },
  { code: "0605", frequency: "OpenEnded" },
];

function row(over: Partial<LegacyFiledReturn>): LegacyFiledReturn {
  return {
    id: 1, form: "1601Cv2018", form_code: "1601C", period: "062025", amendment_version: null, email_copy: false,
    source_file: "demo.xml", imported_at: "2026-11-14 12:00:00", ...over,
  };
}

test("the title the group carries", () => {
  expect(LEGACY_TITLE).toBe("Filed in BIR eBIRForms");
});

test("a period of the old program is read as its file names write it", () => {
  expect(parseLegacyPeriod("062025")).toEqual({ year: 2025, month: 6, quarter: null, day: null });
  expect(parseLegacyPeriod("2026Q1")).toEqual({ year: 2026, month: null, quarter: 1, day: null });
  expect(parseLegacyPeriod("2026q4")).toEqual({ year: 2026, month: null, quarter: 4, day: null });
  expect(parseLegacyPeriod("122026Q1")).toEqual({ year: 2026, month: 12, quarter: 1, day: null });
  expect(parseLegacyPeriod("2025")).toEqual({ year: 2025, month: null, quarter: null, day: null });
  expect(parseLegacyPeriod("03152026")).toEqual({ year: 2026, month: 3, quarter: null, day: 15 });
  // Not a period: a month that is none, a quarter that is none, a day the month does not have, other shapes.
  for (const bad of ["132025", "002025", "2026Q5", "2026Q0", "02302026", "13012026", "", "Q1", "20255", "2025A", "06 2025", "0620255"]) {
    expect(parseLegacyPeriod(bad)).toBeNull();
  }
  // February 29 only in a leap year.
  expect(parseLegacyPeriod("02292024")?.day).toBe(29);
  expect(parseLegacyPeriod("02292025")).toBeNull();
});

test("a period is written as the app writes periods elsewhere, as the form's frequency reads it", () => {
  // Monthly: "Jun 2025"; the form's quarter and year name themselves.
  expect(legacyPeriodLabel("062025", "Monthly")).toBe("Jun 2025");
  expect(legacyPeriodLabel("122025", "Monthly")).toBe("Dec 2025");
  // A form the catalog does not list (frequency unknown) reads a month and year as a month.
  expect(legacyPeriodLabel("062025", null)).toBe("Jun 2025");
  // Quarterly: a month falls in its quarter (the counts' reading), a named quarter is itself.
  expect(legacyPeriodLabel("032025", "Quarterly")).toBe("Q1 2025");
  expect(legacyPeriodLabel("062025", "Quarterly")).toBe("Q2 2025");
  expect(legacyPeriodLabel("072025", "Quarterly")).toBe("Q3 2025");
  expect(legacyPeriodLabel("122025", "Quarterly")).toBe("Q4 2025");
  expect(legacyPeriodLabel("2026Q1", "Quarterly")).toBe("Q1 2026");
  expect(legacyPeriodLabel("122026Q3", "Quarterly")).toBe("Q3 2026");
  expect(legacyPeriodLabel("2026Q2", null)).toBe("Q2 2026");
  // Annual: the year, whatever month a fiscal year ends in.
  expect(legacyPeriodLabel("2024", "Annual")).toBe("TY 2024");
  expect(legacyPeriodLabel("062025", "Annual")).toBe("TY 2025");
  expect(legacyPeriodLabel("2024", "Monthly")).toBe("TY 2024");
  expect(legacyPeriodLabel("2024", null)).toBe("TY 2024");
  // A date is a date.
  expect(legacyPeriodLabel("03152026", "As needed")).toBe("Mar 15, 2026");
  expect(legacyPeriodLabel("03152026", "Monthly")).toBe("Mar 15, 2026");
  // As needed with a month and year.
  expect(legacyPeriodLabel("062025", "As needed")).toBe("Jun 2025");
  // What is no period is shown as it was written.
  expect(legacyPeriodLabel("2025A", "Monthly")).toBe("2025A");
  expect(legacyPeriodLabel("", "Monthly")).toBe("");
});

test("a form's frequency is the host catalog's, and as needed is the host's OpenEnded", () => {
  expect(catalogFrequency(CATALOG, "1601C")).toBe("Monthly");
  expect(catalogFrequency(CATALOG, "2551Q")).toBe("Quarterly");
  expect(catalogFrequency(CATALOG, "1701")).toBe("Annual");
  expect(catalogFrequency(CATALOG, "0605")).toBe("As needed");
  expect(catalogFrequency(CATALOG, "9999X")).toBeNull();
  expect(catalogFrequency(CATALOG, null)).toBeNull();
  expect(catalogFrequency([], "1601C")).toBeNull();
});

test("periods rank in time: newer years first, later months and quarters first, a whole year after December", () => {
  const order = ["2025", "122025", "2025Q4", "092025", "2025Q3", "062025", "2025Q2", "032025", "2025Q1", "012025", "2024", "112024"];
  const ranks = order.map(legacyRank);
  expect(ranks).toEqual([...ranks].sort((a, b) => b - a));
  expect(legacyRank("03152026")).toBeGreaterThan(legacyRank("03012026"));
  expect(legacyRank("03012026")).toBeGreaterThan(legacyRank("022026"));
  expect(legacyRank("nonsense")).toBe(0);
});

test("an amendment says its number and an original says nothing", () => {
  expect(amendedText(2)).toBe("Amended (v2)");
  expect(amendedText(1)).toBe("Amended (v1)");
  expect(amendedText(null)).toBe("");
});

test("the import date is the app's short date of the database's timestamp", () => {
  // Midday UTC lands on the same calendar day in every time zone.
  expect(importedText("2026-11-14 12:00:00")).toBe("Imported Nov 14, 2026");
});

test("the lines: newest period first, an amendment before its original, the form's code and the period as Filings names a return", () => {
  const lines = legacyLines(
    [
      { row: row({ id: 1, period: "062025" }), branch: "" },
      { row: row({ id: 2, period: "062025", amendment_version: 2 }), branch: "" },
      { row: row({ id: 3, period: "062025", email_copy: true }), branch: "" },
      { row: row({ id: 4, form: "2551Qv2018", form_code: "2551Q", period: "2026Q1", imported_at: "2026-11-13 12:00:00" }), branch: "" },
      { row: row({ id: 5, form: "1701v2018", form_code: "1701", period: "2024", imported_at: "2026-11-13 12:00:00" }), branch: "" },
      { row: row({ id: 6, form: "9999Xv2000", form_code: null, period: "2023" }), branch: "" },
      { row: row({ id: 7, period: "072025" }), branch: "Cebu branch" },
    ],
    CATALOG,
  );
  // Of June 2025 the amendment first, then the original and the copy, which came in together (the higher id first).
  expect(lines.map((l) => l.id)).toEqual([4, 7, 2, 3, 1, 5, 6]);
  expect(lines.map((l) => l.title)).toEqual([
    "2551Q Q1 2026",
    "1601C Jul 2025",
    "1601C Jun 2025",
    "1601C Jun 2025",
    "1601C Jun 2025",
    "1701 TY 2024",
    "9999Xv2000 TY 2023",
  ]);
  const byId = (id: number) => lines.find((l) => l.id === id)!;
  expect(byId(2).amended).toBe("Amended (v2)");
  expect(byId(1).amended).toBe("");
  expect(byId(3).emailCopy).toBe(true);
  expect(byId(1).emailCopy).toBe(false);
  expect(byId(7).branch).toBe("Cebu branch");
  expect(byId(1).branch).toBe("");
  expect(byId(1).imported).toBe("Imported Nov 14, 2026");
  expect(legacyLines([], CATALOG)).toEqual([]);
});

test("a quarterly form's month reads in its quarter, in the lines too", () => {
  const lines = legacyLines([{ row: row({ id: 1, form: "2551Qv2018", form_code: "2551Q", period: "062025" }), branch: "" }], CATALOG);
  expect(lines[0].title).toBe("2551Q Q2 2025");
});
