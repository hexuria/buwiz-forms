// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) with the agent bridge running, for
// shell/agent/bridge.test.tsx. Commands arrive the way the host hands them over (JSON text through `take`), one for each press of
// "Next command", so a test sees the frame after each. What a test asks for goes in `open(entry, { host: { params } })`:
//   commands  the JSON texts the agent host would queue, in order
//   pin       true: the owner (Juana Santos, "Just me") has the admin PIN 1234, so the sign-in screen (TIN 000123453) comes first
//   fresh     true: an empty database in front of the first-run question (firstRun), and the first "Next command" also writes the demo
//             taxpayer first, as the host's `profile.save` does behind the window's back
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { cleanup, host, mount, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer } from "./bir/testing";
import { Shell } from "./shell/Shell";
import { startAgentBridge } from "./shell/agent";
import { AppButton } from "./ui";

interface Params {
  commands?: string[];
  fresh?: boolean;
  pin?: boolean;
}

function Demo() {
  const p = host.params as Params;
  const year = new Date().getFullYear();
  const seed = () => {
    const head = createDemoTaxpayer().branches[0];
    // The host's profile.save also settles the first-run question (one computer, one user), as the setup page does.
    if (p.fresh === true) bir.setup_solo(head.tin.slice(0, 9));
    if (bir.list_tax_years(head.tin).length === 0) {
      bir.add_tax_year(head.tin, year, false);
      bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
    }
  };
  if (p.fresh !== true) seed();
  if (p.pin === true && !bir.auth_state().mode) {
    bir.setup_solo("000123453");
    bir.set_admin_pin("1234");
  }
  let seeded = false;
  const later = [...(p.commands ?? [])];
  const queue: string[] = [];
  // The listener is the host's; here it "listens" and the queue is the test's.
  cleanup(startAgentBridge({ start: () => ({ built: true, active: true, address: "127.0.0.1:1", note: null }), drain: () => false, take: () => queue.shift() ?? null, every: 20 }));
  return (
    <View class="flex-1">
      <Shell polling={false} firstRun={p.fresh === true} />
      <View class="absolute bottom-0 left-0">
        <AppButton
          testID="agent-next"
          label="Next command"
          onPress={() => {
            if (p.fresh === true && !seeded) {
              seeded = true;
              seed();
            }
            const next = later.shift();
            if (next) queue.push(next);
          }}
        />
      </View>
    </View>
  );
}

mount(Demo);
