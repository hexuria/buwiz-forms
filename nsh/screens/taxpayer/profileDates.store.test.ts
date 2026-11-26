// The profile's dates are typed and shown as the form page's (MM/DD/YYYY) and still stored as before: ISO (YYYY-MM-DD) in the host.
// Through the real door (scratch database, the demo taxpayer Juana Santos), the box's edge conversion (profileDateStored) and the
// page state the box writes to, without the page.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { app, store } from "../../shell";
import { profileDateShown, profileDateStored } from "./logic";
import { createPageState } from "./page";

const YEAR = new Date().getFullYear();
/** The taxpayer as the host holds it. */
const taxpayerOf = (root: string) => bir.list_taxpayers().find((t) => t.tin_root === root);

function seed() {
  const t = createDemoTaxpayer();
  if (!bir.list_tax_years(DEMO.tin).includes(YEAR)) bir.add_tax_year(DEMO.tin, YEAR, false);
  store.bump();
  app.selectTaxpayer(t.tin_root, { quiet: true });
  app.selectYear(YEAR);
  return t.tin_root;
}

test("a birth date typed as 05/17/1990 or 05171990 is stored as 1990-05-17, and shows as 05/17/1990 again", () => {
  const root = seed();
  for (const typed of ["05/17/1990", "05171990", "May 17, 1990"]) {
    const s = createPageState();
    s.startEdit("taxpayer");
    s.setValue("birth_date", profileDateStored(typed));
    expect(s.edit()?.values.birth_date).toBe("1990-05-17");
    s.saveSection();
    expect(s.edit()).toBeNull();
    store.bump();
    expect(taxpayerOf(root)?.birth_date).toBe("1990-05-17");
  }
  // Opened again, the box gets the stored day in the form's format.
  const s = createPageState();
  s.startEdit("taxpayer");
  expect(profileDateShown(s.edit()?.values.birth_date ?? "")).toBe("05/17/1990");
  // Cleared, nothing is stored.
  s.setValue("birth_date", profileDateStored(""));
  s.saveSection();
  store.bump();
  expect(taxpayerOf(root)?.birth_date ?? null).toBeNull();
});

test("an impossible day is kept as typed and refused on Save; nothing reaches the host", () => {
  const root = seed();
  const before = taxpayerOf(root)?.birth_date ?? null;
  const s = createPageState();
  s.startEdit("taxpayer");
  s.setValue("birth_date", profileDateStored("02/30/1990"));
  expect(s.edit()?.values.birth_date).toBe("02/30/1990");
  s.saveSection();
  expect(s.edit()?.sec).toBe("taxpayer");
  expect(s.errors().birth_date).toBe("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  store.bump();
  expect(taxpayerOf(root)?.birth_date ?? null).toBe(before);
  s.cancelEdit();
});

test("a business start date picked as 01/15/2020 is stored as 2020-01-15 in the year's facts", () => {
  seed();
  const s = createPageState();
  s.startEdit("branch");
  s.setValue("business_start_date", profileDateStored("01/15/2020"));
  s.saveSection();
  expect(s.edit()).toBeNull();
  expect(bir.year_facts(DEMO.tin, YEAR).facts.business_start_date).toBe("2020-01-15");
});
