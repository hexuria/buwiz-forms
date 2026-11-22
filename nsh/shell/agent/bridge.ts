// The agent bridge: while the app's agent remote control is on (Rust feature `agent`, GPUI_AGENT=1), ask the host for the
// commands the agent queued and apply them to the frame, 50 ms apart, as the old app's `cx.timer.every(50, applyAgent)` did.
// Without the feature, or without the opt-in, nothing here runs and no timer is made.
import { bir, drainAgent, startAgent, type AgentStatus } from "../../bir";
import { errorText } from "../../lib/errors";
import { startPolls } from "../../lib/poll";
import { formUi } from "../../screens/form/formUi";
import { app, toast } from "../app";
import { isViewName } from "../routes";
import { store } from "../store";
import { applyCommand, type AgentState, type FrameDeps, parseCommand } from "./commands";

/** Milliseconds between two looks at the agent's queue: the old app's `cx.timer.every(50, applyAgent)`. */
export const AGENT_POLL_MS = 50;

/** Milliseconds between two drains of the agent's requests, on the database's thread: the old app's GPUI timer (32 ms). */
export const AGENT_DRAIN_MS = 32;

/**
 * Whether the frame takes the agent's screen commands: the app is not locked (the sign-in lock or the Buwiz gate in front of it) and
 * no taxpayer is signed in. The host refuses the agent's requests in both cases (src/agent.rs); this is the second lock for what was
 * queued before the frame closed: a command that came from the last person's agent never moves, selects or saves anything for the
 * next one. (First-run setup is not a lock: the agent's profile.save may create the first taxpayer there.)
 */
export function frameOpen(): boolean {
  return !app.agentLocked() && app.session().kind !== "taxpayer";
}

/** The frame's side of the commands: its navigation, its store, the open form page. */
export function frameDeps(): FrameDeps {
  return {
    go: (view) => {
      if (isViewName(view)) app.go(view);
    },
    openReturn: (open) => app.openReturn(open),
    refresh: (tinRoot) => {
      store.bump();
      // The host's profile.save settles the first-run question behind the page's back (it sets the access mode with the
      // first taxpayer). A setup that was waiting for that taxpayer is over; left pending, the frame would stay blank.
      if (app.setup().pending && app.authState().mode) app.setSetup(null, "choose");
      if (tinRoot) app.selectTaxpayer(tinRoot, { quiet: true });
    },
    toast: (message) => void toast(message),
    review: (message) => formUi.review(message),
    whenReady: (ref) => formUi.whenReady(ref),
    choose: (edits, clear) => formUi.choose(edits, clear),
    // The door's quit path included: app.quit() hooks, then shutdown, then the run ends.
    quit: () => app.quit(),
  };
}

/** What the bridge is started with; tests hand in their own. */
export interface BridgeOptions {
  /** Starts the listener and says how it stands (default: the host's `Agent.start`). */
  start?: () => AgentStatus;
  /** Answers the requests that wait for the agent (default: the host's `Agent.drain`). */
  drain?: () => unknown;
  /** The next command as the host hands it over: JSON text, or null (default: `bir.agent_take`). */
  take?: () => string | null;
  deps?: FrameDeps;
  every?: number;
  /** Milliseconds between two drains (default AGENT_DRAIN_MS). */
  drainEvery?: number;
  /** Whether the frame takes screen commands now (default `frameOpen`). */
  open?: () => boolean;
  onError?: (error: unknown) => void;
}

/**
 * Starts the agent's listener and, only when it listens, the poll that applies its commands. Returns the function that stops
 * the poll (the app root calls it when it ends). A build without the feature, or an app started without GPUI_AGENT=1,
 * answers `active: false` and gets no timer.
 */
export function startAgentBridge(options: BridgeOptions = {}): () => void {
  let status: AgentStatus;
  try {
    status = (options.start ?? startAgent)();
  } catch (error) {
    store.logScreen("warn", `agent: ${errorText(error)}`);
    return () => {};
  }
  if (!status.active) return () => {};
  const take = options.take ?? (() => bir.agent_take());
  const drain = options.drain ?? drainAgent;
  const deps = options.deps ?? frameDeps();
  const state: AgentState = { form: null };
  return startPolls(
    [
      // The host's side: the listener's requests are answered on this thread, which owns the database.
      { name: "agent-drain", every: options.drainEvery ?? AGENT_DRAIN_MS, run: () => void drain() },
      // The frame's side: the commands those requests queued.
      {
        name: "agent",
        every: options.every ?? AGENT_POLL_MS,
        run: async () => {
          const cmd = parseCommand(take());
          if (!cmd) return;
          // Locked, or a taxpayer signed in: the command is dropped, not held for whoever signs in next. A quit still ends the run.
          if (cmd.op !== "quit" && !(options.open ?? frameOpen)()) return;
          await applyCommand(cmd, deps, state);
        },
      },
    ],
    (_name, error) => (options.onError ?? ((e) => store.logScreen("warn", `agent command: ${errorText(e)}`)))(error),
  );
}
