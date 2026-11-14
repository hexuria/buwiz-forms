import { describe, expect, test } from "nsh/test";
import {
  EOPT_TIERS,
  ELECTION_CHOICES,
  FACT_LABELS,
  FACT_WHY,
  ROW_FACTS,
  electionSourceText,
  factValue,
  incomeTypeChoices,
  joinNames,
  notSetForForm,
  notSetSentence,
  readerChips,
} from "./facts";

describe("factValue", () => {
  test("an income type reads as its words; a value the table does not name reads as it is", () => {
    expect(factValue("tax_classification", { tax_classification: "SelfEmployed" }, 2026)).toBe("Self-employed");
    expect(factValue("tax_classification", { tax_classification: "EstateOrTrust" }, 2026)).toBe("Estate or trust");
    expect(factValue("tax_classification", { tax_classification: "Something" }, 2026)).toBe("Something");
  });
  test("the rate reads with its year (from test_profile_facts: 8% for 2026)", () => {
    expect(factValue("income_tax_election", { income_tax_election: "EightPercent" }, 2026)).toBe("8% for 2026");
    expect(factValue("income_tax_election", { income_tax_election: "GraduatedOsd" }, 2026)).toBe(
      "Graduated rates, optional standard deduction for 2026",
    );
    expect(factValue("income_tax_election", { income_tax_election: "Other" }, 2026)).toBe("Other for 2026");
  });
  test("a yes or no fact reads Yes or No; false is No, not not set", () => {
    expect(factValue("is_government_withholding_entity", { is_government_withholding_entity: true }, 2026)).toBe("Yes");
    expect(factValue("is_government_withholding_entity", { is_government_withholding_entity: false }, 2026)).toBe("No");
  });
  test("the ATC codes read as a list joined with commas; no codes is not set", () => {
    expect(factValue("atc_codes", { atc_codes: ["II011", "WI010"] }, 2026)).toBe("II011, WI010");
    expect(factValue("atc_codes", { atc_codes: [] }, 2026)).toBeNull();
    expect(factValue("atc_codes", { atc_codes: null }, 2026)).toBeNull();
    expect(factValue("atc_codes", {}, 2026)).toBeNull();
  });
  test("a missing, null or empty fact is not set", () => {
    expect(factValue("birth_date", {}, 2026)).toBeNull();
    expect(factValue("birth_date", { birth_date: null }, 2026)).toBeNull();
    expect(factValue("birth_date", { birth_date: "" }, 2026)).toBeNull();
    expect(factValue("birth_date", null, 2026)).toBeNull();
    expect(factValue("birth_date", undefined, 2026)).toBeNull();
  });
  test("any other fact reads as its text", () => {
    expect(factValue("birth_date", { birth_date: "2000-01-02" }, 2026)).toBe("2000-01-02");
    expect(factValue("eopt_tier", { eopt_tier: "Micro" }, 2026)).toBe("Micro");
    expect(factValue("line_of_business", { line_of_business: "Retail" }, 2026)).toBe("Retail");
  });
});

describe("electionSourceText", () => {
  test("no source is no text", () => {
    expect(electionSourceText(null)).toBeNull();
    expect(electionSourceText(undefined)).toBeNull();
    expect(electionSourceText("")).toBeNull();
  });
  test("the profile says it was set there; a form says which form and period", () => {
    expect(electionSourceText("Tax profile")).toBe("Set on this profile");
    expect(electionSourceText("2551Q Q1")).toBe("Chosen on 2551Q Q1");
  });
});

describe("readerChips", () => {
  test("up to four forms are each a chip (the boundary is four)", () => {
    expect(readerChips("tax_classification", ["2551Q", "1701Q"])).toEqual(["2551Q", "1701Q"]);
    expect(readerChips("tax_classification", ["A", "B", "C", "D"])).toEqual(["A", "B", "C", "D"]);
  });
  test("five or more forms count, except the EOPT tier, which every return's penalties read", () => {
    expect(readerChips("tax_classification", ["A", "B", "C", "D", "E"])).toEqual(["5 forms"]);
    expect(readerChips("eopt_tier", ["A", "B", "C", "D", "E"])).toEqual(["penalties on every form"]);
  });
  test("no forms is no chips", () => {
    expect(readerChips("birth_date", [])).toEqual([]);
    expect(readerChips("birth_date", null)).toEqual([]);
  });
});

describe("joinNames", () => {
  test("zero, one, two, three and four names", () => {
    expect(joinNames([])).toBe("");
    expect(joinNames(["A"])).toBe("A");
    expect(joinNames(["A", "B"])).toBe("A and B");
    expect(joinNames(["A", "B", "C"])).toBe("A, B and C");
    expect(joinNames(["A", "B", "C", "D"])).toBe("A, B, C and D");
  });
});

describe("incomeTypeChoices", () => {
  test("an individual chooses self-employed, mixed income or compensation only", () => {
    expect(incomeTypeChoices("Individual")).toEqual([
      ["SelfEmployed", "Self-employed"],
      ["MixedIncome", "Mixed income"],
      ["PurelyCompensation", "Compensation only"],
    ]);
  });
  test("a cooperative chooses one of its three kinds", () => {
    expect(incomeTypeChoices("Cooperative")).toEqual([
      ["CooperativeTaxable", "Taxable cooperative"],
      ["CooperativeExempt", "Tax-exempt cooperative"],
      ["CooperativeMixed", "Mixed-income cooperative"],
    ]);
  });
  test("an estate or a trust has one choice; any other type gets the corporation choice", () => {
    expect(incomeTypeChoices("Estate")).toEqual([["EstateOrTrust", "Estate or trust"]]);
    expect(incomeTypeChoices("Trust")).toEqual([["EstateOrTrust", "Estate or trust"]]);
    expect(incomeTypeChoices("Corporation")).toEqual([["Corporation", "Corporation or partnership"]]);
    expect(incomeTypeChoices(undefined)).toEqual([["Corporation", "Corporation or partnership"]]);
  });
});

describe("constant lists and labels", () => {
  test("the four EOPT tiers, and the two rate choices", () => {
    expect(EOPT_TIERS.map(([value]) => value)).toEqual(["Micro", "Small", "Medium", "Large"]);
    expect(ELECTION_CHOICES).toEqual([
      ["EightPercent", "8%"],
      ["GraduatedUnspecified", "Graduated rates"],
    ]);
  });
  test("every fact has a label and a reason; the Facts section lists five", () => {
    expect(Object.keys(FACT_LABELS).length).toBe(8);
    expect(Object.keys(FACT_WHY).length).toBe(8);
    expect(ROW_FACTS).toEqual(["tax_classification", "income_tax_election", "eopt_tier", "is_government_withholding_entity", "atc_codes"]);
  });
});

describe("notSetForForm", () => {
  const view = {
    not_set: ["birth_date", "eopt_tier"] as const,
    used_by: { birth_date: ["1701"], eopt_tier: ["2551Q", "1701"] },
  };
  test("a fact is listed for each form that reads it", () => {
    expect(notSetForForm(view, "1701")).toEqual(["birth_date", "eopt_tier"]);
    expect(notSetForForm(view, "2551Q")).toEqual(["eopt_tier"]);
    expect(notSetForForm(view, "9999")).toEqual([]);
  });
  test("the rate is left out when the taxpayer does not choose one (the readiness is not ready)", () => {
    const rate = {
      not_set: ["income_tax_election", "birth_date"] as const,
      used_by: { income_tax_election: ["2551Q"], birth_date: ["2551Q"] },
    };
    expect(notSetForForm(rate, "2551Q", { gap: "ready" })).toEqual(["income_tax_election", "birth_date"]);
    expect(notSetForForm(rate, "2551Q", { gap: "not an individual with business income" })).toEqual(["birth_date"]);
    expect(notSetForForm(rate, "2551Q", null)).toEqual(["income_tax_election", "birth_date"]);
  });
  test("no view, or a view with no not-set facts, lists nothing", () => {
    expect(notSetForForm(null, "1701")).toEqual([]);
    expect(notSetForForm(undefined, "1701")).toEqual([]);
    expect(notSetForForm({}, "1701")).toEqual([]);
  });
});

describe("notSetSentence", () => {
  test("no facts is no sentence", () => {
    expect(notSetSentence([])).toBeNull();
  });
  test("one fact says what it is and why the return reads it", () => {
    expect(notSetSentence(["birth_date"])).toBe("Birth date is not set. It goes on this return's taxpayer details.");
  });
  test("two facts: the first keeps its capitals, a later one is lowered unless it starts with two capitals", () => {
    expect(notSetSentence(["birth_date", "eopt_tier"])).toBe("Birth date and EOPT tier are not set. This return reads them.");
    expect(notSetSentence(["eopt_tier", "birth_date"])).toBe("EOPT tier and birth date are not set. This return reads them.");
  });
  test("three facts join with commas and an and", () => {
    expect(notSetSentence(["tax_classification", "birth_date", "atc_codes"])).toBe(
      "Income type, birth date and usual ATC codes are not set. This return reads them.",
    );
  });
  test("a line of business first, then the EOPT tier", () => {
    expect(notSetSentence(["line_of_business", "eopt_tier"])).toBe("Line of business and EOPT tier are not set. This return reads them.");
  });
});
