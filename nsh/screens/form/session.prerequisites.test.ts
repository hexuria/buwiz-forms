import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession } from "./session";

warmForms();

test("later-quarter 2551Q names missing profile context on opening and links to the exact fields and year", async () => {
  createDemoTaxpayer();
  for (const period of [2, 3, 4]) {
    const opened: unknown[][] = [];
    const s = new FormSession({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period, mode: "return" }, {
      onOpenProfile: (field, year) => opened.push([field, year]),
    });
    try {
      expect(await s.open()).toBe(null);
      const missing = s.header().profilePrerequisites;
      expect(missing.map((item) => item.label)).toEqual(["Business start date", "Annual income-tax election"]);
      expect(missing[1].message).toContain("2025");
      for (const item of missing) s.openProfilePrerequisite(item.field);
      expect(opened).toEqual([["business_start_date", 2025], ["income_tax_election", 2025]]);
    } finally { s.dispose(); }
  }
});

test("profile advice disappears when the missing facts are saved, without queuing or reopening", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2024, { tax_classification: "SelfEmployed" });
  const s = new FormSession({ tin: DEMO.tin, formCode: "2551Q", year: 2024, period: 3, mode: "return" });
  try {
    expect(await s.open()).toBe(null);
    expect(s.header().profilePrerequisites.length).toBe(2);
    s.typeAsk("business_start_date", "2020-01-01");
    s.saveAsk("business_start_date");
    await s.idle();
    expect(s.header().profilePrerequisites.map((item) => item.field)).toEqual(["income_tax_election"]);
    s.pickElection("graduated");
    s.recordElection();
    await s.idle();
    expect(s.header().profilePrerequisites).toEqual([]);
  } finally { s.dispose(); }
});

test("Q1 and other forms do not receive later-quarter profile advice", async () => {
  createDemoTaxpayer();
  for (const [formCode, period] of [["2551Q", 1], ["1601C", 3]] as const) {
    const s = new FormSession({ tin: DEMO.tin, formCode, year: 2023, period, mode: "return" });
    try {
      expect(await s.open()).toBe(null);
      expect(s.header().profilePrerequisites).toEqual([]);
    } finally { s.dispose(); }
  }
});
