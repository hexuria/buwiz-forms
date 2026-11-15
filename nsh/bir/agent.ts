// The agent remote control (feature `agent` of the Rust crate, off by default), beside the host functions: whether this
// build has it, start it, how it stands. Hand-written; see nsh/src/agent.rs. The frame's bridge is shell/agent.
import { Agent } from "ebirforms/native";
import type { AgentStatus } from "ebirforms/native";

export type { AgentStatus };

/**
 * Starts the agent's listener once, after the door booted: only when the build has the `agent` feature AND the app was
 * started with `GPUI_AGENT=1` (and a token). Never throws; `active` says whether it listens, `note` why not.
 */
export function startAgent(): AgentStatus {
  return Agent.start();
}

/**
 * Answers the requests that wait for the agent, on this thread (the one that owns the database). Call it every 32 ms while
 * `active`. True when the agent asked the app to quit (the `quit` command is queued for `bir.agent_take`).
 */
export function drainAgent(): boolean {
  return Agent.drain();
}

/** How the remote control stands, without starting it. */
export function agentStatus(): AgentStatus {
  return Agent.status();
}
