import { describe, expect, test } from "nsh/test";
import {
  DATE_ERROR,
  MONTHS,
  MONTHS_LONG,
  addDays,
  daysUntil,
  formatLongDate,
  formatMDY,
  isoDate,
  oneDateText,
  parseTypedDate,
  shortDate,
  shortIsoDate,
  shownDate,
} from "./dates";

/** The MM/DD/YYYY text of what parseTypedDate reads, or null. */
const mdy = (text: string) => {
  const parsed = parseTypedDate(text);
  return parsed ? formatMDY(parsed) : null;
};

describe("isoDate", () => {
  test("writes the local day as YYYY-MM-DD, padded", () => {
    expect(isoDate(new Date(2026, 0, 5))).toBe("2026-01-05");
    expect(isoDate(new Date(2026, 11, 31))).toBe("2026-12-31");
  });
  test("any hour of a local day is that day", () => {
    expect(isoDate(new Date(2026, 9, 14, 0, 0))).toBe("2026-10-14");
    expect(isoDate(new Date(2026, 9, 14, 23, 59))).toBe("2026-10-14");
  });
  test("a leap day is written", () => {
    expect(isoDate(new Date(2028, 1, 29))).toBe("2028-02-29");
  });
});

describe("addDays", () => {
  test("moves by whole days and leaves the date it was given alone", () => {
    const start = new Date(2026, 9, 14);
    expect(isoDate(addDays(start, 7))).toBe("2026-10-21");
    expect(isoDate(start)).toBe("2026-10-14");
  });
  test("a month and a year roll over", () => {
    expect(isoDate(addDays(new Date(2027, 0, 31), 1))).toBe("2027-02-01");
    expect(isoDate(addDays(new Date(2026, 11, 31), 1))).toBe("2027-01-01");
  });
  test("a leap day is found, and a common February ends on the 28th", () => {
    expect(isoDate(addDays(new Date(2028, 1, 28), 1))).toBe("2028-02-29");
    expect(isoDate(addDays(new Date(2027, 1, 28), 1))).toBe("2027-03-01");
  });
  test("a negative count goes back, across a year end", () => {
    expect(isoDate(addDays(new Date(2026, 0, 1), -1))).toBe("2025-12-31");
    expect(isoDate(addDays(new Date(2026, 0, 1), -366))).toBe("2024-12-31");
  });
  test("zero is the same day", () => {
    expect(isoDate(addDays(new Date(2026, 4, 9), 0))).toBe("2026-05-09");
  });
});

describe("daysUntil", () => {
  // Afternoon of October 14, 2026 (a Wednesday).
  const now = new Date(2026, 9, 14, 15, 30);
  test("counts whole days from today to a date", () => {
    expect(daysUntil("2026-10-14", now)).toBe(0);
    expect(daysUntil("2026-10-15", now)).toBe(1);
    expect(daysUntil("2026-10-21", now)).toBe(7);
  });
  test("a past date counts negative", () => {
    expect(daysUntil("2026-10-13", now)).toBe(-1);
    expect(daysUntil("2026-09-14", now)).toBe(-30);
  });
  test("the hour on the clock does not matter: today starts at local midnight", () => {
    expect(daysUntil("2026-10-15", new Date(2026, 9, 14, 0, 0))).toBe(1);
    expect(daysUntil("2026-10-15", new Date(2026, 9, 14, 23, 59))).toBe(1);
  });
  test("a year rolls over", () => {
    expect(daysUntil("2027-01-01", new Date(2026, 11, 31, 9, 0))).toBe(1);
  });
  test("a leap February is counted", () => {
    expect(daysUntil("2028-03-01", new Date(2028, 1, 28, 9, 0))).toBe(2);
  });
});

describe("shortDate", () => {
  test("an ISO date reads Mon D, YYYY with no zero before the day", () => {
    expect(shortDate("2026-10-09")).toBe("Oct 9, 2026");
    expect(shortDate("2026-12-31")).toBe("Dec 31, 2026");
  });
  test("a SQLite timestamp shows its local day (it is stored in UTC)", () => {
    // Midday UTC is the same calendar day in every zone from UTC-11 to UTC+11.
    expect(shortDate("2026-10-14 12:00:00")).toBe("Oct 14, 2026");
    expect(shortDate("2026-10-14T12:00:00")).toBe("Oct 14, 2026");
  });
  test("a date with trailing words takes its date part", () => {
    expect(shortDate("2026-10-14 ")).toBe("Oct 14, 2026");
  });
  test("empty input is empty", () => {
    expect(shortDate("")).toBe("");
    expect(shortDate(null)).toBe("");
    expect(shortDate(undefined)).toBe("");
  });
  test("text that is not a date is shown as it is", () => {
    expect(shortDate("not a date")).toBe("not a date");
    expect(shortDate("2026-10")).toBe("2026-10");
  });
  test("old behaviour kept: a month past 12 names no month", () => {
    expect(shortDate("2026-13-01")).toBe("undefined 1, 2026");
  });
});

describe("shortIsoDate (form_page.js shortDate)", () => {
  test("a timestamp that starts with an ISO date reads from that date", () => {
    expect(shortIsoDate("2026-10-01T08:30:00")).toBe("Oct 1, 2026");
    expect(shortIsoDate("2026-10-01")).toBe("Oct 1, 2026");
  });
  test("anything else is shown as given, and empty is empty", () => {
    expect(shortIsoDate("")).toBe("");
    expect(shortIsoDate(null)).toBe("");
    expect(shortIsoDate("1 Oct 2026")).toBe("1 Oct 2026");
    expect(shortIsoDate("2026-1-5")).toBe("2026-1-5");
  });
});

describe("month names", () => {
  test("twelve short names and twelve long ones, January first", () => {
    expect(MONTHS.length).toBe(12);
    expect(MONTHS[0]).toBe("Jan");
    expect(MONTHS[11]).toBe("Dec");
    expect(MONTHS_LONG.length).toBe(12);
    expect(MONTHS_LONG[8]).toBe("September");
  });
});

describe("DATE_ERROR", () => {
  test("is the message a date box shows for text that is not a date", () => {
    expect(DATE_ERROR).toBe("Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)");
  });
});

describe("parseTypedDate", () => {
  test("reads each form the filer types as the same day (from test_date_field_rows)", () => {
    for (const text of ["12012020", "12/01/2020", "12-01-2020", "2020-12-01", "Dec 01 2020", "12/1/2020"]) {
      expect(mdy(text)).toBe("12/01/2020");
    }
  });
  test("reads a month name with or without a dot, a comma, or the full name", () => {
    expect(parseTypedDate("Dec 1, 2020")).toEqual({ y: 2020, m: 12, d: 1 });
    expect(parseTypedDate("Sept. 5 2020")).toEqual({ y: 2020, m: 9, d: 5 });
    expect(parseTypedDate("december 10, 2020")).toEqual({ y: 2020, m: 12, d: 10 });
  });
  test("reads single digits and surrounding spaces", () => {
    expect(parseTypedDate("2/9/2021")).toEqual({ y: 2021, m: 2, d: 9 });
    expect(parseTypedDate("  12/01/2020 ")).toEqual({ y: 2020, m: 12, d: 1 });
  });
  test("the last day of each month is the most a month has", () => {
    expect(parseTypedDate("04/30/2021")).toEqual({ y: 2021, m: 4, d: 30 });
    expect(parseTypedDate("04/31/2021")).toBeNull();
    expect(parseTypedDate("12/31/2021")).toEqual({ y: 2021, m: 12, d: 31 });
  });
  test("February 29 exists in leap years only; 1900 is not one", () => {
    expect(parseTypedDate("02/28/2021")).toEqual({ y: 2021, m: 2, d: 28 });
    expect(parseTypedDate("02/29/2021")).toBeNull();
    expect(parseTypedDate("02/29/2020")).toEqual({ y: 2020, m: 2, d: 29 });
    expect(parseTypedDate("02/29/2000")).toEqual({ y: 2000, m: 2, d: 29 });
    expect(parseTypedDate("02/29/1900")).toBeNull();
  });
  test("invalid days and months are not dates (from test_date_field_rows)", () => {
    for (const text of ["13012020", "02302020", "1201202", "abc", "00/00/0000"]) {
      expect(parseTypedDate(text)).toBeNull();
    }
  });
  test("out-of-range fields and odd shapes are not dates", () => {
    expect(parseTypedDate("13/01/2020")).toBeNull();
    expect(parseTypedDate("00/10/2020")).toBeNull();
    expect(parseTypedDate("01/00/2020")).toBeNull();
    expect(parseTypedDate("12/2020")).toBeNull();
    expect(parseTypedDate("12/01/20")).toBeNull();
    expect(parseTypedDate("1899-12-31")).toBeNull();
    expect(parseTypedDate("2101-01-01")).toBeNull();
    expect(parseTypedDate("Foo 01 2020")).toBeNull();
  });
  test("empty input is no date", () => {
    expect(parseTypedDate("")).toBeNull();
    expect(parseTypedDate("   ")).toBeNull();
    expect(parseTypedDate(null)).toBeNull();
    expect(parseTypedDate(undefined)).toBeNull();
  });
  test("old behaviour kept: eight digits are always month-day-year, and a year first with slashes is not read", () => {
    expect(parseTypedDate("20201201")).toBeNull();
    expect(parseTypedDate("2020/12/01")).toBeNull();
  });
});

describe("formatMDY", () => {
  test("MM/DD/YYYY, with the month and day padded", () => {
    expect(formatMDY({ y: 2020, m: 2, d: 9 })).toBe("02/09/2020");
    expect(formatMDY({ y: 1999, m: 12, d: 31 })).toBe("12/31/1999");
  });
});

describe("formatLongDate", () => {
  test("Mon D, YYYY with no zero before the day", () => {
    expect(formatLongDate({ y: 2020, m: 12, d: 10 })).toBe("Dec 10, 2020");
    expect(formatLongDate({ y: 2020, m: 12, d: 1 })).toBe("Dec 1, 2020");
    expect(formatLongDate({ y: 2026, m: 1, d: 1 })).toBe("Jan 1, 2026");
  });
});

describe("shownDate", () => {
  test("a date reads Mon D, YYYY whatever form it was typed in", () => {
    expect(shownDate("12/10/2020")).toBe("Dec 10, 2020");
    expect(shownDate("2020-12-10")).toBe("Dec 10, 2020");
    expect(shownDate("3/31/2026")).toBe("Mar 31, 2026");
  });
  test("empty is empty", () => {
    expect(shownDate("")).toBe("");
    expect(shownDate(null)).toBe("");
    expect(shownDate(undefined)).toBe("");
  });
  test("what is not a whole date shows as it is", () => {
    expect(shownDate("12/2020")).toBe("12/2020");
    expect(shownDate("13/10/2020")).toBe("13/10/2020");
    expect(shownDate("   ")).toBe("   ");
  });
});

describe("oneDateText", () => {
  const boxes = { month: { k: "m" }, day: { k: "d" }, year: { k: "y" } };
  test("three boxes read as MM/DD/YYYY (from test_form_q3_income_tax)", () => {
    expect(oneDateText({ m: "05", d: "17", y: "1980" }, boxes)).toBe("05/17/1980");
  });
  test("three blank boxes are no date", () => {
    expect(oneDateText({}, boxes)).toBe("");
    expect(oneDateText({ m: "  ", d: "", y: "" }, boxes)).toBe("");
  });
  test("a box not typed yet keeps its slash", () => {
    expect(oneDateText({ m: "05" }, boxes)).toBe("05//");
  });
  test("values are trimmed; numbers are written without a zero before them", () => {
    expect(oneDateText({ m: " 05 ", d: "17", y: "1980" }, boxes)).toBe("05/17/1980");
    expect(oneDateText({ m: 5, d: 17, y: 1980 }, boxes)).toBe("5/17/1980");
  });
  test("a form with no day box leaves the middle part empty", () => {
    expect(oneDateText({ m: "05", y: "1980" }, { month: { k: "m" }, day: null, year: { k: "y" } })).toBe("05//1980");
  });
});
