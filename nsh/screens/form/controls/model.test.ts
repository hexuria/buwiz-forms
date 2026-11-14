// The controls' pure rules (model.ts): options, the shown choice, the TIN tell, word search, dates, row titles.
import { expect, test } from "nsh/test";
import type { ChoiceOption } from "../contract";
import { isOn, listedOptions, matchWords, mdy, optionOf, readText, rowTitleOf, selectModel, splitLabel, tinKind } from "./model";

const box = (value: string, options: ChoiceOption[] = [], zeroWhenBlank = false) =>
  ({ value: () => value, options: () => options, zeroWhenBlank }) as never;

test("optionOf: CODE - Label strings give the code as the value, template text is dropped", () => {
  expect(optionOf("03 - March")).toEqual({ value: "03", label: "03 - March" });
  expect(optionOf("Single")).toEqual({ value: "Single", label: "Single" });
  expect(optionOf('" + rdo.description + "')).toBeNull();
  expect(optionOf({ value: "WI010", label: "WI010 — Fees" })).toEqual({ value: "WI010", label: "WI010 — Fees" });
});

test("listedOptions: the host's options win over the schema's", () => {
  expect(listedOptions({ o: ["01 - A"] }, [{ value: "x", label: "X" }])).toEqual([{ value: "x", label: "X" }]);
  expect(listedOptions({ o: ["01 - A", '"x"'] }, [])).toEqual([{ value: "01", label: "01 - A" }]);
  expect(listedOptions({}, [])).toEqual([]);
});

test("selectModel: an empty choice first; zeros and a lone dash read as empty; an unknown value stays choosable", () => {
  const listed = [{ value: "01", label: "01 - January" }, { value: "02", label: "02 - February" }];
  expect(selectModel(listed, "02").shown).toBe("02");
  expect(selectModel(listed, "").shown).toBe("");
  expect(selectModel(listed, "00").shown).toBe("");
  expect(selectModel(listed, "-").shown).toBe("");
  expect(selectModel(listed, "-").options[0]).toEqual({ value: "", label: "Choose…" });
  const odd = selectModel(listed, "13");
  expect(odd.shown).toBe("13");
  expect(odd.options.at(-1)).toEqual({ value: "13", label: "13" });
  // A listed 0 stays selectable.
  expect(selectModel([{ value: "0", label: "0" }], "0").shown).toBe("0");
  // Two options sharing a code: the host's marked one is the chosen one.
  const twin = [{ value: "2000", label: "2000 — A" }, { value: "2000", label: "2000 — B", selected: true }];
  expect(selectModel(twin, "2000").shown).toBe("2000");
});

test("isOn: the old page's ticked values", () => {
  expect(["true", "1", "Y"].every(isOn)).toBe(true);
  expect(["false", "", "0", "N"].some(isOn)).toBe(false);
});

test("tinKind: whole 14-digit boxes and the 3-3-3-5 boxes are TINs, other boxes are not", () => {
  expect(tinKind({ k: "frm1707:txtPg1I7BuyerTIN0", l: "Buyer TIN0", t: "text", m: 14 })).toBe("full");
  expect(tinKind({ k: "frm1700:txtPg2Pt4TIN3", l: "TIN3", t: "text", m: 3 })).toBe("part");
  expect(tinKind({ k: "frm1701:txtPg3BranchCode", l: "Branch code", t: "text", m: 5 })).toBe("part");
  expect(tinKind({ k: "frm1601EQ:txtTIN1", l: "Taxpayer Identification Number", t: "int", m: 3 })).toBe("part");
  expect(tinKind({ k: "frm1702EX:txtPg1Pt2TINofSignatory", l: "TI nof signatory", t: "text", m: 14 })).toBe("full");
  // No length, no TIN: the box stays a text box.
  expect(tinKind({ k: "frm1600PT:txtPg1TIN1", l: "TIN1", t: "text" })).toBeNull();
  expect(tinKind({ k: "frm1:txtInterest", l: "Interest", t: "text", m: 14 })).toBeNull();
  expect(tinKind({ k: "frm1:txtOperatingLoss", l: "Net operating loss", t: "text", m: 12 })).toBeNull();
  expect(tinKind({ k: "frm1:txtTIN1", l: "TIN1", t: "money", m: 3 })).toBeNull();
});

test("mdy: a text that reads as a date is MM/DD/YYYY, anything else stays", () => {
  expect(mdy("12012020")).toBe("12/01/2020");
  expect(mdy("Dec 1, 2020")).toBe("12/01/2020");
  expect(mdy("2020-12-01")).toBe("12/01/2020");
  expect(mdy("13/45")).toBe("13/45");
  expect(mdy("")).toBe("");
});

test("readText: a read-only box shows an amount grouped (0.00 when blank and computed), a date long, a choice by its label", () => {
  const money = { t: "money", k: "a", l: "A" } as never;
  expect(readText({ field: money, box: box("1250000.5") })).toBe("1,250,000.50");
  expect(readText({ field: money, box: box("", [], true) })).toBe("0.00");
  expect(readText({ field: money, box: box("") })).toBe("");
  expect(readText({ field: { t: "money", w: 1, k: "a", l: "A" } as never, box: box("48000") })).toBe("48,000");
  expect(readText({ field: { t: "date", k: "d", l: "D" } as never, box: box("12/01/2020") })).toBe("Dec 1, 2020");
  const sel = { t: "sel", k: "s", l: "S", o: ["01 - January"] } as never;
  expect(readText({ field: sel, box: box("01") })).toBe("01 - January");
  expect(readText({ field: sel, box: box("") })).toBe("");
});

test("splitLabel and matchWords: every typed word must be in the label or code, in any order", () => {
  expect(splitLabel("WI010 — Professional fees")).toEqual(["WI010", "Professional fees"]);
  expect(splitLabel("050 - South Makati")).toEqual(["050", "South Makati"]);
  expect(splitLabel("Single")).toEqual(["Single", ""]);
  const rdo: ChoiceOption[] = [
    { value: "050", label: "050 - South Makati" },
    { value: "047", label: "047 - East Makati" },
    { value: "039", label: "039 - South Quezon City" },
  ];
  expect(matchWords(rdo, "").length).toBe(3);
  expect(matchWords(rdo, "makati south").map((o) => o.value)).toEqual(["050"]);
  expect(matchWords(rdo, "050 makati").map((o) => o.value)).toEqual(["050"]);
  expect(matchWords(rdo, "EAST").map((o) => o.value)).toEqual(["047"]);
  expect(matchWords(rdo, "zzz")).toEqual([]);
  expect(matchWords(rdo, "south", 1).length).toBe(1);
});

test("rowTitleOf: the row's printed number, with its caption after a dot", () => {
  expect(rowTitleOf({}, 0)).toBe("Row 1");
  expect(rowTitleOf({ start: 5 }, 2)).toBe("Row 7");
  expect(rowTitleOf({ row_labels: { caption: "Recipient", rows: ["Rent", "Fees"] } }, 1)).toBe("Row 2 · Fees");
});
