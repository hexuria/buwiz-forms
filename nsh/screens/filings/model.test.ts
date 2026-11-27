// Filings' pure rules, without a window: how a return is named, dated and grouped, what the search and filters keep, and what
// a navigation preset sets. Rows are generated (the demo taxpayer's name and TIN, a Cebu branch, a second generated taxpayer).
import { expect, test } from "nsh/test";
import type { ReturnRow, SendState } from "../../bir";
import {
  cardLabel, COLUMNS, dateLine, type FilterValues, filterRows, localMonth, monthLabel, namedBranch, newestFirst, periodKey, periodRank,
  plural, presetFilters, presetOf, returnTitle, sendChip, STATUSES, whoLine,
} from "./model";

const INDEX = [
  { c: "1601C", freq: "Monthly", t: "Monthly Remittance Return of Income Taxes Withheld on Compensation" },
  { c: "2551Q", freq: "Quarterly", t: "Quarterly Percentage Tax Return" },
  { c: "1604C", freq: "Annual", t: "Annual Information Return" },
  { c: "2000", freq: "As needed", t: "Documentary Stamp Tax Declaration" },
];

function row(over: Partial<ReturnRow>): ReturnRow {
  return {
    id: 1, tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 2,
    form_code: "2551Q", taxable_year: 2026, period: 3, period_label: "Q3 2026", status: "Draft", updated_at: "2026-11-14 09:30:00", return_key: 3, submitted_via_relay: false, send: null, ...over,
  };
}

const ALL: FilterValues = { query: "", tps: null, forms: null, periods: null, statuses: null, updatedMonth: null };
const R = [
  row({ id: 1 }),
  row({ id: 2, form_code: "1601C", period: 9, period_label: "September 2026", status: "Queued" }),
  row({ id: 3, form_code: "1604C", period: 0, period_label: "TY 2025", taxable_year: 2025, status: "Paid", updated_at: "2026-10-31 22:30:00" }),
  row({ id: 4, tin: "00012345300001", branch_code: "00001", branch_name: "Cebu branch", period: 2, period_label: "Q2 2026", updated_at: "2026-11-15 01:00:00" }),
  row({ id: 5, tin: "00011122500000", tin_root: "000111225", taxpayer_name: "Sample Trading Corp", form_code: "1601C", period: 8, period_label: "August 2026", status: "Confirmed" }),
];
const ids = (rows: ReturnRow[]) => rows.map((r) => r.id);

test("a return is named as Recent names it: the form, then its period by how often the form is filed", () => {
  expect(returnTitle(INDEX, R[0])).toBe("2551Q Q3 2026");
  expect(returnTitle(INDEX, R[1])).toBe("1601C Sep 2026");
  expect(returnTitle(INDEX, R[2])).toBe("1604C TY 2025");
  // An as-needed return keeps the host's own label.
  expect(returnTitle(INDEX, row({ form_code: "2000", period: 1, period_label: "2026 #1" }))).toBe("2000 2026 #1");
});

test("a period key puts a month in its quarter; annual and as-needed forms are TY", () => {
  expect(periodKey(INDEX, R[0])).toBe("Q3 2026");
  expect(periodKey(INDEX, R[1])).toBe("Q3 2026");
  expect(periodKey(INDEX, row({ form_code: "1601C", period: 1 }))).toBe("Q1 2026");
  expect(periodKey(INDEX, row({ form_code: "1601C", period: 12 }))).toBe("Q4 2026");
  expect(periodKey(INDEX, R[2])).toBe("TY 2025");
  expect(periodKey(INDEX, row({ form_code: "2000", period: 1 }))).toBe("TY 2026");
  // A form the index does not list is annual.
  expect(periodKey(INDEX, row({ form_code: "9999", period: 2 }))).toBe("TY 2026");
});

test("periods sort newest year first, then the later quarter, TY after Q4", () => {
  const keys = ["Q1 2026", "TY 2025", "Q4 2025", "TY 2026", "Q3 2026"];
  expect(keys.sort((a, b) => periodRank(b) - periodRank(a))).toEqual(["TY 2026", "Q3 2026", "Q1 2026", "TY 2025", "Q4 2025"]);
  expect(periodRank("nonsense")).toBe(0);
});

test("a card names the branch only when it is not the head office", () => {
  expect(namedBranch(R[0])).toBe("");
  expect(namedBranch(row({ branch_code: "0", branch_name: "Head office" }))).toBe("");
  expect(namedBranch(R[3])).toBe("Cebu branch");
  expect(whoLine(R[0])).toBe("Juana Santos");
  expect(whoLine(R[3])).toBe("Juana Santos · Cebu branch");
  expect(cardLabel(R[3])).toBe("2551Q Q2 2026, Juana Santos · Cebu branch, Draft. Open");
});

test("a draft's date is a save, any other return's a change; the date is the local day of the UTC stamp", () => {
  expect(dateLine(R[0])).toMatch(/^Saved Nov 1[45], 2026$/);
  expect(dateLine(R[1])).toMatch(/^Updated Nov 1[45], 2026$/);
  expect(dateLine(row({ status: "Confirmed" }))).toMatch(/^Updated /);
});

test("the month a return changed in is read in local time", () => {
  const d = new Date(Date.UTC(2026, 10, 15, 12, 0));
  const expected = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
  expect(localMonth("2026-11-15 12:00:00")).toBe(expected);
  expect(localMonth("2026-11-15T12:00:00")).toBe(expected);
  // Not a timestamp: the first seven characters, as the old code did.
  expect(localMonth("2026-11-15")).toBe("2026-11");
  expect(localMonth(null)).toBe("");
  expect(monthLabel("2026-11")).toBe("Nov 2026");
  expect(monthLabel("2026-01")).toBe("Jan 2026");
});

test("newest change first", () => {
  expect(ids(newestFirst(R))).toEqual([4, 1, 2, 5, 3]);
  // The input is not reordered.
  expect(ids(R)).toEqual([1, 2, 3, 4, 5]);
});

test("every filter keeps what it names and null means every value", () => {
  expect(ids(filterRows(R, INDEX, ALL))).toEqual([1, 2, 3, 4, 5]);
  expect(ids(filterRows(R, INDEX, { ...ALL, tps: new Set(["000111225"]) }))).toEqual([5]);
  expect(ids(filterRows(R, INDEX, { ...ALL, forms: new Set(["1601C"]) }))).toEqual([2, 5]);
  expect(ids(filterRows(R, INDEX, { ...ALL, periods: new Set(["Q3 2026"]) }))).toEqual([1, 2, 5]);
  expect(ids(filterRows(R, INDEX, { ...ALL, periods: new Set(["TY 2025"]) }))).toEqual([3]);
  expect(ids(filterRows(R, INDEX, { ...ALL, statuses: new Set(["Paid", "Confirmed"]) }))).toEqual([3, 5]);
  // An empty selection keeps nothing ("No statuses").
  expect(filterRows(R, INDEX, { ...ALL, statuses: new Set() })).toEqual([]);
  // Together, each narrows the last.
  expect(ids(filterRows(R, INDEX, { ...ALL, forms: new Set(["1601C"]), tps: new Set(["000123453"]) }))).toEqual([2]);
});

test("the search wants every word, in the form, period, taxpayer, branch, status or TIN, ignoring case", () => {
  const q = (query: string) => ids(filterRows(R, INDEX, { ...ALL, query }));
  expect(q("cebu")).toEqual([4]);
  expect(q("CEBU branch")).toEqual([4]);
  expect(q("2551q q3")).toEqual([1]);
  expect(q("september")).toEqual([2]);
  expect(q("sample trading")).toEqual([5]);
  expect(q("paid")).toEqual([3]);
  expect(q("00012345300001")).toEqual([4]);
  expect(q("  juana   queued ")).toEqual([2]);
  expect(q("juana nothing")).toEqual([]);
  expect(q("   ")).toEqual([1, 2, 3, 4, 5]);
});

test("the month filter keeps what changed in that local month", () => {
  const shown = filterRows(R, INDEX, { ...ALL, updatedMonth: localMonth("2026-10-31 22:30:00") });
  expect(ids(shown)).toContain(3);
  expect(ids(filterRows(R, INDEX, { ...ALL, updatedMonth: "2019-01" }))).toEqual([]);
});

test("Filed only keeps what counts as filed: a real send, a relay send, Confirmed and Paid; never a dry run, a Draft or a Queued one", () => {
  const send = (stage: SendState["stage"]): SendState => ({ stage, line: "", code: null, next_try_at: null, can_resolve: false });
  const rows = [
    row({ id: 11, status: "Submitted", send: send("test_sent") }),
    row({ id: 12, status: "Submitted" }),
    row({ id: 13, status: "Submitted", send: send("sent") }),
    row({ id: 14, status: "Confirmed" }),
    row({ id: 15, status: "Paid" }),
    row({ id: 16, status: "Draft" }),
    row({ id: 17, status: "Queued" }),
  ];
  expect(ids(filterRows(rows, INDEX, { ...ALL, filedOnly: true }))).toEqual([12, 13, 14, 15]);
  expect(ids(filterRows(rows, INDEX, ALL))).toEqual([11, 12, 13, 14, 15, 16, 17]);
  // A navigation preset carries filedOnly only when set.
  expect(presetOf({ statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: "2026-11", filedOnly: true })).toEqual({
    statuses: ["Submitted", "Confirmed", "Paid"],
    updatedMonth: "2026-11",
    filedOnly: true,
  });
  expect(presetOf({ filedOnly: true })).toEqual({ statuses: undefined, updatedMonth: undefined, filedOnly: true });
  expect(presetOf({ filedOnly: "yes" })).toBeNull();
  expect(presetFilters({ updatedMonth: "2026-11", filedOnly: true })).toEqual({ statuses: null, updatedMonth: "2026-11", filedOnly: true });
});

test("the board's columns hold every status once, with Paid in Confirmed", () => {
  expect(COLUMNS.map((c) => c.title)).toEqual(["Draft", "Queued", "Submitted", "Confirmed"]);
  expect(COLUMNS.flatMap((c) => [...c.statuses])).toEqual([...STATUSES]);
  expect(COLUMNS[3].statuses).toEqual(["Confirmed", "Paid"]);
});

test("plural", () => {
  expect(plural(0, "return")).toBe("0 returns");
  expect(plural(1, "return")).toBe("1 return");
  expect(plural(7, "return")).toBe("7 returns");
});

test("a navigation preset: the old names, flat or inside preset", () => {
  expect(presetOf({ statuses: ["Draft"] })).toEqual({ statuses: ["Draft"], updatedMonth: undefined });
  expect(presetOf({ statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: "2026-11" })).toEqual({
    statuses: ["Submitted", "Confirmed", "Paid"],
    updatedMonth: "2026-11",
  });
  expect(presetOf({ preset: { statuses: ["Queued"] } })).toEqual({ statuses: ["Queued"], updatedMonth: undefined });
  expect(presetOf({ updatedMonth: "2026-11" })).toEqual({ statuses: undefined, updatedMonth: "2026-11" });
  // Not a preset: other params, or the wrong types.
  expect(presetOf({})).toBeNull();
  expect(presetOf({ section: "jobs" })).toBeNull();
  expect(presetOf({ statuses: "Draft", updatedMonth: 202611 })).toBeNull();
  expect(presetOf({ statuses: [1, "Draft"] })).toEqual({ statuses: ["Draft"], updatedMonth: undefined });
});

test("a preset keeps the real statuses it names, every status is no filter, and a month must be YYYY-MM", () => {
  expect(presetFilters({ statuses: ["Draft"] })).toEqual({ statuses: new Set(["Draft"]), updatedMonth: null });
  expect(presetFilters({ statuses: ["Draft", "Nonsense"] })).toEqual({ statuses: new Set(["Draft"]), updatedMonth: null });
  expect(presetFilters({ statuses: [...STATUSES] })).toEqual({ statuses: null, updatedMonth: null });
  expect(presetFilters({ statuses: ["Nonsense"] })).toEqual({ statuses: new Set(), updatedMonth: null });
  expect(presetFilters({ updatedMonth: "2026-11" })).toEqual({ statuses: null, updatedMonth: "2026-11" });
  expect(presetFilters({ updatedMonth: "Nov 2026" })).toEqual({ statuses: null, updatedMonth: null });
  expect(presetFilters({ statuses: ["Submitted"], updatedMonth: "2026-1" })).toEqual({ statuses: new Set(["Submitted"]), updatedMonth: null });
});

test("a card's send chip: Checking, Held, Retrying, Needs attention and Not sent with the host's line; the other stages and none say nothing", () => {
  const send = (stage: SendState["stage"], line = `line of ${stage}`): SendState => ({ stage, line, code: null, next_try_at: null, can_resolve: stage === "needs_attention" });
  expect(sendChip(row({ status: "Queued", send: send("held") }))).toEqual({ label: "Held", tone: "neutral", line: "line of held" });
  expect(sendChip(row({ status: "Queued", send: send("waiting") }))).toEqual({ label: "Retrying", tone: "warning", line: "line of waiting" });
  expect(sendChip(row({ status: "Queued", send: send("needs_attention") }))).toEqual({ label: "Needs attention", tone: "warning", line: "line of needs_attention" });
  expect(sendChip(row({ status: "Queued", send: send("checking") }))).toEqual({ label: "Checking", tone: "neutral", line: "line of checking" });
  expect(sendChip(row({ status: "Draft", send: send("refused") }))).toEqual({ label: "Not sent", tone: "danger", line: "line of refused" });
  for (const stage of ["queued", "sending", "sent", "test_sent"] as const) expect(sendChip(row({ send: send(stage) }))).toBe(null);
  expect(sendChip(row({}))).toBe(null);
});

test("a screen reader hears a dry-run send's word, Dry run, and a real send's Submitted", () => {
  const send = (stage: SendState["stage"]): SendState => ({ stage, line: "", code: null, next_try_at: null, can_resolve: false });
  const r = row({ form_code: "2550Q", period_label: "Q2 2026", status: "Submitted" });
  expect(cardLabel({ ...r, send: send("test_sent") }).endsWith(", Dry run. Open")).toBe(true);
  expect(cardLabel(r).endsWith(", Submitted. Open")).toBe(true);
  expect(cardLabel({ ...r, send: send("sent") }).endsWith(", Submitted. Open")).toBe(true);
});
