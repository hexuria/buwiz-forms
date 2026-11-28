// A test fixture, not a screen: the form page on a Queued 1601C of the demo taxpayer, with the SUBMISSION WORKER stood in for
// (FormPageQueued.test.tsx). The return is a stored Draft in the scratch database; the host's open answers it Queued and read-only until
// the stand-in worker sends it, and bir.list_returns("filed") lists it as the stand-in says. The frame's own Queued watch is not mounted
// here: a button does what its look does (drops the polled lists), so a test decides when the lists are read again.
//   fx-look   the frame reads the worker's lists again (store.forgetWhere(POLL_KEYS))
//   fx-send   the worker sends the return: the host's open and the lists both say Submitted (sent Apr 10, 2025)
//   fx-dry    the worker's next send is a dry run: the host's open answers `submitted_dry_run` (press it before fx-send)
//   fx-list   the lists say Submitted while the host's open still says Queued (a page the lists keep disagreeing with)
//   fx-retry  the worker moves the next try two hours ahead and the lists are read again, as after a try that backed off
//   fx-sending  the worker picks the return up: host and lists say sending
//   fx-hide   the page goes away (the route changed)
//   fx-hold   the host's next open waits (a slow read: the real one takes a few tenths of a second) until fx-release answers it
// host.params { send, nextTry, refuse }: the host's open answers `send` as the stage `send` names (send_state.rs; a "refused" return
// is a Draft, "sent" and "test_sent" are Submitted, the rest Queued), its next try at `nextTry` (an RFC 3339 time); bir.resolve_unknown_send
// is stood in for: it records its call, and the next open answers the return as the answer leaves it (received: Submitted, not_received:
// Draft), or with `refuse` it throws the host's refusal.
// Readouts (texts): "opens: N" calls of bir.screen_form_open, "reads: N" reads of the filed list, "queue action: set|none" what
// store.lastQueueAction() was when the filed list was last read, "resolved: <args>" the stand-in's last call, "toasts: a | b" the toasts.
import { effect, host, mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import { bir, type SendState } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { store } from "../../shell";
import { POLL_KEYS } from "../../shell/store";
import { AppButton } from "../../ui";
import { FormPage } from "./FormPage";

const opens = signal(0);
const reads = signal(0);
const queueAt = signal(0);
const shown = signal(true);
const resolved = signal("none");
const toasts = signal<string[]>([]);
/** The fixture's parameters (header). */
interface Params {
  send?: SendState["stage"];
  nextTry?: string;
  refuse?: boolean;
}
const params = (host.params || {}) as Params;
/** The host's words for each stage, as send_state.rs writes them (a stand-in reason where the stage keeps one). */
const LINES: Record<SendState["stage"], string> = {
  queued: "Queued to send.",
  sending: "Sending now. Keep eBIRForms open.",
  checking: "Checking with Buwiz whether this return was sent. Keep eBIRForms open.",
  waiting: "Not sent yet: BIR is not accepting returns right now. It tries again by itself.",
  held: "Buwiz is holding sends for now.",
  needs_attention: "The BIR may have received this return. Check for the BIR's confirmation email before sending it again.",
  refused: "This TIN is not on your Buwiz account.",
  sent: "Sent to the BIR through Buwiz. Waiting for the BIR's confirmation email.",
  test_sent: "Test send: not filed. Nothing reached the BIR.",
};
/** The send the host's open answers while the return is as the parameters say (null once an answer moved it on). */
let send: SendState | null = params.send
  ? {
      stage: params.send,
      line: LINES[params.send],
      code: params.send === "refused" ? "tin_not_active" : null,
      next_try_at: params.send === "held" || params.send === "waiting" ? (params.nextTry ?? null) : null,
      can_resolve: params.send === "needs_attention",
    }
  : null;
/** What the host's open says (Queued until the worker sends), and what the filed list says. */
let opened: "Queued" | "Submitted" | "Draft" = params.send === "refused" ? "Draft" : params.send === "sent" || params.send === "test_sent" ? "Submitted" : "Queued";
let listed: "Queued" | "Submitted" = "Queued";
/** How the stand-in worker sends: a dry run (nothing reached BIR) or a real send. */
let dryRun = false;
const SENT_AT = "2025-04-10T01:02:03+00:00";
/** The open that fx-hold holds (answered by fx-release), and whether the next open is to wait. */
let holding = false;
let answer: (() => void) | null = null;

function Fixture() {
  createDemoTaxpayer();
  bir.save_form_draft(DEMO.tin, "1601C", 2025, 3, "Draft", {});
  const stored = bir.list_returns("saved").find((r) => r.form_code === "1601C")!;
  const fake = bir as unknown as Record<string, unknown>;
  const real = { open: bir.screen_form_open, list: bir.list_returns };
  fake.resolve_unknown_send = (...args: Parameters<typeof bir.resolve_unknown_send>) => {
    resolved.set(args.map(String).join(" "));
    if (params.refuse) throw new Error("This return is not waiting for your answer about the BIR's email.");
    opened = args[4] === "received" ? "Submitted" : "Draft";
    send = null;
    return null;
  };
  // The store and the session call bir[name] at each read, so a swap holds. The counts show a tick later: a signal is not written inside a read.
  fake.screen_form_open = async (...args: Parameters<typeof bir.screen_form_open>) => {
    setTimeout(() => opens.update((n) => n + 1), 0);
    const r = await real.open(...args);
    if (holding) {
      holding = false;
      await new Promise<void>((done) => (answer = done));
    }
    if (opened === "Draft") return { ...r, status: opened, editable: true, send };
    if (opened === "Queued") return { ...r, status: opened, editable: false, send };
    return { ...r, status: opened, editable: false, submitted_at: SENT_AT, submitted_dry_run: dryRun, send };
  };
  fake.list_returns = (which: "filed" | "saved" | "all") => {
    if (which !== "filed") return which === "saved" ? [] : real.list(which);
    setTimeout(() => reads.update((n) => n + 1), 0);
    return [{ ...stored, status: listed, send }];
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
        <FormPage
          tin={DEMO.tin}
          formCode="1601C"
          year={2025}
          period={3}
          mode="return"
          toast={(message) => {
            toasts.update((list) => [...list, message]);
            return "fx-toast";
          }}
          dismissToast={() => {}}
        />
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
        <AppButton
          testID="fx-sending"
          size="sm"
          label="Sending"
          onPress={() => {
            if (send) send = { ...send, stage: "sending", line: LINES.sending, next_try_at: null };
            store.forgetWhere(POLL_KEYS);
          }}
        />
        <AppButton testID="fx-dry" size="sm" label="Dry" onPress={() => void (dryRun = true)} />
        <AppButton testID="fx-list" size="sm" label="List" onPress={() => void (listed = "Submitted")} />
        <AppButton
          testID="fx-retry"
          size="sm"
          label="Retry"
          onPress={() => {
            if (send) send = { ...send, next_try_at: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString() };
            store.forgetWhere(POLL_KEYS);
          }}
        />
        <AppButton testID="fx-hide" size="sm" label="Hide" onPress={() => shown.set(false)} />
        <AppButton testID="fx-hold" size="sm" label="Hold" onPress={() => void (holding = true)} />
        <AppButton testID="fx-release" size="sm" label="Release" onPress={() => answer?.()} />
        <Text testID="fx-opens">{`opens: ${opens()}`}</Text>
        <Text testID="fx-reads">{`reads: ${reads()}`}</Text>
        <Text testID="fx-queue-at">{`queue action: ${queueAt() > 0 ? "set" : "none"}`}</Text>
        <Text testID="fx-resolved">{`resolved: ${resolved()}`}</Text>
        <Text testID="fx-toasts">{`toasts: ${toasts().join(" | ")}`}</Text>
      </View>
    </View>
  );
}

mount(Fixture);
