// Home's row helpers: the chip kind of a status, who a row belongs to, a deadline row's key.
import { expect, test } from "nsh/test";
import { statusWord } from "../../lib/statusWord";
import { deadlineKey, statusKind, whoLine } from "./rows";

test("a status becomes the chip kind the row shows", () => {
  expect(statusKind("Draft")).toBe("draft");
  expect(statusKind("Queued")).toBe("queued");
  for (const filed of ["Submitted", "Confirmed", "Paid"]) expect(statusKind(filed)).toBe("filed");
  expect(statusKind("Not started")).toBe("not_started");
  // A chip passes the shown word (statusWord): a simulated send's "Dry run" is its own chip.
  expect(statusKind(statusWord({ status: "Submitted", submitted_dry_run: true }))).toBe("dry_run");
  expect(statusKind(statusWord({ status: "Submitted", submitted_dry_run: false }))).toBe("filed");
  expect(statusKind("anything else")).toBe("not_started");
});

test("a row names the branch only when the taxpayer has several", () => {
  expect(whoLine({ branch_count: 1, taxpayer_name: "Juana Santos", branch_name: "Head office" })).toBe("Juana Santos");
  expect(whoLine({ branch_count: 2, taxpayer_name: "Juana Santos", branch_name: "Cebu branch" })).toBe("Juana Santos · Cebu branch");
});

test("an as-needed deadline is keyed by its saved return, so two in one period stay two rows", () => {
  const monthly = { tin: "00012345300000", form_code: "1601C", taxable_year: 2026, period: 3, frequency: "Monthly" as const, draft_id: 7 };
  expect(deadlineKey(monthly)).toBe("due-00012345300000-1601C-2026-3");
  const asNeeded = { ...monthly, form_code: "2200M", frequency: "As needed" as const };
  expect(deadlineKey(asNeeded)).toBe("due-00012345300000-2200M-2026-3-7");
  expect(deadlineKey({ ...asNeeded, draft_id: 8 })).not.toBe(deadlineKey(asNeeded));
});
