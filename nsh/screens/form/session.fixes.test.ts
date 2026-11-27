// The F3 fixes of the wave 2 review, each through the real door (scratch database, the demo taxpayer Juana Santos).
// Tests in this file share one database, in order; each uses its own form or period.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { radioProps } from "./rows";
import { Print } from "../print/native";
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

test("money note: a gate zeroing a batched amount raises no note", async () => {
  const key = "frm0619E:txtTax14";
  const s = await opened(at("0619E", 2025, 2));
  s.pickRadio("frm0619E:optWithheld:Y");
  await s.idle();
  s.edit(key, "1250.5");
  s.pickRadio("frm0619E:optWithheld:N"); // commits at once, with the staged amount in the same host batch
  await s.idle();
  expect(s.boxMessageOf(key) ?? "").not.toContain("you typed");
  s.dispose();
});

test("money note: clearing a filled or already-zero amount raises no note (0.00 against blank says nothing)", async () => {
  const key = "frm2551Qv2018:txt15";
  const filled = await opened(at("2551Q", 2024, 1));
  filled.edit(key, "100");
  await filled.idle();
  filled.edit(key, "");
  await filled.idle();
  expect(filled.boxMessageOf(key) ?? "").not.toContain("you typed");
  expect(filled.shownOf(key)).toBe("0.00");
  filled.dispose();

  const zero = await opened(at("2551Q", 2024, 2));
  zero.edit(key, "");
  await zero.idle();
  expect(zero.boxMessageOf(key) ?? "").not.toContain("you typed");
  expect(zero.shownOf(key)).toBe("0.00");
  zero.dispose();
});

test("money note: an amount the host stored as another number shows as stored, with a note naming both; nothing is held and the save is reported", async () => {
  const key = "frm2551Qv2018:txt15";
  const original = bir.screen_form_fill;
  const mutableBir = bir as { screen_form_fill: typeof original };
  mutableBir.screen_form_fill = ((...args: Parameters<typeof original>): ReturnType<typeof original> => {
    const r = original(...args);
    if (args[2][key] === "100") {
      // The case the note is for: the host stores 99.00 for a typed 100 (a long number rounded, a clamp).
      (r.draft as { creditable_tax_withheld?: number }).creditable_tax_withheld = 99;
      r.values[key] = "99.00";
    }
    return r;
  }) as typeof original;
  const note = "Item 15: you typed 100.00; the return holds 99.00.";
  const saves = countSaves();
  let reported = 0;
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2025, 4), { onSaved: () => reported++ });
    s.edit(key, "100");
    s.blur(key);
    await s.idle();
    expect(s.shownOf(key)).toBe("99.00");
    expect(s.boxMessageOf(key)).toBe(note);
    // Autosave went through; an explicit save too, and the return is saved, not unsaved.
    expect(saves.count).toBeGreaterThan(0);
    s.saveNow();
    await s.idle();
    expect(reported).toBeGreaterThan(0);
    expect(s.saveState().state).toBe("saved");
    expect(await s.flushNow()).toBe(true);
    expect(s.boxMessageOf(key)).toBe(note);
    // Back in the box and out again with no edit, the note stays (focus never ends a note).
    s.focus(key);
    expect(s.boxMessageOf(key)).toBe(note);
    s.blur(key);
    await s.idle();
    expect(s.boxMessageOf(key)).toBe(note);
    // An edit takes it away; one the host stores otherwise raises it again; one stored as typed raises none.
    s.edit(key, "100");
    s.blur(key);
    await s.idle();
    expect(s.boxMessageOf(key)).toBe(note);
    s.edit(key, "250");
    expect(s.boxMessageOf(key) ?? "").toBe("");
    await s.idle();
    expect(s.boxMessageOf(key) ?? "").toBe("");
  } finally {
    mutableBir.screen_form_fill = original;
    saves.restore();
    s?.dispose();
  }
});

// The money notes, through the door's seams: `hostFault` is the host fault the net exists for
// (wherever the draft holds `typed` for `key`, the host answers `stored`, in a fill's and a validation's values alike), and `countSaves`
// counts the host's saves.

function hostFault(key: string, typed: string, stored: string): () => void {
  const fill = bir.screen_form_fill;
  const validate = bir.screen_form_validate;
  const door = bir as { screen_form_fill: typeof fill; screen_form_validate: typeof validate };
  const fault = (values: Record<string, unknown> | undefined) => {
    const v = String(values?.[key] ?? "").replace(/,/g, "");
    if (values && v !== "" && Number(v) === Number(typed)) values[key] = stored;
  };
  door.screen_form_fill = ((...args: Parameters<typeof fill>): ReturnType<typeof fill> => {
    const r = fill(...args);
    fault(r.values as Record<string, unknown>);
    return r;
  }) as typeof fill;
  door.screen_form_validate = ((...args: Parameters<typeof validate>): ReturnType<typeof validate> =>
    validate(...args).then((r) => {
      fault(r.values as Record<string, unknown> | undefined);
      return r;
    })) as typeof validate;
  return () => {
    door.screen_form_fill = fill;
    door.screen_form_validate = validate;
  };
}

function countSaves(): { count: number; restore: () => void } {
  const save = bir.screen_form_save;
  const door = bir as { screen_form_save: typeof save };
  const out = { count: 0, restore: () => void (door.screen_form_save = save) };
  door.screen_form_save = ((...args: Parameters<typeof save>): ReturnType<typeof save> => {
    out.count++;
    return save(...args);
  }) as typeof save;
  return out;
}

/** 2550Q Schedule 3's row n (a compacting schedule: a deleted row's rows below move up). */
const sched3 = (n: number) => ["txtDateCovered3" + n, "txtDateCovered3To" + n, "txtNameWithHoldingAgent3" + n, "txtIncomePayment3" + n, "txtTotalTaxWithHeld3" + n];

function fillSched3Row(s: FormSession, n: number, agent: string, payment: string) {
  s.edit("txtDateCovered3" + n, "01/01/2025");
  s.edit("txtDateCovered3To" + n, "03/31/2025");
  s.edit("txtNameWithHoldingAgent3" + n, agent);
  s.edit("txtIncomePayment3" + n, payment);
}

/** The drawer's Delete row (rows.tsx onClearRow): "" for every open cell of the row. */
const deleteRow = (s: FormSession, n: number) => s.choose(Object.fromEntries(sched3(n).filter((k) => !s.box(k).locked()).map((k) => [k, ""])));

test("F10: an amount typed in a row that a Delete row in the same batch clears or moves raises no note; a real one in that batch still does", async () => {
  const other = "frm2550qv2024:vatRefund";
  const restore = hostFault(other, "100", "99.00");
  const mismatch = (s: FormSession, k: string) => (s.boxMessageOf(k) ?? "").includes("you typed");
  const cells = [0, 1, 2].flatMap((n) => sched3(n));
  let s: FormSession | undefined;
  try {
    s = await opened(at("2550Q", 2025, 3));
    fillSched3Row(s, 0, "Agent One", "1000");
    fillSched3Row(s, 1, "Agent Two", "2000");
    await s.idle();
    // Typed, still pending, then its own row deleted: one batch, with a real mismatch in another box.
    s.edit("txtIncomePayment31", "2500");
    s.edit(other, "100");
    deleteRow(s, 1);
    await s.idle();
    expect(cells.filter((k) => mismatch(s!, k))).toEqual([]);
    expect(mismatch(s, other)).toBe(true);
    // Typed in row 1, still pending, then row 0 deleted (row 1's rows move up): one batch, with a real mismatch again.
    fillSched3Row(s, 1, "Agent Two", "2000");
    await s.idle();
    s.edit("txtIncomePayment31", "2600");
    s.edit(other, "100");
    deleteRow(s, 0);
    await s.idle();
    expect(cells.filter((k) => mismatch(s!, k))).toEqual([]);
    expect(mismatch(s, other)).toBe(true);
  } finally {
    restore();
    s?.dispose();
  }
});

test("money note: a note on a schedule amount goes when deleting the row above moves its row up; the box then shows its row's amount", async () => {
  const pay1 = "txtIncomePayment31";
  const restore = hostFault(pay1, "2000", "1,999.00");
  let s: FormSession | undefined;
  try {
    s = await opened(at("2550Q", 2025, 1));
    fillSched3Row(s, 0, "Agent One", "1000");
    fillSched3Row(s, 1, "Agent Two", "2000");
    await s.idle();
    expect(s.shownOf(pay1)).toBe("1,999.00");
    expect(s.boxMessageOf(pay1)).toBe("(C) Income Payment: you typed 2,000.00; the return holds 1,999.00.");
    deleteRow(s, 0);
    await s.idle();
    expect(s.shownOf("txtIncomePayment30")).toBe("2,000.00");
    expect(s.boxMessageOf(pay1) ?? "").toBe("");
    expect(s.shownOf(pay1)).toBe("");
  } finally {
    restore();
    s?.dispose();
  }
});

test("pending edits before row moves: an amount typed in a row is filled on its own row before a Delete row in the same turn moves the rows", async () => {
  const row = (s: FormSession, n: number) => [s.valueOf("txtNameWithHoldingAgent3" + n), s.valueOf("txtIncomePayment3" + n)];
  const errors = (s: FormSession) => [0, 1, 2].flatMap((n) => sched3(n)).filter((k) => Boolean(s.boxMessageOf(k)));
  const three = async (period: number) => {
    const s = await opened(at("2550Q", 2024, period));
    fillSched3Row(s, 0, "A0", "1000");
    fillSched3Row(s, 1, "A1", "2000");
    fillSched3Row(s, 2, "A2", "3000");
    await s.idle();
    return s;
  };
  // Typed in row 1, row 0 deleted: the typed row (now first) holds 7,777; A2's 3,000 is not overwritten.
  let s = await three(1);
  s.edit("txtIncomePayment31", "7777");
  deleteRow(s, 0);
  await s.idle();
  expect([row(s, 0), row(s, 1), row(s, 2)]).toEqual([["A1", "7,777.00"], ["A2", "3,000.00"], ["", ""]]);
  expect(errors(s)).toEqual([]);
  s.dispose();
  // Typed in the row being deleted: the amount goes with the row, no error.
  s = await three(2);
  s.edit("txtIncomePayment31", "7777");
  deleteRow(s, 1);
  await s.idle();
  expect([row(s, 0), row(s, 1), row(s, 2)]).toEqual([["A0", "1,000.00"], ["A2", "3,000.00"], ["", ""]]);
  expect(errors(s)).toEqual([]);
  s.dispose();
  // Typed in a row above the deleted one: it stays as typed.
  s = await three(3);
  s.edit("txtIncomePayment30", "7777");
  deleteRow(s, 1);
  await s.idle();
  expect([row(s, 0), row(s, 1), row(s, 2)]).toEqual([["A0", "7,777.00"], ["A2", "3,000.00"], ["", ""]]);
  expect(errors(s)).toEqual([]);
  s.dispose();
});

test("one leave toast: a save refused on the way out is said once, by flushNow, not again when the page goes", async () => {
  const first = await opened(at("2551Q", 2024, 4));
  const h = hooks();
  const second = await opened(at("2551Q", 2024, 4), h);
  first.edit("txtATCAmt1", "1");
  await first.idle();
  expect(first.isSaved()).toBe(true);
  second.edit("txtATCAmt1", "2");
  expect(await second.flushNow()).toBe(false);
  second.dispose();
  expect(h.said.toasts.filter((t) => /already stored/.test(t.message)).length).toBe(1);
  first.dispose();
});

/** A sample's Save, its period answered (year, quarter) when the dialog asks. */
async function saveSampleAs(session: FormSession, year: number, quarter: number) {
  session.saveNow();
  const o = session.overlay();
  if (o?.kind === "sample-period") {
    const answer: Record<string, string> = {};
    for (const p of o.parts) answer[p.key] = p.part === "year" ? String(year) : p.part === "year_end_month" ? "12" : p.part === "quarter" ? String(p.quarter === quarter) : p.value;
    session.takeSamplePeriod(answer);
  }
  await session.idle();
  await session.idle();
}

/** Counts the host's queue checks: Queue reached them only when nothing on the page stopped it first. */
function countQueueChecks(): { count: number; restore: () => void } {
  const check = bir.screen_form_queue_check;
  const door = bir as { screen_form_queue_check: typeof check };
  const out = { count: 0, restore: () => void (door.screen_form_queue_check = check) };
  door.screen_form_queue_check = ((...args: Parameters<typeof check>): ReturnType<typeof check> => {
    out.count++;
    return check(...args);
  }) as typeof check;
  return out;
}

test("queue settles the box: with an amount box focused, Queue leaves it first (its blur on a later turn), then stops on the note it raised", async () => {
  const key = "frm2551Qv2018:txt15";
  const restore = hostFault(key, "100", "99.00");
  const checks = countQueueChecks();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2021, 3), h);
    s.jumpTo(2);
    // The box's blur reaches the session on a later turn, as a real input's may.
    let blurred = 0;
    s.registerBlur(key, () => void setTimeout(() => (blurred++, s!.blur(key)), 60));
    s.focus(key);
    s.edit(key, "100");
    s.queue();
    expect(s.boxMessageOf(key) ?? "").toBe("");
    await s.idle();
    expect(blurred).toBe(1);
    expect(s.shownOf(key)).toBe("99.00");
    expect(s.boxMessageOf(key)).toBe("Item 15: you typed 100.00; the return holds 99.00.");
    expect(checks.count).toBe(0);
    expect(s.overlay()?.kind ?? "").not.toBe("queue-confirm");
    // Queue showed it (Part II open, its row in view, the toast says it): a second press goes on to the host's check, the note still
    // under its box.
    expect(s.part(2)().open).toBe(true);
    expect(s.stepAt()).toBe(s.itemIdForKey(key));
    expect(h.said.toasts.map((t) => t.message)).toContain("Item 15: you typed 100.00; the return holds 99.00.");
    s.queue();
    await s.idle();
    expect(checks.count).toBeGreaterThan(0);
    expect(s.boxMessageOf(key)).toBe("Item 15: you typed 100.00; the return holds 99.00.");
  } finally {
    restore();
    checks.restore();
    s?.dispose();
  }
});

test("Q2: a box that never reports its blur stops Queue: nothing is checked or queued, the box is left as it is, and the filer is told once", async () => {
  const key = "frm2551Qv2018:txt15";
  const told = "Finish the amount in Item 15 first.";
  const checks = countQueueChecks();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2021, 4), h);
    s.jumpTo(2);
    // A box that is there (its blur registered) but never reports that it let go.
    s.registerBlur(key, () => {});
    s.focus(key);
    s.edit(key, "100");
    s.queue();
    await s.idle();
    expect(checks.count).toBe(0);
    expect(h.said.toasts.filter((t) => t.message === told).length).toBe(1);
    // The session did not take the keys out of the box itself: it still has them as far as the session knows.
    expect(s.focusedPart()).toBe(2);
    expect(s.overlay()?.kind ?? "").not.toBe("queue-confirm");
    // The box really was left: a later Queue goes on to the host's check.
    s.blur(key);
    await s.idle();
    s.queue();
    await s.idle();
    expect(checks.count).toBeGreaterThan(0);
    expect(h.said.toasts.filter((t) => t.message === told).length).toBe(1);
  } finally {
    checks.restore();
    s?.dispose();
  }
});

test("unreadable note: the box's report of text that is not an amount shows under it, stops Queue once (a folded part's too, opened), and stops nothing else", async () => {
  const key = "frm2551Qv2018:txt15";
  const sentence = "Item 15: 1500x is not an amount. The box kept 1,000.00.";
  const checks = countQueueChecks();
  const saves = countSaves();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2020, 3), h);
    s.jumpTo(2);
    s.edit(key, "1000");
    await s.idle();
    // The box put 1,000.00 back and says what it was left with.
    s.leftUnreadable(key, "1500x", "1,000.00");
    expect(s.boxMessageOf(key)).toBe(sentence);
    expect(s.isDirty()).toBe(false);
    // Save and leaving go through.
    const before = saves.count;
    s.edit("frm2551Qv2018:txt17", "5");
    expect(await s.flushNow()).toBe(true);
    expect(saves.count).toBeGreaterThan(before);
    expect(s.boxMessageOf(key)).toBe(sentence);
    // Its part folded: Queue still stops on it, opens the part, brings the row into view and says the sentence.
    s.togglePart(2);
    expect(s.part(2)().open).toBe(false);
    s.queue();
    await s.idle();
    expect(checks.count).toBe(0);
    expect(s.part(2)().open).toBe(true);
    expect(s.stepAt()).toBe(s.itemIdForKey(key));
    expect(h.said.toasts.map((t) => t.message)).toContain(sentence);
    // Shown once: the next press goes on; the note stays under its box, through a remount and a focus.
    s.queue();
    await s.idle();
    expect(checks.count).toBeGreaterThan(0);
    s.registerFocus(key, () => {});
    s.focus(key);
    expect(s.boxMessageOf(key)).toBe(sentence);
    // An edit of the box ends it.
    s.edit(key, "1000");
    expect(s.boxMessageOf(key) ?? "").toBe("");
  } finally {
    checks.restore();
    saves.restore();
    s?.dispose();
  }
});

test("unreadable note: a Delete row of its row takes it away; a sample with one saves as a return", async () => {
  const pay1 = "txtIncomePayment31";
  let s: FormSession | undefined;
  let sample: FormSession | undefined;
  const saves = countSaves();
  try {
    s = await opened(at("2550Q", 2023, 1));
    fillSched3Row(s, 0, "Agent One", "1000");
    fillSched3Row(s, 1, "Agent Two", "2000");
    await s.idle();
    s.leftUnreadable(pay1, "2000,5", "2,000.00");
    expect(s.boxMessageOf(pay1)).toContain("is not an amount");
    deleteRow(s, 1);
    await s.idle();
    expect(s.boxMessageOf(pay1) ?? "").toBe("");

    sample = await opened(at("2551Q", 0, 0, "sample"), hooks());
    sample.edit("frm2551Qv2018:txt15", "15");
    await sample.idle();
    sample.leftUnreadable("frm2551Qv2018:txt15", "15..0", "15.00");
    const before = saves.count;
    await saveSampleAs(sample, 2020, 4);
    expect(saves.count).toBeGreaterThan(before);
    expect(sample.target().mode).toBe("return");
  } finally {
    saves.restore();
    s?.dispose();
    sample?.dispose();
  }
});

test("money note: leaving with a note and other valid edits saves them all; a second session reads them from the database", async () => {
  const key = "frm2551Qv2018:txt15";
  const restore = hostFault(key, "100", "99.00");
  let s: FormSession | undefined;
  let back: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2021, 2));
    s.edit(key, "100");
    s.blur(key);
    s.edit("frm2551Qv2018:txt17", "250");
    s.edit("txtATCAmt1", "300");
    expect(await s.flushNow()).toBe(true);
    s.dispose();
    back = new FormSession(at("2551Q", 2021, 2));
    expect(await back.open()).toBe(null);
    await back.idle();
    expect([back.valueOf("frm2551Qv2018:txt17"), back.valueOf("txtATCAmt1")]).toEqual(["250.00", "300.00"]);
  } finally {
    restore();
    s?.dispose();
    back?.dispose();
  }
});

test("Q1: Queue pressed twice while it settles a focused amount box runs the host's queue check at most once, after the box was left", async () => {
  const key = "frm2551Qv2018:txt17";
  const checks = countQueueChecks();
  let leftAt = -1;
  const check = bir.screen_form_queue_check;
  const door = bir as { screen_form_queue_check: typeof check };
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2020, 1));
    s.jumpTo(2);
    let checkedAfterLeaving = true;
    const counted = door.screen_form_queue_check;
    door.screen_form_queue_check = ((...args: Parameters<typeof check>): ReturnType<typeof check> => {
      if (leftAt < 0) checkedAfterLeaving = false;
      return counted(...args);
    }) as typeof check;
    s.registerBlur(key, () => void setTimeout(() => ((leftAt = Date.now()), s!.blur(key)), 60));
    s.focus(key);
    s.edit(key, "5");
    s.queue();
    s.queue();
    await s.idle();
    expect(checks.count).toBeLessThanOrEqual(1);
    expect(checkedAfterLeaving).toBe(true);
    expect(leftAt).toBeGreaterThan(0);
  } finally {
    door.screen_form_queue_check = check;
    checks.restore();
    s?.dispose();
  }
});

test("V8: an amount still pending when a removal's Undo is pressed is stored on its own row, and Undo then says it is no longer available", async () => {
  const K = (n: string) => "frm1601EQ:" + n;
  const h = hooks();
  const s = await opened(at("1601EQ", 2024, 2), h);
  s.choose({ [K("optWithheld:Y")]: "true", [K("optCategory:P")]: "true" });
  s.choose({ [K("txtAtcCd1")]: "WI010" });
  s.edit(K("txtTaxBase1"), "100");
  s.flush();
  s.choose({ [K("txtAtcCd2")]: "WI011" });
  s.edit(K("txtTaxBase2"), "200");
  s.flush();
  await s.idle();
  // Row 1 removed: WI011's row moves up into row 1, and Undo is offered.
  s.choose({ [K("txtAtcCd1")]: "" });
  s.confirmRemoval();
  await s.idle();
  expect([s.valueOf(K("txtAtcCd1")), s.valueOf(K("txtTaxBase1"))]).toEqual(["WI011", "200.00"]);
  // 888 typed for WI011's row, still pending, then Undo.
  s.edit(K("txtTaxBase1"), "888");
  s.undoLastRemoval();
  await s.idle();
  const rows = [1, 2].map((n) => [s.valueOf(K("txtAtcCd" + n)), s.valueOf(K("txtTaxBase" + n))]);
  expect(rows).toEqual([["WI011", "888.00"], ["", ""]]);
  expect(h.said.toasts.map((t) => t.message)).toContain("Undo is no longer available: you typed since.");
  s.dispose();
});

test("V8: an amount pending in a row when a choice removes that row or a row above: Continue puts it on no other row and raises no note", async () => {
  const s = await opened(at("1601FQ", 2024, 2));
  s.choose({ "frm1601FQ:SpecialTax_1": "true", "frm1601FQ:SpecialTax_2": "false" });
  await s.idle();
  s.choose({ drpATCCode0: s.box("drpATCCode0").options().map((o) => o.value).find((v) => v) ?? "" });
  s.edit("txtAmtIncomePay0", "1000");
  s.flush();
  await s.idle();
  expect(s.valueOf("txtAmtIncomePay0")).toBe("1,000.00");
  // 5555 typed, still pending, then No: Schedule 1's rows go (the page asks first).
  s.edit("txtAmtIncomePay0", "5555");
  s.choose({ "frm1601FQ:SpecialTax_2": "true", "frm1601FQ:SpecialTax_1": "false" });
  expect(s.overlay()?.kind).toBe("removal");
  s.confirmRemoval();
  await s.idle();
  expect([s.valueOf("txtAmtIncomePay0"), s.valueOf("txtAmtIncomePay1")].map((v) => v.replace(/^0\.00$/, ""))).toEqual(["", ""]);
  expect(s.boxMessageOf("txtAmtIncomePay0") ?? "").toBe("");
  s.dispose();

  // A row above removed (1601EQ: row 1's ATC cleared), 777 pending for row 2: it stays with WI011's row as that row moves up.
  const K = (n: string) => "frm1601EQ:" + n;
  const e = await opened(at("1601EQ", 2024, 3));
  e.choose({ [K("optWithheld:Y")]: "true", [K("optCategory:P")]: "true" });
  e.choose({ [K("txtAtcCd1")]: "WI010" });
  e.edit(K("txtTaxBase1"), "100");
  e.flush();
  e.choose({ [K("txtAtcCd2")]: "WI011" });
  e.edit(K("txtTaxBase2"), "200");
  e.flush();
  await e.idle();
  e.edit(K("txtTaxBase2"), "777");
  e.choose({ [K("txtAtcCd1")]: "" });
  e.confirmRemoval();
  await e.idle();
  expect([1, 2].map((n) => [e.valueOf(K("txtAtcCd" + n)), e.valueOf(K("txtTaxBase" + n))])).toEqual([["WI011", "777.00"], ["", ""]]);
  expect(e.boxMessageOf(K("txtTaxBase1")) ?? "").toBe("");
  e.dispose();
});

test("note rules: a box that goes away unregisters its blur; Queue then treats it as left, neither waiting nor asking to finish the amount", async () => {
  const key = "frm2551Qv2018:txt15";
  const checks = countQueueChecks();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2020, 2), h);
    s.jumpTo(2);
    let called = 0;
    s.registerBlur(key, () => void called++);
    s.focus(key);
    s.edit(key, "40");
    // The row is recycled while the box has the keys: its cleanup unregisters.
    s.registerBlur(key, null);
    const started = Date.now();
    s.queue();
    await s.idle();
    expect(called).toBe(0);
    expect(Date.now() - started).toBeLessThan(450);
    expect(h.said.toasts.some((t) => t.message.startsWith("Finish the amount"))).toBe(false);
    expect(checks.count).toBeGreaterThan(0);
    expect(s.valueOf(key)).toBe("40.00");
  } finally {
    checks.restore();
    s?.dispose();
  }
});

test("note rules: a note raised by a box leaving as its part folds is kept, and Queue stops on it and opens that part", async () => {
  const key = "frm2551Qv2018:txt15";
  const sentence = "Item 15: 1500,50 is not an amount. The box kept 100.00. Use a point for centavos, and commas only between thousands.";
  const checks = countQueueChecks();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2551Q", 2019, 3), h);
    s.jumpTo(2);
    s.edit(key, "100");
    s.blur(key);
    await s.idle();
    s.focus(key);
    // The filer presses another part in the navigator: Part II folds while the box, unmounting, reports what it was left with.
    s.jumpTo(1);
    s.edit(key, "100");
    s.leftUnreadable(key, "1500,50", "100.00");
    s.blur(key);
    await s.idle();
    expect(s.part(2)().open).toBe(false);
    expect(s.boxMessageOf(key)).toBe(sentence);
    s.queue();
    await s.idle();
    expect(checks.count).toBe(0);
    expect(s.part(2)().open).toBe(true);
    expect(h.said.toasts.map((t) => t.message)).toContain(sentence);
  } finally {
    checks.restore();
    s?.dispose();
  }
});

test("note in a row drawer: Queue asks the schedule to open the cell's row drawer, and the note counts as shown only once it opened", async () => {
  const pay0 = "txtIncomePayment30";
  const restore = hostFault(pay0, "1000", "999.00");
  const checks = countQueueChecks();
  const h = hooks();
  let s: FormSession | undefined;
  try {
    s = await opened(at("2550Q", 2023, 3), h);
    fillSched3Row(s, 0, "Agent One", "1000");
    await s.idle();
    expect(s.boxMessageOf(pay0)).toBe("(C) Income Payment: you typed 1,000.00; the return holds 999.00.");
    const pi = Number(s.itemIdForKey(pay0)!.split(":")[0]);
    if (s.part(pi)().open) s.togglePart(pi);
    s.queue();
    await s.idle();
    expect(checks.count).toBe(0);
    expect(s.part(pi)().open).toBe(true);
    expect(s.drawerAsk()).toBe(pay0);
    // No schedule opened the drawer (none is drawn here): the note was not shown, so Queue stops on it again.
    s.queue();
    await s.idle();
    expect(checks.count).toBe(0);
    // The schedule opened the row: shown; the next press goes on.
    s.drawerOpened(pay0);
    expect(s.drawerAsk()).toBe(null);
    s.queue();
    await s.idle();
    expect(checks.count).toBeGreaterThan(0);
    expect(s.boxMessageOf(pay0)).toBe("(C) Income Payment: you typed 1,000.00; the return holds 999.00.");
  } finally {
    restore();
    checks.restore();
    s?.dispose();
  }
});

test("number note [L2]: a percent the host answers as another number shows as stored, with a note naming both (1601FQ Schedule 1's rate before the tax-relief answer)", async () => {
  // The host answers every Schedule 1 row blank while the tax-relief question is unanswered: the box shows the return's 0.00,
  // and a percent box says so like an amount box does (before: only money boxes had the sentence).
  const rate = "txtRate0";
  const s = await opened(at("1601FQ", 2025, 1));
  s.focus(rate);
  s.edit(rate, "42");
  s.blur(rate);
  await s.idle();
  expect(s.shownOf(rate)).toBe("0.00");
  expect(s.boxMessageOf(rate) ?? "").toContain("you typed 42; the return holds 0.00.");
  // The amount beside it had its sentence already; it still does.
  const income = "txtAmtIncomePay0";
  s.focus(income);
  s.edit(income, "1000");
  s.blur(income);
  await s.idle();
  expect(s.boxMessageOf(income) ?? "").toContain("you typed 1,000.00; the return holds 0.00.");
  s.dispose();
});

test("number note [L2]: a count the host stores as typed raises no note (2551Q Number of Sheets)", async () => {
  const sheets = "frm2551Qv2018:txtSheets";
  const s = await opened(at("2551Q", 2025, 3));
  s.focus(sheets);
  s.edit(sheets, "2");
  s.blur(sheets);
  await s.idle();
  expect(s.shownOf(sheets)).toBe("2");
  expect(s.boxMessageOf(sheets) ?? "").not.toContain("you typed");
  s.dispose();
});

/**
 * A money box with the keys, as fields.tsx's MaskedField leaves: on its blur or as it goes away (unmount), whichever comes first, it
 * reports its own reading (`reads`) and then its blur; going away also unregisters its blur. `blurOn`: "later" when the blur the
 * session asks for reaches it on a later turn (60 ms), "now" when it comes at once, "never" when it never comes.
 */
function heldBox(s: FormSession, k: string, typed: string, blurOn: "later" | "now" | "never", reads = typed) {
  let focused = true;
  const leave = () => {
    if (!focused) return;
    focused = false;
    s.edit(k, reads);
    s.blur(k);
  };
  let asked = 0;
  s.registerBlur(k, () => {
    asked++;
    if (blurOn === "now") leave();
    else if (blurOn === "later") setTimeout(leave, 60);
  });
  s.focus(k);
  s.edit(k, typed);
  return {
    asked: () => asked,
    /** The drawer closed: the box goes away. */
    unmount: () => {
      leave();
      s.registerBlur(k, null);
    },
  };
}

const sched3Rows = (s: FormSession, n: number) => Array.from({ length: n }, (_x, i) => [s.valueOf("txtNameWithHoldingAgent3" + i), s.valueOf("txtIncomePayment3" + i)]);
const sched3Messages = (s: FormSession) => [0, 1, 2].flatMap((n) => sched3(n)).filter((k) => Boolean(s.boxMessageOf(k)));

test("[B1] Delete row with an amount box in that row still focused: the typed amount goes with its row and never lands on the row that moves up", async () => {
  // Schedule.tsx's remove(): the row's clear (onClearRow), then the drawer closes at once and the focused cell goes away.
  for (const [period, blurOn] of [[4, "later"], [4, "now"]] as const) {
    const s = await opened(at("2550Q", blurOn === "later" ? 2025 : 2022, period));
    fillSched3Row(s, 0, "A0", "1000");
    fillSched3Row(s, 1, "A1", "2000");
    fillSched3Row(s, 2, "A2", "3000");
    await s.idle();
    const box = heldBox(s, "txtIncomePayment31", "500", blurOn);
    deleteRow(s, 1);
    box.unmount();
    await s.idle();
    expect(sched3Rows(s, 3)).toEqual([["A0", "1,000.00"], ["A2", "3,000.00"], ["", ""]]);
    expect(sched3Messages(s)).toEqual([]);
    s.dispose();
  }
});

test("[B1] Delete row with an amount box focused in a row below: the typed amount moves up with its own row and leaves nothing behind", async () => {
  const s = await opened(at("2550Q", 2021, 4));
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  fillSched3Row(s, 2, "A2", "3000");
  await s.idle();
  const box = heldBox(s, "txtIncomePayment32", "500", "later");
  deleteRow(s, 1);
  await s.idle();
  box.unmount();
  await s.idle();
  expect(sched3Rows(s, 3)).toEqual([["A0", "1,000.00"], ["A2", "500.00"], ["", ""]]);
  expect(sched3Messages(s)).toEqual([]);
  s.dispose();
});

test("[B1] Delete row of the last row with its amount box still focused: the row does not come back", async () => {
  const s = await opened(at("2550Q", 2023, 2));
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  await s.idle();
  const box = heldBox(s, "txtIncomePayment31", "500", "later");
  deleteRow(s, 1);
  box.unmount();
  await s.idle();
  expect(sched3Rows(s, 3)).toEqual([["A0", "1,000.00"], ["", ""], ["", ""]]);
  expect(sched3(1).map((k) => s.valueOf(k).replace(/^0\.00$/, ""))).toEqual(["", "", "", "", ""]);
  s.dispose();
});

test("[B1] Delete row while a focused amount box never lets go: nothing is cleared and the filer is told to finish the amount, once", async () => {
  const h = hooks();
  const s = await opened(at("2550Q", 2023, 4), h);
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  await s.idle();
  const box = heldBox(s, "txtIncomePayment30", "500", "never");
  const started = Date.now();
  deleteRow(s, 0);
  await s.idle();
  expect(box.asked()).toBe(1);
  expect(Date.now() - started).toBeGreaterThanOrEqual(450);
  const told = h.said.toasts.map((t) => t.message).filter((m) => /^Finish the amount in .+ first\.$/.test(m));
  expect(told).toEqual(["Finish the amount in (C) Income Payment first."]);
  expect(s.valueOf("txtNameWithHoldingAgent30")).toBe("A0");
  expect(s.valueOf("txtNameWithHoldingAgent31")).toBe("A1");
  // The box really was left: Delete row then clears the row, its typed amount with it.
  box.unmount();
  deleteRow(s, 0);
  await s.idle();
  expect(sched3Rows(s, 2)).toEqual([["A1", "2,000.00"], ["", ""]]);
  s.dispose();
});

test("[B1] a second Delete row while the first settles a focused amount box does nothing", async () => {
  const s = await opened(at("2550Q", 2022, 1));
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  fillSched3Row(s, 2, "A2", "3000");
  await s.idle();
  const box = heldBox(s, "txtIncomePayment30", "500", "later");
  deleteRow(s, 0);
  deleteRow(s, 0);
  await s.idle();
  expect(box.asked()).toBe(1);
  expect(sched3Rows(s, 3)).toEqual([["A1", "2,000.00"], ["A2", "3,000.00"], ["", ""]]);
  s.dispose();
});

test("[B1] Delete row with a focused amount box that is gone (its blur unregistered): treated as left, cleared at once", async () => {
  const h = hooks();
  const s = await opened(at("2550Q", 2022, 2), h);
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  fillSched3Row(s, 2, "A2", "3000");
  await s.idle();
  let called = 0;
  s.registerBlur("txtIncomePayment31", () => void called++);
  s.focus("txtIncomePayment31");
  s.edit("txtIncomePayment31", "500");
  s.registerBlur("txtIncomePayment31", null);
  deleteRow(s, 1);
  // Cleared in the same turn: nothing waited.
  expect(sched3Rows(s, 3)).toEqual([["A0", "1,000.00"], ["A2", "3,000.00"], ["", ""]]);
  await s.idle();
  expect(called).toBe(0);
  expect(s.focusedPart()).toBe(null);
  expect(h.said.toasts.some((t) => t.message.startsWith("Finish the amount"))).toBe(false);
  expect(sched3Rows(s, 3)).toEqual([["A0", "1,000.00"], ["A2", "3,000.00"], ["", ""]]);
  s.dispose();
});

test("[B1] no box focused: Delete row clears the row and moves the rows below up in the same turn, as before", async () => {
  const s = await opened(at("2550Q", 2022, 3));
  fillSched3Row(s, 0, "A0", "1000");
  fillSched3Row(s, 1, "A1", "2000");
  await s.idle();
  s.edit("txtIncomePayment30", "1500");
  deleteRow(s, 1);
  expect(sched3Rows(s, 2)).toEqual([["A0", "1,500.00"], ["", ""]]);
  await s.idle();
  expect(sched3Rows(s, 2)).toEqual([["A0", "1,500.00"], ["", ""]]);
  s.dispose();
});

/** 2551Q Schedule 1's slots, as rows.tsx's scheduleProps reads them (item keys, columns). */
function sched2551(s: FormSession) {
  const it = s.schema!.parts.flatMap((p) => p.items).find((x) => (x.keys || []).some((row) => row.includes("txtATCAmt1")))!;
  const keys = it.keys!;
  const cols = it.cols || [];
  return {
    keys,
    /** The drawer's Delete row (rows.tsx onClearRow): "" for every visible, open cell of slot r. */
    deleteRow: (r: number) => s.choose(Object.fromEntries(keys[r].filter((k, c) => cols[c]?.t !== "hidden" && !s.box(k).locked()).map((k) => [k, ""]))),
    /** Slot r's draft values, key by key. */
    slot: (r: number) => Object.fromEntries(keys[r].map((k) => [k, s.valueOf(k)])),
  };
}

/** An empty slot's value: blank, a zero amount or rate, the cleared ATC's "0", an unticked box. */
const zeroOrBlank = (v: string) => v.trim() === "" || v === "false" || /^0(\.0+)?$/.test(v.replace(/,/g, ""));

test("[B1] 2551Q Schedule 1: after a Delete row with a focused cell holding typed text, no cleared slot carries an amount", async () => {
  for (const [period, r, blurOn] of [[1, 1, "later"], [2, 1, "now"], [3, 2, "later"]] as const) {
    const s = await opened(at("2551Q", 2019, period));
    const t = sched2551(s);
    const atc = s.box("drpATC1").options().map((o) => o.value).filter((v) => v && !v.startsWith("catalog:"));
    for (const [n, amount] of [[0, "1000"], [1, "2000"], [2, "3000"]] as const) {
      s.choose({ [t.keys[n][0]]: atc[n] });
      s.edit("txtATCAmt" + (n + 1), amount);
      s.flush();
      await s.idle();
    }
    // Typed into the open row's amount (the keys go in front of the stored 2,000 or 3,000), not left before Delete row.
    const amt = "txtATCAmt" + (r + 1);
    const box = heldBox(s, amt, "500" + (r + 1) + "000", blurOn);
    t.deleteRow(r);
    box.unmount();
    await s.idle();
    // Schedule 1 keeps its slots (no row moves up): the deleted row's slot is cleared whole, its amount with it (the cleared ATC
    // reads "0"); the other rows keep theirs.
    expect(Object.entries(t.slot(r)).filter(([, v]) => !zeroOrBlank(v))).toEqual([]);
    for (const n of [0, 1, 2].filter((n) => n !== r)) expect(s.valueOf("txtATCAmt" + (n + 1))).toBe(["1,000.00", "2,000.00", "3,000.00"][n]);
    s.dispose();
  }
});

/** A headless session has no preview window: Print previews "natively" here, so print() goes on to the host's door. */
function previewHere(): () => void {
  const native = Print as unknown as { previewsNatively: () => boolean };
  const real = native.previewsNatively;
  native.previewsNatively = () => true;
  return () => void (native.previewsNatively = real);
}

/** Records `read()` each time the host's `name` door is called (what the save or the paper took), then calls it. */
function recordAt<K extends "screen_form_save" | "screen_form_print">(name: K, read: () => string): { seen: string[]; restore: () => void } {
  const door = bir as unknown as Record<K, (...args: unknown[]) => unknown>;
  const real = door[name];
  const out = { seen: [] as string[], restore: () => void (door[name] = real) };
  door[name] = (...args: unknown[]) => {
    out.seen.push(read());
    return real(...args);
  };
  return out;
}

for (const [period, door, name] of [[4, "screen_form_save", "Save"], [2, "screen_form_print", "Print"]] as const) {
  test(`[B2] ${name} with an amount box still focused leaves it first: the ${name === "Save" ? "save" : "paper"} takes the box's amount`, async () => {
    // The box holds "1500,50": it read as 1500 while typed and reads 1,500.50 only on leaving (a decimal comma).
    const key = "frm2551Qv2018:txt15";
    const h = hooks();
    const s = await opened(at("2551Q", 2019, period), h);
    s.jumpTo(2);
    const rec = recordAt(door, () => s.valueOf(key));
    const unstub = previewHere();
    try {
      const box = heldBox(s, key, "1500", "later", "1500.50");
      if (door === "screen_form_save") s.saveNow();
      else s.print();
      await s.idle();
      expect(rec.seen[0]).toBe("1,500.50");
      expect(box.asked()).toBe(1);
      if (door === "screen_form_save") expect(h.said.toasts.map((t) => t.message)).toContain("Draft saved.");
      expect(h.said.toasts.some((t) => t.message.startsWith("Finish the amount"))).toBe(false);
    } finally {
      unstub();
      rec.restore();
      s.dispose();
    }
  });
}

test("[B2] no box focused: Save and Print run in the same turn, as before", async () => {
  const key = "frm2551Qv2018:txt15";
  const s = await opened(at("2551Q", 2022, 4));
  const saves = recordAt("screen_form_save", () => s.valueOf(key));
  const prints = recordAt("screen_form_print", () => s.valueOf(key));
  const unstub = previewHere();
  try {
    s.edit(key, "700");
    s.saveNow();
    expect(saves.seen).toEqual(["700.00"]);
    s.edit(key, "800");
    s.print();
    expect(prints.seen).toEqual(["800.00"]);
  } finally {
    unstub();
    saves.restore();
    prints.restore();
    s.dispose();
  }
});
