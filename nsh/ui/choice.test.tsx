// Choice controls at phone and desktop size: switch, checkboxes, radios, segments, dropdown, combo field, multi combo.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: switch, checkbox, radio, segmented`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.texts()).toContain("Autosave on");
    await app.press("id:kit-switch");
    expect(app.texts()).toContain("Autosave off");
    expect(readout(app, "Include:")).toBe("Include: yes | Declared: no");
    await app.press("id:kit-check");
    await app.press("id:kit-agree");
    expect(readout(app, "Include:")).toBe("Include: no | Declared: yes");
    await app.press("text:8% flat rate");
    await app.press("text:Month >> nth=0");
    expect(readout(app, "View:")).toBe("View: month | Regime: eight_percent");
    // The disabled radio cannot be chosen.
    await expect(app.press("text:Not chosen yet")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
  });

  test(`${size.name}: dropdown opens, chooses, closes`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "Form:")).toBe("Form: none");
    await app.press("id:kit-dropdown-trigger");
    await app.waitFor("text:Form 1701Q");
    await app.press("text:Form 1701Q");
    expect(readout(app, "Form:")).toBe("Form: 1701Q");
    await app.waitFor("text:Form 0619-E", { state: "gone" });
    expect(app.texts()).toContain("Form 1701Q");
  });

  test(`${size.name}: combo field filters a long list by typing`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "RDO:")).toBe("RDO: 050");
    await app.clear("id:kit-rdo-input");
    await app.type("id:kit-rdo-input", "east mak");
    await app.waitFor("text:047 - East Makati");
    expect(app.texts()).not.toContain("039 - South Quezon City");
    await app.press("text:047 - East Makati");
    expect(readout(app, "RDO:")).toBe("RDO: 047");
    expect(app.value("id:kit-rdo-input")).toBe("047 - East Makati");
  });

  test(`${size.name}: multi combo selects, clears, selects all`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.texts()).toContain("All taxpayers");
    await app.press("id:kit-who-trigger");
    await app.waitFor("id:kit-who-clear");
    await app.press("id:kit-who-clear");
    expect(readout(app, "Taxpayers:")).toBe("Taxpayers: none");
    await app.press("id:kit-who-opt-000123453");
    expect(readout(app, "Taxpayers:")).toBe("Taxpayers: 000123453");
    await app.type("id:kit-who-search", "sample");
    await app.press("id:kit-who-all");
    expect(readout(app, "Taxpayers:")).toBe("Taxpayers: all");
  });
}
