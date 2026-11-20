// B6 and B8, through the real door (scratch database, the demo taxpayer Juana Santos). Tests share one database, in order.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";

warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });
const CATEGORY = "tax_profile.is_government_withholding_entity";

async function opened(target: FormTarget, toasts: string[] = []): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, { toast: (message) => (toasts.push(message), "t" + toasts.length), dismissToast: () => {}, afterWrite: () => {} });
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

for (const [code, item] of [["0619E", "12"], ["0619F", "11"]] as const) {
  test(`B8: ${code}'s own Private or Government pick answers the category block; the opening Private does not`, async () => {
    createDemoTaxpayer();
    bir.set_year_facts(DEMO.tin, 2025, { is_government_withholding_entity: null, eopt_tier: "Micro", line_of_business: "Bookkeeping services" } as never);
    const toasts: string[] = [];
    const s = await opened(at(code, 2025, 7), toasts);
    // Q1 P1b: the category radios are a writable box now, so the block reads the boxed-fact sentence
    // ("… is not in the tax profile … Pick it …"), not the old card's "Pick Private or Government … then queue."
    const sentence = `Government withholding agent is not in the tax profile, and this return needs it. Pick it at Item ${item}.`;
    expect(bir.screen_form_queue_check(code, s.draft()!).blocks.map((b) => b.message)).toEqual([sentence]);
    expect(s.errors().some((e) => e.field === CATEGORY)).toBe(true);
    s.pickRadio(`frm${code}:optCategory:G`);
    await s.idle();
    expect(s.valueOf(`frm${code}:optCategory:G`)).toBe("true");
    expect(bir.screen_form_queue_check(code, s.draft()!).blocks).toEqual([]);
    expect(s.errors().some((e) => e.field === CATEGORY)).toBe(false);
    // The pick is a profile-backed box, so it raised "Save to profile?" once; leaving it unanswered
    // (Escape) keeps the profile untouched, which is what the last line proves.
    s.dismissProfileAsk();
    // Queue's own look no longer says the category.
    s.queue();
    await s.idle();
    expect(toasts.some((t) => t.includes("Category of withholding agent"))).toBe(false);
    // The pick wrote nothing to the tax profile.
    expect(bir.year_facts(DEMO.tin, 2025).facts.is_government_withholding_entity ?? null).toBe(null);
    s.dispose();
  });
}

test("B6 guard: a card's Save for another fact leaves a typed profile box as it reads", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2025, { eopt_tier: null, line_of_business: "Bookkeeping services" } as never);
  const s = await opened(at("1601C", 2025, 3));
  const line = "frm1601c:txtLineBus";
  s.startProfileEdit(s.itemIdForKey(line)!);
  s.edit(line, "Retail");
  s.blur(line);
  await s.idle();
  // The return holds the text as the form keeps it (upper case).
  expect(s.valueOf(line)).toBe("RETAIL");
  // The EOPT card is asked (the tier is unset); its Save writes the year's facts, which the read below proves.
  s.pickAsk("eopt_tier", "Micro");
  s.saveAsk("eopt_tier");
  await s.idle();
  expect(bir.year_facts(DEMO.tin, 2025).facts.eopt_tier).toBe("Micro");
  expect(s.valueOf(line)).toBe("RETAIL");
  s.dispose();
});

test("B12: 1701's income type is boxed — Item 6's radios are the input, no card; a pick saves to the profile", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2025, { tax_classification: null } as never);
  const s = await opened(at("1701", 2025, 0));
  const single = "frm1701:rdoPg1I6TaxpayerTypeS"; // Single proprietor → SelfEmployed
  // Q1 P1b: the income type answers in Item 6's own radios, so no fact card is drawn for it.
  const hasCard = () => {
    const parts = s.schema!.parts;
    for (let pi = 0; pi < parts.length; pi++)
      for (let i = 0; i < parts[pi].items.length; i++)
        if (s.row(`${pi}:${i}`)().asks.some((a) => a.fact === "tax_classification")) return true;
    return false;
  };
  expect(hasCard()).toBe(false);
  expect(s.valueOf(single)).toBe("false");
  // The pick is the answer on the return and, the box being profile-backed, raises "Save to profile?" once.
  s.pickRadio(single);
  await s.idle();
  expect(s.valueOf(single)).toBe("true");
  expect(s.valueOf("frm1701:rdoPg1I6TaxpayerTypeP")).toBe("false");
  expect(s.overlay()?.kind).toBe("profile-ask");
  // "Save to profile" writes the income type the profile takes for the year.
  s.profileAskSave();
  await s.idle();
  expect(s.overlay()).toBe(null);
  expect(bir.year_facts(DEMO.tin, 2025).facts.tax_classification).toBe("SelfEmployed");
  s.dispose();
});
