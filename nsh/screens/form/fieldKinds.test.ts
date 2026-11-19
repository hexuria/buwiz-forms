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
import { dateParts } from "./schema";
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
  // The row says the host's own required sentence for the date.
  const row = s.itemIdForKey(day);
  expect(row).toBeTruthy();
  expect(s.messageOf(row!)).toContain("Item 2");
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
