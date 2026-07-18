export type OfficialStaticTextKind =
  | "masthead"
  | "item"
  | "instruction"
  | "section"
  | "signature"
  | "table-heading"
  | "table-entry"
  | "barcode-caption";

export type OfficialStaticTextEntry = Readonly<{
  id: string;
  page: 1 | 2;
  kind: OfficialStaticTextKind;
  text: string;
  selector?: string;
}>;

export type StaticTextPageSnapshot = Readonly<{
  fullText: string;
  selectorText?: Readonly<Record<string, string>>;
}>;

export type StaticTextViolation = Readonly<{
  id: string;
  expectedPage: number;
  text: string;
  foundOnPages: readonly number[];
}>;

/**
 * Human-reviewed against the pinned January 2018 2551Q PDF. These strings are
 * page-scoped evidence only; they are not consumed by the runtime renderer.
 */
export const OFFICIAL_2551Q_STATIC_TEXT: readonly OfficialStaticTextEntry[] = [
  { id: "p1-bir-use-only", page: 1, kind: "masthead", text: "For BIR Use Only" },
  { id: "p1-bcs-item", page: 1, kind: "masthead", text: "BCS/ Item" },
  { id: "p1-government", page: 1, kind: "masthead", text: "Republic of the Philippines Department of Finance Bureau of Internal Revenue" },
  { id: "p1-form-number", page: 1, kind: "masthead", text: "BIR Form No. 2551Q January 2018 (ENCS) Page 1" },
  { id: "p1-title", page: 1, kind: "masthead", text: "Quarterly Percentage Tax Return" },
  { id: "p1-filing-instruction", page: 1, kind: "instruction", text: "Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable boxes with an “X”. Two copies MUST be filed with the BIR and one held by the Taxpayer." },
  { id: "p1-barcode-caption", page: 1, kind: "barcode-caption", text: "2551Q 01/18ENCS P1" },

  { id: "p1-item-1", page: 1, kind: "item", text: "1 For the" },
  { id: "p1-item-1-calendar", page: 1, kind: "item", text: "Calendar" },
  { id: "p1-item-1-fiscal", page: 1, kind: "item", text: "Fiscal" },
  { id: "p1-item-2", page: 1, kind: "item", text: "2 Year Ended (MM/YYYY)" },
  { id: "p1-item-3", page: 1, kind: "item", text: "3 Quarter" },
  { id: "p1-item-3-options", page: 1, kind: "item", text: "1st" },
  { id: "p1-item-3-option-2", page: 1, kind: "item", text: "2nd" },
  { id: "p1-item-3-option-3", page: 1, kind: "item", text: "3rd" },
  { id: "p1-item-3-option-4", page: 1, kind: "item", text: "4th" },
  { id: "p1-item-4", page: 1, kind: "item", text: "4 Amended Return?" },
  { id: "p1-item-5", page: 1, kind: "item", text: "5 Number of Sheet/s" },
  { id: "p1-item-5-attached", page: 1, kind: "item", text: "Attached" },

  { id: "p1-part-i", page: 1, kind: "section", text: "Part I – Background Information" },
  { id: "p1-item-6", page: 1, kind: "item", text: "6 Taxpayer Identification Number (TIN)" },
  { id: "p1-item-7", page: 1, kind: "item", text: "7 RDO Code" },
  { id: "p1-item-8", page: 1, kind: "item", text: "8 Taxpayer’s Name (Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)" },
  { id: "p1-item-9", page: 1, kind: "item", text: "9 Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)" },
  { id: "p1-item-9a", page: 1, kind: "item", text: "9A ZIP Code" },
  { id: "p1-item-10", page: 1, kind: "item", text: "10 Contact Number (Landline/Cellphone No.)" },
  { id: "p1-item-11", page: 1, kind: "item", text: "11 Email Address" },
  { id: "p1-item-12", page: 1, kind: "item", text: "12 Are you availing of tax relief under Special Law or International Tax Treaty?" },
  { id: "p1-item-12a", page: 1, kind: "item", text: "12A If yes, specify" },
  { id: "p1-item-13", page: 1, kind: "item", text: "13 Only for individual taxpayers whose sales/receipts are subject to Percentage Tax under Section 116 of the Tax Code, as amended:" },
  { id: "p1-item-13-question", page: 1, kind: "instruction", text: "What income tax rates are you availing? (choose one)" },
  { id: "p1-item-13-initial-quarter", page: 1, kind: "instruction", text: "(To be filled out only on the initial quarter of the taxable year)" },
  { id: "p1-item-13-graduated", page: 1, kind: "item", text: "Graduated income tax rate on net taxable income" },
  { id: "p1-item-13-eight-percent", page: 1, kind: "item", text: "8% income tax rate on gross sales/receipts/others" },

  { id: "p1-part-ii", page: 1, kind: "section", text: "Part II – Total Tax Payable" },
  { id: "p1-item-14", page: 1, kind: "item", text: "14 Total Tax Due (From Schedule 1 Item 7)" },
  { id: "p1-less-credit", page: 1, kind: "instruction", text: "Less: Tax Credit/Payment (attach proof)" },
  { id: "p1-item-15", page: 1, kind: "item", text: "15 Creditable Percentage Tax Withheld per BIR Form No. 2307" },
  { id: "p1-item-16", page: 1, kind: "item", text: "16 Tax Paid in Return Previously Filed, if this is an Amended Return" },
  { id: "p1-item-17", page: 1, kind: "item", text: "17 Other Tax Credit/Payment (specify)" },
  { id: "p1-item-18", page: 1, kind: "item", text: "18 Total Tax Credits/Payments (Sum of Items 15 to 17)" },
  { id: "p1-item-19", page: 1, kind: "item", text: "19 Tax Still Payable/(Overpayment) (Item 14 Less Item 18)" },
  { id: "p1-add-penalties", page: 1, kind: "instruction", text: "Add: Penalties" },
  { id: "p1-item-20", page: 1, kind: "item", text: "20 Surcharge" },
  { id: "p1-item-21", page: 1, kind: "item", text: "21 Interest" },
  { id: "p1-item-22", page: 1, kind: "item", text: "22 Compromise" },
  { id: "p1-item-23", page: 1, kind: "item", text: "23 Total Penalties (Sum of Items 20 to 22)" },
  { id: "p1-item-24", page: 1, kind: "item", text: "24 TOTAL AMOUNT PAYABLE/(Overpayment) (Sum of Items 19 and 23)" },
  { id: "p1-overpayment", page: 1, kind: "instruction", text: "If overpayment, mark one box only:" },
  { id: "p1-refunded", page: 1, kind: "item", text: "To be refunded" },
  { id: "p1-tax-credit-certificate", page: 1, kind: "item", text: "To be issued a Tax Credit Certificate" },

  { id: "p1-declaration", page: 1, kind: "instruction", text: "I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as contemplated under the “Data Privacy Act of 2012 (R.A. No. 10173)” for legitimate and lawful purposes. (If Authorized Representative, attach authorization letter)" },
  { id: "p1-for-individual", page: 1, kind: "signature", text: "For Individual:" },
  { id: "p1-individual-signature", page: 1, kind: "signature", text: "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent" },
  { id: "p1-for-non-individual", page: 1, kind: "signature", text: "For Non-Individual:" },
  { id: "p1-non-individual-signature", page: 1, kind: "signature", text: "Signature over Printed Name of President/Vice President/ Authorized Officer or Representative/Tax Agent" },
  { id: "p1-signature-instruction", page: 1, kind: "signature", text: "(Indicate title/designation and TIN)" },
  { id: "p1-tax-agent", page: 1, kind: "signature", text: "Tax Agent Accreditation No./ Attorney’s Roll No. (If applicable)" },
  { id: "p1-date-of-issue", page: 1, kind: "signature", text: "Date of Issue (MM/DD/YYYY)" },
  { id: "p1-expiry-date", page: 1, kind: "signature", text: "Expiry Date (MM/DD/YYYY)" },

  { id: "p1-part-iii", page: 1, kind: "section", text: "Part III – Details of Payment" },
  { id: "p1-payment-particulars", page: 1, kind: "table-heading", text: "Particulars" },
  { id: "p1-payment-drawee", page: 1, kind: "table-heading", text: "Drawee Bank/ Agency" },
  { id: "p1-payment-number", page: 1, kind: "table-heading", text: "Number" },
  { id: "p1-payment-date", page: 1, kind: "table-heading", text: "Date (MM/DD/YYYY)" },
  { id: "p1-payment-amount", page: 1, kind: "table-heading", text: "Amount" },
  { id: "p1-item-25", page: 1, kind: "item", text: "25 Cash/Bank Debit Memo" },
  { id: "p1-item-26", page: 1, kind: "item", text: "26 Check" },
  { id: "p1-item-27", page: 1, kind: "item", text: "27 Tax Debit Memo" },
  { id: "p1-item-28", page: 1, kind: "item", text: "28 Others (Specify below)" },
  { id: "p1-machine-validation", page: 1, kind: "instruction", text: "Machine Validation/Revenue Official Receipt (ROR) Details (if not filed with an Authorized Agent Bank)" },
  { id: "p1-receiving-stamp", page: 1, kind: "signature", text: "Stamp of receiving Office/AAB and Date of Receipt (RO’s Signature/Bank Teller’s Initial)" },
  { id: "p1-privacy-note", page: 1, kind: "instruction", text: "*NOTE: Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)" },

  { id: "p2-form-number", page: 2, kind: "masthead", text: "BIR Form No. 2551Q January 2018 (ENCS) Page 2" },
  { id: "p2-title", page: 2, kind: "masthead", text: "Quarterly Percentage Tax Return" },
  { id: "p2-barcode-caption", page: 2, kind: "barcode-caption", text: "2551Q 01/18ENCS P2" },
  { id: "p2-tin", page: 2, kind: "table-heading", text: "TIN" },
  { id: "p2-taxpayer-name", page: 2, kind: "table-heading", text: "Taxpayer’s Last Name (if Individual) / Registered Name (if Non-Individual)" },
  { id: "p2-schedule-title", page: 2, kind: "section", text: "Schedule 1 – Computation of Tax (Attach additional sheet/s, if necessary)" },
  { id: "p2-atc-heading", page: 2, kind: "table-heading", text: "Alphanumeric Tax Code (ATC)" },
  { id: "p2-taxable-heading", page: 2, kind: "table-heading", text: "Taxable Amount" },
  { id: "p2-rate-heading", page: 2, kind: "table-heading", text: "Tax Rate" },
  { id: "p2-tax-due-heading", page: 2, kind: "table-heading", text: "Tax Due" },
  { id: "p2-item-1", page: 2, kind: "item", text: "1", selector: ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number" },
  { id: "p2-item-2", page: 2, kind: "item", text: "2", selector: ".official-schedule-row[data-row-slot='2'] > .official-schedule-row-number" },
  { id: "p2-item-3", page: 2, kind: "item", text: "3", selector: ".official-schedule-row[data-row-slot='3'] > .official-schedule-row-number" },
  { id: "p2-item-4", page: 2, kind: "item", text: "4", selector: ".official-schedule-row[data-row-slot='4'] > .official-schedule-row-number" },
  { id: "p2-item-5", page: 2, kind: "item", text: "5", selector: ".official-schedule-row[data-row-slot='5'] > .official-schedule-row-number" },
  { id: "p2-item-6", page: 2, kind: "item", text: "6", selector: ".official-schedule-row[data-row-slot='6'] > .official-schedule-row-number" },
  { id: "p2-item-7", page: 2, kind: "item", text: "7 Total Tax Due (Sum of Items 1 to 6)(To Part II Item 14)" },
  { id: "p2-table-1", page: 2, kind: "table-heading", text: "Table 1 – Alphanumeric Tax Code (ATC)" },
  { id: "p2-table-atc", page: 2, kind: "table-heading", text: "ATC" },
  { id: "p2-table-percentage-tax-on", page: 2, kind: "table-heading", text: "Percentage Tax On" },
  { id: "p2-table-tax-rate", page: 2, kind: "table-heading", text: "Tax Rate" },

  { id: "p2-pt010", page: 2, kind: "table-entry", text: "PT 010 Persons exempt from VAT under Sec. 109(BB) (Sec. 116)" },
  { id: "p2-pt040", page: 2, kind: "table-entry", text: "PT 040 Domestic carriers and keepers of garages (Sec. 117)" },
  { id: "p2-pt041", page: 2, kind: "table-entry", text: "PT 041 International Carriers (Sec. 118)" },
  { id: "p2-pt060", page: 2, kind: "table-entry", text: "PT 060 Franchises on gas and water utilities (Sec. 119)" },
  { id: "p2-pt070", page: 2, kind: "table-entry", text: "PT 070 Franchises on radio/TV broadcasting companies whose annual gross receipts do not exceed P10 M (Sec. 119)" },
  { id: "p2-pt090", page: 2, kind: "table-entry", text: "PT 090 Overseas dispatch, message or conversation originating from the Philippines (Sec. 120)" },
  { id: "p2-pt140", page: 2, kind: "table-entry", text: "PT 140 Cockpits (Sec. 125)" },
  { id: "p2-pt150", page: 2, kind: "table-entry", text: "PT 150 Tax on amusement places, such as cabarets, night and day clubs, videoke bars, karaoke bars, karaoke television, karaoke boxes, music lounges and other similar establishments (Sec. 125)" },
  { id: "p2-pt160", page: 2, kind: "table-entry", text: "PT 160 Boxing Exhibition (Sec. 125)" },
  { id: "p2-pt170", page: 2, kind: "table-entry", text: "PT 170 Professional Basketball Games (Sec. 125)" },
  { id: "p2-pt180", page: 2, kind: "table-entry", text: "PT 180 Jai-alai and Race Tracks (Sec. 125)" },
  { id: "p2-bank-category", page: 2, kind: "table-entry", text: "Tax on Banks and Non-Bank Financial Intermediaries Performing Quasi-Banking Functions (Sec. 121)" },
  { id: "p2-bank-note", page: 2, kind: "table-entry", text: "1) On interest, commissions and discounts from lending activities as well as income from financial leasing, on the basis of remaining maturities of instruments from which such receipts are derived" },
  { id: "p2-pt105", page: 2, kind: "table-entry", text: "PT 105 - Maturity period is five (5) years or less" },
  { id: "p2-pt101", page: 2, kind: "table-entry", text: "PT 101 - Maturity period is more than five (5) years" },
  { id: "p2-pt102", page: 2, kind: "table-entry", text: "PT 102 2) On dividends and equity shares and net income of subsidiaries" },
  { id: "p2-pt103", page: 2, kind: "table-entry", text: "PT 103 3) On royalties, rentals of property, real or personal, profits from exchange and all other gross income" },
  { id: "p2-pt104", page: 2, kind: "table-entry", text: "PT 104 4) On net trading gains within the taxable year on foreign currency, debt securities, derivatives and other financial instruments" },
  { id: "p2-other-bank-category", page: 2, kind: "table-entry", text: "Tax on Other Non-Bank Financial Intermediaries not Performing Quasi-Banking Functions (Sec. 122)" },
  { id: "p2-pt113", page: 2, kind: "table-entry", text: "PT 113 - Maturity period is five (5) years or less" },
  { id: "p2-pt114", page: 2, kind: "table-entry", text: "PT 114 - Maturity period is more than five (5) years" },
  { id: "p2-pt115", page: 2, kind: "table-entry", text: "PT 115 2) From all other items treated as gross income under the code" },
  { id: "p2-pt120", page: 2, kind: "table-entry", text: "PT 120 Life Insurance Premiums (Sec. 123)" },
  { id: "p2-foreign-insurance-category", page: 2, kind: "table-entry", text: "Agents of Foreign Insurance Companies (Sec. 124)" },
  { id: "p2-pt130", page: 2, kind: "table-entry", text: "PT 130 1) Insurance Agents" },
  { id: "p2-pt132", page: 2, kind: "table-entry", text: "PT 132 2) Owners of property obtaining insurance directly with foreign insurance companies" }
] as const;

export function verifyPageIndexedStaticText(
  pages: readonly (string | StaticTextPageSnapshot)[],
  manifest: readonly OfficialStaticTextEntry[] = OFFICIAL_2551Q_STATIC_TEXT
): StaticTextViolation[] {
  const snapshots = pages.map((page) => typeof page === "string"
    ? { fullText: page, selectorText: {} }
    : { fullText: page.fullText, selectorText: page.selectorText ?? {} }
  );
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

function textForEntry(
  snapshot: { fullText: string; selectorText: Readonly<Record<string, string>> } | undefined,
  entry: OfficialStaticTextEntry
) {
  if (!snapshot) return "";
  const source = entry.selector
    ? snapshot.selectorText[entry.selector] ?? ""
    : snapshot.fullText;
  return normalizeStaticText(source);
}

export function normalizeStaticText(value: string) {
  return value
    .normalize("NFC")
    .replace(/[\u00a0\u2007\u202f]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function containsExactStaticText(pageText: string, expectedText: string) {
  if (expectedText.length === 0) return false;
  return ` ${pageText} `.includes(` ${expectedText} `) || pageText.includes(expectedText);
}
