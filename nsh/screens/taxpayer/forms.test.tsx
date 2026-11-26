// The Forms and Templates tabs at desktop size: the year's list, Edit forms with search, frequency and setups, the
// replace question, Add year and its bounds, the annual-return warning, and templates.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const YEAR = new Date().getFullYear();
const desk = (params: Record<string, unknown> = {}) => open(ENTRY, { ...WIDE, host: { params: { start: "view=forms", ...params } } });

test("desktop: the year's forms show their next period and template; Edit forms searches, filters and saves", async () => {
  const app = await desk();
  await app.waitFor("id:tp-form-1601C");
  expect(app.shows("id:tp-form-2551Q")).toBe(true);
  expect(app.shows("id:tp-form-1601C-check")).toBe(false);
  expect(app.texts()).toContain("No template");
  expect(app.texts().some((t) => t.startsWith("Next: "))).toBe(true);
  expect(app.texts().some((t) => t.startsWith("Due "))).toBe(true);

  await app.press("id:tp-forms-edit");
  await app.waitFor("id:tp-forms-search");
  expect(app.texts()).toContain("2 selected");
  await app.type("id:tp-forms-search", "vat");
  await app.waitUntil(() => !app.shows("id:tp-form-1601C"));
  expect(app.shows("id:tp-form-2550Q")).toBe(true);
  await app.clear("id:tp-forms-search");
  await app.type("id:tp-forms-search", "zzzz");
  await app.waitFor("id:tp-forms-nomatch");
  await app.clear("id:tp-forms-search");
  await app.press("id:tp-forms-freq >> label:Monthly");
  await app.waitFor("id:tp-form-1601C");
  expect(app.shows("id:tp-form-2551Q")).toBe(false);
  await app.press("id:tp-forms-freq >> label:All");
  await app.waitFor("id:tp-form-2551Q");
  await app.press("id:tp-form-2551Q");
  expect(app.texts()).toContain("1 selected");
  expect(app.texts()).toContain("Not filed this year");
  await app.press("id:tp-forms-save");
  await app.waitFor(`text:${YEAR} forms updated: 0 added, 1 removed.`);
  await app.waitFor("id:tp-form-2551Q", { state: "gone" });
  expect(app.shows("id:tp-form-1601C")).toBe(true);

  // Cancel keeps the list as it was.
  await app.press("id:tp-forms-edit");
  await app.press("id:tp-form-1601C");
  await app.press("id:tp-forms-cancel");
  expect(app.shows("id:tp-form-1601C")).toBe(true);
  expect(app.shows("id:tp-forms-search")).toBe(false);
});

test("desktop: a dry run leaves its periods due (Next: ...); real sends file every period", async () => {
  const dry = await desk({ sent: "dry" });
  await dry.waitFor("id:tp-form-1601C");
  expect(dry.shows("id:tp-form-1601C >> text:/^Next: /")).toBe(true);
  expect(dry.shows("id:tp-form-1601C >> text:Every period filed")).toBe(false);
  const real = await desk({ sent: "real" });
  await real.waitFor("id:tp-form-1601C");
  expect(real.shows("id:tp-form-1601C >> text:Every period filed")).toBe(true);
  expect(real.shows("id:tp-form-1601C >> text:/^Next: /")).toBe(false);
});

test("desktop: a setup sets the year's forms, asks before replacing, and one of yours can be deleted", async () => {
  const app = await desk({ setup: true });
  await app.waitFor("id:tp-forms-edit");
  await app.press("id:tp-forms-edit");
  await app.waitFor("id:tp-setups");
  expect(app.texts()).toContain("Bookkeeping clients");
  await app.press("text:Non-VAT professional");
  await app.waitFor(`text:/form(s)? for ${YEAR}\\./`);
  expect(app.shows("id:tp-forms-search")).toBe(false); // the list is saved; the edit closed
  if (app.shows("id:tp-replace")) {
    expect(app.texts().some((t) => t.endsWith("Replace what is set?"))).toBe(true);
    await app.press("id:tp-replace-yes");
    await app.waitFor("text:/^Replaced with /");
    expect(app.shows("id:tp-replace")).toBe(false);
  }
  await app.press("id:tp-forms-edit");
  await app.waitFor("label:Delete the setup Bookkeeping clients");
  await app.press("label:Delete the setup Bookkeeping clients");
  await app.waitFor("text:Setup Bookkeeping clients deleted.");
});

test("desktop: Add year offers the next year, copies the forms, and the bounds stop it", async () => {
  const app = await desk();
  await app.waitFor("id:tp-add-year-open");
  await app.press("id:tp-add-year-open");
  await app.waitFor("id:tp-add-year");
  expect(app.texts()).toContain(`Add ${YEAR + 1}`);
  expect(app.texts()).toContain(`Copy 2 forms from ${YEAR}`);
  await app.press("id:tp-add-year-add");
  await app.waitFor(`text:${YEAR + 1} added with 2 forms from ${YEAR}.`);
  await app.waitFor("id:tp-form-1601C");
  expect(app.shows("id:tp-add-year")).toBe(false);
  // The year segments switch the list.
  await app.press(`id:tp-years >> label:${YEAR}`);
  await app.waitFor("id:tp-form-2551Q");
  // An earlier year without copying.
  await app.press("id:tp-add-year-open");
  expect(app.texts()).toContain(`Add ${YEAR - 1}`); // the year after the latest is past the bound: the newest allowed
  await app.press("id:tp-add-year-select-trigger");
  await app.waitFor(`text:${YEAR - 2}`);
  await app.press(`text:${YEAR - 2}`);
  await app.waitFor(`text:Add ${YEAR - 2}`);
  await app.press("id:tp-add-year-copy");
  await app.press("id:tp-add-year-add");
  await app.waitFor(`text:${YEAR - 2} added. Choose the forms it files.`);
  await app.waitFor("id:tp-forms-empty");
  await app.press("id:tp-forms-choose");
  await app.waitFor("id:tp-forms-search");
});

test("desktop: two annual income tax returns of one kind get a warning, kept on the list", async () => {
  const app = await desk({ conflict: true });
  await app.waitFor("id:tp-annual-conflict-1701-1701A");
  expect(app.shows("id:tp-form-1701")).toBe(true);
  expect(app.shows("id:tp-form-1701A")).toBe(true);
});

test("desktop: templates list the year's forms; Create needs a saved return first", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { start: "view=templates" } } });
  await app.waitFor("id:tp-template-1601C");
  expect(app.texts()).toContain("No template yet");
  await app.press("id:tp-template-1601C-create");
  await app.waitFor("text:Open and save a 1601C return first. The template starts from its answers.");
  expect(app.shows("id:tp-template-1601C-delete")).toBe(false);
});

test("desktop: with no tax year Forms shows the band and Templates offers Add year", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { start: "view=templates", years: false } } });
  await app.waitFor("id:tp-templates-noyear");
  await app.press("id:tp-templates-add-year");
  await app.waitFor("id:tp-add-year");
  expect(app.shows("id:tp-add-year-copy")).toBe(false);
  expect(app.shows("id:tp-add-year-cancel")).toBe(false);
  await app.press("id:tp-add-year-add");
  await app.waitFor(`text:${YEAR} added. Choose the forms it files.`);
  await app.waitFor("id:tp-forms-empty");
});

test("desktop: a tap on the row's text, code or empty part selects it; one tap toggles once", async () => {
  const app = await desk();
  await app.waitFor("id:tp-form-1601C");
  await app.press("id:tp-forms-edit");
  await app.waitFor("id:tp-forms-search");
  expect(app.texts()).toContain("2 selected");

  // A tap on the row's text selects it (the press lands on the one control the row is): one off.
  await app.press("text:2551Q");
  expect(app.texts()).toContain("1 selected");
  // A second tap selects it again: each tap moved the count exactly once.
  await app.press("text:2551Q");
  expect(app.texts()).toContain("2 selected");
  // The control pressed by its row id (the checkbox end) toggles exactly once as well.
  await app.press("id:tp-form-2551Q");
  expect(app.texts()).toContain("1 selected");
});

test("phone: a tap on the row's text selects it at narrow width", async () => {
  const app = await open(ENTRY, { host: { params: { start: "view=forms" } } });
  await app.waitFor("id:tp-form-1601C");
  await app.press("id:tp-forms-edit");
  await app.waitFor("id:tp-forms-search");
  expect(app.texts()).toContain("2 selected");
  await app.press("text:2551Q");
  expect(app.texts()).toContain("1 selected");
  await app.press("text:2551Q");
  expect(app.texts()).toContain("2 selected");
});
