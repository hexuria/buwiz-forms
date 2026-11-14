// The tax facts a year's forms read (facts.js; owner decisions 3, 4, 7), in the words the Tax profile page, the right
// panel and the New return dialog use. The facts themselves, who holds each and which forms read them come from the
// host (year_facts); this file only names them. Pure.

/** The facts a year's forms can read. */
export type FactName =
  | "birth_date"
  | "tax_classification"
  | "income_tax_election"
  | "eopt_tier"
  | "business_start_date"
  | "line_of_business"
  | "is_government_withholding_entity"
  | "atc_codes";

/** The year's facts as the host sends them: text, a yes or no, or the ATC codes. A fact not set is missing, null or empty. */
export type FactValues = Readonly<Partial<Record<FactName, unknown>>>;

/** What the forms read for a fact, per fact: the forms (codes) that use it, and whether the fact is not set. */
export interface NotSetView {
  not_set?: readonly FactName[] | null;
  used_by?: Readonly<Partial<Record<FactName, readonly string[]>>> | null;
}

/** The election readiness the host gives: "ready" when the rate can be recorded. */
export interface ElectionReadiness {
  gap?: string | null;
}

/** Each fact as the page names it. */
export const FACT_LABELS: Readonly<Record<FactName, string>> = {
  birth_date: "Birth date",
  tax_classification: "Income type",
  income_tax_election: "Income-tax rate",
  eopt_tier: "EOPT tier",
  business_start_date: "Business start date",
  line_of_business: "Line of business",
  is_government_withholding_entity: "Government withholding entity",
  atc_codes: "Usual ATC codes",
};

/** What a return does with a fact, for the line that says it is not set. */
export const FACT_WHY: Readonly<Record<FactName, string>> = {
  birth_date: "It goes on this return's taxpayer details.",
  tax_classification: "It decides which parts of this return apply.",
  income_tax_election: "It decides the income-tax rate on this return.",
  eopt_tier: "It sets the penalty rates on this return.",
  business_start_date: "This return asks for it.",
  line_of_business: "It goes on this return.",
  is_government_withholding_entity: "It decides how this return is filled in.",
  atc_codes: "It fills the tax codes on this return.",
};

/** The facts the Facts section lists, in order. Birth date, line of business and start date have their own sections. */
export const ROW_FACTS: readonly FactName[] = [
  "tax_classification",
  "income_tax_election",
  "eopt_tier",
  "is_government_withholding_entity",
  "atc_codes",
];

/** The income types a taxpayer type chooses from, as bir-core names them: [value, label] pairs. */
export function incomeTypeChoices(taxpayerType: string | null | undefined): [string, string][] {
  if (taxpayerType === "Individual") {
    return [
      ["SelfEmployed", "Self-employed"],
      ["MixedIncome", "Mixed income"],
      ["PurelyCompensation", "Compensation only"],
    ];
  }
  if (taxpayerType === "Cooperative") {
    return [
      ["CooperativeTaxable", "Taxable cooperative"],
      ["CooperativeExempt", "Tax-exempt cooperative"],
      ["CooperativeMixed", "Mixed-income cooperative"],
    ];
  }
  if (taxpayerType === "Estate" || taxpayerType === "Trust") return [["EstateOrTrust", "Estate or trust"]];
  return [["Corporation", "Corporation or partnership"]];
}

/** Every income type's label, by its value. */
const INCOME_TYPE_LABELS: Readonly<Record<string, string>> = Object.fromEntries(
  ["Individual", "Cooperative", "Estate", "Corporation"].flatMap((type) => incomeTypeChoices(type)),
);

/** The four EOPT tiers, as [value, label] pairs. */
export const EOPT_TIERS: readonly [string, string][] = [
  ["Micro", "Micro"],
  ["Small", "Small"],
  ["Medium", "Medium"],
  ["Large", "Large"],
];

/** The two choices of the rate row; a graduated method refines "Graduated". */
export const ELECTION_CHOICES: readonly [string, string][] = [
  ["EightPercent", "8%"],
  ["GraduatedUnspecified", "Graduated rates"],
];

/** Every rate method's label, by its value. */
const ELECTION_LABELS: Readonly<Record<string, string>> = {
  EightPercent: "8%",
  GraduatedUnspecified: "Graduated rates",
  GraduatedOsd: "Graduated rates, optional standard deduction",
  GraduatedItemized: "Graduated rates, itemized deductions",
};

/** A fact's value in words, or null when it is not set. */
export function factValue(fact: FactName, facts: FactValues | null | undefined, year: number): string | null {
  const v = facts ? facts[fact] : null;
  if (fact === "atc_codes") return Array.isArray(v) && v.length ? v.join(", ") : null;
  if (v === null || v === undefined || v === "") return null;
  if (fact === "tax_classification") return INCOME_TYPE_LABELS[String(v)] || String(v);
  if (fact === "income_tax_election") return `${ELECTION_LABELS[String(v)] || String(v)} for ${year}`;
  if (fact === "is_government_withholding_entity") return v ? "Yes" : "No";
  return String(v);
}

/** Where the year's rate was chosen, in words. */
export function electionSourceText(source: string | null | undefined): string | null {
  if (!source) return null;
  return source === "Tax profile" ? "Set on this profile" : `Chosen on ${source}`;
}

/**
 * The forms that read a fact, as chip labels: each code, or one chip that counts them when there are many ("penalties on
 * every form" for the EOPT tier, which every return's penalties read).
 */
export function readerChips(fact: FactName, codes: readonly string[] | null | undefined): readonly string[] {
  const list = codes || [];
  if (list.length <= 4) return list;
  return [fact === "eopt_tier" ? "penalties on every form" : `${list.length} forms`];
}

/** A, A and B, A, B and C. */
export function joinNames(names: readonly string[]): string {
  if (names.length < 2) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
}

/**
 * The facts a form reads that are not set for the year. The rate is left out when the taxpayer does not choose one
 * (the readiness says it is not an individual with business income).
 */
export function notSetForForm(view: NotSetView | null | undefined, code: string, readiness?: ElectionReadiness | null): FactName[] {
  if (!view) return [];
  return (view.not_set || []).filter(
    (f) => (view.used_by?.[f] || []).includes(code) && !(f === "income_tax_election" && readiness && readiness.gap !== "ready"),
  );
}

/** The one line that names the facts a return reads that are not set, or null. */
export function notSetSentence(facts: readonly FactName[]): string | null {
  if (!facts.length) return null;
  if (facts.length === 1) return `${FACT_LABELS[facts[0]]} is not set. ${FACT_WHY[facts[0]]}`;
  // "EOPT tier" keeps its capitals after the first name; "Birth date" does not.
  const lower = (text: string) => (/^[A-Z][a-z]/.test(text) ? text[0].toLowerCase() + text.slice(1) : text);
  const names = facts.map((f, i) => (i === 0 ? FACT_LABELS[f] : lower(FACT_LABELS[f])));
  return `${joinNames(names)} are not set. This return reads them.`;
}
