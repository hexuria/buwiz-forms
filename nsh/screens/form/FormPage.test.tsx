// The form page mounted on a 2551Q of the demo taxpayer, at phone size and at desktop size, against a scratch database.
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPagePreview.tsx";

test("phone: open, Check shows the to-dos on the pill, step to one, type, autosave", async () => {
  const app = await open(ENTRY); // 390 x 844
  await app.waitFor("text:BIR Form 2551Q");
  expect(app.texts()).toContain("Quarterly Percentage Tax Return");
  // Narrow: no side list, the pill floats.
  expect(app.count("id:fp-nav")).toBe(0);
  expect(app.shows("id:fp-pill")).toBe(true);
  // Untouched: no message text shows, but the pill counts the to-dos in red at once (F9-1: counts never wait for the text).
  await app.waitFor("id:fp-pill-count");
  expect(app.texts().some((t) => /^Choose the applicable Item 13/.test(t))).toBe(false);
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  const small = app.texts().find((t) => / to do · /.test(t));
  expect(small).toBeTruthy();
  // Step: the pill names the row it went to.
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 13 · /");
  expect(app.texts().some((t) => /^1 of \d+ to do/.test(t))).toBe(true);
  // Item 13 answered (Graduated): one fewer to do.
  const before = app.texts().find((t) => /^1 of \d+ to do/.test(t))!;
  const n = Number(/of (\d+)/.exec(before)![1]);
  await app.press("text:Graduated income tax rate");
  await app.waitUntil(() => !app.texts().some((t) => new RegExp("of " + n + " to do").test(t)));
  // Type an amount in Part II and see it saved.
  await app.press("id:fp-part-2-toggle");
  await app.type("id:fp-frm2551Qv2018:txt15", "100");
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
});

test("desktop: stepping with the pill puts the keyboard in a tick box, then in a money box", async () => {
  // A 1601C: its first to-do is a group of tick boxes (Item 3), and answering it brings the amounts up as to-dos.
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { form: "1601C", period: 3 } } });
  await app.waitFor("text:BIR Form 1601C");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 3 · /");
  // The keys are in the group's first box without a press on it: Space ticks it. The step puts the keyboard there after its scroll, a moment
  // after the pill names the item; a Space pressed before that goes nowhere (seen on the CI runner), so it is pressed again only while the
  // item is still unanswered.
  for (let i = 0; i < 8 && !app.shows("text:/^4 to do/"); i++) {
    await app.keyboard.press("space");
    await app.waitFor("text:/^4 to do/", { timeout: 1500 }).catch(() => undefined);
  }
  await app.waitFor("text:/^4 to do/");
  // Item 11 is another group; Item 14 is an amount: a typed key lands in its box.
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 11 · /");
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 14 · /");
  const box = "id:fp-frm1601c:txtTax14";
  expect(app.value(box) ?? "").not.toContain("5");
  // The same moment as above: the key is pressed again until the box has it.
  for (let i = 0; i < 8 && !(app.value(box) ?? "").includes("5"); i++) {
    await app.keyboard.press("5");
    await app.waitUntil(() => (app.value(box) ?? "").includes("5"), { timeout: 1500 }).catch(() => undefined);
  }
  expect(app.value(box) ?? "").toContain("5");
});

test("desktop: the side list, Expand all, Cmd+Right steps, the sections popover", async () => {
  // A manual clock: `app.scroll` drags 10 points per display frame, so on the system's clock the 3600 points down to the last part
  // take real time (8 s here, four times that on a slow runner: past the 30 s a test has). On the manual clock the drag runs as fast
  // as the machine computes. Nothing else in this test waits for a timer.
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", clock: {} });
  await app.waitFor("text:BIR Form 2551Q");
  expect(app.shows("id:fp-nav")).toBe(true);
  expect(app.texts()).toContain("On this form");
  expect(app.texts()).toContain("1 of 5");
  await app.press("id:fp-parts-toggle");
  await app.waitFor("text:5 parts");
  // The rows are a list in the page (F5): the last part's first row is drawn once the page scrolls to it.
  for (let i = 0; i < 12 && !app.shows("id:fp-item-4:0"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  expect(app.shows("id:fp-item-4:0")).toBe(true);
  await app.press("id:fp-nav-1");
  await app.waitFor("text:2 of 5");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.keyboard.press("mod+right");
  await app.waitFor("text:/^Item 13 · /");
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-sections");
  await app.press("id:fp-section-3");
  await app.waitFor("text:4 of 5");
});
