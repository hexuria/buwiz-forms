// A return the submission worker sent in a dry run (nothing reached BIR) says so on its page: "Dry run <date>, not sent to BIR", where a
// real send says "Sent to BIR <date>". The fact is the host's `submitted_dry_run`, which the worker stores at the send (F21).
// Through the real door: the dry-run worker of this process sends a Queued return of this file's scratch database at once.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { sentWords } from "./schema";
import { FormSession, type FormTarget } from "./session";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const YEAR = 2025;
const at = (formCode: string, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year: YEAR, period, mode: "return" });

/** Waits for what a test proves to have happened (a slow machine takes longer, so no fixed sleep). */
async function until(condition: () => boolean, what: string, cap = 20_000): Promise<void> {
  const end = Date.now() + cap;
  while (!condition()) {
    if (Date.now() > end) throw new Error(`never happened in ${cap} ms: ${what}`);
    await new Promise<void>((done) => setTimeout(done, 20));
  }
}

function stored(formCode: string, period: number): Record<string, unknown> {
  return (bir.get_form_draft(DEMO.tin, formCode, YEAR, period) ?? {}) as Record<string, unknown>;
}

async function line(target: FormTarget): Promise<string> {
  const s = new FormSession(target);
  try {
    expect(await s.open()).toBe(null);
    await s.idle();
    return s.header().submittedLine;
  } finally {
    s.dispose();
  }
}

test("the words: a real send and a dry run", () => {
  expect(sentWords("2026-11-18T01:02:03+00:00", false)).toBe("Sent to BIR Nov 18, 2026");
  expect(sentWords("2026-11-18T01:02:03+00:00", true)).toBe("Dry run Nov 18, 2026, not sent to BIR");
});

test("a return the dry-run worker sent says Dry run, not sent to BIR", async () => {
  createDemoTaxpayer();
  if (bir.list_tax_years(DEMO.tin).length === 0) bir.add_tax_year(DEMO.tin, YEAR, false);
  bir.set_yearly_forms(DEMO.tin, YEAR, ["1601C", "2550Q"]);
  // A 1601C is queued only with its EOPT tier answered and amounts above zero.
  bir.set_year_facts(DEMO.tin, YEAR, { eopt_tier: "Micro" });
  bir.save_form_draft(DEMO.tin, "1601C", YEAR, 9, "Draft", { tax_14_total_compensation: 100000, tax_25_total_taxes_withheld: 10000 });
  bir.queue_form_draft(DEMO.tin, "1601C", YEAR, 9);
  await until(() => stored("1601C", 9).status === "Submitted", "the dry-run worker sent the queued 1601C");
  const sent = stored("1601C", 9);
  expect(sent.submitted_dry_run).toBe(true);
  const when = sentWords(String(sent.submitted_at), true);
  expect(when.startsWith("Dry run ")).toBe(true);
  expect(await line(at("1601C", 9))).toBe(`${when}. Nothing here can be changed.`);
});

test("a return sent for real (no dry-run fact) says Sent to BIR", async () => {
  createDemoTaxpayer();
  // A return as the host builds it, stored as one that reached BIR (as a real send, or an earlier app's, leaves it).
  const draft: Record<string, unknown> = { ...((await bir.screen_form_open(DEMO.tin, "2550Q", YEAR, 2)).draft as Record<string, unknown>) };
  delete draft.screen_form_sample;
  const life = draft.lifecycle && typeof draft.lifecycle === "object" ? (draft.lifecycle as Record<string, unknown>) : draft;
  life.status = "Submitted";
  life.submitted_at = "2025-07-20T01:02:03+00:00";
  bir.save_form_draft(DEMO.tin, "2550Q", YEAR, 2, "Submitted", draft);
  expect(stored("2550Q", 2).submitted_dry_run).toBe(undefined);
  expect(await line(at("2550Q", 2))).toBe("Sent to BIR Jul 20, 2025. Nothing here can be changed.");
});
