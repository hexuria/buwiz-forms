// The Tax profile at desktop size (1440 x 900, macOS): header, tabs, the year's seven sections and their edits, archive
// and delete. Against a scratch database with the demo taxpayer (TaxpayerDemo.tsx; variations through open's host params).
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const YEAR = new Date().getFullYear();
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { start: "view=profile", ...params } } });
const desk = (params: Record<string, unknown> = {}) => open(ENTRY, { ...WIDE, ...asking(params) });

test("desktop: the header names the taxpayer, copies the TIN, and the tabs count the year's forms", async () => {
  const app = await desk();
  await app.waitFor("id:tp-page-profile");
  expect(app.texts()).toContain("Juana Santos");
  expect(app.texts()).toContain("000-123-453-00000");
  await app.press("id:tp-copy-tin");
  await app.waitFor("text:Copied 000-123-453-00000");
  expect(app.clipboard()).toBe("000-123-453-00000");
  for (const t of ["Taxpayer", "Registered activity", `Tax types · your forms for ${YEAR}`, "Facts your forms use", "Address and contact", "Confirmation inbox", "Agent filing"]) {
    expect(app.texts()).toContain(t);
  }
  expect(app.texts()).toContain("2 forms");
  expect(app.shows("id:tp-taxtypes-form-1601C")).toBe(true);
  await app.press("label:Forms");
  await app.waitFor("id:tp-page-forms");
  await app.press("label:Templates");
  await app.waitFor("id:tp-page-templates");
  await app.press("label:Profile");
  await app.waitFor("id:tp-page-profile");
  await app.press("id:tp-new-return");
  await app.waitFor("id:new-return");
});

test("desktop: editing the taxpayer saves the name, shows a bad date once left, and moves the TIN", async () => {
  const app = await desk();
  await app.waitFor("id:tp-sec-taxpayer-edit");
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor("id:tp-taxpayer-name");
  expect(app.shows("id:tp-sec-taxpayer-edit")).toBe(false);
  await app.clear("id:tp-taxpayer-name");
  await app.type("id:tp-taxpayer-name", "Juana Santos Trading");
  await app.type("id:tp-taxpayer-birth_date", "1990-13-40");
  expect(app.texts()).not.toContain("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(app.shows("id:tp-taxpayer-name")).toBe(true); // still editing
  await app.clear("id:tp-taxpayer-birth_date");
  await app.type("id:tp-taxpayer-birth_date", "1990-05-17");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:Changes saved");
  await app.waitFor("id:tp-taxpayer-name", { state: "gone" });
  expect(app.texts()).toContain("Juana Santos Trading");
  expect(app.texts()).toContain("May 17, 1990");

  // A short TIN is refused; nine digits move every branch, year, return and template.
  await app.press("id:tp-sec-taxpayer-edit");
  await app.clear("id:tp-taxpayer-tin");
  await app.type("id:tp-taxpayer-tin", "0001");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:TIN is 9 digits, like 123-456-789.");
  await app.clear("id:tp-taxpayer-tin");
  await app.type("id:tp-taxpayer-tin", "000123461");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:/^TIN changed to 000-123-461/");
  expect(app.texts()).toContain("000-123-461-00000");
  // Cancel drops what was typed.
  await app.press("id:tp-sec-taxpayer-edit");
  await app.clear("id:tp-taxpayer-name");
  await app.type("id:tp-taxpayer-name", "Nobody");
  await app.press("id:tp-taxpayer-cancel");
  expect(app.texts()).not.toContain("Nobody");
});

test("desktop: a branch follows the head office until switched off, and its code moves", async () => {
  const app = await desk({ branch: true, start: "branch=00001;view=profile" });
  await app.waitFor("id:tp-sec-branch-edit");
  expect(app.texts()).toContain("Branch 00001"); // the branch has no tax year of its own yet
  expect(app.texts()).toContain("4 of 4 from head office");
  await app.press("id:tp-sec-address-edit");
  await app.waitFor("id:tp-inherit-phone");
  expect(app.enabled("id:tp-address-phone")).toBe(false);
  await app.press("id:tp-inherit-email");
  await app.clear("id:tp-address-email");
  await app.type("id:tp-address-email", "cebu-at-example");
  expect(app.texts()).not.toContain("Type an email like name@example.com.");
  await app.press("id:tp-inherit-phone");
  expect(app.enabled("id:tp-address-phone")).toBe(true);
  await app.clear("id:tp-address-phone"); // leaving the email box shows its error
  await app.type("id:tp-address-phone", "0322000000");
  await app.waitFor("text:Type an email like name@example.com.");
  await app.clear("id:tp-address-email");
  await app.type("id:tp-address-email", "cebu@example.com");
  await app.press("id:tp-address-save");
  await app.waitFor("text:Changes saved");
  await app.waitFor("text:2 of 4 from head office");
  expect(app.texts()).toContain("0322000000");

  // Registered activity: an own RDO from the list, and a new branch code.
  await app.press("id:tp-sec-branch-edit");
  await app.waitFor("id:tp-branch-code");
  await app.press("id:tp-inherit-rdo_code");
  await app.press("id:tp-branch-rdo_code-trigger");
  // The combobox's box holds the inherited choice's words: empty it before searching (a real window selects them, so typing replaces).
  await app.clear("id:tp-branch-rdo_code-search");
  await app.type("id:tp-branch-rdo_code-search", "cebu city");
  await app.waitFor("id:tp-branch-rdo_code-opt-081");
  await app.press("id:tp-branch-rdo_code-opt-081");
  await app.clear("id:tp-branch-code");
  await app.type("id:tp-branch-code", "00002");
  await app.press("id:tp-branch-save");
  await app.waitFor("text:Changes saved");
  expect(app.texts()).toContain("000-123-453-00002");
  expect(app.texts()).toContain("081");
});

test("desktop: a fact row sets the year's EOPT tier, and Save as a setup asks for a name", async () => {
  const app = await desk();
  await app.waitFor("id:tp-fact-eopt_tier");
  await app.press("id:tp-fact-eopt_tier-edit");
  await app.waitFor("id:tp-fact-eopt_tier-save");
  await app.press("id:tp-fact-eopt_tier-save");
  await app.waitFor("text:Choose one first.");
  await app.press("id:tp-fact-eopt_tier-choice >> label:Small");
  await app.press("id:tp-fact-eopt_tier-save");
  await app.waitFor(`text:EOPT tier saved for ${YEAR}.`);
  expect(app.texts()).toContain("Small");

  await app.press("id:tp-save-setup-open");
  await app.press("id:tp-save-setup-save");
  await app.waitFor("text:Type a name for the setup.");
  await app.type("id:tp-save-setup-name", "Bookkeeping clients");
  await app.press("id:tp-save-setup-save");
  await app.waitFor("text:Saved as the setup Bookkeeping clients. Pick it when you add a taxpayer.");
  expect(app.shows("id:tp-save-setup-name")).toBe(false);
});

test("desktop: agent filing grants and revokes, the inbox keeps credentials, and its network calls are wired", async () => {
  const app = await desk({ google: true });
  await app.waitFor("id:tp-sec-agent-toggle");
  expect(app.texts()).toContain("Assistant must ask before filing");
  await app.press("id:tp-sec-agent-toggle");
  await app.waitFor("id:tp-agent-1601C");
  await app.press("id:tp-agent-1601C");
  await app.waitFor("text:Assistant may file 1601C.");
  await app.waitUntil(() => !app.texts().includes("Assistant must ask before filing"));
  await app.press("id:tp-agent-1601C");
  await app.waitFor("text:Assistant may no longer file 1601C.");

  await app.waitFor("id:tp-inbox-edit");
  expect(app.shows("id:tp-inbox-device-note")).toBe(false);
  expect(app.shows("id:tp-inbox-google-unavailable")).toBe(false);
  await app.press("id:tp-inbox-edit");
  await app.clear("id:tp-inbox-email");
  await app.type("id:tp-inbox-email", "not-an-email");
  await app.press("id:tp-inbox-save");
  await app.waitFor("text:Type an email like name@example.com.");
  await app.clear("id:tp-inbox-email");
  await app.type("id:tp-inbox-email", "juana@example.com");
  await app.type("id:tp-inbox-password", "abcdabcdabcdabcd");
  await app.press("id:tp-inbox-save");
  await app.waitFor("text:Inbox credentials saved. Verify the connection to start tracking.");
  expect(app.texts()).toContain("Stored");
  // Verify, Check now and Connect reach a mail server or a browser: wired and ready, not pressed here.
  expect(app.enabled("id:tp-inbox-verify")).toBe(true);
  expect(app.enabled("id:tp-inbox-check")).toBe(true);
  await app.press("id:tp-inbox-method >> label:Google account (OAuth2)");
  await app.waitFor("id:tp-inbox-connect");
  expect(app.enabled("id:tp-inbox-connect")).toBe(true);
  expect(app.texts()).toContain("Not connected");
  await app.press("id:tp-inbox-cancel");
  await app.press("id:tp-inbox-tracking");
  await app.waitFor("text:/^Automated receipt tracking (on|off)\\.$/");
});

test("desktop: a build with no Google client id offers the inbox's app password only, and says so", async () => {
  const app = await desk({ google: false });
  await app.waitFor("id:tp-inbox-edit");
  expect(app.shows("id:tp-inbox-method")).toBe(false);
  expect(app.shows("id:tp-inbox-connect")).toBe(false);
  expect(app.texts()).not.toContain("Google account (OAuth2)");
  expect(app.texts()).toContain("Google sign-in is not available in this version. Use an app password.");
  expect(app.shows("id:tp-inbox-device-note")).toBe(false);
});

test("desktop: archive, unarchive, and delete after a second press", async () => {
  const app = await desk();
  await app.waitFor("id:tp-archive-btn");
  await app.press("id:tp-archive-btn");
  await app.waitFor("text:Juana Santos archived.");
  await app.waitFor("text:Unarchive");
  await app.press("id:tp-archive-btn");
  await app.waitFor("text:Juana Santos is back in new work.");
  await app.press("id:tp-delete-btn");
  await app.waitFor("text:Every branch and return goes. This cannot be undone");
  await app.press("id:tp-delete-cancel");
  await app.waitFor("text:Removes it and every branch and return");
  await app.press("id:tp-delete-btn");
  await app.press("id:tp-delete-btn");
  await app.waitFor("text:Juana Santos deleted.");
  await app.waitFor("id:tp-page-profile", { state: "gone" });
});

test("desktop: with no tax year the sections say so and Add year opens the band on Forms", async () => {
  const app = await desk({ years: false });
  await app.waitFor("id:tp-page-profile");
  expect(app.texts()).toContain("Add a tax year first");
  expect(app.texts()).toContain("Add a tax year to record the line of business and the start date.");
  expect(app.shows("id:tp-profile-years")).toBe(false);
  await app.press("id:tp-taxtypes-add-year");
  await app.waitFor("id:tp-add-year");
  await app.press("id:tp-add-year-add");
  await app.waitFor(`text:${YEAR} added. Choose the forms it files.`);
  await app.waitFor("id:tp-forms-empty");
});
