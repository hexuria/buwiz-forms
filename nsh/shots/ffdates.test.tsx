import { open, test } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "420-light", width: 420, colorScheme: "light" as const },
  { name: "420-dark", width: 420, colorScheme: "dark" as const },
  { name: "1280-light", width: 1280, colorScheme: "light" as const },
  { name: "1280-dark", width: 1280, colorScheme: "dark" as const },
];

for (const look of LOOKS) {
  test(`dates ${look.name}`, async () => {
    const options = {
      width: look.width, height: 900, platform: "macos" as const, colorScheme: look.colorScheme,
      host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
    };
    const app = await open("../FormAnyShot.tsx", options);
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    if (look.width < 600) await app.scroll("id:form-scroll", 350);
    await app.wait(500);
    await shoot(app, `0605-blank-${look.name}`);
    const key = "id:fp-frm0605:txtDueDateMonth#date";
    const modern = app.shows(key);
    if (modern) {
      if (look.width < 600) {
        await app.press(key);
        await app.waitFor(`${key}-sheet-input`);
        await shoot(app, `0605-sheet-blank-${look.name}`);
        await app.type(`${key}-sheet-input`, "08/17/88");
        await app.press(`${key}-sheet-done`);
        await shoot(app, `0605-two-digit-${look.name}`);
        await app.clear(`${key}-sheet-input`);
        await app.type(`${key}-sheet-input`, "08/17/1");
        await app.press(`${key}-sheet-done`);
        await shoot(app, `0605-incomplete-${look.name}`);
        await app.clear(`${key}-sheet-input`);
        await app.type(`${key}-sheet-input`, "08172023");
        await app.press(`${key}-sheet-done`);
        await app.waitFor(`${key}-sheet-input`, { state: "gone" });
        await shoot(app, `0605-typed-${look.name}`);
        await app.press(key);
        await app.waitFor(`${key}-sheet-input`);
        await shoot(app, `0605-sheet-date-${look.name}`);
        await app.press(`${key}-sheet-done`);
        await app.waitFor(`${key}-sheet-input`, { state: "gone" });
      } else {
        await app.press(`${key}-calendar`);
        await app.waitFor("text:Su");
        await shoot(app, `0605-calendar-blank-${look.name}`);
        await app.press("role:button >> text:15");
        await app.clear(key);
        await app.type(key, "08172023");
        await app.type("id:fp-frm0605:txtNoOfSheets", "");
        await app.wait(500);
        await shoot(app, `0605-typed-${look.name}`);
        await app.type(key, "");
        await shoot(app, `0605-focused-${look.name}`);
        await app.clear(key);
        await app.type(key, "08/17/88");
        await app.type("id:fp-frm0605:txtNoOfSheets", "");
        await shoot(app, `0605-two-digit-${look.name}`);
        await app.clear(key);
        await app.type(key, "08/17/1");
        await app.type("id:fp-frm0605:txtNoOfSheets", "");
        await app.press("id:fp-check");
        await app.wait(500);
        await shoot(app, `0605-incomplete-${look.name}`);
        await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
      }
    } else {
      await app.press("id:fp-date-0:3");
      await app.waitFor("text:Su");
      await shoot(app, `0605-calendar-blank-${look.name}`);
      await app.press("role:button >> text:15");
    }
    await app.scroll("id:form-scroll", look.width < 600 ? 550 : 350);
    await app.wait(500);
    await shoot(app, `0605-item7-${look.name}`);

    for (const example of [
      { code: "0619E", period: 4, name: "0619e" },
      { code: "2552", period: 1, name: "2552-locked" },
      { code: "1606", period: 1, name: "1606" },
    ]) {
      const other = await open("../FormAnyShot.tsx", {
        width: look.width, height: 900, platform: "macos", colorScheme: look.colorScheme,
        host: { params: { formCode: example.code, year: 2025, period: example.period, mode: "return" } },
      });
      await other.waitFor("id:fp-check", { timeout: 30_000 });
      await other.wait(800);
      await shoot(other, `${example.name}-${look.name}`);
    }
  }, 300_000);
}
