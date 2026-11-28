// Settings > Logs at phone size and at desktop size: the level filters, the last 200 lines, Refresh, no file, an unreadable
// file, and that opening the section reads the file afresh. The log is the fixture's (SettingsDemo.tsx log): the scratch
// database's log file is the host's.
import { expect, test } from "nsh/test";
import { shoot } from "../../shots/shoot";
import { backToList, expectTexts, goSection, openSettings, SIZES, WIDE } from "./testing";

test("startup writes today's log and Logs shows its lines", async () => {
  const app = await openSettings(WIDE, {}, "logs");
  await app.waitFor("text:/eBIRForms started/");
  const today = new Date().toISOString().slice(0, 10);
  expect(app.texts().some((text) => text.includes(`ebirforms.${today}.log`))).toBe(true);
  await shoot(app, "nsh-startup-log");
});

for (const { name, size } of SIZES) {
  test(`${name}: the Logs section tails the log, colours its levels, filters by level and reads the file again on Refresh`, async () => {
    const app = await openSettings(size, { log: "mixed" }, "logs");
    await app.waitFor("id:logs-section");
    expectTexts(app, [
      "ebirforms.2026-11-15.log · 8 lines",
      "A whole number of exactly 9, 12 or 14 digits is masked in logs even when it's a reference number.",
    ]);
    const lines = () => app.texts().filter((t) => /^2026-11-15T|^a continuation/.test(t));
    expect(lines()).toHaveLength(8);
    expect(lines()[lines().length - 1]).toMatch(/fixture log read 1$/);

    await app.press("id:logs-filter >> text:Error");
    expect(lines().map((l) => l.includes(" ERROR ") || l.includes(" FATAL "))).toEqual([true, true]);
    await app.press("id:logs-filter >> text:Warn");
    expect(lines()).toHaveLength(1);
    expect(lines()[0]).toContain(" WARN ");
    await app.press("id:logs-filter >> text:Info");
    expect(lines()).toHaveLength(4);
    // The line with no level is only in All.
    await app.press("id:logs-filter >> text:All");
    expect(app.texts()).toContain("a continuation line with no level");
    // The headline counts the whole file, and Refresh reads it again (the fixture adds a line saying which read this is).
    await app.press("id:logs-refresh");
    await app.waitFor("text:/fixture log read 2$/");
    expect(app.texts().some((t) => /fixture log read 1$/.test(t))).toBe(false);
    // Leaving the section and opening it again reads the file once more.
    if (name === "phone") await backToList(app);
    await goSection(app, "jobs", "id:jobs-section");
    if (name === "phone") await backToList(app);
    await goSection(app, "logs", "id:logs-section");
    await app.waitFor("text:/fixture log read 3$/");
  }, 60_000);

  test(`${name}: only the last 200 lines of a long log show, and the headline says so`, async () => {
    const app = await openSettings(size, { log: "long" }, "logs");
    await app.waitFor("id:logs-section");
    expect(app.texts()).toContain("ebirforms.2026-11-15.log · 301 lines · showing the last 200");
    const lines = app.texts().filter((t) => /bir::app line \d+$|fixture log read/.test(t));
    expect(lines).toHaveLength(200);
    expect(lines.some((l) => /bir::app line 300$/.test(l))).toBe(true);
    expect(lines.some((l) => /bir::app line 101$/.test(l))).toBe(false);
    expect(lines.some((l) => /bir::app line 102$/.test(l))).toBe(true);
    // Everything in it is Info: no Error line, and the filter says so.
    await app.press("id:logs-filter >> text:Error");
    await app.waitFor("id:logs-empty");
    expect(app.texts()).toContain("No lines at this level.");
  }, 60_000);

  test(`${name}: a log with no file yet, and a log that cannot be read`, async () => {
    const none = await openSettings(size, { log: "empty" }, "logs");
    await none.waitFor("id:logs-empty");
    expect(none.texts()).toContain("No log entries yet today.");
    expect(none.texts()).toContain("No log file yet · 0 lines");
    const broken = await openSettings(size, { log: "error" }, "logs");
    await broken.waitFor("id:logs-error");
    expect(broken.texts()).toContain("The log file could not be read.");
  }, 60_000);
}
