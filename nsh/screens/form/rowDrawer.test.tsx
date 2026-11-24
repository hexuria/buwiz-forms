// The row drawer's unfinished row (a typed tax base with no ATC chosen) can never be saved:
// Done stays shut with the reason until the row is fixed or cleared, clearing takes the page back
// to Saved, and the count beside the table counts the rows there are, not the rows the paper has.
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const PARAMS = { formCode: "1600VT", year: 2025, period: 1, mode: "return" };
const TABLE = "fp-table-frm1600VT:txtAtcCd1";
const BASE = "id:fp-frm1600VT:txtPg1P2TaxBase1";
const REFUSAL = "Choose this row's ATC first.";

async function openAtTable(params: Record<string, unknown> = PARAMS) {
  const app = await open(ENTRY, { ...DESKTOP, host: { params } });
  await app.waitFor("text:BIR Form 1600VT");
  // The schedule is a collapsed card of its own part: open every part until the table's Add row shows.
  // The computation's ATC table is in Part II (index 2): scroll its card into view, open it, then the table's Add row.
  for (let i = 0; i < 30 && !app.shows("id:fp-part-2-toggle"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-part-2-toggle");
  for (let i = 0; i < 30 && !app.shows(`id:${TABLE}-add`); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.waitFor(`id:${TABLE}-add`, { timeout: 10_000 });
  return app;
}

test("1600VT: no rows yet and no count label beside the table while nothing is typed", async () => {
  const app = await openAtTable();
  expect(app.shows(`id:${TABLE}-none`)).toBe(true);
  expect(app.texts()).toContain("No rows yet.");
  expect(app.texts().some((t) => /^\d+ rows?$/.test(t))).toBe(false);
});

test("1600VT: typing a tax base with no ATC shuts Done with the reason; clearing reopens it and the page saves", async () => {
  const app = await openAtTable();
  await app.press(`id:${TABLE}-add`);
  await app.waitFor("text:Row 13");
  await app.type(BASE, "1000");
  // The host refuses the write: the box carries the sentence, the drawer shows it, Done is shut.
  await app.waitFor(`text:${REFUSAL}`, { timeout: 15_000 });
  expect(app.shows(`id:${TABLE}-unfinished`)).toBe(true);
  expect(app.enabled(`id:${TABLE}-done`)).toBe(false);
  // Clearing the unsavable text takes the refusal away: Done opens, the drawer closes, and the
  // save line settles on Saved — never stuck on Unsaved changes for a row that is not there.
  await app.clear(BASE);
  await app.waitUntil(() => app.enabled(`id:${TABLE}-done`) === true, { timeout: 10_000 });
  await app.press(`id:${TABLE}-done`);
  await app.waitFor("text:Row 13", { state: "gone" });
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  expect(app.shows(`id:${TABLE}-none`)).toBe(true);
  expect(app.texts().some((t) => /^\d+ rows?$/.test(t))).toBe(false);
  expect(app.texts().some((t) => /Unsaved changes/.test(t))).toBe(false);
});

test("1600VT: an ATC pick the host refuses (Item 10 unanswered) shuts Done with the reason on its box", async () => {
  // No answers: the host refuses every ATC pick until Item 10 says the withholding agent's category.
  const app = await openAtTable();
  await app.press(`id:${TABLE}-add`);
  await app.waitFor("text:Row 13");
  await app.press("id:fp-frm1600VT:txtAtcCd1-trigger");
  await app.waitFor("text:/^WV010/", { timeout: 10_000 });
  await app.press("text:/^WV010/");
  // The refusal is the box's own message, the drawer shows it, and Done stays shut.
  await app.waitFor("text:/not necessary when item no. 3|select an option for Item 10/", { timeout: 15_000 });
  expect(app.shows(`id:${TABLE}-unfinished`)).toBe(true);
  expect(app.enabled(`id:${TABLE}-done`)).toBe(false);
  // The way out without reopening the form: Delete row clears the unsavable pick and the page saves.
  await app.press(`id:${TABLE}-delete`);
  await app.waitFor("text:Row 13", { state: "gone" });
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  expect(app.shows(`id:${TABLE}-none`)).toBe(true);
  expect(app.texts().some((t) => /Unsaved changes/.test(t))).toBe(false);
});

test("1600VT: a row made the page's own way lists its line and the count says 1 row, not the paper's five", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: PARAMS } });
  await app.waitFor("text:BIR Form 1600VT");
  // Part I first: Item 3 Yes and the Item 10 category, the answers a row's ATC needs.
  for (let i = 0; i < 30 && !app.shows("id:fp-part-1-toggle"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-part-1-toggle");
  for (let i = 0; i < 30 && !app.shows("id:fp-frm1600VT:rdoPg1I3TaxWithheldYes"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-frm1600VT:rdoPg1I3TaxWithheldYes");
  await app.press("id:fp-frm1600VT:rdoPg1I10CategoryofWthholdAgentPriv");
  // The computation's ATC table in Part II: a row the page's own way.
  for (let i = 0; i < 30 && !app.shows("id:fp-part-2-toggle"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-part-2-toggle");
  for (let i = 0; i < 30 && !app.shows(`id:${TABLE}-add`); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press(`id:${TABLE}-add`);
  await app.waitFor("text:Row 13");
  await app.press("id:fp-frm1600VT:txtAtcCd1-trigger");
  await app.waitFor("text:/^WV012/", { timeout: 10_000 });
  await app.press("text:/^WV012/");
  await app.type(BASE, "1000");
  await app.waitUntil(() => app.enabled(`id:${TABLE}-done`) === true, { timeout: 15_000 });
  await app.press(`id:${TABLE}-done`);
  await app.waitFor("text:Row 13", { state: "gone" });
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  // The row exists: its line lists it with the ATC's code (never the wire value), and the count is one row.
  expect(app.count(`id:fp-row-frm1600VT:txtAtcCd1-0`)).toBe(1);
  expect(app.texts().some((t) => t.startsWith("WV012"))).toBe(true);
  expect(app.texts().some((t) => t.includes("catalog:"))).toBe(false);
  expect(app.shows(`id:${TABLE}-none`)).toBe(false);
  await app.waitFor("text:1 row", { timeout: 5_000 });
  expect(app.texts().some((t) => /^Unsaved changes/.test(t))).toBe(false);
});
