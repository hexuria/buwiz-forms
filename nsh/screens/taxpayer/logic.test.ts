// The taxpayer screens' pure rules: dates, ATC codes, field errors, the touched rule, setups' toasts and question,
// year bounds, schedules and the section summaries.
import { expect, test } from "nsh/test";
import {
  addressMeta, afterApply, agentMeta, allowedYears, checkText, defaultAddYear, emptyNewTaxpayer, factError, factsMeta, formSub, formsSavedText,
  futureDateNote, hiddenFactsText, inboxMeta, newTaxpayerErrors, newTaxpayerProblems, parseAtcCodes, pastDateError, profileDateProblem,
  profileDateShown, profileDateStored, realDate, replaceSentence, schedule,
  searchText, shownErrors, startHint, startYear, templateCounts, templateErrorText, trackingLine, yearAddedText,
} from "./logic";

const NOW = new Date(2026, 10, 15);

test("dates: a real YYYY-MM-DD, not after today", () => {
  expect(realDate("2024-02-29")).toBe(true);
  expect(realDate("2025-02-29")).toBe(false);
  expect(pastDateError("", NOW)).toBeNull();
  // The form page's sentences: the profile's boxes are the form's date boxes.
  expect(pastDateError("1990-13-01", NOW)).toBe("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(pastDateError("02/30/1990", NOW)).toBe("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(pastDateError("08/17/88", NOW)).toBe("Type the four-digit year, for example 08/17/1988.");
  expect(pastDateError("2026-11-16", NOW)).toBe("That date is in the future.");
  expect(pastDateError("2026-11-15", NOW)).toBeNull();
});

test("profile dates: MM/DD/YYYY in the box, ISO in the store, converted at the box's edge", () => {
  expect(profileDateStored("05/17/1990")).toBe("1990-05-17");
  expect(profileDateStored("05171990")).toBe("1990-05-17");
  expect(profileDateStored("May 17, 1990")).toBe("1990-05-17");
  expect(profileDateStored("1990-05-17")).toBe("1990-05-17");
  expect(profileDateStored("")).toBe("");
  // Not a day: kept as typed, so Save refuses it with the box's sentence.
  expect(profileDateStored("02/30/1990")).toBe("02/30/1990");
  expect(profileDateShown("1990-05-17")).toBe("05/17/1990");
  expect(profileDateShown("")).toBe("");
  expect(profileDateShown("02/30/1990")).toBe("02/30/1990");
});

test("profile dates: a day after today is refused, in the sheet and on leaving the box", () => {
  expect(profileDateProblem("", "A birth date", NOW)).toBe("");
  expect(profileDateProblem("11/15/2026", "A birth date", NOW)).toBe("");
  expect(profileDateProblem("11/16/2026", "A birth date", NOW)).toBe("A birth date can't be in the future.");
  expect(profileDateProblem("02/30/1990", "A birth date", NOW)).toBe("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(profileDateProblem("08/17/88", "A birth date", NOW)).toBe("Type the four-digit year, for example 08/17/1988.");
  expect(futureDateNote("A birth date", "1990-05-17")).toBe("A birth date can't be in the future. The box kept 05/17/1990.");
  expect(futureDateNote("A business start date", "")).toBe("A business start date can't be in the future. The box stays empty.");
});

test("ATC codes and field errors", () => {
  expect(parseAtcCodes("wc160, WI010;pt010")).toEqual(["WC160", "WI010", "PT010"]);
  expect(parseAtcCodes("WC16")).toBeNull();
  expect(parseAtcCodes("")).toEqual([]);
  expect(factError("zip_code", "12a6")).toMatch(/^ZIP code is 4 digits/);
  expect(factError("zip_code", "1226")).toBeNull();
  expect(factError("email", "a@b")).toBe("Type an email like name@example.com.");
  expect(factError("rdo_code", "999", (c) => c === "050")).toMatch(/^Pick an RDO/);
  expect(factError("rdo_code", "", () => false)).toBeNull();
});

test("errors show once a field was left, all after submit, and the host's always", () => {
  const p = { tin: "a", name: "b" };
  expect(shownErrors(p, [], false)).toEqual({});
  expect(shownErrors(p, ["name"], false)).toEqual({ name: "b" });
  expect(shownErrors(p, [], true)).toEqual(p);
  expect(shownErrors({}, [], false, { tin: "host" })).toEqual({ tin: "host" });
});

test("new taxpayer: problems, a TIN in use said at once, the start year", () => {
  const taxpayers = [{ tin_root: "000123453", name: "Juana Santos" }] as never;
  const n = { ...emptyNewTaxpayer(), tin: "000123453" };
  const problems = newTaxpayerProblems(n, taxpayers, () => true);
  expect(problems.tin).toBe("TIN 000-123-453 is already Juana Santos.");
  expect(problems.name).toBe("Type the registered name.");
  expect(newTaxpayerErrors(n, problems)).toEqual({ tin: problems.tin });
  expect(newTaxpayerProblems({ ...n, start: "copy" }, taxpayers, () => true).from).toBe("Choose the taxpayer and branch to copy.");
  expect(startYear("copy", [2023, 2024], NOW)).toBe(2024);
  expect(startYear("copy", [2025, 2026], NOW)).toBe(2026);
  expect(startYear("none", null, NOW)).toBe(2026);
  expect(startHint({ adds_forms: true, forms: ["1601C"] }, "setup:x", 2026)).toBe("Adds 2026 with 1601C and the facts they use.");
  expect(startHint(null, "none", 2026)).toBe("Add a tax year after, and choose its forms.");
});

test("setups: the toast, and the question only when something set was kept", () => {
  const base = { forms: ["1601C"], set: [], kept: [], election_locked: false, templates_copied: [], templates_kept: [], forms_kept: [] };
  const quiet = afterApply(base as never, { kind: "setup", tin: "t", year: 2026, id: "x", label: "Employer" });
  expect(quiet.toast).toBe("Employer: 1 form for 2026.");
  expect(quiet.ask).toBeNull();
  const kept = afterApply({ ...base, kept: ["eopt_tier", "income_tax_election"], election_locked: true, forms_kept: ["2551Q"] } as never, {
    kind: "setup", tin: "t", year: 2026, id: "x", label: "Employer",
  });
  expect(kept.toast).toBe("Employer: 1 form for 2026. The 8% or graduated choice for 2026 stays: a return that carries it was filed.");
  expect(kept.ask?.kept).toEqual(["eopt_tier"]);
  expect(replaceSentence(kept.ask as never)).toBe(
    "EOPT tier is already set. Employer has something else. 2551Q is on your forms list and not in Employer; it stays unless you replace. Replace what is set?",
  );
});

test("years, schedules and counts", () => {
  expect(allowedYears({ min: 2024, max: 2027 }, [2025, 2026])).toEqual([2027, 2024]);
  expect(defaultAddYear([2027, 2024], [2025, 2026], NOW)).toBe(2027);
  expect(defaultAddYear([2027, 2026, 2025], [], NOW)).toBe(2026);
  expect(defaultAddYear([], [2026], NOW)).toBeNull();
  const days = (iso: string) => (iso < "2026-11-15" ? -1 : 1);
  const rows = [
    { status: "Filed", due_date: "2026-04-25", period_label: "Q1 2026" },
    { status: "Draft", due_date: "2026-10-25", period_label: "Q3 2026" },
    { status: "Not started", due_date: "2027-01-25", period_label: "Q4 2026" },
  ];
  expect(schedule(rows, days, (d) => d)).toEqual({ next: "Q4 2026", due: "2027-01-25" });
  expect(schedule([], days, (d) => d)).toEqual({ next: null, due: "", none: true });
  // A dry run sent nothing: its period stays the next one due. A real send closes it.
  const sent = (dry: boolean) => [
    { status: "Submitted", submitted_dry_run: dry, due_date: "2027-01-25", period_label: "Q4 2026" },
    { status: "Not started", due_date: "2027-04-25", period_label: "Q1 2027" },
  ];
  expect(schedule(sent(true), days, (d) => d)).toEqual({ next: "Q4 2026", due: "2027-01-25" });
  expect(schedule(sent(false), days, (d) => d)).toEqual({ next: "Q1 2027", due: "2027-04-25" });
  expect(schedule([{ status: "Submitted", submitted_dry_run: false, due_date: "2026-04-25", period_label: "Q1 2026" }], days, (d) => d)).toEqual({ next: null, due: "" });
  expect(formSub(false, true, { next: null }, 2026)).toBe("Not filed this year");
  expect(formSub(true, false, { next: null, none: true }, 2026)).toBe("Filed in 2026");
  expect(formSub(true, false, { next: null }, 2026)).toBe("Every period filed");
  expect(formsSavedText(2026, ["1601C", "2551Q"], ["1601C", "0619E"])).toBe("2026 forms updated: 1 added, 1 removed.");
  expect(yearAddedText(2027, { copied_from: 2026, forms: ["1601C"] })).toBe("2027 added with 1 form from 2026.");
  expect(searchText({ c: "2550-Q", t: "Quarterly Value-Added Tax Return" })).toBe("2550-q 2550q quarterly value-added tax return vat");
  expect(templateCounts({ answers_kept: 1, marked: 2 })).toBe("1 answer · 2 stops");
  expect(templateErrorText("1601C", "No return found")).toBe("Open and save a 1601C return first. The template starts from its answers.");
});

test("section summaries", () => {
  expect(inboxMeta(null)).toBe("Not connected");
  expect(inboxMeta({ tracking_enabled: true, inbox_email: "a@b.c" } as never)).toBe("Tracking on · a@b.c");
  expect(agentMeta([])).toBe("Assistant must ask before filing");
  expect(agentMeta([{ form: "1601C" }, { form: "2551Q" }] as never)).toBe("1601C, 2551Q");
  expect(addressMeta(true, 0)).toBe("Used on every return");
  expect(addressMeta(false, 2)).toBe("2 of 4 from head office");
  expect(factsMeta(false, 0, 0)).toBe("Add a tax year first");
  expect(factsMeta(true, 3, 1)).toBe("1 not set");
  expect(hiddenFactsText(1)).toBe("1 fact is not shown: no form on your list uses it.");
  expect(trackingLine({ tracking_enabled: false, still_pending: false, past_window: false, window_days: 30 })).toBe("○ Verify your connection to activate email tracking.");
});

test("the inbox check never says confirmed without having looked, and a return past the window stays Submitted in words", () => {
  expect(checkText({ ok: true, still_pending: false, looked: false, error: null })).toBe(
    "Every return sent is confirmed or was a test send, so the inbox was not searched.",
  );
  expect(checkText({ ok: true, still_pending: false, looked: true, error: null })).toBe("Checked. Everything submitted is confirmed.");
  expect(checkText({ ok: true, still_pending: true, looked: true, error: null })).toBe("Checked. A submitted return is still waiting for its confirmation.");
  // #508 review: an older return the search did not match is not called confirmed.
  expect(
    checkText({ ok: true, still_pending: false, looked: true, error: null, state: { past_window: true, window_days: 30 } }),
  ).toBe("Checked. No BIR email was found for a return sent more than 30 days ago: it stays Submitted.");
  expect(
    checkText({ ok: true, still_pending: false, looked: true, error: null, state: { past_window: false, window_days: 30 } }),
  ).toBe("Checked. Everything submitted is confirmed.");
  expect(checkText({ ok: false, still_pending: true, looked: true, error: "IMAP authentication failed" })).toBe("IMAP authentication failed");

  const on = { tracking_enabled: true, window_days: 30 };
  expect(trackingLine({ ...on, still_pending: true, past_window: true })).toBe(
    "● Automated BIR receipt tracking is active — a submitted return is still waiting for its confirmation.",
  );
  const past = trackingLine({ ...on, still_pending: false, past_window: true });
  expect(past).toContain("more than 30 days ago was not matched to a BIR email within 30 days and is no longer checked automatically: it stays Submitted.");
  expect(past).toContain("Press Check now to search further back");
  expect(past).toContain("ebirforms-noreply@bir.gov.ph");
  expect(past).not.toContain("confirmed");
  expect(trackingLine({ ...on, still_pending: false, past_window: false })).toBe("● Automated BIR receipt tracking is active.");
});
