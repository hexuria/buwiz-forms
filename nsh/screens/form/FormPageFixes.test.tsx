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
  // Exactly the typed value is what the return holds: item 18 (items 15 to 17, empty but for item 15) moves by it.
  expect(app.texts()).toContain("100.00");
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

test("money round trip desktop: a zero amount is its placeholder and the first key of 15000.50 or 99999.99 is the number", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  for (const [period, amount] of [[4, "15000.50"], [1, "99999.99"]] as const) {
    const app = await open(ENTRY, {
      ...DESKTOP,
      host: { params: { formCode: "2551Q", year: 2025, period, probeKey: "frm2551Qv2018:txt15" } },
    });
    await app.waitFor("text:BIR Form 2551Q");
    await app.press("id:fp-nav-2");
    await app.waitFor(box);
    expect(app.value(box)).toBe("");
    await app.type(box, ""); // focus writes nothing: the zero is only the placeholder
    expect(app.value(box)).toBe("");
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


test("zero placeholder desktop: a 2551Q zero amount types 5, saves 5.00, and clearing returns to the held zero", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 1, probeKey: "frm2551Qv2018:txt15" } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  expect(app.value(box)).toBe("");
  await app.type(box, "5");
  expect(app.value(box)).toBe("5");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "5.00" && app.texts().includes("probe 5.00"), { timeout: 8000 });
  await app.clear(box);
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "" && app.texts().includes("probe 0.00"), { timeout: 8000 });
}, 30_000);

test("money round trip desktop: focus on a filled amount keeps its grouped text and a digit with separators saves the right amount", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 2, probeKey: "frm2551Qv2018:txt15", edits: { "frm2551Qv2018:txt15": "1234.56" } } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.waitUntil(() => app.value(box) === "1,234.56", { timeout: 5000 });
  await app.type(box, ""); // focus only
  await app.wait(300);
  expect(app.value(box)).toBe("1,234.56");
  await app.type(box, "9");
  await app.wait(300); // the host round trip may answer before the assertion
  expect(app.value(box)).toBe("1,234.569");
  await app.submit(box);
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  await app.waitUntil(() => app.texts().includes("probe 1,234.57"), { timeout: 5000 });
  expect(app.value(box)).toBe("1,234.57");
}, 30_000);


test("money round trip desktop: twelve digits count without separators in a filled or replacement amount", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const filled = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 2, probeKey: "frm2551Qv2018:txt15", edits: { "frm2551Qv2018:txt15": "99999999.12" } } },
  });
  await filled.waitFor("text:BIR Form 2551Q");
  await filled.press("id:fp-nav-2");
  await filled.waitFor(box);
  await filled.waitUntil(() => filled.value(box) === "99,999,999.12", { timeout: 5000 });
  await filled.type(box, "99"); // ten digits already there, then two more inside the grouped text: twelve digits in all
  await filled.wait(500);
  expect(filled.value(box)).toBe("99,999,999.1299");
  await filled.submit(box);
  await filled.waitFor("text:✓ Saved", { timeout: 8000 });
  await filled.waitUntil(() => filled.texts().includes("probe 99,999,999.13"), { timeout: 5000 });

  const replacement = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 4, probeKey: "frm2551Qv2018:txt15" } },
  });
  await replacement.waitFor("text:BIR Form 2551Q");
  await replacement.press("id:fp-nav-2");
  await replacement.waitFor(box);
  await replacement.type(box, "");
  await replacement.waitUntil(() => replacement.value(box) === "", { timeout: 3000 });
  await replacement.type(box, "99999999.1299");
  await replacement.wait(500);
  expect(replacement.value(box)).toBe("99999999.1299");
  await replacement.submit(box);
  await replacement.waitFor("text:✓ Saved", { timeout: 8000 });
  await replacement.waitUntil(() => replacement.texts().includes("probe 99,999,999.13"), { timeout: 5000 });
}, 45_000);


test("money unreadable desktop: leaving 1500x keeps the old amount, says so, and the save goes through", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 3, probeKey: "frm2551Qv2018:txt15", edits: { "frm2551Qv2018:txt15": "100" } } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.waitUntil(() => app.value(box) === "100.00", { timeout: 5000 });
  await app.clear(box); // one focus: the amount it held then was 100.00
  await app.type(box, "1500x");
  expect(app.value(box)).toBe("1500x");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "100.00", { timeout: 5000 });
  // In a form the session draws the sentence under the box, with the item it belongs to.
  expect(app.texts()).toContain("Item 15: 1500x is not an amount. The box kept 100.00.");
  await app.waitUntil(() => app.texts().includes("probe 100.00"), { timeout: 5000 });
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
}, 30_000);

test("F7-1 desktop: emptying an amount box returns to the host zero's placeholder, not the old amount, and the save line says Saved", async () => {
  // The amount is typed through the fixture.
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
  await app.waitUntil(() => app.value(box) === "", { timeout: 5000 });
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  expect(app.texts().some((t) => /must be an amount/.test(t))).toBe(false);
});


test("V1 desktop: a nullable schedule-row amount left empty shows blank, not zero", async () => {
  const table = "fp-table-frm1600VT:txtAtcCd1";
  const base = "id:fp-frm1600VT:txtPg1P2TaxBase1";
  const row = "id:fp-row-frm1600VT:txtAtcCd1-0";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "1600VT", year: 2026, period: 1, probeKey: "frm1600VT:txtPg1P2TaxBase1" } } });
  await app.waitFor("text:BIR Form 1600VT");
  for (let i = 0; i < 30 && !app.shows("id:fp-part-1-toggle"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-part-1-toggle");
  for (let i = 0; i < 30 && !app.shows("id:fp-frm1600VT:rdoPg1I3TaxWithheldYes"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-frm1600VT:rdoPg1I3TaxWithheldYes");
  await app.press("id:fp-frm1600VT:rdoPg1I10CategoryofWthholdAgentPriv");
  for (let i = 0; i < 30 && !app.shows("id:fp-part-2-toggle"); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press("id:fp-part-2-toggle");
  for (let i = 0; i < 30 && !app.shows(`id:${table}-add`); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press(`id:${table}-add`);
  await app.waitFor("text:Row 13");
  await app.press("id:fp-frm1600VT:txtAtcCd1-trigger");
  await app.waitFor("text:/^WV012/", { timeout: 10_000 });
  await app.press("text:/^WV012/");
  await app.type(base, "1000");
  await app.waitUntil(() => app.enabled(`id:${table}-done`) === true, { timeout: 15_000 });
  await app.press(`id:${table}-done`);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });

  await app.press(row);
  await app.waitFor(base);
  expect(app.value(base)).toBe("1,000.00");
  await app.clear(base);
  await app.keyboard.press("tab");
  await app.wait(500);
  expect(app.value(base)).toBe("");
  await app.waitUntil(() => app.enabled(`id:${table}-done`) === true, { timeout: 15_000 });
  await app.press(`id:${table}-done`);
  await app.waitFor("text:✓ Saved", { timeout: 15_000 });
  await app.press(row);
  await app.waitFor(base);
  expect(app.value(base)).toBe("");
}, 60_000);

test("group 5 desktop: a decimal comma typed into a zero amount saves as a point amount", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1, probeKey: "frm2551Qv2018:txt15" } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.type(box, "");
  await app.waitUntil(() => app.value(box) === "", { timeout: 3000 });
  await app.type(box, "1500,50");
  expect(app.value(box)).toBe("1500,50");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "1,500.50", { timeout: 5000 });
  await app.waitUntil(() => app.texts().includes("probe 1,500.50"), { timeout: 5000 });
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
}, 30_000);

test("group 5 desktop: decimal-comma shapes read on leaving, while a three-digit comma group stays thousands", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  for (const [period, typed, shown] of [[3, "1500,5", "1,500.50"], [4, "1,50", "1.50"], [1, "1,500", "1,500.00"]] as const) {
    const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2026, period, probeKey: "frm2551Qv2018:txt15" } } });
    await app.waitFor("text:BIR Form 2551Q");
    await app.press("id:fp-nav-2");
    await app.waitFor(box);
    await app.type(box, "");
    await app.waitUntil(() => app.value(box) === "", { timeout: 3000 });
    await app.type(box, typed);
    expect(app.value(box)).toBe(typed);
    await app.submit(box);
    await app.waitUntil(() => app.value(box) === shown, { timeout: 5000 });
    await app.waitUntil(() => app.texts().includes(`probe ${shown}`), { timeout: 5000 });
  }
}, 60_000);

test("group 5 desktop: End and Backspaces inside our grouping still read as digits", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2026, period: 2, probeKey: "frm2551Qv2018:txt15", edits: { "frm2551Qv2018:txt15": "1234.56" } } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.waitUntil(() => app.value(box) === "1,234.56", { timeout: 5000 });
  await app.type(box, "");
  await app.keyboard.press("end");
  for (let i = 0; i < 4; i++) await app.keyboard.press("backspace");
  expect(app.value(box)).toBe("1,23");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "123.00", { timeout: 5000 });
  await app.waitUntil(() => app.texts().includes("probe 123.00"), { timeout: 5000 });
}, 30_000);

test("group 5 desktop: an unreadable comma amount is kept with the comma hint", async () => {
  const box = "id:fp-frm2551Qv2018:txt15";
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 2, probeKey: "frm2551Qv2018:txt15", edits: { "frm2551Qv2018:txt15": "100" } } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-nav-2");
  await app.waitFor(box);
  await app.waitUntil(() => app.value(box) === "100.00", { timeout: 5000 });
  await app.clear(box);
  await app.type(box, "1500,500");
  await app.submit(box);
  await app.waitUntil(() => app.value(box) === "100.00", { timeout: 5000 });
  expect(app.texts()).toContain("Item 15: 1500,500 is not an amount. The box kept 100.00. Use a point for centavos, and commas only between thousands.");
}, 30_000);
