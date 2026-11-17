// The calendar button beside a date printed in split boxes (old form_page.js pickDate): a day picked fills the parts that are open.
// Through the real door, against a scratch database and the demo taxpayer (Juana Santos).
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { dateRange } from "./rows";
import { dateParts } from "./schema";
import { FormSession, type FormTarget } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("1706 item 1 (month, day and year boxes): a picked day fills all three", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" });
  const id = s.itemIdForKey("frm1706:txtDateMonth")!;
  const date = dateParts(s.item(id)!.f || [])!;
  expect([date.month.k, date.day?.k, date.year.k]).toEqual(["frm1706:txtDateMonth", "frm1706:txtDateDay", "frm1706:txtDateYear"]);
  s.pickDate(date, "2025-03-15");
  expect(Number(s.valueOf("frm1706:txtDateMonth"))).toBe(3);
  expect(Number(s.valueOf("frm1706:txtDateDay"))).toBe(15);
  expect(Number(s.valueOf("frm1706:txtDateYear"))).toBe(2025);
  await s.idle();
  expect(Number(s.valueOf("frm1706:txtDateMonth"))).toBe(3);
  expect(Number(s.valueOf("frm1706:txtDateDay"))).toBe(15);
  s.dispose();
});

test("2551Q item 2 (a month list and a locked year): the pick sets the month by its listed value and leaves the year", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" });
  const id = s.itemIdForKey("frm2551Qv2018:rtnMonth")!;
  const date = dateParts(s.item(id)!.f || [])!;
  expect(date.day).toBe(null);
  expect(s.box("frm2551Qv2018:txtYear").locked()).toBe(true);
  // The year is locked: a locked part is skipped, so the pick fills the open ones (June, here of 2025). That is why the page's calendar
  // is limited to the locked year (dateRange, the next tests): 2024-06-15 is never offered there.
  s.pickDate(date, "2024-06-15");
  expect(s.valueOf("frm2551Qv2018:rtnMonth")).toBe("06");
  expect(s.valueOf("frm2551Qv2018:txtYear")).toBe("2025");
  await s.idle();
  s.dispose();
});

test("2551Q item 2: the calendar is limited to the locked year (min and max), and every day in that year fills the whole date", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" });
  const date = dateParts(s.item(s.itemIdForKey("frm2551Qv2018:rtnMonth")!)!.f || [])!;
  expect(dateRange(s, date)).toEqual({ min: "2025-01-01", max: "2025-12-31" });
  s.pickDate(date, "2025-03-31");
  expect(s.valueOf("frm2551Qv2018:rtnMonth")).toBe("03");
  expect(s.valueOf("frm2551Qv2018:txtYear")).toBe("2025");
  await s.idle();
  s.dispose();
});

test("0619E Due Date: a locked month and year limit the calendar to that month, the open day takes the pick", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "0619E", year: 2025, period: 4, mode: "return" });
  const date = dateParts(s.item(s.itemIdForKey("frm0619E:txtDueMonth")!)!.f || [])!;
  expect([s.valueOf(date.month.k), s.valueOf(date.year.k), s.box(date.month.k).locked(), s.box(date.year.k).locked(), s.box(date.day!.k).locked()]).toEqual(["05", "2025", true, true, false]);
  expect(dateRange(s, date)).toEqual({ min: "2025-05-01", max: "2025-05-31" });
  s.pickDate(date, "2025-05-17");
  expect(Number(s.valueOf(date.day!.k))).toBe(17);
  expect(s.valueOf(date.month.k)).toBe("05");
  await s.idle();
  s.dispose();
});

test("a month with a leap day: February of 2024 ends on the 29th; a year printed as 20YY (1702Q) holds two digits and still limits to 2025", async () => {
  const feb = await opened({ tin: DEMO.tin, formCode: "0619E", year: 2024, period: 1, mode: "return" });
  const due = dateParts(feb.item(feb.itemIdForKey("frm0619E:txtDueMonth")!)!.f || [])!;
  expect(feb.valueOf(due.month.k)).toBe("02");
  expect(dateRange(feb, due)).toEqual({ min: "2024-02-01", max: "2024-02-29" });
  feb.dispose();

  const q = await opened({ tin: DEMO.tin, formCode: "1702Q", year: 2025, period: 1, mode: "return" });
  const ended = dateParts(q.item(q.itemIdForKey("frm1702q:txtYrEndYear")!)!.f || [])!;
  expect([q.valueOf(ended.year.k), q.box(ended.year.k).locked(), q.box(ended.month.k).locked()]).toEqual(["25", true, false]);
  expect(dateRange(q, ended)).toEqual({ min: "2025-01-01", max: "2025-12-31" });
  q.dispose();
});

test("nothing locked: the calendar offers every day (1706 Date of Transaction)", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" });
  const date = dateParts(s.item(s.itemIdForKey("frm1706:txtDateMonth")!)!.f || [])!;
  expect(dateRange(s, date)).toEqual({});
  s.dispose();
});
