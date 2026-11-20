import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type ToastAction } from "./session";

warmForms();

async function refusing(message: string) {
  createDemoTaxpayer();
  const said: { message: string; action?: ToastAction }[] = [];
  const profiles: { section: string; year: number }[] = [];
  const s = new FormSession({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 3, mode: "return" }, {
    toast: (message, options) => said.push({ message, action: options?.action }),
    onOpenProfile: (section, year) => profiles.push({ section, year }),
  });
  expect(await s.open()).toBeNull();
  await s.idle();
  s.saveNow();
  await s.idle();
  said.length = 0;
  const fake = bir as unknown as { queue_form_draft: typeof bir.queue_form_draft; screen_form_queue_check: typeof bir.screen_form_queue_check };
  const queue = fake.queue_form_draft;
  const check = fake.screen_form_queue_check;
  fake.queue_form_draft = () => { throw new Error(message); };
  fake.screen_form_queue_check = () => ({ blocks: [], warns: [] });
  return { s, said, profiles, restore: () => { fake.queue_form_draft = queue; fake.screen_form_queue_check = check; s.dispose(); } };
}

test("2551Q Queue refusal marks Item 13 like validation and offers a jump into its closed part", async () => {
  const f = await refusing("item_13_election: Choose the applicable Item 13 income-tax-rate election or explicitly select Not applicable");
  try {
    const key = "frm2551Qv2018:taxRate1";
    const row = f.s.itemIdForKey(key)!;
    const pi = Number(row.split(":")[0]);
    f.s.jumpTo(0);
    expect(f.s.part(pi)().open).toBe(false);
    f.s.queue();
    expect(f.s.row(row)().message).toContain("Choose the applicable Item 13");
    expect(f.s.box(key).invalid()).toBe(true);
    expect(f.s.part(pi)().errors).toBeGreaterThan(0);
    expect(f.said[0].action?.label).toBe("Show item");
    f.said[0].action!.onPress();
    expect(f.s.part(pi)().open).toBe(true);
    expect(f.s.reveal()!.row).toBe(row);
  } finally { f.restore(); }
});

test("profile Queue refusals link to the relevant profile editor and form year", async () => {
  for (const [field, section] of [["annual_income_tax_election", "income_tax_election"], ["tax_profile.business_start_date", "branch"]]) {
    const f = await refusing(`${field}: Set this in the tax profile before filing.`);
    try {
      f.s.queue();
      expect(f.said[0].message).toContain("Set this in the tax profile before filing.");
      expect(f.said[0].action?.label).toBe("Open tax profile");
      f.said[0].action!.onPress();
      expect(f.profiles).toEqual([{ section, year: 2025 }]);
    } finally { f.restore(); }
  }
});

test("unknown keys and unstructured Queue failures keep their full reason, without an invented jump", async () => {
  for (const message of ["unknown_key: Zyzzyva cannot proceed.", "The filing service is unavailable."]) {
    const f = await refusing(message);
    try {
      const before = f.s.reveal();
      f.s.queue();
      expect(f.said[0].message).toBe(message);
      expect(f.said[0].action).toBe(undefined);
      expect(f.s.reveal()).toBe(before);
    } finally { f.restore(); }
  }
});
