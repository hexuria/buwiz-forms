// The refs of the kit's boxes and choices: what a form page's pill step uses to put the keyboard in a box (macOS: ViewRef.focus()).
// The fixture's buttons call it; the next key goes to the box that has the keyboard, so the readouts show who got it.
import { expect, open, test } from "nsh/test";
import { readout } from "./harness";

const ENTRY = "./focus.fixture.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("desktop: money, percent, integer, TIN and date boxes take the keyboard from their ref", async () => {
  const app = await open(ENTRY, WIDE);
  for (const [go, key, shown] of [
    ["go-money", "Money:", "Money: 5"],
    ["go-pct", "Pct:", "Pct: 5"],
    ["go-int", "Int:", "Int: 5"],
    ["go-tin", "Tin:", "Tin: 5"],
    ["go-date", "Date:", "Date: 5"],
  ] as const) {
    await app.press(`id:${go}`);
    await app.keyboard.press("5");
    await app.waitUntil(() => readout(app, key) === shown);
  }
});

test("desktop: a checkbox and a dropdown take the keyboard from their ref", async () => {
  const app = await open(ENTRY, WIDE);
  await app.press("id:go-box");
  await app.keyboard.press("space");
  await app.waitUntil(() => readout(app, "Ticked:") === "Ticked: true");
  await app.press("id:go-drop");
  await app.keyboard.press("enter");
  await app.waitFor("text:Bravo");
  expect(app.shows("text:Alpha")).toBe(true);
});

test("desktop: programmatic focus into a filled amount sends select-all after the plain-text rewrite", async () => {
  const app = await open(ENTRY, WIDE);
  await app.type("id:f-money", "1234.56");
  await app.type("id:f-int", "1");
  expect(app.value("id:f-money")).toBe("1,234.56");
  await app.press("id:go-money");
  await app.wait(100);
  expect(app.value("id:f-money")).toBe("1234.56");
  expect(readout(app, "Money caret:")).toContain("0:7");
  expect(readout(app, "Money ops:")?.endsWith("TC")).toBe(true);
});
