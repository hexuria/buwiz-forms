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
    const sentence = `Category of withholding agent is not set in the tax profile, and this return needs it. Pick Private or Government at Item ${item}, then queue.`;
    expect(bir.screen_form_queue_check(code, s.draft()!).blocks.map((b) => b.message)).toEqual([sentence]);
    expect(s.errors().some((e) => e.field === CATEGORY)).toBe(true);
    s.pickRadio(`frm${code}:optCategory:G`);
    await s.idle();
    expect(s.valueOf(`frm${code}:optCategory:G`)).toBe("true");
    expect(bir.screen_form_queue_check(code, s.draft()!).blocks).toEqual([]);
    expect(s.errors().some((e) => e.field === CATEGORY)).toBe(false);
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

test("B12: 1701's income-type card asks in Item 6's own terms; its Save ticks Item 6 and the card goes", async () => {
  createDemoTaxpayer();
  bir.set_year_facts(DEMO.tin, 2025, { tax_classification: null } as never);
  const s = await opened(at("1701", 2025, 0));
  const professional = "frm1701:rdoPg1I6TaxpayerTypeP";
  const row = () => s.row(s.itemIdForKey(professional)!)();
  const card = row().asks.find((a) => a.fact === "tax_classification")!;
  expect(card.choices.map((c) => c.label)).toEqual([
    "Single proprietor",
    "Professional",
    "Single proprietor with compensation income",
    "Professional with compensation income",
    "Compensation earner",
  ]);
  expect(s.valueOf(professional)).toBe("false");
  s.pickAsk("tax_classification", card.choices[3].value);
  s.saveAsk("tax_classification");
  await s.idle();
  expect(s.valueOf(professional)).toBe("true");
  expect(s.valueOf("frm1701:rdoPg1I6TaxpayerTypeS")).toBe("false");
  expect(bir.year_facts(DEMO.tin, 2025).facts.tax_classification).toBe("MixedIncome");
  expect(row().asks.some((a) => a.fact === "tax_classification")).toBe(false);
  s.dispose();
});
