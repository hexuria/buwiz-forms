// 2551Q Schedule 1 by hand: the ATC dropdown of row 1 changes, rows 2 to 6 take a pick (F1).
import { expect, open, test } from "nsh/test";

test("phone: row 1's ATC changes and row 2 takes a pick", async () => {
  const app = await open("./FormPagePreview.tsx");
  await app.waitFor("text:BIR Form 2551Q");
  const graduated = app.texts().some((t) => /Graduated income tax rate/.test(t));
  if (graduated) await app.press("text:Graduated income tax rate");
  // The not-yet note is gone from every Schedule 1 ATC cell.
  expect(app.texts().some((t) => t.includes("Can't be filled on this page yet."))).toBe(false);
  // Open the Schedule 1 part through the pill's sections list.
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-4", { timeout: 15_000 });
  await app.press("id:fp-section-4");
  await app.wait(900);
  // Row 1 holds PT010 already; open its drawer and change the pick to PT040.
  await app.waitFor("text:/PT010 · 3.0/", { timeout: 8000 });
  await app.press("text:/PT010 · 3.0/");
  await app.waitFor("id:fp-drpATC1-trigger", { timeout: 5000 });
  await app.press("id:fp-drpATC1-trigger");
  await app.waitFor("text:/PT040/", { timeout: 5000 });
  await app.press("text:/^PT040/");
  await app.wait(500);
  await app.press("text:Done");
  await app.wait(500);
  expect(app.texts().some((t) => /PT040/.test(t))).toBe(true);
  // Add row opens row 2; it takes a pick and an amount, and the page saves.
  await app.press("text:Add row");
  await app.waitFor("id:fp-drpATC2-trigger", { timeout: 5000 });
  await app.press("id:fp-drpATC2-trigger");
  await app.waitFor("text:/PT060/", { timeout: 5000 });
  await app.press("text:/^PT060/");
  await app.wait(400);
  await app.clear("id:fp-txtATCAmt2");
  await app.type("id:fp-txtATCAmt2", "2000");
  await app.press("text:Done");
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  expect(app.texts().some((t) => /PT060/.test(t))).toBe(true);
}, 60_000);
