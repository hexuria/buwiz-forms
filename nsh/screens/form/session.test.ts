// The form session through the real door, against an empty scratch database and the demo taxpayer (Juana Santos).
// Tests in this file share one database, in order.
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { todoRows, type TodoPart } from "../../lib/todo";
import type { SchemaItem } from "./contract";
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
  expect(s.fieldCount()).toBe(86);
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

test("validation messages bind to their items: by key, then item number, then TIN, then label words", async () => {
  const s = await opened();
  // Item 13 (keys name its radios) and the EOPT tier (keyed at Item 20).
  expect(s.itemIdForKey("frm2551Qv2018:taxRate1")).toBe("1:7");
  expect(s.messageOf("1:7")).toContain("Item 13");
  expect(s.messageOf("2:6")).toContain("EOPT tier");
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
  expect(s.row("2:6")().message).toContain("EOPT tier");
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
  // Before anything is left or checked, every row with a host message is already to do (F9-1, the old todo()).
  expect(s.todo().map((r) => r.id)).toEqual(todoRows({ parts, messages: s.messages(), values: valuesOf(), blocked: (k) => s.box(k).locked() }).map((r) => r.id));
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
  expect(ids).toEqual(todoRows({ parts, messages: s.messages(), values: Object.fromEntries(s.schema!.parts.flatMap((p) => p.items.flatMap((it) => (it.f || []).map((f) => [f.k, s.valueOf(f.k)])))), blocked: (k) => s.box(k).locked() }).map((r) => r.id));
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
  expect(counts).toEqual({ "1702RT": 365, "1700": 289, "1701": 380 });
});
