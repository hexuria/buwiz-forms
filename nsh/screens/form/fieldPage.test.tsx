// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { expect, open, test } from "nsh/test";
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
