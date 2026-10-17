// Theme preference for the buwiz shell.
//
// The preference is stored in the SAME setting key ("theme_preference") and
// spelling ("System"/"Dark"/"Light") as the painted app, so a user switching
// binaries keeps their choice. Colors come from the design tokens in ui.js:
// `set_theme` receives gpui-base's semantic roles mapped from those tokens,
// so base components (inputs, tooltips, scrollbars) share the palette.

import { set_theme } from "gpui-base";
import { baseColors, noteAppliedAppearance } from "./ui.js";

export const THEME_PREF_KEY = "theme_preference";

/** @typedef {"system" | "dark" | "light"} ThemePreference */

/** Parse the painted app's stored value ("System"/"Dark"/"Light" or lowercase). */
export function parseThemePreference(raw) {
  const v = (raw == null ? "" : String(raw)).trim().toLowerCase();
  if (v === "dark" || v === "light" || v === "system") return v;
  return "system";
}

/** The title-bar toggle's cycle, as the design prototype runs it: Light → Dark → System. */
export function nextThemePreference(pref) {
  if (pref === "light") return "dark";
  if (pref === "dark") return "system";
  return "light";
}

/** Storage spelling shared with the painted app. */
export function storedThemePreference(pref) {
  return pref[0].toUpperCase() + pref.slice(1);
}

/** The toggle's icon for a preference. */
export function themeIcon(pref) {
  return pref === "light" ? "sun" : pref === "dark" ? "moon" : "monitor";
}

/**
 * Apply a preference. "system" follows the window's platform appearance;
 * explicit prefs force that appearance. Call from init or an event handler.
 */
export function applyThemePreference(cx, pref) {
  const theme = cx.theme();
  const system = typeof window !== "undefined" && window.appearance ? window.appearance() : theme.appearance;
  const appearance = pref === "system" ? system : pref === "dark" ? "dark" : "light";
  noteAppliedAppearance(appearance);
  set_theme({
    appearance,
    tokens: {
      // Spread the live palette first: the validator wants a complete
      // ColorTokens, then every role the design names is replaced.
      colors: { ...theme.colors, ...baseColors(appearance) },
      spacing: theme.spacing,
      radius: theme.radius,
      typography: { md: { size: 13 } },
    },
  });
}
