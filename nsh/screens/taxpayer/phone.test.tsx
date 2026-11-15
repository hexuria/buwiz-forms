// The taxpayer screens at phone size (390 x 844): the tabs stay, sections stack full width, an edit opens in a sheet
// with Save at its foot, the Forms list drops its columns, and the New taxpayer and New branch forms fill the width.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const YEAR = new Date().getFullYear();
const phone = (start: string, params: Record<string, unknown> = {}) => open(ENTRY, { host: { params: { start, ...params } } });

test("phone: the profile's tabs stay, and editing a section opens a sheet that saves", async () => {
  const app = await phone("view=profile");
  await app.waitFor("id:tab-bar");
  await app.waitFor("id:tp-tabs");
  expect(app.shows("id:sidebar")).toBe(false);
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor("id:tp-edit-sheet-body");
  expect(app.shows("id:tp-taxpayer-save")).toBe(true);
  await app.clear("id:tp-taxpayer-name");
  await app.type("id:tp-taxpayer-name", "Juana Santos Store");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:Changes saved");
  await app.waitFor("id:tp-edit-sheet-body", { state: "gone" });
  expect(app.texts()).toContain("Juana Santos Store");
  // Back closes the sheet without saving.
  await app.press("id:tp-sec-address-edit");
  await app.waitFor("id:tp-edit-sheet-body");
  await app.back();
  await app.waitFor("id:tp-edit-sheet-body", { state: "gone" });
});

test("phone: Forms keeps code, name and template, and edits the list", async () => {
  const app = await phone("view=forms");
  await app.waitFor("id:tp-form-1601C");
  expect(app.texts().some((t) => t.startsWith("Due "))).toBe(false); // no due column when narrow
  await app.press("id:tp-forms-edit");
  await app.waitFor("id:tp-form-2551Q-check");
  await app.press("id:tp-form-2551Q-check");
  await app.press("id:tp-forms-save");
  await app.waitFor(`text:${YEAR} forms updated: 0 added, 1 removed.`);
  await app.press("label:Templates");
  await app.waitFor("id:tp-template-1601C");
});

test("phone: New taxpayer and New branch fill the width and create", async () => {
  const app = await phone("view=new_branch");
  await app.waitFor("id:new-branch-card");
  await app.type("id:new-branch-name", "Cebu branch");
  await app.press("id:new-branch-save");
  await app.waitFor("text:Cebu branch added.");
  const tp = await phone("view=new_taxpayer");
  await tp.waitFor("id:new-tp-card");
  await tp.type("id:new-tp-tin", "000111245");
  await tp.type("id:new-tp-name", "Sample Owner");
  await tp.press("id:new-tp-rdo_code-trigger");
  await tp.waitFor("id:new-tp-rdo_code-search"); // the pick opens as a sheet when narrow
  await tp.type("id:new-tp-rdo_code-search", "050");
  await tp.press("id:new-tp-rdo_code-opt-050");
  await tp.press("id:new-tp-save");
  await tp.waitFor("text:Sample Owner added. Add a tax year to choose its forms.");
});
