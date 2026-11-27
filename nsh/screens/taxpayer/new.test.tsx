// New taxpayer and New branch at desktop size: the TIN, name and type, "Start from" a setup or a copy, the RDO and ZIP
// pickers, errors once a field was left or after Create, the host's refusal; a branch with the next free code.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const YEAR = new Date().getFullYear();
const desk = (params: Record<string, unknown> = {}) => open(ENTRY, { ...WIDE, host: { params: { start: "view=new_taxpayer", ...params } } });

test("desktop: New taxpayer says what is missing on Create, a TIN in use at once, and starts from a setup", async () => {
  const app = await desk();
  await app.waitFor("id:new-tp-card");
  await app.press("id:new-tp-save");
  await app.waitFor("text:TIN is 9 digits, like 123-456-789.");
  expect(app.texts()).toContain("Type the registered name.");
  await app.type("id:new-tp-tin", "000123453");
  await app.waitFor("text:TIN 000-123-453 is already Juana Santos.");
  await app.clear("id:new-tp-tin");
  await app.type("id:new-tp-tin", "000111240");
  await app.waitFor("text:TIN 000-123-453 is already Juana Santos.", { state: "gone" });
  await app.type("id:new-tp-name", "Sample Owner");
  await app.type("id:new-tp-email", "owner-at-example");
  await app.press("id:new-tp-save");
  await app.waitFor("text:Type an email like name@example.com.");
  await app.clear("id:new-tp-email");
  await app.type("id:new-tp-email", "owner@example.com");
  // ZIP and RDO search by words.
  await app.press("id:new-tp-rdo_code-trigger");
  await app.type("id:new-tp-rdo_code-search", "makati south");
  await app.waitFor("id:new-tp-rdo_code-opt-050");
  await app.press("id:new-tp-rdo_code-opt-050");
  await app.press("id:new-tp-zip_code-trigger");
  await app.type("id:new-tp-zip_code-search", "1226");
  await app.press("id:new-tp-zip_code-opt-1226");
  // Start from a built-in setup.
  await app.press("id:new-tp-start-trigger");
  await app.waitFor("text:Nothing yet: I choose the forms");
  await app.press("text:Non-VAT professional");
  await app.waitFor(`text:/^(Starts|Adds) ${YEAR} with /`);
  await app.press("id:new-tp-save");
  await app.waitFor("id:tp-page-forms");
  await app.waitFor(`text:/form(s)? for ${YEAR}\\./`);
  expect(app.texts()).toContain("Sample Owner");
  expect(app.texts()).toContain("000-111-240-00000");
});

test("desktop: New taxpayer copies a taxpayer of the same type, and a type change resets the choice", async () => {
  const app = await desk({ peer: true });
  await app.waitFor("id:new-tp-card");
  await app.type("id:new-tp-tin", "000111258");
  await app.type("id:new-tp-name", "Sample Copy");
  await app.press("id:new-tp-start-trigger");
  await app.press("text:Same as a taxpayer I have…");
  await app.waitFor("id:new-tp-from-trigger");
  expect(app.texts()).toContain(`Copies the forms, facts and templates of ${YEAR}. Never the name, TIN or address.`);
  await app.press("id:new-tp-save");
  await app.waitFor("text:Choose the taxpayer and branch to copy.");
  await app.press("id:new-tp-type-trigger");
  await app.press("text:Corporation");
  await app.waitFor("id:new-tp-from-trigger", { state: "gone" });
  expect(app.texts()).toContain("Add a tax year after, and choose its forms.");
  await app.press("id:new-tp-type-trigger");
  await app.press("text:Individual");
  await app.press("id:new-tp-start-trigger");
  await app.press("text:Same as a taxpayer I have…");
  await app.press("id:new-tp-from-trigger");
  await app.press("text:Sample Person");
  await app.press("id:new-tp-save");
  await app.waitFor("id:tp-form-1701Q");
  expect(app.texts()).toContain("Sample Copy");
});

test("desktop: New taxpayer on an empty database lands on Forms with the Add year band; Cancel goes back", async () => {
  const app = await desk({ demo: false, start: "view=home" });
  await app.waitFor("id:sidebar");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-new-taxpayer");
  await app.press("id:panel-new-taxpayer");
  await app.waitFor("id:new-tp-card");
  await app.press("id:new-tp-cancel");
  await app.waitFor("id:new-tp-card", { state: "gone" });
  await app.press("id:context-pill");
  await app.press("id:panel-new-taxpayer");
  await app.waitFor("id:new-tp-card");
  await app.type("id:new-tp-tin", "000-123-453");
  await app.type("id:new-tp-name", "Juana Santos");
  await app.press("id:new-tp-save");
  await app.waitFor("text:Juana Santos added. Add a tax year to choose its forms.");
  await app.waitFor("id:tp-add-year");
  expect(app.texts()).toContain(`Add ${YEAR}`);
});

test("desktop: Back after creating a taxpayer leaves the finished New taxpayer form behind", async () => {
  const app = await desk({ demo: false, start: "view=home" });
  await app.waitFor("id:sidebar");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-new-taxpayer");
  await app.press("id:panel-new-taxpayer");
  await app.waitFor("id:new-tp-card");
  await app.type("id:new-tp-tin", "000-123-453");
  await app.type("id:new-tp-name", "Juana Santos");
  await app.press("id:new-tp-save");
  await app.waitFor("id:tp-add-year");
  // The taxpayer exists now: Back goes to where the person was before the form, never to the form they finished.
  await app.press("id:tb-back");
  await app.waitFor("id:tp-add-year", { state: "gone" });
  expect(app.shows("id:new-tp-card")).toBe(false);
  // Forward returns to the taxpayer's Forms.
  await app.press("id:tb-forward");
  await app.waitFor("id:tp-add-year");
});

test("desktop: New branch suggests the next code, follows the head office, and opens the branch's profile", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { start: "view=new_branch" } } });
  await app.waitFor("id:new-branch-card");
  expect(app.value("id:new-branch-code")).toBe("00001");
  expect(app.enabled("id:new-branch-phone")).toBe(false);
  await app.press("id:new-branch-save");
  await app.waitFor("text:Type a branch name, like Cebu branch.");
  await app.clear("id:new-branch-code");
  await app.type("id:new-branch-code", "12");
  await app.type("id:new-branch-name", "Cebu branch");
  await app.press("id:new-branch-save");
  await app.waitFor("text:Branch code is 5 digits, like 00001.");
  await app.clear("id:new-branch-code");
  await app.type("id:new-branch-code", "00001");
  await app.press("id:new-branch-inherit-phone");
  await app.type("id:new-branch-phone", "0322000000");
  await app.press("id:new-branch-save");
  await app.waitFor("text:Cebu branch added.");
  await app.waitFor("id:tp-page-profile");
  expect(app.texts()).toContain("000-123-453-00001");
  expect(app.texts()).toContain("3 of 4 from head office");
  // The finished form is no stop in the history: Back leaves the branch's profile for what came before the form.
  await app.press("id:tb-back");
  await app.waitFor("id:tp-page-profile", { state: "gone" });
  expect(app.shows("id:new-branch-card")).toBe(false);
});

test("desktop: Export this profile asks for the PIN first, then calls the host's save dialog", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { card: "export", pin: true } } });
  await app.waitFor("id:tp-export-btn");
  expect(app.texts()).toContain("The archive leaves out credentials; the profile PIN gates saving it");
  await app.press("id:tp-export-btn");
  await app.waitFor("text:Type all 4 digits of the profile PIN first.");
  const plain = await open(ENTRY, { ...WIDE, host: { params: { card: "export" } } });
  await plain.waitFor("id:tp-export-btn");
  await plain.press("id:tp-export-btn");
  // Headless has no save panel: the host answers cancelled (quiet) or says why; the button is free again either way.
  await plain.waitUntil(() => plain.enabled("id:tp-export-btn"));
});
