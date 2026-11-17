// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) with what the integration tests of
// shell/integration.test.tsx need on top of ShellDemo.tsx. open(entry, { host: { params } }):
//   start    the dev route ("form=1601EQ/2025/3")
//   removal  true: a stored 1601EQ 2025 period 3 with one ATC row (WI010, tax base 250.50), made through the form session
//   filed    true: a submitted 2550Q 2025 period 2 (a stored filed return with one Schedule 3 row, so it can be amended)
//   amend    true: once the frame is up, open an amendment of that filed 2550Q (the form page, not saved yet)
//   show     "route" prints the view the frame shows and the mode of the form it names (live)
// The Shell mounts after the seeding: a read made before it would keep its empty answer.
import { effect, host, mount, signal, Text, untracked, View } from "nsh";
import "../theme/fonts";
import { bir } from "../bir";
import { createDemoTaxpayer, DEMO } from "../bir/testing";
import { FormSession, type FormTarget } from "../screens/form/session";
import { app } from "./app";
import { Shell } from "./Shell";

interface Params {
  start?: string;
  removal?: boolean;
  filed?: boolean;
  amend?: boolean;
  show?: "route";
}

/** The demo taxpayer with the current tax year and the 1601C and 2551Q. */
function seedPeople() {
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  if (bir.list_tax_years(head.tin).length === 0) {
    const year = new Date().getFullYear();
    bir.add_tax_year(head.tin, year, false);
    bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
  }
}

/** A 1601EQ with one ATC row, saved through the page's own session as a person would fill it. */
async function seedRemoval() {
  const target: FormTarget = { tin: DEMO.tin, formCode: "1601EQ", year: 2025, period: 3, mode: "return" };
  const s = new FormSession(target);
  await s.open();
  await s.idle();
  s.choose({ "frm1601EQ:optWithheld:Y": "true", "frm1601EQ:optCategory:P": "true" });
  s.choose({ "frm1601EQ:txtAtcCd1": "WI010" });
  s.edit("frm1601EQ:txtTaxBase1", "250.5");
  await s.idle();
  s.dispose();
}

/** A 2550Q with one Schedule 3 row, saved through the session, then stored as Submitted (the draft the host holds, with the new status). */
async function seedFiled() {
  const target: FormTarget = { tin: DEMO.tin, formCode: "2550Q", year: 2025, period: 2, mode: "return" };
  const s = new FormSession(target);
  await s.open();
  await s.idle();
  s.edit("txtDateCovered30", "04/01/2025");
  s.edit("txtDateCovered3To0", "06/30/2025");
  s.edit("txtNameWithHoldingAgent30", "Agent One");
  s.edit("txtIncomePayment30", "1000");
  await s.flushNow();
  s.dispose();
  const draft = bir.get_form_draft(DEMO.tin, "2550Q", 2025, 2);
  if (!draft) throw new Error("the 2550Q was not saved");
  // The draft carries its own status (what the amend refuses on); the call's argument is the row's.
  bir.save_form_draft(DEMO.tin, "2550Q", 2025, 2, "Submitted", { ...draft, status: "Submitted" });
}

function Demo() {
  const p = host.params as Params;
  seedPeople();
  const ready = signal(!(p.removal || p.filed || p.amend));
  void (async () => {
    if (p.removal) await seedRemoval();
    if (p.filed || p.amend) await seedFiled();
    ready.set(true);
  })();
  if (p.amend) {
    effect(() => {
      if (!app.booted()) return;
      untracked(() => app.openReturn({ tin: DEMO.tin, code: "2550Q", year: 2025, period: 2, mode: "amend" }));
    });
  }
  const mode = () => (app.params().target as FormTarget | undefined)?.mode ?? "-";
  return (
    <View class="flex-1">
      <If when={ready()}>
        <Shell start={p.start} />
      </If>
      <If when={p.show === "route"}>
        <View class="absolute bottom-0 left-0">
          <Text testID="demo-route">{`route: ${app.view()} ${mode()}`}</Text>
        </View>
      </If>
    </View>
  );
}

mount(Demo);
