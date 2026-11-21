// B10: an "Additional details" fold is open while the filer opened it or a shown message, fact card
// or to-do step points inside; it closes by hand only when nothing points in. The built schemas keep
// exactly one such fold — 1602Q's Schedule 1 tail (the sixteen line-of-business forms' folds were
// dissolved by the layout overrides, so their boxes draw in titled rows; the table test keeps that true).
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";

warmForms();

const at = (formCode: string, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year: 2025, period, mode: "return" });

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

const FOLD_KEY = "frm1602Q:txtSched1Amount15";
const FOLD_ID = "6:0";

test("B10: 1602Q's fold opens on what points inside and closes by hand otherwise", async () => {
  const s = await opened(at("1602Q", 1));
  const id = s.itemIdForKey(FOLD_KEY);
  expect(id).toBe(FOLD_ID);
  expect(s.schema!.parts[6].items[0].l).toBe("Additional details");
  // Nothing points inside: closed, and the filer's own flag opens and closes it.
  expect(s.moreOpen(FOLD_ID)).toBe(false);
  expect(s.row(FOLD_ID)().moreOpen).toBe(false);
  s.toggleMore(FOLD_ID);
  expect(s.row(FOLD_ID)().moreOpen).toBe(true);
  s.toggleMore(FOLD_ID);
  expect(s.row(FOLD_ID)().moreOpen).toBe(false);
  // A refused edit's sentence lands shown on the box: the fold opens without the flag.
  s.edit(FOLD_KEY, "abc");
  await s.idle();
  expect(s.boxMessageOf(FOLD_KEY)).not.toBe("");
  expect(s.moreOpen(FOLD_ID)).toBe(false);
  expect(s.row(FOLD_ID)().moreOpen).toBe(true);
  // A to-do step to the row keeps it open.
  let steps = 0;
  while (s.stepAt() !== FOLD_ID && steps++ < 60) s.step(1);
  expect(s.stepAt()).toBe(FOLD_ID);
  expect(s.row(FOLD_ID)().moreOpen).toBe(true);
  s.dispose();
}, 120_000);

test("B10: the fold closes again once nothing points inside", async () => {
  const s = await opened(at("1602Q", 1));
  const id = s.itemIdForKey(FOLD_KEY);
  expect(id).toBe(FOLD_ID);
  // A blank write clears the refused sentence; with no flag set the fold folds back.
  s.edit(FOLD_KEY, "abc");
  await s.idle();
  expect(s.row(FOLD_ID)().moreOpen).toBe(true);
  s.edit(FOLD_KEY, "");
  await s.idle();
  expect(s.boxMessageOf(FOLD_KEY)).toBe("");
  expect(s.moreOpen(FOLD_ID)).toBe(false);
  expect(s.row(FOLD_ID)().moreOpen).toBe(false);
  s.dispose();
}, 120_000);

// The sixteen forms critics.json named: their line-of-business boxes must not sit in a closed
// "Additional details" fold — drawn in a titled row, or in a fold that attention already opened.
const LOB_KEYS: Record<string, string[]> = {
  "1603Q": ["frm1603Q:txtLineBus"],
  "1604E": ["frm1604e:txtLineBus"],
  "1604F": ["frm1604f:txtLineBus"],
  "1700": [], // lobContainer is unbound: the built page schema does not draw it (B13 keeps it that way)
  "1701": ["frm1701:txtLineBus"],
  "1701A": ["frm1701A:txtLineBus"],
  "1702EX": ["frm1702EX:txtLOB"],
  "1707": ["frm1707:txtLOB"],
  "1707A": ["frm1707Av2021:txtLineOfBusiness"], // txtLOB (the spouse's) is unbound in the built schema
  "1800": ["frm1800v2018:txtLineBus"],
  "2000": ["frm2000:txtLineBus"],
  "2000OT": ["frm2000OT:txtLineBus"],
  "2200C": ["frm2200C:txtLOB"],
  "2200P": ["frm2200P:txtLOB"],
  "2200S": ["frm2200S:txtLineBus"],
  "2552": ["frm2552:txtLOB"],
};

// Unbound line-of-business keys the critics named: the built schema must not draw them either.
const UNBOUND_LOB: [string, string[]][] = [
  ["1700", ["frm1700:lobContainer"]],
  ["1707A", ["frm1707Av2021:txtLOB"]],
];

for (const [code, keys] of Object.entries(LOB_KEYS)) {
  test(`B10: ${code}'s line-of-business box is not in a closed fold`, async () => {
    const index = new Map(bir.screen_forms_index().map((f) => [f.c, f]));
    const period = index.get(code)!.freq === "Annual" ? 0 : 1;
    const s = await opened(at(code, period));
    try {
      for (const k of keys) {
        const id = s.itemIdForKey(k);
        expect(id).toBeTruthy();
        const [pi, ii] = id!.split(":").map(Number);
        const it = s.schema!.parts[pi].items[ii];
        // Drawn in its titled row — or, were a fold ever to hold it, open because the fact card points in.
        expect(it.l !== "Additional details" || s.row(id!)().moreOpen).toBe(true);
      }
      for (const k of UNBOUND_LOB.find(([c]) => c === code)?.[1] ?? []) {
        expect(s.itemIdForKey(k)).toBe(undefined);
      }
    } finally {
      s.dispose();
    }
  }, 60_000);
}
