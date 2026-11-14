// The theme contract: what the screens rely on is defined in both appearances, aliases resolve, and small text meets 4.5:1.
// Pure test (no app is opened): it reads theme.json the way the build does and computes WCAG contrast from the hex values.
import { describe, expect, test } from "nsh/test";
import theme from "./theme.json";

type Appearance = "light" | "dark";
const vars = theme.cssVars as { theme: Record<string, string>; light: Record<string, string>; dark: Record<string, string> };

/** A token's value in an appearance: dark falls back to light (as the build does), `var(--x)` follows the same appearance. */
function value(appearance: Appearance, name: string): string {
  const raw = vars[appearance][name] ?? vars.light[name] ?? vars.theme[name];
  if (raw === undefined) throw new Error(`no token ${name} in ${appearance}`);
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(raw);
  return alias ? value(appearance, alias[1]) : raw;
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const SURFACES = ["background", "card", "raised", "hover", "selected"];
const SMALL_TEXT = ["foreground", "muted-foreground", "subtle-foreground", "accent-blue", "success", "warning", "danger"];

for (const appearance of ["light", "dark"] as const) {
  describe(appearance, () => {
    for (const text of SMALL_TEXT) {
      test(`${text} reaches 4.5:1 on every surface`, () => {
        for (const surface of SURFACES) {
          // Status and accent colors are only used on the page and on cards, not on hover or selected fills.
          const used = text === "foreground" || text.endsWith("-foreground") || surface === "background" || surface === "card";
          if (!used) continue;
          const ratio = contrast(value(appearance, text), value(appearance, surface));
          expect(ratio).toBeGreaterThanOrEqual(4.5);
        }
      });
    }

    test("button text reaches 4.5:1 on its fill", () => {
      expect(contrast(value(appearance, "primary-foreground"), value(appearance, "primary"))).toBeGreaterThanOrEqual(4.5);
      expect(contrast(value(appearance, "destructive-foreground"), value(appearance, "destructive"))).toBeGreaterThanOrEqual(4.5);
    });

    test("shadcn slots alias the app's roles", () => {
      expect(value(appearance, "popover")).toBe(value(appearance, "raised"));
      expect(value(appearance, "muted")).toBe(value(appearance, "hover"));
      expect(value(appearance, "accent")).toBe(value(appearance, "selected"));
      expect(value(appearance, "input")).toBe(value(appearance, "border-strong"));
      expect(value(appearance, "ring")).toBe(value(appearance, "accent-blue"));
      expect(value(appearance, "destructive")).toBe(value(appearance, "danger"));
      expect(value(appearance, "card-foreground")).toBe(value(appearance, "foreground"));
    });
  });
}

test("the old fg3 failed 4.5:1 on the light page; the new one passes", () => {
  expect(contrast("#71717a", value("light", "background"))).toBeLessThan(4.5);
  expect(contrast(value("light", "subtle-foreground"), value("light", "background"))).toBeGreaterThanOrEqual(4.5);
});
