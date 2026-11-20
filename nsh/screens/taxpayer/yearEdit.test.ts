// F8-8: a section edit that holds a tax year's facts (Registered activity: the line of business and the start date) belongs to that
// year. A year switch closes it (the page calls leaveYear), and a Save that finds another year on screen writes nothing. Through the
// real door (scratch database, the demo taxpayer Juana Santos with two tax years), without the page: the state the page draws.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { app, store } from "../../shell";
import { createPageState } from "./page";

const YEAR = new Date().getFullYear();
const LAST = YEAR - 1;

function seed() {
  const t = createDemoTaxpayer();
  for (const y of [YEAR, LAST]) {
    if (!bir.list_tax_years(DEMO.tin).includes(y)) bir.add_tax_year(DEMO.tin, y, false);
    bir.set_year_facts(DEMO.tin, y, { line_of_business: `Retail ${y}` });
  }
  store.bump();
  app.selectTaxpayer(t.tin_root, { quiet: true });
  app.selectYear(YEAR);
}

const lineOf = (y: number) => bir.year_facts(DEMO.tin, y).facts.line_of_business;

test("a Save after the year changed under the edit writes nothing into the new year, and says so", () => {
  seed();
  const s = createPageState();
  s.startEdit("branch");
  expect(s.edit()?.yearFields).toBe(true);
  expect(s.edit()?.year).toBe(YEAR);
  s.setValue("line_of_business", "Typed for this year");
  // The year moves without the page's leaveYear (the guard of the save itself).
  app.selectYear(LAST);
  s.saveSection();
  expect(s.edit()).toBeNull();
  store.bump();
  expect(lineOf(LAST)).toBe(`Retail ${LAST}`);
  expect(lineOf(YEAR)).toBe(`Retail ${YEAR}`);
  // Opened again for the year on screen, it saves into that year.
  s.startEdit("branch");
  expect(s.edit()?.year).toBe(LAST);
  s.setValue("line_of_business", "Typed for last year");
  s.saveSection();
  expect(s.edit()).toBeNull();
  store.bump();
  expect(lineOf(LAST)).toBe("Typed for last year");
  expect(lineOf(YEAR)).toBe(`Retail ${YEAR}`);
});

test("a year switch closes an edit that holds the year's facts, and leaves one that does not", () => {
  seed();
  const s = createPageState();
  s.startEdit("branch");
  s.setValue("line_of_business", "Half typed");
  app.selectYear(LAST);
  s.leaveYear();
  expect(s.edit()).toBeNull();
  // The taxpayer's own section (name, TIN) is the same in every year: it stays open.
  s.startEdit("taxpayer");
  s.setValue("name", "Juana Santos Trading");
  app.selectYear(YEAR);
  s.leaveYear();
  expect(s.edit()?.sec).toBe("taxpayer");
  expect(s.edit()?.values.name).toBe("Juana Santos Trading");
  s.cancelEdit();
});

test("a form's business-start-date link focuses the year's activity editor and retains it on a refused save", () => {
  seed();
  const s = createPageState();
  s.startEdit("branch", { focusField: "business_start_date", fromForm: true });
  expect(s.edit()?.focusField).toBe("business_start_date");
  expect(s.edit()?.fromForm).toBe(true);
  expect(s.edit()?.year).toBe(YEAR);
  s.setValue("business_start_date", "not a date");
  s.saveSection();
  expect(s.edit()?.focusField).toBe("business_start_date");
  expect(s.errors().business_start_date).not.toBe(undefined);
  s.setValue("business_start_date", "2020-01-01");
  s.saveSection();
  expect(s.edit()).toBeNull();
  expect(bir.year_facts(DEMO.tin, YEAR).facts.business_start_date).toBe("2020-01-01");
});
