// A month chosen from a year-end date's list while Calendar is ticked (F15). The calendar popup already offers December only. The month LIST
// offers all twelve, and the forms differ in what they do with another one: 1702Q's fill puts December back without a word, 2551Q keeps the
// month and flags it on the row ("Calendar filers must use December as the year-end month"), 2550Q keeps it and says nothing (the official
// 2550Q program has no such rule). The page says why only where the fill put the month back, in the calendar pick's own sentence.
// Through the real door (scratch database, the demo taxpayer).
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const SAYS = "A calendar year ends in December. Choose Fiscal to pick another month.";

// The For Calendar / Fiscal pair and the Year Ended month list that follows it.
const YEAR_END: { code: string; calendar: string; fiscal: string; month: string }[] = [
  { code: "1702Q", calendar: "frm1702q:rbForClndrFscl_1", fiscal: "frm1702q:rbForClndrFscl_2", month: "frm1702q:rbYrEndMonth" },
  { code: "2551Q", calendar: "frm2551Qv2018:forThe_1", fiscal: "frm2551Qv2018:forThe_2", month: "frm2551Qv2018:rtnMonth" },
  { code: "2550Q", calendar: "frm2550qv2024:calendarNo1", fiscal: "frm2550qv2024:fiscalNo1", month: "frm2550qv2024:selectedMonthNo2" },
];

async function opened(code: string) {
  createDemoTaxpayer();
  const said: string[] = [];
  const target: FormTarget = { tin: DEMO.tin, formCode: code, year: 2025, period: 3, mode: "return" };
  const s = new FormSession(target, { toast: (message) => (said.push(message), `t${said.length}`) });
  expect(await s.open()).toBe(null);
  await s.idle();
  return { s, said };
}

/** Ticks a radio of the For Calendar / Fiscal pair. The host keeps the basis per taxpayer and year, so a test says which it wants. */
async function tick(s: FormSession, key: string): Promise<void> {
  s.pickRadio(key);
  await s.idle();
}

test("1702Q, Calendar ticked: a month chosen from the list is put back by the fill, and the page says why", async () => {
  const f = YEAR_END[0];
  const { s, said } = await opened(f.code);
  try {
    await tick(s, f.calendar);
    expect(s.valueOf(f.month)).toBe("12");
    said.length = 0;
    s.edit(f.month, "06");
    await s.idle();
    expect(said).toEqual([SAYS]);
    expect(s.valueOf(f.month)).toBe("12");
    // December is what the form holds: choosing it says nothing.
    said.length = 0;
    s.edit(f.month, "12");
    await s.idle();
    expect(said).toEqual([]);
    expect(s.valueOf(f.month)).toBe("12");
  } finally {
    s.dispose();
  }
});

test("1702Q, Fiscal ticked: another month is taken and nothing is said", async () => {
  const f = YEAR_END[0];
  const { s, said } = await opened(f.code);
  try {
    await tick(s, f.fiscal);
    said.length = 0;
    s.edit(f.month, "06");
    await s.idle();
    expect(said).toEqual([]);
    expect(s.valueOf(f.month)).toBe("06");
  } finally {
    s.dispose();
  }
});

test("2551Q, Calendar ticked: the month stays and the host's own message is on the page, so the page adds no sentence", async () => {
  const f = YEAR_END[1];
  const { s, said } = await opened(f.code);
  try {
    await tick(s, f.calendar);
    said.length = 0;
    s.edit(f.month, "06");
    await s.idle();
    expect(s.valueOf(f.month)).toBe("06");
    expect(said).toEqual([]);
    expect(s.errors().some((e) => /Calendar filers must use December/.test(e.message))).toBe(true);
  } finally {
    s.dispose();
  }
});

test("2550Q, Calendar ticked: the month stays and nothing is said (the form accepts it; nothing is refused here)", async () => {
  const f = YEAR_END[2];
  const { s, said } = await opened(f.code);
  try {
    await tick(s, f.calendar);
    said.length = 0;
    s.edit(f.month, "06");
    await s.idle();
    expect(s.valueOf(f.month)).toBe("06");
    expect(said).toEqual([]);
    expect(s.errors().some((e) => /December/.test(e.message))).toBe(false);
  } finally {
    s.dispose();
  }
});
