// What the door says about itself, beside the host functions: how sending stands, which database file is
// named, the list of functions, and the quit call. Hand-written; the wrappers are in ./functions.ts (generated).
import { Door } from "ebirforms/native";
import type { DoorStatus, HostFunction } from "ebirforms/native";

export type { DoorStatus, HostFunction };

/**
 * How sending stands and which database file is named, without opening it. `sending` is "dry_run" (nothing
 * leaves this computer), "mock" (the local mock BIR server), "relay" (the local mock relay, docs/relay.md) or "refused"
 * (the submission worker did not start).
 * `databasePath` is the file this thread has open, or the file the environment names while none is open.
 */
export function doorStatus(): DoorStatus {
  return Door.status();
}

/** Every host function with its kind ("sync" or "async"), sorted by name. */
export function hostFunctions(): HostFunction[] {
  return Door.functions();
}

/**
 * The quit path of the old host: stops the submission worker and checkpoints the database. Call it once, when the
 * app quits, and use the door no more afterwards (the owner lock is held until the process exits, as before).
 */
export function shutdown(): void {
  Door.shutdown();
}
