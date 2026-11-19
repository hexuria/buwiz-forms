// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { expect, open, test } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const MONTH = "frm0605:txtDueDateMonth";
const box = `id:fp-${MONTH}`;

// Typing shows at once; the host's answer lands after the 300 ms flush. The
// wait covers the flush and the fill's round trip, then the box must still
// hold the digits — a refused write puts the shown value back.
async function typeAndHold(app: Awaited<ReturnType<typeof open>>) {
  await app.waitFor(box);
  await app.type(box, "08");
  await app.waitUntil(() => (app.value(box) ?? "") === "08", { timeout: 5000 });
  await new Promise((resolve) => setTimeout(resolve, 900));
  expect(app.value(box) ?? "").toBe("08");
  expect(app.texts().some((t) => t.includes("draft has no"))).toBe(false);
}

test("phone: a new 0605's Item 4 month box keeps a typed 08", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } } });
  await typeAndHold(app);
});

test("desktop: a new 0605's Item 4 month box keeps a typed 08", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { formCode: "0605", year: 2023, period: 2, mode: "return" } } });
  await typeAndHold(app);
});

// B21: the header's "N fields" chip counts the drawn boxes (the session's
// fieldCount), not the schema entry's raw `n`.
async function chipCountsDrawn(app: Awaited<ReturnType<typeof open>>) {
  await app.waitFor(box);
  await app.waitUntil(() => app.texts().some((t) => /^session fields \d+$/.test(t)), { timeout: 5000 });
  const drawn = /^session fields (\d+)$/.exec(app.texts().find((t) => /^session fields \d+$/.test(t))!)![1];
  const chip = app.texts().map((t) => /^(\d+) fields$/.exec(t)?.[1]).find((n) => n !== undefined);
  expect(chip).toBe(drawn);
  const raw = (bir.screen_form_schema("0605") as { n?: number }).n;
  expect(raw).toBeTruthy();
  expect(chip).not.toBe(String(raw));
}

test("phone: the header chip counts the drawn boxes, not the raw schema count", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } } });
  await chipCountsDrawn(app);
});

test("desktop: the header chip counts the drawn boxes, not the raw schema count", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { formCode: "0605", year: 2023, period: 2, mode: "return" } } });
  await chipCountsDrawn(app);
});
