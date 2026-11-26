// The Tax profile's birth date is the form page's date control: a DateField at a desktop (typed MM/DD/YYYY or 08171990, a calendar,
// "May 17, 1990" once left), a DateTouchField and its date sheet at phone size. Typed, picked, cleared, an impossible day refused with
// the form's sentence, a day after today refused with "A birth date can't be in the future. The box kept ...". The stored format
// (ISO) is checked through the door in profileDates.store.test.ts.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const BAD_DAY = "Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.";
const BOX = "id:tp-taxpayer-birth_date";

const profile = (options: Record<string, unknown> = {}) => open(ENTRY, { ...options, host: { params: { start: "view=profile" } } });

/** Leaves the birth date box, as Tab would: the keys go to the name box (a space there is trimmed on Save). */
const leave = (app: App) => app.type("id:tp-taxpayer-name", " ");

test("desktop: the birth date is typed, picked from the calendar, refused when impossible or after today, and cleared", async () => {
  const app = await profile(WIDE);
  await app.waitFor("id:tp-sec-taxpayer-edit");
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor(BOX);
  // Typed as the form's boxes take it: eight digits are MM/DD/YYYY at once, the long form once left.
  await app.clear(BOX);
  await app.type(BOX, "05171990");
  expect(app.value(BOX)).toBe("05/17/1990");
  await leave(app);
  expect(app.value(BOX)).toBe("May 17, 1990");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:Changes saved");
  await app.waitFor(BOX, { state: "gone" });
  expect(app.texts()).toContain("May 17, 1990");

  // Picked from the calendar, which opens on the box's month.
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor(BOX);
  await app.press(`${BOX}-calendar`);
  await app.waitFor("text:20");
  await app.press("text:20");
  await app.waitUntil(() => app.value(BOX) === "May 20, 1990");

  // An impossible day: the form's sentence under the box once left, and Save keeps the edit open.
  await app.clear(BOX);
  await app.type(BOX, "02/30/1990");
  await leave(app);
  await app.waitFor(`text:${BAD_DAY}`);
  await app.press("id:tp-taxpayer-save");
  expect(app.shows(BOX)).toBe(true);

  // A day after today is not taken: the box goes back to the last day it held and says so.
  await app.clear(BOX);
  await app.type(BOX, "01/01/2099");
  await leave(app);
  await app.waitFor("text:A birth date can't be in the future. The box kept 05/20/1990.");
  expect(app.value(BOX)).toBe("May 20, 1990");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor(BOX, { state: "gone" });
  expect(app.texts()).toContain("May 20, 1990");

  // Cleared: no birth date.
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor(BOX);
  await app.clear(BOX);
  await leave(app);
  await app.press("id:tp-taxpayer-save");
  await app.waitFor(BOX, { state: "gone" });
  expect(app.texts()).not.toContain("May 20, 1990");
});

test("phone: the birth date is a row whose date sheet types, picks, refuses an impossible or future day, and clears", async () => {
  const app = await profile();
  await app.waitFor("id:tp-sec-taxpayer-edit");
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor("id:tp-edit-sheet-body");
  const input = `${BOX}-sheet-input`;
  const done = `${BOX}-sheet-done`;

  // Typed in the sheet, taken on Done; the row says the long form.
  await app.press(BOX);
  await app.waitFor(input);
  await app.clear(input);
  await app.type(input, "05171990");
  await app.press(done);
  await app.waitFor(input, { state: "gone" });
  expect(app.texts()).toContain("May 17, 1990");

  // An impossible day: the sheet stays with the form's sentence, the row keeps its day.
  await app.press(BOX);
  await app.waitFor(input);
  await app.clear(input);
  await app.type(input, "02/30/1990");
  await app.press(done);
  expect(app.shows(input)).toBe(true);
  expect(app.texts()).toContain(BAD_DAY);
  await app.back();
  await app.waitFor(input, { state: "gone" });
  expect(app.texts()).toContain("May 17, 1990");

  // A day after today: refused in the sheet.
  await app.press(BOX);
  await app.waitFor(input);
  await app.clear(input);
  await app.type(input, "01/01/2099");
  await app.press(done);
  expect(app.shows(input)).toBe(true);
  expect(app.texts()).toContain("A birth date can't be in the future.");
  await app.back();
  await app.waitFor(input, { state: "gone" });
  expect(app.texts()).toContain("May 17, 1990");

  // Picked from the sheet's calendar (on the row's month), taken on Done.
  await app.press(BOX);
  await app.waitFor(input);
  await app.waitFor("text:20");
  await app.press("text:20");
  await app.waitUntil(() => app.value(input) === "05/20/1990");
  await app.press(done);
  await app.waitFor(input, { state: "gone" });
  expect(app.texts()).toContain("May 20, 1990");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("text:Changes saved");
  await app.waitFor("id:tp-edit-sheet-body", { state: "gone" });
  expect(app.texts()).toContain("May 20, 1990");

  // Cleared from the sheet.
  await app.press("id:tp-sec-taxpayer-edit");
  await app.waitFor("id:tp-edit-sheet-body");
  await app.press(BOX);
  await app.press(`${BOX}-sheet-clear`);
  await app.waitFor(input, { state: "gone" });
  // The row is empty again (the profile's read view behind the edit sheet still says the saved day until Save).
  expect(app.shows(`${BOX} >> text:MM/DD/YYYY`)).toBe(true);
  expect(app.shows(`${BOX} >> text:May 20, 1990`)).toBe(false);
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("id:tp-edit-sheet-body", { state: "gone" });
  // Saved as no birth date: the read view drops it.
  await app.waitUntil(() => !app.texts().includes("May 20, 1990"));
});
