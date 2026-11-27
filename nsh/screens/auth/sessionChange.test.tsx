// F7-4: state left behind when the session changes. In help-desk mode one taxpayer locks the app and the next one signs in:
// the filters, searches, picks and an open edit of the last person are not carried over (the old app cleared its screen state
// on lock, log out and sign-in: security.leave(), taxpayerUi.reset(), the *Ui controllers rebuilt). Each screen with state of
// its own at module level registers its reset with app.onSessionChange (shell/app.ts); this goes through the real screens.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const A = { tin: "000123453", pin: "4321", name: "Juana Santos" };
const B = { tin: "000111225", pin: "5555", name: "Sample Trading Corp" };
const PARAMS = { demo: true, second: true, years: true, mode: "helpdesk", adminPin: "1234", pins: { [A.tin]: A.pin, [B.tin]: B.pin }, readout: true };

type App = Awaited<ReturnType<typeof open>>;

async function signIn(app: App, tin: string, pin: string) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", tin);
  await app.type("id:auth-tp-pin", pin);
  await app.waitFor("id:frame");
}

/** Nothing of the first taxpayer shows: not her name, not her TIN, in any form. */
function nothingOfA(app: App) {
  expect(app.texts().filter((t) => /Juana|000-123-453|000123453/.test(t))).toEqual([]);
}

test("desktop: lock, then another taxpayer signs in: filters, searches and an open edit are back to their defaults", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: PARAMS } });
  await signIn(app, A.tin, A.pin);
  await app.waitFor("text:session: taxpayer " + A.tin);

  // Filings: a taxpayer filter (her name in the chip) and a search.
  await app.press("id:nav-filings");
  await app.waitFor("id:filings-search");
  await app.press("id:filings-taxpayers-trigger");
  await app.waitFor("id:filings-taxpayers-opt-" + A.tin);
  await app.press("id:filings-taxpayers-clear");
  await app.press("id:filings-taxpayers-opt-" + A.tin);
  await app.type("id:filings-search", "juana");
  await app.waitFor("id:filings-clear");
  // The Form library's search.
  await app.press("id:nav-library");
  await app.waitFor("id:library-search");
  await app.type("id:library-search", "juana");
  // Settings' search.
  await app.keyboard.press("mod+,");
  await app.waitFor("id:settings-search");
  await app.type("id:settings-search", "juana");
  // A tax profile edit with typed values.
  await app.press("id:context-pill");
  await app.press("id:panel-open-profile");
  await app.waitFor("id:tp-sec-taxpayer-edit");
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor("id:tp-taxpayer-name");
  await app.clear("id:tp-taxpayer-name");
  await app.type("id:tp-taxpayer-name", "Juana Santos Trading");
  expect(app.value("id:tp-taxpayer-name")).toBe("Juana Santos Trading");

  // Lock, and the other taxpayer signs in.
  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  await signIn(app, B.tin, B.pin);
  await app.waitFor("text:session: taxpayer " + B.tin);

  await app.press("id:nav-filings");
  await app.waitFor("id:filings-search");
  expect(app.value("id:filings-search")).toBe("");
  expect(app.shows("id:filings-clear")).toBe(false);
  await app.press("id:nav-library");
  await app.waitFor("id:library-search");
  expect(app.value("id:library-search")).toBe("");
  await app.keyboard.press("mod+,");
  await app.waitFor("id:settings-search");
  expect(app.value("id:settings-search")).toBe("");
  await app.press("id:context-pill");
  await app.press("id:panel-open-profile");
  await app.waitFor("id:tp-sec-taxpayer-edit");
  expect(app.shows("id:tp-taxpayer-name")).toBe(false);
  expect(app.texts()).toContain(B.name);
  nothingOfA(app);
});

test("phone: a log out and another taxpayer: the Filings filters and the Form library search start from the defaults", async () => {
  const app = await open(ENTRY, { host: { params: PARAMS } });
  await signIn(app, A.tin, A.pin);
  await app.waitFor("text:session: taxpayer " + A.tin);
  await app.press("id:tab-filings");
  await app.waitFor("id:filings-tabs");
  await app.press("id:filings-tabs >> label:Draft");
  await app.type("id:filings-search", "juana");
  await app.press("id:tab-library");
  await app.waitFor("id:library-search");
  await app.type("id:library-search", "juana");
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-logout");
  await app.press("id:nb-menu-logout");
  await signIn(app, B.tin, B.pin);
  await app.waitFor("text:session: taxpayer " + B.tin);
  await app.press("id:tab-library");
  await app.waitFor("id:library-search");
  expect(app.value("id:library-search")).toBe("");
  await app.press("id:tab-filings");
  await app.waitFor("id:filings-tabs");
  expect(app.value("id:filings-search")).toBe("");
  nothingOfA(app);
});
