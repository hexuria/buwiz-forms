// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { expect, open, test } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const MONTH = "frm0605:txtDueDateMonth";
const box = `id:fp-${MONTH}#date`;

for (const width of [390, 640, 1440]) {
  test(`PR 2: 0605 Item 4 is one date control at ${width} points`, async () => {
    const app = await open(ENTRY, {
      width, height: 900, platform: width < 600 ? "ios" : "macos",
      host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-item-0:3");
    expect(app.shows(`id:fp-${MONTH}#date`)).toBe(true);
    expect(app.shows(`id:fp-${MONTH}`)).toBe(false);
    expect(app.shows("id:fp-frm0605:txtDueDateDay")).toBe(false);
    expect(app.shows("id:fp-frm0605:txtDueDateYear")).toBe(false);
  });
}

test("PR 2: phone date row opens its sheet and Done stores the chosen date", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } } });
  await app.waitFor("id:fp-item-0:3");
  const id = `fp-${MONTH}#date`;
  expect(app.shows(`id:${id}`)).toBe(true);
  await app.press(`id:${id}`);
  await app.waitFor(`id:${id}-sheet-input`);
  await app.type(`id:${id}-sheet-input`, "08172023");
  await app.press(`id:${id}-sheet-done`);
  await app.waitFor(`id:${id}-sheet-input`, { state: "gone" });
  expect(app.texts()).toContain("August 17, 2023");
});

test("PR 2: a desktop date refusal is printed once beside the one field", async () => {
  const app = await open(ENTRY, {
    width: 1440, height: 900, platform: "macos",
    host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
  });
  await app.waitFor(box);
  await app.type(box, "08/17/88");
  await app.type("id:fp-frm0605:txtNoOfSheets", "");
  await app.waitFor(`id:fp-boxmsg-${MONTH}`);
  expect(app.texts().filter((text) => text === "Item 4: type the four-digit year, for example 08/17/1988.").length).toBe(1);
});

test("PR 2: a phone date row includes its required-date error in its accessible name", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } } });
  await app.waitFor(box);
  await app.press("id:fp-check");
  await app.waitFor("text:Item 4 needs a due date.");
  expect(app.shows("label:Due Date, not set, Item 4 needs a due date.")).toBe(true);
});

for (const width of [390, 1280]) {
  test(`PR 2: a locked one-date hint is shown whole at ${width} points`, async () => {
    const app = await open(ENTRY, {
      width, height: 900, platform: width < 600 ? "ios" : "macos",
      host: { params: { formCode: "2552", year: 2025, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-frm2552:txtPg1I1Month#date");
    expect(app.shows("id:fp-frm2552:txtPg1I1Month#date-hint")).toBe(true);
    expect(app.texts()).toContain("Set by the date of transaction you start this return with (New return).");
  });
}

// Typing shows at once; the host's answer lands after the 300 ms flush. The
// wait covers the flush and the fill's round trip, then the box must still
// hold the digits — a refused write puts the shown value back.
async function typeAndHold(app: Awaited<ReturnType<typeof open>>, phone: boolean) {
  await app.waitFor(box);
  if (phone) {
    await app.press(box);
    await app.type(`${box}-sheet-input`, "08172023");
    await app.press(`${box}-sheet-done`);
    await app.waitFor(`${box}-sheet-input`, { state: "gone" });
    expect(app.texts()).toContain("August 17, 2023");
  } else {
    await app.type(box, "08172023");
    await app.waitUntil(() => (app.value(box) ?? "") === "08/17/2023", { timeout: 5000 });
    await new Promise((resolve) => setTimeout(resolve, 900));
    expect(app.value(box) ?? "").toBe("08/17/2023");
  }
  expect(app.texts().some((t) => t.includes("draft has no"))).toBe(false);
}

test("phone: a new 0605's Item 4 date takes a typed date", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } } });
  await typeAndHold(app, true);
});

test("desktop: a new 0605's Item 4 date takes a typed date", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { formCode: "0605", year: 2023, period: 2, mode: "return" } } });
  await typeAndHold(app, false);
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

// PR 3: a registered month+year pair is one period control and a from/to pair one row of two date
// controls — the underlying boxes never draw on their own. 2550Q's Item 2 period and Item 4 range
// sit at the top of the form, so they mount without scrolling.
const PERIOD = "frm2550qv2024:selectedMonthNo2#period";
const RANGE_FROM = "frm2550qv2024:RtnPeriodFromNo4";
const RANGE_TO = "frm2550qv2024:RtnPeriodToNo4";

for (const width of [390, 640, 1440]) {
  test(`PR 3: 2550Q draws its period and range as single controls at ${width} points`, async () => {
    const app = await open(ENTRY, {
      width, height: 900, platform: width < 600 ? "ios" : "macos",
      host: { params: { formCode: "2550Q", year: 2025, period: 4, mode: "return" } },
    });
    await app.waitFor(`id:fp-${PERIOD}`);
    // The pair's own boxes never render on their own (the year is locked: plain text beside the month).
    expect(app.shows("id:fp-frm2550qv2024:selectedMonthNo2")).toBe(false);
    expect(app.shows("id:fp-frm2550qv2024:txtYearNo2")).toBe(false);
    // The range is two date controls with a plain "to" between them.
    await app.waitFor(`id:fp-${RANGE_FROM}`);
    expect(app.shows(`id:fp-${RANGE_TO}`)).toBe(true);
    expect(app.texts()).toContain("to");
  });
}

test("PR 3: phone period field opens its month sheet and a pick lands", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "2550Q", year: 2025, period: 4, mode: "return" } } });
  await app.waitFor(`id:fp-${PERIOD}`);
  // Calendar basis holds the month at December; the month picker still opens on the held year.
  const cal = "frm2550qv2024:selectedMonthNo2#period-calendar";
  await app.press(`id:${cal}`);
  await app.waitFor(`id:${cal}-m12`);
  await app.press(`id:${cal}-m12`);
  await app.waitFor(`id:${cal}-sheet`, { state: "gone" });
});

test("PR 3: a 1601C sample month picker follows the typed filing-period year", async () => {
  const app = await open(ENTRY, { host: { params: { formCode: "1601C", year: 0, period: 0, mode: "sample" } } });
  const period = "frm1601c:txtMonth#period";
  const input = `id:fp-${period}`;
  const calendar = `${period}-calendar`;
  const yearLabel = `id:${calendar}-year`;
  await app.waitFor(input);
  await app.type(input, "06/2025");
  await app.waitUntil(() => app.value(input) === "06/2025");
  await app.press(`id:${calendar}`);
  await app.waitFor(yearLabel);
  // The grid opens on the typed year, not the year it first mounted on.
  expect(app.texts()).toContain("2025");
  expect(app.texts()).not.toContain(String(new Date().getFullYear()));
  await app.waitFor(`id:${calendar}-m7`);
  await app.press(`id:${calendar}-m7`);
  await app.waitFor(`id:${calendar}-sheet`, { state: "gone" });
  // The pick keeps the typed year: July 2025, never July of the year the grid first mounted on.
  await app.waitUntil(() => /^0?7\/2025$|^July 2025$/.test(String(app.value(input))));
});

test("1600WP: the From calendar can move past the copied To date", async () => {
  const fromMonth = "frm1600WP:DateWithholdingMonth";
  const from = `id:fp-${fromMonth}#date`;
  const to = "id:fp-frm1600WP:DateWithholdingToMonth#date";
  const schema = bir.screen_form_schema("1600WP") as { parts: { items: { f?: { k: string }[] }[] }[] };
  const row = schema.parts.flatMap((part, pi) => part.items.map((item, ii) => ({ pi, ii, fields: item.f || [] })))
    .find((item) => item.fields.some((field) => field.k === fromMonth))!;

  const app = await open(ENTRY, {
    width: 1440, height: 900, platform: "macos",
    host: {
      params: {
        formCode: "1600WP", year: 2025, period: 5, mode: "return", stepRow: `${row.pi}:${row.ii}`,
        setup: [{ edits: { [`${fromMonth}#date`]: "05/10/2025" } }],
      },
    },
  });
  await app.waitFor(from);
  const showsDay = (input: string, day: number) =>
    [`05/${String(day).padStart(2, "0")}/2025`, `May ${day}, 2025`].includes(String(app.value(input)));
  await app.waitUntil(() => showsDay(from, 10) && showsDay(to, 10));

  await app.press(`id:${from.slice(3)}-calendar`);
  await app.waitFor("text:15");
  await app.press("text:15");
  await app.waitUntil(() => showsDay(from, 15) && showsDay(to, 15));
});
