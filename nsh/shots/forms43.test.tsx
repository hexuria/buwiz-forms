// D8: the 43 forms, each opened as a Form library sample (year 0, period 0) for the demo taxpayer in a real window, 1440x900 light.
// Each form gets two shots, both in ~/BuwizNsh/evidence/forms43/: the top of the page (<code>-top.png) and the page after Expand all
// (<code>-all.png). A test's name starts with its batch (b1-.. b8-, six forms each), so one locked run is one batch:
//   just nsh-shots forms43 --filter b1-
// The codes and part counts are the door's list (screens/form/all43.test.ts prints them). This file cannot ask the door: a shot
// runs in the host, where ebirforms/native does not load, so the list is written here.
import { open, test } from "nsh/test";
import { shoot } from "./shoot";

const FORMS: [code: string, parts: number][] = [
  ["0605",4],
  ["0619E",5],
  ["0619F",5],
  ["1600PT",6],
  ["1600VT",6],
  ["1600WP",5],
  ["1601C",6],
  ["1601EQ",5],
  ["1601FQ",6],
  ["1602Q",7],
  ["1603Q",5],
  ["1604C",3],
  ["1604E",4],
  ["1604F",4],
  ["1606",8],
  ["1700",6],
  ["1701",17],
  ["1701A",8],
  ["1701MS",8],
  ["1701Q",9],
  ["1702EX",10],
  ["1702MX",17],
  ["1702Q",7],
  ["1702RT",12],
  ["1706",7],
  ["1707",8],
  ["1707A",6],
  ["1800",7],
  ["1801",6],
  ["2000",9],
  ["2000OT",5],
  ["2200A",6],
  ["2200AN",8],
  ["2200C",7],
  ["2200M",7],
  ["2200P",7],
  ["2200S",6],
  ["2200T",6],  // 6 not 7: the official paper is January 2020 (2200-T Jan 2020 ENCS Final version2.pdf, the only 2200T paper in the official offline package v7.9.6.2.1). It prints Schedule 1's rows for the rates effective 2020-2023 that the August 2022 sheet no longer printed, so what was the seventh part "Not on the printed sheet" is empty: all 87 of its keys now sit in Part V with print cells on the sheet.
  ["2550M",12],
  ["2550Q",6],
  ["2551Q",5],
  ["2552",8],
  ["2553",5],
];

FORMS.forEach(([code, parts], i) => {
  const batch = `b${Math.floor(i / 6) + 1}`;
  test(`${batch}-${code}`, async () => {
    const app = await open("../FormAnyShot.tsx", {
      width: 1440,
      height: 900,
      platform: "macos",
      colorScheme: "light",
      host: { params: { formCode: code, year: 0, period: 0, mode: "sample" } },
    });
    await app.waitFor("id:fp-parts-toggle", { timeout: 60_000 });
    await app.waitFor("text:Opening…", { state: "gone", timeout: 60_000 });
    await app.wait(600);
    await shoot(app, `${code}-top`);
    await app.press("id:fp-parts-toggle");
    await app.waitFor(`text:${parts} parts`, { timeout: 60_000 });
    await app.wait(800);
    await shoot(app, `${code}-all`);
  }, 180_000);
});
