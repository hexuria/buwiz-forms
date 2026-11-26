// The F3 fixes of the wave 2 review, each through the real door (scratch database, the demo taxpayer Juana Santos).
// Tests in this file share one database, in order; each uses its own form or period.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { radioProps } from "./rows";
import { emptyCell } from "./schema";
import { FormSession, UNDO_WINDOW, type FormTarget, type SessionHooks, type ToastAction } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number, mode: FormTarget["mode"] = "return"): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode });
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

interface Said {
  toasts: { id: string; message: string; action?: ToastAction }[];
  dismissed: string[];
  writes: string[];
}

function hooks(onToast?: (message: string) => void): SessionHooks & { said: Said } {
  const said: Said = { toasts: [], dismissed: [], writes: [] };
  return {
    said,
    toast: (message, o) => {
      const id = "t" + (said.toasts.length + 1);
      said.toasts.push({ id, message, action: o?.action });
      onToast?.(message);
      return id;
    },
    dismissToast: (id) => said.dismissed.push(id),
    afterWrite: (name) => said.writes.push(name),
  };
}

async function opened(target: FormTarget, h?: SessionHooks): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("F3-1: deleting the first row of a compacting schedule (2550Q Schedule 3) shows the row that moved up, as the draft holds it", async () => {
  const s = await opened(at("2550Q", 2025, 2));
  const row = (n: number) => ["txtDateCovered3" + n, "txtDateCovered3To" + n, "txtNameWithHoldingAgent3" + n, "txtIncomePayment3" + n, "txtTotalTaxWithHeld3" + n];
  s.edit("txtDateCovered30", "04/01/2025");
  s.edit("txtDateCovered3To0", "06/30/2025");
  s.edit("txtNameWithHoldingAgent30", "Agent One");
  s.edit("txtIncomePayment30", "1000");
  s.edit("txtDateCovered31", "04/01/2025");
  s.edit("txtDateCovered3To1", "06/30/2025");
  s.edit("txtNameWithHoldingAgent31", "Agent Two");
  s.edit("txtIncomePayment31", "2000");
  await s.idle();
  expect(s.valueOf("txtNameWithHoldingAgent31")).toBe("Agent Two");
  // The drawer's Delete row (rows.tsx onClearRow): "" for every open cell of the row.
  s.choose(Object.fromEntries(row(0).filter((k) => !s.box(k).locked()).map((k) => [k, ""])));
  await s.idle();
  expect(s.valueOf("txtNameWithHoldingAgent30")).toBe("Agent Two");
  expect(s.shownOf("txtNameWithHoldingAgent30")).toBe("Agent Two");
  expect(s.shownOf("txtIncomePayment30")).toBe("2,000.00");
  expect(s.shownOf("txtDateCovered30")).not.toBe("");
  expect(s.valueOf("txtNameWithHoldingAgent31")).toBe("");
  expect(s.shownOf("txtNameWithHoldingAgent31")).toBe("");
  s.dispose();
});

test("F3-2: a removal's Undo is never outlived by a save: autosave waits out the Undo, then saves and retires it; later saves go through", async () => {
  const h = hooks();
  const s = await opened(at("1601EQ", 2025, 2), h);
  s.choose({ "frm1601EQ:optWithheld:Y": "true", "frm1601EQ:optCategory:P": "true" });
  s.choose({ "frm1601EQ:txtAtcCd1": "WI010" });
  s.edit("frm1601EQ:txtTaxBase1", "250.5");
  s.flush();
  s.choose({ "frm1601EQ:txtAtcCd1": "" });
  expect(s.overlay()?.kind).toBe("removal");
  s.confirmRemoval();
  await s.idle();
  const undo = h.said.toasts.find((t) => t.action?.label === "Undo")!;
  expect(undo).toBeTruthy();
  // While Undo is offered nothing is saved under it.
  expect(s.canUndoRemoval()).toBe(true);
  expect(s.isSaved()).toBe(false);
  expect(s.saveState().state).toBe("unsaved");
  await pause(UNDO_WINDOW + 300);
  await s.idle();
  expect(s.isSaved()).toBe(true);
  expect(s.canUndoRemoval()).toBe(false);
  expect(h.said.dismissed).toContain(undo.id);
  // The old toast's Undo does nothing now; the next edit saves over the stored row.
  undo.action!.onPress();
  s.choose({ "frm1601EQ:txtAtcCd1": "WI010" });
  s.edit("frm1601EQ:txtTaxBase1", "99");
  await s.idle();
  expect(s.saveState().state).toBe("saved");
  s.dispose();
});

test("F3-3: 1701MS item 11 draws the taxpayer's and the spouse's radios together: each choice reads its own tick", async () => {
  const s = await opened(at("1701MS", 2025, 0));
  s.edit("frm1701MS:civilStatus", "Married");
  await s.idle();
  s.pickRadio("frm1701MS:txtIfMarriedJointlyNo5");
  await s.idle();
  const taxpayer = "frm1701MS:txtTaxpayerNo11a";
  const spouse = "frm1701MS:txtTaxpayerNo11a1";
  expect(s.box(spouse).locked()).toBe(false);
  s.pickRadio(taxpayer);
  s.pickRadio(spouse);
  await s.idle();
  expect(s.valueOf(taxpayer)).toBe("true");
  expect(s.valueOf(spouse)).toBe("true");
  const id = s.itemIdForKey(taxpayer)!;
  const it = s.item(id)!;
  const group = radioProps(s, id, it, (it.f || []).filter((f) => f.t === "radio"), () => false);
  // One group, two answers: the group's "selected" names only the first, so each choice reads its own.
  expect(group.checked!(taxpayer)).toBe(true);
  expect(group.checked!(spouse)).toBe(true);
  expect(group.checked!("frm1701MS:txtTaxpayerNo11b1")).toBe(false);
  s.dispose();
});

test("F3-4: every write through the door is told to the page (the frame drops its cached reads)", async () => {
  const h = hooks();
  const s = await opened(at("2551Q", 2025, 2), h);
  s.edit("txtATCAmt1", "1000");
  await s.idle();
  expect(h.said.writes).toContain("screen_form_save");
  const row = s.row("1:7")();
  // Juana Santos has no income type for the year, so Item 13's row asks it in a card.
  const ask = row.asks.find((a) => a.control === "choice")!;
  expect(ask.fact).toBe("tax_classification");
  s.pickAsk(ask.fact, ask.choices[0].value);
  s.saveAsk(ask.fact);
  await s.idle();
  expect(h.said.writes).toContain("set_year_facts");
  s.dispose();
});

test("F3-5: a queue refusal shows every message; a radio pick marks its row touched; the last part offers Check", async () => {
  const h = hooks();
  const s = await opened(at("2551Q", 2025, 3), h);
  // The host's messages are to do at once (F9-1), shown or not: Item 13 and Item 20; past the last part, › is Check.
  expect(s.todo().map((r) => r.id)).toEqual(["1:7", "2:6"]);
  expect(s.errors().length).toBeGreaterThan(0);
  s.jumpTo(s.schema!.parts.length - 1);
  expect(s.stepTarget(1)).toEqual({ check: true });
  // A pick in Item 13 marks the row touched: its message shows at once.
  s.pickRadio("frm2551Qv2018:taxRate1");
  await s.idle();
  expect(s.messageOf("1:7")).not.toBe("");
  expect(s.row("1:7")().message).toBe(s.messageOf("1:7"));
  // Queue refused: every message shows.
  expect(s.row("2:6")().message).toBe("");
  s.queue();
  await s.idle();
  if (s.overlay()?.kind === "queue-confirm") {
    s.confirmQueue();
    await s.idle();
  }
  expect(s.status()).toBe("Draft");
  // B3/B4: a one-box message sits on its box now, not in the row's line;
  // the row still counts it and the box carries the sentence.
  expect(s.todo().map((r) => r.id)).toContain("2:6");
  expect(Object.values(s.boxMessages()).some(Boolean)).toBe(true);
  s.dispose();
});

test("F3-7: a Form library sample keeps what is typed while it reopens as its return", async () => {
  let s: FormSession | null = null;
  const h = hooks((message) => {
    if (/now your return/.test(message)) setTimeout(() => s!.edit("txtATCAmt2", "123"), 0);
  });
  s = await opened(at("2551Q", 0, 0, "sample"), h);
  s.edit("txtATCAmt1", "750");
  await s.idle();
  s.saveNow();
  const o = s.overlay();
  expect(o?.kind).toBe("sample-period");
  const parts = o?.kind === "sample-period" ? o.parts : [];
  const answer: Record<string, string> = {};
  for (const p of parts) answer[p.key] = p.part === "year" ? "2024" : p.part === "year_end_month" ? "12" : p.part === "quarter" ? String(p.quarter === 2) : p.value;
  s.takeSamplePeriod(answer);
  await s.idle();
  await s.idle();
  expect(s.target().mode).toBe("return");
  expect(s.target().year).toBe(2024);
  expect(s.header().loading).toBe(false);
  expect(s.valueOf("txtATCAmt1")).toBe("750.00");
  expect(s.valueOf("txtATCAmt2")).toBe("123.00");
  expect(s.shownOf("txtATCAmt2")).toBe("123");
  expect(s.saveState().state).toBe("saved");
  s.dispose();
});

test("B19/F3-8: a rejected edit keeps what was typed, its sentence sits on the box, and it clears on the next edit", async () => {
  const said: string[] = [];
  createDemoTaxpayer();
  const s = new FormSession(at("2551Q", 2025, 1), { toast: (message) => (said.push(message), "t") });
  expect(await s.open()).toBe(null);
  await s.idle();
  const id = s.itemIdForKey("txtATCAmt1")!;
  s.edit("txtATCAmt1", "abc");
  await s.idle();
  // B19: the box keeps what was typed; the draft never took it; the sentence is the box's own.
  expect(s.shownOf("txtATCAmt1")).toBe("abc");
  expect(s.valueOf("txtATCAmt1")).not.toBe("abc");
  expect(s.boxMessageOf("txtATCAmt1")).toContain("type an amount");
  expect(s.row(id)().message).not.toContain("must be an amount");
  // The refused edit is still unsaved work: the save line says so and Queue refuses.
  expect(s.isDirty()).toBe(true);
  s.saveNow();
  await s.idle();
  expect(s.isDirty()).toBe(true);
  expect(s.saveState().state).toBe("unsaved");
  s.queue();
  await s.idle();
  expect(s.status()).not.toBe("Queued");
  expect(said.length).toBeGreaterThan(0);
  s.edit("txtATCAmt1", "100");
  await s.idle();
  expect(s.boxMessageOf("txtATCAmt1")).toBe("");
  expect(s.valueOf("txtATCAmt1")).toBe("100.00");
  s.dispose();
});

test("F3-9: the save line says Unsaved changes at once; flushNow saves now; a refused save on leaving says why", async () => {
  const s = await opened(at("2551Q", 2024, 1));
  s.edit("txtATCAmt1", "100");
  expect(s.saveState().state).toBe("unsaved");
  await s.idle();
  expect(s.saveState().state).toBe("saved");
  s.edit("txtATCAmt1", "200");
  expect(s.saveState().state).toBe("unsaved");
  s.edit("txtATCAmt1", "300");
  expect(await s.flushNow()).toBe(true);
  expect(s.isDirty()).toBe(false);
  expect(s.saveState().state).toBe("saved");
  expect(s.valueOf("txtATCAmt1")).toBe("300.00");
  s.dispose();
  // Two pages on one new period: the second one's save is refused by the host.
  const first = await opened(at("2551Q", 2024, 3));
  const h = hooks();
  const second = await opened(at("2551Q", 2024, 3), h);
  first.edit("txtATCAmt1", "1");
  await first.idle();
  expect(first.isSaved()).toBe(true);
  second.edit("txtATCAmt1", "2");
  second.dispose();
  expect(h.said.toasts.some((t) => /already stored/.test(t.message))).toBe(true);
  first.dispose();
});

test("F3-10: leaving the page retires a removal's Undo and closes its toast", async () => {
  const h = hooks();
  const s = await opened(at("1601EQ", 2025, 3), h);
  s.choose({ "frm1601EQ:optWithheld:Y": "true", "frm1601EQ:optCategory:P": "true" });
  s.choose({ "frm1601EQ:txtAtcCd1": "WI010" });
  s.edit("frm1601EQ:txtTaxBase1", "10");
  s.flush();
  s.choose({ "frm1601EQ:txtAtcCd1": "" });
  expect(s.overlay()?.kind).toBe("removal");
  s.confirmRemoval();
  const undo = h.said.toasts.find((t) => t.action?.label === "Undo")!;
  expect(s.canUndoRemoval()).toBe(true);
  s.dispose();
  expect(s.canUndoRemoval()).toBe(false);
  expect(h.said.dismissed).toContain(undo.id);
});

test("F3-10: one definition of an empty schedule cell, the host's (blank text, a zero amount, an unticked box)", () => {
  expect(emptyCell("", "text")).toBe(true);
  expect(emptyCell("  ", "text")).toBe(true);
  expect(emptyCell("0", "text")).toBe(false);
  expect(emptyCell("0", "money")).toBe(true);
  expect(emptyCell("0.00", "money")).toBe(true);
  expect(emptyCell("-0.00", "money")).toBe(true);
  expect(emptyCell("1,000.00", "money")).toBe(false);
  expect(emptyCell("0", "int")).toBe(true);
  expect(emptyCell("false", "chk")).toBe(true);
  expect(emptyCell("true", "chk")).toBe(false);
  expect(emptyCell("RENT", "sel")).toBe(false);
});


test("B16: a box another answer keeps shut shows its reason, and a write that reaches the host is refused with it", async () => {
  const s = await opened(at("0619E", 2025, 3));
  const k = "frm0619E:txtTax14";
  // Item 4 "Any Taxes Withheld?" is unanswered: the box is off and locked —
  // and while it is blank the page does not draw it at all (G1); its own
  // tag still says what turns it on.
  expect(s.box(k).off()).toBe("Needs item 4 = Yes");
  expect(s.box(k).locked()).toBe(true);
  // A write that still reaches the host (a stale page, an agent) is
  // refused, and the box says the same thing in the filer's words.
  s.edit(k, "100");
  await s.idle();
  expect(s.boxMessageOf(k)).toContain("used when Item 4 is Yes");
  s.dispose();
});

test("B17: a code off a select's list is refused on the box without a host call", async () => {
  const s = await opened(at("1701Q", 2025, 1));
  const rdo = "frm1701q:txtRDOCode";
  // The RDO box comes from the tax profile; Edit opens it for this return.
  s.startProfileEdit(s.itemIdForKey(rdo)!);
  const before = s.valueOf(rdo);
  s.choose({ [rdo]: "ZZ" });
  // Refused at once, on the box, with the stored value untouched and
  // nothing staged: no host call was needed to say so.
  expect(s.boxMessageOf(rdo)).toBe("Item 6: pick one of the listed choices.");
  expect(s.valueOf(rdo)).toBe(before);
  expect(s.isDirty()).toBe(false);
  s.dispose();
  const q = await opened(at("1702Q", 2025, 1));
  const atc = "frm1702q:cbATC_2";
  const beforeAtc = q.valueOf(atc);
  q.choose({ [atc]: "ZZ" });
  expect(q.boxMessageOf(atc)).toBe("Item 5: pick one of the listed choices.");
  expect(q.valueOf(atc)).toBe(beforeAtc);
  expect(q.isDirty()).toBe(false);
  q.dispose();
});

test("B14: a boxed birth date shows the long month and asks once to save to the profile", async () => {
  const s = await opened(at("1701", 2025, 0));
  // Q1 P1b: birth_date has a writable box on 1701, so it is typed in the box (no card). Leaving it with a
  // new complete value asks "Save to profile?" once; the box still shows the long month.
  const box = "frm1701:txtPg1I10BirthDate";
  s.edit(box, "08/17/1988");
  s.blur(box);
  await s.idle();
  expect(s.shownOf(box)).toBe("August 17, 1988");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("money safety net: a host-stored amount different from the shown amount is named and never reported saved", async () => {
  const key = "frm2551Qv2018:txt15";
  const original = bir.screen_form_fill;
  const mutableBir = bir as { screen_form_fill: typeof original };
  mutableBir.screen_form_fill = ((...args: Parameters<typeof original>): ReturnType<typeof original> => {
    const r = original(...args);
    if (args[2][key] === "100") {
      // The host fault the net exists for: the fill answers as though it stored 99.00 for a shown 100.00.
      (r.draft as { creditable_tax_withheld?: number }).creditable_tax_withheld = 99;
      r.values[key] = "99.00";
    }
    return r;
  }) as typeof original;
  let reported = 0;
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2025, 4), { onSaved: () => reported++ });
    s.edit(key, "100");
    await s.idle();
    s.saveNow();
    expect(s.boxMessageOf(key)).toBe("Item 15: the box shows 100.00, but the draft stored 99.00. Correct this amount; it was not saved.");
    expect(reported).toBe(0);
    expect(s.saveState().state).toBe("unsaved");
  } finally {
    mutableBir.screen_form_fill = original;
    s?.dispose();
  }
});
