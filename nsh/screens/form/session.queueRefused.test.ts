// A Queue the host refuses (the draft fails its validation) says how many items need attention, as Check does, and shows them on the page
// (every message, the pill's to-dos). The host lists what it refused as "field: message; ..." with its own names for the boxes
// ("agent_category: Please select an option for Item 8."): those names are not for the filer. Through the real door (scratch database,
// the demo taxpayer); a 1604C's Item 8 is blank, and the EOPT tier is not set. Queue looks first (session.queueFirst.test.ts): it says the
// count at once, as Check does, and asks nothing, where it used to ask "Queue this return?" and refuse only after Continue.
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget, type SessionHooks } from "./session";
import { listedFields } from "./schema";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const t0 = Date.now();
  while (!condition()) {
    if (Date.now() - t0 > cap) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 25));
  }
}

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

test("listedFields counts the entries of the host's \"field: message; ...\" refusal, and nothing in any other sentence", () => {
  expect(listedFields("agent_category: Please select an option for Item 8.")).toBe(1);
  expect(
    listedFields("taxes_withheld: Please select an option for Item 4.; tin: You have entered an incorrect TIN; agent_category: Please select an option for Item 7."),
  ).toBe(3);
  expect(listedFields("tax_profile.eopt_tier: Answer it in the card; year_end_month: Please enter a valid Date on Item 2.")).toBe(2);
  // A message that carries its own semicolon stays one entry.
  expect(listedFields("tin: Enter the TIN; the branch code is 5 digits")).toBe(1);
  for (const plain of [
    "Save the draft before queueing it.",
    "EOPT tier is not set in the tax profile, and it sets the penalty rates on this return. Answer it in the card at Item 21, then queue.",
    "Other error: Only an unclaimed queued 1601C snapshot can be canceled",
    "1601C is queued with queue_form_draft (month or quarter)",
    "",
  ]) {
    expect([plain, listedFields(plain)]).toEqual([plain, 0]);
  }
});

test("a refused Queue counts the items, names no box of the host, and the page shows every message", async () => {
  createDemoTaxpayer();
  const { said, h } = hooks();
  const s = new FormSession(at("1604C", 2025, 0), h);
  try {
    expect(await s.open()).toBe(null);
    await s.idle();
    s.saveNow();
    await s.idle();
    said.toasts.length = 0;
    s.queue();
    // Queue looks first: the count is said at once and nothing is asked (the EOPT tier is not set, which used to open a question).
    await until(() => said.toasts.length > 0, "the toast");
    await s.idle();
    expect(s.overlay()).toBe(null);
    expect(said.toasts).toEqual(["1 item needs attention."]);
    expect(said.toasts.join(" ")).not.toMatch(/agent_category|: /);
    expect(s.status()).toBe("Draft");
    // What blocks it is on the page: the pill's to-dos, and the row's message.
    expect(s.todo().length).toBeGreaterThan(0);
    expect(s.attention()).toBeGreaterThan(0);
    const row = s.todo()[0];
    expect(s.row(row.id)().message).not.toBe("");
  } finally {
    s.dispose();
  }
});
