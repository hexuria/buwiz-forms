// Home's row helpers, pure (no UI): how a row names its owner, its list key and its status chip.
import type { DeadlineRow } from "../../bir";
import { DRY_RUN_WORD } from "../../lib/statusWord";

/**
 * The chip kind of a return's status: Draft, Queued, filed (Submitted, Confirmed, Paid) or not started. A chip passes the shown word
 * (`statusWord`), so a simulated send's "Test send: not filed" is its own chip; what a filed return offers reads the stored status.
 */
export function statusKind(status: string): "draft" | "queued" | "dry_run" | "filed" | "not_started" {
  if (status === DRY_RUN_WORD) return "dry_run";
  if (status === "Draft") return "draft";
  if (status === "Queued") return "queued";
  if (status === "Submitted" || status === "Confirmed" || status === "Paid") return "filed";
  return "not_started";
}

/** Who a row belongs to: the taxpayer, plus the branch when it has several. */
export function whoLine(row: { branch_count: number; taxpayer_name: string; branch_name: string }): string {
  return row.branch_count > 1 ? `${row.taxpayer_name} · ${row.branch_name}` : row.taxpayer_name;
}

/**
 * A deadline row's id (also its list key). An as-needed row is keyed by its saved return as well: for the 2200 family a month
 * holds one return, but other as-needed forms can hold several that share a year and period, and the return keeps them apart.
 */
export function deadlineKey(d: Pick<DeadlineRow, "tin" | "form_code" | "taxable_year" | "period" | "frequency" | "draft_id">): string {
  return `due-${d.tin}-${d.form_code}-${d.taxable_year}-${d.period}${d.frequency === "As needed" ? `-${d.draft_id}` : ""}`;
}
