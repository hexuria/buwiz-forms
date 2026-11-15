// The palette's index of forms is built when the palette opens, not by a typed letter: the first keystroke used to hold the list
// unfiltered for about a second (a debug build makes the whole form library then) while later ones took milliseconds.
// PaletteDemo.tsx counts the builds (bir.screen_forms_index) and shows them as "builds: N".
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "../PaletteDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

const builds = (app: App): number => {
  const text = app.texts().find((t) => t.startsWith("builds:"));
  return Number(/builds: (\d+)/.exec(text ?? "")?.[1] ?? -1);
};

/** Opens the palette the way each size does: Cmd+K on a desktop, the search tab on a phone. */
async function openPalette(app: App, phone: boolean) {
  if (phone) await app.press("id:nb-search");
  else await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
}

for (const [name, options, phone] of [
  ["desktop", WIDE, false],
  ["phone", {}, true],
] as const) {
  test(`${name}: the forms index is built as the palette opens and not again by any letter`, async () => {
    const app = await open(ENTRY, options);
    await app.waitFor(phone ? "id:nb-search" : "id:sidebar");
    await openPalette(app, phone);
    // Built by the open: the settled count, before a letter is typed.
    await app.waitUntil(() => builds(app) >= 1);
    await app.wait(100);
    const atOpen = builds(app);
    expect(atOpen).toBeGreaterThanOrEqual(1);
    // The first letter and the rest narrow the list to the form, and none of them builds anything.
    await app.type("id:palette-input", "2");
    await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
    await app.wait(100);
    expect(builds(app)).toBe(atOpen);
    await app.type("id:palette-input", "551");
    await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
    expect(app.shows("text:1601C · Monthly Remittance Return of Income Taxes Withheld on Compensation")).toBe(false);
    await app.clear("id:palette-input");
    await app.type("id:palette-input", "monthly");
    await app.waitFor("text:/^1601C · /");
    await app.wait(100);
    expect(builds(app)).toBe(atOpen);
  });

  test(`${name}: opening the palette again, over a store a save emptied, builds at most once more and never per letter`, async () => {
    const app = await open(ENTRY, options);
    await app.waitFor(phone ? "id:nb-search" : "id:sidebar");
    await openPalette(app, phone);
    await app.waitUntil(() => builds(app) >= 1);
    await app.type("id:palette-input", "2551");
    await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
    await app.wait(100);
    const first = builds(app);
    await app.keyboard.press("escape");
    await app.waitFor("id:palette-input", { state: "gone" });
    await app.keyboard.press("mod+shift+9"); // what a save does: every cached read is dropped
    await app.wait(100);
    const dropped = builds(app);
    await openPalette(app, phone);
    await app.wait(100);
    const second = builds(app);
    expect(second - dropped).toBeLessThanOrEqual(1);
    expect(second).toBeGreaterThanOrEqual(first);
    await app.type("id:palette-input", "1601");
    await app.waitFor("text:/^1601C · /");
    await app.wait(100);
    expect(builds(app)).toBe(second);
    await app.type("id:palette-input", "x");
    await app.waitFor("text:No matches. Try a form code like 2550Q.");
    expect(builds(app)).toBe(second);
  });
}
