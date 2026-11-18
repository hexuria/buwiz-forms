// The desktop host in a window (DesktopDemo.tsx), at desktop size on macOS: the application menu follows the state of the app and its
// items drive the frame; the close button hides, the tray shows the window again; the hotkey follows the setting; the update banner
// works end to end against a fake nsh/updater and says nothing at all when no source is configured.
// The system's menu bar and tray cannot be clicked from a test: the fixture runs the same items the system was given (their onSelect),
// and the keys that reach the application menu with no key binding of the frame's own (Cmd+H, Cmd+W) go through the real path.
import { type App, expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

// The first form an app opens builds every form's schema (the host does it lazily, once per process): a slow machine takes more than the 2 s a
// headless run gives one JS call, and an app that does it while it loads has only 5 s to show anything. This file's own load has 10 s.
warmForms();

const ENTRY = "../../DesktopDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("the application menu is installed, and follows the taxpayer, the form and the gate", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.waitFor("text:/^menu=[1-9]/");
  expect(app.shows("id:run:File > New Return…")).toBe(true);
  expect(app.shows("id:run:View > Toggle Tax Profile")).toBe(true);
  expect(app.shows("id:tray:Show eBIRForms")).toBe(true);
  expect(app.shows("id:tray:Quit")).toBe(true);
  // 27 of ours and the platform's own roles are all in the list; the number only has to say "the full menu".
  const full = Number(app.texts().find((t) => t.startsWith("menu="))?.slice(5));
  expect(full > 25).toBe(true);

  // A form open changes nothing.
  const form = await open(ENTRY, { ...WIDE, ...asking({ start: "form=1601C/2025/3" }) });
  await form.waitFor("text:view=form");
  expect(form.texts()).toContain("menu=" + full);

  // The gate in front: the filing menus go, About and Quit stay.
  const locked = await open(ENTRY, { ...WIDE, ...asking({ locked: true }) });
  await locked.waitFor("id:run:eBIRForms > Quit eBIRForms");
  expect(locked.shows("id:run:File > New Return…")).toBe(false);
  expect(locked.shows("id:run:eBIRForms > About eBIRForms")).toBe(true);
  expect(locked.shows("id:run:View > Toggle Sidebar")).toBe(false);
});

test("the menu's items drive the frame: sidebar, panels, theme, palette, new return, settings, About", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.waitFor("id:run:View > Toggle Sidebar");

  await app.press("id:run:View > Toggle Sidebar");
  await app.waitFor("text:sidebar=closed");
  await app.press("id:run:View > Toggle Sidebar");
  await app.waitFor("text:sidebar=open");

  await app.press("id:run:View > Toggle Tax Profile");
  await app.waitFor("text:panel=profile");
  await app.press("id:run:View > Toggle Notifications");
  await app.waitFor("text:panel=notifs");
  await app.press("id:run:View > Toggle Notifications");
  await app.waitFor("text:panel=none");

  expect(app.texts()).toContain("theme=system");
  await app.press("id:run:View > Toggle Theme");
  await app.waitFor("text:/^theme=(light|dark)$/");

  await app.press("id:run:View > Command Palette…");
  await app.waitFor("text:overlay=palette");
  await app.back();
  await app.waitFor("text:overlay=none");

  await app.press("id:run:File > New Return…");
  await app.waitFor("id:new-return");
  expect(app.texts()).toContain("overlay=new");
  await app.press("id:new-return-cancel");

  await app.press("id:run:eBIRForms > Settings…");
  await app.waitFor("text:view=settings");

  await app.press("id:run:eBIRForms > About eBIRForms");
  await app.waitFor("id:about");
  expect(app.texts()).toContain("Version 0.2.0");
  expect(app.texts()).toContain("© 2026 Goldcoders Corp");
  await app.press("id:about-ok");
  await app.waitFor("id:about", { state: "gone" });
});

test("with no taxpayer, New Return and the tax profile do nothing; the rest of the menu still works", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ demo: false }) });
  await app.waitFor("id:run:File > New Return…");
  await app.press("id:run:File > New Return…");
  await app.press("id:run:View > Toggle Sidebar");
  await app.waitFor("text:sidebar=closed");
  expect(app.texts()).toContain("overlay=none");
});

/** The main window is hidden (what no readout on it can say: nothing on a hidden window can be read). */
const hidden = (app: { window(id: string): { state(): { visible: boolean } | null } }) => app.window("main").state()?.visible === false;
const shown = (app: { window(id: string): { state(): { visible: boolean } | null } }) => app.window("main").state()?.visible === true;

/**
 * Presses `keys` until `done()` holds. The application menu is installed again a few times as the frame opens (the host's refresh comes a
 * tick after the chrome does), and the harness picks the item a key chooses from the menu it has seen: a key pressed in that moment names an
 * item the app has already dropped, and nothing happens. On a slow machine the moment is long enough to be hit, so the key is pressed again.
 */
async function pressUntil(app: App, keys: string, done: () => boolean, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!done()) {
    if (Date.now() > end) throw new Error(`${keys} had no effect in ${cap} ms`);
    await app.keyboard.press(keys);
    await app.waitUntil(done, { timeout: 1000 }).catch(() => undefined);
  }
}

test("Cmd+H and Cmd+W reach the application menu: the window hides, and the tray's Show brings it back", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  expect(shown(app)).toBe(true);

  await pressUntil(app, "mod+h", () => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));

  await pressUntil(app, "mod+w", () => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));

  await app.host.send("run", { id: "tray:Hide eBIRForms" });
  await app.waitUntil(() => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  // The app is as it was (a headless window comes back at the default viewport, so the layout is not compared).
  expect(app.texts()).toContain("view=home");
  expect(app.texts()).toContain("overlay=none");
});

test("the window's close button hides the window instead of closing it, and the app keeps running", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.window("main").pressClose();
  await app.waitUntil(() => hidden(app));
  // Still alive: Show brings the window back with the frame as it was.
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  expect(app.texts()).toContain("view=home");
});

test("Cmd+Q hides to the tray on macOS (the quit is in the tray)", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await pressUntil(app, "mod+q", () => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
});

test("the global hotkey follows the stored setting: registered, replaced when it changes, off when it is cleared or the gate is up", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ hotkey: "control+alt+shift+9", system: "fake" }) });
  await app.waitFor("id:sidebar");
  await app.waitFor("text:chords=ctrl+alt+shift+9");
  expect(app.texts()).toContain("hotkey=registered");

  // Pressing the chord hides the window, and pressing it again shows it (the old toggle).
  await app.host.send("run", { id: "chord:ctrl+alt+shift+9" });
  await app.waitUntil(() => app.window("main").state()?.visible === false);
  await app.host.send("run", { id: "chord:ctrl+alt+shift+9" });
  await app.waitUntil(() => app.window("main").state()?.visible === true);

  // Settings stores another chord: the first is given back and the new one taken.
  await app.host.send("run", { id: "hotkey:control+alt+B" });
  await app.waitFor("text:chords=ctrl+alt+b");
  // And an empty setting turns it off.
  await app.host.send("run", { id: "hotkey:" });
  await app.waitFor("text:chords=");
  expect(app.texts()).toContain("hotkey=off");

  const none = await open(ENTRY, { ...WIDE, ...asking({ system: "fake" }) });
  await none.waitFor("id:sidebar");
  expect(none.texts()).toContain("chords=");
  expect(none.texts()).toContain("hotkey=off");

  // On the sign-in screens the chord is not held.
  const gate = await open(ENTRY, { ...WIDE, ...asking({ hotkey: "control+alt+shift+9", locked: true, system: "fake" }) });
  await gate.waitFor("id:run:eBIRForms > Quit eBIRForms");
  expect(gate.texts()).toContain("chords=");
  expect(gate.texts()).toContain("hotkey=off");
});

test("no update source: Check for Updates says so, and the banner never shows", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ poll: true }) });
  await app.waitFor("id:sidebar");
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("text:Automatic updates are not configured in this build, so nothing is checked.");
  // The status poll has run (it starts at half a second): still idle, still no banner.
  await app.wait(2500);
  expect(app.texts()).toContain("update=idle");
  expect(app.shows("id:update-banner")).toBe(false);
});

test("the update banner end to end: a check finds a release, the banner offers it, Later puts it away, Install now installs it", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ poll: true, updater: "found" }) });
  await app.waitFor("id:sidebar");
  expect(app.shows("id:update-banner")).toBe(false);

  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("id:update-banner", { timeout: 15_000 });
  expect(app.texts()).toContain("Version 0.3.0 is ready.");
  expect(app.shows("id:update-install")).toBe(true);
  expect(app.shows("id:update-later")).toBe(true);
  expect(app.texts()).toContain("asked=check,download");

  await app.press("id:update-later");
  await app.waitFor("id:update-banner", { state: "gone" });
  await app.wait(2500);
  expect(app.shows("id:update-banner")).toBe(false);

  // A second manual check offers it again; Install now runs the install.
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("id:update-install", { timeout: 15_000 });
  await app.press("id:update-install");
  await app.waitFor("text:/asked=.*install$/", { timeout: 15_000 });
  await app.waitFor("id:update-banner", { state: "gone", timeout: 15_000 });
});

test("a check that finds nothing says the app is up to date, in the banner, and Later puts it away", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ poll: true, updater: "current" }) });
  await app.waitFor("id:sidebar");
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("id:update-banner", { timeout: 15_000 });
  expect(app.texts()).toContain("eBIRForms 0.2.0 is up to date.");
  expect(app.shows("id:update-install")).toBe(false);
  await app.press("id:update-later");
  await app.waitFor("id:update-banner", { state: "gone" });
});

test("the menu installs on Windows and Linux too (every accelerator is one NSH reads), with the platform's own shape", async () => {
  const win = await open(ENTRY, { width: 1440, height: 900, platform: "windows", ...asking() });
  await win.waitFor("id:sidebar");
  await win.waitFor("id:run:File > Settings…");
  expect(win.shows("id:run:File > Quit eBIRForms")).toBe(true);
  expect(win.shows("id:run:Help > About eBIRForms")).toBe(true);
  // The app-menu design: Windows and Linux start with an eBIRForms menu too (About, Settings…, Hide, Sign Out, Quit).
  expect(win.shows("id:run:eBIRForms > Quit eBIRForms")).toBe(true);
  expect(win.shows("id:run:File > New Tax Profile…")).toBe(true);
  expect(win.shows("id:run:Profiles > New Tax Profile…")).toBe(true);

  const linux = await open(ENTRY, { width: 1440, height: 900, platform: "linux", ...asking() });
  await linux.waitFor("id:sidebar");
  await linux.waitFor("id:run:Edit > Settings…");
  expect(linux.shows("id:run:File > Quit eBIRForms")).toBe(true);
  expect(linux.shows("id:run:View > Enter Full Screen")).toBe(false); // a role: the platform carries it out
});

test("phone: there is no application menu, no tray and no hotkey, and the frame is untouched", async () => {
  const app = await open(ENTRY, asking());
  await app.waitFor("id:tab-bar");
  expect(app.texts()).toContain("menu=0");
  expect(app.texts()).toContain("hotkey=off");
  expect(app.shows("id:run:File > New Return…")).toBe(false);
  expect(app.shows("id:update-banner")).toBe(false);
});
