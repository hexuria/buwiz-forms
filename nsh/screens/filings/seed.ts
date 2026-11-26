// Test and fixture data for Filings (the screen never imports this file): returns of every status created through the host
// against the scratch database, for the demo taxpayer (Juana Santos), her Cebu branch and one generated taxpayer.
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";

/** The generated second taxpayer. */
export const SECOND = { root: "000111225", rootDashed: "000-111-225", tin: "00011122500000", name: "Sample Trading Corp" } as const;

/** Juana's Cebu branch (not the head office): its TIN as host functions take it. */
export const CEBU = { code: "00001", name: "Cebu branch", tin: "00012345300001" } as const;

/** The year the seeded returns are for. */
export const YEAR = 2026;

/**
 * The seeded returns by the ids a fresh scratch database gives them (the order below), which are also the card ids on screen
 * ("return-1"). A test file's own bir calls use another database than the one an open() app reads, so a test names cards by these.
 */
export const IDS = {
  /** Juana, 2551Q Q3 2026, Draft. */
  draft: 1,
  /** Juana, 1601C Sep 2026, Queued. */
  queued: 2,
  /** Juana, 2550Q Q2 2026, Submitted. */
  submitted: 3,
  /** Juana, 1701Q Q1 2026, Confirmed. */
  confirmed: 4,
  /** Juana, 1604C TY 2025, Paid. */
  paid: 5,
  /** Juana's Cebu branch, 2551Q Q2 2026, Draft. */
  cebu: 6,
  /** Sample Trading Corp, 1601C Aug 2026, Draft. */
  second: 7,
} as const;

/** Makes the demo taxpayer, a branch and a second taxpayer, each with the year and forms their returns need. */
export function seedPeople(): void {
  const taxpayer = createDemoTaxpayer();
  if (!taxpayer.branches.some((b) => b.code === CEBU.code)) bir.create_branch(taxpayer.tin_root, { branch_code: CEBU.code, name: CEBU.name });
  if (!bir.list_taxpayers().some((t) => t.tin_root === SECOND.root)) {
    bir.create_taxpayer({
      tin_root: SECOND.rootDashed,
      name: SECOND.name,
      taxpayer_type: "Corporation",
      head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" },
    });
  }
  for (const tin of [DEMO.tin, CEBU.tin, SECOND.tin]) {
    if (bir.list_tax_years(tin).length === 0) {
      bir.add_tax_year(tin, YEAR, false);
      bir.set_yearly_forms(tin, YEAR, ["1601C", "2551Q"]);
    }
  }
}

function save(tin: string, code: string, year: number, period: number | null, status: string, data: object = {}): void {
  bir.save_form_draft(tin, code, year, period, status, data);
}

/** People and seven returns, one of each status and three taxpayers' worth: see IDS. A fresh database only (ids follow the order). */
export function seedFilings(): void {
  seedPeople();
  // A 1601C is queued only with its EOPT tier answered and amounts above zero; the dry-run app sends nothing.
  bir.set_year_facts(DEMO.tin, YEAR, { eopt_tier: "Micro" });
  save(DEMO.tin, "2551Q", YEAR, 3, "Draft");
  save(DEMO.tin, "1601C", YEAR, 9, "Draft", { tax_14_total_compensation: 100000, tax_25_total_taxes_withheld: 10000 });
  bir.queue_form_draft(DEMO.tin, "1601C", YEAR, 9);
  save(DEMO.tin, "2550Q", YEAR, 2, "Submitted");
  save(DEMO.tin, "1701Q", YEAR, 1, "Confirmed");
  save(DEMO.tin, "1604C", YEAR - 1, null, "Paid");
  save(CEBU.tin, "2551Q", YEAR, 2, "Draft");
  save(SECOND.tin, "1601C", YEAR, 8, "Draft");
}
