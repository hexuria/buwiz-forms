// What the update banner says for an update status (old app/shell/update.js promptFor). Pure.
import type { UpdateStatus } from "../../bir";
import { VERIFY_REJECTED } from "./updater";

/** What the banner shows: a sentence, and whether "Install now" goes with "Later". */
export interface Prompt {
  message: string;
  install: boolean;
}

/**
 * The banner for a status: a failed update says so (Later), a downloaded release offers Install now and Later, a manual check that
 * found nothing says "up to date" (Later). Every other phase, and a build with no update source, shows nothing.
 */
export function promptFor(status: UpdateStatus | null): Prompt | null {
  if (!status) return null;
  if (status.phase === "failed") return { message: status.message || VERIFY_REJECTED, install: false };
  if (status.phase === "ready" && status.version) return { message: "Version " + status.version + " is ready.", install: true };
  if (status.phase === "up_to_date" && status.message) return { message: status.message, install: false };
  return null;
}

/** What identifies the prompt on screen, so "Later" hides this one and a different one shows again. */
export function promptKey(status: UpdateStatus | null): string {
  return status ? [status.phase, status.version ?? "", status.message ?? ""].join("|") : "";
}
