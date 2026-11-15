// The Form library's rules (library.ts), pure: no screen and no door.
import { expect, test } from "nsh/test";
import type { ScreenFormIndexEntry } from "../../bir";
import { asFrequency, type FormFilter, FREQS, matchingForms, searchText } from "./rules";

const form = (c: string, t: string, freq: string, q = 1): ScreenFormIndexEntry => ({ c, t, rev: "R1", freq, pages: 2, n: 10, q });

const FORMS: ScreenFormIndexEntry[] = [
  form("1601C", "Monthly Remittance Return of Income Taxes Withheld on Compensation", "Monthly"),
  form("2550Q", "Quarterly Value-Added Tax Return", "Quarterly", 0.5),
  form("2551Q", "Quarterly Percentage Tax Return", "Quarterly"),
  form("1702-RT", "Annual Income Tax Return", "Annual"),
  form("0605", "Payment Form", "As needed"),
];
const OWNED = new Set(["1601C", "2551Q"]);
const BASE: FormFilter = { scope: "all", owned: OWNED, freq: "All", query: "" };
const codes = (filter: Partial<FormFilter>) => matchingForms(FORMS, { ...BASE, ...filter }).map((f) => f.c);

test("the frequency choices keep the old order, and an unknown choice is All", () => {
  expect([...FREQS]).toEqual(["All", "Monthly", "Quarterly", "Annual", "As needed"]);
  expect(asFrequency("Annual")).toBe("Annual");
  expect(asFrequency("Weekly")).toBe("All");
});

test("a search reads the code with and without dashes, the title, and VAT for the Value-Added forms", () => {
  expect(searchText(FORMS[3])).toBe("1702-rt 1702rt annual income tax return");
  expect(searchText(FORMS[1])).toBe("2550q 2550q quarterly value-added tax return vat");
  expect(searchText(FORMS[2])).not.toContain("vat");
});

test("scope: owned shows the forms filed this year, all shows every form", () => {
  expect(codes({ scope: "owned" })).toEqual(["1601C", "2551Q"]);
  expect(codes({ scope: "all" })).toHaveLength(5);
});

test("the frequency narrows within the scope", () => {
  expect(codes({ freq: "Quarterly" })).toEqual(["2550Q", "2551Q"]);
  expect(codes({ scope: "owned", freq: "Quarterly" })).toEqual(["2551Q"]);
  expect(codes({ scope: "owned", freq: "Annual" })).toEqual([]);
});

test("the search trims, ignores case, finds a code without its dashes, and VAT finds the Value-Added forms", () => {
  expect(codes({ query: "  1702 " })).toEqual(["1702-RT"]);
  expect(codes({ query: "1702rt" })).toEqual(["1702-RT"]);
  expect(codes({ query: "ANNUAL income" })).toEqual(["1702-RT"]);
  expect(codes({ query: "vat" })).toEqual(["2550Q"]);
  expect(codes({ query: "zzz" })).toEqual([]);
  expect(codes({ query: "   " })).toHaveLength(5);
});

test("scope, frequency and search together", () => {
  expect(codes({ scope: "owned", freq: "Monthly", query: "remittance" })).toEqual(["1601C"]);
  expect(codes({ scope: "owned", freq: "Monthly", query: "vat" })).toEqual([]);
});
