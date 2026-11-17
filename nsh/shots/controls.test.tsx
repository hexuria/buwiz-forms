// Real-window screenshots of the form page's field controls (their gallery): wide and narrow, light and dark, the page in steps
// from top to end, then the layers the controls open (a dropdown, the searched list, the calendar, a schedule row's drawer).
// `just nsh-shots controls` saves ~/BuwizNsh/evidence/controls/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS) {
  test(`controls gallery, ${look.name}`, async () => {
    const app = await open("../ControlsShot.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme });
    await app.waitFor("text:Field controls");
    expect(app.texts()).toContain("Schedule table");
    const step = look.height - 120;
    await shoot(app, `controls-${look.name}-00`);
    // A screen at a time. Headless, a scroll past the end throws ("it is at X of Y"); a real window stops quietly there, so the
    // steps past the end repeat the last picture: the fixed count is generous (wide needs 4 pictures, narrow 6) and the
    // repeats are equal files, easy to drop.
    const steps = look.width >= 600 ? 5 : 12;
    for (let i = 1; i <= steps; i++) {
      try {
        await app.scroll("id:controls-scroll", step);
      } catch (error) {
        const at = /at (\d+(?:\.\d+)?) of (\d+(?:\.\d+)?)/.exec(String(error));
        if (!at) throw error;
        const rest = Number(at[2]) - Number(at[1]);
        if (rest >= 1) {
          await app.scroll("id:controls-scroll", rest);
          await shoot(app, `controls-${look.name}-${String(i).padStart(2, "0")}`);
        }
        break;
      }
      await shoot(app, `controls-${look.name}-${String(i).padStart(2, "0")}`);
    }
  }, 180_000);

  test(`controls layers, ${look.name}`, async () => {
    const app = await open("../ControlsShot.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme });
    await app.waitFor("text:Field controls");
    // The schedule drawer, over the page.
    await app.press("id:fp-row-sch1pay-0");
    await app.waitFor("text:Row 1");
    await app.wait(500);
    await shoot(app, `controls-${look.name}-drawer`);
    await app.press("id:fp-sch1atc-trigger");
    await app.waitFor("text:WC010 - Professional fees, juridical");
    await app.wait(300);
    await shoot(app, `controls-${look.name}-drawer-select`);
    await app.back();
    await app.press("id:fp-table-sch1pay-done");
    await app.waitFor("text:Row 1", { state: "gone" });
    // A few choices, then a long list searched by words.
    await app.press("id:fp-month-trigger");
    await app.waitFor("text:04 - April");
    await shoot(app, `controls-${look.name}-select`);
    await app.back();
    await app.press("id:fp-atc-trigger");
    await app.waitFor("id:fp-atc-search");
    await app.type("id:fp-atc-search", "juridical fees");
    await app.waitFor("id:fp-atc-opt-WC011");
    await app.wait(300);
    await shoot(app, `controls-${look.name}-search`);
    await app.back();
    // The calendar of a date box.
    await app.press("id:fp-date-calendar");
    await app.wait(500);
    await shoot(app, `controls-${look.name}-calendar`);
  }, 180_000);
}
