// Filings at desktop size (1440 x 900, macOS: the board) and at phone size (390 x 844: the list under status tabs), against a
// scratch database seeded through the host (seed.ts: the demo taxpayer, her Cebu branch, a second taxpayer, seven returns).
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { bir } from "../../bir";
import { IDS } from "./seed";

// The submission worker starts once per process, on the first database an engine thread opens. Opening this thread's own
// scratch database first gives the worker to it: the apps below open their own databases, where a queued return stays queued.
bir.system_info();

const ENTRY = "../../FilingsDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const card = (n: number) => `id:return-${n}`;
const inCol = (col: string, n: number) => `id:filings-col-${col} >> id:return-${n}`;
const count = (text: string) => `id:filings-count >> text:${text}`;
/** The local month "YYYY-MM": the seeded returns were all changed just now. */
const thisMonth = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
};
const seen = (app: App, ...ids: number[]) => ids.every((n) => app.shows(card(n)));
const unseen = (app: App, ...ids: number[]) => ids.every((n) => !app.shows(card(n)));

async function board(params: Record<string, unknown> = {}, size: { width: number; height: number; platform: "macos" } = WIDE): Promise<App> {
  const app = await open(ENTRY, { ...size, ...asking(params) });
  await app.waitFor("id:filings-board", { timeout: 20000 });
  return app;
}

test("desktop: four columns, a card per return, Paid inside Confirmed", async () => {
  const app = await board();
  await app.waitFor(count("7 returns"));
  for (const title of ["Draft", "Queued", "Submitted", "Confirmed"]) expect(app.texts()).toContain(title);
  expect(app.shows("id:tab-bar")).toBe(false);
  expect(app.shows("id:filings-tabs")).toBe(false);

  expect(app.shows(inCol("draft", IDS.draft))).toBe(true);
  expect(app.shows(inCol("draft", IDS.cebu))).toBe(true);
  expect(app.shows(inCol("draft", IDS.second))).toBe(true);
  expect(app.shows(inCol("queued", IDS.queued))).toBe(true);
  expect(app.shows(inCol("submitted", IDS.submitted))).toBe(true);
  expect(app.shows(inCol("confirmed", IDS.confirmed))).toBe(true);
  expect(app.shows(inCol("confirmed", IDS.paid))).toBe(true);

  // A card: "2551Q Q3 2026", whose it is, its date.
  const texts = app.texts();
  for (const title of ["2551Q Q3 2026", "1601C Sep 2026", "2550Q Q2 2026", "1701Q Q1 2026", "1604C TY 2025", "2551Q Q2 2026", "1601C Aug 2026"]) {
    expect(texts).toContain(title);
  }
  expect(app.count("id:filings-board >> text:Juana Santos")).toBe(6);
  expect(app.count("id:filings-board >> text:Sample Trading Corp")).toBe(1);
  // The branch only where it is not the head office.
  expect(app.count("id:filings-board >> text:Cebu branch")).toBe(1);
  expect(texts).not.toContain("Head office");
  // A draft's date is its last save, any other return's its last change.
  expect(texts.filter((t) => /^Saved [A-Z][a-z]{2} \d{1,2}, \d{4}$/.test(t)).length).toBe(3);
  expect(texts.filter((t) => /^Updated [A-Z][a-z]{2} \d{1,2}, \d{4}$/.test(t)).length).toBe(4);
  // The column says the status; a card repeats it only where the two differ (Paid under Confirmed).
  expect(app.shows("id:return-5-status")).toBe(true);
  expect(app.shows("id:return-5-status >> text:Paid")).toBe(true);
  for (const n of [IDS.draft, IDS.queued, IDS.submitted, IDS.confirmed, IDS.cebu, IDS.second]) expect(app.shows(`id:return-${n}-status`)).toBe(false);
  // Cancel queue belongs to the queued card alone.
  expect(app.shows("id:return-2-cancel")).toBe(true);
  expect(app.count("text:Cancel queue")).toBe(1);
  // Nothing to clear yet.
  expect(app.shows("id:filings-clear")).toBe(false);
});

test("desktop: a card whose send needs a word says it: Held, Retrying, Needs attention, Not sent, each with the host's line", async () => {
  // Without stand-ins the host's own answer for the seeded queued return is "Queued to send.": no chip for that.
  const plain = await board();
  await plain.waitFor(count("7 returns"));
  for (const n of [IDS.draft, IDS.queued, IDS.cebu, IDS.second]) expect(plain.shows(`id:return-${n}-send`)).toBe(false);

  const app = await board({ send: true });
  await app.waitFor("id:return-2-send", { timeout: 20000 });
  const chip = (n: number, label: string) => app.shows(`id:return-${n}-send-chip >> text:${label}`);
  expect(chip(IDS.draft, "Not sent")).toBe(true);
  expect(chip(IDS.queued, "Needs attention")).toBe(true);
  expect(chip(IDS.cebu, "Held")).toBe(true);
  expect(chip(IDS.second, "Retrying")).toBe(true);
  expect(app.shows(`id:return-${IDS.draft}-send >> text:This TIN is not on your Buwiz account.`)).toBe(true);
  expect(app.shows(`id:return-${IDS.queued}-send >> text:The BIR may have received this return. Check for the BIR's confirmation email before sending it again.`)).toBe(true);
  expect(app.shows(`id:return-${IDS.cebu}-send >> text:Buwiz is holding sends for now.`)).toBe(true);
  expect(app.shows(`id:return-${IDS.second}-send >> text:Not sent yet: BIR is not accepting returns right now. It tries again by itself.`)).toBe(true);
  // The two held as Queued sit in the Queued column; nothing else changes on the board.
  expect(app.shows(inCol("queued", IDS.cebu))).toBe(true);
  expect(app.shows(inCol("queued", IDS.second))).toBe(true);
  for (const n of [IDS.submitted, IDS.confirmed, IDS.paid]) expect(app.shows(`id:return-${n}-send`)).toBe(false);
});

test("desktop: a dry-run send's card says Dry run in the Submitted column; a real send's card says nothing more", async () => {
  const real = await board();
  await real.waitFor(count("7 returns"));
  expect(real.shows(`id:return-${IDS.submitted}-status`)).toBe(false);
  expect(real.count("text:Dry run")).toBe(0);

  const app = await board({ dry: true });
  await app.waitFor(`id:return-${IDS.submitted}-status`, { timeout: 20000 });
  // Same column as a real send (the stored status is Submitted); the card says the banner's word.
  expect(app.shows(inCol("submitted", IDS.submitted))).toBe(true);
  expect(app.shows(`id:return-${IDS.submitted}-status >> text:Dry run`)).toBe(true);
  expect(app.texts()).toContain("Submitted"); // the column's title stays
  for (const n of [IDS.draft, IDS.queued, IDS.confirmed, IDS.cebu, IDS.second]) expect(app.shows(`id:return-${n}-status`)).toBe(false);
});

test("desktop: a card opens its return, selects its branch, and Back finds the filters as they were", async () => {
  const app = await board();
  await app.type("id:filings-search", "cebu");
  await app.waitFor(count("1 return"));
  await app.press(card(IDS.cebu));
  await app.waitFor("id:form-page", { timeout: 20000 });
  expect(app.texts()).toContain("000-123-453-00001");
  await app.press("id:tb-back");
  await app.waitFor("id:filings-board");
  expect(app.value("id:filings-search")).toBe("cebu");
  expect(app.shows(count("1 return"))).toBe(true);
  expect(seen(app, IDS.cebu)).toBe(true);
  expect(unseen(app, IDS.draft)).toBe(true);
});

test("desktop: Cancel queue makes the return a draft again and does not open it", async () => {
  const app = await board();
  await app.press("id:return-2-cancel");
  await app.waitFor("text:Queue cancelled. The return is an editable draft again.");
  expect(app.shows("id:form-page")).toBe(false);
  await app.waitFor(inCol("draft", IDS.queued));
  expect(app.shows("id:filings-col-queued >> text:Nothing here.")).toBe(true);
  expect(app.shows("id:return-2-cancel")).toBe(false);
  // Its date now reads as a save.
  expect(app.texts().filter((t) => /^Saved /.test(t)).length).toBe(4);
});

test("desktop: the search keeps the returns holding every word, and Clear filters empties it", async () => {
  const app = await board();
  const search = async (text: string, shown: number[], hidden: number[]) => {
    await app.clear("id:filings-search");
    await app.type("id:filings-search", text);
    await app.waitFor(count(`${shown.length} return${shown.length === 1 ? "" : "s"}`));
    expect(seen(app, ...shown)).toBe(true);
    expect(unseen(app, ...hidden)).toBe(true);
  };
  await search("cebu", [IDS.cebu], [IDS.draft, IDS.queued, IDS.second]);
  expect(app.shows("id:filings-clear")).toBe(true);
  await search("1601c", [IDS.queued, IDS.second], [IDS.draft, IDS.cebu, IDS.paid]);
  await search("2551q q3", [IDS.draft], [IDS.cebu, IDS.queued]);
  await search("sample trading", [IDS.second], [IDS.draft]);
  await search("paid", [IDS.paid], [IDS.confirmed]);
  await search("00012345300001", [IDS.cebu], [IDS.draft]);
  await app.clear("id:filings-search");
  await app.type("id:filings-search", "no such return");
  await app.waitFor("id:filings-empty");
  expect(app.texts()).toContain("No returns match these filters.");
  expect(app.shows("id:filings-board")).toBe(false);
  await app.press("id:filings-clear-empty");
  await app.waitFor("id:filings-board");
  expect(app.value("id:filings-search")).toBe("");
  expect(app.shows(count("7 returns"))).toBe(true);
  expect(app.shows("id:filings-clear")).toBe(false);
});

test("desktop: each of the four filters keeps its returns; they combine; Clear filters resets them", async () => {
  const app = await board();
  // Taxpayers: Clear unticks every box, then one taxpayer.
  await app.press("id:filings-taxpayers-trigger");
  await app.waitFor("id:filings-taxpayers-opt-000111225");
  await app.press("id:filings-taxpayers-clear");
  await app.press("id:filings-taxpayers-opt-000111225");
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.second)).toBe(true);
  expect(unseen(app, IDS.draft, IDS.queued, IDS.cebu)).toBe(true);
  expect(app.shows("id:filings-clear")).toBe(true);
  await app.press("id:filings-clear");
  await app.waitFor(count("7 returns"));

  // Forms.
  await app.press("id:filings-forms-trigger");
  await app.waitFor("id:filings-forms-opt-2551Q");
  await app.press("id:filings-forms-clear");
  await app.press("id:filings-forms-opt-2551Q");
  await app.waitFor(count("2 returns"));
  expect(seen(app, IDS.draft, IDS.cebu)).toBe(true);
  expect(unseen(app, IDS.queued, IDS.second, IDS.paid)).toBe(true);

  // Periods: a monthly return belongs to its quarter (1601C Aug and Sep 2026 are in Q3 2026 with the 2551Q).
  await app.press("id:filings-forms-clear");
  await app.press("id:filings-forms-all");
  await app.waitFor(count("7 returns"));
  await app.press("id:filings-periods-trigger");
  await app.waitFor("id:filings-periods-opt-Q3 2026");
  await app.press("id:filings-periods-clear");
  await app.press("id:filings-periods-opt-Q3 2026");
  await app.waitFor(count("3 returns"));
  expect(seen(app, IDS.draft, IDS.queued, IDS.second)).toBe(true);
  expect(unseen(app, IDS.cebu, IDS.submitted, IDS.confirmed, IDS.paid)).toBe(true);
  await app.press("id:filings-periods-all");
  await app.waitFor(count("7 returns"));
  // An annual return is "TY 2025".
  await app.press("id:filings-periods-clear");
  await app.press("id:filings-periods-opt-TY 2025");
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.paid)).toBe(true);
  await app.press("id:filings-periods-all");
  await app.waitFor(count("7 returns"));

  // Statuses: Paid joins Confirmed on the board, but is a status of its own in the filter.
  await app.press("id:filings-statuses-trigger");
  await app.waitFor("id:filings-statuses-opt-Paid");
  await app.press("id:filings-statuses-clear");
  await app.press("id:filings-statuses-opt-Paid");
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.paid)).toBe(true);
  expect(unseen(app, IDS.confirmed)).toBe(true);
  await app.press("id:filings-statuses-opt-Confirmed");
  await app.waitFor(count("2 returns"));

  // Together: Confirmed or Paid, and only the second taxpayer: none.
  await app.press("id:filings-taxpayers-trigger");
  await app.waitFor("id:filings-taxpayers-opt-000111225");
  await app.press("id:filings-taxpayers-clear");
  await app.press("id:filings-taxpayers-opt-000111225");
  await app.waitFor("id:filings-empty");
  expect(app.texts()).toContain("No returns match these filters.");
  await app.press("id:filings-clear-empty");
  await app.waitFor(count("7 returns"));
  expect(app.shows("id:filings-clear")).toBe(false);
});

test("desktop: no returns yet says so, and the filters stay", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ seed: "none" }) });
  await app.waitFor("id:filings-empty", { timeout: 20000 });
  expect(app.texts()).toContain("Saved and filed returns show here.");
  expect(app.shows("id:filings-board")).toBe(false);
  expect(app.shows("id:filings-search")).toBe(true);
  expect(app.shows("id:filings-taxpayers-trigger")).toBe(true);
  expect(app.shows(count("0 returns"))).toBe(true);
  expect(app.shows("id:filings-clear")).toBe(false);
});

test("desktop: a navigation preset (statuses, updatedMonth) replaces every filter, once", async () => {
  // Drafts.
  let app = await board({ go: { statuses: ["Draft"] } });
  await app.waitFor(count("3 returns"));
  expect(seen(app, IDS.draft, IDS.cebu, IDS.second)).toBe(true);
  expect(unseen(app, IDS.queued, IDS.paid)).toBe(true);
  expect(app.shows("id:filings-statuses >> text:Draft")).toBe(true);
  expect(app.shows("id:filings-month-chip")).toBe(false);
  expect(app.shows("id:filings-clear")).toBe(true);

  // Waiting to send.
  app = await board({ go: { statuses: ["Queued"] } });
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.queued)).toBe(true);

  // Filed in this month: Submitted, Confirmed and Paid, changed in this month.
  const month = thisMonth();
  app = await board({ go: { statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: month } });
  await app.waitFor(count("3 returns"));
  expect(seen(app, IDS.submitted, IDS.confirmed, IDS.paid)).toBe(true);
  expect(app.shows("id:filings-statuses >> text:3 statuses")).toBe(true);
  expect(app.shows("id:filings-month-chip")).toBe(true);
  expect(app.texts().some((t) => /^Changed in [A-Z][a-z]{2} \d{4}$/.test(t))).toBe(true);
  // The chip comes off with a press; the statuses stay.
  await app.press("id:filings-month-chip");
  await app.waitFor("id:filings-month-chip", { state: "gone" });
  expect(app.shows(count("3 returns"))).toBe(true);
  expect(app.shows("id:filings-clear")).toBe(true);

  // A month with no change: nothing matches, and Clear filters brings every return back.
  app = await open(ENTRY, { ...WIDE, ...asking({ go: { statuses: ["Draft", "Queued", "Submitted", "Confirmed", "Paid"], updatedMonth: "2020-01" } }) });
  await app.waitFor("id:filings-empty", { timeout: 20000 });
  expect(app.texts()).toContain("No returns match these filters.");
  expect(app.shows("id:filings-statuses >> text:All statuses")).toBe(true);
  await app.press("id:filings-clear-empty");
  await app.waitFor(count("7 returns"));
  expect(app.shows("id:filings-month-chip")).toBe(false);

  // A preset is applied once: after Clear filters, opening a return and coming Back does not bring it back.
  app = await board({ go: { statuses: ["Draft"] } });
  await app.waitFor(count("3 returns"));
  await app.press("id:filings-clear");
  await app.waitFor(count("7 returns"));
  await app.press(card(IDS.draft));
  await app.waitFor("id:form-page", { timeout: 20000 });
  await app.press("id:tb-back");
  await app.waitFor("id:filings-board");
  expect(app.shows(count("7 returns"))).toBe(true);
});

// ---------------------------------------------------------------- phone: 390 x 844 (default viewport, iOS: touch targets)

async function list(params: Record<string, unknown> = {}): Promise<App> {
  const app = await open(ENTRY, asking(params));
  await app.waitFor("id:filings-tabs", { timeout: 20000 });
  return app;
}
const tab = (name: string) => `id:filings-tabs >> label:${name}`;

test("phone: one list under status tabs, with the four filters behind a Filters button", async () => {
  const app = await list();
  await app.waitFor(count("7 returns"));
  expect(app.shows("id:tab-bar")).toBe(true);
  expect(app.shows("id:filings-board")).toBe(false);
  expect(app.shows("id:filings-search")).toBe(true);
  expect(app.shows("id:filings-filters-open")).toBe(true);
  for (const name of ["All", "Draft", "Queued", "Submitted", "Confirmed"]) expect(app.shows(tab(name))).toBe(true);
  // The four combos are not on the page; they are in the sheet.
  expect(app.shows("id:filings-taxpayers-trigger")).toBe(false);
  expect(seen(app, IDS.draft, IDS.queued, IDS.submitted, IDS.confirmed, IDS.paid, IDS.cebu, IDS.second)).toBe(true);
  // On All every row says its status (Draft and Queued are the kit's chips); Paid keeps its word.
  expect(app.shows("id:return-1-status")).toBe(true);
  expect(app.shows("id:return-2-status >> text:Queued")).toBe(true);
  expect(app.shows("id:return-3-status >> text:Submitted")).toBe(true);
  expect(app.shows("id:return-4-status >> text:Confirmed")).toBe(true);
  expect(app.shows("id:return-5-status >> text:Paid")).toBe(true);
  expect(app.shows("id:return-6-status >> text:Draft")).toBe(true);
  expect(app.count("text:Cancel queue")).toBe(1);
  expect(app.count("id:filings-rows >> text:Cebu branch")).toBe(1);
});

test("phone: a row whose send needs a word has its chip and line", async () => {
  const app = await list({ send: true });
  await app.waitFor("id:return-1-send", { timeout: 20000 });
  expect(app.shows(`id:return-${IDS.draft}-send-chip >> text:Not sent`)).toBe(true);
  expect(app.shows(`id:return-${IDS.queued}-send-chip >> text:Needs attention`)).toBe(true);
  expect(app.texts()).toContain("This TIN is not on your Buwiz account.");
});

test("phone: a dry-run send's row says Dry run, on All and under Submitted; a real send's says Submitted on All", async () => {
  const app = await list({ dry: true });
  await app.waitFor(count("7 returns"));
  expect(app.shows(`id:return-${IDS.submitted}-status >> text:Dry run`)).toBe(true);
  expect(app.shows(`id:return-${IDS.submitted}-status >> text:Submitted`)).toBe(false);
  await app.press(tab("Submitted"));
  await app.waitFor(count("1 return"));
  expect(app.shows(`id:return-${IDS.submitted}-status >> text:Dry run`)).toBe(true);
});

test("phone: a tab keeps the returns of its status; Confirmed holds Paid", async () => {
  const app = await list();
  await app.waitFor(count("7 returns"));
  await app.press(tab("Draft"));
  await app.waitFor(count("3 returns"));
  expect(seen(app, IDS.draft, IDS.cebu, IDS.second)).toBe(true);
  expect(unseen(app, IDS.queued, IDS.paid)).toBe(true);
  // The tab says the status, so the rows do not repeat it.
  expect(app.shows("id:return-1-status")).toBe(false);
  await app.press(tab("Queued"));
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.queued)).toBe(true);
  await app.press(tab("Submitted"));
  await app.waitFor(card(IDS.submitted));
  expect(unseen(app, IDS.queued)).toBe(true);
  await app.press(tab("Confirmed"));
  await app.waitFor(count("2 returns"));
  expect(seen(app, IDS.confirmed, IDS.paid)).toBe(true);
  expect(app.shows("id:return-5-status >> text:Paid")).toBe(true);
  expect(app.shows("id:return-4-status")).toBe(false);
  await app.press(tab("All"));
  await app.waitFor(count("7 returns"));
});

test("phone: unticking a status hides its tab, ticking it again shows it, and a hidden current tab returns to All", async () => {
  const app = await list();
  await app.press(tab("Draft"));
  await app.waitFor(count("3 returns"));

  await app.press("id:filings-filters-open");
  await app.waitFor("id:filings-sheet-done");
  await app.press("id:filings-statuses-trigger");
  await app.waitFor("id:filings-statuses-opt-Draft");
  await app.press("id:filings-statuses-opt-Draft");
  await app.waitFor("text:Show 4 returns");
  await app.press("id:filings-sheet-done");
  await app.waitFor("id:filings-sheet-done", { state: "gone" });

  expect(app.shows(tab("Draft"))).toBe(false);
  for (const name of ["All", "Queued", "Submitted", "Confirmed"]) expect(app.shows(tab(name))).toBe(true);
  expect(app.shows(count("4 returns"))).toBe(true);
  expect(seen(app, IDS.queued, IDS.submitted, IDS.confirmed, IDS.paid)).toBe(true);
  expect(unseen(app, IDS.draft, IDS.cebu, IDS.second)).toBe(true);

  await app.press("id:filings-filters-open");
  await app.waitFor("id:filings-sheet-done");
  await app.press("id:filings-statuses-trigger");
  await app.waitFor("id:filings-statuses-opt-Draft");
  await app.press("id:filings-statuses-opt-Draft");
  await app.waitFor("text:Show 7 returns");
  await app.press("id:filings-sheet-done");
  await app.waitFor("id:filings-sheet-done", { state: "gone" });

  expect(app.shows(tab("Draft"))).toBe(true);
  expect(app.shows(count("7 returns"))).toBe(true);
  expect(seen(app, IDS.draft, IDS.cebu, IDS.second)).toBe(true);
});

test("phone: search stays on the page and works inside a tab", async () => {
  const app = await list();
  await app.press(tab("Draft"));
  await app.waitFor(count("3 returns"));
  await app.type("id:filings-search", "sample");
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.second)).toBe(true);
  expect(unseen(app, IDS.draft, IDS.cebu)).toBe(true);
  expect(app.shows("id:filings-clear")).toBe(true);
  await app.press("id:filings-clear");
  await app.waitFor(count("3 returns"));
  expect(app.value("id:filings-search")).toBe("");
  // A tab with nothing in it says so while the search keeps returns on another tab; the tabs show where they are.
  await app.press(tab("Queued"));
  await app.waitFor(count("1 return"));
  await app.type("id:filings-search", "cebu");
  await app.waitFor("id:filings-tab-empty");
  expect(app.texts()).toContain("Nothing here.");
  expect(app.shows(count("0 returns"))).toBe(true);
  expect(app.shows(tab("Draft") + " >> text:1")).toBe(true);
  // A search that keeps nothing at all says so, with Clear filters.
  await app.clear("id:filings-search");
  await app.type("id:filings-search", "no such return");
  await app.waitFor("id:filings-empty");
  expect(app.texts()).toContain("No returns match these filters.");
  await app.press("id:filings-clear-empty");
  await app.waitFor(count("1 return"));
  expect(seen(app, IDS.queued)).toBe(true);
});

test("phone: the Filters sheet holds the four filters; its button counts the narrowed ones", async () => {
  const app = await list();
  await app.press("id:filings-filters-open");
  await app.waitFor("id:filings-sheet-done");
  for (const label of ["Taxpayers", "Forms", "Periods", "Statuses"]) expect(app.texts()).toContain(label);
  expect(app.shows("id:filings-sheet-clear")).toBe(false);
  await app.press("id:filings-taxpayers-trigger");
  await app.waitFor("id:filings-taxpayers-opt-000111225");
  await app.press("id:filings-taxpayers-clear");
  await app.press("id:filings-taxpayers-opt-000111225");
  await app.waitFor("text:Show 1 return");
  expect(app.shows("id:filings-sheet-clear")).toBe(true);
  await app.press("id:filings-sheet-done");
  await app.waitFor("id:filings-sheet-done", { state: "gone" });
  expect(app.shows(count("1 return"))).toBe(true);
  expect(seen(app, IDS.second)).toBe(true);
  expect(unseen(app, IDS.draft)).toBe(true);
  expect(app.shows("id:filings-filters-open >> text:1")).toBe(true);
  // Clear filters on the page lifts them.
  await app.press("id:filings-clear");
  await app.waitFor(count("7 returns"));
  expect(app.shows("id:filings-filters-open >> text:1")).toBe(false);

  // Statuses, in the sheet, with Clear filters there too.
  await app.press("id:filings-filters-open");
  await app.waitFor("id:filings-sheet-done");
  await app.press("id:filings-statuses-trigger");
  await app.waitFor("id:filings-statuses-opt-Paid");
  await app.press("id:filings-statuses-clear");
  await app.press("id:filings-statuses-opt-Paid");
  await app.waitFor("text:Show 1 return");
  await app.press("id:filings-sheet-clear");
  await app.waitFor("text:Show 7 returns");
  await app.press("id:filings-sheet-done");
  await app.waitFor("id:filings-sheet-done", { state: "gone" });
  expect(app.shows(count("7 returns"))).toBe(true);
});

test("phone: a row opens its return; Cancel queue turns a queued one back into a draft", async () => {
  const app = await list();
  await app.press("id:return-2-cancel");
  await app.waitFor("text:Queue cancelled. The return is an editable draft again.");
  expect(app.shows("id:form-page")).toBe(false);
  await app.waitFor("id:return-2-status >> text:Draft");
  expect(app.shows("id:return-2-cancel")).toBe(false);
  await app.press(card(IDS.cebu));
  await app.waitFor("id:form-page", { timeout: 20000 });
  expect(app.shows("id:tab-bar")).toBe(true);
});

test("phone: a navigation preset shows its month and counts it on the Filters button", async () => {
  const app = await list({ go: { statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: thisMonth() } });
  await app.waitFor(count("3 returns"));
  expect(seen(app, IDS.submitted, IDS.confirmed, IDS.paid)).toBe(true);
  expect(unseen(app, IDS.draft)).toBe(true);
  expect(app.shows("id:filings-month-chip")).toBe(true);
  expect(app.shows("id:filings-filters-open >> text:2")).toBe(true);
  await app.press("id:filings-month-chip");
  await app.waitFor("id:filings-month-chip", { state: "gone" });
  expect(app.shows("id:filings-filters-open >> text:1")).toBe(true);
});

test("phone: no returns yet says so", async () => {
  const app = await open(ENTRY, asking({ seed: "none" }));
  await app.waitFor("id:filings-empty", { timeout: 20000 });
  expect(app.texts()).toContain("Saved and filed returns show here.");
  expect(app.shows("id:filings-tabs")).toBe(false);
  expect(app.shows("id:filings-search")).toBe(true);
});

test("the layout follows the page width: the board from 720 points of page, the list under it", async () => {
  const app = await board();
  expect(app.shows("id:filings-tabs")).toBe(false);
  await app.setViewport(390, 844);
  await app.waitFor("id:filings-tabs");
  expect(app.shows("id:filings-board")).toBe(false);
  expect(app.shows("id:filings-filters-open")).toBe(true);
  await app.setViewport(1440, 900);
  await app.waitFor("id:filings-board");
  expect(app.shows("id:filings-tabs")).toBe(false);
  // 860 points wide with the sidebar docked leaves a page narrower than 720: the list, and the filters in a drawer.
  await app.setViewport(900, 800);
  await app.waitFor("id:filings-tabs");
  await app.press("id:filings-filters-open");
  await app.waitFor("id:filings-sheet-done");
});
