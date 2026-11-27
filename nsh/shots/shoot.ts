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

/**
 * The pages before the app are shot in more window shapes than the rest: a tall desktop window (does the page use the
 * height?) and a short one (does the fullest page scroll rather than clip?), light; then the four usual looks.
 */
export const AUTH_LOOKS = [
  ...LOOKS,
  { name: "tall-light", width: 1100, height: 1300, platform: "macos", colorScheme: "light" },
  { name: "short-light", width: 1280, height: 640, platform: "macos", colorScheme: "light" },
] as const;

/**
 * Empties digit cells (the TIN and branch code of the sign-in and the taxpayer pages) in a real window: `app.clear` does nothing to an
 * OtpInput there (headless it does), so the cells take the keys and Backspace takes each digit.
 */
export async function clearCells(app: App, selector: string, digits = 9): Promise<void> {
  await app.type(selector, ""); // the click that gives the cells the keys (a press finds nothing pressable in them)
  for (let i = 0; i < digits; i++) await app.keyboard.press("backspace");
}
