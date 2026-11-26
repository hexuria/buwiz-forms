// F2 by hand: the misleading "Can't be filled on this page yet." line is gone from the tables
// whose columns are computed or preprinted (1600WP, 1601FQ Schedule 1, 1701 NOLCO, 2000 Schedule 1,
// 2553 rows 14-18). Opens each form with every part expanded and scans the visible texts.
import { expect, open, test } from "nsh/test";

async function notYetLines(formCode: string, year: number, period: number): Promise<string[]> {
  const app = await open("./FormPageAny.tsx", {
    host: { params: { formCode, year, period, openParts: "all" } },
  });
  await app.waitFor(`text:BIR Form ${formCode}`, { timeout: 20_000 });
  await app.wait(2500);
  return app.texts().filter((t) => t.includes("Can't be filled on this page yet."));
}

test("phone: 2553 shows no not-yet line on rows 14 to 18", async () => {
  const lines = await notYetLines("2553", 2025, 1);
  console.log("2553 not-yet lines:", lines.length);
  expect(lines).toEqual([]);
}, 60_000);

test("phone: 1600WP table columns carry no not-yet line", async () => {
  const lines = await notYetLines("1600WP", 2025, 1);
  console.log("1600WP not-yet lines:", lines.length);
  expect(lines).toEqual([]);
}, 60_000);

test("phone: 1601FQ Schedule 1 carries no not-yet line", async () => {
  const lines = await notYetLines("1601FQ", 2025, 1);
  console.log("1601FQ not-yet lines:", lines.length);
  expect(lines).toEqual([]);
}, 60_000);

test("phone: 2000 Schedule 1 carries no not-yet line", async () => {
  const lines = await notYetLines("2000", 2025, 6);
  console.log("2000 not-yet lines:", lines.length);
  expect(lines).toEqual([]);
}, 60_000);

test("phone: 1701 NOLCO column E carries no not-yet line", async () => {
  const lines = await notYetLines("1701", 2025, 1);
  console.log("1701 not-yet lines:", lines.length);
  expect(lines).toEqual([]);
}, 60_000);
