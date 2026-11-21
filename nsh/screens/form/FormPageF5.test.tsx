// The F5 fixes of the real-window verifier's form-page findings (FormPageAny.tsx, the demo taxpayer, a scratch database).
// A 2551Q of 2026 Q3 for the demo taxpayer has four host messages after Check, three of them on Item 13 and one on
// Item 20, and its to-do list after Check is those two rows.
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const Q3 = { formCode: "2551Q", year: 2026, period: 3 };

test("F5-2 desktop: Check's toast counts every host message; the pill and the part badges count the rows holding one", async () => {
  // F9-1: the owner's pick is the old page's numbers (form_page.js check toast = errors.length; drawPill red = to-do
  // rows with a message; partErrors = rows with a message), so 4 in the toast, 2 on the pill, 1 and 1 on the parts.
  const app = await open(ENTRY, { ...DESKTOP, host: { params: Q3 } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("text:4 items need attention.", { timeout: 5000 });
  await app.waitFor("label:2 items need attention");
  expect(app.shows("id:fp-pill-count >> text:2")).toBe(true);
  // Item 13 (Part I) carries three messages, Item 20 (Part II) one: one row each.
  expect(app.shows("id:fp-nav-1 >> text:1")).toBe(true);
  expect(app.shows("id:fp-nav-2 >> text:1")).toBe(true);
});

test("F5-3 desktop: after Expand all a step opens nothing new and folds nothing; past the last to-do Check runs and ‹ steps back", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { ...Q3, period: 3 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-parts-toggle");
  await app.waitFor("text:5 parts");
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 13 · /");
  // Every part is still open: the navigator says "5 parts", not "2 of 5".
  expect(app.texts()).toContain("5 parts");
  expect(app.shows("label:Collapse all parts")).toBe(true);
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 20 · /");
  expect(app.texts()).toContain("5 parts");
  // Past the last to-do: › is Check; it runs, and ‹ is still there and goes back to the last to-do.
  await app.press("id:fp-pill-next");
  await app.waitFor("text:4 items need attention.", { timeout: 5000 });
  await app.waitFor("text:/^2 to do · /");
  expect(app.enabled("id:fp-pill-prev")).toBe(true);
  await app.press("id:fp-pill-prev");
  await app.waitFor("text:/^Item 20 · /");
});

test("phone: previous and next to-do reopen the collapsed part containing their row", async () => {
  const app = await open(ENTRY, { host: { params: Q3 } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-pill-next");
  await app.waitFor("id:fp-item-1:7");
  await app.press("id:fp-part-1-toggle");
  await app.waitFor("id:fp-item-1:7", { state: "gone" });
  await app.press("id:fp-pill-next");
  await app.waitFor("id:fp-item-2:6");
  await app.press("id:fp-part-2-toggle");
  await app.waitFor("id:fp-item-2:6", { state: "gone" });
  await app.press("id:fp-pill-prev");
  await app.waitFor("id:fp-item-1:7");
  await app.press("id:fp-pill-next");
  await app.waitFor("id:fp-item-2:6");
});

test("F5-4 desktop: the pill names the part at the top of the viewport while the page scrolls", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-parts-toggle");
  await app.waitFor("text:5 parts");
  // With rows to do the pill reads "n to do · <part>" (F9-1, old drawPill); the part is still the one at the top.
  const first = "text:/ to do · Page 1$/";
  await app.waitFor(first);
  for (let i = 0; i < 10 && app.shows(first); i++) await app.scroll("id:form-scroll", 400).catch(() => {});
  await app.waitFor("text:/ to do · (Part|Schedule) /");
  // Back at the top: the first part again.
  for (let i = 0; i < 20 && !app.shows(first); i++) await app.scroll("id:form-scroll", -400).catch(() => {});
  await app.waitFor(first);
});

test("F5-5 phone: a fact card's Segmented choice takes the press and saves", async () => {
  const app = await open(ENTRY, { host: { params: Q3 } }); // 390 x 844
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-part-1-toggle");
  // F2: income type's choices are a Segmented of short labels ("Mixed income"), not the long card text.
  for (let i = 0; i < 12 && !app.shows("label:Mixed income"); i++) await app.scroll("id:form-scroll", 400).catch(() => {});
  await app.press("label:Mixed income");
  // A pick is the explicit answer and saves at once: the card's note turns to the saved words.
  await app.waitFor("text:/saved to the tax profile/");
});
