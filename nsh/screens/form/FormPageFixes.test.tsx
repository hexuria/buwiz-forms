// The F3 fixes of the wave 2 review that show on the page (FormPageAny.tsx, the demo taxpayer, a scratch database).
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

test("F3-4 desktop: an autosave drops the frame's cached reads (the saved returns re-read)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor("text:saved returns 0");
  await app.press("id:fp-parts-toggle");
  await app.type("id:fp-frm2551Qv2018:txt15", "100");
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  await app.waitFor("text:saved returns 1", { timeout: 3000 });
});

test("F3-11 desktop: Cmd+Right inside a text box stays in the box; outside one it steps", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 2 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  // Jump to Part II so the step's current part is its own (the navigator sets it), then scroll txt15
  // into the mount window: Check shows every message, so the rows run taller.
  await app.press("id:fp-nav-2");
  for (let i = 0; i < 20 && !app.shows("id:fp-frm2551Qv2018:txt15"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  // Clear the prefilled 0.00 first: app.type appends, so "5" beside it reads 0.005 (>2 decimals) and flags Item 15.
  await app.clear(box);
  await app.type(box, "5");
  // Let the edit's validation land before stepping, so a stale row message cannot keep Item 15 on the to-do list.
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  await app.wait(800);
  await app.keyboard.press("mod+right");
  await app.wait(300);
  expect(app.texts().some((t) => /^Item \d+ · /.test(t))).toBe(false);
  // The return key leaves the box: now the shortcut steps.
  await app.submit(box);
  await app.keyboard.press("mod+right");
  await app.waitFor("text:/^Item 20 · /");
});

test("F7-1 desktop: emptying an amount box shows the host's zero, not the old amount, and the save line says Saved", async () => {
  // The amount is typed through the fixture (app.type lands beside a box's prefilled 0.00).
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 3, edits: { "frm2551Qv2018:txt15": "100" } } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  expect(app.value(box)).toContain("100");
  // Select-all and Backspace: the box reports "". The old amount does not come back, the zero does.
  await app.clear(box);
  await app.waitUntil(() => app.value(box) === "0.00", { timeout: 5000 });
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  expect(app.texts().some((t) => /must be an amount/.test(t))).toBe(false);
});
