// What a person reads when a cancel-queue is refused, said once for every screen that offers the button (the frame's Cancel queue in
// shell/queue.ts, and the form page's own in screens/form/session.ts). The submission worker sends a queued return within a second or
// two, so a Cancel queue pressed on a stale card or page is refused by the host ("Other error: Only an unclaimed queued 1601C snapshot
// can be canceled"): the person reads that the return was already sent. Pure.
import { errorText } from "./errors";

/** What a person reads when the return had been sent (or claimed by the worker) before the cancel got there. */
export const ALREADY_SENT = "This return was already sent.";

/** The host's refusal of a cancel-queue: only an unclaimed queued snapshot can be canceled. */
export function isAlreadySent(error: unknown): boolean {
  return /unclaimed queued .* can be canceled/i.test(errorText(error));
}

/** The sentence a failed cancel-queue shows: the plain one for a return that was sent, else the host's own words without its "Other error:" label. */
export function cancelQueueMessage(error: unknown): string {
  return isAlreadySent(error) ? ALREADY_SENT : errorText(error).replace(/^Other error:\s*/, "");
}
