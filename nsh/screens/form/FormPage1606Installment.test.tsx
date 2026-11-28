// 1606 Items 21 to 26 on the page: shut unless Item 20 is an installment sale; closing the gate clears the installment amounts once.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const COST = "id:fp-frm1606:txtCost";
const TRANSACTION = "id:fp-radio-4:0-trigger";
const INSTALLMENT = "id:fp-radio-4:0-opt-frm1606:j_id395:_3";
const CASH = "id:fp-radio-4:0-opt-frm1606:j_id395:_1";
const SHUT = "Items 21 to 26 apply only to an installment sale. Leave them blank unless Item 20 is Installment Sale.";
const CLEARED = "Items 21 to 26 were cleared: they apply only to an installment sale.";

async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 50 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

async function reachUp(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 50 && !app.shows(sel); i++) await app.scroll("id:form-scroll", -420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

async function chooseTransaction(app: App, option: string): Promise<void> {
  await reachUp(app, TRANSACTION);
  await app.press(TRANSACTION);
  await app.waitFor(option, { timeout: 1_000 }).catch(() => app.press(TRANSACTION));
  await app.waitFor(option, { timeout: 5_000 });
  await app.press(option);
}

test("1606 installment sale: Items 21 to 26 are shut, open at Installment Sale, and close by clearing once", async () => {
  const app = await open(ENTRY, {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "1606", year: 2025, period: 1, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1606", { timeout: 20_000 });

  // Item 20 unanswered: the boxes are drawn shut and blank, with the gate's reason as their sentence.
  await reach(app, COST);
  expect(app.shows(COST)).toBe(true);
  expect(app.texts()).toContain(SHUT);
  await app.type(COST, "42").catch(() => {});
  await app.submit(COST).catch(() => {});
  expect(String(app.value(COST))).not.toContain("42");
  expect(app.texts()).not.toContain("42.00");

  // Installment Sale opens them; typing 42 is kept and shown.
  await chooseTransaction(app, INSTALLMENT);
  await reach(app, COST);
  await app.waitUntil(() => !app.texts().includes(SHUT), { timeout: 10_000 });
  await app.clear(COST);
  await app.type(COST, "42");
  await app.submit(COST);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await app.waitUntil(() => String(app.value(COST)).includes("42"), { timeout: 10_000 });

  // Cash Sale closes the gate: the box is blank shut, the row says the clearing once,
  // and the return no longer holds the typed 42.
  await chooseTransaction(app, CASH);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await reach(app, COST);
  expect(app.texts()).toContain(CLEARED);
  expect(String(app.value(COST))).not.toContain("42");
  expect(app.texts()).not.toContain("42.00");
}, 70_000);
