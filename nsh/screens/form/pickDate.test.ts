// The calendar button beside a date printed in split boxes (old form_page.js pickDate): a day picked fills the parts that are open.
// Through the real door, against a scratch database and the demo taxpayer (Juana Santos).
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer } from "../../bir/testing";
import { dateParts } from "./schema";
import { FormSession, type FormTarget } from "./session";

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
  s.pickDate(date, "2024-06-15");
  expect(s.valueOf("frm2551Qv2018:rtnMonth")).toBe("06");
  expect(s.valueOf("frm2551Qv2018:txtYear")).toBe("2025");
  await s.idle();
  s.dispose();
});
