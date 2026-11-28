// The form session through the real door, against an empty scratch database and the demo taxpayer (Juana Santos).
// Tests in this file share one database, in order.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { todoRows, type TodoPart } from "../../lib/todo";
import type { SchemaItem } from "./contract";
import { dateRanges, onePeriods } from "./schema";
import { FormSession, bindError, type FormTarget } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const Q1: FormTarget = { tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" };

async function opened(target: FormTarget = Q1): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("a 2551Q opens: its schema, the profile's locked boxes, a fresh Draft not saved yet", async () => {
  const s = await opened();
  expect(s.fieldCount()).toBe(85); // PR 3: the Year Ended month+year is one period field
  expect(s.status()).toBe("Draft");
  expect(s.header().editable).toBe(true);
  expect(s.valueOf("frm2551Qv2018:txtTIN1")).toBe("000");
  expect(s.box("frm2551Qv2018:txtTIN1").locked()).toBe(true);
  expect(s.box("txtATCAmt1").locked()).toBe(false);
  // A computed amount shows 0.00 and is shut.
  expect(s.box("txtATCDue1").locked()).toBe(true);
  expect(s.saveState().state).toBe("idle");
  s.dispose();
});

test("typing stages; 300 ms later the fill computes the tax and the amount reformats on blur", async () => {
  const s = await opened();
  s.edit("txtATCAmt1", "10000");
  // Typing shows at once and the draft has not taken it yet.
  expect(s.box("txtATCAmt1").value()).toBe("10000");
  expect(s.valueOf("txtATCDue1")).not.toBe("300.00");
  await s.idle();
  expect(s.valueOf("txtATCDue1")).toBe("300.00");
  expect(s.box("txtATCDue1").value()).toBe("300.00");
  expect(s.box("frm2551Qv2018:txt14").value()).toBe("300.00");
  // The box the user typed keeps the typing until they leave it.
  expect(s.box("txtATCAmt1").value()).toBe("10000");
  s.blur("txtATCAmt1");
  expect(s.box("txtATCAmt1").value()).toBe("10,000.00");
  await s.idle();
  s.dispose();
});

test("autosave: a successful fill saves after the quiet; reopening reads the amount back", async () => {
  const s = await opened();
  s.edit("txtATCAmt2", "2000");
  await s.idle();
  expect(s.saveState().state).toBe("saved");
  expect(s.isSaved()).toBe(true);
  expect(s.isDirty()).toBe(false);
  s.dispose();
  const again = await opened();
  expect(again.valueOf("txtATCAmt1")).toBe("10,000.00");
  expect(again.valueOf("txtATCAmt2")).toBe("2,000.00");
  expect(again.saveState().state).toBe("saved");
  again.dispose();
});

test("leaving the page within the quiet still saves what was typed", async () => {
  const s = await opened();
  s.edit("txtATCAmt3", "500");
  s.dispose();
  const again = await opened();
  expect(again.valueOf("txtATCAmt3")).toBe("500.00");
  again.dispose();
});

test("2551Q: a refused calendar Year Ended month stays visible while ATC edits autosave", async () => {
  // Q3: the touched-only test below needs Q4 never saved.
  const target = { ...Q1, period: 3 };
  const s = await opened(target);
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.month.k === "frm2551Qv2018:rtnMonth")!;
  const calendarMonthRefusal = () => s.boxMessageOf(period.month.k);
  s.pickRadio("frm2551Qv2018:forThe_1");
  await s.idle();
  s.edit(period.periodKey, "06/2025");
  await s.idle();
  expect(calendarMonthRefusal()).toContain("December");

  s.edit("txtATCAmt2", "2500");
  await s.idle();
  expect(calendarMonthRefusal()).toContain("December");
  const again = await opened(target);
  const savedAmount = again.valueOf("txtATCAmt2");
  again.dispose();
  s.dispose();
  expect(savedAmount).toBe("2,500.00");
});

test("validation messages bind to their items: by key, then item number, then TIN, then label words", async () => {
  const s = await opened();
  // Item 13 (keys name its radios) stays a row message; the EOPT tier names one box, so its
  // sentence lives in the box home and binds to the row through the box's key (B3/B4).
  expect(s.itemIdForKey("frm2551Qv2018:taxRate1")).toBe("1:7");
  expect(s.messageOf("1:7")).toContain("Item 13");
  const eoptKey = Object.keys(s.boxMessages()).find((k) => s.boxMessageOf(k).includes("EOPT tier"));
  expect(eoptKey).toBeTruthy();
  expect(s.itemIdForKey(eoptKey!)).toBe("2:6");
  const items = s.schema!.parts.flatMap((p, pi) => p.items.map((it: SchemaItem, ii) => ({ id: pi + ":" + ii, it })));
  const none = () => undefined;
  expect(bindError({ field: "x", message: "Item 06 is wrong", keys: [] }, items, none)).toBe("1:0");
  expect(bindError({ field: "x", message: "The TIN is not valid", keys: [] }, items, none)).toBe("1:0");
  expect(bindError({ field: "x", message: "Registered Address is too long", keys: [] }, items, none)).toBe("1:3");
  expect(bindError({ field: "x", message: "zzz", keys: [] }, items, none)).toBe("0:0");
  s.dispose();
});

test("touched-only errors: a message shows after its row was left, all of them after Check", async () => {
  // A return never saved (Q1 is saved by the tests above, and a stored return's opening messages show at once: F9-1).
  const s = await opened({ ...Q1, period: 4 });
  expect(s.messageOf("1:7")).not.toBe("");
  expect(s.row("1:7")().message).toBe("");
  expect(s.row("2:6")().message).toBe("");
  // Counts never wait for the text (F9-1, the old partErrors): Part I's badge says 1 before anything is left.
  expect(s.part(1)().errors).toBe(1);
  expect(s.box("frm2551Qv2018:taxRate1").invalid()).toBe(false);
  s.focus("frm2551Qv2018:taxRate1");
  s.blur("frm2551Qv2018:taxRate1");
  expect(s.row("1:7")().message).toContain("Item 13");
  expect(s.box("frm2551Qv2018:taxRate1").invalid()).toBe(true);
  expect(s.part(1)().errors).toBe(Object.keys(s.messages()).filter((id) => id === "1:7").length);
  expect(s.row("2:6")().message).toBe("");
  s.check();
  await s.idle();
  // The EOPT tier's sentence sits in the box home; after Check it shows on its box.
  const eoptKey = Object.keys(s.boxMessages()).find((k) => s.boxMessageOf(k).includes("EOPT tier"))!;
  expect(s.itemIdForKey(eoptKey)).toBe("2:6");
  expect(s.box(eoptKey).invalid()).toBe(true);
  const eoptShown = s.box(eoptKey).message?.() || s.row("2:6")().boxMessages.map((b) => b.sentence).join(" ");
  expect(eoptShown).toContain("EOPT tier");
  // A part's badge counts its rows with a message, as the old page's partErrors did (F9-1: two messages on one row are one).
  const items = s.schema!.parts.flatMap((p, pi) => p.items.map((it, ii) => ({ id: `${pi}:${ii}`, it })));
  const rows = new Set(s.errors().map((e) => bindError(e, items, (k) => s.itemIdForKey(k))).filter((id) => id?.startsWith("2:")));
  expect(s.part(2)().errors).toBe(rows.size);
  expect(s.part(2)().errors).toBeGreaterThan(0);
  s.dispose();
});

test("the to-do list (lib/todo): required-empty boxes and messages count at once, shown or not", async () => {
  const s = await opened();
  const parts = s.schema!.parts as unknown as TodoPart[];
  const valuesOf = () => Object.fromEntries(s.schema!.parts.flatMap((p) => p.items.flatMap((it) => (it.f || []).map((f) => [f.k, s.valueOf(f.k)]))));
  // Before anything is left or checked, every row with a host message is already to do (F9-1, the old todo());
  // a box-home sentence counts its row the same way (B3/B4).
  const boxRowsOf = () =>
    Object.fromEntries(
      Object.keys(s.boxMessages())
        .map((k) => s.itemIdForKey(k))
        .filter((id): id is string => id !== undefined && !s.messages()[id])
        .map((id) => [id, " "])
    );
  expect(s.todo().map((r) => r.id)).toEqual(todoRows({ parts, messages: { ...s.messages(), ...boxRowsOf() }, values: valuesOf(), blocked: (k) => s.box(k).locked() }).map((r) => r.id));
  expect(s.todo().length).toBeGreaterThan(0);
  // A sample's period boxes are blank and required: they are to do at once, untouched, before any message shows.
  const sample = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 0, period: 0, mode: "sample" });
  const required = sample.todo().filter((r) => r.reason === "required").map((r) => r.id);
  expect(required.length).toBeGreaterThan(0);
  expect(required.every((id) => id.startsWith("0:"))).toBe(true);
  expect(sample.part(0)().todo).toBe(required.length);
  sample.dispose();
  // Check shows every message: they join the list, strongest reason first, in form order.
  s.check();
  await s.idle();
  const ids = s.todo().map((r) => r.id);
  expect(ids).toEqual(todoRows({ parts, messages: { ...s.messages(), ...boxRowsOf() }, values: Object.fromEntries(s.schema!.parts.flatMap((p) => p.items.flatMap((it) => (it.f || []).map((f) => [f.k, s.valueOf(f.k)])))), blocked: (k) => s.box(k).locked() }).map((r) => r.id));
  expect(ids).toContain("1:7");
  expect(ids).toContain("2:6");
  // The pill's step goes to the first, and the next.
  s.step(1);
  expect(s.stepAt()).toBe(ids[0]);
  expect(s.reveal()!.row).toBe(ids[0]);
  s.step(1);
  expect(s.stepAt()).toBe(ids[1]);
  s.dispose();
});

test("a radio group: a pick ticks one, and the row re-renders alone", async () => {
  const s = await opened();
  const before = { tin: s.row("1:0")(), part2: s.row("2:0")() };
  s.pickRadio("frm2551Qv2018:taxRateNA");
  await s.idle();
  expect(s.valueOf("frm2551Qv2018:taxRateNA")).toBe("true");
  expect(s.radioSelected(["frm2551Qv2018:taxRate1", "frm2551Qv2018:taxRate2", "frm2551Qv2018:taxRateNA"])()).toBe("frm2551Qv2018:taxRateNA");
  expect(s.messageOf("1:7")).not.toContain("Choose the applicable Item 13");
  // Rows the pick did not change keep the very same info object (their signals did not notify).
  expect(s.row("1:0")()).toBe(before.tin);
  expect(s.row("2:0")()).toBe(before.part2);
  s.dispose();
});

test("an answer that removes typed rows asks first; Continue removes them and Undo brings them back", async () => {
  createDemoTaxpayer();
  const t: FormTarget = { tin: DEMO.tin, formCode: "1601EQ", year: 2025, period: 3, mode: "return" };
  const s = await opened(t);
  s.choose({ "frm1601EQ:optWithheld:Y": "true", "frm1601EQ:optCategory:P": "true" });
  s.choose({ "frm1601EQ:txtAtcCd1": "WI010" });
  s.edit("frm1601EQ:txtTaxBase1", "250.5");
  await s.idle();
  expect(s.valueOf("frm1601EQ:txtAtcCd1")).toBe("WI010");
  const toasts: { message: string; action?: { label: string; onPress: () => void } }[] = [];
  const asked = new FormSession(t, { toast: (message, o) => toasts.push({ message, action: o?.action }) });
  await asked.open();
  await asked.idle();
  asked.choose({ "frm1601EQ:txtAtcCd1": "" });
  const overlay = asked.overlay();
  expect(overlay?.kind).toBe("removal");
  expect(overlay?.kind === "removal" ? overlay.message : "").toContain("Clearing this ATC removes its row");
  // Held: the old answer still shows.
  expect(asked.valueOf("frm1601EQ:txtAtcCd1")).toBe("WI010");
  asked.confirmRemoval();
  await asked.idle();
  expect(asked.overlay()).toBe(null);
  expect(asked.valueOf("frm1601EQ:txtAtcCd1")).toBe("");
  const undo = toasts.find((x) => x.action?.label === "Undo");
  expect(undo).toBeTruthy();
  undo!.action!.onPress();
  await asked.idle();
  expect(asked.valueOf("frm1601EQ:txtAtcCd1")).toBe("WI010");
  expect(asked.shownOf("frm1601EQ:txtTaxBase1")).toBe("250.50");
  // Cancel keeps the row.
  asked.choose({ "frm1601EQ:txtAtcCd1": "" });
  expect(asked.overlay()?.kind).toBe("removal");
  asked.cancelRemoval();
  expect(asked.valueOf("frm1601EQ:txtAtcCd1")).toBe("WI010");
  await asked.idle();
  s.dispose();
  asked.dispose();
});

test("1601FQ item 4 No uses the official warning; Cancel restores Yes and nothing clears", async () => {
  createDemoTaxpayer();
  const t: FormTarget = { tin: DEMO.tin, formCode: "1601FQ", year: 2025, period: 3, mode: "return" };
  const s = await opened(t);
  s.choose({ "frm1601FQ:TaxWithheld_1": "true", "frm1601FQ:CatAgent_P": "true" });
  s.choose({ "frm1601FQ:txtAtcCode1": "catalog:0" });
  s.edit("frm1601FQ:txtTaxBase1", "1000");
  await s.idle();

  s.choose({ "frm1601FQ:TaxWithheld_2": "true", "frm1601FQ:TaxWithheld_1": "false" });
  const overlay = s.overlay();
  expect(overlay?.kind).toBe("removal");
  expect(overlay?.kind === "removal" ? overlay.message : "").toBe(
    "You are about to change the value to 'No'. Doing this will clear the entries in Part II and Schedule II.",
  );
  expect(s.valueOf("frm1601FQ:TaxWithheld_1")).toBe("true");
  s.cancelRemoval();
  await s.idle();
  expect(s.valueOf("frm1601FQ:TaxWithheld_1")).toBe("true");
  expect(s.valueOf("frm1601FQ:txtAtcCode1")).toBe("WI226");
  expect(s.shownOf("frm1601FQ:txtTaxBase1")).toBe("1,000.00");
  s.dispose();
});

test("a Form library sample is not autosaved until its period is set; Save as return asks for the period", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 0, period: 0, mode: "sample" });
  expect(s.header().sample).toBe(true);
  s.edit("txtATCAmt1", "750");
  await s.idle();
  expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
  expect(s.isSaved()).toBe(false);
  s.saveNow();
  expect(s.overlay()?.kind).toBe("sample-period");
  s.closeSamplePeriod();
  expect(s.overlay()).toBe(null);
  s.dispose();
});

test("Sample: typing 06/2025 in 1601C's filing-period box looks up that period and offers the existing return", async () => {
  createDemoTaxpayer();
  const saved = await bir.screen_form_open(DEMO.tin, "1601C", 2025, 6);
  bir.screen_form_save("1601C", saved.draft);

  const s = await opened({ tin: DEMO.tin, formCode: "1601C", year: 0, period: 0, mode: "sample" });
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.month.k === "frm1601c:txtMonth")!;
  s.edit(period.periodKey, "06/2025");
  await s.idle();
  expect(s.overlay()?.kind).toBe("existing");

  s.saveNow();
  expect(s.overlay()?.kind).not.toBe("sample-period");
  s.dispose();
});

test("1600WP: changing the From date from May 10 to May 15 updates both ends without a box message", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1600WP", year: 2025, period: 5, mode: "return" });
  const fromMonth = "frm1600WP:DateWithholdingMonth";
  const fromDay = "frm1600WP:DateWithholdingDay";
  const toDay = "frm1600WP:DateWithholdingToDay";
  const item = s.schema!.parts.flatMap((part) => part.items).find((it) =>
    dateRanges(it).some((range) => range.from.anchor === fromMonth),
  )!;
  expect(item).toBeDefined();

  s.edit(`${fromMonth}#date`, "05/10/2025");
  await s.idle();
  expect(s.valueOf(fromDay)).toBe("10");
  expect(s.valueOf(toDay)).toBe("10");

  s.edit(`${fromMonth}#date`, "05/15/2025");
  await s.idle();
  expect(s.valueOf(fromDay)).toBe("15");
  expect(s.valueOf(toDay)).toBe("15");
  expect(s.row(s.itemIdForKey(fromMonth)!)().boxMessages).toEqual([]);
  s.dispose();
});

test("Sample: an unfinished 1702EX year holds its period edits; a complete year commits, and blur commits too", async () => {
  createDemoTaxpayer();
  const target = { tin: DEMO.tin, formCode: "1702EX", year: 0, period: 0, mode: "sample" } as FormTarget;

  // Mid-typing "12/20" must not be read as December 2020: no lookup, no rebuilt or saved 2020 return, no autosave.
  const s = await opened(target);
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.year.k === "frm1702EX:txtPg1I2YearEnd")!;
  s.edit(period.periodKey, "12/20");
  await s.idle();
  expect(s.box(period.periodKey).value()).toBe("12/20"); // the box keeps what was typed
  expect(s.valueOf(period.year.k)).toBe("");
  expect(s.overlay()).toBe(null);
  expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
  expect(bir.list_returns("saved").filter((r) => r.form_code === "1702EX" && r.taxable_year === 2020)).toEqual([]);

  // A finished four-digit year commits at once: December 2025 is looked up (the two-digit box then holds "25").
  s.edit(period.periodKey, "12/2025");
  await s.idle();
  expect(s.valueOf(period.year.k)).toBe("25");
  s.saveNow();
  expect(s.overlay()?.kind).not.toBe("sample-period");
  s.dispose();

  // Held edits also land when the box is left: "12/20" commits as December 2020 on blur.
  const blurred = await opened(target);
  blurred.edit(period.periodKey, "12/20");
  blurred.blur(period.periodKey);
  await blurred.idle();
  expect(blurred.valueOf(period.year.k)).toBe("20");
  blurred.saveNow();
  expect(blurred.overlay()?.kind).not.toBe("sample-period");
  blurred.dispose();
});

test("Sample: retyping over a held 1702EX period year drops the hold; blur commits nothing stale", async () => {
  createDemoTaxpayer();
  const target = { tin: DEMO.tin, formCode: "1702EX", year: 0, period: 0, mode: "sample" } as FormTarget;
  const yearKey = "frm1702EX:txtPg1I2YearEnd";
  const fresh = async () => {
    const s = await opened(target);
    const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.year.k === yearKey)!;
    s.edit(period.periodKey, "12/20"); // held, not looked up
    return { s, period };
  };

  // "12/20" held, then backspaced to "12/2" (a period text that refuses): blur must not commit December 2020.
  {
    const { s, period } = await fresh();
    const before = bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length;
    s.edit(period.periodKey, "12/2");
    s.blur(period.periodKey);
    await s.idle();
    expect(s.valueOf(yearKey)).toBe("");
    expect(s.overlay()).toBe(null);
    expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
    s.saveNow();
    await s.idle();
    expect(s.overlay()?.kind).toBe("sample-period");
    expect(bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length).toBe(before);
    s.dispose();
  }

  // Same for an out-of-range month: "13/20" refuses, so the held "12/20" is gone before blur.
  {
    const { s, period } = await fresh();
    const before = bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length;
    s.edit(period.periodKey, "13/20");
    s.blur(period.periodKey);
    await s.idle();
    expect(s.valueOf(yearKey)).toBe("");
    expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
    s.saveNow();
    await s.idle();
    expect(s.overlay()?.kind).toBe("sample-period");
    expect(bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length).toBe(before);
    s.dispose();
  }

  // And a three-digit year is still mid-typing: nothing commits on blur, and a finished year still works after it.
  {
    const { s, period } = await fresh();
    s.edit(period.periodKey, "12/202");
    s.blur(period.periodKey);
    await s.idle();
    expect(s.valueOf(yearKey)).toBe("");
    expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
    s.edit(period.periodKey, "12/2025");
    await s.idle();
    s.saveNow();
    expect(s.overlay()?.kind).not.toBe("sample-period");
    s.dispose();
  }
});

test("Sample: restoring a refused period with no other edit creates no return", async () => {
  const target: FormTarget = { tin: DEMO.tin, formCode: "1601C", year: 0, period: 0, mode: "sample" };
  const s = await opened(target);
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.month.k === "frm1601c:txtMonth")!;
  const saved = () => bir.list_returns("saved").filter((r) => r.form_code === "1601C" && r.taxable_year === 2025 && r.period === 11);
  expect(saved()).toEqual([]);

  s.edit(period.periodKey, "11/2025");
  await s.idle();
  const before = s.saveState();
  s.edit(period.periodKey, "13/2025");
  await s.idle();
  expect(s.boxMessageOf(period.month.k)).toBeTruthy();
  s.edit(period.periodKey, "11/2025");
  await s.idle();
  // The save line is back to what it said before the refusal, not "set its period to save".
  expect(s.saveState()).toEqual(before);
  s.dispose();
  expect(saved()).toEqual([]);
});

test("Sample: correcting 09/202 to 09/2025 autosaves the statutory minimum wage", async () => {
  const target: FormTarget = { tin: DEMO.tin, formCode: "1601C", year: 0, period: 0, mode: "sample" };
  const s = await opened(target);
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.month.k === "frm1601c:txtMonth")!;
  const saved = () => bir.list_returns("saved").filter((r) => r.form_code === "1601C" && r.taxable_year === 2025 && r.period === 9);
  expect(saved()).toEqual([]);

  s.edit(period.periodKey, "09/2025");
  await s.idle();
  s.edit(period.periodKey, "09/202");
  await s.idle();
  s.edit("frm1601c:txtTax15", "1234");
  await s.idle();
  s.edit(period.periodKey, "09/2025");
  await s.idle();
  s.dispose();

  const again = await opened({ ...target, year: 2025, period: 9, mode: "return" });
  expect(again.valueOf("frm1601c:txtTax15")).toBe("1,234.00");
  expect(saved()).toHaveLength(1);
  again.dispose();
});

test("Sample: an incomplete 1601C year cannot save under the previous resolved period", async () => {
  createDemoTaxpayer();
  const target: FormTarget = { tin: DEMO.tin, formCode: "1601C", year: 0, period: 0, mode: "sample" };
  const s = await opened(target);
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.month.k === "frm1601c:txtMonth")!;
  const saved = () => bir.list_returns("saved").filter((r) => r.form_code === "1601C" && r.taxable_year === 2025 && r.period === 8);
  expect(saved()).toEqual([]);

  s.edit(period.periodKey, "08/2025");
  await s.idle();
  expect(saved()).toEqual([]);
  s.edit(period.periodKey, "08/202");
  await s.idle();
  s.saveNow();
  await s.idle();
  expect(s.overlay()?.kind).toBe("sample-period");
  expect(saved()).toEqual([]);

  s.closeSamplePeriod();
  s.edit(period.periodKey, "08/2025");
  await s.idle();
  s.saveNow();
  await s.idle();
  expect(s.overlay()?.kind).not.toBe("sample-period");
  expect(saved()).toHaveLength(1);
  s.dispose();
});

test("Sample: 1600PT and 1600VT take a four-digit year for their period, typed or picked on the month grid", async () => {
  createDemoTaxpayer();
  for (const [code, yearKey] of [
    ["1600PT", "frm1600PT:txtPg1I1Year"],
    ["1600VT", "frm1600VT:txtPg1I1Year"],
  ] as const) {
    const target: FormTarget = { tin: DEMO.tin, formCode: code, year: 0, period: 0, mode: "sample" };
    const saved = await bir.screen_form_open(DEMO.tin, code, 2025, 6);
    bir.screen_form_save(code, saved.draft);

    // Typed: the stored 2025-06 return is offered.
    let s = await opened(target);
    const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.year.k === yearKey)!;
    s.edit(period.periodKey, "06/2025");
    await s.idle();
    expect(s.overlay()?.kind).toBe("existing");
    s.dispose();

    // The month-grid pick offers it too.
    s = await opened(target);
    s.pickPeriodMonth(period, 6, 2025);
    await s.idle();
    expect(s.overlay()?.kind).toBe("existing");
    s.dispose();

    // A period with no stored return prepares a fresh sample: Save goes on without the period dialog.
    s = await opened(target);
    s.edit(period.periodKey, "07/2025");
    await s.idle();
    expect(s.overlay()).toBe(null);
    const stored = () => bir.list_returns("saved").filter((r) => r.form_code === code && r.taxable_year === 2025 && r.period === 7);
    expect(stored()).toEqual([]);
    s.saveNow();
    await s.idle();
    expect(s.overlay()?.kind).not.toBe("sample-period");
    // The save itself went through: a failed write only shows in the save state, so check the state and the stored return.
    expect(s.saveState().state).toBe("saved");
    expect(stored().length).toBe(1);
    s.dispose();
  }
});

test("Sample: a four-digit year below 1900 (12/0025) on 1702EX is not read as 2025", async () => {
  createDemoTaxpayer();
  const s = await opened({ tin: DEMO.tin, formCode: "1702EX", year: 0, period: 0, mode: "sample" });
  const yearKey = "frm1702EX:txtPg1I2YearEnd";
  const period = s.schema!.parts.flatMap((part) => part.items.flatMap(onePeriods)).find((p) => p.year.k === yearKey)!;
  const before = bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length;
  s.edit(period.periodKey, "12/0025");
  await s.idle();
  // The host gets all four digits and refuses year 25: the two-digit box never holds "25" (which would mean 2025).
  expect(s.valueOf(yearKey)).not.toBe("25");
  expect(s.overlay()).toBe(null);
  expect(s.saveState()).toEqual({ state: "manual", message: "Sample · set its period to save" });
  s.saveNow();
  await s.idle();
  expect(s.overlay()?.kind).toBe("sample-period");
  expect(bir.list_returns("saved").filter((r) => r.form_code === "1702EX").length).toBe(before);
  s.dispose();
});

test("the three heaviest forms build a session without error", async () => {
  createDemoTaxpayer();
  const counts: Record<string, number> = {};
  for (const code of ["1702RT", "1700", "1701"]) {
    const s = new FormSession({ tin: DEMO.tin, formCode: code, year: 2025, period: 0, mode: "return" });
    expect(await s.open()).toBe(null);
    await s.idle();
    expect(s.header().error).toBe("");
    counts[code] = s.fieldCount();
    s.dispose();
  }
  expect(counts).toEqual({ "1702RT": 364, "1700": 289, "1701": 379 }); // PR 3: each year-end pair is one period field
});
