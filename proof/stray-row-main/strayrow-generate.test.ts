import { test } from "nsh/test";
import { bir } from "./bir";
import { DEMO, createDemoTaxpayer, useDatabase, warmForms } from "./bir/testing";

useDatabase("/home/ubuntu/forms-validation/stray-row-main-proof/stray-main-validation.db");
warmForms();

test("seed additional row-schedule drafts", async () => {
  createDemoTaxpayer();
  const out: Record<string, unknown> = {};

  const eq = await bir.screen_form_open(DEMO.tin, "1601EQ", 2026, 4);
  const eqDraft = { ...(eq.draft as Record<string, unknown>) };
  eqDraft.schedule = [
    { atc_code: "WI010", tax_base: 1000, tax_withheld: 100 },
    { atc_code: "WI011", tax_base: 2000, tax_withheld: 200 },
  ];
  const eqSaved = bir.screen_form_save("1601EQ", eqDraft);
  out["1601EQ"] = { saved: eqSaved.saved, id: eqSaved.id, opens: eqSaved.opens, rows: (eqSaved.draft as Record<string, unknown> | undefined)?.schedule };

  const vat = await bir.screen_form_open(DEMO.tin, "2550Q", 2026, 4);
  const vatDraft = { ...(vat.draft as Record<string, unknown>) };
  vatDraft.schedule_1 = [
    { instance_id: null, purchase_or_import_date: null, source_code: "D", description: "INVENTED CAPITAL A", purchase_or_import_amount: 1000, input_tax: 120, estimated_life_months: 60, recognized_life_months: 12, allowable_input_tax_for_period: 24, balance_to_next_period: 96 },
    { instance_id: null, purchase_or_import_date: null, source_code: "I", description: "INVENTED CAPITAL B", purchase_or_import_amount: 2000, input_tax: 240, estimated_life_months: 60, recognized_life_months: 12, allowable_input_tax_for_period: 48, balance_to_next_period: 192 },
  ];
  const vatSaved = bir.screen_form_save("2550Q", vatDraft);
  out["2550Q"] = { saved: vatSaved.saved, id: vatSaved.id, opens: vatSaved.opens, rows: (vatSaved.draft as Record<string, unknown> | undefined)?.schedule_1 };

  console.log("STRAYSEED " + JSON.stringify(out));
});
