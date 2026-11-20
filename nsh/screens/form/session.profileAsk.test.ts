// Q1 P1b / Q1·a / Q1b-F2 through the real door (scratch database, the demo taxpayer Juana Santos).
// Tests share one database, in order.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";

warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });
const BIRTH = "frm1701:txtPg1I10BirthDate";

async function opened(target: FormTarget): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, { toast: () => "t", dismissToast: () => {}, afterWrite: () => {} });
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

/** Type a birth date into 1701's box and leave it: the "Save to profile?" ask opens once. */
async function askBirth(s: FormSession, ymd: string) {
  s.edit(BIRTH, ymd);
  s.blur(BIRTH);
  await s.idle();
}

test("P1b: a boxed birth date asks 'Save to profile?' once on a new complete value", async () => {
  const s = await opened(at("1701", 2025, 0));
  await askBirth(s, "08/17/1988");
  const o = s.overlay();
  expect(o?.kind).toBe("profile-ask");
  if (o?.kind === "profile-ask") {
    expect(o.ask.field).toBe("birth_date");
    expect(o.ask.personal).toBe(true);
    expect(o.ask.value).toContain("1988");
  }
  // Leaving again with the same value does not stack a second ask.
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("P1b: 'Only this return' writes nothing, shows the kept line, and is remembered", async () => {
  const s = await opened(at("1701", 2025, 0));
  await askBirth(s, "08/17/1988");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.profileAskOnly();
  await s.idle();
  expect(s.overlay()).toBe(null);
  const row = s.row(s.itemIdForKey(BIRTH)!)();
  expect(row.profileAsk?.ok).toBe(false);
  expect(row.profileAsk?.text).toContain("Kept on this return only");
  // The answer is remembered for this taxpayer and year: a different value does not ask again.
  await askBirth(s, "01/02/1975");
  expect(s.overlay()).toBe(null);
  s.dispose();
});

test("P1b: 'Save to profile' writes the profile, shows the saved line, and is remembered", async () => {
  const s = await opened(at("1701", 2026, 0));
  await askBirth(s, "08/17/1988");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.profileAskSave();
  await s.idle();
  expect(s.overlay()).toBe(null);
  const row = s.row(s.itemIdForKey(BIRTH)!)();
  expect(row.profileAsk?.ok).toBe(true);
  expect(row.profileAsk?.text).toContain("Saved to the tax profile");
  // The box now equals the profile: leaving it again does not ask.
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()).toBe(null);
  s.dispose();
});

test("P1b: a value equal to the profile's never asks", async () => {
  // 2026's birth date was saved to the profile above; typing it again is not a change.
  const s = await opened(at("1701", 2026, 0));
  await askBirth(s, "08/17/1988");
  expect(s.overlay()).toBe(null);
  s.dispose();
});

test("Q1a: a different tax year asks again even after an answer", async () => {
  // 2025 was answered "only" above; 2024 has no remembered answer for birth_date.
  const s = await opened(at("1701", 2024, 0));
  await askBirth(s, "08/17/1988");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("P1b: closing the ask (Escape/scrim) answers nothing and asks again on a changed leave", async () => {
  const s = await opened(at("1701", 2023, 0));
  await askBirth(s, "08/17/1988");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dismissProfileAsk();
  await s.idle();
  expect(s.overlay()).toBe(null);
  // Same value left again: still asks (nothing was remembered).
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("F2: a choice card's pick saves at once, shows one chip, and the card stays for the session", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2025, { eopt_tier: null, line_of_business: "Bookkeeping services" } as never);
  const s = await opened(at("1601C", 2025, 3));
  const row = s.row("1:7")();
  const ask = row.asks.find((a) => a.fact === "eopt_tier");
  expect(ask?.control).toBe("choice");
  expect(ask?.done).toBe(false);
  // The pick is the explicit answer: it saves at once (no Save button for a choice card).
  s.pickAsk("eopt_tier", ask!.choices[0].value);
  await s.idle();
  expect(bir.year_facts(DEMO.tin, 2025).facts.eopt_tier).toBe(ask!.choices[0].value);
  // One status chip now reads saved, and the card stays so a wrong pick can be corrected.
  const after = s.row("1:7")();
  const card = after.asks.find((a) => a.fact === "eopt_tier")!;
  expect(card.done).toBe(true);
  expect(card.pick).toBe(JSON.stringify(ask!.choices[0].value));
  // A re-pick saves again.
  const other = ask!.choices[1];
  if (other) {
    s.pickAsk("eopt_tier", other.value);
    await s.idle();
    expect(bir.year_facts(DEMO.tin, 2025).facts.eopt_tier).toBe(other.value);
  }
  s.dispose();
});
