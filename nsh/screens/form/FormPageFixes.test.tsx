// The F3 fixes of the wave 2 review that show on the page (FormPageAny.tsx, the demo taxpayer, a scratch database).
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

test("F3-4 desktop: an autosave drops the frame's cached reads (the saved returns re-read)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor("text:saved returns 0");
  await app.press("id:fp-parts-toggle");
  await app.clear("id:fp-frm2551Qv2018:txt15");
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

test("money round trip desktop: a focused zero amount is never rewritten while 15000.50 or 99999.99 is typed", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  for (const [period, amount] of [[4, "15000.50"], [1, "99999.99"]] as const) {
    const app = await open(ENTRY, {
      ...DESKTOP,
      host: { params: { formCode: "2551Q", year: 2025, period, probeKey: "frm2551Qv2018:txt15" } },
    });
    await app.waitFor("text:BIR Form 2551Q");
    await app.press("id:fp-nav-2");
    await app.waitFor(box);
    expect(app.value(box)).toBe("0.00");
    await app.type(box, ""); // focus: a zero amount becomes empty once, before the first key
    await app.waitUntil(() => app.value(box) === "", { timeout: 3000 });
    let expected = "";
    for (const ch of amount) {
      expected += ch;
      await app.type(box, ch);
      await app.wait(300); // let the host round trip answer between real separate keys
      expect(app.value(box)).toBe(expected);
    }
    await app.submit(box);
    await app.waitFor("text:✓ Saved", { timeout: 8000 });
    await app.waitUntil(() => app.texts().includes(`probe ${amount === "15000.50" ? "15,000.50" : "99,999.99"}`), { timeout: 5000 });
    expect(app.value(box)).toBe(amount === "15000.50" ? "15,000.50" : "99,999.99");
  }
}, 60_000);


test("money keyboard focus desktop: Tab away and Shift+Tab back into 1,234.56 sends select-all after the plain rewrite", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2024, period: 1, probeKey: "frm2551Qv2018:txt15", probeWrites: "frm2551Qv2018:txt15" } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.type(box, "");
  await app.waitUntil(() => app.value(box) === "", { timeout: 3000 });
  await app.type(box, "1234.56");
  await app.submit(box);
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  expect(app.value(box)).toBe("1,234.56");
  await app.type(box, "");
  await app.keyboard.press("tab");
  await app.wait(100);
  await app.keyboard.press("shift+tab");
  await app.wait(150);
  expect(app.value(box)).toBe("1234.56");
  // The headless input has no real selection store: typing always appends at the end. This page-level assertion is
  // therefore the write order the box receives: text rewrite first, then the explicit (0, 7) selection.
  expect(app.texts().find((t) => t.startsWith("writes "))?.endsWith("TC0:7")).toBe(true);
}, 30_000);

test("money keyboard focus desktop: Shift+Tab back into a zero amount selects its empty plain text", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2024, period: 2, probeWrites: "frm2551Qv2018:txt15" } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  expect(app.value(box)).toBe("0.00");
  await app.type(box, "");
  await app.keyboard.press("tab");
  await app.wait(100);
  await app.keyboard.press("shift+tab");
  await app.wait(150);
  expect(app.value(box)).toBe("");
  expect(app.texts().find((t) => t.startsWith("writes "))?.endsWith("TC0:0")).toBe(true);
}, 30_000);

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
  // While the box has the keys its emptied text is never rewritten; the host-held zero returns when it is left.
  await app.wait(300);
  expect(app.value(box)).toBe("");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "0.00", { timeout: 5000 });
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  expect(app.texts().some((t) => /must be an amount/.test(t))).toBe(false);
});
