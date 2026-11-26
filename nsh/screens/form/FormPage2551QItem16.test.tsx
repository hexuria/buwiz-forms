// 2551Q Item 16 on the page: shut unless Amended Return is Yes without the 8% option; closing the gate clears it once.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const BOX = "id:fp-frm2551Qv2018:txt16";
const YES = "id:fp-frm2551Qv2018:amendedRtn_1";
const NO = "id:fp-frm2551Qv2018:amendedRtn_2";
const SHUT = "Item 16 applies only to an amended return.";
const CLEARED = "Item 16 was cleared: it applies only to an amended return.";

async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 40 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

async function reachUp(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 40 && !app.shows(sel); i++) await app.scroll("id:form-scroll", -420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

test("2551Q Item 16: shut with its sentence, opens at Amended Yes, and Amended No clears it once", async () => {
  const app = await open(ENTRY, {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "2551Q", year: 2025, period: 1, openParts: [0, 1, 2] } },
  });
  await app.waitFor("text:BIR Form 2551Q", { timeout: 20_000 });

  // Original/unanswered: the box is drawn shut, at zero, with the official reason as its sentence.
  await reach(app, BOX);
  expect(app.shows(BOX)).toBe(true);
  expect(app.texts()).toContain(SHUT);
  expect(app.texts()).toContain("0.00");
  await app.type(BOX, "42").catch(() => {});
  await app.submit(BOX).catch(() => {});
  expect(String(app.value(BOX))).not.toContain("42");
  expect(app.texts()).not.toContain("42.00");

  // Amended Yes without the 8% option opens it; typing 42 is kept and shown.
  await reachUp(app, YES);
  await app.press(YES);
  await reach(app, BOX);
  await app.waitUntil(() => !app.texts().includes(SHUT), { timeout: 10_000 });
  await app.clear(BOX);
  await app.type(BOX, "42");
  await app.submit(BOX);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await app.waitUntil(() => String(app.value(BOX)).includes("42"), { timeout: 10_000 });

  // Amended No closes the gate: the box shows zero shut, says the clearing once in the row,
  // and the saved return holds zero rather than a dormant 42.
  await reachUp(app, NO);
  await app.press(NO);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await reach(app, BOX);
  expect(app.texts()).toContain("0.00");
  expect(app.texts()).toContain(CLEARED);
  // The 42 is gone from the return's own computation, not merely hidden in the box.
  expect(app.texts()).not.toContain("42.00");
  expect(app.texts()).toContain("0.00");
}, 60_000);
