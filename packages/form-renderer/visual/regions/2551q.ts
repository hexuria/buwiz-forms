// Reviewed 2551Q:2018 critical-region rectangles in 144-DPI reference pixels.
// These drive both the geometry assertions in form-parity.spec.ts and the
// region-ranked diff report; coordinates are pinned against the official
// reference rasters and must only change with reviewed geometry evidence.

export interface CriticalRegion {
  name: string;
  selector: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export const PAGE_ONE_CRITICAL_REGIONS: readonly CriticalRegion[] = [
  { name: "Items 1-5", selector: ".official-header-options", x: 45, y: 223, width: 1137, height: 67 },
  { name: "Items 1-2 filing basis", selector: ".filing-basis", x: 45, y: 223, width: 398, height: 67 },
  { name: "Item 1 label", selector: ".filing-basis > .option-label:first-child", x: 45, y: 223, width: 114, height: 31 },
  { name: "Item 1 choices", selector: ".filing-basis > .option-choices", x: 159, y: 223, width: 284, height: 31 },
  { name: "Item 2 label", selector: ".filing-basis > .year-label", x: 45, y: 253, width: 226, height: 37 },
  { name: "Item 2 value", selector: ".filing-basis > .comb-value", x: 271, y: 253, width: 172, height: 37 },
  { name: "Item 3 quarter", selector: ".quarter-options", x: 443, y: 223, width: 369, height: 67 },
  { name: "Item 3 label", selector: ".quarter-options > .option-label", x: 443, y: 223, width: 369, height: 30 },
  { name: "Item 3 choices", selector: ".quarter-options > .option-choices", x: 443, y: 253, width: 369, height: 37 },
  { name: "Item 4 amended", selector: ".amended-options", x: 812, y: 223, width: 199, height: 67 },
  { name: "Item 4 label", selector: ".amended-options > .option-label", x: 812, y: 223, width: 199, height: 30 },
  { name: "Item 4 choices", selector: ".amended-options > .option-choices", x: 812, y: 253, width: 199, height: 37 },
  { name: "Item 5 sheets", selector: ".sheets-options", x: 1011, y: 223, width: 168, height: 67 },
  { name: "Item 5 label", selector: ".sheets-options > .option-label", x: 1011, y: 223, width: 168, height: 30 },
  { name: "Item 5 value", selector: ".sheets-options > .sheets-value", x: 1011, y: 253, width: 168, height: 37 },
  { name: "Calendar checkbox", selector: ".filing-basis .check-choice:nth-child(1) .check-box", x: 169, y: 225, width: 28, height: 26 },
  { name: "Fiscal checkbox", selector: ".filing-basis .check-choice:nth-child(2) .check-box", x: 314, y: 226, width: 28, height: 26 },
  { name: "First-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(1) .check-box", x: 488, y: 252, width: 28, height: 26 },
  { name: "Second-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(2) .check-box", x: 573, y: 252, width: 28, height: 26 },
  { name: "Third-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(3) .check-box", x: 658, y: 252, width: 28, height: 26 },
  { name: "Fourth-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(4) .check-box", x: 742, y: 252, width: 28, height: 26 },
  { name: "Amended yes checkbox", selector: ".amended-options .check-choice:nth-child(1) .check-box", x: 851, y: 254, width: 28, height: 26 },
  { name: "Amended no checkbox", selector: ".amended-options .check-choice:nth-child(2) .check-box", x: 934, y: 255, width: 28, height: 26 },
  { name: "Items 6-13", selector: ".background-information", x: 45, y: 295, width: 1137, height: 399 },
  { name: "Items 6-7", selector: ".tin-rdo-row", x: 45, y: 320, width: 1137, height: 36 },
  { name: "Item 6 label", selector: ".tin-rdo-row > .field-label", x: 47, y: 320, width: 394, height: 35 },
  { name: "TIN first group", selector: ".tin-rdo-row > .comb-value:nth-child(2)", x: 441, y: 320, width: 85, height: 35 },
  { name: "TIN first separator", selector: ".tin-rdo-row > .tin-separator:nth-child(3)", x: 526, y: 320, width: 29, height: 35 },
  { name: "TIN second group", selector: ".tin-rdo-row > .comb-value:nth-child(4)", x: 555, y: 320, width: 85, height: 35 },
  { name: "TIN second separator", selector: ".tin-rdo-row > .tin-separator:nth-child(5)", x: 640, y: 320, width: 28, height: 35 },
  { name: "TIN third group", selector: ".tin-rdo-row > .comb-value:nth-child(6)", x: 668, y: 320, width: 86, height: 35 },
  { name: "TIN third separator", selector: ".tin-rdo-row > .tin-separator:nth-child(7)", x: 754, y: 320, width: 28, height: 35 },
  { name: "TIN branch group", selector: ".tin-rdo-row > .comb-value:nth-child(8)", x: 782, y: 320, width: 142, height: 35 },
  { name: "Item 7 label", selector: ".tin-rdo-row > .rdo-label", x: 924, y: 320, width: 170, height: 35 },
  { name: "Item 7 value", selector: ".tin-rdo-row > .comb-value:last-child", x: 1094, y: 320, width: 86, height: 35 },
  { name: "Item 8", selector: ".name-field", x: 45, y: 356, width: 1137, height: 58 },
  { name: "Item 8 label", selector: ".name-field > .field-label", x: 47, y: 357, width: 1133, height: 23 },
  { name: "Item 8 value", selector: ".name-field > .comb-value", x: 47, y: 380, width: 1133, height: 35 },
  { name: "Items 9-9A", selector: ".address-field", x: 45, y: 414, width: 1137, height: 96 },
  { name: "Item 9 label", selector: ".address-field > .field-label", x: 47, y: 415, width: 1133, height: 23 },
  { name: "Item 9 first value row", selector: ".address-field > .comb-value", x: 47, y: 438, width: 1133, height: 35 },
  { name: "Item 9 continuation", selector: ".address-continuation > .comb-value:first-child", x: 47, y: 473, width: 881, height: 38 },
  { name: "Item 9A label", selector: ".address-continuation > .zip-label", x: 928, y: 473, width: 141, height: 38 },
  { name: "Item 9A value", selector: ".address-continuation > .comb-value:last-child", x: 1069, y: 473, width: 111, height: 38 },
  { name: "Items 10-11", selector: ".contact-email-field", x: 45, y: 510, width: 1137, height: 59 },
  { name: "Item 10 label", selector: ".contact-email-field > .field-label:first-child", x: 47, y: 510, width: 337, height: 23 },
  { name: "Item 11 label", selector: ".contact-email-field > .field-label:nth-child(2)", x: 383, y: 510, width: 796, height: 23 },
  { name: "Item 10 value", selector: ".contact-email-field > .comb-value:nth-child(3)", x: 47, y: 533, width: 337, height: 35 },
  { name: "Item 11 value", selector: ".contact-email-field > .comb-value:nth-child(4)", x: 383, y: 533, width: 796, height: 35 },
  { name: "Items 12-12A", selector: ".tax-relief-field", x: 45, y: 569, width: 1137, height: 40 },
  { name: "Item 12 label", selector: ".tax-relief-field > .field-label", x: 47, y: 568, width: 337, height: 38 },
  { name: "Item 12 choices", selector: ".tax-relief-field > .relief-choices", x: 383, y: 568, width: 171, height: 38 },
  { name: "Item 12 Yes box", selector: ".relief-choices .check-choice:first-child .check-box", x: 383, y: 574, width: 28, height: 28 },
  { name: "Item 12 No box", selector: ".relief-choices .check-choice:last-child .check-box", x: 469, y: 574, width: 28, height: 28 },
  { name: "Item 12A label", selector: ".tax-relief-field > .tax-relief-spec", x: 554, y: 568, width: 170, height: 38 },
  { name: "Item 12A value", selector: ".tax-relief-field > .comb-value", x: 724, y: 568, width: 455, height: 38 },
  { name: "Item 13", selector: ".income-rate-field", x: 45, y: 607, width: 1137, height: 87 },
  { name: "Items 14-24 totals", selector: ".tax-payable", x: 45, y: 696, width: 1137, height: 507 },
  { name: "Item 14 total", selector: ".official-tax-line[data-item='14']", x: 45, y: 723, width: 1137, height: 36 },
  { name: "Item 17 inline specification field", selector: ".tax-credit-description", x: 444, y: 862, width: 309, height: 24 },
  { name: "official declaration and signatures", selector: ".official-declaration", x: 45, y: 1207, width: 1137, height: 231 },
  { name: "declaration copy", selector: ".official-declaration > p", x: 47, y: 1208, width: 1133, height: 56 },
  { name: "official signature boxes", selector: ".official-signature-grid", x: 47, y: 1264, width: 1133, height: 134 },
  { name: "individual signature caption", selector: ".official-signature-grid > div:first-child .signature-caption", x: 47, y: 1345, width: 567, height: 53 },
  { name: "non-individual signature caption", selector: ".official-signature-grid > div:last-child .signature-caption", x: 615, y: 1345, width: 565, height: 53 },
  { name: "tax-agent strip", selector: ".tax-agent-strip", x: 47, y: 1398, width: 1133, height: 38 },
  { name: "Part III item 25 decimal cell", selector: ".payment-row-25 .decimal-separator", x: 1097, y: 1502, width: 30, height: 35 },
  { name: "Part III item 25 cents cells", selector: ".payment-row-25 .blank-money-value > .comb-value:last-child", x: 1127, y: 1502, width: 53, height: 35 },
  { name: "Part III item 26 decimal cell", selector: ".payment-row-26 .decimal-separator", x: 1097, y: 1538, width: 30, height: 35 },
  { name: "Part III item 26 cents cells", selector: ".payment-row-26 .blank-money-value > .comb-value:last-child", x: 1127, y: 1538, width: 53, height: 35 },
  { name: "Part III item 27 decimal cell", selector: ".payment-row-27 .decimal-separator", x: 1097, y: 1575, width: 30, height: 35 },
  { name: "Part III item 27 cents cells", selector: ".payment-row-27 .blank-money-value > .comb-value:last-child", x: 1127, y: 1575, width: 53, height: 35 },
  { name: "Part III item 28 continuation decimal cell", selector: ".payment-other-row .decimal-separator", x: 1097, y: 1636, width: 30, height: 35 },
  { name: "Part III item 28 continuation cents cells", selector: ".payment-other-row .blank-money-value > .comb-value:last-child", x: 1127, y: 1636, width: 53, height: 35 },
  { name: "Part III machine validation", selector: ".machine-validation", x: 45, y: 1672, width: 1137, height: 133 },
  { name: "privacy note", selector: ".privacy-note", x: 45, y: 1811, width: 1137, height: 16 }
];

export const PAGE_TWO_CRITICAL_REGIONS: readonly CriticalRegion[] = [
  { name: "Schedule 1 masthead", selector: ".page-two-masthead", x: 45, y: 78, width: 1137, height: 117 },
  { name: "Schedule 1 identity", selector: ".page-two-identity", x: 45, y: 193, width: 1137, height: 60 },
  { name: "Schedule 1", selector: ".official-schedule", x: 45, y: 256, width: 1137, height: 327 },
  { name: "Schedule 1 ATC table", selector: ".official-atc-table", x: 45, y: 587, width: 1137, height: 677 }
];

const REGION_TABLES: ReadonlyMap<string, readonly CriticalRegion[]> = new Map([
  ["2551Q:2018:1", PAGE_ONE_CRITICAL_REGIONS],
  ["2551Q:2018:2", PAGE_TWO_CRITICAL_REGIONS]
]);

/** Named critical regions for one exact official page; empty when unreviewed. */
export function criticalRegionsFor(
  code: string,
  revision: string,
  page: number
): readonly CriticalRegion[] {
  return REGION_TABLES.get(`${code}:${revision}:${page}`) ?? [];
}
