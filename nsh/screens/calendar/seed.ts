// Generated data for the Calendar's tests, screenshots and its fixture (CalendarDemo.tsx): the demo taxpayer (Juana Santos), an
// optional second branch and second taxpayer, with this year's forms, so the deadlines the host lists are real ones. Tests only:
// the screen never imports this file. Each call is safe to repeat.
import { bir, type DeadlineRow } from "../../bir";
import { createDemoTaxpayer } from "../../bir/testing";
import { isoDate } from "../../lib/dates";

/** The tax year the data is for. */
export const YEAR = new Date().getFullYear();

/** What a test asks for (the fixture's host params). */
export interface SeedOptions {
  /** false: an empty database (no taxpayer at all). */
  demo?: boolean;
  /** false: the taxpayers have no tax year yet (so no deadline). */
  years?: boolean;
  /** true: a second branch of hers (00001, Cebu branch) with 1601C and 2551Q (her rows then name the branch). */
  branch?: boolean;
  /** true: a second taxpayer (Sample Trading Corp) with 1601C and 2550Q. */
  second?: boolean;
}

/** The TIN root of the second taxpayer, as the host lists it. */
export const SECOND_ROOT = "000111229";

/** Creates what `options` ask for. */
export function seed(options: SeedOptions = {}): void {
  if (options.demo === false) return;
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  const years = options.years !== false;
  if (years && bir.list_tax_years(head.tin).length === 0) {
    bir.add_tax_year(head.tin, YEAR, false);
    bir.set_yearly_forms(head.tin, YEAR, ["1601C", "2551Q", "0619E", "2550Q"]);
  }
  if (options.branch && !taxpayer.branches.some((b) => b.code === "00001")) {
    const branch = bir.create_branch(taxpayer.tin_root, { branch_code: "00001", name: "Cebu branch" });
    if (years) {
      bir.add_tax_year(branch.tin, YEAR, false);
      bir.set_yearly_forms(branch.tin, YEAR, ["1601C", "2551Q"]);
    }
  }
  if (options.second && !bir.list_taxpayers().some((t) => t.tin_root === SECOND_ROOT)) {
    const corp = bir.create_taxpayer({
      tin_root: "000-111-229",
      name: "Sample Trading Corp",
      taxpayer_type: "Corporation",
      head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" },
    });
    if (years) {
      bir.add_tax_year(corp.branches[0].tin, YEAR, false);
      bir.set_yearly_forms(corp.branches[0].tin, YEAR, ["1601C", "2550Q"]);
    }
  }
}

/** Saves a draft of a return the way the form page does (open, then save): the deadline then reads Draft. */
export async function saveDraft(tin: string, code: string, period: number): Promise<void> {
  const opened = await bir.screen_form_open(tin, code, YEAR, period);
  bir.screen_form_save(code, opened.draft);
}

/**
 * Makes the deadlines read answer some returns as filed: the past 0619E returns as Submitted, the first half-year of 1601C as
 * Confirmed. A filed return cannot be made without sending something, so only the read changes: nothing is stored.
 */
export function readSomeAsFiled(): void {
  const real = bir.deadlines_between;
  (bir as { deadlines_between: typeof real }).deadlines_between = (from, to) =>
    real(from, to).map((d: DeadlineRow) => {
      const past = d.due_date < isoDate(new Date());
      if (d.form_code === "0619E" && d.status === "Not started" && past) return { ...d, status: "Submitted" };
      if (d.form_code === "1601C" && d.status === "Not started" && d.period <= 6) return { ...d, status: "Confirmed" };
      return d;
    });
}
