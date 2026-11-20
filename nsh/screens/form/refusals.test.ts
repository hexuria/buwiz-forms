import { expect, test } from "nsh/test";
import { hostSentence, refusalSentence, stripOtherError, type RefusalContext } from "./refusals";

const ctx = (raw: string, over: Partial<RefusalContext> = {}): RefusalContext => ({
  raw, key: "frm0605:txtAtc", item: "6", label: "ATC", kind: "sel", form: "0605", ...over,
});

test("B4: every refusal kind becomes a plain sentence on the item", () => {
  expect(refusalSentence(ctx("draft has no `/atc/code`"))).toBe("Item 6: this answer could not be saved here.");
  expect(refusalSentence(ctx("rows of `/x` cannot be added here"))).toBe("Item 6: this answer could not be saved here.");
  expect(refusalSentence(ctx("`k` must be a whole number (got `abc`)"))).toBe("Item 6: type a whole number, for example 3.");
  expect(refusalSentence(ctx("`k` must be an amount such as 1234.50 (got `x`)"))).toBe("Item 6: type an amount, for example 1234.50.");
  expect(refusalSentence(ctx("`k` must be a date as MM/DD/YYYY (got `x`)"))).toBe("Item 6: type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(refusalSentence(ctx("Type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023"))).toBe("Item 6: type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.");
  expect(refusalSentence(ctx("month must be 01-12", { part: "month" }))).toBe("Item 6: the month is 1 to 12.");
  expect(refusalSentence(ctx("day must be 01-31", { part: "day" }))).toBe("Item 6: the day is 1 to 31.");
  expect(refusalSentence(ctx("year must be a four-digit year", { part: "year" }))).toBe("Item 6: type the four-digit year.");
  expect(refusalSentence(ctx("`k` must be the month (01-12) of a date (got `13`)"))).toBe("Item 6: the month is 1 to 12.");
  expect(refusalSentence(ctx("not a real date"))).toBe("Item 6: there is no such date. Check the day.");
  expect(refusalSentence(ctx("Pick an ATC from the list."))).toBe("Item 6: pick one of the listed choices.");
  expect(refusalSentence(ctx("Pick a tax type from the list."))).toBe("Item 6: pick one of the listed choices.");
  expect(refusalSentence(ctx("`k` is not one of the listed choices"))).toBe("Item 6: pick one of the listed choices.");
});

test("PR 2: a two-digit date year gets the four-digit example", () => {
  expect(refusalSentence(ctx("two-digit date year", { item: "4", kind: "date" })))
    .toBe("Item 4: type the four-digit year, for example 08/17/1988.");
});

test("PR 2: a bounded date names its allowed span without exposing keys", () => {
  expect(refusalSentence(ctx("date outside allowed span 2025-05-01 2025-05-31", { item: "2", kind: "date" })))
    .toBe("Item 2: choose a date from May 1, 2025 to May 31, 2025.");
});

test("R2: an impossible complete date beats the part rules; a non-range raw keeps its own sentence", () => {
  expect(refusalSentence(ctx("Date is not a real calendar date", { part: "year", item: "4" }))).toBe("Item 4: there is no such date. Check the day.");
  expect(refusalSentence(ctx("date is not a real calendar date", { part: "month" }))).toBe("Item 6: there is no such date. Check the day.");
  // A part box refused for a reason that is not a range problem does not get a range sentence.
  expect(refusalSentence(ctx("the draft cannot take this", { part: "month" }))).toBe("The draft cannot take this.");
  expect(refusalSentence(ctx("the draft cannot take this", { part: "day" }))).toBe("The draft cannot take this.");
});

test("R3/R4: official messages with slashes pass through; a real leak becomes 'needs a valid answer'", () => {
  expect(hostSentence(ctx("Please enter a valid date (MM/DD/YYYY) on Item 4.", { form: "0619E" }), "due_date")).toBe("Please enter a valid date (MM/DD/YYYY) on Item 4.");
  expect(hostSentence(ctx("File it and/or amend it.", { form: "0619E" }), "x")).toBe("File it and/or amend it.");
  expect(hostSentence(ctx("draft has no `/foo`"), "foo")).toBe("Item 6 needs a valid answer.");
  expect(hostSentence(ctx("draft has no `/foo`", { item: null, label: "ZIP code" }), "foo")).toBe("ZIP code needs a valid answer.");
  expect(hostSentence(ctx("set /atc/code first", { form: "0619E" }), "x")).toBe("Item 6 needs a valid answer.");
  expect(hostSentence(ctx("at /schedule/0/amount", { form: "0619E" }), "x")).toBe("Item 6 needs a valid answer.");
});

test("B16/B4: a gate tag becomes the used-when sentence", () => {
  expect(refusalSentence(ctx("Needs item 4 = Yes", { item: "14", label: "Amount of remittance", kind: "money" })))
    .toBe("Item 14 is used when Item 4 is Yes.");
  expect(refusalSentence(ctx("Needs Amended Return = Yes", { item: "9", label: "X", kind: "text" })))
    .toBe("Item 9 is used when Amended Return is Yes.");
});

test("B4: locks become their three sentences", () => {
  expect(refusalSentence(ctx("`k` comes from the taxpayer profile; change it in the profile"))).toBe("TIN comes from the tax profile.".replace("TIN", "Item 6"));
  expect(refusalSentence(ctx("`k` is computed by BIR's own formulas"))).toBe("Item 6 is computed.");
  expect(refusalSentence(ctx("`k` picks which return this is; reopen the form for another TIN or period"))).toBe("Item 6 is set by the return's period.");
});

test("B4: no box with an item number falls back to the label; the fallback never leaks", () => {
  expect(refusalSentence(ctx("something odd `with/key`", { item: null, label: "ZIP code" }))).toBe("ZIP code: that was not accepted. Check it and type it again.");
  expect(refusalSentence(ctx("`k` = `-5` does not fit this box: not a 0605 draft: invalid value: integer `-5`, expected u16", { kind: "text" })))
    .toBe("Item 6: that was not accepted. Check it and type it again.");
  expect(refusalSentence(ctx("at /schedule/0/amount"))).toBe("Item 6: that was not accepted. Check it and type it again.");
});

test("B4: the host's own plain sentence shows as it is, without the key in front of it", () => {
  // Said whole by the host: no key, no path.
  expect(refusalSentence(ctx("Enter the month as MM/YYYY.", { item: null, label: "1 Previous Month/s" }))).toBe("Enter the month as MM/YYYY.");
  expect(refusalSentence(ctx("Selecting an ATC is not necessary when item no. 4 is set to ' NO '", { item: "13" })))
    .toBe("Selecting an ATC is not necessary when item no. 4 is set to ' NO '.");
  expect(refusalSentence(ctx("Fill Items 1 to 3 first; row 4_1 is Item 4.", { item: "4" }))).toBe("Fill Items 1 to 3 first; row 4_1 is Item 4.");
  // Keyed by the host: the key goes, the item number takes its place.
  expect(refusalSentence(ctx("`frm2551Qv2018:txt20`: Penalties are not filled on an 8% return.", { item: "20" })))
    .toBe("Item 20: penalties are not filled on an 8% return.");
  expect(refusalSentence(ctx("`k` is computed from the days late; press Edit penalties to type it", { item: "20A" })))
    .toBe("Item 20A is computed from the days late; press Edit penalties to type it.");
  expect(refusalSentence(ctx("`k` cannot be filled from here: it is written through the form's own editors: the ATC popup")))
    .toBe("Item 6: cannot be filled from here: it is written through the form's own editors: the ATC popup.");
  // A value the box's type cannot hold names the kind, never the type.
  expect(refusalSentence(ctx("`k` = `-5` does not fit this box: not a 0605 draft: invalid value: integer `-5`, expected u16", { kind: "int" })))
    .toBe("Item 6: type a whole number, for example 3.");
});

test("B4: host validation keeps official alerts, rewrites 0605's four, nets leaks", () => {
  expect(hostSentence(ctx("Please enter a valid date on item 4.", { form: "0605" }), "due_date")).toBe("Item 4 needs a due date.");
  expect(hostSentence(ctx("Invalid date entry on item 7.", { form: "0605" }), "return_period")).toBe("Item 7 needs a return period.");
  expect(hostSentence(ctx("Please enter a valid ATC on Item 6.", { form: "0605" }), "atc")).toBe("Item 6: pick the ATC, for example FP010.");
  expect(hostSentence(ctx("Please select Tax Type Code on Item 8.", { form: "0605" }), "tax_type")).toBe("Item 8: pick the tax type code.");
  expect(hostSentence(ctx("Taxes withheld is required."), "taxes_withheld")).toBe("Taxes withheld is required.");
  expect(hostSentence(ctx("draft has no `/foo`"), "foo")).toBe("Item 6 needs a valid answer.");
  expect(stripOtherError("Other error: nope")).toBe("nope");
});
