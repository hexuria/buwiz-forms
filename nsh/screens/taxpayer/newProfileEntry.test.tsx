// "New tax profile" always there for the owner: one label on every entry (the panel, the picker, the menu, the page,
// Settings > Account, the Home empty state), always offered to the "Just me" owner and the help desk admin, and never
// to a help desk client beyond the first profile. Phone and desktop.
import { expect, open, test } from "nsh/test";

const SHELL = "../../ShellDemo.tsx";
const AUTH = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const HELPDESK = { demo: true, second: true, years: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" } };

async function signIn(app: Awaited<ReturnType<typeof open>>, tin: string, pin: string) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", tin);
  await app.type("id:auth-tp-pin", pin);
  await app.waitFor("id:frame");
}

test("desktop: the owner with profiles sees the same label in the panel, the menu and the picker, and it opens the page", async () => {
  const app = await open(SHELL, { ...WIDE, ...asking({ second: true }) });
  await app.waitFor("id:sidebar");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-profile");
  expect(app.shows("id:panel-new-tp")).toBe(true);
  expect(app.texts()).toContain("New tax profile");
  await app.press("id:panel-new-tp");
  await app.waitFor("id:new-tp");
  expect(app.texts()).toContain("New tax profile");
  await app.press("text:Cancel");
  await app.waitFor("id:home");
  // The picker's add row.
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  expect(app.texts()).toContain("New tax profile");
  await app.back();
  await app.waitFor("id:palette-input", { state: "gone" });
});

test("phone: the picker's add row and the Home empty state button, one label, opening the new-profile page", async () => {
  const app = await open(SHELL, asking()); // 390 x 844, iOS
  await app.waitFor("id:tab-bar");
  await app.press("id:nb-search");
  await app.waitFor("id:palette-input");
  expect(app.texts()).toContain("New tax profile");
  await app.back();
  await app.waitFor("id:palette-input", { state: "gone" });
  // The empty state: a Home with no tax profiles has one clear New tax profile button.
  const fresh = await open(SHELL, asking({ demo: false }));
  await fresh.waitFor("id:home-welcome");
  expect(fresh.shows("id:welcome-add-taxpayer")).toBe(true);
  await fresh.press("id:welcome-add-taxpayer");
  await fresh.waitFor("id:new-tp");
});

test("phone: a help desk client never sees the entry with profiles present; the admin always does", async () => {
  const client = await open(AUTH, { host: { params: HELPDESK } });
  await signIn(client, "000123453", "4321");
  await client.waitFor("id:tab-bar");
  await client.press("id:nb-search");
  await client.waitFor("id:palette-input");
  expect(client.texts()).not.toContain("New tax profile");
  await client.back();

  const admin = await open(AUTH, { host: { params: HELPDESK } });
  await signIn(admin, "000000000", "1234");
  await admin.waitFor("id:tab-bar");
  await admin.press("id:nb-search");
  await admin.waitFor("id:palette-input");
  expect(admin.texts()).toContain("New tax profile");
  await admin.back();
});

test("desktop: a help desk client has no add control in Settings, the panel or the menu; the admin keeps it everywhere", async () => {
  const client = await open(AUTH, { ...WIDE, host: { params: HELPDESK } });
  await signIn(client, "000123453", "4321");
  await client.waitFor("id:sidebar");
  await client.press("id:context-pill");
  await client.waitFor("id:panel-profile");
  expect(client.shows("id:panel-new-tp")).toBe(false);
  expect(client.shows("id:panel-new-taxpayer")).toBe(false);

  const admin = await open(AUTH, { ...WIDE, host: { params: HELPDESK } });
  await signIn(admin, "000000000", "1234");
  await admin.waitFor("id:sidebar");
  await admin.press("id:context-pill");
  await admin.waitFor("id:panel-profile");
  expect(admin.shows("id:panel-new-tp")).toBe(true);
  // Settings > Account: the add control, same label.
  await admin.press("text:New tax profile");
  await admin.waitFor("id:new-tp");
});
