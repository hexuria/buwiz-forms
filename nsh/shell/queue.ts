// Taking a return out of the queue, said once for every screen that offers it (Home, Filings, Settings > Jobs). The submission worker
// sends a queued return within a second or two (dry-run: "Submitted"), so a Cancel queue pressed on a stale card is refused by the
// host ("Other error: Only an unclaimed queued 1601C snapshot can be canceled"): the person reads that the return was already sent,
// and the lists are read again at once (and after 1 s, 3 s and 10 s) so the card stops saying Queued.
import { errorText } from "../lib/errors";
import { toast } from "./app";
import { store } from "./store";

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

/**
 * "Cancel queue": the return is an editable draft again, or the person is told why not. The cached lists are dropped either way,
 * and read again after 1 s, 3 s and 10 s. True when the queue was cancelled.
 */
export function cancelQueue(tin: string, code: string, year: number, period: number | null): boolean {
  try {
    store.call("cancel_queued", tin, code, year, period);
    toast("Queue cancelled. The return is an editable draft again.");
    return true;
  } catch (error) {
    // A return the worker has sent is no fault of the screen: only an unexpected refusal goes to the log.
    if (!isAlreadySent(error)) store.logScreen("error", `bir.cancel_queued: ${errorText(error)}`);
    toast(cancelQueueMessage(error), "triangle-alert");
    return false;
  } finally {
    store.bump("cancel_queued");
  }
}
