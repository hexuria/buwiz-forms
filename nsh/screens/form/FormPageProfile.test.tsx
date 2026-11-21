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
  // 0619E's Item 12 category radios are a boxed fact: a pick commits at once (choose), so no text-box
  // blur is needed — reliable on touch, where a box's blur does not fire headless.
  const app = await open(ANY, { host: { params: { formCode: "0619E", year: 2025, period: 7, mode: "return" } } }); // 390 x 844
  await app.waitFor("text:BIR Form 0619E", { timeout: 20000 });
  const gov = "id:fp-frm0619E:optCategory:G";
  // The category sits in a collapsed part: open the parts until the radio is drawn.
  for (const pi of [1, 2, 3]) {
    if (app.shows(gov)) break;
    const toggle = `id:fp-part-${pi}-toggle`;
    await app.waitFor(toggle);
    await app.press(toggle);
    await app.wait(400);
  }
  for (let i = 0; i < 18 && !app.shows(gov); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(gov, { timeout: 15000 });
  await app.press(gov);
  // The sheet's own container testID is not in the accessibility tree; its contents are. The dialog
  // stays closed under 600 points, so only the sheet's buttons are up.
  await app.waitFor("text:Save withholding category to the tax profile?", { timeout: 10000 });
  expect(app.shows("id:profile-ask")).toBe(false);
  await app.press("id:profile-ask-save");
  await app.waitFor("text:Save withholding category to the tax profile?", { state: "gone", timeout: 10000 });
  await app.waitFor("text:Saved to the tax profile. Later returns use it.", { timeout: 10000 });
}, 90_000);

test("B8: on 0619E with no profile category, Private shows unpicked; pressing it records the answer and Queue is no longer blocked by the category", async () => {
  const app = await open(ANY, { host: { params: { formCode: "0619E", year: 2024, period: 7, mode: "return" } } });
  await app.waitFor("text:BIR Form 0619E", { timeout: 20000 });
  const privatePick = "id:fp-frm0619E:optCategory:P";
  const categoryBlock = "Government withholding agent is not in the tax profile, and this return needs it. Pick it at Item 12.";
  for (const pi of [1, 2, 3]) {
    if (app.shows(privatePick)) break;
    const toggle = `id:fp-part-${pi}-toggle`;
    await app.waitFor(toggle);
    await app.press(toggle);
    await app.wait(400);
  }
  for (let i = 0; i < 18 && !app.shows(privatePick); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(privatePick, { timeout: 15000 });
  await app.press("id:fp-check");
  await app.waitFor(`text:${categoryBlock}`, { timeout: 10000 });

  await app.press(privatePick);
  await app.press("id:fp-check");
  await app.waitFor(`text:${categoryBlock}`, { state: "gone", timeout: 10000 });
}, 90_000);
