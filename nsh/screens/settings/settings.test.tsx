// Settings at phone size (390 x 844) and at desktop size (1440 x 900, macOS): the sections and how a phone walks them, General, the
// search over rows, who sees what, Import / Export and the old Background tasks address. Against a scratch database with the demo taxpayer
// (SettingsDemo.tsx; a test asks for variations with open's host params).
import { expect, test } from "nsh/test";
import { calls, expectTexts, goSection, openSettings, SIZES, state } from "./testing";

for (const { name, size } of SIZES) {
  test(`${name}: General shows the platform's rows and its controls write what the app keeps`, async () => {
    const app = await openSettings(size, { update: true }, "general");
    await app.waitFor("id:settings-appearance");
    expectTexts(app, ["General", "Appearance", "Theme"]);
    expectTexts(app, [`System follows this ${name === "phone" ? "device" : "computer"}`]);
    expect(state(app)).toContain("theme=system recent=true compact=false");

    await app.press("id:settings-theme-trigger");
    await app.waitFor("text:Dark");
    await app.press("text:Dark");
    await app.waitFor("text:/theme=dark/");
    if (name === "phone") {
      for (const id of ["settings-recent", "settings-compact-form-nav", "settings-auto-update"]) expect(app.shows(`id:${id}`)).toBe(false);
      expect(calls(app)).toEqual([]);
      return;
    }
    expectTexts(app, ["Show recent returns", "Compact form sections", "Download updates automatically"]);
    await app.press("id:settings-recent-switch");
    await app.waitFor("text:/recent=false/");
    await app.press("id:settings-compact-form-nav-switch");
    await app.waitFor("text:/compact=true/");
    expect(state(app)).toContain("theme=dark recent=false compact=true");

    // The update switch starts on (the status says auto download is on); a press turns it off and tells the host.
    expect(calls(app)).toEqual([]);
    await app.press("id:settings-auto-update-switch");
    await app.waitFor("text:calls: set_auto_update_download false");
    await app.press("id:settings-auto-update-switch");
    await app.waitFor("text:calls: set_auto_update_download false | set_auto_update_download true");
  }, 60_000);

  test(`${name}: the search keeps the rows whose words match, the section when its name matches, and says when nothing does`, async () => {
    const app = await openSettings(size, {}, undefined);
    await app.waitFor("id:settings-nav-general");
    expectTexts(app, ["Personal", "General", "Account", "Data", "Import / Export", "Jobs", "System", name === "phone" ? "Device" : "Computer"]);
    expect(app.shows("id:settings-nav-keyboard")).toBe(name !== "phone");
    await app.type("id:settings-search", "pin");
    await app.waitFor("text:/Account/");
    expect(app.shows("id:settings-nav-account")).toBe(true);
    for (const gone of ["general", "transfer", "jobs", "computer"]) expect(app.shows(`id:settings-nav-${gone}`)).toBe(false);
    await goSection(app, "account", "id:settings-security");
    // Only the security row of Account is kept: no Buwiz, who-is-signed-in or Google cards.
    expect(app.shows("id:settings-account")).toBe(false);
    expect(app.shows("id:settings-account-card")).toBe(false);
    expect(app.shows("id:settings-google")).toBe(false);
    expect(app.texts()).toContain("My PIN");

    // The section's name keeps every row of it. (A phone's search sits on the list, so Back first, and the cards on the section's page.)
    if (name === "phone") await app.press("id:settings-back");
    await app.clear("id:settings-search");
    await app.type("id:settings-search", "account");
    await app.waitFor("id:settings-nav-account");
    if (name === "phone") await goSection(app, "account", "id:settings-security");
    expect(app.shows("id:settings-account")).toBe(true);
    // Google (Gmail connect) is desktop only (settings_model.ts).
    expect(app.shows("id:settings-google")).toBe(name === "desktop");
    expect(app.shows("id:settings-security")).toBe(true);

    if (name === "phone") await app.press("id:settings-back");
    await app.clear("id:settings-search");
    await app.type("id:settings-search", "zzzz");
    await app.waitFor("id:settings-nav-empty");
    expect(app.texts()).toContain("No matching settings.");
    if (name === "desktop") expect(app.texts()).toContain("No settings match that search.");

    await app.clear("id:settings-search");
    await app.waitFor("id:settings-nav-jobs");
    expect(app.shows("id:settings-nav-computer")).toBe(true);
  }, 60_000);

  test(`${name}: a taxpayer who signed in sees no Jobs and no Computer, and only Export under Import / Export`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: "taxpayer" });
    await app.waitFor("id:settings-nav-general");
    expect(app.shows("id:settings-nav-jobs")).toBe(false);
    expect(app.shows("id:settings-nav-computer")).toBe(false);
    expect(app.shows("id:settings-nav-transfer")).toBe(true);
    await goSection(app, "transfer", "id:tp-export");
    expect(app.shows("id:settings-import-btn")).toBe(false);
    expect(app.texts()).toContain("Export");
    // A search that would keep only admin rows finds nothing for them.
    if (name === "phone") await app.press("id:settings-back");
    await app.type("id:settings-search", "retention");
    await app.waitFor("id:settings-nav-empty");
  }, 60_000);
}

test("phone: the list comes first, a section is its own page with Back, and the page shows its title", async () => {
  const app = await openSettings({}, {});
  await app.waitFor("id:settings-nav-account");
  expect(app.shows("id:settings-back")).toBe(false);
  expect(app.shows("id:settings-appearance")).toBe(false);
  await goSection(app, "account", "id:settings-security");
  expect(app.shows("id:settings-back")).toBe(true);
  expect(app.shows("id:settings-nav-account")).toBe(false);
  expect(app.texts()).toContain("Account");
  await app.press("id:settings-back");
  await app.waitFor("id:settings-nav-account");
  expect(app.shows("id:settings-security")).toBe(false);
  // A section asked for by name opens at once.
  const direct = await openSettings({}, {}, "computer");
  await direct.waitFor("id:settings-files");
  expect(direct.shows("id:settings-back")).toBe(true);
  expect(direct.texts()).toContain("Device");
  expect(direct.texts()).toContain("Device setup");
  expect(direct.shows("id:settings-hotkey-recorder")).toBe(false);
});

for (const platform of ["ios", "android"] as const) {
  test(`${platform}: desktop-only settings are absent, while local files and device setup remain`, async () => {
    const app = await openSettings({ platform });
    await app.waitFor("id:settings-nav-computer");
    expect(app.texts()).toContain("Device");
    expect(app.shows("id:settings-nav-keyboard")).toBe(false);
    await goSection(app, "general", "id:settings-theme-row");
    for (const id of ["settings-recent", "settings-compact-form-nav", "settings-auto-update"]) expect(app.shows(`id:${id}`)).toBe(false);
    await app.press("id:settings-back");
    await goSection(app, "computer", "id:settings-files");
    expect(app.shows("id:settings-retention-row")).toBe(true);
    expect(app.shows("id:settings-setup")).toBe(true);
    expect(app.shows("id:settings-hotkey-recorder")).toBe(false);
    expect(app.shows("id:settings-quit-key")).toBe(false);
    await app.press("id:settings-back");
    await app.type("id:settings-search", "device");
    await app.waitFor("id:settings-nav-computer");
    await goSection(app, "computer", "id:settings-files");
    expect(app.shows("id:settings-setup")).toBe(true);
    await app.press("id:settings-back");
    for (const query of ["keyboard", "hotkey", "sidebar", "compact", "download", "tray", "menu bar", "close window", "launch at login", "file associations"]) {
      await app.clear("id:settings-search");
      await app.type("id:settings-search", query);
      await app.waitFor("id:settings-nav-empty");
    }
  }, 60_000);
}

test("desktop: the section list stays beside the cards, a section opens without leaving the page, and the page returns to the last one", async () => {
  const app = await openSettings({ width: 1440, height: 900, platform: "macos" }, {});
  await app.waitFor("id:settings-appearance");
  expect(app.shows("id:settings-back")).toBe(false);
  await goSection(app, "jobs", "id:jobs-section");
  expect(app.shows("id:settings-nav-general")).toBe(true);
  expect(app.shows("id:settings-appearance")).toBe(false);
  // Away and back by the account menu: Settings opens on the section it was left on.
  await app.press("id:nav-home");
  await app.waitFor("id:home-dashboard");
  await app.press("id:account-row");
  await app.press("id:menu-account");
  await app.waitFor("id:settings");
  expect(app.shows("id:jobs-section")).toBe(true);
});

test("desktop: the old Background tasks address opens Jobs", async () => {
  const app = await openSettings({ width: 1440, height: 900, platform: "macos" }, { start: "view=cron_tasks" });
  await app.waitFor("id:jobs-section");
  expect(state(app)).toContain('view=settings {"section":"jobs"}');
});

for (const { name, size } of SIZES) {
  test(`${name}: Import asks the host; each answer says what happened, and a cancelled dialog says nothing`, async () => {
    const cases: [string, string | null][] = [
      ["database", "Database imported."],
      ["profile", "Taxpayer data restored."],
      ["invalid", "That zip has no taxpayer or database export in it."],
      ["fail", "Import failed: The backup's database could not be restored. The current database was left as it is."],
      ["cancelled", null],
    ];
    for (const [kind, toastText] of cases) {
      const app = await openSettings(size, { import: kind }, "transfer");
      await app.waitFor("id:settings-import-btn");
      expect(app.enabled("id:settings-import-btn")).toBe(true);
      await app.press("id:settings-import-btn");
      await app.waitFor("text:calls: import_archive");
      if (toastText) await app.waitFor(`text:${toastText}`);
      else expect(app.texts().some((t) => /imported|restored|failed/i.test(t))).toBe(false);
      // The button is free again once the host has answered.
      await app.waitFor("text:Import", { timeout: 5000 });
      expect(app.enabled("id:settings-import-btn")).toBe(true);
    }
  }, 120_000);

  test(`${name}: Export is the Tax profile's card; with no taxpayer there is nothing to export`, async () => {
    const app = await openSettings(size, {}, "transfer");
    await app.waitFor("id:tp-export");
    expectTexts(app, ["Import", "Import data", "Export", "Export this profile"]);
    const empty = await openSettings(size, { demo: false }, "transfer");
    await empty.waitFor("id:settings-import-btn");
    expect(empty.shows("id:tp-export")).toBe(false);
    // A search for the export row alone, with no taxpayer and no import row for it: the sentence says why.
    const lone = await openSettings(size, { demo: false, solo: false }, undefined);
    await lone.type("id:settings-search", "backup");
    await lone.waitFor("id:settings-nav-transfer");
    await goSection(lone, "transfer", "id:settings-transfer-empty");
    expect(lone.texts()).toContain("Add a taxpayer before exporting a profile.");
  }, 90_000);
}
