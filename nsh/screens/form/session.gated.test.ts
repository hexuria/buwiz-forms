// G1 (Q9): a box another answer keeps shut is not drawn — not its box, not its "Needs …" line, and when every
// drawn box of a row is shut the whole row (number and title too) is not drawn. A shut box that still holds a
// value, a shown sentence or a to-do stop stays drawn, faded; a schedule's grid stays while any cell is open.
// Through the real door (scratch database, the demo taxpayer Juana Santos); each test uses its own form/period.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { scheduleProps } from "./rows";
import { FormSession, type FormTarget } from "./session";
import type { SchemaItem } from "./contract";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

/** The private G1 predicates, for the sweep's per-key checks (same objects the page reads). */
const gated = (s: FormSession) =>
  s as unknown as {
    hiddenBox(k: string): boolean;
    drawnKeys(it: SchemaItem): string[];
    blankShown(k: string): boolean;
    st: {
      boxMessages: Record<string, string>;
      messages: Record<string, string>;
      messageKeys: Record<string, string[]>;
      stopKeys: Set<string>;
    };
  };

/** Every drawn box key of the form, with its row id (a schedule's cells, an item's fields). */
function drawnBoxes(s: FormSession): { k: string; id: string }[] {
  const out: { k: string; id: string }[] = [];
  s.schema!.parts.forEach((p, pi) =>
    p.items.forEach((it, ii) => {
      const id = `${pi}:${ii}`;
      for (const k of gated(s).drawnKeys(it)) out.push({ k, id });
    }),
  );
  return out;
}

/** The item of a part with the printed number `no` ("15", "12A"). */
function itemNo(s: FormSession, pi: number, no: string): { id: string; it: SchemaItem } | null {
  const ii = s.schema!.parts[pi].items.findIndex((it) => it.no === no);
  return ii < 0 ? null : { id: `${pi}:${ii}`, it: s.schema!.parts[pi].items[ii] };
}

/** The radio field of an item whose label matches (the opener a gate names). */
function radioLabeled(it: SchemaItem, re: RegExp) {
  return (it.f || []).find((f) => f.t === "radio" && re.test(f.l));
}

test("G1 0605 item 17: the Specify box is absent until Others (Specify) is picked, then comes back in place", async () => {
  const s = await opened(at("0605", 2025, 1));
  const k = "frm0605:txtOthersName";
  const id = s.itemIdForKey(k)!;
  const info = () => s.row(id)();
  // The radio group and the second manner group draw; only the specify box is gated.
  expect(s.box(k).off()).toBe("Needs item 17 = Others");
  expect(info().hidden).toBe(false);
  expect(info().hiddenBoxes).toContain(k);
  expect(info().needs).not.toContain("Needs item 17 = Others");
  // Pick Others (Specify): the box opens at its schema place, still in item 17's row.
  const item = s.item(id)!;
  const others = radioLabeled(item, /others \(specify\)/i);
  expect(others).toBeTruthy();
  s.pickRadio(others!.k);
  await s.idle();
  expect(s.box(k).off()).toBe("");
  expect(info().hiddenBoxes).not.toContain(k);
  expect(info().hidden).toBe(false);
  // Pick another manner: it shuts again.
  const self = radioLabeled(item, /self-assessment/i);
  expect(self).toBeTruthy();
  s.pickRadio(self!.k);
  await s.idle();
  expect(s.box(k).off()).toBe("Needs item 17 = Others");
  expect(info().hiddenBoxes).toContain(k);
  s.dispose();
});

test("G1 0605: the specify box is a gated box inside a drawn row, and the approval radios are a gated row of their own", async () => {
  const s = await opened(at("0605", 2025, 1));
  const othersK = "frm0605:txtOthersName";
  const yn1 = "frm0605:itemApprovedYN:_1";
  const yn2 = "frm0605:itemApprovedYN:_2";
  const id17 = s.itemIdForKey(othersK)!;
  const idYn = s.itemIdForKey(yn1)!;
  // Item 17's manner radios are never gated, so its row draws with only the specify box shut.
  expect(s.item(id17)!.no).toBe("17");
  expect(s.row(id17)().hidden).toBe(false);
  expect(s.row(id17)().hiddenBoxes).toContain(othersK);
  expect(s.row(id17)().hiddenBoxes).not.toContain("frm0605:itemMannerOfPayment:_1");
  expect(s.row(id17)().needs).not.toContain(s.box(othersK).off());
  // The approval pair lives in its own item (3:0): every drawn box of it is gated, so the whole row is not drawn.
  expect(s.box(yn1).off()).not.toBe("");
  expect(s.box(yn2).off()).not.toBe("");
  expect(s.row(idYn)().hidden).toBe(true);
  expect([...s.row(idYn)().hiddenBoxes].sort()).toEqual([yn1, yn2].sort());
  // Walk item 17's manner picks: the assessment/AR manner opens the approval row in place.
  const manners = (s.item(id17)!.f || []).filter((f) => f.t === "radio" && f.k.startsWith("frm0605:itemMannerOfPayment"));
  let assessPick = "";
  for (const f of manners) {
    s.pickRadio(f.k);
    await s.idle();
    if (!s.box(yn1).off()) {
      assessPick = f.k;
      break;
    }
  }
  expect(assessPick).not.toBe("");
  expect(s.row(idYn)().hidden).toBe(false);
  expect(s.row(idYn)().hiddenBoxes).toEqual([]);
  s.dispose();
});

test("G1 0619E item 4: item 14's whole row hides until Any Taxes Withheld = Yes; a stored value or a refusal keeps it drawn", async () => {
  const s = await opened(at("0619E", 2025, 3));
  const k = "frm0619E:txtTax14";
  const id = s.itemIdForKey(k)!;
  const info = () => s.row(id)();
  expect(s.item(id)!.no).toBe("14");
  expect(s.box(k).off()).toBe("Needs item 4 = Yes");
  // One money box, shut: the row is not drawn and not counted.
  expect(info().hidden).toBe(true);
  expect(info().hiddenBoxes).toEqual([k]);
  expect(s.part(2)().items).toBe(7);
  // The to-do never points at it: hidden required boxes are not counted.
  expect(s.todo().some((r) => r.id === id)).toBe(false);
  // Answer Yes: the row appears in schema order (still part 2, item 14).
  s.pickRadio("frm0619E:optWithheld:Y");
  await s.idle();
  expect(s.box(k).off()).toBe("");
  expect(info().hidden).toBe(false);
  expect(s.part(2)().items).toBe(8);
  // Type a remittance, then answer No again: the host zeroes a gated amount, so the row hides again.
  s.edit(k, "1250.50");
  await s.idle();
  expect(s.valueOf(k)).not.toBe("");
  s.pickRadio("frm0619E:optWithheld:N");
  await s.idle();
  expect(s.box(k).off()).toBe("Needs item 4 = Yes");
  expect(s.box(k).locked()).toBe(true);
  expect(info().hidden).toBe(true);
  // A value that reached the draft another way (an import, an old draft — the host keeps it while the
  // gate is shut) keeps the box drawn, faded, with its "Needs …" line. White-box: the value is placed
  // where open() would have loaded it, then the page state is republished.
  const inner = s as unknown as { st: { values: Record<string, string> }; publish(): void };
  inner.st.values[k] = "321.00";
  inner.publish();
  expect(gated(s).blankShown(k)).toBe(false);
  expect(info().hidden).toBe(false);
  expect(info().hiddenBoxes).toEqual([]);
  expect(info().needs).toContain("Needs item 4 = Yes");
  // Back to blank: hidden again — it was the value keeping it drawn.
  inner.st.values[k] = "";
  inner.publish();
  expect(info().hidden).toBe(true);
  // A refused edit on the shut box keeps it drawn too, with the host's own sentence (B16 unchanged).
  s.edit(k, "750.00");
  await s.idle();
  expect(s.boxMessageOf(k)).not.toBe("");
  expect(info().hidden).toBe(false);
  expect(info().hiddenBoxes).toEqual([]);
  expect(info().boxMessages.some((m) => m.key === k && m.sentence !== "")).toBe(true);
  s.dispose();
});

test("G1 2551Q item 12: the tax-relief specify box follows the Yes radio", async () => {
  const s = await opened(at("2551Q", 2025, 1));
  const k = "frm2551Qv2018:txtTaxReliefSpecify";
  const id = s.itemIdForKey(k)!;
  const info = () => s.row(id)();
  expect(s.item(id)!.no).toBe("12");
  // Item 12's No is the fresh return's pick: the specify box is shut but the row (its radios) draws.
  expect(s.box(k).off()).toBe("Needs item 12 = Yes");
  expect(info().hidden).toBe(false);
  expect(info().hiddenBoxes).toContain(k);
  s.pickRadio("frm2551Qv2018:taxTreaty_1");
  await s.idle();
  expect(s.box(k).off()).toBe("");
  expect(info().hiddenBoxes).not.toContain(k);
  s.pickRadio("frm2551Qv2018:taxTreaty_2");
  await s.idle();
  expect(info().hiddenBoxes).toContain(k);
  s.dispose();
});

test("G1 1700: the spouse chain opens one level at a time — Married, then spouse income, then joint filing, then the spouse part", async () => {
  const s = await opened(at("1700", 2025, 1));
  const p1 = s.schema!.parts[1];
  const atNo = (no: string) => itemNo(s, 1, no)!;
  const rowHidden = (no: string) => s.row(atNo(no).id)().hidden;
  // Fresh return: items 16 and 17 and all of part 2 (items 18-25) are gated away.
  expect(rowHidden("16")).toBe(true);
  expect(rowHidden("17")).toBe(true);
  expect(s.part(2)().items).toBe(0);
  expect(s.part(2)().todo).toBe(0);
  // Married opens item 16 only: item 17 still needs "item 16 = Yes".
  const i15 = atNo("15");
  const married = radioLabeled(i15.it, /married/i);
  expect(married).toBeTruthy();
  s.pickRadio(married!.k);
  await s.idle();
  expect(rowHidden("16")).toBe(false);
  expect(rowHidden("17")).toBe(true);
  expect(s.part(2)().items).toBe(0);
  // Spouse income = Yes opens item 17; part 2 still waits for Joint Filing.
  const yes16 = radioLabeled(atNo("16").it, /^yes$/i);
  expect(yes16).toBeTruthy();
  s.pickRadio(yes16!.k);
  await s.idle();
  expect(rowHidden("17")).toBe(false);
  expect(s.part(2)().items).toBe(0);
  // Joint Filing opens the spouse part — but item 25 waits one level deeper, on item 24 = Yes.
  const joint = radioLabeled(atNo("17").it, /joint filing/i);
  expect(joint).toBeTruthy();
  s.pickRadio(joint!.k);
  await s.idle();
  expect(s.part(2)().items).toBe(7);
  expect(s.row(itemNo(s, 2, "18")!.id)().hidden).toBe(false);
  expect(s.row(itemNo(s, 2, "25")!.id)().hidden).toBe(true);
  // Item 24 = Yes opens item 25 too: the whole spouse part draws.
  const i24 = itemNo(s, 2, "24")!;
  const yes24 = radioLabeled(i24.it, /^yes$/i);
  expect(yes24).toBeTruthy();
  s.pickRadio(yes24!.k);
  await s.idle();
  expect(s.part(2)().items).toBe(8);
  expect(s.row(i24.id)().hidden).toBe(false);
  s.dispose();
});

test("G1 sweep: on a fresh return every blank box with an off reason is hidden, no hidden row is to do, and every opener exists", async () => {
  createDemoTaxpayer();
  const codes = bir.screen_forms_index().map((f) => f.c);
  expect(codes.length).toBe(43);
  const problems: string[] = [];
  const chains: string[] = [];
  let gatedKeys = 0;
  let hiddenKeys = 0;
  let keptVisible = 0;
  for (const code of codes) {
    let s = new FormSession(at(code, 2025, 1));
    let err = await s.open();
    await s.idle();
    if (err) {
      // Some forms only open on their annual period.
      s.dispose();
      s = new FormSession(at(code, 2025, 0));
      err = await s.open();
      await s.idle();
    }
    if (err) {
      problems.push(`${code}: open failed: ${err}`);
      s.dispose();
      continue;
    }
    try {
      const g = gated(s);
      const hiddenRows = new Set<string>();
      s.schema!.parts.forEach((p, pi) =>
        p.items.forEach((_it, ii) => {
          const id = `${pi}:${ii}`;
          if (s.row(id)().hidden) hiddenRows.add(id);
        }),
      );
      for (const r of s.todo()) {
        if (hiddenRows.has(r.id)) problems.push(`${code}: to-do row ${r.id} is hidden`);
      }
      // Part counts agree with the drawn rows.
      s.schema!.parts.forEach((_p, pi) => {
        const drawn = s.schema!.parts[pi].items.filter((_it, ii) => !s.row(`${pi}:${ii}`)().hidden).length;
        if (s.part(pi)().items !== drawn) problems.push(`${code}: part ${pi} items ${s.part(pi)().items} != drawn ${drawn}`);
      });
      for (const { k, id } of drawnBoxes(s)) {
        const off = s.box(k).off();
        if (!off) continue;
        gatedKeys++;
        const blank = g.blankShown(k);
        const hid = g.hiddenBox(k);
        if (blank) {
          hiddenKeys++;
          // A blank gated box stays drawn only for the documented reasons: a sentence on the box, a
          // row message naming it, or a to-do stop. Anything else must hide.
          const st = g.st;
          const kept =
            Boolean(st.boxMessages[k]) ||
            st.stopKeys.has(k) ||
            Boolean(st.messages[id] && (st.messageKeys[id] || []).includes(k));
          if (!hid && !kept) problems.push(`${code}: ${k} (row ${id}) is blank+off but not hidden`);
          if (hid && kept) problems.push(`${code}: ${k} (row ${id}) is hidden though a message/stop keeps it`);
        } else {
          keptVisible++;
          if (hid) problems.push(`${code}: ${k} holds a value but is hidden`);
        }
        // The opener the tag names exists on the form (an item number, a schedule, or a named gate).
        for (const m of off.matchAll(/items?\s+(\d+[A-Z]?)/gi)) {
          const no = m[1];
          const found = s.schema!.parts.some((p) => p.items.some((x) => x.no === no));
          if (!found) problems.push(`${code}: ${k} names missing item ${no} (${off})`);
          else {
            // A chain: every item with that number is itself gated away.
            const rows = s.schema!.parts.flatMap((p, pi) =>
              p.items.map((x, ii) => ({ x, id: `${pi}:${ii}` })).filter((e) => e.x.no === no),
            );
            if (rows.length && rows.every((e) => s.row(e.id)().hidden)) chains.push(`${code}: ${k} behind gated item ${no}`);
          }
        }
      }
    } finally {
      s.dispose();
    }
  }
  console.log(`G1 sweep: ${codes.length} forms, ${gatedKeys} gated keys, ${hiddenKeys} hidden, ${keptVisible} kept for a value, ${chains.length} chain links`);
  for (const c of chains.slice(0, 20)) console.log(`   chain ${c}`);
  expect(problems).toEqual([]);
  // The spouse chains of 1700/1701 are the known two-level gates; the sweep must find them.
  expect(chains.length).toBeGreaterThan(0);
}, 300000);

test("G1 is page-only: with item 14's row hidden, the draft is the host's own and it still prints", async () => {
  const s = await opened(at("0619E", 2025, 4));
  const id = s.itemIdForKey("frm0619E:txtTax14")!;
  expect(s.row(id)().hidden).toBe(true);
  const host = await bir.screen_form_open(DEMO.tin, "0619E", 2025, 4);
  // Each open stamps its own created_at/updated_at; everything else is the host's draft.
  const stamped = (d: object) => ({ ...d, created_at: "", updated_at: "" });
  expect(stamped(s.draft()!)).toEqual(stamped(host.draft));
  const printed = bir.screen_form_print("0619E", s.draft()!);
  expect(printed.path).toBeTruthy();
  s.dispose();
});

test("G1 1700 item 18: refused spouse TIN boxes say a shared reason once, not once per box", async () => {
  const s = await opened(at("1700", 2023, 1));
  const it = s.schema!.parts[2].items.find((i) => i.no === "18")!;
  const keys = (it.f ?? []).map((f) => f.k);
  expect(keys.length).toBe(4);
  const id = s.itemIdForKey(keys[0])!;

  for (const k of keys) s.edit(k, /branch/i.test(k) ? "00000" : "123");
  await s.idle();
  for (const k of keys) expect(s.boxMessageOf(k)).not.toBe("");
  // Three boxes got the same sentence from the host: the row prints it once.
  const shown = s.row(id)().boxMessages.map((m) => m.sentence);
  expect(shown.length).toBe(new Set(shown).size);
  expect(shown.length).toBeLessThan(keys.length);
  s.dispose();
});

test("1700 Schedule 1: a row's Taxpayer and Spouse columns are tick boxes, and never both ticked", async () => {
  const s = await opened(at("1700", 2022, 1));
  const it = s.schema!.parts.flatMap((p) => p.items).find((i) => i.rows && (i.cols ?? []).filter((c) => c.t === "radio").length === 2)!;
  const id = s.itemIdForKey(it.keys![0][0])!;
  const table = scheduleProps(s, id, it, () => true);
  const [taxpayer, spouse] = [table.cell(0, 0), table.cell(0, 1)];
  expect(taxpayer.field.t).toBe("chk");
  expect(spouse.field.t).toBe("chk");
  // The host keeps a tick only on a row that holds something.
  table.cell(0, 2).onEdit("ACME CORP");
  await s.idle();
  taxpayer.onEdit("true");
  await s.idle();
  expect(s.valueOf(it.keys![0][0])).toBe("true");
  // Spouse on a return that is not joint: the host refuses it, and Taxpayer stays ticked.
  spouse.onEdit("true");
  await s.idle();
  expect(s.valueOf(it.keys![0][0]) === "true" && s.valueOf(it.keys![0][1]) === "true").toBe(false);
  expect(table.summary(0)()).toBe("Taxpayer · ACME CORP");
  s.dispose();
});

test("1700 Schedule 1: switching row 4_2 to Taxpayer keeps row 4_3 ACME as Spouse with its amounts", async () => {
  const s = await opened(at("1700", 2025, 1));
  const atNo = (no: string) => itemNo(s, 1, no)!;
  const married = radioLabeled(atNo("15").it, /married/i)!;
  s.pickRadio(married.k);
  await s.idle();
  const spouseIncome = radioLabeled(atNo("16").it, /^yes$/i)!;
  s.pickRadio(spouseIncome.k);
  await s.idle();
  const joint = radioLabeled(atNo("17").it, /joint filing/i)!;
  s.pickRadio(joint.k);
  await s.idle();

  const schedules = s.schema!.parts.flatMap((p) => p.items).filter((i) => i.rows && (i.cols ?? []).filter((c) => c.t === "radio").length === 2);
  const pageItem = schedules.find((i) => i.rows === 4)!;
  const popupItem = schedules.find((i) => i.rows === 12)!;
  const page = scheduleProps(s, s.itemIdForKey(pageItem.keys![0][0])!, pageItem, () => true);
  const popup = scheduleProps(s, s.itemIdForKey(popupItem.keys![0][0])!, popupItem, () => true);

  for (let row = 0; row < 4; row++) {
    page.cell(row, 2).onEdit(`Employer ${row + 1}`);
    await s.idle();
    page.cell(row, 8).onEdit(String(1000 + row));
    page.cell(row, 10).onEdit(String(100 + row));
    await s.idle();
    page.cell(row, 0).onEdit("true");
    await s.idle();
  }

  popup.cell(1, 1).onEdit("true");
  await s.idle();
  popup.cell(2, 2).onEdit("ACME");
  await s.idle();
  popup.cell(2, 8).onEdit("10000");
  popup.cell(2, 10).onEdit("1000");
  await s.idle();
  popup.cell(2, 1).onEdit("true");
  await s.idle();

  const acmeBefore = popupItem.keys!.find((row) => s.valueOf(row[2]) === "ACME")!;
  expect(s.valueOf(acmeBefore[1])).toBe("true");
  const compensation = s.valueOf(acmeBefore[8]);
  const withholding = s.valueOf(acmeBefore[10]);

  popup.cell(1, 0).onEdit("true");
  await s.idle();

  const acmeAfter = popupItem.keys!.find((row) => s.valueOf(row[2]) === "ACME")!;
  expect(acmeAfter).toBeTruthy();
  expect(s.valueOf(acmeAfter[1])).toBe("true");
  expect(s.valueOf(acmeAfter[8])).toBe(compensation);
  expect(s.valueOf(acmeAfter[10])).toBe(withholding);
  s.dispose();
});
