// The theme preference (the old shell's theme.js): the same setting key and spelling as the painted app ("System",
// "Dark", "Light"), so a person switching binaries keeps the choice. Pure; app.ts applies it with setColorScheme.

/** What the person chose: follow the system, or force one appearance. */
export type ThemePreference = "system" | "dark" | "light";

/** The setting the preference is stored under (bir get_setting / set_setting). */
export const THEME_PREF_KEY = "theme_preference";

/** Reads the stored value ("System", "Dark", "Light" or lowercase); anything else is "system". */
export function parseThemePreference(raw: string | null | undefined): ThemePreference {
  const v = (raw == null ? "" : String(raw)).trim().toLowerCase();
  if (v === "dark" || v === "light" || v === "system") return v;
  return "system";
}

/** The theme from a dev start route ("tp=...;theme=dark;view=library"), or null when it names none. */
export function themePreferenceFromStart(raw: string | null | undefined): ThemePreference | null {
  for (const part of String(raw ?? "").split(";")) {
    const [key, value = ""] = part.split("=");
    if (key.trim() === "theme" && value.trim()) return parseThemePreference(value);
  }
  return null;
}

/** The title-bar toggle's cycle: Light, then Dark, then System. */
export function nextThemePreference(pref: ThemePreference): ThemePreference {
  if (pref === "light") return "dark";
  if (pref === "dark") return "system";
  return "light";
}

/** The spelling the setting is stored in. */
export function storedThemePreference(pref: ThemePreference): string {
  return pref[0].toUpperCase() + pref.slice(1);
}

/** The toggle's icon for a preference (names in ui/icons). */
export function themeIcon(pref: ThemePreference): "sun" | "moon" | "monitor" {
  return pref === "light" ? "sun" : pref === "dark" ? "moon" : "monitor";
}
