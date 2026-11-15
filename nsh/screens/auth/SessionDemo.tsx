// A test fixture, not a screen: the frame over two taxpayers with PINs in help-desk mode (the sign-in screen first), and four hooks
// registered with app.onSessionChange: one unregistered at once, one that throws, two that say they ran. The readout (testID
// sd-log) lists, in order, what ran at each lock, sign-in and log out. Taxpayer A (Juana Santos, PIN 4321), B (PIN 5555).
import { mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import { bir } from "../../bir";
import { createDemoTaxpayer } from "../../bir/testing";
import { app } from "../../shell/app";
import { Shell } from "../../shell/Shell";

const ran = signal<string[]>([]);
const say = (what: string) => ran.set([...ran(), what]);

function Demo() {
  createDemoTaxpayer();
  bir.create_taxpayer({ tin_root: "000-111-229", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" } });
  bir.setup_helpdesk("1234");
  bir.set_taxpayer_pin("000123453", "4321");
  bir.set_taxpayer_pin("000111229", "5555");
  app.onSessionChange(() => say("never"))();
  app.onSessionChange(() => say("first"));
  app.onSessionChange(() => {
    say("fails");
    throw new Error("a hook that fails");
  });
  app.onSessionChange(() => say("last"));
  return (
    <View class="flex-1">
      <Shell polling={false} firstRun={false} />
      <View class="absolute top-0 right-0" style={{ pointerEvents: "none" }}>
        <Text testID="sd-log">{`hooks: ${ran().join(",") || "none"}`}</Text>
        <Text testID="sd-session">{`session: ${app.session().kind}`}</Text>
      </View>
    </View>
  );
}

mount(Demo);
