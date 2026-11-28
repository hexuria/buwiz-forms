// 1601FQ Schedule 1 on the page: shut until Item 13 is Yes; closing Item 13 clears typed rows once.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const TABLE = "fp-table-drpTreatyCode0";
const AMOUNT = "id:fp-txtAmtIncomePay0";
const YES = "id:fp-frm1601FQ:SpecialTax_1";
const NO = "id:fp-frm1601FQ:SpecialTax_2";
const SHUT = "You are not availing of tax relief under Special Law or International Tax Treaty. There is no need to fill up schedule 1.";
const CLEARED = "Schedule 1 row 1 was cleared: it applies only when item 13 is Yes.";

async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 60 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

async function reachUp(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 60 && !app.shows(sel); i++) await app.scroll("id:form-scroll", -420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}


async function openRow(app: App): Promise<void> {
  await reach(app, `id:${TABLE}`);
  if (app.shows(`id:${TABLE}-add`)) {
    await app.press(`id:${TABLE}-add`);
  } else {
    await app.press(`id:fp-row-drpTreatyCode0-0`);
  }
  await app.waitFor("text:Row 1", { timeout: 10_000 });
}

function amountShown(app: App): boolean {
  const texts = app.texts();
  const at = texts.indexOf("Amount of Income Payment (E)");
  return at >= 0 && texts.slice(at, at + 5).includes("0.00");
}

async function closeRow(app: App): Promise<void> {
  if (app.shows(`id:${TABLE}-done`)) {
    await app.press(`id:${TABLE}-done`).catch(() => {});
    await app.waitFor("text:Row 1", { state: "gone", timeout: 5_000 }).catch(() => {});
  }
}

test("1601FQ Schedule 1 is shut before Item 13 Yes, opens there, and closes by clearing once", async () => {
  const app = await open(ENTRY, {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "1601FQ", year: 2025, period: 1, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1601FQ", { timeout: 20_000 });

  await openRow(app);
  expect(app.shows(AMOUNT)).toBe(true);
  expect(app.texts()).toContain(SHUT);
  expect(amountShown(app)).toBe(true);
  await app.type(AMOUNT, "42").catch(() => {});
  await app.submit(AMOUNT).catch(() => {});
  expect(app.texts()).not.toContain("42.00");
  await closeRow(app);

  await reachUp(app, YES);
  await app.press(YES);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await openRow(app);
  await app.waitUntil(() => !app.texts().includes(SHUT), { timeout: 10_000 });
  await app.clear(AMOUNT);
  await app.type(AMOUNT, "42");
  await app.submit(AMOUNT);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await app.waitUntil(() => String(app.value(AMOUNT)).includes("42"), { timeout: 10_000 });
  await closeRow(app);

  await reachUp(app, NO);
  await app.press(NO);
  await app.press("text:Continue").catch(() => {});
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await reach(app, `id:${TABLE}`);
  expect(app.texts()).toContain(CLEARED);
  await openRow(app);
  expect(amountShown(app)).toBe(true);
  expect(app.texts()).not.toContain("42.00");
}, 70_000);
