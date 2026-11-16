// A form page follows the submission worker: FormSession.refreshQueued() reads a Queued, read-only return again when the frame's lists no
// longer hold it as Queued (FormPage asks it; FormPageQueued.test.tsx proves the page's side). The page stays drawn while it reads, never
// reads over typing or over a read already under way, and the lists and the page name a return by the same stored id.
// Through the real door (scratch database, the demo taxpayer); the host's open is stood in for, to answer the return Queued or Submitted.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

/** Waits for what a test proves to have happened (a slow machine takes longer, so no fixed sleep). */
async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!condition()) {
    if (Date.now() > end) throw new Error(`never happened in ${cap} ms: ${what}`);
    await new Promise<void>((done) => setTimeout(done, 5));
  }
}

/** Makes bir.screen_form_open answer the return as the stored status says (Queued and Submitted are read-only), and counts the opens. */
function storedAs(stored: { now: "Draft" | "Queued" | "Submitted" }) {
  const fake = bir as unknown as { screen_form_open: typeof bir.screen_form_open };
  const real = fake.screen_form_open;
  const count = { opens: 0 };
  fake.screen_form_open = async (...args) => {
    count.opens += 1;
    const r = await real(...args);
    return stored.now === "Draft" ? r : { ...r, status: stored.now, editable: false };
  };
  return { count, restore: () => void (fake.screen_form_open = real) };
}

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  bir.save_form_draft(target.tin, target.formCode, target.year, target.period || null, "Draft", {});
  const s = new FormSession(target);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("a Queued page reads the return again when asked, stays drawn while it reads, and shows what is stored", async () => {
  const stored = { now: "Queued" as "Draft" | "Queued" | "Submitted" };
  const spy = storedAs(stored);
  const s = await opened(at("1601C", 2025, 3));
  try {
    expect(s.status()).toBe("Queued");
    expect(s.queuedShown()).toBe(true);
    expect(spy.count.opens).toBe(1);
    // The worker sent it. The read is under way without the page going back to "Opening...": it stays what the filer sees until the answer.
    stored.now = "Submitted";
    s.refreshQueued();
    expect(s.header().loading).toBe(false);
    expect(s.queuedShown()).toBe(true);
    // A second ask while the first read is under way adds none.
    s.refreshQueued();
    await until(() => s.status() === "Submitted", "the page shows the stored status");
    expect(s.queuedShown()).toBe(false);
    expect(s.header().editable).toBe(false);
    expect(s.header().loading).toBe(false);
    expect(spy.count.opens).toBe(2);
    // A page that no longer says Queued reads nothing.
    s.refreshQueued();
    expect(spy.count.opens).toBe(2);
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("a return the worker did not move is read again and stays as it is", async () => {
  const stored = { now: "Queued" as "Draft" | "Queued" | "Submitted" };
  const spy = storedAs(stored);
  const s = await opened(at("2551Q", 2025, 2));
  try {
    expect(s.queuedShown()).toBe(true);
    s.refreshQueued();
    await until(() => spy.count.opens === 2, "the host was asked again");
    await s.idle();
    expect(s.status()).toBe("Queued");
    expect(s.queuedShown()).toBe(true);
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("an editable draft, typing waiting or not, is never read again", async () => {
  const stored = { now: "Draft" as "Draft" | "Queued" | "Submitted" };
  const spy = storedAs(stored);
  const s = await opened(at("2551Q", 2025, 1));
  try {
    expect(s.header().editable).toBe(true);
    expect(s.queuedShown()).toBe(false);
    s.edit("txtATCAmt1", "100");
    expect(s.isDirty()).toBe(true);
    s.refreshQueued();
    await s.idle();
    expect(spy.count.opens).toBe(1);
    expect(s.valueOf("txtATCAmt1")).toBe("100.00");
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("a page that is disposed, or a sample, reads nothing", async () => {
  const stored = { now: "Queued" as "Draft" | "Queued" | "Submitted" };
  const spy = storedAs(stored);
  const s = await opened(at("1601C", 2025, 4));
  const sample = new FormSession({ tin: DEMO.tin, formCode: "2551Q", year: 0, period: 0, mode: "sample" });
  try {
    expect(await sample.open()).toBe(null);
    const before = spy.count.opens;
    sample.refreshQueued();
    expect(spy.count.opens).toBe(before);
    expect(sample.queuedShown()).toBe(false);
    s.dispose();
    s.refreshQueued();
    expect(spy.count.opens).toBe(before);
  } finally {
    spy.restore();
    sample.dispose();
  }
});

test("the lists and the page name a return by the same stored id (every form that can be queued)", async () => {
  createDemoTaxpayer();
  const picks: [string, number, number][] = [["1601C", 2025, 5], ["2551Q", 2025, 3], ["1604C", 2025, 0], ["1702Q", 2025, 2], ["2200A", 2025, 6]];
  for (const [code, year, period] of picks) {
    const s = new FormSession(at(code, year, period));
    expect(await s.open()).toBe(null);
    await s.idle();
    s.saveNow();
    await s.idle();
    const row = bir.list_returns("saved").find((r) => r.form_code === code && r.taxable_year === year && r.period === period);
    expect(row?.form_code).toBe(code);
    expect(s.isReturn(row!.id)).toBe(true);
    expect(s.isReturn(row!.id + 1)).toBe(false);
    s.dispose();
  }
});
