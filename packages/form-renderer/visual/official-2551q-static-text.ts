/**
 * `static-text-exhaustive-v1` — the content-correctness component of
 * `official-fidelity-v1`.
 *
 * WHY THIS FILE IS LOAD-BEARING
 * -----------------------------
 * Every pixel component of the visual gate is blind to what the text says.
 * Replacing each statutory tax rate on 2551Q page 2 with a wrong value measured
 * a max per-region regression of 0.19e-4 and violated no existing assertion.
 * The official PDFs do not embed their fonts, so text pixels encode the
 * rasterizer's substituted outlines and can never carry a content proof.
 * On a tax return, this manifest is what stands between the project and
 * shipping a form with a wrong statutory rate.
 *
 * PROVENANCE — read before editing an entry
 * -----------------------------------------
 *  - `text` is transcribed from the pinned official PDF text layer
 *    (sha256 1f270ecf…9b24), reproduced by
 *    `packages/form-renderer/references/2551q-2018-official-static-text.json`,
 *    which `scripts/generate_static_text_manifest.py` derives from that PDF.
 *    It is NEVER read out of our own DOM: a DOM-derived manifest would assert
 *    only that the renderer equals itself.
 *  - `selector` and `order` are DOM addresses and DOM sequence. They locate a
 *    reviewed string; they do not define it.
 *
 * ORDER SEMANTICS. `order` is the order the strings appear in the rendered
 * page's text, not the geometric order of the PDF. The two cannot coincide: a
 * two-column masthead prints "Republic of the Philippines" between "For BIR"
 * and "Use Only" on the page, while the DOM emits each label whole. A single
 * linear sequence has to pick one, and only the DOM sequence is checkable
 * against `innerText`. Content still comes from the PDF; only the sequence is
 * ours, and a reviewer confirms it against the printed page.
 */

export type OfficialStaticTextKind =
  | "masthead"
  | "item"
  | "instruction"
  | "section"
  | "signature"
  | "choice"
  | "table-heading"
  | "table-entry"
  | "tax-rate"
  | "barcode-caption";

export type OfficialStaticTextEntry = Readonly<{
  id: string;
  page: 1 | 2;
  /**
   * Position in the rendered page's text stream, ascending, unique per page.
   * Consumed by `verifyStaticTextExhaustive` to close order-blindness.
   */
  order: number;
  kind: OfficialStaticTextKind;
  /** Transcribed from the pinned official PDF. Never scraped from the DOM. */
  text: string;
  /**
   * Mandatory. Scopes `verifyPageIndexedStaticText` to the element that is
   * supposed to carry the string, so a label satisfied by an unrelated part of
   * the page can no longer pass.
   */
  selector: string;
  /**
   * Set only where the official PDF's literal differs from `text` by
   * whitespace alone — the official 2551Q prints "18 %" for PT140/PT150 and
   * "10 %" / "15 %" / "30 %" for PT160/PT170/PT180 while every other rate is
   * printed tight. `officialText` records what the PDF prints; a unit test
   * asserts the two differ by whitespace only, so a wrong *value* can never
   * hide in this field.
   */
  officialText?: string;
}>;

export type StaticTextPageSnapshot = Readonly<{
  fullText: string;
  /**
   * Page text with fixture-supplied glyphs removed — the same fixture-owned
   * set the pixel gate blanks (`prepareOfficialBlankComparison`). Absent
   * snapshots fall back to `fullText`.
   */
  staticText?: string;
  selectorText?: Readonly<Record<string, string>>;
}>;

export type StaticTextViolation = Readonly<{
  id: string;
  expectedPage: number;
  text: string;
  foundOnPages: readonly number[];
}>;

export type ExhaustiveStaticTextViolation =
  | Readonly<{ kind: "missing-or-reordered"; id: string; page: number; text: string; foundEarlierAt: number | null }>
  | Readonly<{ kind: "unexpected-residual"; page: number; tokens: readonly string[] }>;

export type StaticTextCompletenessViolation = Readonly<{
  kind: "unmanifested-element";
  page: number;
  selector: string;
  text: string;
  /** The tokens no reviewed entry accounts for; names the defect precisely. */
  unaccounted: readonly string[];
}>;

/**
 * Tokens the ordered walk may leave unconsumed. These are the structural
 * glyphs the comb/money scaffolding prints between labels — a decimal point, a
 * percent sign, a TIN separator. Any other leftover token is a string that
 * reached the page without review, which is exactly the addition-blindness
 * this component exists to close, so the list is deliberately this short.
 */
export const OFFICIAL_2551Q_ALLOWED_RESIDUAL: readonly string[] = [".", "%", "-"];

/**
 * The official PDF every `text` below was transcribed from. Byte-locked in
 * `crates/bir-print/src/html_forms/form_2551q.rs` as `official_source_sha256`
 * and re-derived by `scripts/generate_static_text_manifest.py`.
 */
export const OFFICIAL_2551Q_SOURCE_SHA256 =
  "1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24";

/**
 * Human-reviewed against the pinned January 2018 2551Q PDF. Ordered by the
 * rendered page's text stream (see ORDER SEMANTICS above). These strings are
 * evidence only; the runtime renderer never reads them.
 */
export const OFFICIAL_2551Q_STATIC_TEXT: readonly OfficialStaticTextEntry[] = [
  {
    id: "p1-bir-use-only",
    page: 1,
    order: 1,
    kind: "masthead",
    text: "For BIR Use Only",
    selector: ".bir-use-only"
  },
  {
    id: "p1-bcs-item",
    page: 1,
    order: 2,
    kind: "masthead",
    text: "BCS/ Item",
    selector: ".bcs-item"
  },
  {
    id: "p1-government",
    page: 1,
    order: 3,
    kind: "masthead",
    text: "Republic of the Philippines Department of Finance Bureau of Internal Revenue",
    selector: ".government-wordmark > strong"
  },
  {
    id: "p1-form-number",
    page: 1,
    order: 4,
    kind: "masthead",
    text: "BIR Form No. 2551Q January 2018 (ENCS) Page 1",
    selector: ".official-form-number"
  },
  {
    id: "p1-title",
    page: 1,
    order: 5,
    kind: "masthead",
    text: "Quarterly Percentage Tax Return",
    selector: ".official-form-title > strong"
  },
  {
    id: "p1-filing-instruction",
    page: 1,
    order: 6,
    kind: "instruction",
    text: "Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable boxes with an “X”. Two copies MUST be filed with the BIR and one held by the Taxpayer.",
    selector: ".official-form-title > em"
  },
  {
    id: "p1-barcode-caption",
    page: 1,
    order: 7,
    kind: "barcode-caption",
    text: "2551Q 01/18ENCS P1",
    selector: ".official-barcode > small"
  },
  {
    id: "p1-item-1",
    page: 1,
    order: 8,
    kind: "item",
    text: "1 For the",
    selector: ".filing-basis > .option-label:not(.year-label)"
  },
  {
    id: "p1-item-1-calendar",
    page: 1,
    order: 9,
    kind: "choice",
    text: "Calendar",
    selector: ".filing-basis > .option-choices > .check-choice:nth-of-type(1)"
  },
  {
    id: "p1-item-1-fiscal",
    page: 1,
    order: 10,
    kind: "choice",
    text: "Fiscal",
    selector: ".filing-basis > .option-choices > .check-choice:nth-of-type(2)"
  },
  {
    id: "p1-item-2",
    page: 1,
    order: 11,
    kind: "item",
    text: "2 Year Ended (MM/YYYY)",
    selector: ".filing-basis > .year-label"
  },
  {
    id: "p1-item-3",
    page: 1,
    order: 12,
    kind: "item",
    text: "3 Quarter",
    selector: ".quarter-options > .option-label"
  },
  {
    id: "p1-item-3-options",
    page: 1,
    order: 13,
    kind: "choice",
    text: "1st",
    selector: ".quarter-options > .option-choices > .check-choice:nth-of-type(1)"
  },
  {
    id: "p1-item-3-option-2",
    page: 1,
    order: 14,
    kind: "choice",
    text: "2nd",
    selector: ".quarter-options > .option-choices > .check-choice:nth-of-type(2)"
  },
  {
    id: "p1-item-3-option-3",
    page: 1,
    order: 15,
    kind: "choice",
    text: "3rd",
    selector: ".quarter-options > .option-choices > .check-choice:nth-of-type(3)"
  },
  {
    id: "p1-item-3-option-4",
    page: 1,
    order: 16,
    kind: "choice",
    text: "4th",
    selector: ".quarter-options > .option-choices > .check-choice:nth-of-type(4)"
  },
  {
    id: "p1-item-4",
    page: 1,
    order: 17,
    kind: "item",
    text: "4 Amended Return?",
    selector: ".amended-options > .option-label"
  },
  {
    id: "p1-item-4-yes",
    page: 1,
    order: 18,
    kind: "choice",
    text: "Yes",
    selector: ".amended-options > .option-choices > .check-choice:nth-of-type(1)"
  },
  {
    id: "p1-item-4-no",
    page: 1,
    order: 19,
    kind: "choice",
    text: "No",
    selector: ".amended-options > .option-choices > .check-choice:nth-of-type(2)"
  },
  {
    id: "p1-item-5",
    page: 1,
    order: 20,
    kind: "item",
    text: "5 Number of Sheet/s",
    selector: ".sheets-options > .option-label"
  },
  {
    id: "p1-item-5-attached",
    page: 1,
    order: 21,
    kind: "item",
    text: "Attached",
    selector: ".sheets-value > span"
  },
  {
    id: "p1-part-i",
    page: 1,
    order: 22,
    kind: "section",
    text: "Part I – Background Information",
    selector: ".background-information > h2"
  },
  {
    id: "p1-item-6",
    page: 1,
    order: 23,
    kind: "item",
    text: "6 Taxpayer Identification Number (TIN)",
    selector: ".tin-rdo-row > .field-label"
  },
  {
    id: "p1-item-7",
    page: 1,
    order: 24,
    kind: "item",
    text: "7 RDO Code",
    selector: ".tin-rdo-row > .rdo-label"
  },
  {
    id: "p1-item-8",
    page: 1,
    order: 25,
    kind: "item",
    text: "8 Taxpayer’s Name (Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)",
    selector: ".name-field > .field-label"
  },
  {
    id: "p1-item-9",
    page: 1,
    order: 26,
    kind: "item",
    text: "9 Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)",
    selector: ".address-field > .field-label"
  },
  {
    id: "p1-item-9a",
    page: 1,
    order: 27,
    kind: "item",
    text: "9A ZIP Code",
    selector: ".address-continuation > .zip-label"
  },
  {
    id: "p1-item-10",
    page: 1,
    order: 28,
    kind: "item",
    text: "10 Contact Number (Landline/Cellphone No.)",
    selector: ".contact-email-field > .field-label:nth-of-type(1)"
  },
  {
    id: "p1-item-11",
    page: 1,
    order: 29,
    kind: "item",
    text: "11 Email Address",
    selector: ".contact-email-field > .field-label:nth-of-type(2)"
  },
  {
    id: "p1-item-12",
    page: 1,
    order: 30,
    kind: "item",
    text: "12 Are you availing of tax relief under Special Law or International Tax Treaty?",
    selector: ".tax-relief-field > .field-label"
  },
  {
    id: "p1-item-12-yes",
    page: 1,
    order: 31,
    kind: "choice",
    text: "Yes",
    selector: ".tax-relief-field > .relief-choices > .check-choice:nth-of-type(1)"
  },
  {
    id: "p1-item-12-no",
    page: 1,
    order: 32,
    kind: "choice",
    text: "No",
    selector: ".tax-relief-field > .relief-choices > .check-choice:nth-of-type(2)"
  },
  {
    id: "p1-item-12a",
    page: 1,
    order: 33,
    kind: "item",
    text: "12A If yes, specify",
    selector: ".tax-relief-spec"
  },
  {
    id: "p1-item-13",
    page: 1,
    order: 34,
    kind: "item",
    text: "13 Only for individual taxpayers whose sales/receipts are subject to Percentage Tax under Section 116 of the Tax Code, as amended:",
    selector: ".income-rate-field"
  },
  {
    id: "p1-item-13-question",
    page: 1,
    order: 35,
    kind: "instruction",
    text: "What income tax rates are you availing? (choose one)",
    selector: ".income-rate-question"
  },
  {
    id: "p1-item-13-initial-quarter",
    page: 1,
    order: 36,
    kind: "instruction",
    text: "(To be filled out only on the initial quarter of the taxable year)",
    selector: ".income-rate-note"
  },
  {
    id: "p1-item-13-graduated",
    page: 1,
    order: 37,
    kind: "choice",
    text: "Graduated income tax rate on net taxable income",
    selector: ".income-rate-choice.graduated > .check-choice"
  },
  {
    id: "p1-item-13-eight-percent",
    page: 1,
    order: 38,
    kind: "choice",
    text: "8% income tax rate on gross sales/receipts/others",
    selector: ".income-rate-choice.eight-percent > .check-choice"
  },
  {
    id: "p1-part-ii",
    page: 1,
    order: 39,
    kind: "section",
    text: "Part II – Total Tax Payable",
    selector: ".tax-payable > h2"
  },
  {
    id: "p1-item-14",
    page: 1,
    order: 40,
    kind: "item",
    text: "14 Total Tax Due (From Schedule 1 Item 7)",
    selector: "[data-item='14'] > .tax-line-label"
  },
  {
    id: "p1-less-credit",
    page: 1,
    order: 41,
    kind: "instruction",
    text: "Less: Tax Credit/Payment (attach proof)",
    selector: ".tax-subheading:not(.penalties)"
  },
  {
    id: "p1-item-15",
    page: 1,
    order: 42,
    kind: "item",
    text: "15 Creditable Percentage Tax Withheld per BIR Form No. 2307",
    selector: "[data-item='15'] > .tax-line-label"
  },
  {
    id: "p1-item-16",
    page: 1,
    order: 43,
    kind: "item",
    text: "16 Tax Paid in Return Previously Filed, if this is an Amended Return",
    selector: "[data-item='16'] > .tax-line-label"
  },
  {
    id: "p1-item-17",
    page: 1,
    order: 44,
    kind: "item",
    text: "17 Other Tax Credit/Payment (specify)",
    selector: "[data-item='17'] .tax-line-primary"
  },
  {
    id: "p1-item-18",
    page: 1,
    order: 45,
    kind: "item",
    text: "18 Total Tax Credits/Payments (Sum of Items 15 to 17)",
    selector: "[data-item='18'] > .tax-line-label"
  },
  {
    id: "p1-item-19",
    page: 1,
    order: 46,
    kind: "item",
    text: "19 Tax Still Payable/(Overpayment) (Item 14 Less Item 18)",
    selector: "[data-item='19'] > .tax-line-label"
  },
  {
    id: "p1-add-penalties",
    page: 1,
    order: 47,
    kind: "instruction",
    text: "Add: Penalties",
    selector: ".tax-subheading.penalties"
  },
  {
    id: "p1-item-20",
    page: 1,
    order: 48,
    kind: "item",
    text: "20 Surcharge",
    selector: "[data-item='20'] > .tax-line-label"
  },
  {
    id: "p1-item-21",
    page: 1,
    order: 49,
    kind: "item",
    text: "21 Interest",
    selector: "[data-item='21'] > .tax-line-label"
  },
  {
    id: "p1-item-22",
    page: 1,
    order: 50,
    kind: "item",
    text: "22 Compromise",
    selector: "[data-item='22'] > .tax-line-label"
  },
  {
    id: "p1-item-23",
    page: 1,
    order: 51,
    kind: "item",
    text: "23 Total Penalties (Sum of Items 20 to 22)",
    selector: "[data-item='23'] > .tax-line-label"
  },
  {
    id: "p1-item-24",
    page: 1,
    order: 52,
    kind: "item",
    text: "24 TOTAL AMOUNT PAYABLE/(Overpayment) (Sum of Items 19 and 23)",
    selector: "[data-item='24'] > .tax-line-label"
  },
  {
    id: "p1-overpayment",
    page: 1,
    order: 53,
    kind: "instruction",
    text: "If overpayment, mark one box only:",
    selector: ".overpayment-options > span:nth-of-type(1)"
  },
  {
    id: "p1-refunded",
    page: 1,
    order: 54,
    kind: "choice",
    text: "To be refunded",
    selector: ".overpayment-options > .check-choice:nth-of-type(2)"
  },
  {
    id: "p1-tax-credit-certificate",
    page: 1,
    order: 55,
    kind: "choice",
    text: "To be issued a Tax Credit Certificate",
    selector: ".overpayment-options > .check-choice:nth-of-type(3)"
  },
  {
    id: "p1-declaration",
    page: 1,
    order: 56,
    kind: "instruction",
    text: "I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as contemplated under the “Data Privacy Act of 2012 (R.A. No. 10173)” for legitimate and lawful purposes. (If Authorized Representative, attach authorization letter)",
    selector: ".official-declaration > p"
  },
  {
    id: "p1-for-individual",
    page: 1,
    order: 57,
    kind: "signature",
    text: "For Individual:",
    selector: ".official-signature-grid > div:nth-of-type(1) > .signature-space"
  },
  {
    id: "p1-individual-signature",
    page: 1,
    order: 58,
    kind: "signature",
    text: "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent",
    selector: ".official-signature-grid > div:nth-of-type(1) > .signature-caption > b"
  },
  {
    id: "p1-signature-instruction",
    page: 1,
    order: 59,
    kind: "signature",
    text: "(Indicate title/designation and TIN)",
    selector: ".official-signature-grid > div:nth-of-type(1) > .signature-caption > em"
  },
  {
    id: "p1-for-non-individual",
    page: 1,
    order: 60,
    kind: "signature",
    text: "For Non-Individual:",
    selector: ".official-signature-grid > div:nth-of-type(2) > .signature-space"
  },
  {
    id: "p1-non-individual-signature",
    page: 1,
    order: 61,
    kind: "signature",
    text: "Signature over Printed Name of President/Vice President/ Authorized Officer or Representative/Tax Agent",
    selector: ".official-signature-grid > div:nth-of-type(2) > .signature-caption > b"
  },
  {
    id: "p1-signature-instruction-non-individual",
    page: 1,
    order: 62,
    kind: "signature",
    text: "(Indicate title/designation and TIN)",
    selector: ".official-signature-grid > div:nth-of-type(2) > .signature-caption > em"
  },
  {
    id: "p1-tax-agent",
    page: 1,
    order: 63,
    kind: "signature",
    text: "Tax Agent Accreditation No./ Attorney’s Roll No. (If applicable)",
    selector: ".tax-agent-strip > span:nth-of-type(1)"
  },
  {
    id: "p1-date-of-issue",
    page: 1,
    order: 64,
    kind: "signature",
    text: "Date of Issue (MM/DD/YYYY)",
    selector: ".tax-agent-strip > span:nth-of-type(3)"
  },
  {
    id: "p1-expiry-date",
    page: 1,
    order: 65,
    kind: "signature",
    text: "Expiry Date (MM/DD/YYYY)",
    selector: ".tax-agent-strip > span:nth-of-type(5)"
  },
  {
    id: "p1-part-iii",
    page: 1,
    order: 66,
    kind: "section",
    text: "Part III – Details of Payment",
    selector: ".official-payment-details > h2"
  },
  {
    id: "p1-payment-particulars",
    page: 1,
    order: 67,
    kind: "table-heading",
    text: "Particulars",
    selector: ".payment-headings > b:nth-of-type(1)"
  },
  {
    id: "p1-payment-drawee",
    page: 1,
    order: 68,
    kind: "table-heading",
    text: "Drawee Bank/ Agency",
    selector: ".payment-headings > b:nth-of-type(2)"
  },
  {
    id: "p1-payment-number",
    page: 1,
    order: 69,
    kind: "table-heading",
    text: "Number",
    selector: ".payment-headings > b:nth-of-type(3)"
  },
  {
    id: "p1-payment-date",
    page: 1,
    order: 70,
    kind: "table-heading",
    text: "Date (MM/DD/YYYY)",
    selector: ".payment-headings > b:nth-of-type(4)"
  },
  {
    id: "p1-payment-amount",
    page: 1,
    order: 71,
    kind: "table-heading",
    text: "Amount",
    selector: ".payment-headings > b:nth-of-type(5)"
  },
  {
    id: "p1-item-25",
    page: 1,
    order: 72,
    kind: "item",
    text: "25 Cash/Bank Debit Memo",
    selector: ".payment-row-25 > span:nth-of-type(1)"
  },
  {
    id: "p1-item-26",
    page: 1,
    order: 73,
    kind: "item",
    text: "26 Check",
    selector: ".payment-row-26 > span:nth-of-type(1)"
  },
  {
    id: "p1-item-27",
    page: 1,
    order: 74,
    kind: "item",
    text: "27 Tax Debit Memo",
    selector: ".payment-row-27 > span:nth-of-type(1)"
  },
  {
    id: "p1-item-28",
    page: 1,
    order: 75,
    kind: "item",
    text: "28 Others (Specify below)",
    selector: ".payment-other-label"
  },
  {
    id: "p1-machine-validation",
    page: 1,
    order: 76,
    kind: "instruction",
    text: "Machine Validation/Revenue Official Receipt (ROR) Details (if not filed with an Authorized Agent Bank)",
    selector: ".machine-validation > span:nth-of-type(1)"
  },
  {
    id: "p1-receiving-stamp",
    page: 1,
    order: 77,
    kind: "signature",
    text: "Stamp of receiving Office/AAB and Date of Receipt (RO’s Signature/Bank Teller’s Initial)",
    selector: ".machine-validation > span:nth-of-type(2)"
  },
  {
    id: "p1-privacy-note",
    page: 1,
    order: 78,
    kind: "instruction",
    text: "*NOTE: Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)",
    selector: ".privacy-note"
  },

  {
    id: "p2-form-number",
    page: 2,
    order: 79,
    kind: "masthead",
    text: "BIR Form No. 2551Q January 2018 (ENCS) Page 2",
    selector: ".page-two-form-number"
  },
  {
    id: "p2-title",
    page: 2,
    order: 80,
    kind: "masthead",
    text: "Quarterly Percentage Tax Return",
    selector: ".page-two-form-title > strong"
  },
  {
    id: "p2-barcode-caption",
    page: 2,
    order: 81,
    kind: "barcode-caption",
    text: "2551Q 01/18ENCS P2",
    selector: ".page-two-barcode > small"
  },
  {
    id: "p2-tin",
    page: 2,
    order: 82,
    kind: "table-heading",
    text: "TIN",
    selector: ".page-two-identity-label:not(.taxpayer-name-label)"
  },
  {
    id: "p2-taxpayer-name",
    page: 2,
    order: 83,
    kind: "table-heading",
    text: "Taxpayer’s Last Name (if Individual) / Registered Name (if Non-Individual)",
    selector: ".taxpayer-name-label"
  },
  {
    id: "p2-schedule-title",
    page: 2,
    order: 84,
    kind: "section",
    text: "Schedule 1 – Computation of Tax (Attach additional sheet/s, if necessary)",
    selector: ".official-schedule > h2"
  },
  {
    id: "p2-atc-heading",
    page: 2,
    order: 85,
    kind: "table-heading",
    text: "Alphanumeric Tax Code (ATC)",
    selector: ".official-schedule-head > span:nth-of-type(1)"
  },
  {
    id: "p2-taxable-heading",
    page: 2,
    order: 86,
    kind: "table-heading",
    text: "Taxable Amount",
    selector: ".official-schedule-head > span:nth-of-type(2)"
  },
  {
    id: "p2-rate-heading",
    page: 2,
    order: 87,
    kind: "table-heading",
    text: "Tax Rate",
    selector: ".official-schedule-head > span:nth-of-type(3)"
  },
  {
    id: "p2-tax-due-heading",
    page: 2,
    order: 88,
    kind: "table-heading",
    text: "Tax Due",
    selector: ".official-schedule-head > span:nth-of-type(4)"
  },
  {
    id: "p2-item-1",
    page: 2,
    order: 89,
    kind: "item",
    text: "1",
    selector: ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-2",
    page: 2,
    order: 90,
    kind: "item",
    text: "2",
    selector: ".official-schedule-row[data-row-slot='2'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-3",
    page: 2,
    order: 91,
    kind: "item",
    text: "3",
    selector: ".official-schedule-row[data-row-slot='3'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-4",
    page: 2,
    order: 92,
    kind: "item",
    text: "4",
    selector: ".official-schedule-row[data-row-slot='4'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-5",
    page: 2,
    order: 93,
    kind: "item",
    text: "5",
    selector: ".official-schedule-row[data-row-slot='5'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-6",
    page: 2,
    order: 94,
    kind: "item",
    text: "6",
    selector: ".official-schedule-row[data-row-slot='6'] > .official-schedule-row-number"
  },
  {
    id: "p2-item-7",
    page: 2,
    order: 95,
    kind: "item",
    text: "7 Total Tax Due (Sum of Items 1 to 6)(To Part II Item 14)",
    selector: ".official-schedule-total-label"
  },
  {
    id: "p2-table-1",
    page: 2,
    order: 96,
    kind: "table-heading",
    text: "Table 1 – Alphanumeric Tax Code (ATC)",
    selector: ".official-atc-table > caption"
  },
  {
    id: "p2-table-atc",
    page: 2,
    order: 97,
    kind: "table-heading",
    text: "ATC",
    selector: ".official-atc-table thead th:nth-child(1)"
  },
  {
    id: "p2-table-percentage-tax-on",
    page: 2,
    order: 98,
    kind: "table-heading",
    text: "Percentage Tax On",
    selector: ".official-atc-table thead th:nth-child(2)"
  },
  {
    id: "p2-table-tax-rate",
    page: 2,
    order: 99,
    kind: "table-heading",
    text: "Tax Rate",
    selector: ".official-atc-table thead th:nth-child(3)"
  },
  {
    id: "p2-pt010",
    page: 2,
    order: 100,
    kind: "table-entry",
    text: "PT 010 Persons exempt from VAT under Sec. 109(BB) (Sec. 116)",
    selector: ".official-atc-table tr[data-atc-code='PT010']"
  },
  {
    id: "p2-pt010-rate",
    page: 2,
    order: 101,
    kind: "tax-rate",
    text: "3%",
    selector: ".official-atc-table tr[data-atc-code='PT010'] > td:nth-child(3)"
  },
  {
    id: "p2-pt040",
    page: 2,
    order: 102,
    kind: "table-entry",
    text: "PT 040 Domestic carriers and keepers of garages (Sec. 117)",
    selector: ".official-atc-table tr[data-atc-code='PT040']"
  },
  {
    id: "p2-pt040-rate",
    page: 2,
    order: 103,
    kind: "tax-rate",
    text: "3%",
    selector: ".official-atc-table tr[data-atc-code='PT040'] > td:nth-child(3)"
  },
  {
    id: "p2-pt041",
    page: 2,
    order: 104,
    kind: "table-entry",
    text: "PT 041 International Carriers (Sec. 118)",
    selector: ".official-atc-table tr[data-atc-code='PT041']"
  },
  {
    id: "p2-pt041-rate",
    page: 2,
    order: 105,
    kind: "tax-rate",
    text: "3%",
    selector: ".official-atc-table tr[data-atc-code='PT041'] > td:nth-child(3)"
  },
  {
    id: "p2-pt060",
    page: 2,
    order: 106,
    kind: "table-entry",
    text: "PT 060 Franchises on gas and water utilities (Sec. 119)",
    selector: ".official-atc-table tr[data-atc-code='PT060']"
  },
  {
    id: "p2-pt060-rate",
    page: 2,
    order: 107,
    kind: "tax-rate",
    text: "2%",
    selector: ".official-atc-table tr[data-atc-code='PT060'] > td:nth-child(3)"
  },
  {
    id: "p2-pt070",
    page: 2,
    order: 108,
    kind: "table-entry",
    text: "PT 070 Franchises on radio/TV broadcasting companies whose annual gross receipts do not exceed P10 M (Sec. 119)",
    selector: ".official-atc-table tr[data-atc-code='PT070']"
  },
  {
    id: "p2-pt070-rate",
    page: 2,
    order: 109,
    kind: "tax-rate",
    text: "3%",
    selector: ".official-atc-table tr[data-atc-code='PT070'] > td:nth-child(3)"
  },
  {
    id: "p2-pt090",
    page: 2,
    order: 110,
    kind: "table-entry",
    text: "PT 090 Overseas dispatch, message or conversation originating from the Philippines (Sec. 120)",
    selector: ".official-atc-table tr[data-atc-code='PT090']"
  },
  {
    id: "p2-pt090-rate",
    page: 2,
    order: 111,
    kind: "tax-rate",
    text: "10%",
    selector: ".official-atc-table tr[data-atc-code='PT090'] > td:nth-child(3)"
  },
  {
    id: "p2-pt140",
    page: 2,
    order: 112,
    kind: "table-entry",
    text: "PT 140 Cockpits (Sec. 125)",
    selector: ".official-atc-table tr[data-atc-code='PT140']"
  },
  {
    id: "p2-pt140-rate",
    page: 2,
    order: 113,
    kind: "tax-rate",
    text: "18%",
    selector: ".official-atc-table tr[data-atc-code='PT140'] > td:nth-child(3)",
    officialText: "18 %"
  },
  {
    id: "p2-pt150",
    page: 2,
    order: 114,
    kind: "table-entry",
    text: "PT 150 Tax on amusement places, such as cabarets, night and day clubs, videoke bars, karaoke bars, karaoke television, karaoke boxes, music lounges and other similar establishments (Sec. 125)",
    selector: ".official-atc-table tr[data-atc-code='PT150']"
  },
  {
    id: "p2-pt150-rate",
    page: 2,
    order: 115,
    kind: "tax-rate",
    text: "18%",
    selector: ".official-atc-table tr[data-atc-code='PT150'] > td:nth-child(3)",
    officialText: "18 %"
  },
  {
    id: "p2-pt160",
    page: 2,
    order: 116,
    kind: "table-entry",
    text: "PT 160 Boxing Exhibition (Sec. 125)",
    selector: ".official-atc-table tr[data-atc-code='PT160']"
  },
  {
    id: "p2-pt160-rate",
    page: 2,
    order: 117,
    kind: "tax-rate",
    text: "10%",
    selector: ".official-atc-table tr[data-atc-code='PT160'] > td:nth-child(3)",
    officialText: "10 %"
  },
  {
    id: "p2-pt170",
    page: 2,
    order: 118,
    kind: "table-entry",
    text: "PT 170 Professional Basketball Games (Sec. 125)",
    selector: ".official-atc-table tr[data-atc-code='PT170']"
  },
  {
    id: "p2-pt170-rate",
    page: 2,
    order: 119,
    kind: "tax-rate",
    text: "15%",
    selector: ".official-atc-table tr[data-atc-code='PT170'] > td:nth-child(3)",
    officialText: "15 %"
  },
  {
    id: "p2-pt180",
    page: 2,
    order: 120,
    kind: "table-entry",
    text: "PT 180 Jai-alai and Race Tracks (Sec. 125)",
    selector: ".official-atc-table tr[data-atc-code='PT180']"
  },
  {
    id: "p2-pt180-rate",
    page: 2,
    order: 121,
    kind: "tax-rate",
    text: "30%",
    selector: ".official-atc-table tr[data-atc-code='PT180'] > td:nth-child(3)",
    officialText: "30 %"
  },
  {
    id: "p2-bank-category",
    page: 2,
    order: 122,
    kind: "table-entry",
    text: "Tax on Banks and Non-Bank Financial Intermediaries Performing Quasi-Banking Functions (Sec. 121)",
    selector: ".official-atc-table tr.atc-category"
  },
  {
    id: "p2-bank-note",
    page: 2,
    order: 123,
    kind: "table-entry",
    text: "1) On interest, commissions and discounts from lending activities as well as income from financial leasing, on the basis of remaining maturities of instruments from which such receipts are derived",
    selector: ".official-atc-table tr[data-atc-note]"
  },
  {
    id: "p2-pt105",
    page: 2,
    order: 124,
    kind: "table-entry",
    text: "PT 105 - Maturity period is five (5) years or less",
    selector: ".official-atc-table tr[data-atc-code='PT105']"
  },
  {
    id: "p2-pt105-rate",
    page: 2,
    order: 125,
    kind: "tax-rate",
    text: "5%",
    selector: ".official-atc-table tr[data-atc-code='PT105'] > td:nth-child(3)"
  },
  {
    id: "p2-pt101",
    page: 2,
    order: 126,
    kind: "table-entry",
    text: "PT 101 - Maturity period is more than five (5) years",
    selector: ".official-atc-table tr[data-atc-code='PT101']"
  },
  {
    id: "p2-pt101-rate",
    page: 2,
    order: 127,
    kind: "tax-rate",
    text: "1%",
    selector: ".official-atc-table tr[data-atc-code='PT101'] > td:nth-child(3)"
  },
  {
    id: "p2-pt102",
    page: 2,
    order: 128,
    kind: "table-entry",
    text: "PT 102 2) On dividends and equity shares and net income of subsidiaries",
    selector: ".official-atc-table tr[data-atc-code='PT102']"
  },
  {
    id: "p2-pt102-rate",
    page: 2,
    order: 129,
    kind: "tax-rate",
    text: "0%",
    selector: ".official-atc-table tr[data-atc-code='PT102'] > td:nth-child(3)"
  },
  {
    id: "p2-pt103",
    page: 2,
    order: 130,
    kind: "table-entry",
    text: "PT 103 3) On royalties, rentals of property, real or personal, profits from exchange and all other gross income",
    selector: ".official-atc-table tr[data-atc-code='PT103']"
  },
  {
    id: "p2-pt103-rate",
    page: 2,
    order: 131,
    kind: "tax-rate",
    text: "7%",
    selector: ".official-atc-table tr[data-atc-code='PT103'] > td:nth-child(3)"
  },
  {
    id: "p2-pt104",
    page: 2,
    order: 132,
    kind: "table-entry",
    text: "PT 104 4) On net trading gains within the taxable year on foreign currency, debt securities, derivatives and other financial instruments",
    selector: ".official-atc-table tr[data-atc-code='PT104']"
  },
  {
    id: "p2-pt104-rate",
    page: 2,
    order: 133,
    kind: "tax-rate",
    text: "7%",
    selector: ".official-atc-table tr[data-atc-code='PT104'] > td:nth-child(3)"
  },
  {
    id: "p2-other-bank-category",
    page: 2,
    order: 134,
    kind: "table-entry",
    text: "Tax on Other Non-Bank Financial Intermediaries not Performing Quasi-Banking Functions (Sec. 122)",
    selector: ".official-atc-table tr.atc-category"
  },
  {
    id: "p2-bank-note-2",
    page: 2,
    order: 135,
    kind: "table-entry",
    text: "1) On interest, commissions and discounts from lending activities as well as income from financial leasing, on the basis of remaining maturities of instruments from which such receipts are derived",
    selector: ".official-atc-table tr[data-atc-note]"
  },
  {
    id: "p2-pt113",
    page: 2,
    order: 136,
    kind: "table-entry",
    text: "PT 113 - Maturity period is five (5) years or less",
    selector: ".official-atc-table tr[data-atc-code='PT113']"
  },
  {
    id: "p2-pt113-rate",
    page: 2,
    order: 137,
    kind: "tax-rate",
    text: "5%",
    selector: ".official-atc-table tr[data-atc-code='PT113'] > td:nth-child(3)"
  },
  {
    id: "p2-pt114",
    page: 2,
    order: 138,
    kind: "table-entry",
    text: "PT 114 - Maturity period is more than five (5) years",
    selector: ".official-atc-table tr[data-atc-code='PT114']"
  },
  {
    id: "p2-pt114-rate",
    page: 2,
    order: 139,
    kind: "tax-rate",
    text: "1%",
    selector: ".official-atc-table tr[data-atc-code='PT114'] > td:nth-child(3)"
  },
  {
    id: "p2-pt115",
    page: 2,
    order: 140,
    kind: "table-entry",
    text: "PT 115 2) From all other items treated as gross income under the code",
    selector: ".official-atc-table tr[data-atc-code='PT115']"
  },
  {
    id: "p2-pt115-rate",
    page: 2,
    order: 141,
    kind: "tax-rate",
    text: "5%",
    selector: ".official-atc-table tr[data-atc-code='PT115'] > td:nth-child(3)"
  },
  {
    id: "p2-pt120",
    page: 2,
    order: 142,
    kind: "table-entry",
    text: "PT 120 Life Insurance Premiums (Sec. 123)",
    selector: ".official-atc-table tr[data-atc-code='PT120']"
  },
  {
    id: "p2-pt120-rate",
    page: 2,
    order: 143,
    kind: "tax-rate",
    text: "2%",
    selector: ".official-atc-table tr[data-atc-code='PT120'] > td:nth-child(3)"
  },
  {
    id: "p2-foreign-insurance-category",
    page: 2,
    order: 144,
    kind: "table-entry",
    text: "Agents of Foreign Insurance Companies (Sec. 124)",
    selector: ".official-atc-table tr.atc-category"
  },
  {
    id: "p2-pt130",
    page: 2,
    order: 145,
    kind: "table-entry",
    text: "PT 130 1) Insurance Agents",
    selector: ".official-atc-table tr[data-atc-code='PT130']"
  },
  {
    id: "p2-pt130-rate",
    page: 2,
    order: 146,
    kind: "tax-rate",
    text: "4%",
    selector: ".official-atc-table tr[data-atc-code='PT130'] > td:nth-child(3)"
  },
  {
    id: "p2-pt132",
    page: 2,
    order: 147,
    kind: "table-entry",
    text: "PT 132 2) Owners of property obtaining insurance directly with foreign insurance companies",
    selector: ".official-atc-table tr[data-atc-code='PT132']"
  },
  {
    id: "p2-pt132-rate",
    page: 2,
    order: 148,
    kind: "tax-rate",
    text: "5%",
    selector: ".official-atc-table tr[data-atc-code='PT132'] > td:nth-child(3)"
  }
] as const;

export function verifyPageIndexedStaticText(
  pages: readonly (string | StaticTextPageSnapshot)[],
  manifest: readonly OfficialStaticTextEntry[] = OFFICIAL_2551Q_STATIC_TEXT
): StaticTextViolation[] {
  const snapshots = pages.map(toSnapshot);
  return manifest.flatMap((entry) => {
    const expectedText = normalizeStaticText(entry.text);
    const expectedPageText = textForEntry(snapshots[entry.page - 1], entry);
    if (containsExactStaticText(expectedPageText, expectedText)) return [];

    const foundOnPages = snapshots.flatMap((snapshot, index) =>
      containsExactStaticText(textForEntry(snapshot, entry), expectedText)
        ? [index + 1]
        : []
    );
    return [{
      id: entry.id,
      expectedPage: entry.page,
      text: entry.text,
      foundOnPages
    }];
  });
}

/**
 * ORDERED, CONSUMING match. Walks one page's normalized static text left to
 * right; every manifest entry for that page, taken in `order`, must be found
 * at or after the previous entry's end. Whatever is never consumed is the
 * residual, and every residual token must be in `allowedResidual`.
 *
 * This is what closes the two attacks `containsExactStaticText` cannot see:
 *  - swapped column headings still contain both strings, but no longer in
 *    order, so the second lookup fails -> `missing-or-reordered`;
 *  - a fabricated "NOT VALID FOR FILING" advisory is consumed by no entry, so
 *    its words land in the residual -> `unexpected-residual`.
 */
export function verifyStaticTextExhaustive(
  pageText: string,
  entriesForPage: readonly OfficialStaticTextEntry[],
  allowedResidual: readonly string[] = OFFICIAL_2551Q_ALLOWED_RESIDUAL
): ExhaustiveStaticTextViolation[] {
  const normalized = normalizeStaticText(pageText);
  const ordered = [...entriesForPage].sort((left, right) => left.order - right.order);
  const page = ordered[0]?.page ?? 0;
  const violations: ExhaustiveStaticTextViolation[] = [];
  const residual: string[] = [];
  let cursor = 0;

  for (const entry of ordered) {
    const expected = normalizeStaticText(entry.text);
    const index = expected.length === 0 ? -1 : normalized.indexOf(expected, cursor);
    if (index < 0) {
      const earlier = expected.length === 0 ? -1 : normalized.indexOf(expected);
      violations.push({
        kind: "missing-or-reordered",
        id: entry.id,
        page: entry.page,
        text: entry.text,
        foundEarlierAt: earlier >= 0 ? earlier : null
      });
      continue;
    }
    residual.push(normalized.slice(cursor, index));
    cursor = index + expected.length;
  }
  residual.push(normalized.slice(cursor));

  const allowed = new Set(allowedResidual.map(normalizeStaticText));
  const leftover = [
    ...new Set(
      residual
        .join(" ")
        .split(/\s+/)
        .filter((token) => token.length > 0 && !allowed.has(token))
    )
  ].sort();
  if (leftover.length > 0) {
    violations.push({ kind: "unexpected-residual", page, tokens: leftover });
  }
  return violations;
}

/**
 * Manifest-completeness. Every element the renderer marks as carrying
 * reviewable static text must have a manifest entry, so the manifest cannot
 * silently fall behind the renderer. Callers pass what the DOM actually
 * contains; this compares it to what the manifest claims.
 */
export function verifyStaticTextManifestCompleteness(
  observed: readonly Readonly<{ page: number; selector: string; text: string }>[],
  manifest: readonly OfficialStaticTextEntry[] = OFFICIAL_2551Q_STATIC_TEXT
): StaticTextCompletenessViolation[] {
  return observed.flatMap((element) => {
    const text = normalizeStaticText(element.text);
    if (text.length === 0) return [];
    // Coverage is TOKEN COVERAGE, because neither containment direction works.
    //
    // Asking whether the observed element contains a manifest entry is too
    // weak: a fabricated `PT 999 … (Sec. 999) 3%` row satisfies it because
    // `3%` is itself an entry. That was a measured miss.
    //
    // Asking whether an entry contains the observed element is too strict: a
    // legitimate ATC row's innerText concatenates several separate entries
    // (code, description, rate), so no single entry ever contains the row.
    //
    // Requiring every token of the observed text to be accounted for by this
    // page's reviewed vocabulary satisfies both: the fabricated row's
    // `Fabricated` and `999)` are unaccounted, while a genuine row decomposes
    // completely. Word-level substitutions built entirely from reviewed
    // vocabulary are backstopped by the selector-scoped indexed check.
    const vocabulary = manifest
      .filter((entry) => entry.page === element.page)
      .map((entry) => normalizeStaticText(entry.text));
    const unaccounted = text
      .split(" ")
      .filter((token) => token.length > 0)
      .filter((token) => !vocabulary.some((entry) => entry.includes(token)));
    return unaccounted.length === 0
      ? []
      : [{
        kind: "unmanifested-element" as const,
        page: element.page,
        selector: element.selector,
        text,
        unaccounted
      }];
  });
}

/**
 * Every fixture-owned element must be EXPLAINABLE, because both halves of the
 * criterion deliberately look away from this set.
 *
 * The pixel gate blanks these selectors (the official reference is an unfilled
 * form, so fixture glyphs must not count as differences) and the exhaustive
 * walk suppresses them (so fixture characters do not become residual). Both
 * are correct in isolation and together they created a shared blind spot:
 * anything wearing one of these class hooks printed normally on the real form
 * while being invisible to the entire criterion. Measured on 2551Q, that is a
 * 725-element surface, and a fabricated advisory line wrapped in
 * `.comb-value > span` scored zero violations everywhere.
 *
 * This closes it by constraining the set rather than by looking at it harder:
 *  - comb cells and check boxes are single-glyph by construction, so more than
 *    one character in one is a structural impossibility, not a value question;
 *  - free-text fixture fields must match a value the envelope actually
 *    supplied, so invented prose cannot hide there.
 *
 * The result is that suppression is no longer a free pass: the criterion's
 * scope stops being defined by CSS class names the renderer itself controls.
 */
export interface FixtureOwnedObservation {
  page: number;
  selector: string;
  text: string;
  /** True for per-character comb cells and check boxes. */
  singleGlyph: boolean;
}

export interface FixtureOwnedViolation {
  kind: "multi-glyph-cell" | "unexplained-fixture-text";
  page: number;
  selector: string;
  text: string;
}

export function verifyFixtureOwnedText(
  observed: readonly FixtureOwnedObservation[],
  envelopeValues: readonly string[]
): FixtureOwnedViolation[] {
  const allowed = envelopeValues
    .map((value) => normalizeStaticText(value))
    .filter((value) => value.length > 0);

  return observed.flatMap((element): FixtureOwnedViolation[] => {
    const text = normalizeStaticText(element.text);
    if (text.length === 0) return [];

    if (element.singleGlyph) {
      // Array.from so astral characters count as one, not two UTF-16 units.
      if (Array.from(text).length > 1) {
        return [{
          kind: "multi-glyph-cell",
          page: element.page,
          selector: element.selector,
          text
        }];
      }
      return [];
    }

    const explained = allowed.some((value) => containsExactStaticText(value, text));
    return explained
      ? []
      : [{
        kind: "unexplained-fixture-text",
        page: element.page,
        selector: element.selector,
        text
      }];
  });
}

/** Fixture-owned selectors that are single-glyph by construction. */
export const SINGLE_GLYPH_FIXTURE_SELECTORS: readonly string[] = [
  ".comb-value > span",
  ".check-box"
];

/** Collect every string an envelope supplies, for fixture-text explanation. */
export function collectEnvelopeStrings(envelope: unknown): string[] {
  const out: string[] = [];
  const walk = (value: unknown) => {
    if (typeof value === "string") {
      out.push(value);
      return;
    }
    if (typeof value === "number") {
      out.push(String(value));
      return;
    }
    if (Array.isArray(value)) {
      for (const entry of value) walk(entry);
      return;
    }
    if (value && typeof value === "object") {
      for (const entry of Object.values(value)) walk(entry);
    }
  };
  walk(envelope);
  return out;
}

/** Selectors the completeness assertion enumerates in the DOM. */
export const OFFICIAL_2551Q_COMPLETENESS_SELECTORS: readonly string[] = [
  ".official-atc-table [data-atc-code]",
  ".payment-headings > b"
];

export function staticTextEntriesForPage(
  page: number,
  manifest: readonly OfficialStaticTextEntry[] = OFFICIAL_2551Q_STATIC_TEXT
) {
  return manifest.filter((entry) => entry.page === page);
}

type ResolvedSnapshot = {
  fullText: string;
  staticText: string;
  selectorText: Readonly<Record<string, string>>;
  /**
   * True when the caller supplied per-selector text. Selector scoping is only
   * meaningful then; a caller that passed a bare page string gets the
   * page-wide check it asked for rather than a guaranteed failure.
   */
  scoped: boolean;
};

function toSnapshot(page: string | StaticTextPageSnapshot): ResolvedSnapshot {
  if (typeof page === "string") {
    return { fullText: page, staticText: page, selectorText: {}, scoped: false };
  }
  return {
    fullText: page.fullText,
    staticText: page.staticText ?? page.fullText,
    selectorText: page.selectorText ?? {},
    scoped: page.selectorText !== undefined
  };
}

function textForEntry(
  snapshot: ResolvedSnapshot | undefined,
  entry: OfficialStaticTextEntry
) {
  if (!snapshot) return "";
  // A scoped snapshot whose selector matched nothing yields "", which is a
  // violation. That is deliberate: a manifest entry pointing at an element the
  // renderer no longer emits must fail, not silently fall back to the page.
  const source = snapshot.scoped
    ? snapshot.selectorText[entry.selector] ?? ""
    : snapshot.fullText;
  return normalizeStaticText(source);
}

export function normalizeStaticText(value: string) {
  return value
    .normalize("NFC")
    .replace(/[   ]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function containsExactStaticText(pageText: string, expectedText: string) {
  if (expectedText.length === 0) return false;
  return ` ${pageText} `.includes(` ${expectedText} `) || pageText.includes(expectedText);
}
