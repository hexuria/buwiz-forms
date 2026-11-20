import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { app, store } from "../../shell";
import { createPageState } from "./page";

function seed() {
  const taxpayer = createDemoTaxpayer();
  if (!bir.list_tax_years(DEMO.tin).includes(2026)) bir.add_tax_year(DEMO.tin, 2026, false);
  bir.set_year_facts(DEMO.tin, 2026, { tax_classification: "SelfEmployed", income_tax_election: "GraduatedUnspecified" });
  store.bump();
  app.selectTaxpayer(taxpayer.tin_root, { quiet: true });
  app.selectYear(2026);
}

test("a form-linked year outside the profile's forms list owns the fact editor and its save", async () => {
  seed();
  expect(app.year()).toBe(2026);
  expect(bir.list_tax_years(DEMO.tin)).toEqual([2026]);
  app.go("profile", { year: 2027, edit: "income_tax_election", fromForm: true });
  await app.settled();
  expect(app.years()).toEqual([2026, 2027]);
  // TaxpayerPage selects the route year before creating the fact edit.
  app.selectYear(2027);
  const s = createPageState();
  s.startFactEdit("income_tax_election");
  expect(app.year()).toBe(2027);
  expect(s.yearFacts()?.year).toBe(2027);
  expect(s.factEdit()?.year).toBe(2027);
  expect(s.factEdit()?.value).toBeNull();
  expect(bir.list_tax_years(DEMO.tin)).toEqual([2026]);
  s.saveFactEdit();
  expect(s.factEdit()?.error).toBe("Choose one first.");
  expect(bir.year_facts(DEMO.tin, 2026).facts.income_tax_election).toBe("GraduatedUnspecified");
  s.pickFact("EightPercent");
  s.saveFactEdit();
  expect(s.factEdit()).toBeNull();
  expect(bir.year_facts(DEMO.tin, 2027).facts.income_tax_election).toBe("EightPercent");
  expect(bir.year_facts(DEMO.tin, 2026).facts.income_tax_election).toBe("GraduatedUnspecified");
});

test("ordinary profile routes and invalid form years do not extend the profile's year choices", async () => {
  seed();
  for (const params of [
    { year: 2027 },
    { fromForm: true },
    { fromForm: true, year: "2027" },
    { fromForm: true, year: 2027.5 },
    { fromForm: true, year: store.yearBounds().max + 1 },
    { fromForm: true, year: store.yearBounds().min - 1 },
  ]) {
    app.go("profile", params);
    await app.settled();
    app.selectYear(2026);
    expect(app.years()).toEqual([2026]);
    expect(app.year()).toBe(2026);
  }
});
