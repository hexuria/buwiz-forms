// Helpers for the Settings tests (a test file imports it; the app never does): open the SettingsDemo fixture at phone or desktop size, on
// a section, and read the fixture's one-line readout of the app state.
import { type App, expect, open } from "nsh/test";

export const ENTRY = "../../SettingsDemo.tsx";
export const WIDE = { width: 1440, height: 900, platform: "macos" as const };
/** Phone (390 x 844, the default) and desktop (1440 x 900, macOS). */
export const SIZES = [
  { name: "phone", size: {} },
  { name: "desktop", size: WIDE },
] as const;
export type Size = (typeof SIZES)[number]["size"];

/** Opens Settings (on `section` when given: a phone then shows that section's page, not the list) and waits for the page. */
export async function openSettings(size: Size, params: Record<string, unknown> = {}, section?: string): Promise<App> {
  const start = section ? `section=${section}` : "view=settings";
  const app = await open(ENTRY, { ...size, host: { params: { readout: true, noise: true, start, ...params } } });
  await app.waitFor("id:settings", { timeout: 30000 });
  return app;
}

/**
 * Waits until `text` is no longer among the texts. A Select's popup is still on screen, with all its options, for a few frames after a
 * pick: a test that asserts what the page shows now has to let it slide away first (a slow machine takes longer than a fast one).
 */
export async function goneText(app: App, text: string): Promise<void> {
  await app.waitUntil(() => !app.texts().includes(text), { timeout: 15000 });
}

/** The fixture's readout line. */
export function state(app: App): string {
  return app.texts().find((t) => t.startsWith("theme=")) ?? "";
}

/** What the fixture's fake host calls were, in order. */
export function calls(app: App): string[] {
  const line = app.texts().find((t) => t.startsWith("calls:")) ?? "";
  const list = line.slice("calls:".length).trim();
  return list ? list.split(" | ") : [];
}

/** Opens a section from the list (a phone) or the rail (desktop). */
export async function goSection(app: App, id: string, ready: string): Promise<void> {
  await app.press(`id:settings-nav-${id}`);
  await app.waitFor(ready, { timeout: 15000 });
}

/** Every text in `list` is on screen. */
export function expectTexts(app: App, list: string[]): void {
  const texts = app.texts();
  for (const t of list) expect(texts).toContain(t);
}
