// What the number boxes do with focus and typing, headless. Selections and a real caret exist only in a real window:
// those cases (Cmd+A, a selected range, a click's caret, Tab) are in the shots lane, `just nsh-shots select`.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { readout } from "./harness";

const WIDE = { width: 800, height: 600, platform: "macos" as const };
const read = (app: App, key: string) => readout(app, key);

test("the clear gesture empties a nonzero box and reports empty; typing then starts from nothing", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  await app.clear("id:s-some");
  expect(app.value("id:s-some")).toBe("");
  expect(read(app, "Some:")).toBe("Some: empty");
  await app.type("id:s-some", "250");
  expect(app.value("id:s-some")).toBe("250");
  expect(read(app, "Some:")).toBe("Some: 250");
});

test("an unfocused zero money, percent and count box is empty with the zero placeholder; blank and code boxes differ", async () => {
  const app = await open("./select.fixture.tsx", { ...WIDE, height: 1000 });
  expect(app.value("id:s-zero")).toBe("");
  expect(app.value("id:s-rate")).toBe("");
  expect(app.value("id:s-count")).toBe("");
  expect(app.value("id:s-whole-zero")).toBe("");
  expect(app.value("id:s-blank")).toBe("");
  expect(app.value("id:s-code")).toBe("000");
  // The harness has no placeholder accessor. Its text selector also matches an input's placeholder: with every zero
  // box's text empty here, these counts can only be the placeholders (money 0.00; percent and count 0).
  expect(app.count("text:0.00")).toBe(1);
  expect(app.count("text:0")).toBe(3);
  await app.press("id:s-show-readonly-zeros");
  expect(app.value("id:s-zero-disabled")).toBe("0.00");
  expect(app.value("id:s-zero-locked")).toBe("0.00");
});

test("focus writes nothing and reports nothing in an untouched zero box", async () => {
  const app = await open("./select.fixture.tsx", { ...WIDE, clock: {} });
  await app.press("id:s-go-zero");
  await app.clock?.runFor(50);
  expect(app.value("id:s-zero")).toBe("");
  expect(read(app, "Zwrites:")).toBe("Zwrites: none");
  expect(read(app, "Zreports:")).toBe("Zreports: 0");
  expect(app.count("text:0.00")).toBe(1); // focused, still untouched: the zero placeholder stays visible under the caret
  await app.type("id:s-free", "x");
  expect(app.value("id:s-zero")).toBe("");
  expect(read(app, "Zwrites:")).toBe("Zwrites: none");
  expect(read(app, "Zreports:")).toBe("Zreports: 0");
});

test("the first key is the number in money, percent and count on focus and Tab entry", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  for (const [before, id, key] of [
    ["s-some", "s-zero", "Zero:"],
    ["s-zero", "s-rate", "Rate:"],
    ["s-rate", "s-count", "Count:"],
  ] as const) {
    await app.type(`id:${before}`, "");
    await app.keyboard.press("tab");
    await app.keyboard.press("4");
    expect(app.value(`id:${id}`)).toBe("4");
    await app.keyboard.press("2");
    expect(app.value(`id:${id}`)).toBe("42");
    expect(read(app, key)).toBe(`${key} 42`);
  }
});

test("the macOS same-turn order and the Windows 0 ms click burst read the first input as the number", async () => {
  for (const clock of [true, false]) {
    const app = await open("./select.fixture.tsx", clock ? { ...WIDE, clock: {} } : WIDE);
    for (const [button, id, key] of [
      ["s-zero-same", "s-zero", "Zero:"],
      ["s-rate-same", "s-rate", "Rate:"],
      ["s-count-same", "s-count", "Count:"],
    ] as const) {
      await app.press(`id:${button}`);
      await app.clock?.runFor(30);
      await app.waitUntil(() => readout(app, key) === `${key} 15`);
      expect(app.value(`id:${id}`)).toBe("15");
    }
    await app.dispose();
  }
});

test("the iPad first digit at 120, 200 and 400 ms after focus is the number", async () => {
  for (const ms of [120, 200, 400]) {
    const app = await open("./select.fixture.tsx", { ...WIDE, clock: {} });
    await app.type("id:s-zero", "");
    await app.clock?.runFor(ms);
    await app.keyboard.press("1");
    expect(app.value("id:s-zero")).toBe("1");
    expect(read(app, "Zero:")).toBe("Zero: 1");
    await app.dispose();
  }
});

test("input delivered before the focus callback reads as the number in money, percent and count", async () => {
  for (const clock of [true, false]) {
    const app = await open("./select.fixture.tsx", clock ? { ...WIDE, clock: {} } : WIDE);
    for (const [button, key] of [
      ["s-zero-input-first", "Zero:"],
      ["s-rate-input-first", "Rate:"],
      ["s-count-input-first", "Count:"],
    ] as const) {
      await app.press(`id:${button}`);
      await app.clock?.runFor(30);
      await app.waitUntil(() => readout(app, key) === `${key} 15`);
    }
    await app.type("id:s-free", "x");
    await app.clock?.runFor(30);
    expect(app.value("id:s-zero")).toBe("15.00");
    expect(read(app, "Zero:")).toBe("Zero: 15");
    await app.dispose();
  }
});

test("a typed zero left in a zero box returns to the placeholder without a new report", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  await app.type("id:s-zero", "0.00");
  await app.type("id:s-free", "x");
  await app.wait(50);
  expect(app.value("id:s-zero")).toBe("");
  expect(read(app, "Zero:")).toBe("Zero: 0");
  expect(read(app, "Zreports:")).toBe("Zreports: 0");
});

test("unreadable text left in a zero box returns to the placeholder with the ordinary sentence", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  await app.type("id:s-zero", "1500x");
  await app.type("id:s-free", "x");
  await app.wait(50);
  expect(app.value("id:s-zero")).toBe("");
  expect(app.texts()).toContain("1500x is not an amount. The box kept 0.00.");
  expect(read(app, "Zero:")).toBe("Zero: 0");
});

test("outside values move an unfocused zero box from placeholder to amount and back", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  expect(app.value("id:s-zero")).toBe("");
  await app.press("id:s-zero-1250");
  expect(app.value("id:s-zero")).toBe("1,250.00");
  await app.press("id:s-zero-back-0");
  expect(app.value("id:s-zero")).toBe("");
  expect(app.count("text:0.00")).toBe(1);
  await app.press("id:s-zero-blank");
  expect(app.value("id:s-zero")).toBe("");
  expect(app.count("text:0.00")).toBe(0);
});

test("a percent typed into a zero stays shown after leaving, matching what it reported", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  await app.type("id:s-rate", "42");
  await app.type("id:s-free", "x");
  await app.wait(100);
  expect(readout(app, "Rate:")).toBe("Rate: 42");
  expect(app.value("id:s-rate")).toBe("42");
});
