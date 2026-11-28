// The pieces the screens and the frame share, tested together in the mounted frame (desktop 1440 x 900): a removal's Undo toast, what
// is typed saved before the app leaves the form page, an amendment's first save reaching the history, a saved return listed at once
// in Filings, and the dev start route's keys reaching a screen. Fixtures: ShellDemo.tsx (the demo taxpayer) and FrameDemo.tsx
// (a stored 1601EQ with an ATC row, a filed 2550Q to amend).
import { expect, open, test } from "nsh/test";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const YEAR = new Date().getFullYear();
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
/** 2551Q "No. of Sheets Attached": a two-digit box in the first part. It starts as "0" and a typed key lands beside it ("07"). */
const SHEETS = "id:fp-frm2551Qv2018:txtSheets";
const typed7 = (app: { value(s: string): string | null }) => (app.value(SHEETS) ?? "").includes("7");
/** The page is drawn (its Expand all button): the header shows while the form is still opening. */
const PAGE = "id:fp-parts-toggle";

test("desktop: removing a schedule row on a form shows the row-deleted Undo in the frame with no confirmation, and Undo brings the row back exactly", async () => {
  const app = await open("./FrameDemo.tsx", { ...WIDE, ...asking({ removal: true, start: "form=1601EQ/2025/3" }) });
  await app.waitFor(PAGE, { timeout: 30_000 });
  await app.press(PAGE);
  const row = "id:fp-row-frm1601EQ:txtAtcCd1-0";
  await app.waitFor(row);
  const atc = () => app.texts().some((t) => t.startsWith("WI010"));
  expect(atc()).toBe(true);
  await app.press(row);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-delete");
  await app.press("id:fp-table-frm1601EQ:txtAtcCd1-delete");
  // Delete row removes the row at once (session.ts deleteRow: "no confirmation"): the removal
  // question never shows — the toast offering the exact Undo appears with no Continue step.
  await app.waitFor("id:toast-action");
  expect(app.shows("id:removal-confirm")).toBe(false);
  expect(app.texts()).toContain("Row deleted.");
  expect(app.texts()).toContain("Undo");
  expect(atc()).toBe(false);
  await app.press("id:toast-action");
  await app.waitFor("id:toast-action", { state: "gone" });
  // Undo restores the draft exactly: the row is back, and its drawer holds its own values again.
  await app.waitUntil(atc);
  await app.press(row);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-done");
  expect(app.value("id:fp-frm1601EQ:txtAtcCd1") ?? "").toContain("WI010");
  expect(app.value("id:fp-frm1601EQ:txtTaxBase1")).toBe("250.50");
});

test("desktop: a choice clear that removes typed values still asks; Continue removes them and the frame's Undo restores the row", async () => {
  const app = await open("./FrameDemo.tsx", { ...WIDE, ...asking({ removal: true, start: "form=1601EQ/2025/3" }) });
  await app.waitFor(PAGE, { timeout: 30_000 });
  await app.press(PAGE);
  const row = "id:fp-row-frm1601EQ:txtAtcCd1-0";
  const atcBox = "id:fp-frm1601EQ:txtAtcCd1";
  await app.waitFor(row);
  await app.press(row);
  await app.waitFor(atcBox);
  expect(app.value(atcBox) ?? "").toContain("WI010");
  expect(app.value("id:fp-frm1601EQ:txtTaxBase1")).toBe("250.50");

  await app.press("id:fp-frm1601EQ:txtAtcCd1-trigger");
  await app.waitFor("id:fp-frm1601EQ:txtAtcCd1-opt-", { timeout: 10_000 });
  await app.press("id:fp-frm1601EQ:txtAtcCd1-opt-");
  // Unlike Delete row, this side-effect removal still asks first. While the question is open,
  // the old choice and its typed base are untouched.
  await app.waitFor("id:removal-confirm", { timeout: 10_000 });
  expect(app.texts().some((t) => t.includes("Clearing this ATC removes its row"))).toBe(true);
  expect(app.value(atcBox) ?? "").toContain("WI010");
  expect(app.value("id:fp-frm1601EQ:txtTaxBase1")).toBe("250.50");

  await app.press("id:removal-continue");
  await app.waitFor("id:removal-confirm", { state: "gone" });
  await app.waitFor("id:toast-action", { timeout: 10_000 });
  expect(app.texts()).toContain("Removed the row.");
  expect(app.texts().some((t) => t.startsWith("WI010"))).toBe(false);

  await app.press("id:toast-action");
  await app.waitFor("id:toast-action", { state: "gone" });
  await app.waitFor(row, { timeout: 10_000 });
  // Confirming the choice clear does not close the row editor; close it once, then reopen
  // the restored row and read the draft exactly as the screen shows it.
  await app.press("id:fp-table-frm1601EQ:txtAtcCd1-done");
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-drawer", { state: "gone" });
  await app.press(row);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-done");
  expect(app.value(atcBox) ?? "").toContain("WI010");
  expect(app.value("id:fp-frm1601EQ:txtTaxBase1")).toBe("250.50");
}, 40_000);

test("phone: Delete row removes the row, the later row moves up, and the frame's Undo brings both rows back with their values", async () => {
  const app = await open("./FrameDemo.tsx", { width: 390, height: 844, platform: "ios", ...asking({ removal: true, removal2: true, start: "form=1601EQ/2025/3" }) });
  await app.waitFor("id:fp-pill-open", { timeout: 30_000 });
  // Narrow has no side list: open Part II through the pill's sections list, then the schedule's rows.
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-2", { timeout: 15_000 });
  await app.press("id:fp-section-2");
  const first = "id:fp-row-frm1601EQ:txtAtcCd1-0";
  const second = "id:fp-row-frm1601EQ:txtAtcCd1-1";
  for (let i = 0; i < 20 && !app.shows(first); i++) await app.scroll("id:form-scroll", 500).catch(() => {});
  await app.waitFor(first, { timeout: 10_000 });
  await app.waitFor(second);
  expect(app.texts().some((t) => t.startsWith("WI010"))).toBe(true);
  expect(app.texts().some((t) => t.startsWith("WI011"))).toBe(true);

  await app.press(first);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-delete", { timeout: 10_000 });
  await app.press("id:fp-table-frm1601EQ:txtAtcCd1-delete");
  await app.waitFor("id:toast-action", { timeout: 10_000 });
  expect(app.shows("id:removal-confirm")).toBe(false);
  expect(app.texts()).toContain("Row deleted.");
  expect(app.texts().some((t) => t.startsWith("WI010"))).toBe(false);
  expect(app.count(second)).toBe(0);
  expect(app.count(first)).toBe(1);
  expect(app.texts().some((t) => t.startsWith("WI011"))).toBe(true);

  await app.press("id:toast-action");
  await app.waitFor("id:toast-action", { state: "gone" });
  await app.waitFor(second, { timeout: 10_000 });
  await app.press(first);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-done", { timeout: 10_000 });
  expect(app.texts().some((t) => t.startsWith("WI010 —"))).toBe(true);
  expect(app.value("id:fp-frm1601EQ:txtTaxBase1")).toBe("250.50");
  await app.press("id:fp-table-frm1601EQ:txtAtcCd1-done");
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-drawer", { state: "gone" });
  await app.press(second);
  await app.waitFor("id:fp-table-frm1601EQ:txtAtcCd1-done", { timeout: 10_000 });
  expect(app.texts().some((t) => t.startsWith("WI011 —"))).toBe(true);
  expect(app.value("id:fp-frm1601EQ:txtTaxBase2")).toBe("200.00");
}, 40_000);

test("desktop: what is typed is saved when the person leaves the form within a moment, and the return opens with it", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: `form=2551Q/${YEAR}/3` }) });
  await app.waitFor(PAGE, { timeout: 30_000 });
  await app.type(SHEETS, "7");
  await app.waitUntil(() => typed7(app));
  // Away at once, long before the autosave's own delay (700 ms).
  await app.press("id:nav-library");
  await app.waitFor("id:library");
  // Its Recent row opens the return again.
  const recent = () => app.texts().find((t) => /^2551Q .* · Juana Santos$/.test(t));
  await app.waitUntil(() => recent() !== undefined);
  await app.press(`text:${recent()}`);
  await app.waitFor(PAGE, { timeout: 30_000 });
  await app.waitUntil(() => typed7(app));
});

test("desktop: a return created on the form page is in a Filings list that was already read", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=filings" }) });
  await app.waitFor("id:filings-empty");
  await app.press("id:sb-new");
  await app.press("id:new-return-2551Q");
  await app.press("id:new-return-open");
  await app.waitFor(PAGE, { timeout: 30_000 });
  await app.type(SHEETS, "7");
  await app.waitUntil(() => typed7(app));
  await app.waitFor("text:✓ Saved", { timeout: 10_000 });
  await app.press("id:nav-filings");
  await app.waitFor("id:filings");
  // The draft's card is on the board with no wait for a poll, and the empty note is gone.
  expect(app.shows("id:return-1")).toBe(true);
  expect(app.shows("id:filings-empty")).toBe(false);
});

test("desktop: an amendment's first save makes it a stored return in the history: Back opens the saved return, not a new amendment", async () => {
  const app = await open("./FrameDemo.tsx", { ...WIDE, ...asking({ amend: true, show: "route" }) });
  await app.waitFor("id:fp-save", { timeout: 30_000 });
  expect(app.texts()).toContain("route: form amend");
  await app.press("id:fp-save");
  await app.waitFor("text:route: form return");
  await app.press("id:nav-library");
  await app.waitFor("id:library");
  await app.press("id:tb-back");
  await app.waitFor(PAGE, { timeout: 30_000 });
  expect(app.texts()).toContain("route: form return");
  expect(app.shows("id:fp-save")).toBe(false); // a saved return has no "Save amendment" button
});

test("desktop: the dev start route's scope key reaches the Form library; a value it does not know is ignored", async () => {
  const count = (app: { texts(): string[] }) => app.texts().find((t) => /^\d+ of \d+$/.test(t));
  const all = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=library;scope=all;nothing=here" }) });
  await all.waitFor("id:library-count");
  await all.waitUntil(() => count(all) === "43 of 43");
  const owned = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=library;scope=owned" }) });
  await owned.waitFor("id:library-count");
  await owned.waitUntil(() => count(owned) === "2 of 43");
  // Not a scope the library knows: the old default, the forms the taxpayer files this year.
  const unknown = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=library;scope=sideways" }) });
  await unknown.waitFor("id:library-count");
  await unknown.waitUntil(() => count(unknown) === "2 of 43");
});
