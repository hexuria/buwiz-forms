// Job E by hand: the counter appears once typed, warns past the boxes, never blocks.
import { expect, open, test } from "nsh/test";

async function open2551Q(width: number, platform: "ios" | "macos") {
  const app = await open("./FormPageAny.tsx", {
    width,
    height: platform === "ios" ? 844 : 900,
    platform,
    host: { params: { formCode: "2551Q", year: 2025, period: 1, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 2551Q", { timeout: 20_000 });
  await app.wait(1500);
  return app;
}

test("phone: the sheets box counts what is typed against its two printed boxes", async () => {
  const app = await open2551Q(390, "ios");
  // The box is pre-filled with 1: the counter reads "1 of 2 characters".
  expect(app.texts().some((t) => t === "1 of 2 characters")).toBe(true);
}, 60_000);

test("desktop: past the boxes the line says it prints as a plain line, and the official maximum caps typing", async () => {
  // The sheets box's official maximum is its paper count (2): typing more never lands.
  const app = await open2551Q(1440, "macos");
  await app.type("id:fp-frm2551Qv2018:txtSheets", "98765");
  await app.wait(600);
  const digits = (app.value("id:fp-frm2551Qv2018:txtSheets") ?? "").replace(/\D/g, "");
  expect(digits.length).toBeLessThanOrEqual(2);
  // A value past the boxes (arriving seeded, as an import or a carried draft
  // brings it) shows the plain-line notice; the sheets box prints in 2.
  const wide = await open("./FormPageAny.tsx", {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "2551Q", year: 2025, period: 1, openParts: "all", setup: [{ edits: { "frm2551Qv2018:txtSheets": "123" } }] } },
  });
  await wide.waitFor("text:BIR Form 2551Q", { timeout: 20_000 });
  await wide.wait(1200);
  const line = wide.texts().find((t) => t.includes("of 2 characters"));
  expect(line).toBeDefined();
  expect(line!).toContain("3 of 2 characters. This will print as a plain line.");
}, 60_000);

test("desktop: an amount box warns only past the integer digits", async () => {
  // Typing digit runs into a money mask is unreliable in the harness (fast keystrokes reconcile as
  // cents); the box is filled through the session's own edit path instead — same value the input writes.
  const small = await open("./FormPageAny.tsx", {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "2551Q", year: 2025, period: 1, openParts: "all", setup: [{ edits: { "frm2551Qv2018:txt15": "1000.00" } }] } },
  });
  await small.waitFor("text:BIR Form 2551Q", { timeout: 20_000 });
  await small.wait(1200);
  expect(small.texts().some((t) => t.includes("Too long for the boxes on paper"))).toBe(false);
  const big = await open("./FormPageAny.tsx", {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: "2551Q", year: 2025, period: 1, openParts: "all", setup: [{ edits: { "frm2551Qv2018:txt15": "99999999999999.99" } }] } },
  });
  await big.waitFor("text:BIR Form 2551Q", { timeout: 20_000 });
  await big.wait(1200);
  expect(
    big.texts().some((t) => t.includes("Too long for the boxes on paper. This will print as a plain number.")),
  ).toBe(true);
}, 60_000);
