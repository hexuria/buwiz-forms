import { describe, expect, test } from "nsh/test";
import {
  THEME_OPTIONS,
  accountSummary,
  hits,
  hotkeyLabel,
  hotkeyOptions,
  queryWords,
  rowShown,
  retentionLabel,
  sectionById,
  settingsView,
  shownSection,
} from "./settings_model";

// Translated from tools/test_settings_window.mjs (the search, the account card, the hotkey and retention labels).
const rowsOf = (view: ReturnType<typeof settingsView>, id: string) => view.find((section) => section.id === id)?.rows;

describe("settingsView", () => {
  test("an admin sees every section and every row", () => {
    const view = settingsView("", true);
    expect(view.map((section) => section.id)).toEqual(["general", "account", "transfer", "jobs", "computer"]);
    expect(rowsOf(view, "general")).toEqual(["theme", "recent", "compact", "updates"]);
    expect(rowsOf(view, "transfer")).toContain("import");
  });
  test("a non-admin sees no admin-only section and no admin-only row", () => {
    const view = settingsView("", false);
    expect(view.map((section) => section.id)).toEqual(["general", "account", "transfer"]);
    expect(rowsOf(view, "transfer")).toEqual(["export"]);
  });
  test("a non-admin does not see the import row even when searching for it", () => {
    expect(settingsView("import", false)).toEqual([
      { id: "transfer", label: "Import / Export", icon: "archive", group: "Data", rows: ["export"] },
    ]);
  });
  test("a query keeps the rows whose own words match", () => {
    expect(settingsView("theme", true).map((s) => [s.id, s.rows])).toEqual([["general", ["theme"]]]);
    expect(settingsView("buwiz", true).map((s) => [s.id, s.rows])).toEqual([["account", ["buwiz"]]]);
    expect(settingsView("HOTKEY", true).map((s) => [s.id, s.rows])).toEqual([["computer", ["hotkey"]]]);
  });
  test("a section word keeps every row of that section", () => {
    const view = settingsView("personal", true);
    expect(view.map((s) => s.id)).toEqual(["general", "account"]);
    expect(rowsOf(view, "general")).toEqual(["theme", "recent", "compact", "updates"]);
  });
  test("a section named by the query keeps all its rows, even ones whose own words do not match", () => {
    expect(rowsOf(settingsView("account", true), "account")).toEqual(["buwiz", "identity", "google", "security"]);
  });
  test("every word of the query must match: a word that matches nothing leaves nothing", () => {
    expect(settingsView("theme zzz", true)).toEqual([]);
  });
  test("a query matching nothing shows no section", () => {
    expect(settingsView("zzzz-not-a-setting", true)).toEqual([]);
  });
});

describe("shownSection", () => {
  test("the selected section is drawn when the view still shows it; otherwise the first one", () => {
    const view = settingsView("", true);
    expect(shownSection("account", view)).toBe("account");
    expect(shownSection("jobs", settingsView("", false))).toBe("general");
    expect(shownSection(null, view)).toBe("general");
    expect(shownSection("general", [])).toBeNull();
  });
});

describe("sectionById and rowShown", () => {
  test("a section by its id, or null", () => {
    const view = settingsView("", true);
    expect(sectionById(view, "account")?.label).toBe("Account");
    expect(sectionById(view, "nope")).toBeNull();
  });
  test("a row is drawn only when its section keeps it", () => {
    const general = sectionById(settingsView("", true), "general");
    expect(rowShown(general, "theme")).toBe(true);
    expect(rowShown(general, "import")).toBe(false);
    expect(rowShown(null, "theme")).toBe(false);
  });
});

describe("queryWords and hits", () => {
  test("the words of a query are its lower-case pieces, split on spaces", () => {
    expect(queryWords("  Gmail   CONNECT ")).toEqual(["gmail", "connect"]);
    expect(queryWords("")).toEqual([]);
    expect(queryWords(null)).toEqual([]);
  });
  test("every word of the query must occur in the text", () => {
    expect(hits("gmail connect", "google gmail connect disconnect")).toBe(true);
    expect(hits("gmail calendar", "google gmail connect")).toBe(false);
  });
  test("an empty query matches everything, even no text", () => {
    expect(hits("", "anything")).toBe(true);
    expect(hits("   ", null)).toBe(true);
  });
  test("a query with words matches nothing in an empty text", () => {
    expect(hits("x", "")).toBe(false);
    expect(hits("x", null)).toBe(false);
  });
});

describe("hotkeyLabel and hotkeyOptions", () => {
  const presets = [{ value: "super+shift+E", label: "Super+Shift+E" }];
  test("empty is Off; a preset reads with its label; an unknown chord reads as stored", () => {
    expect(hotkeyLabel("", [])).toBe("Off");
    expect(hotkeyLabel("  ", presets)).toBe("Off");
    expect(hotkeyLabel(null, presets)).toBe("Off");
    expect(hotkeyLabel("super+shift+E", presets)).toBe("Super+Shift+E");
    expect(hotkeyLabel("ctrl+alt+Z", [])).toBe("ctrl+alt+Z");
  });
  test("the options start with Off, then the presets, and a custom chord is kept at the end", () => {
    expect(hotkeyOptions("ctrl+alt+Z", presets)).toEqual([
      { value: "", label: "Off" },
      { value: "super+shift+E", label: "Super+Shift+E" },
      { value: "ctrl+alt+Z", label: "ctrl+alt+Z" },
    ]);
  });
  test("a chord that is a preset is not repeated; no chord gives Off and the presets", () => {
    expect(hotkeyOptions("super+shift+E", presets).length).toBe(2);
    expect(hotkeyOptions("", presets)).toEqual([{ value: "", label: "Off" }, ...presets]);
    expect(hotkeyOptions(undefined, undefined)).toEqual([{ value: "", label: "Off" }]);
  });
});

describe("retentionLabel", () => {
  test("one day is singular; more is plural (from test_settings_window)", () => {
    expect(retentionLabel(1)).toBe("1 day");
    expect(retentionLabel(14)).toBe("14 days");
  });
  test("zero, negative, blank and unreadable mean the seven-day default", () => {
    expect(retentionLabel(0)).toBe("7 days");
    expect(retentionLabel(-3)).toBe("7 days");
    expect(retentionLabel(undefined)).toBe("7 days");
    expect(retentionLabel(null)).toBe("7 days");
    expect(retentionLabel("abc")).toBe("7 days");
  });
  test("a number held as text is read as its number", () => {
    expect(retentionLabel("30")).toBe("30 days");
  });
});

describe("THEME_OPTIONS", () => {
  test("system, light and dark, in that order", () => {
    expect(THEME_OPTIONS.map((option) => option.value)).toEqual(["system", "light", "dark"]);
  });
});

describe("accountSummary", () => {
  const taxpayers = [{ tin_root: "123456788", name: "Example Trading", tin_display: "123-456-788-00000" }];
  const branch = { tin: "12345678800000", name: "Head office", facts: { email: "office@example.com" } };
  test("a help desk admin is not the taxpayer: no email on the card; the mailbox keeps the branch's", () => {
    const card = accountSummary({
      isAdmin: true,
      isHelpdesk: true,
      adminRoot: "000000000",
      sessionKind: "admin",
      taxpayers,
      branch,
      canAddTaxpayer: true,
    });
    expect(card.name).toBe("Help desk admin");
    expect(card.detail).toBe("TIN 000-000-000");
    expect(card.email).toBe("");
    expect(card.mailbox).toEqual({ tin: "12345678800000", label: "Head office", email: "office@example.com" });
    expect(card.showAdd).toBe(true);
  });
  test("an owner admin is the taxpayer: the admin line and the branch's email", () => {
    const card = accountSummary({
      isAdmin: true,
      isHelpdesk: false,
      adminRoot: "123456788",
      sessionKind: "admin",
      taxpayers,
      branch,
      canAddTaxpayer: false,
    });
    expect(card.name).toBe("Example Trading");
    expect(card.email).toBe("office@example.com");
    expect(card.detail).toBe("Admin · this computer");
    expect(card.showAdd).toBe(false);
  });
  test("a taxpayer sign-in shows that taxpayer and its TIN", () => {
    const card = accountSummary({ isAdmin: false, sessionKind: "taxpayer", sessionRoot: "123456788", taxpayers, branch });
    expect(card.name).toBe("Example Trading");
    expect(card.detail).toBe("TIN 123-456-788-00000");
    expect(card.email).toBe("office@example.com");
  });
  test("nobody signed in is the local account on this computer, with no mailbox and no add", () => {
    expect(accountSummary({})).toEqual({ name: "Local account", detail: "This computer", email: "", mailbox: null, showAdd: false });
  });
  test("a taxpayer session for a taxpayer not listed keeps the defaults", () => {
    const card = accountSummary({ sessionKind: "taxpayer", sessionRoot: "999999999", taxpayers: [] });
    expect(card.name).toBe("Local account");
    expect(card.detail).toBe("This computer");
  });
  test("a branch with no name is labelled with the account's name; a branch with no email has none", () => {
    const card = accountSummary({ isAdmin: true, adminRoot: "123456788", taxpayers, branch: { tin: "1", name: "", facts: null } });
    expect(card.mailbox).toEqual({ tin: "1", label: "Example Trading", email: "" });
    expect(card.email).toBe("");
  });
  test("a taxpayer with no TIN to show keeps the 'This computer' detail", () => {
    const card = accountSummary({
      sessionKind: "taxpayer",
      sessionRoot: "123456788",
      taxpayers: [{ tin_root: "123456788", name: "Example Trading" }],
    });
    expect(card.detail).toBe("This computer");
  });
});
