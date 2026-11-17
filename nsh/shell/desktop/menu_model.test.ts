// The application menu as data: which menus and items exist, which are enabled, and their accelerators, in every state the app
// can be in (signed in with and without a taxpayer, a form open, locked) on each platform. A plain test: no window.
import { expect, test } from "nsh/test";
import { buildMenu, buildTrayMenu, CLOSE_POLICY, flatten, printMenu, type MenuAction, type MenuState, type Os } from "./menu_model";

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
  expect(titles(s)).toEqual(["eBIRForms", "File", "Edit", "View", "Window", "Help"]);
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
  expect(shape(s, "File")).toEqual(["New Return…", "-", "Close Window"]);
  expect(linesOf(s, "File")).toEqual(["New Return…  mod+n", "Close Window  mod+w"]);
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
  expect(shape(s, "Help")).toEqual(["Check for Updates…", "Contact Support…", "Goldcoders Website"]);
  // AppKit adds its own Enter Full Screen to View: ours would show it twice.
  expect(flatten(menusOf(s)).some((l) => l.path.includes("Full Screen"))).toBe(false);
});

test("Windows and Linux signed in: File holds the app commands, Help holds About, View has Full Screen on F11", () => {
  const win = state("windows");
  expect(titles(win)).toEqual(["File", "Edit", "View", "Window", "Help"]);
  expect(shape(win, "File")).toEqual(["New Return…", "-", "Settings…", "Sign Out", "-", "Hide eBIRForms", "-", "Quit eBIRForms"]);
  expect(linesOf(win, "File")).toContain("Settings…  mod+,");
  expect(linesOf(win, "File")).toContain("Quit eBIRForms"); // the docs: no Ctrl+Q on Windows
  expect(shape(win, "Edit")).not.toContain("Settings…");
  expect(shape(win, "Help")).toEqual(["About eBIRForms", "-", "Check for Updates…", "Contact Support…", "Goldcoders Website"]);
  expect(linesOf(win, "View")).toContain("Enter Full Screen  f11");
  expect(shape(win, "Window")).toEqual(["role:minimize", "role:zoom"]);

  const linux = state("linux");
  expect(titles(linux)).toEqual(["File", "Edit", "View", "Window", "Help"]);
  expect(shape(linux, "File")).toEqual(["New Return…", "-", "Close Window", "Hide eBIRForms", "-", "Sign Out", "Quit eBIRForms"]);
  expect(linesOf(linux, "File")).toContain("Quit eBIRForms  mod+q");
  expect(shape(linux, "Edit").slice(-2)).toEqual(["-", "Settings…"]);
  expect(linesOf(linux, "Edit")).toContain("Settings…  mod+,");
});

test("with no taxpayer the items that need one are there but disabled; nothing else changes", () => {
  for (const os of OSES) {
    const none = flatten(menusOf(state(os, { hasTaxpayer: false })));
    const some = flatten(menusOf(state(os, { hasTaxpayer: true })));
    expect(none.map((l) => l.path)).toEqual(some.map((l) => l.path));
    const disabled = none.filter((l) => l.disabled).map((l) => l.path);
    expect(disabled).toEqual(["File > New Return…", "View > Toggle Tax Profile"]);
    expect(some.filter((l) => l.disabled)).toEqual([]);
  }
});

test("a form open changes nothing: its page owns the text fields, the menu never takes their keys", () => {
  for (const os of OSES) {
    for (const hasTaxpayer of [true, false]) {
      expect(printMenu(menusOf(state(os, { formOpen: true, hasTaxpayer })))).toEqual(printMenu(menusOf(state(os, { formOpen: false, hasTaxpayer }))));
    }
  }
});

test("locked (first-run setup, sign-in, the account gate): only About, Quit and editing are there", () => {
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
    "Hide eBIRForms",
    "Close Window",
  ];
  for (const os of OSES) {
    const lines = flatten(menusOf(state(os, { unlocked: false, hasTaxpayer: false })));
    for (const banned of BANNED) expect(lines.some((l) => l.path.endsWith("> " + banned))).toBe(false);
    expect(lines.some((l) => l.path.includes("About"))).toBe(true);
    expect(lines.some((l) => l.path.includes("Quit"))).toBe(true);
    expect(lines.filter((l) => l.disabled)).toEqual([]);
  }
  const mac = state("macos", { unlocked: false });
  expect(titles(mac)).toEqual(["eBIRForms", "Edit"]);
  expect(shape(mac, "eBIRForms")).toEqual(["About eBIRForms", "-", "Quit eBIRForms"]);
  expect(shape(mac, "Edit")).toContain("role:paste");
  expect(titles(state("windows", { unlocked: false }))).toEqual(["File", "Edit", "Help"]);
  expect(shape(state("windows", { unlocked: false }), "File")).toEqual(["Quit eBIRForms"]);
  expect(shape(state("linux", { unlocked: false }), "Help")).toEqual(["About eBIRForms"]);
  expect(shape(state("linux", { unlocked: false }), "Edit")).not.toContain("Settings…");
});

test("every state has a tidy menu: no stray separators, no shortcut used twice, a way to quit", () => {
  for (const os of OSES) {
    for (const unlocked of [true, false]) {
      for (const hasTaxpayer of [true, false]) {
        for (const formOpen of [true, false]) {
          const s = state(os, { unlocked, hasTaxpayer, formOpen });
          for (const menu of menusOf(s)) {
            if (!("submenu" in menu) || !menu.submenu) throw new Error("a top-level item has no submenu");
            const kinds = menu.submenu.map((e) => (!e ? "?" : "type" in e ? "-" : "item"));
            expect(kinds[0]).toBe("item");
            expect(kinds[kinds.length - 1]).toBe("item");
            expect(kinds.join("").includes("--")).toBe(false);
          }
          const shortcuts = flatten(menusOf(s)).flatMap((l) => (l.shortcut ? [l.shortcut] : []));
          expect(new Set(shortcuts).size).toBe(shortcuts.length);
          expect(flatten(menusOf(s)).some((l) => l.path.endsWith("Quit eBIRForms"))).toBe(true);
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
    "File > New Return…": "new-return",
    "File > Close Window": "close-window",
    "View > Toggle Sidebar": "toggle-sidebar",
    "View > Toggle Tax Profile": "toggle-profile",
    "View > Toggle Notifications": "toggle-notifications",
    "View > Toggle Theme": "toggle-theme",
    "View > Command Palette…": "palette",
    "Window > Bring All to Front": "bring-all-to-front",
    "Help > Check for Updates…": "check-updates",
    "Help > Contact Support…": "support",
    "Help > Goldcoders Website": "website",
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
  ]);
});

test("the tray menu is Show, Hide and Quit, the same in every state", () => {
  const ran: MenuAction[] = [];
  const lines = flatten(buildTrayMenu((a) => ran.push(a)));
  expect(lines.map((l) => l.path)).toEqual(["Show eBIRForms", "Hide eBIRForms", "Quit"]);
  for (const line of lines) line.select?.();
  expect(ran).toEqual(["show", "hide", "quit-now"]);
});

test("the platform docs decide what close and the quit key do", () => {
  // docs/platforms/macos.md: X and Cmd+Q hide to the menu bar; quit is in the tray. windows.md: X and Alt+F4 hide, no Ctrl+Q. linux.md: X and Ctrl+Q quit.
  expect(CLOSE_POLICY.macos).toEqual({ closeHides: true, quitKeyHides: true, quitShortcut: "mod+q" });
  expect(CLOSE_POLICY.windows).toEqual({ closeHides: true, quitKeyHides: false, quitShortcut: null });
  expect(CLOSE_POLICY.linux).toEqual({ closeHides: false, quitKeyHides: false, quitShortcut: "mod+q" });
});
