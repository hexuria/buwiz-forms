// A test fixture, not a screen: the 1601C form page (February 2025) for the demo taxpayer saved with NO RDO in its
// profile, which the taxpayer page allows (screens/taxpayer/logic.ts, factError: "Empty is fine"). Each opened app starts
// on an empty scratch database, so the demo taxpayer is made here, before the page opens.
import { mount, View } from "nsh";
import { bir } from "../../bir";
import "../../theme/fonts";
import { DEMO } from "../../bir/testing";
import { FormPage } from "./FormPage";

function Fixture() {
  if (!bir.list_taxpayers().some((t) => t.tin_root === DEMO.tinRoot.replaceAll("-", ""))) {
    bir.create_taxpayer({
      tin_root: DEMO.tinRoot,
      name: DEMO.name,
      taxpayer_type: "Individual",
      head_office: { registered_address: "Quezon City", zip_code: "1100", phone: "09170000000", email: "juana@example.com" },
    });
  }
  return (
    <View class="flex-1 bg-background">
      <FormPage tin={DEMO.tin} formCode="1601C" year={2025} period={2} mode="return" toast={() => "fx"} dismissToast={() => {}} />
    </View>
  );
}

mount(Fixture);
