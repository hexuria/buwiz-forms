// Which rows the chrome has (chrome_layout.ts): per OS, desktop (KDE's Global Menu or not) and width. A plain test.
import { expect, test } from "nsh/test";
import { chromeLayout, MENU_ROW, menuKey, menuPlace, toastPlace, TOOLBAR, type ChromeInput } from "./chrome_layout";

const at = (os: string, width: number, over: Partial<ChromeInput> = {}) => chromeLayout({ os, desktop: true, globalMenu: false, width, ...over });

test("macOS: the system's menu bar; one row (traffic lights, then the toolbar) at every width; no ☰", () => {
  for (const width of [1440, 700, 420]) {
    const l = at("macos", width);
    expect(l.menu).toBe("system");
    expect(l.menuRow).toBe(false);
    expect(l.hamburger).toBe(false);
    expect(l.toolbarHoldsWindowButtons).toBe(true);
    expect(l.titleBarHeight).toBe(TOOLBAR);
  }
});

test("KDE with a Global Menu: one row, window buttons at the toolbar's end; phone width unchanged", () => {
  for (const width of [1440, 700, 420]) {
    const l = at("linux", width, { globalMenu: true });
    expect(l.menu).toBe("global");
    expect(l.menuRow).toBe(false);
    expect(l.hamburger).toBe(false);
    expect(l.toolbarHoldsWindowButtons).toBe(true);
    expect(l.titleBarHeight).toBe(TOOLBAR);
  }
});

test("Windows, GNOME, KDE without a registrar: two rows, the window buttons in the menu row", () => {
  for (const [os, icon] of [["windows", true], ["linux", false]] as const) {
    const wide = at(os, 1440);
    expect(wide.menu).toBe("window");
    expect(wide.menuRow).toBe(true);
    expect(wide.hamburger).toBe(false);
    expect(wide.chevrons).toBe(true);
    expect(wide.pillTin).toBe(true);
    expect(wide.appIcon).toBe(icon); // the small app icon is Windows' only
    expect(wide.toolbarHoldsWindowButtons).toBe(false);
    expect(wide.titleBarHeight).toBe(MENU_ROW);
  }
});

test("tablet width (600 to 860): the same rows; no chevrons, no TIN in the pill", () => {
  for (const os of ["windows", "linux", "macos"]) {
    for (const width of [600, 700, 859]) {
      const l = at(os, width);
      expect(l.menuRow).toBe(os !== "macos");
      expect(l.chevrons).toBe(false);
      expect(l.pillTin).toBe(false);
      expect(l.narrow).toBe(false);
    }
  }
  expect(at("windows", 860).chevrons).toBe(true);
  expect(at("windows", 860).pillTin).toBe(true);
});

test("phone width on Windows and Linux windows: no menu row, ☰ in the narrow header", () => {
  for (const os of ["windows", "linux"]) {
    const l = at(os, 599);
    expect(l.narrow).toBe(true);
    expect(l.menuRow).toBe(false);
    expect(l.hamburger).toBe(true);
    expect(l.appIcon).toBe(false);
    expect(l.titleBarHeight).toBe(TOOLBAR);
  }
  expect(at("linux", 420, { globalMenu: true }).hamburger).toBe(false);
});

test("real phones and the web: no menu at all", () => {
  for (const os of ["ios", "android"]) {
    const l = chromeLayout({ os, desktop: false, globalMenu: false, width: 390 });
    expect(l.menu).toBe("none");
    expect(l.menuRow).toBe(false);
    expect(l.hamburger).toBe(false);
  }
  expect(menuPlace({ os: "ios", desktop: false, globalMenu: false })).toBe("none");
  expect(menuPlace({ os: "web", desktop: true, globalMenu: false })).toBe("none");
  expect(menuPlace({ os: "macos", desktop: false, globalMenu: false })).toBe("none");
  // The Global Menu answer only counts on Linux.
  expect(menuPlace({ os: "windows", desktop: true, globalMenu: true })).toBe("window");
});

test("Alt+letter: each title its first free letter, eBIRForms takes B", () => {
  const taken: string[] = [];
  const keys = ["eBIRForms", "File", "Edit", "View", "Window", "Help", "Profiles"].map((t) => {
    const k = menuKey(t, taken);
    if (k) taken.push(k);
    return k;
  });
  expect(keys).toEqual(["b", "f", "e", "v", "w", "h", "p"]);
});

test("toastPlace: on a phone's form page toasts sit at the top, under the header, so none lies over a row's boxes or the pill", () => {
  // The header is 52 pt on touch, 40 pt in a narrow desktop window; the toast starts 8 pt under it.
  expect(toastPlace("form", true, true)).toEqual({ edge: "top", offset: 60 });
  expect(toastPlace("form", true, false)).toEqual({ edge: "top", offset: 48 });
  // A wide form page keeps them above the pill; every other screen at the bottom, above the phone's tab bar.
  expect(toastPlace("form", false, false)).toEqual({ edge: "bottom", offset: 96 });
  expect(toastPlace("home", true, true)).toEqual({ edge: "bottom", offset: 76 });
  expect(toastPlace("home", false, false)).toEqual({ edge: "bottom", offset: 20 });
});
