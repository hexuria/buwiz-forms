// The kit's pressable card and cell, and the status chips for the filed states, at phone and desktop size.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";
import { statusChip } from "./logic";

test("status chips name the filed states: Submitted (a send mark), Confirmed and Paid (a check)", () => {
  expect(statusChip("submitted")).toEqual({ label: "Submitted", tone: "neutral", check: false, send: true });
  expect(statusChip("confirmed")).toEqual({ label: "Confirmed", tone: "success", check: true });
  expect(statusChip("paid")).toEqual({ label: "Paid", tone: "success", check: true });
  expect(statusChip("filed").label).toBe("Filed");
  expect(statusChip("Submitted").label).toBe("Not started"); // names are lower case: the kit's words, not a database's
});

for (const size of SIZES) {
  test(`${size.name}: a card presses, shows the selected state, and a disabled one refuses`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "Card presses:")).toBe("Card presses: 0");
    await app.press("id:kit-press-tile");
    await app.press("id:kit-press-row");
    expect(readout(app, "Card presses:")).toBe("Card presses: 2");
    expect(app.texts()).toContain("Not picked");
    await app.press("id:kit-press-pick");
    expect(app.texts()).toContain("Picked");
    await app.press("id:kit-press-pick");
    expect(app.texts()).toContain("Not picked");
    expect(app.enabled("id:kit-press-tile")).toBe(true);
    expect(app.enabled("id:kit-press-off")).toBe(false);
    await expect(app.press("id:kit-press-off")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    expect(readout(app, "Card presses:")).toBe("Card presses: 2");
    // What screen readers say of the card is its label.
    expect(app.shows("label:Form 2551Q")).toBe(true);
  });

  test(`${size.name}: a cell presses and the chosen one is the only selected`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "Day:")).toBe("Day: 11");
    await app.press("id:kit-cell-12");
    expect(readout(app, "Day:")).toBe("Day: 12");
    await app.press("id:kit-strip-14");
    expect(readout(app, "Day:")).toBe("Day: 14");
    await app.press("label:November 13");
    expect(readout(app, "Day:")).toBe("Day: 13");
  });

  test(`${size.name}: the gallery shows the words of the filed states`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    for (const s of ["Submitted", "Confirmed", "Paid"]) expect(app.texts()).toContain(s);
  });
}
