// What the number boxes do with the selection, in the window a person uses. Emptying a box (Cmd+A and Backspace or Delete, a
// selected range and Backspace, a typed digit over a selection) reaches the page as one edit and reports "" for an empty box; a
// box of zero is already empty with its zero as the placeholder, so the first digit starts it; Tab and the box's own focus() select the
// whole text whatever it holds. Focused money text is not grouped; grouping returns when the box is left. Selections and a real caret exist only in a real window, so this file is in the shots lane:
// `just nsh-shots select` (nsh test --target macos). A headless run of it checks nothing, as every shots file's does; what a
// headless box can do (the clear gesture, typing) is in ui/fields.select.test.tsx.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { openKit, readout, SIZES } from "../ui/harness";

// Tall enough for every box of the fixture (s-tin and s-kept are its last rows): a box below the window cannot be typed into.
const WIDE = { width: 800, height: 1000, platform: "macos" as const };
const KIT = SIZES[1].options;
const FIXTURE = "../ui/select.fixture.tsx";

/** Whether the app runs in a real window (a screenshot is only taken there). */
async function real(app: App): Promise<boolean> {
  try {
    await app.screenshot();
    return true;
  } catch (error) {
    if (String(error).includes("needs a real platform")) return false;
    throw error;
  }
}

/** The box takes the keys (a click), then its whole text is replaced by `text`, typed key by key. */
async function fill(app: App, id: string, text: string) {
  await app.type(id, "");
  await app.keyboard.press("mod+a");
  for (const ch of text) await app.keyboard.press(ch);
}

const read = (app: App, key: string) => readout(app, key);

test("a real window: Cmd+A and Backspace, Cmd+A and Delete, Cmd+A and a digit, each leaves the box empty or replaced and reports it", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  // The box held 1,250.00; emptied, it reports empty and nothing of the old amount stays in the box's own text.
  await app.type("id:s-some", "");
  await app.keyboard.press("mod+a");
  await app.keyboard.press("backspace");
  expect(app.value("id:s-some")).toBe("");
  expect(read(app, "Some:")).toBe("Some: empty");
  await app.keyboard.press("2");
  await app.keyboard.press("5");
  await app.keyboard.press("0");
  expect(app.value("id:s-some")).toBe("250");
  expect(read(app, "Some:")).toBe("Some: 250");
  // Forward Delete over the whole selection.
  await app.keyboard.press("mod+a");
  await app.keyboard.press("delete");
  expect(app.value("id:s-some")).toBe("");
  expect(read(app, "Some:")).toBe("Some: empty");
  // A digit over the whole selection.
  await fill(app, "id:s-some", "1234");
  expect(app.value("id:s-some")).toBe("1234");
  await app.keyboard.press("mod+a");
  await app.keyboard.press("7");
  expect(app.value("id:s-some")).toBe("7");
  expect(read(app, "Some:")).toBe("Some: 7");
  // Percent, integer and TIN boxes: the same.
  for (const [id, key, start, typed, shown] of [
    ["s-rate", "Rate:", "12.5", "9", "9"],
    ["s-count", "Count:", "4821", "3", "3"],
    ["s-tin", "Tin:", "000123453", "0001", "000-1"],
  ] as const) {
    await fill(app, `id:${id}`, start);
    await app.keyboard.press("mod+a");
    await app.keyboard.press("backspace");
    expect(app.value(`id:${id}`)).toBe("");
    expect(read(app, key)).toBe(`${key} empty`);
    for (const ch of typed) await app.keyboard.press(ch);
    expect(app.value(`id:${id}`)).toBe(shown);
  }
}, 120_000);

test("a real window: a selected range that holds a separator, deleted or typed over", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  const select = async (from: number, count: number) => {
    await app.keyboard.press("mod+left");
    for (let i = 0; i < from; i++) await app.keyboard.press("right");
    for (let i = 0; i < count; i++) await app.keyboard.press("shift+right");
  };
  // The filled amount keeps its grouped text on focus: the first three characters of "1,250.00" are "1,2".
  await app.type("id:s-some", "");
  expect(app.value("id:s-some")).toBe("1,250.00");
  await select(0, 3);
  await app.keyboard.press("backspace");
  expect(app.value("id:s-some")).toBe("50.00");
  expect(read(app, "Some:")).toBe("Some: 50.00");
  // The same range typed over by a digit, in an amount grouped on its last leaving.
  await fill(app, "id:s-some", "1250000");
  await app.type("id:s-free", "");
  await app.type("id:s-some", "");
  expect(app.value("id:s-some")).toBe("1,250,000.00");
  await select(0, 3);
  await app.keyboard.press("9");
  expect(app.value("id:s-some")).toBe("950,000.00");
  expect(read(app, "Some:")).toBe("Some: 950000.00");
  // A range at the end that holds its separator, deleted forward.
  await fill(app, "id:s-some", "1250000");
  await app.type("id:s-free", "");
  await app.type("id:s-some", "");
  await app.keyboard.press("mod+right");
  for (let i = 0; i < 7; i++) await app.keyboard.press("shift+left");
  await app.keyboard.press("delete");
  expect(app.value("id:s-some")).toBe("1,250");
  expect(read(app, "Some:")).toBe("Some: 1250");
  // The caret stays where the range was: the digit typed next goes in there, with no live regrouping.
  await fill(app, "id:s-some", "1250");
  await app.keyboard.press("left");
  await app.keyboard.press("left");
  await app.keyboard.press("9");
  expect(app.value("id:s-some")).toBe("12950");
}, 120_000);

test("a real window: a box of zero is empty when a click gives it the keys, so the first digit starts it", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  expect(app.value("id:s-zero")).toBe("");
  await app.type("id:s-zero", "100");
  expect(app.value("id:s-zero")).toBe("100");
  expect(read(app, "Zero:")).toBe("Zero: 100");
  await app.type("id:s-zero", "1234"); // the box is not zero now: the click puts the caret, the digits go in beside it
  expect(read(app, "Zero:")).not.toBe("Zero: 100");
  await app.type("id:s-rate", "7.5");
  expect(app.value("id:s-rate")).toBe("7.5");
  expect(read(app, "Rate:")).toBe("Rate: 7.5");
  await app.type("id:s-count", "42");
  expect(app.value("id:s-count")).toBe("42");
  expect(read(app, "Count:")).toBe("Count: 42");
}, 120_000);

test("a real window: a click into an amount puts the caret where it landed; Tab and the box's own focus() select it whole", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  expect(app.value("id:s-some")).toBe("1,250.00");
  // The click lands left of the right-aligned figures: the caret is at the start, the 7 goes in front, nothing is replaced.
  await app.type("id:s-some", "7");
  expect(app.value("id:s-some")).toBe("71,250.00");
  // Tab from another box into it: the whole amount is selected, so the first digit replaces it.
  await fill(app, "id:s-some", "1250");
  await app.type("id:s-free", "");
  await app.keyboard.press("tab");
  await app.keyboard.press("7");
  expect(app.value("id:s-some")).toBe("7");
  expect(read(app, "Some:")).toBe("Some: 7");
  // The form's step is the box's own focus(): the same, for an amount and for a zero.
  await fill(app, "id:s-some", "1250");
  await app.type("id:s-free", "");
  await app.press("id:s-go-some");
  await app.keyboard.press("8");
  expect(app.value("id:s-some")).toBe("8");
  await app.type("id:s-free", "");
  await app.press("id:s-go-zero");
  await app.keyboard.press("5");
  expect(app.value("id:s-zero")).toBe("5");
}, 120_000);

test("a real window: after Tab, a caret the user moves later is not turned back into a selection", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  await fill(app, "id:s-some", "1250");
  await app.type("id:s-free", "");
  await app.keyboard.press("tab"); // into the filled amount, selected whole
  await app.wait(900);
  await app.keyboard.press("right"); // one arrow key collapses the selection at the end; End does not in a single-line macOS box
  await app.keyboard.press("9");
  expect(app.value("id:s-some")).toBe("1,250.009");
  expect(read(app, "Some:")).toBe("Some: 1250.009");
}, 120_000);

test("a real window: an outside refill waits for the box to be left, and typing supersedes it", async () => {
  const app = await open(FIXTURE, WIDE);
  if (!(await real(app))) return;
  await app.type("id:s-kept", "");
  await app.keyboard.press("mod+a");
  await app.keyboard.press("backspace");
  expect(app.value("id:s-kept")).toBe("");
  await app.wait(700); // the page put 1,250 back outside, but focused text is never rewritten
  expect(app.value("id:s-kept")).toBe("");
  await app.type("id:s-free", "");
  expect(app.value("id:s-kept")).toBe("1,250.00");
  expect(read(app, "Kept:")).toBe("Kept: 1250");

  const typed = await open(FIXTURE, WIDE);
  if (!(await real(typed))) return;
  await typed.type("id:s-kept", "");
  await typed.keyboard.press("mod+a");
  await typed.keyboard.press("backspace");
  await typed.keyboard.press("7");
  await typed.wait(800);
  expect(typed.value("id:s-kept")).toBe("7");
  expect(read(typed, "Kept:")).toBe("Kept: 7");
}, 120_000);

test("a real window on the kit gallery: a date and the kit's amounts empty and refill", async () => {
  const app = await openKit("../BasePreview.tsx", KIT);
  if (!(await real(app))) return;
  await app.type("id:kit-date", "");
  await app.keyboard.press("mod+a");
  await app.keyboard.press("backspace");
  expect(app.value("id:kit-date")).toBe("");
  expect(readout(app, "Stored:")).toBe("Stored: empty");
  for (const ch of "12102020") await app.keyboard.press(ch);
  expect(readout(app, "Stored:")).toBe("Stored: 12/10/2020");
  await app.type("id:kit-money", "");
  await app.keyboard.press("mod+a");
  await app.keyboard.press("backspace");
  expect(app.value("id:kit-money")).toBe("");
  expect(readout(app, "Value:")).toBe("Value: empty");
  for (const ch of "250") await app.keyboard.press(ch);
  expect(app.value("id:kit-money")).toBe("250");
  expect(readout(app, "Value:")).toBe("Value: 250");
}, 120_000);
