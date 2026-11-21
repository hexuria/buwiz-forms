// MonthPicker at phone and desktop size: the trigger opens the month grid (a sheet on touch, a popover
// on desktop), the year steps inside its bounds, and a picked month lands with the grid's year.
import { expect, test } from "nsh/test";
import { openKit, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: the month picker steps its year inside bounds and picks a month`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    // The gallery holds December 2023.
    expect(app.texts()).toContain("12/2023");
    await app.press("id:kit-month");
    await app.waitFor("id:kit-month-next");
    // 2023 -> 2020 is the floor (minYear 2020): three steps back, then prev is done.
    await app.press("id:kit-month-prev");
    await app.press("id:kit-month-prev");
    await app.press("id:kit-month-prev");
    expect(app.enabled("id:kit-month-prev")).toBe(false);
    expect(app.enabled("id:kit-month-next")).toBe(true);
    // 2020 -> 2024, then June.
    await app.press("id:kit-month-next");
    await app.press("id:kit-month-next");
    await app.press("id:kit-month-next");
    await app.press("id:kit-month-next");
    await app.press("id:kit-month-m6");
    await app.waitFor("text:06/2024");
    if (size.name === "phone") {
      // A pick closes the touch sheet.
      await app.waitFor("id:kit-month-sheet", { state: "gone" });
    }
    expect(app.texts()).toContain("06/2024");

    if (app.shows("id:kit-month-next")) await app.press("id:kit-month");
    await app.press("id:kit-month");
    await app.waitFor("id:kit-month-next");
    expect(app.viewStyle("id:kit-month-m6")?.backgroundColor).not.toBe(app.viewStyle("id:kit-month-m12")?.backgroundColor);
    expect(app.enabled("id:kit-month-next")).toBe(false);
  });
}
