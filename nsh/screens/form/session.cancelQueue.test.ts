// The form page's own Cancel queue, when the host refuses it. A return the worker has already sent cannot be taken out of the queue
// ("Other error: Only an unclaimed queued 1601C snapshot can be canceled"): the page said that raw sentence, told the frame's reads nothing and
// did not read the return again, so the page and the lists kept saying Queued. Now it says the frame's sentence (lib/queue_message.ts),
// calls the page's afterWrite hook with the cancel-queue's name (the frame's reads, and its re-reads) and opens the return again.
// Through the real door (scratch database, the demo taxpayer); the host's refusal is put in by hand: a never-queued return gets another
// sentence ("No queued 1601C draft for that period"), and the dry-run worker sends a real queued one before a test can press Cancel.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { ALREADY_SENT } from "../../lib/queue_message";
import { FormSession, type FormTarget, type SessionHooks } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number, mode: FormTarget["mode"] = "return"): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode });

function hooks() {
  const said = { toasts: [] as string[], writes: [] as string[] };
  const h: SessionHooks = {
    toast: (message) => {
      said.toasts.push(message);
      return `t${said.toasts.length}`;
    },
    afterWrite: (name) => said.writes.push(name),
  };
  return { said, h };
}

async function opened(target: FormTarget, h: SessionHooks): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

/** The host's refusal of a return the worker has sent or claimed. */
const SENT = "Other error: Only an unclaimed queued 1601C snapshot can be canceled";

/** Makes bir.cancel_queued refuse with this sentence; the returned function puts the host's back. */
function refuseCancel(sentence: string): () => void {
  const fake = bir as unknown as { cancel_queued: unknown };
  const real = fake.cancel_queued;
  fake.cancel_queued = () => {
    throw new Error(sentence);
  };
  return () => void (fake.cancel_queued = real);
}

/** Counts the opens of a return (bir.screen_form_open), so a reload shows. */
function countOpens() {
  const fake = bir as unknown as { screen_form_open: typeof bir.screen_form_open };
  const real = fake.screen_form_open;
  const count = { opens: 0 };
  fake.screen_form_open = (...args) => {
    count.opens += 1;
    return real(...args);
  };
  return { count, restore: () => void (fake.screen_form_open = real) };
}

test("a Cancel queue the host refuses (the return was already sent) says so plainly, tells the frame's reads, and reads the return again", async () => {
  const { said, h } = hooks();
  const s = await opened(at("1601C", 2025, 3), h);
  const spy = countOpens();
  const put = refuseCancel(SENT);
  try {
    s.cancelQueue();
    expect(said.toasts).toEqual([ALREADY_SENT]);
    expect(said.toasts.some((t) => t.includes("Other error") || t.includes("unclaimed"))).toBe(false);
    expect(said.writes).toEqual(["cancel_queued"]);
    await s.idle();
    expect(spy.count.opens).toBe(1);
  } finally {
    put();
    spy.restore();
    s.dispose();
  }
});

test("another refusal keeps the host's words (without the error type's label), and still tells the frame and reads again", async () => {
  const { said, h } = hooks();
  const s = await opened(at("2551Q", 2025, 1), h);
  const put = refuseCancel("Other error: The database is locked");
  const spy = countOpens();
  try {
    s.cancelQueue();
    expect(said.toasts).toEqual(["The database is locked"]);
    expect(said.writes).toEqual(["cancel_queued"]);
    await s.idle();
    expect(spy.count.opens).toBe(1);
  } finally {
    put();
    spy.restore();
    s.dispose();
  }
});
