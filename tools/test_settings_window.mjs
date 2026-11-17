// Settings search and the account card. The screen draws a row only when
// settingsView names it, so this is the list of controls Settings can show.
// Then the Settings title bar, through the real app/main.js and chrome.js:
// only the window buttons, Back and Theme, and nothing changed on Settings
// shows up after leaving it.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { makeApp, cx, one, byId, ids as treeIds, arg, click, dispatch } from "./shell_mount.mjs";

const root = new URL("../", import.meta.url);
const context = vm.createContext({ console });
const source = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/settings_model.js", root), "utf8"), {
  context,
  identifier: "app/shell/settings_model.js",
});
await source.link(() => {
  throw new Error("settings_model.js imports nothing");
});
await source.evaluate();
const { settingsView, shownSection, accountSummary, hotkeyLabel, hotkeyOptions, retentionLabel, hits } = source.namespace;

const host = (value) => JSON.parse(JSON.stringify(value));
const ids = (view) => host(view.map((section) => section.id));
const rows = (view, id) => host(view.find((section) => section.id === id).rows);

{
  const view = settingsView("", true);
  assert.deepEqual(ids(view), ["general", "account", "transfer", "jobs", "computer"]);
  assert.deepEqual(rows(view, "general"), ["theme", "recent", "compact", "updates"]);
  assert.ok(rows(view, "transfer").includes("import"));
  assert.equal(String(shownSection("account", view)), "account");
}

{
  const view = settingsView("", false);
  assert.deepEqual(ids(view), ["general", "account", "transfer"]);
  assert.deepEqual(rows(view, "transfer"), ["export"]);
  assert.equal(String(shownSection("jobs", view)), "general");
}

{
  const view = settingsView("theme", true);
  assert.deepEqual(ids(view), ["general"]);
  assert.deepEqual(rows(view, "general"), ["theme"]);
}

{
  const view = settingsView("buwiz", true);
  assert.deepEqual(ids(view), ["account"]);
  assert.deepEqual(rows(view, "account"), ["buwiz"]);
}

{
  const view = settingsView("HOTKEY", true);
  assert.deepEqual(ids(view), ["computer"]);
  assert.deepEqual(rows(view, "computer"), ["hotkey"]);
}

{
  const view = settingsView("personal", true);
  assert.deepEqual(ids(view), ["general", "account"]);
  assert.deepEqual(rows(view, "general"), ["theme", "recent", "compact", "updates"], "a section word keeps every row");
}

{
  assert.deepEqual(host(settingsView("zzzz-not-a-setting", true)), []);
  assert.equal(shownSection("general", []), null);
  assert.equal(Boolean(hits("gmail connect", "google gmail connect disconnect")), true);
  assert.equal(Boolean(hits("gmail calendar", "google gmail connect")), false);
}

{
  assert.equal(String(hotkeyLabel("", [])), "Off");
  assert.equal(String(hotkeyLabel("super+shift+E", [{ value: "super+shift+E", label: "Super+Shift+E" }])), "Super+Shift+E");
  assert.equal(String(hotkeyLabel("ctrl+alt+Z", [])), "ctrl+alt+Z");
  const options = host(hotkeyOptions("ctrl+alt+Z", [{ value: "super+shift+E", label: "Super+Shift+E" }]));
  assert.equal(options[0].label, "Off");
  assert.ok(options.some((option) => option.value === "ctrl+alt+Z"));
  assert.equal(String(retentionLabel(1)), "1 day");
  assert.equal(String(retentionLabel(14)), "14 days");
  assert.equal(String(retentionLabel(0)), "7 days");
}

{
  const helpdesk = accountSummary({
    isAdmin: true,
    isHelpdesk: true,
    adminRoot: "000000000",
    sessionKind: "admin",
    taxpayers: [{ tin_root: "123456788", name: "Example Trading", tin_display: "123-456-788-00000" }],
    branch: { tin: "12345678800000", name: "Head office", facts: { email: "office@example.com" } },
    canAddTaxpayer: true,
  });
  assert.equal(String(helpdesk.name), "Help desk admin");
  assert.equal(String(helpdesk.email), "");
  assert.equal(String(helpdesk.mailbox.email), "office@example.com");
  assert.equal(Boolean(helpdesk.showAdd), true);

  const owner = accountSummary({
    isAdmin: true,
    isHelpdesk: false,
    adminRoot: "123456788",
    sessionKind: "admin",
    taxpayers: [{ tin_root: "123456788", name: "Example Trading", tin_display: "123-456-788-00000" }],
    branch: { tin: "12345678800000", name: "Head office", facts: { email: "office@example.com" } },
    canAddTaxpayer: false,
  });
  assert.equal(String(owner.name), "Example Trading");
  assert.equal(String(owner.email), "office@example.com");
  assert.equal(Boolean(owner.showAdd), false);
  assert.equal(String(owner.detail), "Admin · this computer");
}

// ---------------------------------------------------------------- title bar

const CAPTIONS = ["caption-minimize", "caption-maximize", "caption-close"];
const titleBar = (app) => treeIds(one(app.paint(cx), "title-bar")).slice(1);

for (const os of ["macos", "windows", "linux"]) {
  const captions = os === "windows" ? CAPTIONS : [];
  const app = makeApp({ os });
  assert.deepEqual(
    titleBar(app),
    ["tb-sidebar", "tb-back", "tb-forward", "context-pill", "tb-notifications", "tb-theme", "tb-panel", ...captions],
    `${os}: the main window keeps its title bar`,
  );
  app.go("settings", cx);
  assert.deepEqual(titleBar(app), ["tb-back", "tb-theme", ...captions], `${os}: Settings shows only Back and Theme`);
  // Back is always there on Settings, also on a narrow window.
  const narrow = makeApp({ os, width: 500, view: "settings" });
  assert.deepEqual(titleBar(narrow), ["tb-back", "tb-theme", ...captions], `${os} at 500: Back and Theme`);
}

{
  // Back leaves Settings for where it was opened; the theme button works there.
  const app = makeApp();
  app.go("calendar", cx);
  app.go("settings", cx);
  const tree = app.paint(cx);
  assert.equal(arg(one(tree, "tb-back"), "tooltip"), "Back to app");
  click(tree, "tb-theme", cx);
  assert.equal(app.themePref, "dark", "the theme button works on Settings");
  click(app.paint(cx), "tb-back", cx);
  assert.equal(app.view, "calendar", "Back returns to the page Settings was opened from");
  // Opened first, with nothing to go back to, Back lands on Home.
  const fresh = makeApp({ view: "settings" });
  click(fresh.paint(cx), "tb-back", cx);
  assert.equal(fresh.view, "home");
}

{
  // Nothing that Settings covers changes there: no toggle by action, by the
  // ⌘K list or by a method, so nothing pops open on return.
  const app = makeApp();
  app.togglePanel("notifs", cx);
  assert.equal(app.panel, "notifs");
  app.go("settings", cx);
  const before = JSON.stringify({ sidebar: app.sidebarOpen, panel: app.panel, last: app.lastPanel });
  const tree = app.paint(cx);
  for (const action of ["toggle-sidebar", "toggle-profile", "toggle-notifications"]) {
    assert.ok(dispatch(tree, action, cx), `${action} reaches the app root`);
  }
  app.toggleSidebar(cx);
  app.togglePanel("profile", cx);
  app.toggleRightPanel(cx);
  const offered = app.paletteItems().map((x) => String(x.label));
  for (const label of ["Toggle sidebar", "Show tax profile", "Hide tax profile", "Show notifications", "Hide notifications"]) {
    assert.ok(!offered.includes(label), `the ⌘K list on Settings has no "${label}"`);
  }
  assert.ok(offered.includes("Change theme"), "the ⌘K list on Settings keeps Change theme");
  assert.equal(JSON.stringify({ sidebar: app.sidebarOpen, panel: app.panel, last: app.lastPanel }), before, "Settings left the chrome as it was");
  app.closeSettings(cx);
  assert.equal(app.view, "home");
  const back = app.paint(cx);
  assert.equal(byId(back, "panel-notifications").length, 1, "the panel open before Settings is open again");
  assert.equal(byId(back, "panel-profile").length, 0, "no panel opened on Settings pops open");
  assert.equal(byId(back, "sidebar").length, 1, "the sidebar is where it was");
  assert.deepEqual(treeIds(one(back, "title-bar")).slice(1, 5), ["tb-sidebar", "tb-back", "tb-forward", "context-pill"], "the title bar is whole again");
}

console.log("test_settings_window: ok");
