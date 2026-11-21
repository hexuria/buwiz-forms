// What the number boxes do with the clear gesture and with typing, headless: emptying a box reaches the page as one edit and reports
// "" for an empty box, and typing then starts from nothing. Selections and a real caret exist only in a real window: those cases
// (Cmd+A, a selected range, a click's caret, Tab) are in the shots lane, `just nsh-shots select` (shots/select.test.tsx).
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { readout } from "./harness";

const WIDE = { width: 800, height: 600, platform: "macos" as const };

const read = (app: App, key: string) => readout(app, key);

test("the clear gesture empties every number box and reports empty; typing then starts from nothing", async () => {
  const app = await open("./select.fixture.tsx", WIDE);
  for (const [id, key, typed, shown, reported] of [
    ["s-some", "Some:", "250", "250", "Some: 250"],
    ["s-zero", "Zero:", "75", "75", "Zero: 75"],
    ["s-rate", "Rate:", "7.5", "7.5", "Rate: 7.5"],
    ["s-count", "Count:", "93", "93", "Count: 93"],
    ["s-tin", "Tin:", "0001", "000-1", "Tin: 0001"],
  ] as const) {
    await app.clear(`id:${id}`);
    expect(app.value(`id:${id}`)).toBe("");
    expect(read(app, key)).toBe(`${key} empty`);
    await app.type(`id:${id}`, typed);
    expect(app.value(`id:${id}`)).toBe(shown);
    expect(read(app, key)).toBe(reported);
  }
}, 120_000);
