// The Buwiz sign-in, one flow for the account gate and Settings > Account: Sign in and Create account both start it. One
// owner: this module's signals. The screens draw its state (signInCards.tsx) and call its actions; neither keeps a copy.
//
// How the first tokens are obtained sits behind one seam, TokenPath: today the device flow (the browser approves this
// device while the host polls). A later path (an email, then a six-digit code, inside the app) is another TokenPath with
// cards of its own as more state kinds; it ends the same way, in `approved`, and everything after that is shared here.
import { batch, signal } from "nsh";
import type { CloudAccountStatus, CloudSignInFinish, CloudSignInStart } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, store, toast } from "../../shell";

/** Which button started it: the account site's sign-in page or its sign-up page. */
export type SignInEntry = "sign-in" | "create";

/**
 * Every state of the sign-in. `idle` is the start card before anything happened; `cancelled`, `expired` and `failed` are the
 * start card again (with nothing, the expiry sentence, or the reason). `approved` lasts no longer than the screen takes to
 * move on (Home, or the account in Settings).
 */
export type SignInState =
  | { kind: "idle" }
  | { kind: "opening"; entry: SignInEntry }
  | { kind: "waiting"; entry: SignInEntry; code: string }
  | { kind: "waiting-no-browser"; entry: SignInEntry; code: string; address: string }
  | { kind: "approved"; status: CloudAccountStatus }
  | { kind: "cancelled" }
  | { kind: "expired" }
  | { kind: "failed"; message: string };

/** What a path's start answers: the code to show, the short address to type, and whether a page opened by itself. */
export interface PathStart {
  code: string;
  address: string;
  opened: boolean;
}

/** How the first tokens are obtained. The host saves the sign-in when `finish` answers `approved`. */
export interface TokenPath {
  start(entry: SignInEntry): Promise<PathStart>;
  finish(): Promise<CloudSignInFinish>;
  cancel(): void;
  /** Opens the same page for the same code again; whether it opened. */
  reopen(): Promise<boolean>;
}

/** The device flow through the host (crates/bir-app-api/src/cloud_account.rs). */
export const devicePath: TokenPath = {
  async start(entry) {
    const started = (await store.call(entry === "create" ? "cloud_account_begin_sign_up" : "cloud_account_begin_sign_in")) as CloudSignInStart;
    return { code: started.user_code, address: started.short_uri, opened: started.browser_opened };
  },
  finish: () => store.call("cloud_account_finish_sign_in") as Promise<CloudSignInFinish>,
  cancel: () => void store.call("cloud_account_cancel_sign_in"),
  reopen: () => store.call("cloud_account_reopen_sign_in") as Promise<boolean>,
};

/** The sentence when the code ran out. */
export const EXPIRED = "The sign-in code expired before it was approved. Press Sign in again for a new code.";

const state = signal<SignInState>({ kind: "idle" });
const reopening = signal(false);
/** Each start is a new run; an answer of an older run (after a Cancel, or a second start) changes nothing. */
let run = 0;
let path: TokenPath = devicePath;

function waitingState(entry: SignInEntry, started: PathStart): SignInState {
  return started.opened
    ? { kind: "waiting", entry, code: started.code }
    : { kind: "waiting-no-browser", entry, code: started.code, address: started.address };
}

/**
 * Sign in or Create account: start the path, show what to do while the person approves, and once approved open the session
 * (the gate goes to Home; Settings shows the account). Resolves when this run ends, however it ends.
 */
async function begin(entry: SignInEntry): Promise<SignInState> {
  const current = state().kind;
  if (current === "opening" || current === "waiting" || current === "waiting-no-browser") return state();
  const mine = ++run;
  batch(() => {
    state.set({ kind: "opening", entry });
    app.setGateError(null);
  });
  try {
    const started = await path.start(entry);
    if (mine !== run) return state();
    state.set(waitingState(entry, started));
    const finished = await path.finish();
    if (mine !== run) return state();
    if (finished.outcome === "cancelled") state.set({ kind: "cancelled" });
    else if (finished.outcome === "expired") state.set({ kind: "expired" });
    else if (await app.finishUnlock()) state.set({ kind: "approved", status: finished.status });
    else state.set({ kind: "failed", message: app.gateError() ?? "The sign-in could not be opened. Try again." });
  } catch (error) {
    if (mine === run) state.set({ kind: "failed", message: errorText(error) });
  }
  return state();
}

/** Cancel: the wait stops (the host forgets the code) and the start card comes back with nothing to say. */
function cancel(): void {
  run++;
  path.cancel();
  batch(() => {
    reopening.set(false);
    state.set({ kind: "cancelled" });
  });
}

/** Open the page again (or try opening it, when nothing opened): the same page for the same code. */
async function reopen(): Promise<void> {
  const now = state();
  if (reopening() || (now.kind !== "waiting" && now.kind !== "waiting-no-browser")) return;
  const mine = run;
  reopening.set(true);
  try {
    const opened = await path.reopen();
    if (mine !== run) return;
    const current = state();
    if (opened && current.kind === "waiting-no-browser") state.set({ kind: "waiting", entry: current.entry, code: current.code });
    if (!opened) toast("The browser did not open. Go to the address shown instead.", "triangle-alert");
  } catch (error) {
    if (mine === run) toast(errorText(error), "triangle-alert");
  }
  reopening.set(false);
}

/** The flow's state and actions. */
export const signInFlow = {
  state: (): SignInState => state(),
  reopening: (): boolean => reopening(),
  /** The start button that is waiting for its own request (its spinner), if any. */
  opening: (entry: SignInEntry): boolean => {
    const now = state();
    return now.kind === "opening" && now.entry === entry;
  },
  /** Whether the person is approving now (a waiting card, not the start card). */
  waiting: (): boolean => state().kind === "waiting" || state().kind === "waiting-no-browser",
  begin,
  cancel,
  reopen,
  /** Back to the start card (a signed-out session, a test). */
  reset: (): void => {
    run++;
    state.set({ kind: "idle" });
  },
  /** Another way to the tokens (a test, or the next path). */
  usePath: (next: TokenPath): void => {
    path = next;
  },
};

/** The start card's line under the title, for the state: the expiry sentence, the reason, or nothing. */
export function startMessage(now: SignInState): { text: string; danger: boolean } | null {
  if (now.kind === "expired") return { text: EXPIRED, danger: false };
  if (now.kind === "failed") return { text: now.message, danger: true };
  return null;
}

/** One row of a waiting card: a value with Copy; `opens` when pressing the value opens the page. */
export interface CardRow {
  id: "address" | "code";
  label: string;
  value: string;
  opens: boolean;
}

/** One action of a card, in order; the first is the main one. */
export interface CardAction {
  id: "reopen" | "cancel";
  label: string;
}

/** A card of the flow beside the start card: its title, one sentence, its rows and its actions. */
export interface FlowCard {
  kind: SignInState["kind"];
  title: string;
  sentence: string;
  rows: CardRow[];
  actions: CardAction[];
}

/** The card the state draws instead of the start card; null when the start card is drawn. Pure. Copy says "this computer". */
export function flowCard(now: SignInState): FlowCard | null {
  if (now.kind === "waiting") {
    return {
      kind: now.kind,
      title: "Finish signing in in your browser",
      sentence: "Approve this computer in the page that opened, then come back here.",
      rows: [{ id: "code", label: "Code", value: now.code, opens: false }],
      actions: [
        { id: "reopen", label: "Open the page again" },
        { id: "cancel", label: "Cancel" },
      ],
    };
  }
  if (now.kind === "waiting-no-browser") {
    return {
      kind: now.kind,
      title: "Open the sign-in page",
      sentence: "Go to this address in a browser, sign in, and enter the code to approve this computer.",
      rows: [
        { id: "address", label: "Address", value: now.address, opens: true },
        { id: "code", label: "Code", value: now.code, opens: false },
      ],
      actions: [
        { id: "reopen", label: "Try opening it again" },
        { id: "cancel", label: "Cancel" },
      ],
    };
  }
  return null;
}
