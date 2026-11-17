// F8-13: "today" is read again when the day changes (a timer to the next midnight) and when the app comes back to the foreground on
// another day (timers do not run while a laptop sleeps). It used to be read once at import: across the year end the Calendar stayed on
// the old year and Home kept yesterday's date. On a manual clock (nsh/test `clock`), over the Calendar's fixture.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../CalendarDemo.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" } as const;
const EVE = new Date(2026, 11, 31, 23, 59, 0);

test("desktop: at midnight on December 31 the Calendar moves to the new year, and Home to the new day; back from sleep, to that day", async () => {
  const app = await open(ENTRY, { ...DESKTOP, clock: { time: EVE.getTime() }, host: { params: {} } });
  await app.waitFor("id:calendar-year-text");
  expect(app.texts()).toContain("2026");
  await app.press("id:nav-home");
  await app.waitFor("text:Thursday, December 31, 2026.");

  // Midnight passes with the app open.
  await app.clock!.fastForward(90_000);
  await app.waitFor("text:Friday, January 1, 2027.");
  await app.press("id:nav-calendar");
  await app.waitFor("id:calendar-year-text");
  await app.waitUntil(() => app.texts().includes("2027"));

  // The app sleeps through a day (no timer runs), then comes back to the foreground.
  await app.setLifecycle("background");
  app.clock!.setSystemTime(new Date(2027, 0, 2, 10, 0, 0).getTime());
  await app.setLifecycle("active");
  // The phone layout from here (NSH 0.5.1 keeps the window's size across a lifecycle change; before, a window shown again
  // headless came back at 390 x 844).
  await app.setViewport(390, 844);
  await app.waitFor("text:In 13 days");
  await app.press("id:tab-home");
  await app.waitFor("text:Saturday, January 2, 2027.");
});
