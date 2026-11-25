// PR 3 schema coverage: every registered one_periods pair and date_ranges pair must appear in the built
// schema with both ends on one item, the year box's own kind saying how it stores the year, and the host's
// two_digit mark naming the 20YY/select years (never a guess from labels).
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer } from "../../bir/testing";
import type { FormSchema, SchemaItem } from "./contract";
import { dateRanges, onePeriods, withoutHidden } from "./schema";

const PAIRS: { code: string; month: string; year: string; twoDigit: boolean }[] = [
  { code: "0605", month: "frm0605:itemYearEndMonth", year: "frm0605:txtYearEnded", twoDigit: false },
  { code: "0619E", month: "frm0619E:txtMonth", year: "frm0619E:txtYear", twoDigit: false },
  { code: "0619F", month: "frm0619F:txtMonth", year: "frm0619F:txtYear", twoDigit: false },
  { code: "1600PT", month: "frm1600PT:txtPg1I1Month", year: "frm1600PT:txtPg1I1Year", twoDigit: true },
  { code: "1600VT", month: "frm1600VT:txtPg1I1Month", year: "frm1600VT:txtPg1I1Year", twoDigit: true },
  { code: "1601C", month: "frm1601c:txtMonth", year: "frm1601c:txtYear", twoDigit: false },
  { code: "1701", month: "frm1701:txtPg1I1Month", year: "frm1701:txtPg1I1Year", twoDigit: false },
  { code: "1701A", month: "frm1701A:txtMonth", year: "frm1701A:txtYear", twoDigit: false },
  { code: "1701MS", month: "frm1701MS:txtMonthNo1", year: "frm1701MS:txtYearNo1", twoDigit: false },
  { code: "2000", month: "frm2000:txtMonth", year: "frm2000:txtYear", twoDigit: false },
  { code: "2550M", month: "frm2550m:RtnYear", year: "frm2550m:txtYear", twoDigit: false },
  { code: "2550Q", month: "frm2550qv2024:selectedMonthNo2", year: "frm2550qv2024:txtYearNo2", twoDigit: false },
  { code: "2551Q", month: "frm2551Qv2018:rtnMonth", year: "frm2551Qv2018:txtYear", twoDigit: false },
  { code: "2553", month: "frm2553:itemYearEndMonth", year: "frm2553:txtYearEnded", twoDigit: false },
  { code: "1702EX", month: "frm1702EX:ddlPg1I2Date", year: "frm1702EX:txtPg1I2YearEnd", twoDigit: true },
  { code: "1702MX", month: "frm1702MX:ddlPg1I2Date", year: "frm1702MX:txtPg1I2YearEnd", twoDigit: true },
  { code: "1702Q", month: "frm1702q:rbYrEndMonth", year: "frm1702q:txtYrEndYear", twoDigit: true },
  { code: "1702RT", month: "frm1702RT:ddlPg1I2Month", year: "frm1702RT:txtPg1I2Year", twoDigit: true },
];

const RANGES: { code: string; from: string; to: string; derivedTo?: string }[] = [
  { code: "1600WP", from: "frm1600WP:DateWithholdingMonth", to: "frm1600WP:DateWithholdingToMonth", derivedTo: "frm1600WP:DateWithholdingToMonth" },
  { code: "2550Q", from: "frm2550qv2024:RtnPeriodFromNo4", to: "frm2550qv2024:RtnPeriodToNo4" },
  { code: "1702EX", from: "frm1702EX:txtPg1Pt1I17EffectivityFrom", to: "frm1702EX:txtPg1Pt1I17EffectivityTo" },
  { code: "1701MS", from: "frm1701MS:txtTaxpayerNo16a", to: "frm1701MS:txtTaxpayerNo16a1" },
  { code: "1701MS", from: "frm1701MS:txtTaxpayerNo16b", to: "frm1701MS:txtTaxpayerNo16b1" },
  { code: "2551Q", from: "txtDateIssue", to: "txtDateExpiry" },
];

function items(code: string): SchemaItem[] {
  const schema = withoutHidden(bir.screen_form_schema(code) as FormSchema);
  return (schema.parts || []).flatMap((p) => p.items || []);
}

// One test per form: building a form's schema is the slow part, and one call into the script has a 2000 ms budget. Eighteen
// schemas in a single test went over it on slow Windows runners; one schema per test stays far inside it.
test("PR3: the eighteen month+year pairs are all listed", () => {
  expect(PAIRS.length).toBe(18);
});

for (const pair of PAIRS) {
  test(`PR3: ${pair.code}'s month+year pair is marked one_periods on one item, with the host's two_digit truth`, () => {
    createDemoTaxpayer();
    const found = items(pair.code).flatMap(onePeriods);
    const period = found.find((p) => p.month.k === pair.month && p.year.k === pair.year);
    expect(`${pair.code}: ${period ? "marked" : "missing"}`).toBe(`${pair.code}: marked`);
    expect(`${pair.code} twoDigit=${period!.twoDigit}`).toBe(`${pair.code} twoDigit=${pair.twoDigit}`);
    expect(period!.periodKey).toBe(`${pair.month}#period`);
  });
}

test("PR3: the six from-to pairs are all listed", () => {
  expect(RANGES.length).toBe(6);
});

for (const range of RANGES) {
  test(`PR3: ${range.code}'s from-to pair ${range.from} is marked date_ranges on one item`, () => {
    createDemoTaxpayer();
    const formItems = items(range.code);
    const found = formItems.flatMap(dateRanges);
    const hit = found.find((r) => r.from.anchor === range.from && r.to.anchor === range.to);
    expect(`${range.code}: ${hit ? "marked" : "missing"}`).toBe(`${range.code}: marked`);
    expect(hit!.derived).toBe(Boolean(range.derivedTo));
    const item = formItems.find((it) => (it.date_ranges || []).some(([from, to]) => from === range.from && to === range.to))!;
    if (range.derivedTo) expect(item.derived_to).toEqual([range.derivedTo]);
    else expect(item.derived_to).toBeUndefined();
  });
}

test("PR3: 2551Q's bare keys are whole-date boxes", () => {
  createDemoTaxpayer();
  const issue = items("2551Q").flatMap(dateRanges)[0];
  expect(issue.from.field?.t).toBe("date");
  expect(issue.to.field?.t).toBe("date");
});

// The year box's own kind says how it stores the year (select code, 20YY two digits, four digits). One form per test, as above.
function yearOf(code: string, key: string) {
  for (const it of items(code)) {
    const f = (it.f || []).find((x) => x.k === key);
    if (f) return f;
  }
  return null;
}

test("PR3: 1600PT's year is a select that lists about a hundred years", () => {
  createDemoTaxpayer();
  const year = yearOf("1600PT", "frm1600PT:txtPg1I1Year");
  expect(year?.t).toBe("sel");
  expect((year?.o || []).length).toBeGreaterThan(50);
});

for (const code of ["1702EX", "1702MX", "1702RT"]) {
  test(`PR3: ${code}'s 20YY year box caps at two digits`, () => {
    createDemoTaxpayer();
    const key = code === "1702RT" ? "frm1702RT:txtPg1I2Year" : `frm${code}:txtPg1I2YearEnd`;
    expect(`${code}: m=${yearOf(code, key)?.m}`).toBe(`${code}: m=2`);
  });
}

test("PR3: 1702Q's year box is a number box", () => {
  createDemoTaxpayer();
  expect(yearOf("1702Q", "frm1702q:txtYrEndYear")?.t).toBe("int");
});

for (const pair of PAIRS.filter((p) => !p.twoDigit)) {
  test(`PR3: ${pair.code}'s four-digit year box is a number box`, () => {
    createDemoTaxpayer();
    expect(`${pair.code}: ${yearOf(pair.code, pair.year)?.t}`).toBe(`${pair.code}: int`);
  });
}

test("PR3: 1701's month moved onto the year's item and 2000's year joined its month — one item per pair", () => {
  createDemoTaxpayer();
  const all = items("1701");
  const monthItems = all.filter((it) => (it.f || []).some((f) => f.k === "frm1701:txtPg1I1Month"));
  const yearItems = all.filter((it) => (it.f || []).some((f) => f.k === "frm1701:txtPg1I1Year"));
  expect(monthItems.length).toBe(1);
  expect(monthItems[0].no).toBe(yearItems[0].no);
  const all2000 = items("2000");
  const m2000 = all2000.filter((it) => (it.f || []).some((f) => f.k === "frm2000:txtMonth"));
  const y2000 = all2000.filter((it) => (it.f || []).some((f) => f.k === "frm2000:txtYear"));
  expect(m2000[0].no).toBe(y2000[0].no);
});
