// Rows, headers, tabs and status parts at phone and desktop size.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: tabs, list rows, setting rows`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "Tab:")).toBe("Tab: profile");
    await app.press("id:kit-tabs >> text:Forms");
    expect(readout(app, "Tab:")).toBe("Tab: forms");
    await app.press("label:2550Q Quarterly VAT");
    await app.waitFor("text:Opened 2550Q");
    expect(app.texts()).toContain("No returns filed yet.");
    await app.press("label:Sounds");
  });

  test(`${size.name}: status parts show their words`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    for (const s of ["Not started", "Draft", "Queued", "Filed", "Due today", "In 5 days", "1 day overdue", "Add a taxpayer"]) {
      expect(app.texts()).toContain(s);
    }
    expect(app.texts()).toContain("JS");
    await app.press("id:kit-pill");
    await app.press("id:kit-pill");
  });
}
