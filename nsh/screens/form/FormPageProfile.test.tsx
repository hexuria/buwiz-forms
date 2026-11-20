// Q1 P1b on the page: the "Save to profile?" ask is a Dialog from 600 points up and a bottom
// Sheet under it; both buttons are found by label and "Only this return" shows the kept line.
import { expect, open, test } from "nsh/test";

const ANY = "./FormPageAny.tsx";
const BIRTH = "id:fp-frm1701:txtPg1I10BirthDate";

async function toBirth(app: Awaited<ReturnType<typeof open>>) {
  await app.waitFor("text:BIR Form 1701", { timeout: 20000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-1", { timeout: 15000 });
  await app.press("id:fp-section-1");
  await app.wait(600);
  if (app.shows("id:fp-profile-1")) {
    await app.press("id:fp-profile-1");
    await app.wait(400);
  }
  for (let i = 0; i < 16 && !app.shows(BIRTH); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(BIRTH, { timeout: 15000 });
}

test("desktop: the ask is a dialog with both buttons; 'Only this return' keeps it on the return", async () => {
  const app = await open(ANY, { width: 1280, height: 900, platform: "macos", host: { params: { formCode: "1701", year: 2025, period: 0, mode: "return" } } });
  await toBirth(app);
  await app.clear(BIRTH);
  await app.type(BIRTH, "08171988");
  await app.keyboard.press("Tab");
  await app.waitFor("id:profile-ask", { timeout: 10000 });
  expect(app.shows("text:Save date of birth to the tax profile?")).toBe(true);
  await app.press("text:Only this return");
  await app.waitFor("id:profile-ask", { state: "gone", timeout: 10000 });
  await app.waitFor("text:Kept on this return only.", { timeout: 10000 });
}, 90_000);

test("phone: the ask is a bottom sheet with stacked buttons; 'Save to profile' shows the saved line", async () => {
  const app = await open(ANY, { host: { params: { formCode: "1701", year: 2026, period: 0, mode: "return" } } }); // 390 x 844
  await toBirth(app);
  await app.clear(BIRTH);
  await app.type(BIRTH, "08171988");
  await app.keyboard.press("Tab");
  await app.waitFor("id:profile-ask-sheet", { timeout: 10000 });
  expect(app.shows("text:Save date of birth to the tax profile?")).toBe(true);
  await app.press("text:Save to profile");
  await app.waitFor("id:profile-ask-sheet", { state: "gone", timeout: 10000 });
  await app.waitFor("text:Saved to the tax profile. Later returns use it.", { timeout: 10000 });
}, 90_000);
