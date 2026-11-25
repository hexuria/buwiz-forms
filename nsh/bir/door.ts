// What the door says about itself, beside the host functions: how sending stands, which database file is
// named, the list of functions, and the quit call. Hand-written; the wrappers are in ./functions.ts (generated).
import { Door } from "ebirforms/native";
import type { DoorStatus, HostFunction } from "ebirforms/native";

export type { DoorStatus, HostFunction };

/**
 * How sending stands and which database file is named, without opening it. `sending` is "dry_run" (nothing
 * leaves this computer), "mock" (the local mock BIR server), "relay" (the local mock relay, docs/relay.md), "buwiz"
 * (returns go to the BIR through the Buwiz server: every release build on a desktop, and a development build asked to
 * use one on this computer), "off" (this build cannot send a return and refuses to queue one; everything else works) or
 * "refused" (the submission worker did not start). "live" is a build that files with the BIR itself, which no build
 * that ships is.
 * `databasePath` is the file this thread has open, or the file the environment names while none is open.
 */
export function doorStatus(): DoorStatus {
  return Door.status();
}

/** Equal non-null tokens mean neither this database connection nor another one wrote. */
export function dataChangeToken(): string | null {
  return Door.dataChangeToken();
}

/** Reads and deletes one command from the opt-in Android test mailbox; inert elsewhere. */
export function devCommand(): string | null {
  return Door.devCommand();
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
