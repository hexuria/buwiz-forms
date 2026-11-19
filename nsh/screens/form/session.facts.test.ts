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
