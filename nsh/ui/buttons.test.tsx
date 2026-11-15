// Buttons and rows at phone and desktop size: presses count, disabled ones refuse, toggles and nav rows follow state.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: buttons press, disabled ones do not`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(readout(app, "Presses:")).toBe("Presses: 0");
    await app.press("id:kit-primary");
    await app.press("text:Secondary >> nth=0");
    await app.press("text:New return >> nth=0");
    expect(readout(app, "Presses:")).toBe("Presses: 3");
    expect(app.enabled("id:kit-primary")).toBe(true);
    expect(app.enabled("id:kit-primary-off")).toBe(false);
    await expect(app.press("id:kit-primary-off")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    expect(readout(app, "Presses:")).toBe("Presses: 3");
  });

  test(`${size.name}: icon buttons and nav rows`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("label:Notifications");
    expect(readout(app, "Presses:")).toBe("Presses: 1");
    expect(app.enabled("label:Delete")).toBe(false);
    await app.press("id:kit-sidebar");
    await app.press("id:kit-nav-forms");
    await app.press("id:kit-nav-home");
    expect(app.texts()).toContain("Form library");
  });
}
