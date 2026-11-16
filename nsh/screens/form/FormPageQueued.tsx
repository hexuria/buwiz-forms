// A test fixture, not a screen: the form page on a Queued 1601C of the demo taxpayer, with the SUBMISSION WORKER stood in for
// (FormPageQueued.test.tsx). The return is a stored Draft in the scratch database; the host's open answers it Queued and read-only until
// the stand-in worker sends it, and bir.list_returns("filed") lists it as the stand-in says. The frame's own Queued watch is not mounted
// here: a button does what its look does (drops the polled lists), so a test decides when the lists are read again.
//   fx-look   the frame reads the worker's lists again (store.forgetWhere(POLL_KEYS))
//   fx-send   the worker sends the return: the host's open and the lists both say Submitted
//   fx-list   the lists say Submitted while the host's open still says Queued (a page the lists keep disagreeing with)
//   fx-hide   the page goes away (the route changed)
// Readouts (texts): "opens: N" calls of bir.screen_form_open, "reads: N" reads of the filed list, "queue action: set|none" what
// store.lastQueueAction() was when the filed list was last read.
import { effect, mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { store } from "../../shell";
import { POLL_KEYS } from "../../shell/store";
import { AppButton } from "../../ui";
import { FormPage } from "./FormPage";

const opens = signal(0);
const reads = signal(0);
const queueAt = signal(0);
const shown = signal(true);
/** What the host's open says (Queued until the worker sends), and what the filed list says. */
let opened: "Queued" | "Submitted" = "Queued";
let listed: "Queued" | "Submitted" = "Queued";

function Fixture() {
  createDemoTaxpayer();
  bir.save_form_draft(DEMO.tin, "1601C", 2025, 3, "Draft", {});
  const stored = bir.list_returns("saved").find((r) => r.form_code === "1601C")!;
  const fake = bir as unknown as Record<string, unknown>;
  const real = { open: bir.screen_form_open, list: bir.list_returns };
  // The store and the session call bir[name] at each read, so a swap holds. The counts show a tick later: a signal is not written inside a read.
  fake.screen_form_open = async (...args: Parameters<typeof bir.screen_form_open>) => {
    setTimeout(() => opens.update((n) => n + 1), 0);
    const r = await real.open(...args);
    return { ...r, status: opened, editable: false };
  };
  fake.list_returns = (which: "filed" | "saved" | "all") => {
    if (which !== "filed") return which === "saved" ? [] : real.list(which);
    setTimeout(() => reads.update((n) => n + 1), 0);
    return [{ ...stored, status: listed }];
  };
  // What the store held before the swap (the frame's module-level reads) is the real, empty list.
  store.forgetWhere(/^returns:/);
  // Read the filed list as a screen of the frame does (Home, the bell), so a look reads it again, and keep what the queue clock said.
  effect(() => {
    store.returns("filed");
    queueAt.set(store.lastQueueAction());
  });
  return (
    <View class="flex-1 bg-card">
      <If when={shown()}>
        <FormPage tin={DEMO.tin} formCode="1601C" year={2025} period={3} mode="return" toast={() => "fx-toast"} dismissToast={() => {}} />
      </If>
      <View class="absolute bottom-0 left-0 flex-row gap-2">
        <AppButton testID="fx-look" size="sm" label="Look" onPress={() => store.forgetWhere(POLL_KEYS)} />
        <AppButton
          testID="fx-send"
          size="sm"
          label="Send"
          onPress={() => {
            opened = "Submitted";
            listed = "Submitted";
          }}
        />
        <AppButton testID="fx-list" size="sm" label="List" onPress={() => void (listed = "Submitted")} />
        <AppButton testID="fx-hide" size="sm" label="Hide" onPress={() => shown.set(false)} />
        <Text testID="fx-opens">{`opens: ${opens()}`}</Text>
        <Text testID="fx-reads">{`reads: ${reads()}`}</Text>
        <Text testID="fx-queue-at">{`queue action: ${queueAt() > 0 ? "set" : "none"}`}</Text>
      </View>
    </View>
  );
}

mount(Fixture);
