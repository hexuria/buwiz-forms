// Queue looks before it asks (F15). Queue used to open "Queue this return?" (the tier warning) on a draft the host would then refuse, and only
// after Continue said "N items need attention.": the question read as if the return were ready. Now Queue looks as Check does (the same
// validation, `screen_form_validate`): a return with items to fix says so at once (the toast, every message on its row, the pill) and asks
// nothing; a return with nothing to fix is asked once when the tier warning applies and queued straight away when it does not.
// Where Check and the host's Queue can disagree, the present behaviour stays: Check clean and the host still refuses (the dialog, then the
// refusal), a message that is only a page notice, and 2551Q (its Queue reconciles the stored return with the year's tax profile first).
// Through the real door (scratch database, the demo taxpayer); each test has a return of its own (a form, year and period nobody queued).
import { expect, test } from "nsh/test";
import { bir, type YearFactsView } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { isPageNotice } from "./schema";
import { FormSession, type FormTarget, type SessionHooks } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

type Facts = Partial<YearFactsView["facts"]>;
/** The tax facts a Queue is stopped for when they are not set (its blocks): answered, so only the tier warning's fact (line of business or start date) is left. */
const ANSWERED: Facts = { eopt_tier: "Micro", tax_classification: "SelfEmployed", is_government_withholding_entity: false, income_tax_election: "EightPercent" };
const QUEUED = "Queued for background submission.";

function hooks() {
  const said = { toasts: [] as string[] };
  const h: SessionHooks = {
    toast: (message) => {
      said.toasts.push(message);
      return `t${said.toasts.length}`;
    },
  };
  return { said, h };
}

async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const t0 = Date.now();
  while (!condition()) {
    if (Date.now() - t0 > cap) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 25));
  }
}

/** Opens the return and saves it once, so Queue has a stored draft; the toasts so far are forgotten. */
async function opened(target: FormTarget, h: SessionHooks, said: { toasts: string[] }): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  s.saveNow();
  await s.idle();
  said.toasts.length = 0;
  return s;
}

/** Counts the host's Queue (and, with `refuse`, makes it refuse with that sentence); `restore` puts the host's back. */
function spyQueue(refuse?: string) {
  const fake = bir as unknown as { screen_form_queue: typeof bir.screen_form_queue };
  const real = fake.screen_form_queue;
  const calls = { n: 0 };
  fake.screen_form_queue = (...args) => {
    calls.n += 1;
    if (refuse) throw new Error(refuse);
    return real(...args);
  };
  return { calls, restore: () => void (fake.screen_form_queue = real) };
}

test("a page notice is no reason for a refused Queue, and a validation message on the same fields of another form is", () => {
  for (const [form, field] of [
    ["2553", "schedule[0].atc_code"],
    ["1604E", "schedule2[3].penalties"],
    ["1606", "transaction_date"],
    ["1701", "spouse_rdo_code"],
    ["1702EX", "other_deductions.2"],
    ["1702EX", "special_rows.1"],
    ["1702RT", "schedule_4.rows[1].mcit"],
    ["1604C", "one_date"],
  ]) {
    expect([form, field, isPageNotice(form, field)]).toEqual([form, field, true]);
  }
  for (const [form, field] of [
    ["1604C", "agent_category"],
    ["1600WP", "taxes_withheld"],
    ["1701", "tin"],
    ["1702Q", "spouse_rdo_code"],
    ["1702RT", "schedule_4.rows[1].tax"],
    ["2551Q", "schedule[0].atc_code"],
    ["1606", "transaction_dates"],
  ]) {
    expect([form, field, isPageNotice(form, field)]).toEqual([form, field, false]);
  }
});

test("items to fix and the tier warning: Queue says what Check says, shows every message, and asks nothing", async () => {
  const { said, h } = hooks();
  // A 1604C's Item 8 is blank, and no EOPT tier is set: the host would ask first (its warning) and refuse after.
  const s = await opened(at("1604C", 2023, 0), h, said);
  const spy = spyQueue();
  try {
    expect(bir.screen_form_queue_check("1604C", s.draft() as object).warns.length).toBeGreaterThan(0);
    s.queue();
    await until(() => said.toasts.length > 0, "the toast");
    expect(said.toasts).toEqual(["1 item needs attention."]);
    expect(s.overlay()).toBe(null);
    expect(spy.calls.n).toBe(0);
    expect(s.status()).toBe("Draft");
    // What Check shows: every message on its row, the pill's to-dos and red count.
    expect(s.todo().length).toBeGreaterThan(0);
    expect(s.attention()).toBeGreaterThan(0);
    expect(s.row(s.todo()[0].id)().message).not.toBe("");
    expect(said.toasts.join(" ")).not.toMatch(/agent_category|: /);
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("a second Queue with the same list goes to the host, which decides: the page's reading never keeps a return from being queued", async () => {
  const { said, h } = hooks();
  const s = await opened(at("1604C", 2022, 0), h, said);
  const spy = spyQueue();
  try {
    s.queue();
    await until(() => said.toasts.length === 1, "the first toast");
    expect(s.overlay()).toBe(null);
    expect(spy.calls.n).toBe(0);
    // Nothing was fixed: pressed again, the question is asked (the tier warning) and the host is the one that answers.
    s.queue();
    await until(() => s.overlay()?.kind === "queue-confirm", "the question");
    expect(spy.calls.n).toBe(0);
    s.confirmQueue();
    await until(() => spy.calls.n === 1, "the host's Queue");
    await until(() => said.toasts.length === 2, "the host's refusal");
    expect(said.toasts[1]).toBe("1 item needs attention.");
    expect(s.status()).toBe("Draft");
    // And the next press is told again, not asked.
    s.queue();
    await until(() => said.toasts.length === 3, "the third toast");
    expect(s.overlay()).toBe(null);
    expect(spy.calls.n).toBe(1);
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("nothing to fix and the tier warning: Queue asks once; Cancel queues nothing, Continue queues it", async () => {
  const { said, h } = hooks();
  bir.set_year_facts(DEMO.tin, 2024, ANSWERED);
  const s = await opened(at("0619E", 2024, 4), h, said);
  const spy = spyQueue();
  try {
    expect(s.errors().length).toBe(0);
    expect(bir.screen_form_queue_check("0619E", s.draft() as object).warns.length).toBeGreaterThan(0);
    s.queue();
    await until(() => s.overlay()?.kind === "queue-confirm", "the question");
    expect(said.toasts).toEqual([]);
    expect(spy.calls.n).toBe(0);
    s.cancelQueueConfirm();
    expect(s.overlay()).toBe(null);
    expect(spy.calls.n).toBe(0);
    expect(s.status()).toBe("Draft");
    s.queue();
    await until(() => s.overlay()?.kind === "queue-confirm", "the question again");
    s.confirmQueue();
    expect(said.toasts).toEqual([QUEUED]);
    expect(spy.calls.n).toBe(1);
    await s.idle();
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("nothing to fix and no warning: Queue queues straight away, once, even when pressed twice", async () => {
  const { said, h } = hooks();
  bir.set_year_facts(DEMO.tin, 2022, { ...ANSWERED, tax_classification: "Corporation", business_start_date: "2020-01-01" });
  const s = await opened(at("1702RT", 2022, 0), h, said);
  const spy = spyQueue();
  try {
    expect(s.errors().length).toBe(0);
    expect(bir.screen_form_queue_check("1702RT", s.draft() as object).warns.length).toBe(0);
    s.queue();
    s.queue();
    await until(() => said.toasts.length > 0, "the toast");
    await s.idle();
    expect(said.toasts).toEqual([QUEUED]);
    expect(s.overlay()).toBe(null);
    expect(spy.calls.n).toBe(1);
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("Check clean and the host still refuses: the question, then the refusal counted as Check counts (the present behaviour)", async () => {
  const { said, h } = hooks();
  bir.set_year_facts(DEMO.tin, 2024, ANSWERED);
  const s = await opened(at("0619E", 2024, 6), h, said);
  const spy = spyQueue("agent_category: Please select an option for Item 8.");
  try {
    expect(s.errors().length).toBe(0);
    s.queue();
    await until(() => s.overlay()?.kind === "queue-confirm", "the question");
    expect(said.toasts).toEqual([]);
    s.confirmQueue();
    expect(said.toasts).toEqual(["1 item needs attention."]);
    expect(spy.calls.n).toBe(1);
    expect(s.status()).toBe("Draft");
  } finally {
    spy.restore();
    s.dispose();
  }
});

test("a message that is only a page notice does not stop Queue", async () => {
  const { said, h } = hooks();
  bir.set_year_facts(DEMO.tin, 2021, { ...ANSWERED, tax_classification: "Corporation", business_start_date: "2020-01-01" });
  const s = await opened(at("1702RT", 2021, 0), h, said);
  const spy = spyQueue();
  // The host's validation answers a notice beside its messages (1702RT's MCIT in column C): Check lists it, the host's Queue does not mind.
  const fake = bir as unknown as { screen_form_validate: typeof bir.screen_form_validate };
  const real = fake.screen_form_validate;
  fake.screen_form_validate = (...args) =>
    real(...args).then((r) => ({ ...r, errors: [...r.errors, { field: "schedule_4.rows[1].mcit", message: "Row 2 holds an MCIT amount that may have been typed in column C; check it before you file.", keys: [] }] }));
  try {
    s.check();
    await until(() => said.toasts.length > 0, "Check's toast");
    expect(said.toasts).toEqual(["1 item needs attention."]);
    said.toasts.length = 0;
    s.queue();
    await until(() => said.toasts.length > 0, "the toast");
    await s.idle();
    expect(said.toasts).toEqual([QUEUED]);
    expect(spy.calls.n).toBe(1);
  } finally {
    fake.screen_form_validate = real;
    spy.restore();
    s.dispose();
  }
});

test("2551Q: its Queue reconciles the stored return with the year's tax profile, so Queue still asks and the host decides", async () => {
  const { said, h } = hooks();
  bir.set_year_facts(DEMO.tin, 2024, ANSWERED);
  const s = await opened(at("2551Q", 2024, 2), h, said);
  try {
    // Check lists the Item 13 election; the warning (business start date) is there to ask.
    expect(s.errors().length).toBeGreaterThan(0);
    expect(bir.screen_form_queue_check("2551Q", s.draft() as object).warns.length).toBeGreaterThan(0);
    s.queue();
    await until(() => s.overlay()?.kind === "queue-confirm", "the question");
    expect(said.toasts).toEqual([]);
    s.cancelQueueConfirm();
    expect(s.status()).toBe("Draft");
  } finally {
    s.dispose();
  }
});
