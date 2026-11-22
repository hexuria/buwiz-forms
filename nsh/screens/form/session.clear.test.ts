// F7-1: emptying a number box (an amount, a percent, a whole number), through the real door (scratch database, the demo taxpayer).
// Select-all and Backspace, Cut: the kit's MoneyInput reports "". The host refuses a blank amount or whole number
// ("must be an amount such as 1234.50 (got ``)") where it holds no blank, and the page used to put the draft's OLD number back
// in the box 250 ms later and call it saved. The old page (app/shell/form_page.js edit() and flush()) staged the "" too, the host
// refused it, syncChanged left the typed (blank) text in the box and the draft kept the old amount: the box and the draft
// disagreed and a save wrote the old number. Now a refused blank is staged as the host's cleared value, zero.
// Tests in this file share one database, in order; each uses its own form or period.
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget, type SessionHooks } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

function hooks(): SessionHooks & { toasts: string[] } {
  const toasts: string[] = [];
  return {
    toasts,
    toast: (message) => {
      toasts.push(message);
      return "t" + toasts.length;
    },
  };
}

async function opened(target: FormTarget, h?: SessionHooks): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("F7-1: 1601C, an emptied amount box holds zero: the draft, the box and the save line agree; then 250 is 250", async () => {
  const h = hooks();
  const K = "frm1601c:txtTax15";
  const s = await opened(at("1601C", 2025, 3), h);
  s.edit(K, "1000");
  await s.idle();
  expect(s.valueOf(K)).toBe("1,000.00");
  expect(s.saveState().state).toBe("saved");
  // Select-all and Backspace: the box is empty at once, and the save line says there is something to save.
  s.edit(K, "");
  expect(s.box(K).value()).toBe("");
  expect(s.saveState().state).toBe("unsaved");
  await s.idle();
  expect(s.valueOf(K)).toBe("0.00");
  expect(s.box(K).value()).toBe("0.00");
  expect(s.messageOf(s.itemIdForKey(K)!)).not.toContain("must be an amount");
  expect(s.saveState().state).toBe("saved");
  expect(h.toasts).toEqual([]);
  // What is stored is zero too: a new page on the same return reads it back.
  s.dispose();
  const again = await opened(at("1601C", 2025, 3));
  expect(again.valueOf(K)).toBe("0.00");
  expect(again.box(K).value()).toBe("0.00");
  // The box took the keys with its zero selected: the filer types 250 over it.
  again.edit(K, "250");
  expect(again.box(K).value()).toBe("250");
  await again.idle();
  expect(again.valueOf(K)).toBe("250.00");
  expect(again.saveState().state).toBe("saved");
  again.dispose();
});

test("F7-1: 2551Q, the same, and what the box computes follows the zero", async () => {
  const K = "txtATCAmt1";
  const s = await opened(at("2551Q", 2025, 2));
  s.edit(K, "10000");
  await s.idle();
  expect(s.valueOf("txtATCDue1")).toBe("300.00");
  s.edit(K, "");
  await s.idle();
  expect(s.valueOf(K)).toBe("0.00");
  expect(s.box(K).value()).toBe("0.00");
  expect(s.valueOf("txtATCDue1")).toBe("0.00");
  expect(s.box("txtATCDue1").value()).toBe("0.00");
  expect(s.saveState().state).toBe("saved");
  s.edit(K, "250");
  await s.idle();
  expect(s.valueOf(K)).toBe("250.00");
  expect(s.box(K).value()).toBe("250");
  s.dispose();
});

test("F7-1: emptied and left at once (Backspace, then Tab before the flush): the zero shows once it is held", async () => {
  const K = "frm2551Qv2018:txt15";
  const s = await opened(at("2551Q", 2024, 2));
  s.edit(K, "75.5");
  s.blur(K);
  await s.idle();
  expect(s.valueOf(K)).toBe("75.50");
  s.edit(K, "");
  s.blur(K);
  await s.idle();
  expect(s.valueOf(K)).toBe("0.00");
  expect(s.box(K).value()).toBe("0.00");
  expect(s.saveState().state).toBe("saved");
  s.dispose();
});

test("F7-1: a whole-number box (1601C sheets attached) holds zero when emptied, then takes the next digit", async () => {
  const K = "frm1601c:txtSheets";
  const s = await opened(at("1601C", 2024, 5));
  s.edit(K, "3");
  await s.idle();
  expect(s.valueOf(K)).toBe("3");
  s.edit(K, "");
  expect(s.box(K).value()).toBe("");
  await s.idle();
  expect(s.valueOf(K)).toBe("0");
  expect(s.box(K).value()).toBe("0");
  expect(s.saveState().state).toBe("saved");
  s.edit(K, "7");
  await s.idle();
  expect(s.valueOf(K)).toBe("7");
  expect(s.box(K).value()).toBe("7");
  s.dispose();
});

test("F7-1: a percent box (1600PT schedule 1's tax rate) holds zero when emptied, then takes the next digits", async () => {
  const K = "frm1600PT:Pg2Sc1TaxRate1";
  const s = await opened(at("1600PT", 2025, 3));
  s.edit(K, "12.5");
  await s.idle();
  expect(s.valueOf(K)).toBe("12.50");
  s.edit(K, "");
  expect(s.box(K).value()).toBe("");
  await s.idle();
  expect(s.valueOf(K)).toBe("0.00");
  expect(s.box(K).value()).toBe("0.00");
  expect(s.saveState().state).toBe("saved");
  s.edit(K, "3");
  await s.idle();
  expect(s.valueOf(K)).toBe("3.00");
  expect(s.box(K).value()).toBe("3");
  s.dispose();
});

test("F7-1: a box the host holds blank stays blank: nothing is staged as zero", async () => {
  // 1701Q's birth year is a whole-number box the host holds blank (it is not a count the form sums).
  const K = "frm1701q:txtBirthYear";
  const s = await opened(at("1701Q", 2025, 2));
  s.edit(K, "1990");
  await s.idle();
  expect(s.valueOf(K)).toBe("1990");
  s.edit(K, "");
  await s.idle();
  expect(s.valueOf(K)).toBe("");
  expect(s.box(K).value()).toBe("");
  s.dispose();
});

test("B19/F7-1: a box the host refuses keeps what was typed, with the reason on the box", async () => {
  const K = "txtATCAmt1";
  const s = await opened(at("2551Q", 2023, 2));
  s.edit(K, "12");
  await s.idle();
  expect(s.valueOf(K)).toBe("12.00");
  s.edit(K, "abc");
  await s.idle();
  expect(s.box(K).value()).toBe("abc");
  expect(s.boxMessageOf(K)).toContain("type an amount");
  s.dispose();
});

test("an agent's fill that dropped a choice: the page drops it from its own draft, and its save does not store it again", async () => {
  const METHOD = "frm1701q:optMethodOfDeduction";
  const target = at("1701Q", 2025, 3);
  const s = await opened(target);
  s.choose({ "frm1701q:optType_1": "true", "frm1701q:optATC_1": "true", [METHOD + ":_2"]: "true" });
  await s.idle();
  s.saveNow();
  await s.idle();
  expect(s.valueOf(METHOD + ":_2")).toBe("true");
  // The agent's `choose`: one edit, and the answer its fill left unanswered.
  s.choose({ "frm1701q:txtSheets": "3" }, [METHOD]);
  await s.idle();
  expect(s.valueOf(METHOD + ":_2")).toBe("false");
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.valueOf("frm1701q:txtSheets")).toBe("3");
  expect(again.valueOf(METHOD + ":_1")).toBe("false");
  expect(again.valueOf(METHOD + ":_2")).toBe("false");
  // A clear alone is saved too.
  again.choose({ [METHOD + ":_2"]: "true" });
  await again.idle();
  again.choose({}, [METHOD]);
  await again.idle();
  again.saveNow();
  await again.idle();
  again.dispose();
  const last = await opened(target);
  expect(last.valueOf(METHOD + ":_2")).toBe("false");
  last.dispose();
});

test("an agent's fill that dropped the ATC and the rate beside a pick of Estate: one fill, and the graduated income stays", async () => {
  const INCOME = "frm1701q:txt36A";
  const target = at("1701Q", 2025, 1);
  const s = await opened(target);
  s.choose({ "frm1701q:optType_2": "true", "frm1701q:optATC_1": "true", "frm1701q:optTaxRate_1": "true", "frm1701q:optMethodOfDeduction:_2": "true" });
  await s.idle();
  s.edit(INCOME, "250000");
  await s.idle();
  s.saveNow();
  await s.idle();
  expect(s.valueOf(INCOME)).toBe("250,000.00");
  // Estate brings the graduated rate back; dropped in a fill of their own, the ATC and the rate would zero the income first.
  s.choose({ "frm1701q:optType_3": "true" }, ["frm1701q:optATC", "frm1701q:optTaxRate"]);
  await s.idle();
  expect(s.valueOf("frm1701q:optType_3")).toBe("true");
  expect(s.valueOf(INCOME)).toBe("250,000.00");
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.valueOf("frm1701q:optType_3")).toBe("true");
  expect(again.valueOf(INCOME)).toBe("250,000.00");
  again.dispose();
});

test("an agent's fill that dropped the ATC and the rate alone asks first: Cancel, or leaving with the question open, keeps the income", async () => {
  const INCOME = "frm1701q:txt36A";
  const DROPPED = ["frm1701q:optATC", "frm1701q:optTaxRate"];
  const target = at("1701Q", 2025, 2);
  const s = await opened(target);
  s.choose({ "frm1701q:optType_2": "true", "frm1701q:optATC_1": "true", "frm1701q:optTaxRate_1": "true", "frm1701q:optMethodOfDeduction:_2": "true" });
  await s.idle();
  s.edit(INCOME, "250000");
  await s.idle();
  s.saveNow();
  await s.idle();
  // The removal goes through the page's normal question, and nothing is stored while it is open.
  s.choose({}, DROPPED);
  expect(s.overlay()?.kind).toBe("removal");
  expect(s.valueOf(INCOME)).toBe("250,000.00");
  s.dispose();
  const left = await opened(target);
  expect(left.valueOf(INCOME)).toBe("250,000.00");
  expect(left.valueOf("frm1701q:optATC_1")).toBe("true");
  left.choose({}, DROPPED);
  expect(left.overlay()?.kind).toBe("removal");
  left.cancelRemoval();
  await left.idle();
  expect(left.valueOf(INCOME)).toBe("250,000.00");
  // Cancel dropped the question with its answers: a later edit does not drop them after all.
  left.choose({ "frm1701q:txtSheets": "2" });
  await left.idle();
  expect(left.overlay()).toBe(null);
  expect(left.valueOf("frm1701q:optATC_1")).toBe("true");
  left.choose({}, DROPPED);
  left.confirmRemoval();
  await left.idle();
  expect(left.valueOf("frm1701q:optATC_1")).not.toBe("true");
  left.dispose();
});
