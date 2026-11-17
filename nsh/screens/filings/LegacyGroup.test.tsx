// "Filed in BIR eBIRForms" on Filings at desktop size (1440 x 900, macOS: the board) and at phone size (390 x 844: the list under status
// tabs), against the fixture's answers (FilingsDemo.tsx, `legacy: "rows"`: made-up returns of the demo taxpayer, her Cebu branch and a
// second generated taxpayer). The host's own answer is tested in Rust (crates/bir-app-api/src/legacy_import_module.rs): an import
// needs the host's folder dialog, so a page test stands in for the answer.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { bir, type LegacyFiledReturn } from "../../bir";
import { createDemoTaxpayer } from "../../bir/testing";
import { store } from "../../shell/store";
import { IDS } from "./seed";

const ENTRY = "../../FilingsDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const WITH = { legacy: "rows" };
const GROUP = "id:filings-legacy";
const tab = (name: string) => `id:filings-tabs >> label:${name}`;
const count = (text: string) => `id:filings-count >> text:${text}`;

async function board(params: Record<string, unknown> = {}): Promise<App> {
  const app = await open(ENTRY, { ...WIDE, ...asking(params) });
  await app.waitFor("id:filings-board", { timeout: 20000 });
  return app;
}
async function list(params: Record<string, unknown> = {}): Promise<App> {
  const app = await open(ENTRY, asking(params));
  await app.waitFor("id:filings-tabs", { timeout: 20000 });
  return app;
}

/** The titles of the demo taxpayer's lines, top to bottom: the Cebu branch's July, June three times (amendment, e-mailed copy, original), and the older ones. */
const TITLES = ["1601C Jul 2025", "1601C Jun 2025", "1601C Jun 2025", "1601C Jun 2025", "2550Q Q1 2025", "1701 TY 2024", "9999Xv2000 TY 2023"];
const titlesShown = (app: App) =>
  app.texts().filter((t) => /^(1601C Jun 2025|2550Q Q1 2025|1701 TY 2024|9999Xv2000 TY 2023|1601C Jul 2025)$/.test(t));

test("the store lists the imported returns of a branch it may see, once, and none of another taxpayer's", () => {
  const demo = createDemoTaxpayer();
  const row: LegacyFiledReturn = {
    id: 1, form: "1601Cv2018", form_code: "1601C", period: "062025", amendment_version: null, email_copy: false,
    source_file: "demo.xml", imported_at: "2026-11-14 09:30:00",
  };
  let asked = 0;
  const host = bir as unknown as Record<string, unknown>;
  const real = host.list_legacy_filed_returns;
  host.list_legacy_filed_returns = (tin: string) => {
    asked += 1;
    return tin === demo.branches[0].tin ? [row] : [];
  };
  try {
  const tin = demo.branches[0].tin;
  expect(store.legacyFiled(tin)).toEqual([row]);
  expect(store.legacyFiled(tin)).toEqual([row]);
  expect(asked).toBe(1);
  // An import is a write: the list is read again.
  store.bump("legacy_import");
  expect(store.legacyFiled(tin)).toEqual([row]);
  expect(asked).toBe(2);
  // A taxpayer's session sees its own and nothing of another's, and the host is not asked.
  store.setScope("000111229");
  expect(store.legacyFiled(tin)).toEqual([]);
  expect(asked).toBe(2);
  store.setScope(demo.tin_root);
  expect(store.legacyFiled(tin)).toEqual([row]);
  store.setScope(null);
  expect(store.legacyFiled(null)).toEqual([]);
  expect(store.legacyFiled(undefined)).toEqual([]);
  } finally {
    host.list_legacy_filed_returns = real;
    store.setScope(null);
  }
});

test("desktop: the group lists the taxpayer in view, under the board, newest period first", async () => {
  const app = await board(WITH);
  await app.waitFor(GROUP);
  const texts = app.texts();
  expect(texts).toContain("Filed in BIR eBIRForms");
  // Six of the head office and the Cebu branch's one, all the same taxpayer's; the second taxpayer's line is not here.
  expect(app.shows("id:filings-legacy-count >> text:7")).toBe(true);
  expect(texts).toContain("Imported from the old program for Juana Santos. Records only: they cannot be opened or sent.");
  expect(app.count("id:filings-legacy-list >> id:legacy-1")).toBe(1);
  expect(app.shows("id:legacy-8")).toBe(false);
  // Newest period first; of June 2025 the amendment, then the e-mailed copy and the original (which came in together).
  expect(titlesShown(app)).toEqual(TITLES);
  expect(app.count("id:legacy-2 >> text:1601C Jun 2025")).toBe(1);
  // An amendment says its number, the e-mailed copy says so; the original says nothing more.
  expect(app.shows("id:legacy-2-amended >> text:Amended (v2)")).toBe(true);
  expect(app.shows("id:legacy-3-email >> text:E-mailed copy")).toBe(true);
  expect(app.shows("id:legacy-1-amended")).toBe(false);
  expect(app.shows("id:legacy-1-email")).toBe(false);
  expect(app.count("text:Amended (v2)")).toBe(1);
  expect(app.count("text:E-mailed copy")).toBe(1);
  // The import date of each, and the branch where it is not the head office.
  // (Five came in on Nov 14 and two on Nov 13; the fixture's times are midday UTC, so the day is the same in every time zone.)
  expect(app.count("text:Imported Nov 14, 2026")).toBe(5);
  expect(app.count("text:Imported Nov 13, 2026")).toBe(2);
  expect(app.shows("id:legacy-1-imported")).toBe(true);
  expect(app.count("id:filings-legacy-list >> text:Cebu branch")).toBe(1);
  // A form this app has no code for is named as the old program named it.
  expect(texts).toContain("9999Xv2000 TY 2023");
});

test("desktop: the group is read-only: nothing in it can be pressed, and the board counts only this app's returns", async () => {
  const app = await board(WITH);
  await app.waitFor(GROUP);
  expect(app.count("id:filings-legacy >> role:button")).toBe(0);
  expect(app.shows("id:filings-legacy >> text:Cancel queue")).toBe(false);
  expect(app.shows("id:filings-legacy >> text:Open")).toBe(false);
  // The seven returns of the database are the board's; the imported ones are not among them.
  await app.waitFor(count("7 returns"));
  for (const n of [IDS.draft, IDS.queued, IDS.submitted, IDS.confirmed, IDS.paid, IDS.cebu, IDS.second]) expect(app.shows(`id:return-${n}`)).toBe(true);
  expect(app.count("id:filings-board >> text:1601C Jun 2025")).toBe(0);
});

test("desktop: without an import there is no group, no title and no empty box (the host's own answer: none)", async () => {
  const app = await board();
  await app.waitFor(count("7 returns"));
  expect(app.shows(GROUP)).toBe(false);
  expect(app.shows("id:filings-legacy-list")).toBe(false);
  expect(app.texts()).not.toContain("Filed in BIR eBIRForms");
  // With no return at all the page says so, and still draws no group.
  const none = await open(ENTRY, { ...WIDE, ...asking({ seed: "none" }) });
  await none.waitFor("id:filings-empty", { timeout: 20000 });
  expect(none.shows(GROUP)).toBe(false);
  expect(none.texts()).not.toContain("Filed in BIR eBIRForms");
});

test("desktop: a taxpayer in view with nothing imported draws nothing, while another taxpayer has lines", async () => {
  const app = await board({ legacy: "second" });
  await app.waitFor(count("7 returns"));
  expect(app.shows(GROUP)).toBe(false);
  expect(app.texts()).not.toContain("Filed in BIR eBIRForms");
});

test("desktop: the group is the taxpayer in view's: another taxpayer shows their own lines", async () => {
  const app = await board({ ...WITH, start: "view=filings;tp=000111229" });
  await app.waitFor(GROUP);
  expect(app.shows("id:filings-legacy-count >> text:1")).toBe(true);
  expect(app.texts()).toContain("2551Q Q3 2025");
  expect(app.texts()).toContain("Imported from the old program for Sample Trading Corp. Records only: they cannot be opened or sent.");
  expect(app.shows("id:legacy-8")).toBe(true);
  expect(app.shows("id:legacy-1")).toBe(false);
});

test("desktop: a search or a filter speaks of this app's returns, so the group steps aside until it is cleared", async () => {
  const app = await board(WITH);
  await app.waitFor(GROUP);
  await app.type("id:filings-search", "cebu");
  await app.waitFor(count("1 return"));
  expect(app.shows(GROUP)).toBe(false);
  await app.press("id:filings-clear");
  await app.waitFor(GROUP);
  expect(app.shows(count("7 returns"))).toBe(true);
});

test("phone: the group sits under the list on All, in the narrow layout, and not under another tab", async () => {
  const app = await list(WITH);
  await app.waitFor(GROUP);
  expect(app.shows("id:tab-bar")).toBe(true);
  expect(app.texts()).toContain("Filed in BIR eBIRForms");
  expect(titlesShown(app)).toEqual(TITLES);
  // Narrow: the date sits under the title; every line of the group has one.
  for (const n of [1, 2, 3, 4, 5, 6, 7]) expect(app.shows(`id:legacy-${n}-imported`)).toBe(true);
  expect(app.shows("id:legacy-2-amended >> text:Amended (v2)")).toBe(true);
  expect(app.shows("id:legacy-3-email >> text:E-mailed copy")).toBe(true);
  expect(app.count("id:filings-legacy >> role:button")).toBe(0);
  // The list above it is still this app's seven returns.
  await app.waitFor(count("7 returns"));
  // A status tab is this app's statuses: the group is not under it.
  await app.press(tab("Draft"));
  await app.waitFor(count("3 returns"));
  expect(app.shows(GROUP)).toBe(false);
  await app.press(tab("All"));
  await app.waitFor(GROUP);
});

test("phone: without an import there is no group", async () => {
  const app = await list();
  await app.waitFor(count("7 returns"));
  expect(app.shows(GROUP)).toBe(false);
  expect(app.texts()).not.toContain("Filed in BIR eBIRForms");
});

test("phone: a search steps the group aside and Clear filters brings it back", async () => {
  const app = await list(WITH);
  await app.waitFor(GROUP);
  await app.type("id:filings-search", "sample");
  await app.waitFor(count("1 return"));
  expect(app.shows(GROUP)).toBe(false);
  await app.press("id:filings-clear");
  await app.waitFor(GROUP);
});
