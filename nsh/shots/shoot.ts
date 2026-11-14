// Real-window screenshots from a test. Headless runs cannot draw (app.screenshot rejects), so a shots test
// that runs in plain `nsh test` still passes: it just takes no pictures. Run them for real with `just nsh-shots <name>`
// (nsh test --target macos); the PNGs land in the current directory, which the recipe sets to ~/BuwizNsh/evidence/<name>/.
import type { App } from "nsh/test";

/** Takes `<name>.png` of the main window; false (and no file) when the run is headless. */
export async function shoot(app: App, name: string): Promise<boolean> {
  try {
    await app.screenshot(`${name}.png`);
    return true;
  } catch (error) {
    if (String(error).includes("needs a real platform")) return false;
    throw error;
  }
}

/** The four looks every screen is shot in: wide and narrow, light and dark. Desktop size, then a phone's. */
export const LOOKS = [
  { name: "wide-light", width: 1440, height: 900, platform: "macos", colorScheme: "light" },
  { name: "wide-dark", width: 1440, height: 900, platform: "macos", colorScheme: "dark" },
  { name: "narrow-light", width: 390, height: 844, platform: "macos", colorScheme: "light" },
  { name: "narrow-dark", width: 390, height: 844, platform: "macos", colorScheme: "dark" },
] as const;
