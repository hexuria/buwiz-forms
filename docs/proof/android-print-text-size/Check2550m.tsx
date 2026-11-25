// Scratch fixture (untracked, measurement only): seeds the demo taxpayer's 2550M January 2026 with the exact
// values of form-review/check/355/2550M/note.txt, saves the return, opens it in the shell, and says on screen
// which keys the fill rejected. The test then presses fp-print; adb drives the system print preview.
import { host, mount, signal, Text, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO, warmForms } from "./bir/testing";
import { app } from "./shell";
import { Shell } from "./shell/Shell";
import "./screens/print/host";

warmForms();

const EDITS: Record<string, string> = {
  "frm2550m:txtTaxPayerName": "SANTOS, JUANA MARIE DELA CRUZ-VILLANUEVA",
  "frm2550m:txtAddress": "UNIT 27F THE VERY LONG CORPORATE TOWER 12345 DONA JUANA MARIE DELA CRUZ AVENUE CORNER VILLANUEVA BOULEVARD BARANGAY SAN LORENZO MAKATI",
  "frm2550m:txtTelephoneNum": "09171234567",
  "frm2550m:txtZipCode": "1229",
  "frm2550m:txtAtcCde1": "VT010",
  "frm2550m:txtAmountSales1": "1,234,567.89",
  "frm2550m:txtTax13A": "987,654.32",
  "frm2550m:txtTax14": "45,678.90",
  "frm2550m:txtTax15": "12,345.67",
  "frm2550m:txtTax17A": "10,000.25",
  "frm2550m:txtTax17B": "20,000.50",
  "frm2550m:txtTax17C": "30,000.75",
  "frm2550m:txtTax17D": "40,000.10",
  "frm2550m:txtTax17E": "50,000.20",
  "txtDatePurchased0": "01/10/2026",
  "txtDescription0": "DELIVERY VAN",
  "txtAmt0": "250,000.00",
  "txtDatePurchased3A0": "01/12/2026",
  "txtDescription3A0": "PRODUCTION MACHINE",
  "txtAmt3A0": "1,200,000.00",
  "txtEstLife3A0": "60",
  "txtRecogLife3A0": "60",
  "txtDatePurchased3B0": "12/15/2025",
  "txtDescription3B0": "PRIOR CAPITAL EQUIPMENT",
  "txtAmt3B0": "600,000.00",
  "txtEstLife3B0": "60",
  "txtRecogLife3B0": "60",
  "txtinputtaxSched4": "10,000.00",
  "txtInputTaxnotDirectSched4": "20,000.00",
  "txtinputtaxSched5": "5,000.00",
  "txtAmountInputnotDirectSched5": "8,000.00",
  "frm2550m:txtTax18E": "500,000.00",
  "frm2550m:txtTax18F": "60,000.00",
  "frm2550m:txtTax18G": "300,000.00",
  "frm2550m:txtTax18H": "36,000.00",
  "frm2550m:txtTax18I": "200,000.00",
  "frm2550m:txtTax18J": "24,000.00",
  "frm2550m:txtTax18K": "100,000.00",
  "frm2550m:txtTax18L": "12,000.00",
  "frm2550m:txtTax18M": "75,000.00",
  "frm2550m:txtTax18N": "80,000.00",
  "frm2550m:txtTax18O": "9,600.00",
  "frm2550m:txtTax20D": "7,654.32",
  "frm2550m:txtTax20E": "3,333.33",
  "frm2550m:txtTax23D": "0.00",
  "frm2550m:txtTax23E": "10,000.10",
  "frm2550m:txtTax23F": "5,000.05",
  "txtNameAgent0": "ABC CREDITABLE WITHHOLDER",
  "txtIncomePayment0": "500,000.00",
  "txtAppliedCurr0": "20,000.75",
  "txtPeriodCovered0": "01/2026",
  "txtNameMillerSch70": "SAMPLE RICE MILL",
  "txtORNumSch70": "OR-123456",
  "txtAmountPaidSch70": "35,000.50",
  "txtAppliedCurrSch70": "30,000.50",
  "txtPeriodCoveredSch70": "01/2026",
  "txtNameAgentSch80": "NATIONAL GOVERNMENT AGENCY",
  "txtIncomePaymentSch80": "800,000.00",
  "txtAppliedCurrSch80": "40,000.25",
  "txtPeriodCoveredSch80": "01/2026",
  "frm2550m:txtTax25A": "1,234.56",
  "frm2550m:txtTax25B": "2,345.67",
  "frm2550m:txtTax25C": "3,456.78",
};

const line = signal("seeding…");
let ran = false;

function seed() {
  if (ran) return;
  ran = true;
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  if (bir.list_tax_years(head.tin).length === 0) {
    bir.add_tax_year(head.tin, 2026, false);
    bir.set_yearly_forms(head.tin, 2026, ["2550M"]);
  }
  void bir.screen_form_open(DEMO.tin, "2550M", 2026, 1).then((opened) => {
    const unlocked = Object.keys(EDITS);
    const filled = bir.screen_form_fill("2550M", opened.draft, EDITS, unlocked);
    const saved = bir.screen_form_save("2550M", filled.draft);
    const rej = Object.keys(filled.rejected ?? {});
    line.set(`saved id ${saved.id}; rejected ${rej.length}: ${rej.join(", ")}`.slice(0, 400));
    app.openReturn({ tin: DEMO.tin, code: "2550M", year: 2026, period: 1 });
    setTimeout(() => {
      try {
        const printed = bir.screen_form_print("2550M", saved.draft ?? filled.draft);
        line.set(`printed: ${printed.path}; rejected ${rej.length}`.slice(0, 300));
      } catch (e2) {
        line.set(`print error: ${String((e2 as Error).message ?? e2)}`.slice(0, 300));
      }
    }, 8000);
  }, (error) => line.set(`open failed: ${String((error as Error).message ?? error)}`.slice(0, 300)));
}

function Demo() {
  seed();
  return (
    <View class="flex-1">
      <Shell polling={false} />
      <View class="absolute bottom-0 left-0 bg-card p-1">
        <Text testID="seed-line">{line()}</Text>
      </View>
    </View>
  );
}

mount(Demo);
