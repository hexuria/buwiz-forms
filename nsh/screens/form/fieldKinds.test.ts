// Form fields, the kinds that were wrong (form-fields PR 1, bugs B1, B2 and
// B15), through FormSession — the page's own entry points: edit() per
// keystroke, choose() for a select, pickDate() for the calendar, blur() on
// leaving. Each test opens its own return of the demo taxpayer; tests in
// this file share one database, in order, so each uses its own form or period.
//
// B1: 0605 Items 4 and 7 are three month/day/year boxes on a draft whose
// dates start empty; a typed part must land, part by part, and the batch's
// parts are judged as one date.
// B15: a cleared date-part box shows blank, never the official zero; an
// ordinary number still shows the host's zero.
// B2: 0605 Items 6 and 8 are picked from the official lists; a code the list
// does not have is refused without touching the draft.
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { dateParts, onePeriods } from "./schema";
import { FormSession, type FormTarget } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

interface DateGroup {
  /** The item's row id in the page ("part:item"). */
  row: string;
  name: string;
  month: string;
  day: string;
  year: string;
  /** The draft field the completed date lands in. */
  field: "due_date" | "return_period";
}

// Test plan A's rows for this PR: the two split dates of Form 0605. Later
// PRs add their forms' groups here.
const DATE_GROUPS: DateGroup[] = [
  {
    row: "0:3",
    name: "Item 4 due date",
    month: "frm0605:txtDueDateMonth",
    day: "frm0605:txtDueDateDay",
    year: "frm0605:txtDueDateYear",
    field: "due_date",
  },
  {
    row: "0:6",
    name: "Item 7 return period",
    month: "frm0605:txtReturnPeriodMonth",
    day: "frm0605:txtReturnPeriodDay",
    year: "frm0605:txtReturnPeriodYear",
    field: "return_period",
  },
];

const dateOf = (s: FormSession, field: string) =>
  (s.draft() as Record<string, { year: number; month: number; day: number } | null> | null)?.[field] ?? null;

for (const [index, g] of DATE_GROUPS.entries()) {
  test(`B1: ${g.name} takes typed parts on a new return, part by part`, async () => {
    const s = await opened(at("0605", 2023, index + 1));
    expect(dateOf(s, g.field)).toBeNull();
    // The bug's shape: a lone 0 first, then the real digits.
    s.edit(g.month, "0");
    await s.idle();
    s.edit(g.month, "08");
    await s.idle();
    expect(s.shownOf(g.month)).toBe("08");
    expect(s.valueOf(g.month)).toBe("08");
    // No refusal landed on the row ("draft has no ..." was the bug).
    expect(s.messageOf(g.row)).not.toContain("draft has no");
    s.edit(g.day, "17");
    await s.idle();
    s.edit(g.year, "2023");
    await s.idle();
    expect(dateOf(s, g.field)).toEqual({ year: 2023, month: 8, day: 17 });
    for (const k of [g.month, g.day, g.year]) {
      expect(s.shownOf(k)).not.toBe("");
      expect(s.valueOf(k)).not.toBe("");
    }
    // The completed date clears the item's incomplete-date sentence.
    expect(s.messageOf(g.row)).not.toContain("Invalid date entry");
    s.dispose();
  });
}

test("B1: 0605 Item 4 takes a calendar pick as one batch and keeps it", async () => {
  const s = await opened(at("0605", 2023, 3));
  const item = s.item("0:3");
  expect(item).toBeTruthy();
  const parts = dateParts(item!.f || []);
  expect(parts).toBeTruthy();
  s.pickDate(parts!, "2023-08-17");
  await s.idle();
  const g = DATE_GROUPS[0];
  for (const [k, v] of [
    [g.month, "08"],
    [g.day, "17"],
    [g.year, "2023"],
  ] as const) {
    expect(s.shownOf(k)).toBe(v);
    expect(s.valueOf(k)).toBe(v);
  }
  expect(dateOf(s, g.field)).toEqual({ year: 2023, month: 8, day: 17 });
  s.dispose();
});

test("B1: typed 0605 date parts survive a save and a reopen", async () => {
  const target = at("0605", 2023, 4);
  const s = await opened(target);
  const g = DATE_GROUPS[0];
  s.edit(g.month, "08");
  await s.idle();
  s.edit(g.day, "17");
  await s.idle();
  s.edit(g.year, "2023");
  await s.idle();
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  for (const [k, v] of [
    [g.month, "08"],
    [g.day, "17"],
    [g.year, "2023"],
  ] as const) {
    expect(again.shownOf(k)).toBe(v);
    expect(again.valueOf(k)).toBe(v);
  }
  expect(dateOf(again, g.field)).toEqual({ year: 2023, month: 8, day: 17 });
  again.dispose();
});

test("B15: a cleared 0619E due-date day shows blank, never zero; the draft keeps the host's zero", async () => {
  const target = at("0619E", 2025, 4);
  const day = "frm0619E:txtDueDay";
  const s = await opened(target);
  // Give the box a day first, so the clear is a real transition.
  s.edit(day, "15");
  await s.idle();
  s.edit(day, "");
  await s.idle();
  // The draft holds the host's cleared value (its two-digit zero); the box shows blank.
  expect(s.valueOf(day)).toBe("00");
  expect(s.shownOf(day)).toBe("");
  expect(s.box(day).value()).toBe("");
  // The row says the host's own required sentence for the date — on its box now (B3/B4).
  const row = s.itemIdForKey(day);
  expect(row).toBeTruthy();
  const partMsgs = (s.item(row!)?.f || []).map((f) => s.boxMessageOf(f.k)).join(" ");
  expect(partMsgs).toContain("Item 2");
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.valueOf(day)).toBe("00");
  expect(again.shownOf(day)).toBe("");
  expect(again.box(day).value()).toBe("");
  again.dispose();
});

test("B15: an ordinary whole number still shows the host's zero after a clear", async () => {
  const s = await opened(at("0605", 2023, 5));
  const sheets = "frm0605:txtNoOfSheets";
  s.edit(sheets, "3");
  await s.idle();
  s.edit(sheets, "");
  await s.idle();
  expect(s.valueOf(sheets)).toBe("0");
  expect(s.shownOf(sheets)).toBe("0");
  s.dispose();
});

test("B2: 0605's ATC and tax type are picked from the official lists", async () => {
  const s = await opened(at("0605", 2023, 6));
  const atc = "txtATCCode";
  const taxType = "txtTaxTypeCode";
  s.choose({ [taxType]: "DO" });
  await s.idle();
  s.choose({ [atc]: "FP010" });
  await s.idle();
  expect(s.shownOf(atc)).toBe("FP010");
  expect(s.valueOf(atc)).toBe("FP010");
  const draft = s.draft() as { atc?: { code: string; xml_index: number } | null; tax_type?: { code: string } | null } | null;
  expect(draft?.atc?.code).toBe("FP010");
  expect(draft?.atc?.xml_index).toBe(1);
  expect(draft?.tax_type?.code).toBe("DO");
  // A code the list does not have is refused and nothing about it lands.
  s.choose({ [atc]: "ZZ999" });
  await s.idle();
  expect(s.shownOf(atc)).toBe("FP010");
  expect((s.draft() as { atc?: { code: string } | null })?.atc?.code).toBe("FP010");
  expect(JSON.stringify(s.messages())).not.toContain("draft has no");
  // Blank clears the pick.
  s.choose({ [atc]: "" });
  await s.idle();
  expect((s.draft() as { atc?: unknown })?.atc ?? null).toBeNull();
  s.dispose();
});

test("B2: the ATC list follows the tax type — PT offers only PT ATCs and clears what it drops", async () => {
  const s = await opened(at("0605", 2023, 7));
  const atc = "txtATCCode";
  const taxType = "txtTaxTypeCode";
  s.choose({ [taxType]: "PT" });
  await s.idle();
  // FP010 is not offered under PT.
  s.choose({ [atc]: "FP010" });
  await s.idle();
  expect(s.shownOf(atc)).not.toBe("FP010");
  expect((s.draft() as { atc?: unknown })?.atc ?? null).toBeNull();
  // A PT ATC is.
  s.choose({ [atc]: "PT010" });
  await s.idle();
  expect(s.shownOf(atc)).toBe("PT010");
  // Moving off PT clears the ATC PT does not offer: PT010 is a PT ATC, so
  // under DO it is not offered and clears.
  s.choose({ [taxType]: "DO" });
  await s.idle();
  expect((s.draft() as { atc?: unknown })?.atc ?? null).toBeNull();
  s.dispose();
});

test("B20: a box the host answers in capitals takes them on blur; one it does not keeps its case", async () => {
  // 0605's official map capital()s the line-of-business text: the box keeps
  // "Retail" while it is typed, takes "RETAIL" when left, and a reopened
  // return shows the same — the box, the reopen and the print agree.
  const target = at("0605", 2024, 1);
  const lob = "frm0605:txtLineBus";
  const s = await opened(target);
  s.edit(lob, "Retail");
  await s.idle();
  // While the box could still have the keys it shows what was typed.
  expect(s.shownOf(lob)).toBe("Retail");
  s.blur(lob);
  await s.idle();
  expect(s.shownOf(lob)).toBe("RETAIL");
  expect(s.valueOf(lob)).toBe("RETAIL");
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.shownOf(lob)).toBe("RETAIL");
  again.dispose();
  // 2551Q's item-17 specify box is an ordinary text box its host does not
  // touch: the same typing stays as typed.
  const q = await opened(at("2551Q", 2025, 1));
  const qBox = "frm2551Qv2018:txt17Specify";
  q.edit(qBox, "TaX TrEaTy ReLiEf");
  await q.idle();
  q.blur(qBox);
  await q.idle();
  expect(q.shownOf(qBox)).toBe("TaX TrEaTy ReLiEf");
  q.dispose();
});



// ---------------------------------------------------------------- PR 3: month+year pairs and from-to ranges
//
// P3a: an enumerated month/year pair is one MM/YYYY field ("<month key>#period"). The year is the
// return's own and locked; the month is the filer's where the form asks a year-end month — under the
// Fiscal basis on 1702EX/MX/RT and the quarterlies (Calendar holds it at December). A typed period
// must agree with the locked year; a bad one lands nothing, keeps the typed text and its sentence.
// R1: a from/to pair is one row of two date controls; a reversed pair is refused on the To end only
// (1702EX's effectivity dates are the editable range).

const periodKeyOf = (month: string) => `${month}#period`;
const EX_MONTH = "frm1702EX:ddlPg1I2Date";
const EX_YEAR = "frm1702EX:txtPg1I2YearEnd";
const EX_PERIOD = periodKeyOf(EX_MONTH);
const EX_FISCAL = "frm1702EX:rdoPg1I1Fiscal";

async function openedFiscal(): Promise<FormSession> {
  const s = await opened(at("1702EX", 2024, 0));
  s.pickRadio(EX_FISCAL);
  await s.idle();
  return s;
}

test("PR3: a period takes every typed shape — 06/2024, 06-2024, 062024, 06 2024, 06/24 — and keeps it", async () => {
  const shapes = ["06/2024", "06-2024", "062024", "06 2024", "06/24"];
  for (const typed of shapes) {
    const s = await openedFiscal();
    s.edit(EX_PERIOD, typed);
    s.blur(EX_PERIOD);
    await s.idle();
    expect(s.shownOf(EX_PERIOD)).toBe("06/2024");
    expect(s.valueOf(EX_MONTH)).toBe("06");
    expect(s.valueOf(EX_YEAR)).toBe("24");
    expect(s.boxMessageOf(EX_MONTH)).toBe("");
    s.dispose();
  }
});

test("PR3: a bad period is refused whole on the month box and blocks Queue", async () => {
  const messages: string[] = [];
  createDemoTaxpayer();
  const s = new FormSession(at("1702EX", 2024, 0), { toast: (t) => void messages.push(t) });
  expect(await s.open()).toBe(null);
  await s.idle();
  const before = [s.valueOf(EX_MONTH), s.valueOf(EX_YEAR)];
  s.edit(EX_PERIOD, "13/2024");
  s.blur(EX_PERIOD);
  await s.idle();
  expect(s.boxMessageOf(EX_MONTH)).toContain("the month is 1 to 12");
  expect([s.valueOf(EX_MONTH), s.valueOf(EX_YEAR)]).toEqual(before);
  expect(s.isDirty()).toBe(true);
  s.queue();
  await s.idle();
  expect(messages[messages.length - 1]).toMatch(/needs? attention/);
  s.dispose();
});

test("PR3: text that is no MM/YYYY is refused; the typed text stays with its sentence", async () => {
  const s = await openedFiscal();
  const before = [s.valueOf(EX_MONTH), s.valueOf(EX_YEAR)];
  s.edit(EX_PERIOD, "not a period");
  s.blur(EX_PERIOD);
  await s.idle();
  expect(s.boxMessageOf(EX_MONTH)).toContain("type the month and year as MM/YYYY, for example 12/2023.");
  expect(s.shownOf(EX_PERIOD)).toBe("not a period");
  expect([s.valueOf(EX_MONTH), s.valueOf(EX_YEAR)]).toEqual(before);
  s.dispose();
});

test("PR3: a blank period clears the open month and keeps the locked year", async () => {
  const s = await openedFiscal();
  s.edit(EX_PERIOD, "06/2024");
  s.blur(EX_PERIOD);
  await s.idle();
  expect(s.valueOf(EX_MONTH)).toBe("06");
  s.edit(EX_PERIOD, "");
  s.blur(EX_PERIOD);
  await s.idle();
  expect(s.shownOf(EX_PERIOD)).toBe("");
  // The sel's empty choice is the host's "00"; the period reads blank either way.
  expect(s.valueOf(EX_MONTH)).toMatch(/^0*$/);
  expect(s.valueOf(EX_YEAR)).toBe("24");
  s.dispose();
});

test("PR3: the month picker fills the month with the held year; a wrong typed year is refused on the locked part", async () => {
  const s = await openedFiscal();
  const row = s.itemIdForKey(EX_MONTH)!;
  const period = onePeriods(s.item(row)!)[0];
  s.pickPeriodMonth(period, 9);
  await s.idle();
  expect(s.shownOf(EX_PERIOD)).toBe("09/2024");
  expect(s.valueOf(EX_YEAR)).toBe("24");
  // Another year than the return's is refused; the typed text stays with its sentence.
  s.edit(EX_PERIOD, "10/2025");
  s.blur(EX_PERIOD);
  await s.idle();
  expect(s.boxMessageOf(EX_MONTH)).not.toBe("");
  expect(s.shownOf(EX_PERIOD)).toBe("10/2025");
  expect(s.valueOf(EX_MONTH)).toBe("09");
  expect(s.valueOf(EX_YEAR)).toBe("24");
  s.dispose();
});

test("PR3: 1702Q's 20YY year takes two digits or four and stores the held two; a wrong year is refused", async () => {
  const s = await opened(at("1702Q", 2025, 1));
  const month = "frm1702q:rbYrEndMonth";
  const year = "frm1702q:txtYrEndYear";
  const key = periodKeyOf(month);
  // Fiscal basis, so a non-December month is allowed.
  s.pickRadio("frm1702q:rbForClndrFscl_2");
  await s.idle();
  s.edit(key, "06/25");
  s.blur(key);
  await s.idle();
  expect(s.valueOf(month)).toBe("06");
  expect(s.valueOf(year)).toBe("25");
  expect(s.shownOf(key)).toBe("06/2025");
  // Four digits typed land the same; the two-digit box is untouched (locked).
  s.edit(key, "09/2025");
  s.blur(key);
  await s.idle();
  expect(s.valueOf(year)).toBe("25");
  expect(s.shownOf(key)).toBe("09/2025");
  s.edit(key, "10/2026");
  s.blur(key);
  await s.idle();
  expect(s.boxMessageOf(month)).not.toBe("");
  expect(s.shownOf(key)).toBe("10/2026");
  expect(s.valueOf(month)).toBe("09");
  s.dispose();
});

test("PR3: a calendar-year period refuses a non-December month and takes December", async () => {
  const s = await opened(at("2550Q", 2025, 3));
  const month = "frm2550qv2024:selectedMonthNo2";
  const year = "frm2550qv2024:txtYearNo2";
  const key = periodKeyOf(month);
  s.pickRadio("frm2550qv2024:calendarNo1");
  await s.idle();
  const before = [s.valueOf(month), s.valueOf(year)];
  s.edit(key, "06/2025");
  s.blur(key);
  await s.idle();
  expect(s.boxMessageOf(month)).toMatch(/December/);
  expect([s.valueOf(month), s.valueOf(year)]).toEqual(before);
  s.edit(key, "12/2025");
  s.blur(key);
  await s.idle();
  expect(s.valueOf(month)).toBe("12");
  expect(s.boxMessageOf(month)).toBe("");
  s.dispose();
});

test("PR3: a typed period survives a save and a reopen", async () => {
  const target = at("1702EX", 2024, 0);
  const s = await opened(target);
  s.pickRadio(EX_FISCAL);
  await s.idle();
  s.edit(EX_PERIOD, "08/2024");
  s.blur(EX_PERIOD);
  await s.idle();
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.shownOf(EX_PERIOD)).toBe("08/2024");
  expect(again.valueOf(EX_MONTH)).toBe("08");
  expect(again.valueOf(EX_YEAR)).toBe("24");
  again.dispose();
});

test("PR3 R1: a reversed range is refused on the To end only, and clears when the end is fixed", async () => {
  const s = await opened(at("1702EX", 2024, 0));
  // An earlier test saved this return Fiscal (year-end August); the host's effectivity rule wants
  // dates inside the taxable year, so put it back on Calendar (December) first. A saved Fiscal month
  // would be cleared by the switch, so the host asks first (the removal guard): Continue it.
  s.pickRadio("frm1702EX:rdoPg1I1Calendar");
  await s.idle();
  if (s.overlay()?.kind === "removal") {
    s.confirmRemoval();
    await s.idle();
  }
  const from = "frm1702EX:txtPg1Pt1I17EffectivityFrom";
  const to = "frm1702EX:txtPg1Pt1I17EffectivityTo";
  // The host wants the effectivity inside the taxable year (2024): from <= Dec 2024, to >= Dec 2024.
  s.edit(from, "11/01/2024");
  s.blur(from);
  await s.idle();
  expect(s.valueOf(from)).toBe("11/01/2024");
  s.edit(to, "10/15/2024");
  s.blur(to);
  await s.idle();
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  expect(s.boxMessageOf(from)).toBe("");
  // The draft keeps no end for the refused pick.
  expect(s.valueOf(to)).toBe("");
  // Fixing the end clears the sentence and lands.
  s.edit(to, "12/15/2024");
  s.blur(to);
  await s.idle();
  expect(s.boxMessageOf(to)).toBe("");
  expect(s.valueOf(to)).toBe("12/15/2024");
  // Reversing from the other side is refused the same way.
  s.edit(from, "12/20/2024");
  s.blur(from);
  await s.idle();
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  expect(s.valueOf(from)).toBe("11/01/2024");
  s.dispose();
});

test("PR3 R1: a reversed To keeps the staged From — fixed and saved, both ends are stored on reopen", async () => {
  const target = at("1702EX", 2024, 0);
  const s = await opened(target);
  // An earlier test saved this return Fiscal (year-end August); the host's effectivity rule wants
  // dates inside the taxable year, so put it back on Calendar (December) first. A saved Fiscal month
  // would be cleared by the switch, so the host asks first (the removal guard): Continue it.
  s.pickRadio("frm1702EX:rdoPg1I1Calendar");
  await s.idle();
  if (s.overlay()?.kind === "removal") {
    s.confirmRemoval();
    await s.idle();
  }
  const from = "frm1702EX:txtPg1Pt1I17EffectivityFrom";
  const to = "frm1702EX:txtPg1Pt1I17EffectivityTo";
  s.edit(from, "11/01/2024");
  s.edit(to, "10/15/2024");
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  s.edit(to, "12/15/2024");
  expect(s.boxMessageOf(to)).toBe("");
  s.saveNow();
  await s.idle();
  s.dispose();

  const again = await opened(target);
  expect(again.valueOf(from)).toBe("11/01/2024");
  expect(again.valueOf(to)).toBe("12/15/2024");
  again.dispose();
});

test("PR3 R1: a reversed From fixed from the To side saves both ends on reopen", async () => {
  const target = at("1702EX", 2024, 0);
  const s = await opened(target);
  s.pickRadio("frm1702EX:rdoPg1I1Calendar");
  await s.idle();
  if (s.overlay()?.kind === "removal") {
    s.confirmRemoval();
    await s.idle();
  }
  const from = "frm1702EX:txtPg1Pt1I17EffectivityFrom";
  const to = "frm1702EX:txtPg1Pt1I17EffectivityTo";

  s.edit(from, "11/01/2024");
  s.edit(to, "12/15/2024");
  s.saveNow();
  await s.idle();

  s.edit(from, "12/20/2024");
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  s.edit("frm1702EX:txtPg1Pt1I8RegisteredName", "JUANA SANTOS");
  await s.idle();
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  s.edit(to, "12/25/2024");
  s.saveNow();
  await s.idle();
  s.dispose();

  const again = await opened(target);
  expect(again.valueOf(from)).toBe("12/20/2024");
  expect(again.valueOf(to)).toBe("12/25/2024");
  again.dispose();
});

test("PR3 R1: editing a held end while still reversed keeps the refusal and latest value", async () => {
  const target = at("1702EX", 2024, 0);
  const s = await opened(target);
  s.pickRadio("frm1702EX:rdoPg1I1Calendar");
  await s.idle();
  if (s.overlay()?.kind === "removal") {
    s.confirmRemoval();
    await s.idle();
  }
  const from = "frm1702EX:txtPg1Pt1I17EffectivityFrom";
  const to = "frm1702EX:txtPg1Pt1I17EffectivityTo";

  s.edit(from, "11/01/2024");
  s.edit(to, "12/15/2024");
  s.saveNow();
  await s.idle();

  s.edit(from, "12/20/2024");
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  s.edit(to, "12/10/2024");
  expect(s.boxMessageOf(to)).toContain("the end date is before the start date");
  expect(s.valueOf(from)).toBe("11/01/2024");
  expect(s.valueOf(to)).toBe("12/15/2024");
  s.edit(to, "12/25/2024");
  s.saveNow();
  await s.idle();
  s.dispose();

  const again = await opened(target);
  expect(again.valueOf(from)).toBe("12/20/2024");
  expect(again.valueOf(to)).toBe("12/25/2024");
  again.dispose();
});

test("PR3 R1: a locked range refuses edits on either end", async () => {
  const s = await opened(at("2550Q", 2025, 4));
  const from = "frm2550qv2024:RtnPeriodFromNo4";
  const to = "frm2550qv2024:RtnPeriodToNo4";
  expect(s.box(from).locked()).toBe(true);
  expect(s.box(to).locked()).toBe(true);
  const before = [s.valueOf(from), s.valueOf(to)];
  s.edit(to, "01/01/2025");
  s.blur(to);
  await s.idle();
  expect([s.valueOf(from), s.valueOf(to)]).toEqual(before);
  s.dispose();
});

test("PR3: Calendar over a saved Fiscal month asks before clearing it, then holds December", async () => {
  const target = at("1702EX", 2024, 0);
  // Put the return on Fiscal with a typed year-end month and save it.
  const s = await opened(target);
  s.pickRadio(EX_FISCAL);
  await s.idle();
  if (s.overlay()?.kind === "removal") {
    s.confirmRemoval();
    await s.idle();
  }
  s.edit(EX_PERIOD, "08/2024");
  s.blur(EX_PERIOD);
  await s.idle();
  s.saveNow();
  await s.idle();
  s.dispose();
  // Reopened Fiscal with month 08: Calendar would clear that month, so the host asks first and the
  // pick waits for Continue (the removal guard).
  const again = await opened(target);
  expect(again.valueOf(EX_FISCAL)).toBe("true");
  again.pickRadio("frm1702EX:rdoPg1I1Calendar");
  await again.idle();
  expect(again.overlay()?.kind).toBe("removal");
  // Still Fiscal while the question is open.
  expect(again.valueOf(EX_FISCAL)).toBe("true");
  again.confirmRemoval();
  await again.idle();
  expect(again.valueOf("frm1702EX:rdoPg1I1Calendar")).toBe("true");
  expect(again.valueOf(EX_FISCAL)).toBe("false");
  // Calendar holds the year-end at December.
  expect(again.valueOf(EX_MONTH)).toBe("12");
  expect(again.shownOf(EX_PERIOD)).toBe("12/2024");
  again.dispose();
});

// PR 5: the one-answer groups now draw as a segmented bar, a dropdown or a search list by size, but the draft keys and
// the untick rules are the session's own, unchanged.
test("PR5: 0605 Item 1 Calendar/Fiscal is a pair — one answer, picking the other moves it", async () => {
  const s = await opened(at("0605", 2024, 1));
  const cal = "frm0605:itemFiscalStartMonth:_1";
  const fis = "frm0605:itemFiscalStartMonth:_2";
  s.pickRadio(cal);
  await s.idle();
  if (s.overlay()?.kind === "removal") s.confirmRemoval();
  await s.idle();
  expect(s.valueOf(cal)).toBe("true");
  expect(s.valueOf(fis)).not.toBe("true");
  // A pair keeps one answer: re-picking the ticked one reports but changes nothing.
  s.pickRadio(cal);
  await s.idle();
  expect(s.valueOf(cal)).toBe("true");
  s.pickRadio(fis);
  await s.idle();
  if (s.overlay()?.kind === "removal") s.confirmRemoval();
  await s.idle();
  expect(s.valueOf(fis)).toBe("true");
  expect(s.valueOf(cal)).toBe("false");
  s.dispose();
});

test("PR5: 0605 Item 3 Quarter picks one of four and moves the answer", async () => {
  const s = await opened(at("0605", 2024, 1));
  const q = ["itemQuarter_1", "itemQuarter_2", "itemQuarter_3", "itemQuarter_4"];
  s.pickRadio(q[1]);
  await s.idle();
  if (s.overlay()?.kind === "removal") s.confirmRemoval();
  await s.idle();
  expect(s.valueOf(q[1])).toBe("true");
  s.pickRadio(q[2]);
  await s.idle();
  if (s.overlay()?.kind === "removal") s.confirmRemoval();
  await s.idle();
  expect(s.valueOf(q[2])).toBe("true");
  expect(s.valueOf(q[1])).not.toBe("true");
  s.dispose();
});

test("PR5: 0605 Item 17 Manner of Payment unticks on a re-pick and survives a save and reopen", async () => {
  const target = at("0605", 2024, 1);
  const s = await opened(target);
  const mop = (n: number) => `frm0605:itemMannerOfPayment:_${n}`;
  const others = "frm0605:itemMannerOfPaymentB:_1";
  // A group of seven: picking one unticks the last, a re-pick of the ticked one clears it (rg >= 3).
  s.pickRadio(mop(1));
  await s.idle();
  expect(s.valueOf(mop(1))).toBe("true");
  s.pickRadio(mop(5));
  await s.idle();
  expect(s.valueOf(mop(5))).toBe("true");
  expect(s.valueOf(mop(1))).toBe("false");
  s.pickRadio(mop(5));
  await s.idle();
  expect(s.valueOf(mop(5))).toBe("false");
  // The Others choice and its specify box are the same draft keys as before.
  s.pickRadio(others);
  await s.idle();
  expect(s.valueOf(others)).toBe("true");
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  expect(again.valueOf(others)).toBe("true");
  expect(again.valueOf(mop(5))).toBe("false");
  again.dispose();
});

// ---------------------------------------------------------------- PR 8 (Q8): typed TINs and the profile line
// 1801 item 11's executor TIN is typed_tin: four separate boxes the filer types (the draft joins them into
// administrator_tin's nine digits and administrator_branch_code). The "From the tax profile" line belongs to a
// TIN the profile holds locked — a typed TIN, an editable other-party TIN and a TIN opened for Edit have none.

const TIN_E = ["frm1801v2018:txtTINE1", "frm1801v2018:txtTINE2", "frm1801v2018:txtTINE3", "frm1801v2018:txtBranchCodeE"];

async function openedSample(formCode: string): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession({ tin: DEMO.tin, formCode, year: 0, period: 0, mode: "sample" });
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("PR8: the profile line is on a locked profile TIN and off a typed one", async () => {
  const locked = await opened(at("0605", 2025, 1));
  expect(locked.row("1:0")().tinLine).toBe(true);
  locked.dispose();
  const typed = await openedSample("1801");
  expect(typed.row("1:6")().tinLine).toBe(false);
  typed.dispose();
  const other = await opened({ tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" });
  expect(other.row("1:6")().tinLine).toBe(false); // 1706's buyer TIN (row 1:6 in the built schema)
  other.dispose();
});

test("PR8: a typed TIN's four boxes stay four keys and join the draft as administrator_tin + branch", async () => {
  const s = await openedSample("1801");
  s.edit(TIN_E[0], "000");
  s.edit(TIN_E[1], "123");
  s.edit(TIN_E[2], "453");
  s.edit(TIN_E[3], "00001");
  await s.idle();
  // The four boxes are still four keys on the page...
  for (const [i, k] of TIN_E.entries()) {
    expect(s.valueOf(k)).toBe(["000", "123", "453", "00001"][i]);
  }
  // ...and the draft keeps the mapping the print and the upload already use: nine digits + the branch code.
  const draft = s.draft() as { administrator_tin?: string; administrator_branch_code?: string } | null;
  expect(draft?.administrator_tin).toBe("000123453");
  expect(draft?.administrator_branch_code).toBe("00001");
  // Clearing the last box (the branch code) clears its own key and its draft part.
  s.edit(TIN_E[3], "");
  await s.idle();
  expect(s.valueOf(TIN_E[3])).toBe("");
  expect((s.draft() as { administrator_branch_code?: string } | null)?.administrator_branch_code ?? "").toBe("");
  expect(s.valueOf(TIN_E[0])).toBe("000");
  s.dispose();
});

test("PR8: typed TIN digits survive a save and a reopen, box for box", async () => {
  const target = { tin: DEMO.tin, formCode: "1801", year: 2025, period: 0, mode: "return" } as const;
  const s = await opened(target);
  s.edit(TIN_E[0], "987");
  s.edit(TIN_E[1], "654");
  s.edit(TIN_E[2], "321");
  s.edit(TIN_E[3], "00042");
  await s.idle();
  s.saveNow();
  await s.idle();
  s.dispose();
  const again = await opened(target);
  for (const [i, k] of TIN_E.entries()) {
    expect(again.valueOf(k)).toBe(["987", "654", "321", "00042"][i]);
  }
  const draft = again.draft() as { administrator_tin?: string; administrator_branch_code?: string } | null;
  expect(draft?.administrator_tin).toBe("987654321");
  expect(draft?.administrator_branch_code).toBe("00042");
  again.dispose();
});
