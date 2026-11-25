// The frame at desktop size (1440 x 900, macOS): title bar, sidebar, history, the panels, overlays under 860 points, Esc,
// the theme cycle, the account menu and the dev start route. Against a scratch database with the demo taxpayer
// (ShellDemo.tsx; a test asks for variations with open's host params).
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("desktop: the frame names the taxpayer and the sidebar walks the pages, with Back and Forward", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  expect(app.shows("id:title-bar")).toBe(true);
  expect(app.shows("id:tab-bar")).toBe(false);
  expect(app.texts()).toContain("Juana Santos");
  expect(app.texts()).toContain("000-123-453-00000");
  expect(app.shows("id:home")).toBe(true);
  expect(app.enabled("id:tb-back")).toBe(false);
  expect(app.enabled("id:tb-forward")).toBe(false);

  await app.press("id:nav-library");
  await app.waitFor("id:library");
  await app.press("id:nav-filings");
  await app.waitFor("id:filings");
  await app.press("id:nav-calendar");
  await app.waitFor("id:calendar");
  expect(app.enabled("id:tb-back")).toBe(true);
  expect(app.enabled("id:tb-forward")).toBe(false);

  await app.press("id:tb-back");
  await app.waitFor("id:filings");
  expect(app.enabled("id:tb-forward")).toBe(true);
  await app.keyboard.press("mod+[");
  await app.waitFor("id:library");
  await app.keyboard.press("mod+]");
  await app.waitFor("id:filings");
  await app.press("id:tb-forward");
  await app.waitFor("id:calendar");
  expect(app.enabled("id:tb-forward")).toBe(false);

  // Moving somewhere new empties Forward.
  await app.press("id:tb-back");
  await app.press("id:nav-home");
  await app.waitFor("id:home");
  expect(app.enabled("id:tb-forward")).toBe(false);
});

test("desktop: the sidebar, the profile panel and the notifications toggle from the title bar and the keys", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  expect(app.shows("id:tb-search")).toBe(false); // search and "+" are in the sidebar
  await app.press("id:tb-sidebar");
  expect(app.shows("id:sidebar")).toBe(false);
  expect(app.shows("id:tb-search")).toBe(true);
  expect(app.shows("id:tb-new")).toBe(true);
  await app.keyboard.press("mod+b");
  await app.waitFor("id:sidebar");
  expect(app.shows("id:tb-search")).toBe(false);

  // The context pill toggles the tax profile.
  await app.press("id:context-pill");
  await app.waitFor("id:panel-profile");
  expect(app.texts()).toContain("Open");
  expect(app.texts()).toContain("Edit in profile");
  expect(app.texts()).toContain("Add branch");
  expect(app.texts()).toContain("Quezon City");
  await app.press("id:panel-open-forms");
  await app.waitFor("id:tp-page-forms");
  await app.press("id:context-pill");
  expect(app.shows("id:panel-profile")).toBe(false);

  // The bell toggles the notifications; the panel button hides and reopens the last panel.
  await app.press("id:tb-notifications");
  await app.waitFor("id:panel-notifications");
  expect(app.texts()).toContain("No notifications yet.");
  expect(app.enabled("id:notif-mark-all")).toBe(false);
  await app.press("id:tb-panel");
  expect(app.shows("id:panel-notifications")).toBe(false);
  await app.press("id:tb-panel");
  await app.waitFor("id:panel-notifications");
  await app.keyboard.press("mod+shift+b");
  await app.waitFor("id:panel-profile");
  await app.keyboard.press("mod+shift+b");
  expect(app.shows("id:panel-profile")).toBe(false);
  await app.keyboard.press("mod+shift+n");
  await app.waitFor("id:panel-notifications");
});

test("desktop: the profile panel switches branch and taxpayer, and the pill follows", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ branch: true, second: true }) });
  await app.waitFor("id:sidebar");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-profile");
  expect(app.texts()).toContain("Branches");
  await app.press("id:panel-branch-00001");
  await app.waitFor("text:000-123-453-00001");
  // The choice is kept under the logical key, suffixed by the database path.
  await app.waitUntil(() => Object.keys(app.storage()).some((k) => k.startsWith("redesign.selection@")));
  const selection = Object.entries(app.storage()).find(([k]) => k.startsWith("redesign.selection@"))!;
  expect(JSON.parse(selection[1])).toEqual({ root: "000123453", code: "00001" });
  await app.press("id:panel-tp-000111229");
  await app.waitFor("text:Sample Trading Corp");
  await app.waitFor("text:Switched to Sample Trading Corp");
});

test("desktop: below 860 points the sidebar and the panel float, close on a move, and Esc closes the top layer first", async () => {
  const app = await open(ENTRY, { width: 760, height: 800, platform: "macos", ...asking() });
  await app.waitFor("id:title-bar");
  expect(app.shows("id:sidebar")).toBe(false);
  expect(app.shows("id:tb-search")).toBe(true);

  await app.press("id:tb-sidebar");
  await app.waitFor("id:sidebar-overlay");
  expect(app.shows("id:overlay-scrim")).toBe(true);
  await app.press("id:overlay-scrim");
  expect(app.shows("id:sidebar-overlay")).toBe(false);

  await app.press("id:tb-sidebar");
  await app.waitFor("id:sidebar-overlay");
  await app.press("id:nav-library");
  await app.waitFor("id:library");
  expect(app.shows("id:sidebar-overlay")).toBe(false); // moving closes it

  // Esc: the dialog over a panel closes first, then the panel.
  await app.press("id:context-pill");
  await app.waitFor("id:panel-overlay");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.keyboard.press("escape");
  await app.waitFor("id:palette-input", { state: "gone" });
  expect(app.shows("id:panel-overlay")).toBe(true);
  await app.keyboard.press("escape");
  await app.waitFor("id:panel-overlay", { state: "gone" });

  // Esc closes the floating sidebar too.
  await app.press("id:tb-sidebar");
  await app.waitFor("id:sidebar-overlay");
  await app.keyboard.press("escape");
  await app.waitFor("id:sidebar-overlay", { state: "gone" });

  // Widening past 860 docks the sidebar open; narrowing again floats it closed.
  await app.setViewport(1440, 900);
  await app.waitFor("id:sidebar");
  expect(app.shows("id:sidebar-overlay")).toBe(false);
  await app.press("id:context-pill");
  await app.waitFor("id:panel-profile");
  await app.setViewport(760, 800);
  await app.waitFor("id:sidebar", { state: "gone" });
  expect(app.shows("id:panel-profile")).toBe(false);
});

test("desktop: the theme button cycles light, dark, system and the choice is stored in the database", async () => {
  const app = await open(ENTRY, { ...WIDE, colorScheme: "dark", ...asking({ show: "setting" }) });
  await app.waitFor("id:sidebar");
  expect(app.shows("label:Theme: system, press to change")).toBe(true);
  await app.press("id:tb-theme");
  await app.waitFor("text:theme setting: Light");
  expect(app.shows("label:Theme: light, press to change")).toBe(true);
  await app.press("id:tb-theme");
  await app.waitFor("text:theme setting: Dark");
  await app.keyboard.press("mod+t");
  await app.waitFor("text:theme setting: System");
  expect(app.shows("label:Theme: system, press to change")).toBe(true);
});

test("desktop: a stored theme is applied at start", async () => {
  const app = await open(ENTRY, { ...WIDE, colorScheme: "light", ...asking({ theme: "Dark" }) });
  await app.waitFor("id:sidebar");
  expect(app.shows("label:Theme: dark, press to change")).toBe(true);
});

test("desktop: the account menu offers Lock app, Account settings and Log out", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:account-row");
  expect(app.texts()).toContain("Local account");
  await app.press("id:account-row");
  await app.waitFor("id:menu-account");
  expect(app.texts()).toContain("Lock app");
  expect(app.texts()).toContain("Log out");
  await app.press("id:menu-account");
  await app.waitFor("id:settings");
  await app.press("id:account-row");
  await app.press("id:menu-lock");
  // No PIN is set, so there would be nothing to sign back in with.
  await app.waitFor("text:Set your PIN in Settings first. With no PIN there is nothing to sign back in with.");
  expect(app.shows("id:sidebar")).toBe(true);
});

test("desktop: Cmd+comma opens Settings; a view that needs a taxpayer falls back to Home without one", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ demo: false }) });
  await app.waitFor("id:sidebar");
  expect(app.texts()).toContain("Add a taxpayer");
  expect(app.shows("id:dev-door")).toBe(false); // no dev strip on an empty database: the dev pages are in the palette only
  await app.keyboard.press("mod+,");
  await app.waitFor("id:settings");
  // The panel says there is no taxpayer yet, and offers one.
  await app.press("id:context-pill");
  await app.waitFor("id:profile-empty");
  expect(app.texts()).toContain("No taxpayer yet");
  await app.press("id:panel-new-taxpayer");
  await app.waitFor("id:new-tp");
});

test("desktop: the dev start route opens a view, the panel and the palette at launch", async () => {
  const library = await open(ENTRY, { ...WIDE, ...asking({ start: "view=library;panel=profile" }) });
  await library.waitFor("id:library");
  await library.waitFor("id:panel-profile");
  const palette = await open(ENTRY, { ...WIDE, ...asking({ start: "palette=calendar" }) });
  await palette.waitFor("id:palette-input");
  expect(palette.value("id:palette-input")).toBe("calendar");
  const dark = await open(ENTRY, { ...WIDE, colorScheme: "light", ...asking({ start: "theme=dark;view=filings" }) });
  await dark.waitFor("id:filings");
  expect(dark.shows("label:Theme: dark, press to change")).toBe(true);
});

test("desktop: the polls stay off unless a test asks, and run when it does", async () => {
  const off = await open(ENTRY, { ...WIDE, clock: {}, ...asking({ show: "update" }) });
  await off.waitFor("text:update: none");
  await off.clock!.runFor(6000);
  expect(off.texts()).toContain("update: none");

  const on = await open(ENTRY, { ...WIDE, clock: {}, ...asking({ show: "update", poll: true }) });
  await on.waitFor("text:update: none");
  await on.clock!.runFor(1500);
  await on.waitUntil(() => !on.texts().includes("update: none"));
});

test("desktop: the dev pages open from the palette, and back returns to the app", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ demo: false }) });
  await app.waitFor("id:sidebar");
  expect(app.shows("id:dev-door")).toBe(false);
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "ui kit");
  await app.submit("id:palette-input");
  await app.waitFor("text:Text and marks");
  expect(app.shows("id:sidebar")).toBe(false); // the dev pages take the whole window
  await app.press("text:Theme");
  await app.waitFor("text:Native module says: pong");
  await app.press("id:dev-back");
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "theme page");
  await app.submit("id:palette-input");
  await app.waitFor("id:dev-pages");
});

test("desktop: a page that throws shows a message, and the frame stays", async () => {
  // A return view with nothing to open is the page that fails.
  const app = await open(ENTRY, { ...WIDE, ...asking({ start: "view=form" }) });
  await app.waitFor("id:view-error");
  expect(app.shows("id:sidebar")).toBe(true);
  expect(app.texts()).toContain("This page could not be shown");
  await app.press("id:view-error-home");
  await app.waitFor("id:home");
  expect(app.shows("id:view-error")).toBe(false);
});

test("desktop: filing events become notifications: an unread dot, the list, Mark all read, a press reads one and opens Filings", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ filed: true }) });
  await app.waitFor("id:sidebar");
  await app.press("id:tb-notifications");
  await app.waitFor("id:panel-notifications");
  await app.waitFor("text:2551Q for Q1 2025 was submitted");
  expect(app.texts()).toContain("BIR confirmed 1601C for March 2025");
  expect(app.texts()).toContain("Waiting for BIR's confirmation email · Juana Santos");
  expect(app.texts()).toContain("Confirmation email received · Juana Santos");
  // A mock-relay send never reaches BIR, so it does not wait for BIR's email.
  expect(app.texts()).toContain("Sent to the mock relay, not to BIR: no BIR email will come · Juana Santos");
  // A send the worker only simulated (a dry run) is no submission: it finished a dry run and sent nothing.
  expect(app.texts()).toContain("2551Q for Q2 2025: dry run finished");
  expect(app.texts()).toContain("Nothing was sent to the BIR · Juana Santos");
  expect(app.texts()).not.toContain("2551Q for Q2 2025 was submitted");
  expect(app.shows("id:notif-unread-return:9001:submitted")).toBe(true);
  expect(app.shows("id:notif-unread-return:9002:confirmed")).toBe(true);
  expect(app.enabled("id:notif-mark-all")).toBe(true);

  // A press reads that one and shows Filings.
  await app.press("id:notif-return:9001:submitted");
  await app.waitFor("id:filings");
  expect(app.shows("id:panel-notifications")).toBe(true); // docked: it stays beside the page
  await app.waitFor("id:notif-unread-return:9001:submitted", { state: "gone" });
  expect(app.shows("id:notif-unread-return:9002:confirmed")).toBe(true);

  // Mark all read: no dots, the button waits, and the ids are kept per database.
  await app.press("id:notif-mark-all");
  await app.waitFor("id:notif-unread-return:9002:confirmed", { state: "gone" });
  expect(app.enabled("id:notif-mark-all")).toBe(false);
  await app.waitUntil(() => {
    const kept = Object.entries(app.storage()).find(([k]) => k.startsWith("redesign.read_notifications@"));
    return !!kept && JSON.parse(kept[1]).includes("return:9002:confirmed");
  });
});
