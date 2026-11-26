// Scratch fixture (untracked, measurement only): seeds the demo taxpayer's 0605 for 2026 with long
// registered name / address / line-of-business values, saves, opens it, prints through the real print path.
// Case 1: the 137-character registered address. Case 2: all three at 200 characters (forces "…" + additional sheet).
import { host, mount, signal, Text, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO, warmForms } from "./bir/testing";
import { app } from "./shell";
import { Shell } from "./shell/Shell";
import "./screens/print/host";

warmForms();

const ADDR137 =
  "UNIT 1234 TOWER B, THE VERY LONG BUILDING NAME RESIDENCES, 5678 BARANGAY SAN ISIDRO LABRADOR EXTENSION ROAD, QUEZON CITY, METRO MANILA PH";
const N200 =
  "AAAAA BBBBB CCCCCC DDDDDD EEEEEE FFFFFF GGGGGG HHHHHH IIIIII JJJJJJ KKKKKK LLLLLL MMMMMM NNNNNN OOOOOO PPPPPP QQQQQQ RRRRRR SSSSSS TTTTTT UUUUUU VVVV EXTRA LONG WORDS EXTRA LONG WORDS EXTRA LONG WORDS";
const A200 =
  "UNIT 1234 TOWER B, THE VERY LONG BUILDING NAME RESIDENCES, 5678 BARANGAY SAN ISIDRO LABRADOR EXTENSION ROAD, QUEZON CITY, METRO MANILA, NATIONAL CAPITAL REGION, REPUBLIC OF THE PHILIPPINES EXTRA LONG ";
const L200 =
  "WHOLESALE AND RETAIL TRADE OF LONG-NAME IMPORTED SPECIALTY GOODS, CUSTOMS BROKERAGE, WAREHOUSING, COLD STORAGE, LOGISTICS AND DISTRIBUTION SERVICES EXTRA LONG WORDS EXTRA LONG WORDS EXTRA LONG WORDS E";
// exact lengths are checked in the test output below
const which = "case2";

const EDITS1: Record<string, string> = {
  "frm0605:txtAddress": ADDR137,
};
const EDITS2: Record<string, string> = {
  "frm0605:txtTaxPayerName": N200,
  "frm0605:txtAddress": A200,
  "frm0605:txtLineBus": L200,
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
    bir.set_yearly_forms(head.tin, 2026, ["0605"]);
  }
  const edits = which === "case2" ? EDITS2 : EDITS1;
  void bir.screen_form_open(DEMO.tin, "0605", 2026, 1).then((opened) => {
    const unlocked = Object.keys(edits);
    const filled = bir.screen_form_fill("0605", opened.draft, edits, unlocked);
    const saved = bir.screen_form_save("0605", filled.draft);
    const rej = Object.keys(filled.rejected ?? {});
    line.set(`saved id ${saved.id}; rejected ${rej.length}: ${rej.join(", ")}`.slice(0, 400));
    app.openReturn({ tin: DEMO.tin, code: "0605", year: 2026, period: 1 });
    setTimeout(() => {
      try {
        const printed = bir.screen_form_print("0605", saved.draft ?? filled.draft);
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
