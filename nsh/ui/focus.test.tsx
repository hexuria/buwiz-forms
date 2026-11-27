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

test("desktop: programmatic focus on a filled amount writes no text and selects it whole", async () => {
  const app = await open(ENTRY, WIDE);
  expect(app.value("id:f-filled")).toBe("1,234.56");
  const textWrites = readout(app, "Text writes:");
  const caretWrites = readout(app, "Caret writes:");
  await app.press("id:go-filled");
  await app.wait(200);
  expect(app.value("id:f-filled")).toBe("1,234.56");
  expect(readout(app, "Text writes:")).toBe(textWrites);
  expect(readout(app, "Caret writes:")).not.toBe(caretWrites);
  expect(readout(app, "Caret range:")).toBe("Caret range: 0:8");
  // Headless typing always goes at the end and does not model a native selection; the selection operation itself is
  // the cross-platform contract here, and real keys prove the replacement.
});

test("desktop: a repeated focus without a blur does not rewrite, reselect or renotify a filled amount", async () => {
  const app = await open(ENTRY, WIDE);
  await app.press("id:go-filled-twice");
  await app.wait(100);
  expect(app.value("id:f-filled")).toBe("1,234.56");
  expect(readout(app, "Text writes:")).toBe("Text writes: 0");
  expect(readout(app, "Caret writes:")).toBe("Caret writes: 1");
  expect(readout(app, "Focus events:")).toBe("Focus events: 1");
});

test("desktop: programmatic focus does not select a plain text box, but selects a money box whole", async () => {
  const app = await open(ENTRY, WIDE);
  await app.press("id:go-plain");
  await app.wait(100);
  expect(readout(app, "Plain selection:")).not.toBe("Plain selection: 0:15");
  await app.press("id:go-filled");
  await app.wait(100);
  expect(readout(app, "Caret range:")).toBe("Caret range: 0:8");
});
