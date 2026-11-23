import { describe, expect, test } from "nsh/test";
import { allChords, CHORD_KEYS, CHORD_MODIFIERS, EXTRA_KEYS } from "../ui/chords";
import {
  THEME_OPTIONS,
  accountSummary,
  hits,
  HOTKEY_KEYS,
  HOTKEY_NSH_KEYS,
  comboToKeystroke,
  hotkeyProblem,
  hotkeyRegistrable,
  keystrokeToCombo,
  modifierWords,
  queryWords,
  rowShown,
  retentionLabel,
  sectionById,
  settingsView,
  shownSection,
} from "./settings_model";

test("Mac App Store Settings does not show the global hotkey or update controls", () => {
  const view = settingsView("", true, false, true);
  expect(rowsOf(view, "computer")).not.toContain("hotkey");
  expect(rowsOf(view, "general")).not.toContain("updates");
});

// Translated from tools/test_settings_window.mjs (the search, the account card, the hotkey and retention labels).
const rowsOf = (view: ReturnType<typeof settingsView>, id: string) => view.find((section) => section.id === id)?.rows;

describe("settingsView", () => {
  test("phones omit desktop-only sections and rows before searching", () => {
    const view = settingsView("", true, true);
    expect(view.map((section) => section.id)).toEqual(["general", "account", "transfer", "jobs", "computer"]);
    expect(rowsOf(view, "general")).toEqual(["theme"]);
    expect(rowsOf(view, "computer")).toEqual(["data", "logs", "setup", "version"]);
    for (const query of ["keyboard", "hotkey", "shortcut", "sidebar", "compact", "download", "tray", "menu bar", "close window", "launch at login", "file associations"]) {
      expect(settingsView(query, true, true)).toEqual([]);
    }
    expect(rowsOf(settingsView("general", true, true), "general")).toEqual(["theme"]);
    expect(settingsView("device", true, true).find((section) => section.id === "computer")?.label).toBe("Device");
  });
  test("an admin sees every section and every row", () => {
    const view = settingsView("", true);
    expect(view.map((section) => section.id)).toEqual(["general", "account", "transfer", "jobs", "keyboard", "computer"]);
    expect(rowsOf(view, "keyboard")).toEqual(["quit", "hide", "close"]);
    expect(rowsOf(view, "general")).toEqual(["theme", "recent", "compact", "updates"]);
    expect(rowsOf(view, "transfer")).toContain("import");
  });
  test("a non-admin sees no admin-only section and no admin-only row", () => {
    const view = settingsView("", false);
    expect(view.map((section) => section.id)).toEqual(["general", "account", "transfer", "keyboard"]);
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
    expect(settingsView("tray", false).map((s) => [s.id, s.rows])).toEqual([["keyboard", ["hide", "close"]]]);
    expect(settingsView("keyboard", true).map((s) => [s.id, s.rows])).toEqual([["keyboard", ["quit", "hide", "close"]], ["computer", ["hotkey"]]]);
    // "eBIRForms" also names the app (the version row); in Import / Export it is the import from the old program.
    expect(rowsOf(settingsView("eBIRForms", true), "transfer")).toEqual(["import"]);
    expect(settingsView("eBIRForms folder", true).map((s) => [s.id, s.rows])).toEqual([["transfer", ["import"]]]);
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

describe("the global hotkey: stored combo and keystroke", () => {
  /** Every subset of control, alt, shift, super (as stored, in that order) with one key. */
  const subsets = (): string[][] => {
    const words = ["control", "alt", "shift", "super"];
    const out: string[][] = [];
    for (let set = 1; set < 16; set++) out.push(words.filter((_, i) => (set & (1 << i)) !== 0));
    return out;
  };
  test("a stored combo is the keystroke keymap names: control+alt+E is ctrl+alt+e, super is cmd", () => {
    expect(comboToKeystroke("control+alt+E", "macos")).toBe("ctrl+alt+e");
    expect(comboToKeystroke("super+shift+F12", "linux")).toBe("shift+cmd+f12");
    expect(comboToKeystroke("ctrl+option+9", "macos")).toBe("ctrl+alt+9");
    expect(comboToKeystroke("Platform+Win+meta+cmd+b", "windows")).toBe("cmd+b");
    expect(comboToKeystroke(" control + alt + h ", "macos")).toBe("ctrl+alt+h");
  });
  test("a bare stored key gets the platform's default modifiers: control+alt on macOS, super+shift elsewhere", () => {
    expect(comboToKeystroke("E", "macos")).toBe("ctrl+alt+e");
    expect(comboToKeystroke("e", "macos")).toBe("ctrl+alt+e");
    expect(comboToKeystroke("F12", "windows")).toBe("shift+cmd+f12");
    expect(comboToKeystroke("B", "linux")).toBe("shift+cmd+b");
  });
  test("words that are no modifier are ignored; what has no key of the list is null", () => {
    expect(comboToKeystroke("control+hyper+alt+E", "macos")).toBe("ctrl+alt+e");
    expect(comboToKeystroke("hyper+E", "macos")).toBe("e");
    expect(comboToKeystroke("", "macos")).toBeNull();
    expect(comboToKeystroke(null, "macos")).toBeNull();
    expect(comboToKeystroke(undefined, "macos")).toBeNull();
    expect(comboToKeystroke(" + ", "macos")).toBeNull();
    expect(comboToKeystroke("control+notakey", "macos")).toBeNull();
    expect(comboToKeystroke("control+alt+F21", "macos")).toBeNull();
    // The character is never a stored key ("+" is the separator), and a word of the old list's neighbours is no key.
    for (const key of ["-", "=", "[", "tab", "enter", "escape", "insert", "bracketleft", "arrowleft", "plus"]) {
      expect(comboToKeystroke(`control+alt+${key}`, "macos")).toBeNull();
    }
  });
  test("a keystroke is the stored combo: ctrl+alt+e is control+alt+E, cmd is super, the order is control, alt, shift, super", () => {
    expect(keystrokeToCombo("ctrl+alt+e")).toBe("control+alt+E");
    expect(keystrokeToCombo("cmd+shift+f12")).toBe("shift+super+F12");
    expect(keystrokeToCombo("shift+cmd+ctrl+alt+0")).toBe("control+alt+shift+super+0");
    expect(keystrokeToCombo("Command+Option+B")).toBe("alt+super+B");
    expect(keystrokeToCombo("win+h")).toBe("super+H");
  });
  test("no modifier (it would read as the platform's defaults), a modifier it cannot read, or a key outside the list is null", () => {
    expect(keystrokeToCombo("e")).toBeNull();
    expect(keystrokeToCombo("mod+e")).toBeNull();
    expect(keystrokeToCombo("space")).toBeNull();
    expect(keystrokeToCombo("shift+left")).not.toBeNull();
    expect(keystrokeToCombo("ctrl+f21")).toBeNull();
    expect(keystrokeToCombo("ctrl+tab")).toBeNull();
    expect(keystrokeToCombo("ctrl+enter")).toBeNull();
    expect(keystrokeToCombo("ctrl+escape")).toBeNull();
    expect(keystrokeToCombo("ctrl+")).toBeNull();
    expect(keystrokeToCombo("")).toBeNull();
    expect(keystrokeToCombo(null)).toBeNull();
  });
  test("round trip: every modifier subset with every key, stored to keystroke and back", () => {
    expect(HOTKEY_KEYS.length).toBe(48);
    let n = 0;
    for (const words of subsets()) {
      for (const key of HOTKEY_KEYS) {
        const stored = [...words, key].join("+");
        const keystroke = comboToKeystroke(stored, "macos");
        expect(keystroke).not.toBeNull();
        expect(keystrokeToCombo(keystroke)).toBe(stored);
        // And the other way: the keystroke's own spelling comes back unchanged.
        expect(comboToKeystroke(keystrokeToCombo(keystroke), "linux")).toBe(keystroke);
        n++;
      }
    }
    expect(n).toBe(15 * 48);
  });
  test("the keys only this app registers: space, the arrows, home, end, page up and down, punctuation, F13 to F20, each a word, never the character", () => {
    // The words and how keymap spells each (src/desktop.rs key_name and hotkey_combo.rs NSH_KEYS hold the same list; hotkey.test.ts asks the host).
    const pairs: [string, string][] = [
      ["SPACE", "space"], ["UP", "up"], ["DOWN", "down"], ["LEFT", "left"], ["RIGHT", "right"], ["HOME", "home"], ["END", "end"],
      ["PAGEUP", "pageup"], ["PAGEDOWN", "pagedown"], ["MINUS", "minus"], ["EQUAL", "plus"], ["LEFTBRACKET", "["], ["RIGHTBRACKET", "]"],
      ["BACKSLASH", "\\"], ["SEMICOLON", ";"], ["QUOTE", "'"], ["COMMA", ","], ["PERIOD", "."], ["SLASH", "/"], ["BACKTICK", "`"],
      ["F13", "f13"], ["F14", "f14"], ["F15", "f15"], ["F16", "f16"], ["F17", "f17"], ["F18", "f18"], ["F19", "f19"], ["F20", "f20"],
    ];
    expect(Object.entries(HOTKEY_NSH_KEYS)).toEqual(pairs);
    for (const [word] of pairs) expect(HOTKEY_KEYS).not.toContain(word);
    for (const [word, key] of pairs) {
      expect(comboToKeystroke(`control+alt+${word}`, "macos")).toBe(`ctrl+alt+${key}`);
      expect(keystrokeToCombo(`ctrl+alt+${key}`)).toBe(`control+alt+${word}`);
    }
    expect(comboToKeystroke("control+alt+space", "macos")).toBe("ctrl+alt+space");
    expect(comboToKeystroke("super+shift+LEFT", "windows")).toBe("shift+cmd+left");
    expect(keystrokeToCombo("cmd+minus")).toBe("super+MINUS");
    expect(keystrokeToCombo("ctrl+alt+shift+cmd+f20")).toBe("control+alt+shift+super+F20");
    // A bare stored NSH key takes the platform's default modifiers like any other.
    expect(comboToKeystroke("SPACE", "macos")).toBe("ctrl+alt+space");
    expect(comboToKeystroke("COMMA", "linux")).toBe("shift+cmd+,");
  });
  test("keymap's other spellings of the same keys mean the same word", () => {
    expect(keystrokeToCombo("ctrl+-")).toBe("control+MINUS");
    expect(keystrokeToCombo("ctrl+=")).toBe("control+EQUAL");
    expect(keystrokeToCombo("ctrl+plus")).toBe("control+EQUAL");
    expect(keystrokeToCombo("ctrl+arrowleft")).toBe("control+LEFT");
    expect(keystrokeToCombo("ctrl+ArrowUp")).toBe("control+UP");
    expect(keystrokeToCombo("Ctrl+Alt+SPACE")).toBe("control+alt+SPACE");
    // The recorder's own spelling is the one that comes back.
    expect(comboToKeystroke(keystrokeToCombo("ctrl+-"), "macos")).toBe("ctrl+minus");
    expect(comboToKeystroke(keystrokeToCombo("ctrl+="), "macos")).toBe("ctrl+plus");
  });
  test("round trip: every modifier subset with every key only this app registers, stored to keystroke and back", () => {
    const words = Object.keys(HOTKEY_NSH_KEYS);
    expect(words.length).toBe(28);
    let n = 0;
    for (const mods of subsets()) {
      for (const key of words) {
        const stored = [...mods, key].join("+");
        const keystroke = comboToKeystroke(stored, "macos");
        expect(keystroke).not.toBeNull();
        expect(keystrokeToCombo(keystroke)).toBe(stored);
        expect(comboToKeystroke(keystrokeToCombo(keystroke), "linux")).toBe(keystroke);
        n++;
      }
    }
    expect(n).toBe(15 * 28);
  });
  test("every chord the recorder offers is one the page can store, and writes back as it was recorded", () => {
    expect(CHORD_KEYS.length).toBe(HOTKEY_KEYS.length + Object.keys(HOTKEY_NSH_KEYS).length);
    expect(EXTRA_KEYS.length).toBe(28);
    for (const chord of allChords()) {
      const stored = keystrokeToCombo(chord);
      // A chord with no modifier at all is not a stored combo (a bare stored key would mean the platform's modifiers).
      if (!chord.includes("+")) {
        expect(stored).toBeNull();
        continue;
      }
      expect(stored).not.toBeNull();
      expect(comboToKeystroke(stored, "macos")).toBe(chord);
    }
    expect(CHORD_MODIFIERS.length).toBe(4);
  });
  test("the four old presets (the host's chords) read as the chord they are; macOS's write back byte for byte", () => {
    // macOS: control+alt+E ... are in the order the recorder stores (control, alt, shift, super), so nothing stored changes.
    for (const key of ["E", "B", "H", "F12"]) {
      const preset = `control+alt+${key}`;
      const keystroke = comboToKeystroke(preset, "macos");
      expect(keystroke).toBe(`ctrl+alt+${key.toLowerCase()}`);
      expect(keystrokeToCombo(keystroke)).toBe(preset);
      expect(hotkeyRegistrable(keystroke)).toBe(true);
    }
    // Elsewhere the host writes super+shift+E: the same chord as shift+super+E, which is how the recorder writes it (same words, the
    // listeners read them in any order), so a stored preset reads the same and only a chord recorded again is written in the new order.
    for (const key of ["E", "B", "H", "F12"]) {
      const preset = `super+shift+${key}`;
      const keystroke = comboToKeystroke(preset, "windows");
      expect(keystroke).toBe(`shift+cmd+${key.toLowerCase()}`);
      expect(keystrokeToCombo(keystroke)).toBe(`shift+super+${key}`);
      expect(comboToKeystroke(`shift+super+${key}`, "windows")).toBe(keystroke);
      expect(hotkeyRegistrable(keystroke)).toBe(true);
    }
  });
  test("the door takes Control, Alt or Command with a key; Shift alone or a bare key it refuses", () => {
    expect(hotkeyRegistrable("ctrl+alt+e")).toBe(true);
    expect(hotkeyRegistrable("cmd+e")).toBe(true);
    expect(hotkeyRegistrable("alt+f1")).toBe(true);
    expect(hotkeyRegistrable("shift+cmd+9")).toBe(true);
    expect(hotkeyRegistrable("shift+e")).toBe(false);
    expect(hotkeyRegistrable("e")).toBe(false);
    expect(hotkeyRegistrable("f5")).toBe(false);
    expect(hotkeyRegistrable("space")).toBe(false);
    expect(hotkeyRegistrable("shift+space")).toBe(false);
    expect(hotkeyRegistrable("shift+left")).toBe(false);
    expect(hotkeyRegistrable("f13")).toBe(false);
    expect(hotkeyRegistrable("ctrl+alt+space")).toBe(true);
    expect(hotkeyRegistrable("cmd+,")).toBe(true);
    expect(hotkeyRegistrable("alt+f20")).toBe(true);
    expect(hotkeyRegistrable("ctrl+f21")).toBe(false);
    expect(hotkeyRegistrable("")).toBe(false);
  });
  test("what the box says about a chord with no modifier, in the platform's words; and about the host's answer", () => {
    expect(modifierWords("macos")).toBe("Add Control, Option or Command.");
    expect(modifierWords("ios")).toBe("Add Control, Option or Command.");
    expect(modifierWords("windows")).toBe("Add Ctrl, Alt or the Windows key.");
    expect(modifierWords("linux")).toBe("Add Ctrl, Alt or Super.");
    expect(hotkeyProblem("refused")).toBe("Another app is using this shortcut. Pick another one.");
    expect(hotkeyProblem("invalid")).toBe("This computer cannot register that shortcut.");
    for (const status of ["off", "pending", "registered"]) expect(hotkeyProblem(status)).toBe("");
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
