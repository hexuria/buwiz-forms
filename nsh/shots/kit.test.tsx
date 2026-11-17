// Real-window screenshots of the UI kit's gallery: wide and narrow, light and dark, the page in steps from top to end,
// then the layers open (dialog, sheet, menu, select list, date picker, toast, a hovered button).
// `just nsh-shots kit` saves ~/BuwizNsh/evidence/kit/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS) {
  test(`kit gallery, ${look.name}`, async () => {
    const app = await open("../BasePreview.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme });
    await app.press("text:UI kit");
    await app.waitFor("text:Text and marks");
    expect(app.texts()).toContain("Layers");
    const step = look.height - 120;
    await shoot(app, `kit-${look.name}-00`);
    // Down a screen at a time; the last step goes to the end (the error says how far that is).
    for (let i = 1; i < 40; i++) {
      let last = false;
      try {
        await app.scroll("id:kit-scroll", step);
      } catch (error) {
        const at = /at (\d+(?:\.\d+)?) of (\d+(?:\.\d+)?)/.exec(String(error));
        if (!at) throw error;
        const rest = Number(at[2]) - Number(at[1]);
        if (rest < 1) break;
        await app.scroll("id:kit-scroll", rest);
        last = true;
      }
      await shoot(app, `kit-${look.name}-${String(i).padStart(2, "0")}`);
      if (last) break;
    }
  }, 180_000);

  test(`kit layers, ${look.name}`, async () => {
    const app = await open("../BasePreview.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme });
    await app.press("text:UI kit");
    await app.waitFor("text:Text and marks");
    await app.hover("id:kit-primary");
    await shoot(app, `kit-${look.name}-hover`);
    await app.press("id:kit-dropdown-trigger");
    await app.waitFor("text:Form 0619-E");
    await shoot(app, `kit-${look.name}-select`);
    await app.back();
    await app.press("id:kit-open-dialog");
    await app.waitFor("text:Delete this return?");
    await shoot(app, `kit-${look.name}-dialog`);
    await app.back();
    await app.waitFor("text:Delete this return?", { state: "gone" });
    await app.press("id:kit-open-sheet");
    await app.waitFor("text:Schedule 1, row 3");
    await app.wait(400);
    await shoot(app, `kit-${look.name}-sheet`);
    await app.press("label:Close");
    await app.waitFor("text:Schedule 1, row 3", { state: "gone" });
    await app.press("text:More");
    await app.waitFor("text:Duplicate");
    await shoot(app, `kit-${look.name}-menu`);
    await app.press("text:Delete return");
    await app.waitFor("text:Return deleted");
    await app.wait(500);
    await shoot(app, `kit-${look.name}-toast`);
    await app.press("id:kit-date-calendar");
    await app.wait(400);
    await shoot(app, `kit-${look.name}-datepicker`);
    await app.back();
    await app.press("id:kit-who-trigger");
    await app.waitFor("id:kit-who-clear");
    await shoot(app, `kit-${look.name}-multicombo`);
  }, 90_000);
}
