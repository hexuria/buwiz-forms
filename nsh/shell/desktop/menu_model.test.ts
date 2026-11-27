// The application menu as data: which menus and items exist, which are enabled, and their accelerators, in every state the app
// can be in (signed in with and without a taxpayer, a form open, locked) on each platform. A plain test: no window.
import { expect, test } from "nsh/test";
import {
  appKeyUsable, buildMenu, buildTrayMenu, chord, closePolicy, DEFAULT_CLOSE_PREFS, DEFAULT_KEYS, flatten, itemKeys, keyHolder, parseClosePrefs,
  printMenu, sendingWarning, type MenuAction, type MenuState, type Os,
} from "./menu_model";

const JUANA = { root: "000123453", name: "Juana Santos" };
const PEDRO = { root: "000777888", name: "Pedro Reyes" };

const OSES: Os[] = ["macos", "windows", "linux"];

function state(os: Os, over: Partial<MenuState> = {}): MenuState {
  return { os, unlocked: true, hasTaxpayer: true, formOpen: false, ...over };
}

function menusOf(s: MenuState, run: (a: MenuAction) => void = () => {}) {
  return buildMenu(s, run);
}

/** The menu bar's titles. */
function titles(s: MenuState): string[] {
  return menusOf(s).map((m) => ("label" in m ? (m.label ?? "") : ""));
}

/** "File > New Return…  mod+n" lines of one top-level menu. */
function linesOf(s: MenuState, menu: string): string[] {
  const found = menusOf(s).find((m) => "label" in m && m.label === menu);
  return found && "label" in found ? printMenu([found]).map((l) => l.slice(menu.length + 3)) : [];
}

/** The items of one top-level menu, in order, with "-" for a separator. */
function shape(s: MenuState, menu: string): string[] {
  const found = menusOf(s).find((m) => "label" in m && m.label === menu);
  if (!found || !("submenu" in found) || !found.submenu) return [];
  return found.submenu.map((e) => (!e ? "?" : "type" in e ? "-" : (e.label ?? "role:" + e.role)));
}

test("macOS signed in: the same menus, items and accelerators as the old native menu bar", () => {
  const s = state("macos");
  expect(titles(s)).toEqual(["eBIRForms", "File", "Edit", "View", "Window", "Help", "Profiles"]);
  expect(shape(s, "eBIRForms")).toEqual([
    "About eBIRForms",
    "-",
    "Settings…",
    "-",
    "Hide eBIRForms",
    "role:hideOthers",
    "role:showAll",
    "-",
    "Sign Out",
    "-",
    "Quit eBIRForms",
  ]);
  expect(linesOf(s, "eBIRForms")).toContain("Settings…  mod+,");
  expect(linesOf(s, "eBIRForms")).toContain("Hide eBIRForms  mod+h");
  expect(linesOf(s, "eBIRForms")).toContain("Quit eBIRForms  mod+q");
  expect(shape(s, "File")).toEqual(["New tax profile…", "New Return…", "-", "Close Window"]);
  expect(linesOf(s, "File")).toEqual(["New tax profile…", "New Return…  mod+n", "Close Window  mod+w"]); // Cmd+N stays New Return…
  expect(shape(s, "Edit")).toEqual(["role:undo", "role:redo", "-", "role:cut", "role:copy", "role:paste", "-", "role:selectAll"]);
  expect(shape(s, "View")).toEqual(["Toggle Sidebar", "Toggle Tax Profile", "Toggle Notifications", "Toggle Theme", "-", "Command Palette…"]);
  expect(linesOf(s, "View")).toEqual([
    "Toggle Sidebar  mod+b",
    "Toggle Tax Profile  mod+shift+b",
    "Toggle Notifications  mod+shift+n",
    "Toggle Theme  mod+t",
    "Command Palette…  mod+k",
  ]);
  expect(shape(s, "Window")).toEqual(["role:minimize", "role:zoom", "-", "Bring All to Front"]);
  expect(shape(s, "Help")).toEqual(["Check for Updates…", "Contact Support…", "eBIRForms Website"]);
  // AppKit adds its own Enter Full Screen to View: ours would show it twice.
  expect(flatten(menusOf(s)).some((l) => l.path.includes("Full Screen"))).toBe(false);
});

test("Windows and Linux signed in: the eBIRForms menu first, File holds the app commands, Help holds About, View has Full Screen on F11", () => {
  const win = state("windows");
  expect(titles(win)).toEqual(["eBIRForms", "File", "Edit", "View", "Window", "Help", "Profiles"]);
  expect(shape(win, "File")).toEqual(["New tax profile…", "New Return…", "-", "Settings…", "Sign Out", "-", "Hide eBIRForms", "-", "Quit eBIRForms"]);
  expect(linesOf(win, "File")).toContain("Settings…  mod+,");
  // Hide and Quit keep their keys in the eBIRForms menu (Ctrl+Q quits on Windows too); File's copies carry none.
  expect(linesOf(win, "File")).toContain("Quit eBIRForms");
  expect(linesOf(win, "File")).toContain("Hide eBIRForms");
  expect(linesOf(win, "eBIRForms")).toContain("Quit eBIRForms  mod+q");
  expect(shape(win, "Edit")).not.toContain("Settings…");
  expect(shape(win, "Help")).toEqual(["About eBIRForms", "-", "Check for Updates…", "Contact Support…", "eBIRForms Website"]);
  expect(linesOf(win, "View")).toContain("Enter Full Screen  f11");
  expect(shape(win, "Window")).toEqual(["role:minimize", "role:zoom"]);

  const linux = state("linux");
  expect(titles(linux)).toEqual(["eBIRForms", "File", "Edit", "View", "Window", "Help", "Profiles"]);
  expect(shape(linux, "File")).toEqual(["New tax profile…", "New Return…", "-", "Close Window", "Hide eBIRForms", "-", "Sign Out", "Quit eBIRForms"]);
  expect(linesOf(linux, "File")).toContain("Quit eBIRForms");
  expect(linesOf(linux, "eBIRForms")).toContain("Quit eBIRForms  mod+q");
  expect(linesOf(linux, "eBIRForms")).toContain("Hide eBIRForms  mod+h");
  expect(shape(linux, "Edit").slice(-2)).toEqual(["-", "Settings…"]);
  expect(linesOf(linux, "Edit")).toContain("Settings…  mod+,");
});

test("an install its store updates has no Check for Updates: a Mac App Store build, a Microsoft Store package", () => {
  expect(shape(state("macos", { masBuild: true }), "Help")).toEqual(["Contact Support…", "eBIRForms Website"]);
  expect(shape(state("windows", { storeManaged: true }), "Help")).toEqual(["About eBIRForms", "-", "Contact Support…", "eBIRForms Website"]);
});

test("with no taxpayer the items that need one are there but disabled; nothing else changes", () => {
  for (const os of OSES) {
    const none = flatten(menusOf(state(os, { hasTaxpayer: false })));
    const some = flatten(menusOf(state(os, { hasTaxpayer: true })));
    expect(none.map((l) => l.path)).toEqual(some.map((l) => l.path));
    const disabled = none.filter((l) => l.disabled).map((l) => l.path);
    // "No taxpayers yet" is a note, never an item (no taxpayers listed in either state here).
    expect(disabled).toEqual(["File > New Return…", "View > Toggle Tax Profile", "Profiles > No taxpayers yet"]);
    expect(some.filter((l) => l.disabled).map((l) => l.path)).toEqual(["Profiles > No taxpayers yet"]);
  }
});

test("a form open changes nothing: its page owns the text fields, the menu never takes their keys", () => {
  for (const os of OSES) {
    for (const hasTaxpayer of [true, false]) {
      expect(printMenu(menusOf(state(os, { formOpen: true, hasTaxpayer })))).toEqual(printMenu(menusOf(state(os, { formOpen: false, hasTaxpayer }))));
    }
  }
});

test("locked (first-run setup, sign-in, the account gate): only About, Hide, Quit and editing are there", () => {
  const BANNED = [
    "Settings…",
    "New Return…",
    "Sign Out",
    "Command Palette…",
    "Check for Updates…",
    "Toggle Sidebar",
    "Toggle Tax Profile",
    "Toggle Notifications",
    "Toggle Theme",
    "Close Window",
    "New tax profile…",
    "No taxpayers yet",
  ];
  for (const os of OSES) {
    const lines = flatten(menusOf(state(os, { unlocked: false, hasTaxpayer: false })));
    for (const banned of BANNED) expect(lines.some((l) => l.path.endsWith("> " + banned))).toBe(false);
    expect(lines.some((l) => l.path.includes("About"))).toBe(true);
    expect(lines.some((l) => l.path.includes("Quit"))).toBe(true);
    // A locked app still hides and quits with its keys.
    expect(linesOf(state(os, { unlocked: false }), "eBIRForms")).toEqual(["About eBIRForms", "Hide eBIRForms  mod+h", "Quit eBIRForms  mod+q"]);
    expect(lines.filter((l) => l.disabled)).toEqual([]);
  }
  const mac = state("macos", { unlocked: false });
  expect(titles(mac)).toEqual(["eBIRForms", "Edit"]);
  expect(shape(mac, "eBIRForms")).toEqual(["About eBIRForms", "-", "Hide eBIRForms", "-", "Quit eBIRForms"]);
  expect(shape(mac, "Edit")).toContain("role:paste");
  expect(titles(state("windows", { unlocked: false }))).toEqual(["eBIRForms", "File", "Edit", "Help"]);
  expect(titles(state("linux", { unlocked: false }))).toEqual(["eBIRForms", "File", "Edit", "Help"]);
  for (const os of ["windows", "linux"] as Os[]) {
    expect(shape(state(os, { unlocked: false }), "eBIRForms")).toEqual(["About eBIRForms", "-", "Hide eBIRForms", "-", "Quit eBIRForms"]);
    expect(titles(state(os, { unlocked: false }))).not.toContain("Profiles");
  }
  expect(titles(mac)).not.toContain("Profiles");
  expect(shape(state("windows", { unlocked: false }), "File")).toEqual(["Hide eBIRForms", "Quit eBIRForms"]);
  expect(shape(state("linux", { unlocked: false }), "Help")).toEqual(["About eBIRForms"]);
  expect(shape(state("linux", { unlocked: false }), "Edit")).not.toContain("Settings…");
});

test("every state has a tidy menu: no stray separators, no shortcut used twice, a way to quit", () => {
  for (const os of OSES) {
    for (const unlocked of [true, false]) {
      for (const hasTaxpayer of [true, false]) {
        for (const formOpen of [true, false]) {
          for (const keys of [DEFAULT_KEYS, { quit: "alt+cmd+q", hide: "cmd+q" }, { quit: "ctrl+shift+q", hide: "ctrl+q" }, { quit: "", hide: "" }])
          for (const capturing of [false, true]) {
          const s = state(os, { unlocked, hasTaxpayer, formOpen, taxpayers: hasTaxpayer ? [JUANA, PEDRO] : [], currentTaxpayer: hasTaxpayer ? JUANA.root : null, keys, capturing });
          for (const menu of menusOf(s)) {
            if (!("submenu" in menu) || !menu.submenu) throw new Error("a top-level item has no submenu");
            const kinds = menu.submenu.map((e) => (!e ? "?" : "type" in e ? "-" : "item"));
            expect(kinds[0]).toBe("item");
            expect(kinds[kinds.length - 1]).toBe("item");
            expect(kinds.join("").includes("--")).toBe(false);
          }
          const shortcuts = flatten(menusOf(s)).flatMap((l) => (l.shortcut ? [chord(os, l.shortcut)] : []));
          expect(new Set(shortcuts).size).toBe(shortcuts.length);
          expect(flatten(menusOf(s)).some((l) => l.path === "eBIRForms > Quit eBIRForms")).toBe(true);
          }
        }
      }
    }
  }
});

test("each item runs the action its label says", () => {
  const ran: MenuAction[] = [];
  const lines = flatten(menusOf(state("macos"), (a) => ran.push(a)));
  const want: Record<string, MenuAction> = {
    "eBIRForms > About eBIRForms": "about",
    "eBIRForms > Settings…": "settings",
    "eBIRForms > Hide eBIRForms": "hide",
    "eBIRForms > Sign Out": "sign-out",
    "eBIRForms > Quit eBIRForms": "quit",
    "File > New tax profile…": "new-tax-profile",
    "File > New Return…": "new-return",
    "Profiles > New tax profile…": "new-tax-profile",
    "File > Close Window": "close-window",
    "View > Toggle Sidebar": "toggle-sidebar",
    "View > Toggle Tax Profile": "toggle-profile",
    "View > Toggle Notifications": "toggle-notifications",
    "View > Toggle Theme": "toggle-theme",
    "View > Command Palette…": "palette",
    "Window > Bring All to Front": "bring-all-to-front",
    "Help > Check for Updates…": "check-updates",
    "Help > Contact Support…": "support",
    "Help > eBIRForms Website": "website",
  };
  for (const [path, action] of Object.entries(want)) {
    ran.length = 0;
    const line = lines.find((l) => l.path === path);
    if (!line?.select) throw new Error("no item " + path);
    line.select();
    expect(ran).toEqual([action]);
  }
  // The editing and window items are the platform's own (roles): nothing of ours runs.
  expect(lines.filter((l) => l.select === null).map((l) => l.path)).toEqual([
    "eBIRForms > role:hideOthers",
    "eBIRForms > role:showAll",
    "Edit > role:undo",
    "Edit > role:redo",
    "Edit > role:cut",
    "Edit > role:copy",
    "Edit > role:paste",
    "Edit > role:selectAll",
    "Window > role:minimize",
    "Window > role:zoom",
    "Profiles > No taxpayers yet",
  ]);
});

test("the tray menu is Show, Hide and Quit on every platform; Show or Hide is disabled to match the window", () => {
  const ran: MenuAction[] = [];
  const up = flatten(buildTrayMenu((a) => ran.push(a), true));
  expect(up.map((l) => l.path)).toEqual(["Show eBIRForms", "Hide eBIRForms", "Quit eBIRForms"]);
  expect(printMenu(buildTrayMenu(() => {}, true))).toEqual(["Show eBIRForms  (disabled)", "Hide eBIRForms", "Quit eBIRForms"]);
  expect(printMenu(buildTrayMenu(() => {}, false))).toEqual(["Show eBIRForms", "Hide eBIRForms  (disabled)", "Quit eBIRForms"]);
  for (const line of up) line.select?.();
  expect(ran).toEqual(["show", "hide", "quit"]);
  // Quit is never disabled and carries no shortcut (the application menu has the keys).
  for (const visible of [true, false]) expect(flatten(buildTrayMenu(() => {}, visible)).every((l) => l.shortcut === null)).toBe(true);
});

test("the close policy per platform: macOS hides (the Dock icon goes), Windows and Linux ask or do the remembered choice; quit and hide keys everywhere", () => {
  expect(closePolicy("macos")).toEqual({ close: "hide", hideTakesDockIcon: true, quitShortcut: "mod+q", hideShortcut: "mod+h" });
  expect(closePolicy("windows")).toEqual({ close: "ask", hideTakesDockIcon: false, quitShortcut: "mod+q", hideShortcut: "mod+h" });
  expect(closePolicy("linux")).toEqual({ close: "ask", hideTakesDockIcon: false, quitShortcut: "mod+q", hideShortcut: "mod+h" });
  // "Don't ask again" remembered a choice: Windows and Linux follow it; macOS's red button always hides.
  for (const close of ["hide", "quit"] as const) {
    expect(closePolicy("windows", { ...DEFAULT_CLOSE_PREFS, close }).close).toBe(close);
    expect(closePolicy("linux", { ...DEFAULT_CLOSE_PREFS, close }).close).toBe(close);
    expect(closePolicy("macos", { ...DEFAULT_CLOSE_PREFS, close }).close).toBe("hide");
  }
  // Apple review: out of the box Cmd+Q is a real quit, and every platform's menu has a Quit item with it.
  for (const os of OSES) {
    const quit = flatten(menusOf(state(os))).find((l) => l.path === "eBIRForms > Quit eBIRForms");
    expect(quit?.shortcut).toBe("mod+q");
    const ran: MenuAction[] = [];
    flatten(menusOf(state(os), (a) => ran.push(a))).find((l) => l.path === "eBIRForms > Quit eBIRForms")?.select?.();
    expect(ran).toEqual(["quit"]);
  }
  // Super+Q is off on Linux until the recorder binds it.
  expect(flatten(menusOf(state("linux"))).some((l) => l.shortcut !== null && chord("linux", l.shortcut) === "cmd+q")).toBe(false);
  const superQ = state("linux", { keys: { quit: "cmd+q", hide: "mod+h" } });
  expect(linesOf(superQ, "eBIRForms")).toContain("Quit eBIRForms  cmd+q");
});

test("the stored choices are read back, and anything unknown is the default", () => {
  expect(parseClosePrefs(null)).toEqual(DEFAULT_CLOSE_PREFS);
  expect(parseClosePrefs("not json")).toEqual(DEFAULT_CLOSE_PREFS);
  expect(parseClosePrefs(JSON.stringify({ quit: "alt+cmd+q", hide: "cmd+q", close: "hide" }))).toEqual({ keys: { quit: "alt+cmd+q", hide: "cmd+q" }, close: "hide" });
  expect(parseClosePrefs(JSON.stringify({ quit: "", close: "quit" }))).toEqual({ keys: { quit: "", hide: "mod+h" }, close: "quit" });
  expect(parseClosePrefs(JSON.stringify({ quit: 4, close: "sometimes" }))).toEqual(DEFAULT_CLOSE_PREFS);
});

test("remapped keys show in the menu: Cmd+Q hides and Option+Cmd+Q quits", () => {
  const keys = { quit: "alt+cmd+q", hide: "cmd+q" };
  expect(linesOf(state("macos", { keys }), "eBIRForms")).toContain("Hide eBIRForms  cmd+q");
  expect(linesOf(state("macos", { keys }), "eBIRForms")).toContain("Quit eBIRForms  alt+cmd+q");
  // The same on the locked app menu, and on Windows and Linux in the eBIRForms menu.
  expect(linesOf(state("macos", { keys, unlocked: false }), "eBIRForms")).toEqual(["About eBIRForms", "Hide eBIRForms  cmd+q", "Quit eBIRForms  alt+cmd+q"]);
  const win = { quit: "ctrl+shift+q", hide: "ctrl+q" };
  for (const os of ["windows", "linux"] as Os[]) {
    expect(linesOf(state(os, { keys: win }), "eBIRForms")).toContain("Hide eBIRForms  ctrl+q");
    expect(linesOf(state(os, { keys: win }), "eBIRForms")).toContain("Quit eBIRForms  ctrl+shift+q");
  }
  // No key: the item stays, with no shortcut.
  expect(linesOf(state("macos", { keys: { quit: "mod+q", hide: "" } }), "eBIRForms")).toContain("Hide eBIRForms");
  // The policy carries the same keys.
  expect(closePolicy("macos", { keys, close: "ask" })).toEqual({ close: "hide", hideTakesDockIcon: true, quitShortcut: "alt+cmd+q", hideShortcut: "cmd+q" });
});

test("while a recorder listens, Quit and Hide carry the platform's own quit and hide keys (nsh/keymap cannot bind them)", () => {
  const keys = { quit: "alt+cmd+q", hide: "ctrl+cmd+h" };
  expect(itemKeys({ os: "macos", keys, capturing: true })).toEqual({ quit: "cmd+q", hide: "cmd+h" });
  expect(itemKeys({ os: "macos", keys, capturing: false })).toEqual({ quit: "alt+cmd+q", hide: "ctrl+cmd+h" });
  expect(itemKeys({ os: "windows", keys: DEFAULT_KEYS, capturing: true })).toEqual({ quit: "ctrl+q", hide: "mod+h" });
  // Hide on Ctrl+Q would be bound twice while Quit carries Ctrl+Q: it lets go of it.
  expect(itemKeys({ os: "linux", keys: { quit: "ctrl+shift+q", hide: "ctrl+q" }, capturing: true })).toEqual({ quit: "ctrl+q", hide: null });
});

test("a key another item, the platform, the frame or the global hotkey holds is refused; a free one is taken", () => {
  expect(keyHolder("macos", "hide", "cmd+k")).toBe("Command Palette…");
  expect(keyHolder("macos", "hide", "mod+q")).toBe("Quit eBIRForms");
  expect(keyHolder("macos", "quit", "cmd+h")).toBe("Hide eBIRForms");
  expect(keyHolder("macos", "quit", "cmd+w")).toBe("Close Window");
  expect(keyHolder("macos", "quit", "cmd+c")).toBe("Copy");
  expect(keyHolder("macos", "quit", "cmd+m")).toBe("Minimize");
  expect(keyHolder("macos", "quit", "cmd+l")).toBe("Lock");
  expect(keyHolder("windows", "quit", "ctrl+n")).toBe("New Return…");
  expect(keyHolder("windows", "quit", "alt+f4")).toBe("Close");
  expect(keyHolder("linux", "hide", "f11")).toBe("Enter Full Screen");
  expect(keyHolder("macos", "quit", "ctrl+alt+b", DEFAULT_KEYS, "ctrl+alt+b")).toBe("the global show hotkey");
  // Its own key is free to keep, and a swap goes in two steps: Quit to Option+Cmd+Q first, then Cmd+Q is free for Hide.
  expect(keyHolder("macos", "quit", "cmd+q")).toBe(null);
  expect(keyHolder("macos", "quit", "alt+cmd+q")).toBe(null);
  expect(keyHolder("macos", "hide", "cmd+q", { quit: "alt+cmd+q", hide: "mod+h" })).toBe(null);
  // Super+Q on Linux is free; a plain key or Shift with one is never a quit or hide key.
  expect(keyHolder("linux", "quit", "cmd+q")).toBe(null);
  expect(appKeyUsable("cmd+q")).toBe(true);
  expect(appKeyUsable("ctrl+h")).toBe(true);
  expect(appKeyUsable("shift+q")).toBe(false);
  expect(appKeyUsable("q")).toBe(false);
  expect(chord("macos", "Mod+Shift+K")).toBe("shift+cmd+k");
  expect(chord("windows", "cmd+alt+ctrl+-")).toBe("ctrl+alt+cmd+minus");
});

test("the warning before quitting while returns are still sending", () => {
  expect(sendingWarning(0)).toBe(null);
  expect(sendingWarning(1)).toEqual({ title: "1 return is still sending", message: "Quitting stops them. Hide keeps them going in the background." });
  expect(sendingWarning(2)?.title).toBe("2 returns are still sending");
});

test("Windows and Linux: the eBIRForms menu is About, Settings…, Hide, Sign Out, Quit; Hide and Quit show their keys", () => {
  for (const os of ["windows", "linux"] as Os[]) {
    const s = state(os);
    expect(shape(s, "eBIRForms")).toEqual(["About eBIRForms", "-", "Settings…", "-", "Hide eBIRForms", "-", "Sign Out", "-", "Quit eBIRForms"]);
    // File (Windows) and Edit (Linux) keep Settings' shortcut; Hide's and Quit's are here, and File's copies carry none: never bound twice.
    expect(linesOf(s, "eBIRForms")).toEqual(["About eBIRForms", "Settings…", "Hide eBIRForms  mod+h", "Sign Out", "Quit eBIRForms  mod+q"]);
    // About, Settings and Quit show twice (the owner accepted it); each runs the same action.
    const ran: MenuAction[] = [];
    const lines = flatten(menusOf(s, (a) => ran.push(a)));
    lines.find((l) => l.path === "eBIRForms > Settings…")?.select?.();
    lines.find((l) => l.path === "eBIRForms > Quit eBIRForms")?.select?.();
    lines.find((l) => l.path === "eBIRForms > Hide eBIRForms")?.select?.();
    lines.find((l) => l.path === "eBIRForms > About eBIRForms")?.select?.();
    lines.find((l) => l.path === "eBIRForms > Sign Out")?.select?.();
    expect(ran).toEqual(["settings", "quit", "hide", "about", "sign-out"]);
  }
});

test("Profiles: the taxpayers as a switcher, the current one checked, then New tax profile…", () => {
  for (const os of OSES) {
    const s = state(os, { taxpayers: [JUANA, PEDRO], currentTaxpayer: JUANA.root });
    expect(titles(s)[titles(s).length - 1]).toBe("Profiles");
    expect(shape(s, "Profiles")).toEqual(["Juana Santos", "Pedro Reyes", "-", "New tax profile…"]);
    expect(linesOf(s, "Profiles")).toEqual(["Juana Santos  (checked)", "Pedro Reyes", "New tax profile…"]);
    const picked: [MenuAction, string | undefined][] = [];
    const lines = flatten(buildMenu(s, (a, root) => picked.push([a, root])));
    lines.find((l) => l.path === "Profiles > Pedro Reyes")?.select?.();
    lines.find((l) => l.path === "Profiles > New tax profile…")?.select?.();
    expect(picked).toEqual([
      ["switch-taxpayer", PEDRO.root],
      ["new-tax-profile", undefined],
    ]);
    expect(lines.find((l) => l.path === "Profiles > Pedro Reyes")?.checked).toBe(false);
  }
});

test("Profiles with no taxpayers: a disabled note, then New tax profile…", () => {
  for (const os of OSES) {
    const s = state(os, { hasTaxpayer: false, taxpayers: [], currentTaxpayer: null });
    expect(shape(s, "Profiles")).toEqual(["No taxpayers yet", "-", "New tax profile…"]);
    expect(linesOf(s, "Profiles")).toEqual(["No taxpayers yet  (disabled)", "New tax profile…"]);
  }
});

test("New tax profile… follows who may add a taxpayer; the switcher follows who may switch", () => {
  for (const os of OSES) {
    const s = state(os, { taxpayers: [JUANA, PEDRO], currentTaxpayer: JUANA.root, canAddTaxpayer: false, canSwitchTaxpayer: false });
    const disabled = flatten(menusOf(s)).filter((l) => l.disabled).map((l) => l.path);
    expect(disabled).toEqual(["File > New tax profile…", "Profiles > Pedro Reyes", "Profiles > New tax profile…"]);
    // New tax profile… never takes a shortcut: Ctrl/Cmd+N is New Return….
    expect(flatten(menusOf(s)).filter((l) => l.path.endsWith("New tax profile…")).map((l) => l.shortcut)).toEqual([null, null]);
  }
});
