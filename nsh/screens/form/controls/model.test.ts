// The controls' pure rules (model.ts): options, the shown choice, the TIN tell, word search, dates, row titles.
import { expect, test } from "nsh/test";
import type { ChoiceOption } from "../contract";
import { drawerGroups, drawerTin, headFits, isOn, listedOptions, matchWords, mdy, optionOf, pickLabel, readText, resolveTyped, rowHead, rowTitleOf, searchKeyboardType, selectModel, splitLabel, tinKind } from "./model";

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
  expect(readText({ field: { t: "date", k: "d", l: "D" } as never, box: box("12/01/2020") })).toBe("December 1, 2020");
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

test("matchWords: an accent does not have to be typed, and typing it still matches", () => {
  const zip = [
    { value: "1100", label: "1100 - Central, Piñahan, Project 6" },
    { value: "1700", label: "1700 - Parañaque" },
    { value: "4114", label: "4114 - Dasmariñas" },
  ];
  expect(matchWords(zip, "pinahan").map((o) => o.value)).toEqual(["1100"]);
  expect(matchWords(zip, "Piñahan").map((o) => o.value)).toEqual(["1100"]);
  expect(matchWords(zip, "PARANAQUE").map((o) => o.value)).toEqual(["1700"]);
  expect(matchWords(zip, "dasmarinas 4114").map((o) => o.value)).toEqual(["4114"]);
  expect(matchWords(zip, "PIÑAHAN").map((o) => o.value)).toEqual(["1100"]);
});

test("rowTitleOf: the row's printed number, with its caption after a dot", () => {
  expect(rowTitleOf({}, 0)).toBe("Row 1");
  expect(rowTitleOf({ start: 5 }, 2)).toBe("Row 7");
  expect(rowTitleOf({ row_labels: { caption: "Recipient", rows: ["Rent", "Fees"] } }, 1)).toBe("Row 2 · Fees");
});

test("rowHead: on a phone the label keeps 160 pt, so Edit penalties wraps under it and a lone mark stays beside it", () => {
  // A 320 pt phone's row head is 262 pt wide; the number box is 30.
  expect(headFits(262, 30, 20)).toBe(true); // a mark
  expect(headFits(262, 30, 120)).toBe(false); // "Edit penalties"
  expect(headFits(262, 30, 20 + 6 + 62)).toBe(false); // a mark and "Edit"
  // The label's share beside the number is no narrower than the old 32 pt column + 8 pt gap left it.
  expect(30 + rowHead(true).gap).toBeLessThanOrEqual(40);
  expect(rowHead(false)).toEqual({ gap: 8, labelMin: undefined });
});

test("drawerGroups: a run of TIN digit columns is one group; a whole-TIN column and everything else stay single", () => {
  const cols = [
    { col: { l: "Taxpayer", t: "radio" }, k: "f:rdoT" },
    { col: { l: "a. Name of Employer", t: "text" }, k: "f:txtName" },
    { col: { l: "b. TIN (1-3)", t: "text" }, k: "f:txtEmployerTIN1" },
    { col: { l: "TIN (4-6)", t: "text" }, k: "f:txtEmployerTIN2" },
    { col: { l: "TIN (7-9)", t: "text" }, k: "f:txtEmployerTIN3" },
    { col: { l: "Branch code", t: "text" }, k: "f:txtEmployerBranchCode" },
    { col: { l: "e. Tax Withheld", t: "money" }, k: "f:txtTW" },
  ];
  expect(drawerGroups(cols, (c) => c.k).map((g) => g.length)).toEqual([1, 1, 4, 1]);
  const whole = [{ col: { l: "TIN (Including Branch Code)", t: "text" }, k: "f:txtTin1" }, { col: { l: "ATC", t: "text" }, k: "f:atc" }];
  expect(drawerGroups(whole, (c) => c.k).map((g) => g.length)).toEqual([1, 1]);
  expect(drawerTin(["b. TIN (1-3)", "TIN (4-6)", "TIN (7-9)", "Branch code"])).toEqual({ weights: [1, 1, 1, 1.4], title: "b. TIN" });
});

test("pickLabel: a code and its name joined by an em dash, a label with no code as it is", () => {
  expect(pickLabel({ label: "050 - South Makati" })).toBe("050 — South Makati");
  expect(pickLabel({ label: "WI010 — Professional fees" })).toBe("WI010 — Professional fees");
  expect(pickLabel({ label: "Single" })).toBe("Single");
});

test("resolveTyped: what a desktop pick takes from its box on leaving, and the sentence when it takes nothing", () => {
  const rdo: ChoiceOption[] = [
    { value: "", label: "Choose…" },
    { value: "050", label: "050 - South Makati" },
    { value: "047", label: "047 - East Makati" },
    { value: "039", label: "039 - South Quezon City" },
    { value: "081", label: "081 - Cebu City North", disabled: true },
  ];
  // The current choice's own words keep it; so does nothing typed with nothing chosen.
  expect(resolveTyped(rdo, "050 — South Makati", "050")).toEqual({ kind: "keep" });
  expect(resolveTyped(rdo, "  ", "")).toEqual({ kind: "keep" });
  // A code, a whole label (either dash, any case) or words that fit one choice pick it.
  expect(resolveTyped(rdo, "047", "050")).toEqual({ kind: "pick", value: "047" });
  expect(resolveTyped(rdo, "039 - south quezon city", "")).toEqual({ kind: "pick", value: "039" });
  expect(resolveTyped(rdo, "east mak", "050")).toEqual({ kind: "pick", value: "047" });
  // An emptied box clears a clearable choice; a required one goes back and says why.
  expect(resolveTyped(rdo, "", "050")).toEqual({ kind: "clear" });
  expect(resolveTyped(rdo, "", "050", false)).toEqual({ kind: "revert", note: "This choice can't be left empty. The box kept 050 — South Makati." });
  // Text that is no choice is not taken: the box keeps the last pick and the note names both.
  expect(resolveTyped(rdo, "xyz", "050")).toEqual({ kind: "revert", note: "“xyz” is not in the list. The box kept 050 — South Makati." });
  expect(resolveTyped(rdo, "xyz", "")).toEqual({ kind: "revert", note: "“xyz” is not in the list. The box stays empty." });
  // Text that fits several choices is not guessed.
  expect(resolveTyped(rdo, "makati", "039")).toEqual({
    kind: "revert",
    note: "“makati” fits 2 choices. Pick one from the list. The box kept 039 — South Quezon City.",
  });
  // A choice that takes no pick is not taken by typing either.
  expect(resolveTyped(rdo, "081", "050")).toEqual({ kind: "revert", note: "“081” can't be chosen here. The box kept 050 — South Makati." });
});

test("searchKeyboardType: a ZIP search opens on digits only where letters stay one key away", () => {
  expect(searchKeyboardType("digits-first", "ios")).toBe("numbers-and-punctuation");
  // Android's number keyboards have no letters: the ordinary keyboard, so a city can still be typed.
  expect(searchKeyboardType("digits-first", "android")).toBe("default");
  expect(searchKeyboardType("digits-first", "macos")).toBe("default");
  expect(searchKeyboardType("default", "ios")).toBe("default");
  expect(searchKeyboardType(undefined, "ios")).toBe("default");
});
