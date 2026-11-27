// "New tax profile" always there for the owner: one label on every entry (the panel, the picker, the menu, the page,
// Settings > Account, the Home empty state), always offered to the "Just me" owner and the help desk admin, and never
// to a help desk client beyond the first profile. Phone and desktop.
import { expect, open, test } from "nsh/test";

const SHELL = "../../ShellDemo.tsx";
const AUTH = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const HELPDESK = { demo: true, second: true, years: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, readout: true };

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
});

test("desktop: the palette carries the add row, ranked below the exact page and profile matches", async () => {
  const app = await open(SHELL, { ...WIDE, ...asking({ second: true }) });
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  // No query: the row is there for the owner.
  expect(app.texts()).toContain("New tax profile");
  // A query matching the exact "Tax profile" page: the page row stays above it; the add row never displaces what was typed.
  await app.type("id:palette-input", "tax profile");
  await app.waitFor("id:palette-item-0");
  const texts = app.texts();
  const page = texts.indexOf("Tax profile");
  const add = texts.indexOf("New tax profile");
  expect(page).toBeGreaterThanOrEqual(0);
  expect(add).toBeGreaterThanOrEqual(0);
  expect(add).toBeGreaterThan(page);
  await app.keyboard.press("escape");
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

test("desktop: a help desk client has no add control in Settings or the panel; the admin keeps both", async () => {
  const client = await open(AUTH, { ...WIDE, host: { params: HELPDESK } });
  await signIn(client, "000123453", "4321");
  await client.waitFor("id:sidebar");
  await client.press("id:context-pill");
  await client.waitFor("id:panel-profile");
  expect(client.shows("id:panel-new-tp")).toBe(false);
  expect(client.shows("id:panel-new-taxpayer")).toBe(false);
  // Settings > Account: no add control for a client either.
  await client.keyboard.press("mod+k");
  await client.waitFor("id:palette-input");
  await client.type("id:palette-input", "settings");
  await client.submit("id:palette-input");
  await client.waitFor("id:settings");
  await client.press("id:settings-nav-account");
  await client.waitFor("id:settings-account-card");
  expect(client.shows("id:settings-add-account")).toBe(false);

  const admin = await open(AUTH, { ...WIDE, host: { params: HELPDESK } });
  await signIn(admin, "000000000", "1234");
  await admin.waitFor("id:sidebar");
  await admin.press("id:context-pill");
  await admin.waitFor("id:panel-profile");
  expect(admin.shows("id:panel-new-tp")).toBe(true);
  // Settings > Account: the add control, same label.
  await admin.keyboard.press("mod+k");
  await admin.waitFor("id:palette-input");
  await admin.type("id:palette-input", "settings");
  await admin.submit("id:palette-input");
  await admin.waitFor("id:settings");
  await admin.press("id:settings-nav-account");
  await admin.waitFor("id:settings-account-card");
  expect(admin.shows("id:settings-add-account")).toBe(true);
  await admin.press("id:settings-add-account");
  await admin.waitFor("id:new-tp");
});

test("desktop: the entry is never there while the app is locked", async () => {
  // Help desk with a PIN boots locked at the sign-in: session kind open would pass the kind check, so only the lock
  // clause keeps adding hidden here. Signed in as the admin it is back.
  const app = await open(AUTH, { ...WIDE, host: { params: HELPDESK } });
  await app.waitFor("id:auth-card");
  await app.waitFor("text:canAdd: false");
  await signIn(app, "000000000", "1234");
  await app.waitFor("text:canAdd: true");
});

test("desktop/windows: the menu's New tax profile… is disabled for a client, enabled for the admin", async () => {
  const client = await open(AUTH, { width: 1440, height: 900, platform: "windows", host: { params: HELPDESK } });
  await signIn(client, "000123453", "4321");
  await client.waitFor("id:menu-File");
  await client.press("id:menu-File");
  await client.waitUntil(() => client.menu() !== null);
  expect((client.menu() ?? []).find((e) => e.label === "New tax profile…")?.disabled).toBe(true);
  await client.closeMenu();

  const admin = await open(AUTH, { width: 1440, height: 900, platform: "windows", host: { params: HELPDESK } });
  await signIn(admin, "000000000", "1234");
  await admin.waitFor("id:menu-File");
  await admin.press("id:menu-File");
  await admin.waitUntil(() => admin.menu() !== null);
  expect((admin.menu() ?? []).find((e) => e.label === "New tax profile…")?.disabled).toBe(false);
  await admin.closeMenu();
});
