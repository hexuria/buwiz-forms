// The word a return's status shows. A send the worker only simulated (a dry run, a practice account) is stored as
// Submitted with the worker's `submitted_dry_run` (the host's `send.stage` "test_sent"); every screen shows it as "Dry run", the
// word of the form page's banner ("Dry run <date>, not sent to BIR"). What is stored stays the status. A dry run is never filed:
// it counts nowhere as filed and leaves its deadline open (`countsAsFiled`). Tested in statusWord.test.ts.

/** What a shown status is decided from: the stored status, and the dry-run fact as each host answer carries it. */
export interface StatusFacts {
  status: string;
  /** `screen_form_open`, `deadlines_between`, `as_needed_forms` and the offer of a stored return carry it. */
  submitted_dry_run?: boolean | null;
  /** `list_returns` rows carry the send state ("test_sent" is a dry run). */
  send?: { stage: string } | null;
}

/** The word a simulated send shows instead of "Submitted". */
export const DRY_RUN_WORD = "Dry run";

/** A Submitted return whose send was only simulated: nothing reached BIR. */
export function isDryRun(r: StatusFacts): boolean {
  return r.status === "Submitted" && (r.submitted_dry_run === true || r.send?.stage === "test_sent");
}

/**
 * Whether a return counts as filed (a deadline met, a filed count): sent for real (Submitted and not a dry run), Confirmed or
 * Paid. A dry run sent nothing, so its deadline stays open; Draft, Queued and Not started are not filed either.
 */
export function countsAsFiled(r: StatusFacts): boolean {
  if (r.status === "Confirmed" || r.status === "Paid") return true;
  return r.status === "Submitted" && !isDryRun(r);
}

/**
 * Whether a deadline is still to do: no return, a Draft, or a dry run (it sent nothing, so the deadline is not met). A Queued
 * return is on its way and filed ones are done; neither is open.
 */
export function isOpenDeadline(r: StatusFacts): boolean {
  return r.status === "Not started" || r.status === "Draft" || isDryRun(r);
}

/** The word to show for a return's status: "Dry run" for a simulated send, else the stored status ("Submitted", "Draft" ...). */
export function statusWord(r: StatusFacts): string {
  return isDryRun(r) ? DRY_RUN_WORD : r.status;
}

/** The status chip's name for that word (`StatusChip`'s `status`): "dry_run", "submitted", "draft" ... */
export function statusChipName(r: StatusFacts): string {
  return statusWord(r).toLowerCase().replace(/ /g, "_");
}
