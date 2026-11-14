// Opens the app on the UI kit's gallery for the kit's tests: the phone (390 x 844, iOS: touch sizes) or a desktop
// (1440 x 900, macOS: pointer sizes). Test files only.
import { type App, open } from "nsh/test";

/** The two sizes every kit test runs at. */
export const SIZES = [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const;

/** The app, switched to the kit page. `entry` is App.tsx relative to the calling test file. */
export async function openKit(entry: string, options: (typeof SIZES)[number]["options"]): Promise<App> {
  const app = await open(entry, options);
  await app.press("text:UI kit");
  await app.waitFor("text:Text and marks");
  return app;
}

/** The one text on screen that starts with `prefix` (the gallery's live readouts: "Value: ..."). */
export function readout(app: App, prefix: string): string | undefined {
  return app.texts().find((t) => t.startsWith(prefix));
}
