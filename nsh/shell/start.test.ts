import { expect, test } from "nsh/test";
import { allowedStart, parseStart, parseStartForm } from "./start";
import { nextThemePreference, parseThemePreference, storedThemePreference, themeIcon, themePreferenceFromStart } from "./theme";

test("a start route is read step by step", () => {
  expect(parseStart("tp=123456788;theme=dark;view=library")).toEqual([
    { key: "tp", value: "123456788" },
    { key: "theme", value: "dark" },
    { key: "view", value: "library" },
  ]);
  expect(parseStart("save; ;palette=a=b")).toEqual([
    { key: "save", value: "" },
    { key: "palette", value: "a=b" },
  ]);
  expect(parseStart(undefined)).toEqual([]);
});

test("form=CODE/YEAR/PERIOD[/KEY]", () => {
  expect(parseStartForm("2551Q/2026/3")).toEqual({ code: "2551Q", year: 2026, period: 3, key: null });
  expect(parseStartForm("2551Q/2026/0/1201")).toEqual({ code: "2551Q", year: 2026, period: 0, key: 1201 });
  expect(parseStartForm("2551Q/2026")).toBeNull();
  expect(parseStartForm("2551Q/x/1")).toBeNull();
});

test("the theme preference: parse, cycle, stored spelling, icon, start route", () => {
  expect(parseThemePreference("Dark")).toBe("dark");
  expect(parseThemePreference(" LIGHT ")).toBe("light");
  expect(parseThemePreference("System")).toBe("system");
  expect(parseThemePreference(null)).toBe("system");
  expect(parseThemePreference("blue")).toBe("system");
  expect(nextThemePreference("light")).toBe("dark");
  expect(nextThemePreference("dark")).toBe("system");
  expect(nextThemePreference("system")).toBe("light");
  expect(storedThemePreference("dark")).toBe("Dark");
  expect(storedThemePreference("system")).toBe("System");
  expect(themeIcon("light")).toBe("sun");
  expect(themeIcon("dark")).toBe("moon");
  expect(themeIcon("system")).toBe("monitor");
  expect(themePreferenceFromStart("tp=1;theme=dark")).toBe("dark");
  expect(themePreferenceFromStart("tp=1")).toBeNull();
});

test("F8-12: a start route is honoured only in a test environment", () => {
  const route = "signin=000123453:1234;theme=dark";
  expect(allowedStart(route, true)).toBe(route);
  expect(allowedStart(route, false)).toBeUndefined();
  expect(allowedStart("", true)).toBeUndefined();
  expect(allowedStart(undefined, true)).toBeUndefined();
  expect(allowedStart(null, false)).toBeUndefined();
});
