// Amount notes on the real form page (FormPageAny.tsx, the demo taxpayer, a scratch database): after a box is left it shows what
// the return holds, and when that is not what was typed a note under it says so. Nothing is held: Save, autosave and leaving go
// on; Queue stops on a note on an open part, the way it stops on any message under a box.
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const BOX = "id:fp-frm2551Qv2018:txt15";
const PROBE = "frm2551Qv2018:txt15";

/** The page on 2551Q (year, quarter), Part II open (Items 14 to 28), the stored txt15 published as `probe <value>`. */
async function page(year: number, period: number) {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year, period, probeKey: PROBE } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(BOX);
  return app;
}

const probe = (app: Awaited<ReturnType<typeof page>>) => (app.texts().find((t) => t.startsWith("probe ")) ?? "").slice("probe ".length);

test("a 20-digit amount: once the box is left it shows what the return holds, a note names both, and the save goes through", async () => {
  const app = await page(2025, 1);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  await app.submit(BOX);
  // The host refuses it (the official field has 12 digits): the return keeps 0.00, and the save goes through.
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  const stored = probe(app);
  expect(stored).toBe("0.00");
  // The box shows the stored amount, never the typed text (nor another number made from it); a stored zero is its placeholder.
  expect(app.value(BOX)).toBe(stored === "0.00" ? "" : stored);
  const note = app.texts().find((t) => t.includes("you typed")) ?? "";
  expect(note.startsWith("Item 15: you typed 12,345,678,901,234,567,890.00; the return holds 0.00. ")).toBe(true);
  expect(note).toContain("12-digit");
  expect(app.texts()).not.toContain("Unsaved changes");
}, 30_000);

test("the note survives folding and reopening the part, still true: the box shows the stored amount and nothing is stuck", async () => {
  const app = await page(2025, 2);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  await app.submit(BOX);
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  await app.waitUntil(() => app.texts().some((t) => t.includes("you typed")), { timeout: 5000 });
  const stored = probe(app);
  await app.press("id:fp-part-2-toggle");
  await app.waitUntil(() => !app.shows(BOX), { timeout: 3000 });
  await app.press("id:fp-part-2-toggle");
  await app.waitFor(BOX);
  expect(app.value(BOX)).toBe(stored === "0.00" ? "" : stored);
  expect(app.texts().some((t) => t.includes(`the return holds ${stored}`))).toBe(true);
  // Back in the box and out again with no edit: the note stays (focus never ends a note; an edit does).
  await app.type(BOX, "");
  await app.submit(BOX);
  await app.wait(600);
  expect(app.texts().some((t) => t.includes("you typed"))).toBe(true);
}, 30_000);

test("valid edits made after a note are saved with it: the save line says Saved and the return holds the other box's amount", async () => {
  // Each headless app has a database of its own, so "on disk" is read through the saved draft here (the fixture's probe of the
  // other box) and through a second session on the same database in session.fixes.test.ts.
  const other = "frm2551Qv2018:txt17";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 3, probeKey: other } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(BOX);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  await app.submit(BOX);
  await app.waitUntil(() => app.texts().some((t) => t.includes("you typed")), { timeout: 8000 });
  await app.clear(`id:fp-${other}`);
  await app.type(`id:fp-${other}`, "250");
  await app.submit(`id:fp-${other}`);
  await app.waitUntil(() => probe(app) === "250.00" && app.texts().includes("✓ Saved"), { timeout: 8000 });
  expect(app.texts()).toContain("saved returns 1");
  expect(app.texts()).not.toContain("Unsaved changes");
  expect(app.value(BOX)).toBe("");
}, 40_000);

/** The host's queue check spoke (the demo taxpayer's profile is incomplete): Queue went past the page's own stops. Never a send here. */
const checked = (app: Awaited<ReturnType<typeof page>>) => app.texts().some((x) => /is not set in the tax profile/.test(x));

test("Queue with the amount box still focused leaves it first and stops once on its note, naming it; the next press goes on", async () => {
  const app = await page(2025, 4);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  // No leaving step: Queue itself takes the keys out of the box.
  await app.press("id:fp-queue");
  await app.waitUntil(() => app.texts().some((t) => t.includes("you typed")), { timeout: 5000 });
  await app.wait(400);
  // The stored amount (0.00; shown empty as an untouched zero amount is, whether or not the page's reveal put the keys back in the box).
  expect([probe(app), ""]).toContain(app.value(BOX) ?? "");
  // The toast says the note (it names Item 15), on top of the note under the box.
  expect(app.texts().filter((t) => t.startsWith("Item 15: you typed")).length).toBeGreaterThanOrEqual(2);
  expect(checked(app)).toBe(false);
  expect(app.shows("text:Queue this return?")).toBe(false);
  // Shown once: the next press goes on to the host's checks, the note still under its box.
  await app.press("id:fp-queue");
  await app.waitUntil(() => checked(app), { timeout: 5000 });
  expect(app.texts().some((t) => t.startsWith("Item 15: you typed"))).toBe(true);
  expect(app.shows("text:Queue this return?")).toBe(false);
}, 40_000);

test("a note in a folded part stops Queue once: Queue opens the part, brings the box into view and names the item", async () => {
  const app = await page(2024, 4);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  await app.submit(BOX);
  await app.waitUntil(() => app.texts().some((t) => t.startsWith("Item 15: you typed")), { timeout: 8000 });
  await app.press("id:fp-part-2-toggle");
  await app.waitUntil(() => !app.shows(BOX), { timeout: 3000 });
  await app.press("id:fp-queue");
  await app.waitFor(BOX, { timeout: 5000 });
  await app.wait(400);
  expect(app.texts().filter((t) => t.startsWith("Item 15: you typed")).length).toBeGreaterThanOrEqual(2);
  expect(checked(app)).toBe(false);
  await app.press("id:fp-queue");
  await app.waitUntil(() => checked(app), { timeout: 5000 });
}, 40_000);

// A box that goes away while it has the keys (its part folds) runs its leave logic from its cleanup and unregisters its blur: the money
// box's side of note rule 4, not in this tree yet. Until then the session still holds the box as focused, Queue's settle waits on a
// blur that never comes and stops with "Finish the amount in Item 15 first." Turn BOX_LEAVES_ON_UNMOUNT on once it lands.
const BOX_LEAVES_ON_UNMOUNT = true;

(BOX_LEAVES_ON_UNMOUNT ? test : test.skip)("a note raised as the navigator folds its part still stops Queue: Queue opens the part, shows the box and names the item", async () => {
  const app = await page(2024, 3);
  await app.clear(BOX);
  await app.type(BOX, "12345678901234567890");
  // Another part in the navigator: the box is left as its part folds.
  await app.press("id:fp-nav-1");
  await app.waitUntil(() => !app.shows(BOX), { timeout: 3000 });
  await app.wait(400);
  await app.press("id:fp-queue");
  await app.waitFor(BOX, { timeout: 5000 });
  await app.wait(400);
  expect(app.texts().filter((t) => t.startsWith("Item 15: you typed")).length).toBeGreaterThanOrEqual(2);
  expect([probe(app), ""]).toContain(app.value(BOX) ?? "");
  expect(checked(app)).toBe(false);
  await app.press("id:fp-queue");
  await app.waitUntil(() => checked(app), { timeout: 5000 });
}, 40_000);

// Unreadable text: on leaving, the money box puts the return's amount back and calls onLeftUnreadable; the session draws the note.
const unreadable = test;

for (const typed of ["1500x", "15,00,50"]) {
  unreadable(`unreadable ${typed}: after leaving, the box shows the old amount, a note names both, the draft holds the old amount, Save works`, async () => {
    const app = await page(2024, typed === "1500x" ? 1 : 2);
    await app.clear(BOX);
    await app.type(BOX, "1000");
    await app.submit(BOX);
    await app.waitFor("text:✓ Saved", { timeout: 8000 });
    await app.clear(BOX);
    await app.type(BOX, typed);
    await app.submit(BOX);
    await app.waitUntil(() => app.texts().some((t) => t.includes("is not an amount")), { timeout: 5000 });
    expect(app.value(BOX)).toBe("1,000.00");
    const hint = typed.includes(",") ? " Use a point for centavos, and commas only between thousands." : "";
    expect(app.texts()).toContain(`Item 15: ${typed} is not an amount. The box kept 1,000.00.${hint}`);
    expect(probe(app)).toBe("1,000.00");
    await app.press("id:fp-save").catch(() => {});
    await app.waitFor("text:✓ Saved", { timeout: 8000 });
  }, 30_000);
}

test("[comma] decimal comma 1500,50: after leaving, the box shows the point amount and one note says how the comma was read; Save and Queue go on", async () => {
  const app = await page(2024, 2);
  await app.type(BOX, "");
  await app.waitUntil(() => app.value(BOX) === "", { timeout: 3000 });
  await app.type(BOX, "1500,50");
  await app.submit(BOX);
  await app.waitUntil(() => app.value(BOX) === "1,500.50", { timeout: 5000 });
  // The fixture republishes the draft's amount every 100 ms.
  await app.waitUntil(() => probe(app) === "1,500.50", { timeout: 3000 });
  expect(app.texts().some((t) => t.includes("is not an amount"))).toBe(false);
  const said = "Item 15: 1500,50 was read as 1,500.50.";
  await app.waitUntil(() => app.texts().includes(said), { timeout: 3000 });
  expect(app.texts().filter((t) => t === said).length).toBe(1);
  // Drawn as a note (the row's note line), not as the box's red message.
  expect(app.shows(`id:fp-boxnote-${PROBE}`)).toBe(true);
  expect(app.shows(`id:fp-boxmsg-${PROBE}`)).toBe(false);
  await app.press("id:fp-save").catch(() => {});
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  // The note stops nothing: the first Queue goes on to the host's checks, with no toast of it.
  await app.press("id:fp-queue");
  await app.waitUntil(() => checked(app), { timeout: 5000 });
  expect(app.texts().filter((t) => t === said).length).toBe(1);
}, 40_000);

// 2551Q Schedule 1, row 1's Taxable Amount: a cell edited in its row's drawer.
const CELL = "id:fp-txtATCAmt1";
const ROW = "id:fp-row-drpATC1-0";
const TABLE = "fp-table-drpATC1";

/** The page on 2551Q (year, quarter) with Schedule 1's row 1 holding 1,234.56, its drawer open on the Taxable Amount. */
async function scheduleCell(year: number, period: number) {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year, period, probeKey: "txtATCAmt1", openParts: [4], edits: { txtATCAmt1: "1234.56" } } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor(ROW, { timeout: 8000 });
  await app.press(ROW);
  await app.waitFor(CELL);
  return app;
}

/** Types `text` key by key, as a filer does, so the host answers (and refuses) on the way, then Tab leaves the cell. */
async function typeAndTab(app: Awaited<ReturnType<typeof scheduleCell>>, text: string) {
  await app.clear(CELL);
  for (const ch of text) {
    await app.type(CELL, ch);
    await app.wait(40);
  }
  await app.wait(400);
  await app.keyboard.press("tab");
  await app.wait(800);
}

for (const [period, typed, shown] of [[1, "12345678901234567890", "12,345,678,901,234,567,890.00"], [2, "1234567890123", "1,234,567,890,123.00"]] as const) {
  test(`refused amount in a schedule cell: ${typed.length} digits, Tab: the cell shows what the return holds and the sentence names what was typed, also after focus comes back`, async () => {
    const app = await scheduleCell(2022, period);
    await typeAndTab(app, typed);
    const said = `Taxable Amount: you typed ${shown}; the return holds 1,234.56.`;
    expect(app.value(CELL)).toBe("1,234.56");
    expect(probe(app)).toBe("1,234.56");
    expect(app.texts().some((t) => t.startsWith(said))).toBe(true);
    // Focus coming back to the cell (the window's or the drawer's own) does not erase the sentence.
    await app.type(CELL, "");
    await app.keyboard.press("tab");
    await app.wait(400);
    expect(app.texts().some((t) => t.startsWith(said))).toBe(true);
  }, 40_000);
}

test("note in a row drawer: with the drawer closed and its part folded, Queue opens both, the cell and its sentence are on screen, and the next press goes on", async () => {
  const app = await scheduleCell(2022, 3);
  await typeAndTab(app, "12345678901234567890");
  const said = "Taxable Amount: you typed 12,345,678,901,234,567,890.00; the return holds 1,234.56.";
  const saying = () => app.texts().filter((t) => t.startsWith(said)).length;
  await app.press(`id:${TABLE}-done`);
  await app.waitUntil(() => !app.shows(CELL), { timeout: 3000 });
  await app.press("id:fp-part-4-toggle");
  await app.waitUntil(() => !app.shows(ROW), { timeout: 3000 });
  await app.press("id:fp-queue");
  await app.waitFor(CELL, { timeout: 5000 });
  await app.wait(400);
  // Under the cell, and the toast.
  expect(saying()).toBeGreaterThanOrEqual(2);
  expect(app.value(CELL)).toBe("1,234.56");
  const before = saying();
  await app.press("id:fp-queue");
  await app.wait(1200);
  // Shown once: no second toast of it; Queue went on to the host's checks.
  expect(saying()).toBe(before);
  expect(checked(app)).toBe(true);
}, 40_000);

test("[Z1] unreadable text in a NEW 2551Q Schedule 1 row: the row was never stored, so the note says the box was left blank", async () => {
  const pay = "txtATCAmt2";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2021, period: 1, probeKey: pay, openParts: [4], edits: { txtATCAmt1: "1234.56" } } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor(ROW, { timeout: 8000 });
  // Add row opens row 2, nothing in it yet: its Taxable Amount holds no amount.
  await app.press("text:Add row");
  await app.waitFor(`id:fp-${pay}`, { timeout: 5000 });
  for (const ch of "abc") {
    await app.type(`id:fp-${pay}`, ch);
    await app.wait(40);
  }
  await app.keyboard.press("tab");
  await app.waitUntil(() => app.texts().some((t) => t.includes("abc is not an amount")), { timeout: 5000 });
  expect(app.texts().some((t) => t.endsWith("Taxable Amount: abc is not an amount. The box was left blank."))).toBe(true);
  expect(app.texts().some((t) => t.includes("The box kept 0.00"))).toBe(false);
  expect(app.texts()).toContain("1 row");
}, 40_000);

test("[Z1] unreadable text in a stored zero 2551Q Schedule 1 amount: the note still says the box kept 0.00", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2021, period: 2, probeKey: "txtATCAmt1", openParts: [4], edits: { txtATCAmt1: "0" } } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor(ROW, { timeout: 8000 });
  await app.press(ROW);
  await app.waitFor(CELL);
  for (const ch of "abc") {
    await app.type(CELL, ch);
    await app.wait(40);
  }
  await app.keyboard.press("tab");
  await app.waitUntil(() => app.texts().some((t) => t.includes("abc is not an amount")), { timeout: 5000 });
  expect(app.texts().some((t) => t.endsWith("Taxable Amount: abc is not an amount. The box kept 0.00."))).toBe(true);
  expect(probe(app)).toBe("0.00");
}, 40_000);

// 2550Q Schedule 3 (a nullable schedule: the cells of a row not started hold no amount, not zero).
const S3_TABLE = "fp-table-txtDateCovered30";

test("[Z1] unreadable text left in a blank schedule cell: the cell stays blank and the note says it was left blank, not that it kept 0.00", async () => {
  const edits = { txtDateCovered30: "01/01/2024", txtDateCovered3To0: "03/31/2024", txtNameWithHoldingAgent30: "A0", txtIncomePayment30: "1000" };
  const pay = "txtIncomePayment31";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2550Q", year: 2024, period: 1, probeKey: pay, setup: [{ edits }] } } });
  await app.waitFor("text:BIR Form 2550Q");
  await app.press("id:fp-nav-5");
  for (let i = 0; i < 30 && !app.shows(`id:${S3_TABLE}-add`); i++) await app.scroll("id:form-scroll", 400).catch(() => {});
  // Add row opens row 2, nothing in it yet: its Income Payment holds no amount.
  await app.press(`id:${S3_TABLE}-add`);
  await app.waitFor(`id:fp-${pay}`);
  expect(probe(app)).toBe("");
  // Text that is no amount from its first key: the box reports nothing, so the row stays not started.
  for (const ch of "abc") {
    await app.type(`id:fp-${pay}`, ch);
    await app.wait(40);
  }
  await app.keyboard.press("tab");
  await app.waitUntil(() => app.texts().some((t) => t.includes("abc is not an amount")), { timeout: 5000 });
  expect(app.texts().some((t) => t.endsWith("abc is not an amount. The box was left blank."))).toBe(true);
  expect(app.texts().some((t) => t.includes("The box kept"))).toBe(false);
  expect(probe(app)).toBe("");
}, 40_000);
