// Delete row on the real form page (FormPageAny.tsx, the demo taxpayer, a scratch database) while an amount cell of the row still
// has the keys: Schedule.tsx's remove() clears the row and closes the drawer at once, so the cell goes away as its row is cleared.
// The typed amount goes with its row; it never lands on the row that moves up into its keys, and a deleted last row stays deleted.
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
// 2550Q Schedule 3 (a compacting schedule: a deleted row's rows below move up).
const TABLE = "fp-table-txtDateCovered30";
const PAY1 = "txtIncomePayment31";

const row = (n: number, agent: string, payment: string) => ({
  ["txtDateCovered3" + n]: "01/01/2025",
  ["txtDateCovered3To" + n]: "03/31/2025",
  ["txtNameWithHoldingAgent3" + n]: agent,
  ["txtIncomePayment3" + n]: payment,
});

/** The page on 2550Q (year, quarter) with Schedule 3's rows filled, Part V open and in view, the stored row 2 amount published as `probe <value>`. */
async function schedule(year: number, period: number, rows: Record<string, string>) {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2550Q", year, period, probeKey: PAY1, setup: [{ edits: rows }] } } });
  await app.waitFor("text:BIR Form 2550Q");
  await app.press("id:fp-nav-5");
  for (let i = 0; i < 30 && !app.shows(`id:${TABLE}-add`); i++) await app.scroll("id:form-scroll", 400).catch(() => {});
  await app.waitFor(`id:${TABLE}-add`, { timeout: 10_000 });
  return app;
}

const probe = (app: Awaited<ReturnType<typeof schedule>>) => (app.texts().find((t) => t.startsWith("probe ")) ?? "").slice("probe ".length);

test("Delete row with the row's amount cell still focused: the row that moves up keeps its own amount", async () => {
  const app = await schedule(2025, 1, { ...row(0, "A0", "1000"), ...row(1, "A1", "2000"), ...row(2, "A2", "3000") });
  await app.waitUntil(() => probe(app) === "2,000.00", { timeout: 8000 });
  await app.press(`id:${TABLE.replace("fp-table-", "fp-row-")}-1`);
  await app.waitFor(`id:fp-${PAY1}`);
  await app.clear(`id:fp-${PAY1}`);
  await app.type(`id:fp-${PAY1}`, "500");
  // No leaving step: Delete row is pressed with the cell still holding the keys.
  await app.press(`id:${TABLE}-delete`);
  await app.waitUntil(() => !app.shows(`id:${TABLE}-drawer`), { timeout: 3000 });
  await app.wait(1200);
  // A2's row moved up into the deleted row's keys with its own 3,000.00, not the 500 typed in the deleted row.
  expect(probe(app)).toBe("3,000.00");
  expect(app.texts().some((t) => t.includes("Finish the amount"))).toBe(false);
}, 40_000);

test("Delete row of the last row with its amount cell still focused: the row does not come back", async () => {
  const app = await schedule(2025, 2, { ...row(0, "A0", "1000"), ...row(1, "A1", "2000") });
  await app.waitUntil(() => probe(app) === "2,000.00", { timeout: 8000 });
  await app.press(`id:${TABLE.replace("fp-table-", "fp-row-")}-1`);
  await app.waitFor(`id:fp-${PAY1}`);
  await app.clear(`id:fp-${PAY1}`);
  await app.type(`id:fp-${PAY1}`, "500");
  await app.press(`id:${TABLE}-delete`);
  await app.waitUntil(() => !app.shows(`id:${TABLE}-drawer`), { timeout: 3000 });
  await app.wait(1200);
  expect(["", "0.00"]).toContain(probe(app));
  expect(app.shows(`id:${TABLE.replace("fp-table-", "fp-row-")}-1`)).toBe(false);
}, 40_000);

// 2551Q Schedule 1 keeps its slots (no row moves up), and every slot is in the filing: a cleared slot must hold no amount.
const ATC_TABLE = "fp-table-drpATC1";

/** The page on 2551Q (year, quarter) with Schedule 1's three rows filled and its part open, `probeKey` published as `probe <value>`. */
async function schedule1(year: number, period: number, probeKey: string) {
  const rows = { drpATC1: "1", txtATCAmt1: "1000", drpATC2: "2", txtATCAmt2: "2000", drpATC3: "3", txtATCAmt3: "3000" };
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year, period, probeKey, openParts: [4], setup: [{ edits: rows }] } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor(`id:${ATC_TABLE}-add`, { timeout: 10_000 });
  return app;
}

for (const [period, r] of [[1, 1], [2, 2]] as const) {
  test(`2551Q Schedule 1: Delete row ${r + 1} with its amount cell still holding typed keys leaves no amount in the cleared slot`, async () => {
    const amt = `txtATCAmt${r + 1}`;
    const app = await schedule1(2023, period, amt);
    await app.waitUntil(() => probe(app) === `${r + 1},000.00`, { timeout: 8000 });
    await app.press(`id:fp-row-drpATC1-${r}`);
    await app.waitFor(`id:fp-${amt}`);
    // Keys typed into the cell as it is, then Delete row with the cell still holding them.
    await app.type(`id:fp-${amt}`, "500");
    await app.press(`id:${ATC_TABLE}-delete`);
    await app.waitUntil(() => !app.shows(`id:${ATC_TABLE}-drawer`), { timeout: 3000 });
    await app.wait(1200);
    expect(["", "0.00"]).toContain(probe(app));
    expect(app.texts().some((t) => t.includes("Finish the amount"))).toBe(false);
  }, 40_000);
}
