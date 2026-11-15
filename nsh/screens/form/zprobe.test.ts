
import { expect, test } from "nsh/test";
import { DEMO, createDemoTaxpayer } from "../../bir/testing";
import { FormSession, type FormTarget } from "./session";
const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });
const cases: [string, number, number, string][] = [["1600PT", 2025, 3, "frm1600PT:Pg2Sc1TaxRate1"], ["1602Q", 2025, 2, "frm1602Q:txtSched2TaxRate1"], ["2553", 2025, 2, "frm2553:txt14D"], ["2552", 2025, 0, "frm2552:txtPg2Pt5Sch2TaxRate1"], ["1707A", 2025, 0, "frm1707Av2021:txtI16ApplicableTaxRate"]];
for (const [code, y, p, k] of cases) {
  test("probe " + code, async () => {
    createDemoTaxpayer();
    const s = new FormSession(at(code, y, p));
    await s.open(); await s.idle();
    console.log("PROBE " + code + " " + k + " locked=" + s.box(k).locked() + " before=" + JSON.stringify(s.valueOf(k)));
    s.edit(k, "12.5"); await s.idle();
    console.log("PROBE   typed -> value=" + JSON.stringify(s.valueOf(k)) + " box=" + JSON.stringify(s.box(k).value()));
    s.edit(k, ""); await s.idle();
    console.log("PROBE   emptied -> value=" + JSON.stringify(s.valueOf(k)) + " box=" + JSON.stringify(s.box(k).value()));
    s.dispose();
    expect(1).toBe(1);
  }, 120000);
}
