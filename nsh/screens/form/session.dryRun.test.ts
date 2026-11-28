// A return the submission worker sent in a dry run (nothing reached BIR) says so on its page: "Test send: not filed. Nothing reached the BIR.", where a
// real send says "Sent to BIR <date>". The fact is the host's `submitted_dry_run`, which the worker stores at the send (F21). A return
// still in the queue says the host's send words instead (send_state.rs).
// Through the real door: the dry-run worker of this process sends a Queued return of this file's scratch database at once.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { existingOfferFor, profileChangedNote, rfc3339, sendWords, sentWords, tryTime } from "./schema";
import type { ScreenFormExisting, SendState } from "../../bir";
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

/** The page's sent line and the status its chip says ("Test send: not filed" for a simulated send). */
async function header(target: FormTarget): Promise<{ line: string; chip: string }> {
  const s = new FormSession(target);
  try {
    expect(await s.open()).toBe(null);
    await s.idle();
    return { line: s.header().submittedLine, chip: s.header().statusWord };
  } finally {
    s.dispose();
  }
}

async function line(target: FormTarget): Promise<string> {
  return (await header(target)).line;
}

test("the words: a real send and a dry run", () => {
  expect(sentWords("2026-11-18T01:02:03+00:00", false)).toBe("Sent to BIR Nov 18, 2026");
  expect(sentWords("2026-11-18T01:02:03+00:00", true)).toBe("Test send: not filed. Nothing reached the BIR.");
  expect(sentWords("2026-11-18T01:02:03+00:00", false, true)).toBe("Sent to the mock relay Nov 18, 2026, not to BIR");
  expect(sentWords("2026-11-18T01:02:03+00:00", true, true)).toBe("Test send: not filed. Nothing reached the BIR.");
});

test("a read-only return's profile-changed note: a filed return's, and a dry run's (never filed)", () => {
  expect(profileChangedNote(false)).toBe("The tax profile changed after this return was filed. This return keeps what was filed.");
  expect(profileChangedNote(true)).toBe("The tax profile changed after this test send. This return keeps what the test send held.");
});

test("a sample's offer of a stored return: a dry-run send says dry run, a real send says submitted", () => {
  const existing = (submitted_dry_run: boolean): ScreenFormExisting => ({
    form_code: "1601C", year: 2025, period: 9, label: "September 2025", status: "Submitted", updated_at: "2025-10-01 09:00:00", total_payable: null,
    actions: ["view", "keep"], submitted_at: "2025-10-01T01:02:03+00:00", submitted_dry_run, confirmed_at: null,
  });
  const dry = existingOfferFor(existing(true));
  expect(dry.title).toBe("Test send: not filed. Nothing reached the BIR.");
  expect(dry.choices[0].hint).toBe("See exactly what was in the test send.");
  const real = existingOfferFor(existing(false));
  expect(real.title).toBe("September 2025 was submitted");
  expect(real.choices[0].hint).toBe("See exactly what was submitted.");
});

const send = (stage: SendState["stage"], line: string, next: string | null): SendState => ({ stage, line, code: null, next_try_at: next, can_resolve: false });

test("the send's words: a held or waiting send says when it goes again, in local time, with the date when it is not today", () => {
  const now = new Date(2026, 10, 24, 10, 0);
  const at = (d: Date) => d.toISOString();
  expect(rfc3339("2026-11-24T07:40:00.123456+00:00")?.getTime()).toBe(Date.UTC(2026, 10, 24, 7, 40));
  expect(rfc3339("2026-11-24T15:40:00+08:00")?.getTime()).toBe(Date.UTC(2026, 10, 24, 7, 40));
  expect(rfc3339("soon")).toBe(null);
  expect(tryTime(new Date(2026, 10, 24, 15, 40), now)).toBe("at 3:40 PM");
  expect(tryTime(new Date(2026, 10, 24, 0, 5), now)).toBe("at 12:05 AM");
  expect(tryTime(new Date(2026, 10, 25, 12, 0), now)).toBe("on Nov 25 at 12:00 PM");
  expect(tryTime(new Date(2027, 0, 2, 9, 5), now)).toBe("on Jan 2, 2027 at 9:05 AM");
  expect(sendWords(send("held", "Sign in to send returns", at(new Date(2026, 10, 24, 15, 40))), now)).toBe("Sign in to send returns. It goes again at 3:40 PM.");
  expect(sendWords(send("waiting", "Not sent yet: x. It tries again by itself.", at(new Date(2026, 10, 25, 9, 5))), now)).toBe(
    "Not sent yet: x. It tries again by itself. Next try on Nov 25 at 9:05 AM.",
  );
  // No time, or a stage that has none: the line alone.
  expect(sendWords(send("held", "Held.", null), now)).toBe("Held.");
  expect(sendWords(send("held", "Held.", "later"), now)).toBe("Held.");
  expect(sendWords(send("queued", "Queued to send.", at(now)), now)).toBe("Queued to send.");
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
  // A sent return keeps its dated words; the host's send words (send_state.rs) are for a return still in the queue.
  const when = sentWords(String(sent.submitted_at), true);
  expect(when === "Test send: not filed. Nothing reached the BIR.").toBe(true);
  expect(await line(at("1601C", 9))).toBe(`${when} Nothing here can be changed.`);
  // The status chip says the banner's word, not "Submitted": what is stored stays Submitted.
  expect((await header(at("1601C", 9))).chip).toBe("Test send: not filed");
  expect(stored("1601C", 9).status).toBe("Submitted");
});

test("a Confirmed return carrying the old test-send mark reads as filed", async () => {
  createDemoTaxpayer();
  const draft: Record<string, unknown> = { ...((await bir.screen_form_open(DEMO.tin, "2550Q", YEAR, 2)).draft as Record<string, unknown>) };
  delete draft.screen_form_sample;
  const life = draft.lifecycle && typeof draft.lifecycle === "object" ? (draft.lifecycle as Record<string, unknown>) : draft;
  life.status = "Confirmed";
  life.submitted_at = "2025-07-20T01:02:03+00:00";
  life.confirmed_at = "2025-07-21T01:02:03+00:00";
  life.submitted_dry_run = true;
  bir.save_form_draft(DEMO.tin, "2550Q", YEAR, 2, "Confirmed", draft);
  expect(await line(at("2550Q", 2))).toBe("Sent to BIR Jul 20, 2025, confirmed Jul 21, 2025. Nothing here can be changed.");
  expect((await header(at("2550Q", 2))).chip).toBe("Confirmed");
});

test("a return sent for real (no dry-run fact) says Sent to BIR", async () => {
  createDemoTaxpayer();
  // A return as the host builds it, stored as one that reached BIR (as a real send, or an earlier app's, leaves it).
  // Another period than the Confirmed test's: a stored return is never written over another status.
  const draft: Record<string, unknown> = { ...((await bir.screen_form_open(DEMO.tin, "2550Q", YEAR, 3)).draft as Record<string, unknown>) };
  delete draft.screen_form_sample;
  const life = draft.lifecycle && typeof draft.lifecycle === "object" ? (draft.lifecycle as Record<string, unknown>) : draft;
  life.status = "Submitted";
  life.submitted_at = "2025-07-20T01:02:03+00:00";
  bir.save_form_draft(DEMO.tin, "2550Q", YEAR, 3, "Submitted", draft);
  expect(stored("2550Q", 3).submitted_dry_run).toBe(undefined);
  expect(await line(at("2550Q", 3))).toBe("Sent to BIR Jul 20, 2025. Nothing here can be changed.");
  expect((await header(at("2550Q", 3))).chip).toBe("Submitted");
});
