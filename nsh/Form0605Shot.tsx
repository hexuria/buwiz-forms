// A test fixture, not a screen: Form 0605 (Payment Form) of the demo taxpayer whose 2023 tax profile has no line of business. It is
// the case the owner reported (Item 12 and its tax-profile card, Item 4 Due Date). shots/ffbugs.test.tsx opens it. It lives at the nsh
// root: a real-window run serves assets (fonts, icons) only from the entry's folder down (nsh/AGENTS.md pitfall 14).
import { mount, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { FormPage } from "./screens/form/FormPage";
import { Toaster } from "./ui";

const YEAR = 2023;

/**
 * Makes the taxpayer and 2023 tax year once. The year's facts that Form 0605 reads are set except line of business, so the one card
 * the page shows is the line of business card (EOPT tier is set: the form queues on it, and an unset tier would add a second card).
 */
function makeTaxpayer(): void {
  createDemoTaxpayer();
  if (!bir.list_tax_years(DEMO.tin).includes(YEAR)) {
    bir.add_tax_year(DEMO.tin, YEAR, false);
    bir.set_yearly_forms(DEMO.tin, YEAR, ["0605"]);
    bir.set_year_facts(DEMO.tin, YEAR, { eopt_tier: "Micro" });
  }
}

function Preview() {
  makeTaxpayer();
  return (
    <View class="flex-1 bg-card">
      <FormPage
        tin={DEMO.tin}
        formCode="0605"
        year={YEAR}
        period={12}
        mode="return"
        onSession={(s) =>
          void s.ready().then(() => {
            // The draft as a new return opens it: Item 3 Quarter 4th only. Items 4 Due Date, 6 ATC, 7 Return Period, 8 Tax Type and
            // Item 12 Line of Business stay blank: they are the complaints.
            s.choose({ itemQuarter_4: "true" });
          })
        }
      />
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
