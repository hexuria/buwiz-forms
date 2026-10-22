// Settings search and the account card. The screen draws a row only when
// settingsView names it, so this is the list of controls Settings can show.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

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
  assert.deepEqual(rows(view, "general"), ["theme", "recent", "compact"]);
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
  const view = settingsView("HOTKEY", true);
  assert.deepEqual(ids(view), ["computer"]);
  assert.deepEqual(rows(view, "computer"), ["hotkey"]);
}

{
  const view = settingsView("personal", true);
  assert.deepEqual(ids(view), ["general", "account"]);
  assert.deepEqual(rows(view, "general"), ["theme", "recent", "compact"], "a section word keeps every row");
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

console.log("test_settings_window: ok");
