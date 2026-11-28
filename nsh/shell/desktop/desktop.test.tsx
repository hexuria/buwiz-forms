// The desktop host in a window (DesktopDemo.tsx), at desktop size on macOS: the application menu follows the state of the app and its
// items drive the frame; close, quit and hide follow the owner's rules (macOS: the close button hides and the Dock icon goes, Cmd+Q
// quits; Windows and Linux: the close button asks, the hide key never does); any quit warns while returns are still sending; the tray
// shows the window again; the hotkey follows the setting; the update banner works end to end against a fake nsh/updater and says
// nothing at all when no source is configured.
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

/** What the fixture last wrote of what a hidden window cannot show: the Dock icon, the tray's menu, the quits. */
function posted(app: App): { dock: boolean; tray: string[]; quits: number } | null {
  const raw = app.storage()["fixture.desktop"];
  return raw ? (JSON.parse(raw) as { dock: boolean; tray: string[]; quits: number }) : null;
}

test("the application menu is installed, and follows the taxpayer, the form and the gate", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.waitFor("text:/^menu=[1-9]/");
  expect(app.shows("id:run:File > New Return…")).toBe(true);
  expect(app.shows("id:run:View > Toggle Tax Profile")).toBe(true);
  expect(app.shows("id:tray:Show eBIRForms")).toBe(true);
  expect(app.shows("id:tray:Quit eBIRForms")).toBe(true);
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
  expect(app.texts()).toContain("Version 0.2.1");
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

test("Cmd+H and Cmd+W reach the application menu: the window hides, the Dock icon goes, and the tray's Show brings both back", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  expect(shown(app)).toBe(true);
  expect(app.texts()).toContain("dock=shown");

  await pressUntil(app, "mod+h", () => hidden(app));
  await app.waitUntil(() => posted(app)?.dock === false);
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  await app.waitFor("text:dock=shown");

  await pressUntil(app, "mod+w", () => hidden(app));
  await app.waitUntil(() => posted(app)?.dock === false);
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));

  await app.host.send("run", { id: "tray:Hide eBIRForms" });
  await app.waitUntil(() => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  // The app is as it was (a headless window comes back at the default viewport, so the layout is not compared).
  expect(app.texts()).toContain("view=home");
  expect(app.texts()).toContain("overlay=none");
  expect(app.texts()).toContain("dock=shown");
});

test("the tray's Show and Hide follow the window: one of them is always disabled, Quit never is", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  const tray = () => posted(app)?.tray.join(" | ");
  await app.waitUntil(() => tray() === "Show eBIRForms  (disabled) | Hide eBIRForms | Quit eBIRForms");
  await app.host.send("run", { id: "tray:Hide eBIRForms" });
  await app.waitUntil(() => hidden(app));
  await app.waitUntil(() => tray() === "Show eBIRForms | Hide eBIRForms  (disabled) | Quit eBIRForms");
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  await app.waitUntil(() => tray() === "Show eBIRForms  (disabled) | Hide eBIRForms | Quit eBIRForms");
});

test("macOS: the window's close button hides the window and the Dock icon, and the app keeps running", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ quit: "fake" }) });
  await app.waitFor("id:sidebar");
  await app.window("main").pressClose();
  await app.waitUntil(() => hidden(app));
  await app.waitUntil(() => posted(app)?.dock === false);
  expect(posted(app)?.quits).toBe(0);
  // Still alive: Show brings the window back with the frame as it was, and no question was asked.
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  expect(app.texts()).toContain("view=home");
  expect(app.shows("id:close-ask")).toBe(false);
});

test("macOS: Cmd+Q quits (Apple review), from the app menu and the tray alike, signed in or locked", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ quit: "fake" }) });
  await app.waitFor("id:sidebar");
  await pressUntil(app, "mod+q", () => app.texts().includes("quits=1"));
  expect(shown(app)).toBe(true);
  await app.host.send("run", { id: "tray:Quit eBIRForms" });
  await app.waitFor("text:quits=2");

  // Locked: the quit and hide keys still work.
  const locked = await open(ENTRY, { ...WIDE, ...asking({ locked: true, quit: "fake" }) });
  await locked.waitFor("id:run:eBIRForms > Hide eBIRForms");
  await pressUntil(locked, "mod+q", () => locked.texts().includes("quits=1"));
  await pressUntil(locked, "mod+h", () => hidden(locked));
  await locked.host.send("run", { id: "tray:Show eBIRForms" });
  await locked.waitUntil(() => shown(locked));
});

test("remapped keys: with Hide on Cmd+Q and Quit on Option+Cmd+Q, Cmd+Q hides and Option+Cmd+Q quits; the menu shows them", async () => {
  const storage = { "desktop.close": JSON.stringify({ quit: "alt+cmd+q", hide: "cmd+q", close: "ask" }) };
  const app = await open(ENTRY, { ...WIDE, storage, ...asking({ quit: "fake" }) });
  await app.waitFor("id:sidebar");
  await app.waitFor("text:close=ask");
  await app.waitFor("id:run:eBIRForms > Hide eBIRForms");
  await pressUntil(app, "mod+q", () => hidden(app));
  expect(posted(app)?.quits).toBe(0);
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  await pressUntil(app, "alt+cmd+q", () => app.texts().includes("quits=1"));
});

test("any quit warns while returns are still sending: Wait stays, Hide to tray hides, Quit anyway quits", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ quit: "fake", sending: 2 }) });
  await app.waitFor("id:sidebar");
  await pressUntil(app, "mod+q", () => app.shows("id:sending-ask"));
  expect(app.texts()).toContain("2 returns are still sending");
  expect(app.texts()).toContain("Quitting stops them. Hide keeps them going in the background.");
  await app.press("id:sending-ask-wait");
  await app.waitFor("id:sending-ask", { state: "gone" });
  expect(app.texts()).toContain("quits=0");
  expect(shown(app)).toBe(true);

  // From the tray: the window comes up with the warning; Hide to tray keeps sending in the background.
  await app.host.send("run", { id: "tray:Quit eBIRForms" });
  await app.waitFor("id:sending-ask");
  await app.press("id:sending-ask-hide");
  await app.waitUntil(() => hidden(app));
  expect(posted(app)?.quits).toBe(0);

  await app.host.send("run", { id: "tray:Quit eBIRForms" });
  await app.waitUntil(() => shown(app));
  await app.waitFor("id:sending-ask");
  await app.press("id:sending-ask-quit");
  await app.waitFor("text:quits=1");

  // Nothing sending: no warning.
  const idle = await open(ENTRY, { ...WIDE, ...asking({ quit: "fake", sending: 0 }) });
  await idle.waitFor("id:sidebar");
  await idle.host.send("run", { id: "tray:Quit eBIRForms" });
  await idle.waitFor("text:quits=1");
  expect(idle.shows("id:sending-ask")).toBe(false);
});

for (const platform of ["windows", "linux"] as const) {
  const SIZE = { width: 1440, height: 900, platform };

  test(`${platform}: the close button asks "Hide to tray or Quit?"; Esc keeps the window, Quit quits, and the hide key never asks`, async () => {
    const app = await open(ENTRY, { ...SIZE, ...asking({ quit: "fake" }) });
    await app.waitFor("id:sidebar");
    await app.window("main").pressClose();
    await app.waitFor("id:close-ask");
    expect(app.texts()).toContain("Close eBIRForms?");
    expect(app.texts()).toContain("Hide to tray keeps sending your queued returns and reminders. Quit stops everything.");
    expect(app.texts()).toContain("Don't ask again. You can change this in Settings.");
    expect(shown(app)).toBe(true);
    await app.keyboard.press("escape");
    await app.waitFor("id:close-ask", { state: "gone" });
    expect(shown(app)).toBe(true);
    expect(app.texts()).toContain("quits=0");

    await app.window("main").pressClose();
    await app.waitFor("id:close-ask");
    await app.press("id:close-ask-quit");
    await app.waitFor("text:quits=1");
    // Not remembered: the next close asks again.
    expect(app.texts()).toContain("close=ask");
    expect(app.storage()["desktop.close"]).toBe(undefined);

    // Ctrl+H hides at once, no question; Ctrl+Q quits.
    await pressUntil(app, "mod+h", () => hidden(app));
    await app.host.send("run", { id: "tray:Show eBIRForms" });
    await app.waitUntil(() => shown(app));
    expect(app.shows("id:close-ask")).toBe(false);
    await pressUntil(app, "mod+q", () => app.texts().includes("quits=2"));
  });

  test(`${platform}: "Don't ask again" remembers Hide to tray: it is stored, and the next close hides without asking`, async () => {
    const app = await open(ENTRY, { ...SIZE, ...asking({ quit: "fake" }) });
    await app.waitFor("id:sidebar");
    await app.window("main").pressClose();
    await app.waitFor("id:close-ask");
    await app.press("id:close-ask-remember");
    await app.press("id:close-ask-hide");
    await app.waitUntil(() => hidden(app));
    await app.waitUntil(() => JSON.parse(app.storage()["desktop.close"] ?? "{}").close === "hide");
    await app.host.send("run", { id: "tray:Show eBIRForms" });
    await app.waitUntil(() => shown(app));
    await app.waitFor("text:close=hide");
    await app.window("main").pressClose();
    await app.waitUntil(() => hidden(app));
    expect(posted(app)?.quits).toBe(0);

    // Stored as Quit: the close button quits at once (through the warning when returns are sending).
    const quitting = await open(ENTRY, { ...SIZE, storage: { "desktop.close": JSON.stringify({ close: "quit" }) }, ...asking({ quit: "fake" }) });
    await quitting.waitFor("text:close=quit");
    await quitting.window("main").pressClose();
    await quitting.waitFor("text:quits=1");
    expect(quitting.shows("id:close-ask")).toBe(false);
  });
}

test("macOS: the Dock icon comes back with every show, NSH's own reopen (the Finder, Spotlight) included", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ quit: "fake" }) });
  await app.waitFor("id:sidebar");
  await pressUntil(app, "mod+h", () => hidden(app));
  await app.waitUntil(() => posted(app)?.dock === false);
  // The window comes back without showMain: the Dock icon follows the window's visible state.
  await app.host.send("run", { id: "reopen" });
  await app.waitUntil(() => shown(app));
  await app.waitUntil(() => posted(app)?.dock === true);
});

test("linux without a tray icon (GNOME without AppIndicator): the close question offers Quit, Enter quits, and nothing hides into nothing", async () => {
  const NO_TRAY = { width: 1440, height: 900, platform: "linux" as const, tray: false };
  const app = await open(ENTRY, { ...NO_TRAY, ...asking({ quit: "fake" }) });
  await app.waitFor("id:sidebar");
  await app.window("main").pressClose();
  await app.waitFor("id:close-ask");
  expect(app.shows("id:close-ask-no-tray")).toBe(true);
  expect(app.shows("id:close-ask-hide")).toBe(false);
  expect(app.shows("id:close-ask-cancel")).toBe(true);
  await app.keyboard.press("enter");
  await app.waitFor("text:quits=1");

  // A remembered "Hide to tray" asks instead; the hide key minimizes the window (the taskbar brings it back) rather than hiding it.
  const kept = await open(ENTRY, { ...NO_TRAY, storage: { "desktop.close": JSON.stringify({ close: "hide" }) }, ...asking({ quit: "fake" }) });
  await kept.waitFor("text:close=hide");
  await kept.window("main").pressClose();
  await kept.waitFor("id:close-ask");
  expect(kept.shows("id:close-ask-hide")).toBe(false);
  await kept.press("id:close-ask-cancel");
  await kept.waitFor("id:close-ask", { state: "gone" });
  await pressUntil(kept, "mod+h", () => kept.window("main").state()?.minimized === true);
  expect(posted(kept)?.quits ?? 0).toBe(0);
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
  const app = await open(ENTRY, { ...WIDE, ...asking({ poll: true, updater: "unconfigured" }) });
  await app.waitFor("id:sidebar");
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("text:Automatic updates are not configured in this build, so nothing is checked.");
  // The status poll has run (it starts at half a second): still idle, still no banner.
  await app.wait(2500);
  expect(app.texts()).toContain("update=idle");
  expect(app.shows("id:update-banner")).toBe(false);
});

test("a managed install offers the release page even on the account gate", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ locked: true, poll: true, updater: "external" }) });
  await app.waitFor("id:locked-frame");
  await app.waitFor("id:update-release", { timeout: 15_000 });
  expect(app.shows("id:update-install")).toBe(false);
  expect(app.texts()).toContain("Version 0.3.0 is available.");
  expect(app.texts()).toContain("asked=check");
  await app.press("id:update-later");
  await app.waitFor("id:update-banner", { state: "gone" });
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
  expect(app.texts()).toContain("eBIRForms 0.2.1 is up to date.");
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
  expect(win.shows("id:run:File > New tax profile…")).toBe(true);
  expect(win.shows("id:run:Profiles > New tax profile…")).toBe(true);

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
