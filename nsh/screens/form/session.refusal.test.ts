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
    await f.s.idle();
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
  for (const [field, section] of [["annual_income_tax_election", "income_tax_election"], ["tax_profile.business_start_date", "branch"], ["tax_profile.taxpayer_name", "taxpayer"], ["tax_profile.eopt_tier", "eopt_tier"]]) {
    const f = await refusing(`${field}: Set this in the tax profile before filing.`);
    try {
      f.s.queue();
      await f.s.idle();
      expect(f.said[0].message).toContain("Set this in the tax profile before filing.");
      expect(f.said[0].action?.label).toBe("Open tax profile");
      f.said[0].action!.onPress();
      expect(f.profiles).toEqual([{ section, year: 2025 }]);
    } finally { f.restore(); }
  }
});

test("a blank Item 8 name edited on the return offers Show item, not the unchanged taxpayer profile", async () => {
  const f = await refusing("taxpayer_name: Item 8 Taxpayer Name is required.");
  try {
    const key = "frm2551Qv2018:registeredName";
    const row = f.s.itemIdForKey(key)!;
    const profileName = bir.list_taxpayers().find((t) => t.tin_root === DEMO.tinRoot.replaceAll("-", ""))!.name;
    f.s.startProfileEdit(row);
    expect(f.s.box(key).locked()).toBe(false);
    f.s.edit(key, "");
    await f.s.idle();
    f.s.saveNow();
    await f.s.idle();
    expect(f.s.box(key).value()).toBe("");
    expect(bir.list_taxpayers().find((t) => t.tin_root === DEMO.tinRoot.replaceAll("-", ""))!.name).toBe(profileName);
    f.s.endProfileEdit([key]);
    f.s.jumpTo(0);
    f.said.length = 0;
    f.s.queue();
    await f.s.idle();
    expect(f.s.box(key).invalid()).toBe(true);
    expect(f.said[0].action?.label).toBe("Show item");
    f.said[0].action!.onPress();
    expect(f.s.reveal()!.row).toBe(row);
    expect(f.s.part(Number(row.split(":")[0]))().open).toBe(true);
    expect(f.profiles).toEqual([]);
  } finally { f.restore(); }
});

test("unknown keys and unstructured Queue failures keep their full reason, without an invented jump", async () => {
  for (const message of ["unknown_key: Zyzzyva cannot proceed.", "tax_profile.unknown_key: Zyzzyva cannot proceed.", "The filing service is unavailable."]) {
    const f = await refusing(message);
    try {
      const before = f.s.reveal();
      f.s.queue();
      await f.s.idle();
      expect(f.said[0].message).toBe(message);
      expect(f.said[0].action).toBe(undefined);
      expect(f.s.reveal()).toBe(before);
    } finally { f.restore(); }
  }
});
