// CalendarButton: the calendar icon DateField carries, on its own (the form page's split date boxes). A press opens the popup, a day
// picked reports its ISO date, and the days outside min and max cannot be picked. At phone and desktop size.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "./calendarButton.fixture.tsx";
const SIZES = [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const;
const readout = (app: App, prefix: string) => app.texts().find((t) => t.startsWith(prefix));

for (const size of SIZES) {
  test(`${size.name}: a press opens the calendar and a picked day reports its ISO date`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:cb-trigger");
    expect(readout(app, "Picked:")).toBe("Picked: 2026-11-10");
    await app.press("id:cb-trigger");
    await app.waitFor("text:20");
    await app.press("text:20");
    await app.waitUntil(() => readout(app, "Picked:") === "Picked: 2026-11-20");
    expect(readout(app, "Calls:")).toBe("Calls: 1");
  });

  test(`${size.name}: a day before min or after max is not reported`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:cb-trigger");
    await app.press("id:cb-trigger");
    await app.waitFor("text:20");
    await app.press("text:30").catch(() => {});
    await app.wait(200);
    expect(readout(app, "Calls:")).toBe("Calls: 0");
    expect(readout(app, "Picked:")).toBe("Picked: 2026-11-10");
    await app.press("text:25");
    await app.waitUntil(() => readout(app, "Picked:") === "Picked: 2026-11-25");
  });
}
