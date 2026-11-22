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

test("Q1·a: when the remembered answers can't be read, no 'Save to profile?' ask opens", async () => {
  createDemoTaxpayer();
  const fake = bir as unknown as { screen_form_open: typeof bir.screen_form_open };
  const real = fake.screen_form_open;
  const s = new FormSession(at("1701", 2021, 0));
  try {
    try {
      fake.screen_form_open = async (...args) =>
        ({ ...(await real(...args)), profile_ask_unknown: true }) as Awaited<ReturnType<typeof real>>;
      expect(await s.open()).toBe(null);
      await s.idle();
    } finally {
      fake.screen_form_open = real;
    }

    await askBirth(s, "04/05/1976");
    expect(s.overlay()).toBe(null);
  } finally {
    s.dispose();
  }
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

test("P1b: a refused 'Only this return' record shows the host's refusal in a toast", async () => {
  createDemoTaxpayer();
  const toasts: string[] = [];
  const s = new FormSession(at("1701", 2022, 0), {
    toast: (text) => (toasts.push(text), `t${toasts.length}`),
    dismissToast: () => {},
    afterWrite: () => {},
  });
  expect(await s.open()).toBe(null);
  await s.idle();
  await askBirth(s, "02/03/1977");
  expect(s.overlay()?.kind).toBe("profile-ask");
  (s as any).st.target.tin = "12AB";

  s.profileAskOnly();
  await s.idle();

  expect(toasts).toEqual(["tin is not a TIN of 12 to 14 digits"]);
  expect(s.overlay()).toBe(null);
  const row = s.row(s.itemIdForKey(BIRTH)!)();
  expect(row.profileAsk?.ok).toBe(false);
  expect(row.profileAsk?.text).toContain("Kept on this return only");
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
  // birth_date is the taxpayer's, not a year's: once saved it is the profile's value on every year,
  // so a year not yet answered asks on a different value. 2024 has no remembered answer.
  const s = await opened(at("1701", 2024, 0));
  s.edit(BIRTH, "03/04/1960");
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("P1b: closing the ask (Escape/scrim) answers nothing and asks again on a changed leave", async () => {
  const s = await opened(at("1701", 2023, 0));
  await askBirth(s, "07/08/1955");
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dismissProfileAsk();
  await s.idle();
  expect(s.overlay()).toBe(null);
  // The same value left again is not a changed value: no re-ask. A new value asks again.
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()).toBe(null);
  s.edit(BIRTH, "09/10/1944");
  s.blur(BIRTH);
  await s.idle();
  expect(s.overlay()?.kind).toBe("profile-ask");
  s.dispose();
});

test("F2: a choice card's pick saves at once, shows one chip, and the card stays for the session", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2025, { eopt_tier: null, line_of_business: "Bookkeeping services" } as never);
  const s = await opened(at("1601C", 2025, 3));
  // Find the row carrying the eopt_tier card (its placement row is the host's, not a fixed index).
  const askRow = () => {
    const parts = s.schema!.parts;
    for (let pi = 0; pi < parts.length; pi++) for (let i = 0; i < parts[pi].items.length; i++) {
      const id = `${pi}:${i}`;
      const a = s.row(id)().asks.find((x) => x.fact === "eopt_tier");
      if (a) return { id, ask: a };
    }
    return null;
  };
  const found = askRow();
  expect(found).not.toBe(null);
  const ask = found!.ask;
  expect(ask.control).toBe("choice");
  expect(ask.done).toBe(false);
  // The pick is the explicit answer: it saves at once (no Save button for a choice card).
  const pick = ask.choices[0].value;
  s.pickAsk("eopt_tier", pick);
  await s.idle();
  expect(bir.year_facts(DEMO.tin, 2025).facts.eopt_tier).toBe(pick);
  // One status chip now reads saved, and the card stays so a wrong pick can be corrected.
  const after = s.row(found!.id)().asks.find((a) => a.fact === "eopt_tier")!;
  expect(after.done).toBe(true);
  expect(after.pick).toBe(JSON.stringify(pick));
  // A re-pick saves again.
  const other = ask.choices[1];
  if (other) {
    s.pickAsk("eopt_tier", other.value);
    await s.idle();
    expect(bir.year_facts(DEMO.tin, 2025).facts.eopt_tier).toBe(other.value);
  }
  s.dispose();
});
