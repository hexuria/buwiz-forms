// Theme preference + palettes for the buwiz shell.
//
// Mirrors bir-desktop/src/theme.rs: the AppThemeMode cycle is
// System -> Dark -> Light -> System, persisted in the SAME setting key
// ("theme_preference") as the painted app so a user switching binaries keeps
// their choice. The painted app reads serde-JSON variants ("System", "Dark",
// "Light"); we accept those and the lowercase spellings.
//
// `set_theme` requires the full token set, so we re-send spacing/radius/
// typography from the live theme and only swap appearance + colors.
//
// The palettes below are gpui-component's stock light/dark tables — the same
// defaults the painted app resolves through `Theme::change`, so semantic
// roles match 1:1. Anything the painted UI drew from extra Theme fields
// (warning/danger/success tints) is a literal here instead.

import { set_theme } from "gpui-base";

export const LIGHT_COLORS = {
  background: "#ffffff",
  foreground: "#0a0a0a",
  surface: "#ffffff",
  surface_foreground: "#0a0a0a",
  primary: "#171717",
  primary_foreground: "#fafafa",
  secondary: "#e5e5e5",
  secondary_foreground: "#171717",
  muted: "#f5f5f5",
  muted_foreground: "#737373",
  accent: "#f5f5f5",
  accent_foreground: "#171717",
  destructive: "#ef4444",
  destructive_foreground: "#fafafa",
  border: "#e5e5e5",
  input: "#e5e5e5",
  ring: "#a3a3a3",
};

export const DARK_COLORS = {
  background: "#0a0a0a",
  foreground: "#fafafa",
  surface: "#0a0a0a",
  surface_foreground: "#fafafa",
  primary: "#fafafa",
  primary_foreground: "#171717",
  secondary: "#262626",
  secondary_foreground: "#fafafa",
  muted: "#262626",
  muted_foreground: "#a3a3a3",
  accent: "#262626",
  accent_foreground: "#fafafa",
  destructive: "#f87171",
  destructive_foreground: "#dc2626",
  border: "#262626",
  input: "#2f2f2f",
  ring: "#737373",
};

// Literal hues for places the painted app used non-semantic theme colors:
// dev_mode::dev_orange() and the logout tint's red.
export const DEV_ORANGE = "#EA580C";
export const LOGOUT_RED = "#EF4444";

export const THEME_PREF_KEY = "theme_preference";

/** @typedef {"system" | "dark" | "light"} ThemePreference */

/** Parse the painted app's stored value ("System"/"Dark"/"Light" or lowercase). */
export function parseThemePreference(raw) {
  const v = (raw == null ? "" : String(raw)).trim().toLowerCase();
  if (v === "dark" || v === "light" || v === "system") return v;
  return "system";
}

/** AppThemeMode::next — System -> Dark -> Light -> System. */
export function nextThemePreference(pref) {
  if (pref === "system") return "dark";
  if (pref === "dark") return "light";
  return "system";
}

/**
 * Apply a preference. "system" re-applies the host-resolved appearance (which
 * already tracks the OS); explicit prefs force that appearance.
 * Must be called from init or an event handler (a live host call).
 */
export function applyThemePreference(cx, pref) {
  const theme = cx.theme();
  const appearance =
    pref === "system" ? theme.appearance : pref === "dark" ? "dark" : "light";
  set_theme({
    appearance,
    tokens: {
      // Spread the live palette first: the validator wants a COMPLETE
      // ColorTokens (including `selection`, which gpui-base's stock tables
      // leave to a built-in default), so only the swapped roles are literal.
      colors: {
        ...theme.colors,
        ...(appearance === "dark" ? DARK_COLORS : LIGHT_COLORS),
      },
      spacing: theme.spacing,
      radius: theme.radius,
      typography: theme.typography,
    },
  });
}
