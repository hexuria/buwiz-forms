import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import type { ReactNode } from "react";
import {
  AdaptiveCombValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary
} from "../components";
import { bool, field, text } from "../values";
import {
  OFFICIAL_1701_PDF417,
  OFFICIAL_1701_SEAL
} from "./official1701Assets";
import "./Form1701.css";

type PairedRow = readonly [string, ReactNode, string];

const PART_TWO_ROWS: readonly PairedRow[] = [
  ["22", <>Tax Due <small>(From Part VI Item 5)</small></>, "part_ii_22"],
  ["23", <>Less: Total Tax Credits/Payments <small>(From Part VII Item 10)</small></>, "part_ii_23"],
  ["24", <>Tax Payable/(Overpayment) <small>(Item 22 Less Item 23)</small></>, "part_ii_24"],
  ["25", <>Less: Portion of Tax Payable Allowed for 2nd Installment to be paid on or before October 15 <small>(50% or less of Item 22)</small></>, "part_ii_25"],
  ["26", <>Amount of Tax Payable/(Overpayment) <small>(Item 24 Less Item 25)</small></>, "part_ii_26"],
  ["27", "Add: Penalties - Interest", "part_ii_27"],
  ["28", "Surcharge", "part_ii_28"],
  ["29", "Compromise", "part_ii_29"],
  ["30", <>Total Penalties <small>(Sum of Items 27 to 29)</small></>, "part_ii_30"],
  ["31", <>Total Amount Payable/(Overpayment) <small>(Sum of Items 26 and 30)</small></>, "part_ii_31"]
];

const SCHEDULE_TWO_ROWS: readonly PairedRow[] = [
  ["4", <>Gross Compensation Income <small>(From Part V Schedule 1 Item 3A/3B/c)</small></>, "schedule_2_4"],
  ["5", "Less: Non-Taxable / Exempt Compensation", "schedule_2_5"],
  ["6", <>Taxable Compensation Income <small>(Item 4 Less Item 5)</small></>, "schedule_2_6"],
  ["7", <>Tax Due-Compensation Income <small>(Item 6 × applicable Income Tax Rate)</small></>, "schedule_2_7"]
];

const SCHEDULE_THREE_A_ROWS: readonly PairedRow[] = [
  ["8", "Sales/Revenues/Receipts/Fees", "schedule_3_8"],
  ["9", "Less: Sales Returns, Allowances and Discounts", "schedule_3_9"],
  ["10", <>Net Sales/Revenues/Receipts/Fees <small>(Item 8 Less Item 9)</small></>, "schedule_3_10"],
  ["11", <>Less: Cost of Sales/Services <em>(applicable only if availing Itemized Deductions)</em></>, "schedule_3_11"],
  ["12", <>Gross Income/(Loss) from Operation <small>(Item 10 Less Item 11)</small></>, "schedule_3_12"],
  ["13", <>Ordinary Allowable Itemized Deductions <small>(From Schedule 4 Item 18)</small></>, "schedule_3_13"],
  ["14", <>Special Allowable Itemized Deductions <small>(From Schedule 5 Item 3 and/or Item 6)</small></>, "schedule_3_14"],
  ["15", <>Allowance for Net Operating Loss Carry Over (NOLCO) <small>(From Schedule 6 Item 8 and/or Item 13)</small></>, "schedule_3_15"],
  ["16", <>Total Allowable Itemized Deductions <small>(Sum of Items 13 to 15)</small></>, "schedule_3_16"],
  ["17", <>Optional Standard Deduction (OSD) <small>(40% of Item 10)</small></>, "schedule_3_17"],
  ["18", <>Net Income/(Loss) <small>(If Itemized: Item 12 Less Item 16; If OSD: Item 10 Less Item 17)</small></>, "schedule_3_18"],
  ["19", "Add: Other Non-Operating Income", "schedule_3_19"],
  ["20", "Other Non-Operating Income", "schedule_3_20"],
  ["21", "Amount Received/Share in Income by a Partner from General Professional Partnership (GPP)", "schedule_3_21"],
  ["22", <>Total Other Non-Operating Income <small>(Sum of Items 19 to 21)</small></>, "schedule_3_22"],
  ["23", <>Taxable Income-Business <small>(Sum of Items 18 and 22)</small></>, "schedule_3_23"],
  ["24", <>Total Taxable Income - Compensation &amp; Business <small>(Sum of Items 6 and 23)</small></>, "schedule_3_24"],
  ["25", <>Total Tax Due-Compensation and Business Income <small>(under graduated rates; Item 24 × applicable rate)</small></>, "schedule_3_25"]
];

const SCHEDULE_THREE_B_ROWS: readonly PairedRow[] = [
  ["26", "Sales/Revenues/Receipts/Fees (net of sales returns, allowances and discounts)", "schedule_3_26"],
  ["27", "Add: Other Non-Operating Income", "schedule_3_27"],
  ["28", <>Total Income <small>(Sum of Items 26 and 27)</small></>, "schedule_3_28"],
  ["29", "Less: Allowable reduction of P250,000 for eligible purely self-employed individuals/professionals", "schedule_3_29"],
  ["30", <>Taxable Income/(Loss) <small>(Item 28 Less Item 29)</small></>, "schedule_3_30"],
  ["31", <>Tax Due-Business Income <small>(Item 30 × 8% Flat Income Tax Rate)</small></>, "schedule_3_31"],
  ["32", <>Total Tax Due-Compensation &amp; Business Income <small>(under flat rate; Sum of Items 7 and 31)</small></>, "schedule_3_32"]
];

const SCHEDULE_FOUR_LABELS = [
  "Amortizations", "Bad Debts", "Charitable and Other Contributions", "Depletion",
  "Depreciation", "Entertainment, Amusement and Recreation", "Fringe Benefits", "Interest",
  "Losses", "Pension Trusts", "Rental", "Research and Development",
  "Salaries, Wages and Allowances", "SSS, GSIS, Philhealth, HDMF and Other Contributions",
  "Taxes and Licenses", "Transportation and Travel"
] as const;

const PART_SIX_ROWS: readonly PairedRow[] = [
  ["1", "Regular Rate-Income Tax Due (From Part V, Either Item 25 or Item 32)", "part_vi_1"],
  ["2", "Special Rate-Income Tax Due (From Part X Item 17B/17F)", "part_vi_2"],
  ["3", "Less: Share of Other Government Agency, if remitted directly to the Agency", "part_vi_3"],
  ["4", "Net Special Rate-Income Tax Due/Share of National Govt.", "part_vi_4"],
  ["5", <>Total Income Tax Due <small>(Sum of Items 1 &amp; 4) (To Part II Item 22)</small></>, "part_vi_5"]
];

const PART_SEVEN_LABELS = [
  "Prior Year’s Excess Credits", "Tax Payments for the First Three (3) Quarters",
  "Creditable Tax Withheld for the First Three (3) Quarters",
  "Creditable Tax Withheld per BIR Form No. 2307 for the 4th Quarter",
  "Creditable Tax Withheld per BIR Form No. 2316",
  "Tax Paid in Return Previously Filed, if this is an Amended Return",
  "Foreign Tax Credits, if applicable", "Special Tax Credits, if applicable",
  "Other Tax Credits/Payments (specify)", "Total Tax Credits/Payments (Sum of Items 1 to 9)"
] as const;

const PART_EIGHT_A_LABELS = [
  "Regular Income Tax Otherwise Due", "Tax Relief on Special Allowable Itemized Deductions",
  "Sub-Total - Tax Relief (Sum of Items 1 and 2)", "Less: Income Tax Due",
  "Tax Relief Availment Before Special Tax Credit", "Add: Special Tax Credit, if any",
  "Total Tax Relief Availment- SPECIAL"
] as const;

const PART_NINE_LABELS = [
  "Net Income/(Loss) per Books", "Add: Non-Deductible Expenses/Taxable Other Income",
  "Non-Deductible Expenses/Taxable Other Income", "Non-Deductible Expenses/Taxable Other Income",
  "Total (Sum of Items 1 to 4)", "Less: Non-Taxable Income and Income Subjected to Final Tax",
  "Non-Taxable Income and Income Subjected to Final Tax", "Special/Other Allowable Deductions",
  "Special/Other Allowable Deductions", "Total (Sum of Items 6 to 9)",
  "Net Taxable Income/(Loss) (Item 5 Less Item 10)"
] as const;

export function Form1701({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("1701", "2018");
  if (envelope.schedules.length !== 0) {
    throw new Error("1701 has no repeatable renderer schedule; its reviewed rows have fixed official capacity");
  }
  return (
    <main className="form-document" data-form-code="1701">
      <FolioPage pageNumber={1} paper="folio" className="form-1701-page page-one-1701">
        <GovernmentHeader1701 />
        <Masthead1701 page={1} />
        <HeaderOptions1701 envelope={envelope} />
        <TaxpayerBackground1701 envelope={envelope} />
        <PairedSection1701 title="PART II - Total Tax Payable" rows={PART_TWO_ROWS} envelope={envelope} />
        <Aggregate1701 envelope={envelope} />
        <Overpayment1701 envelope={envelope} />
        <Declaration1701 envelope={envelope} />
        <PaymentDetails1701 envelope={envelope} />
        <p className="privacy-note-1701">*NOTE: The BIR Data Privacy Policy is in the BIR website (www.bir.gov.ph)</p>
      </FolioPage>

      <FolioPage pageNumber={2} paper="folio" className="form-1701-page continuation-page-1701 page-two-1701">
        <Masthead1701 page={2} compact />
        <ContinuationIdentity1701 envelope={envelope} />
        <SpouseBackground1701 envelope={envelope} />
        <SectionBand1701>PART V - Computation of Tax</SectionBand1701>
        <ScheduleOne1701 envelope={envelope} />
        <PairedSection1701 title="Schedule 2 - Taxable Compensation Income" rows={SCHEDULE_TWO_ROWS} envelope={envelope} compact />
        <PairedSection1701 title="Schedule 3 - Taxable Business Income" subtitle="3.A - For Graduated Income Tax Rates" rows={SCHEDULE_THREE_A_ROWS} envelope={envelope} compact descriptions />
      </FolioPage>

      <FolioPage pageNumber={3} paper="folio" className="form-1701-page continuation-page-1701 page-three-1701">
        <Masthead1701 page={3} compact />
        <ContinuationIdentity1701 envelope={envelope} />
        <ScheduleThreeB1701 envelope={envelope} />
        <ScheduleFour1701 envelope={envelope} />
        <ScheduleFive1701 envelope={envelope} />
        <ScheduleSixTaxpayer1701 envelope={envelope} />
      </FolioPage>

      <FolioPage pageNumber={4} paper="folio" className="form-1701-page continuation-page-1701 page-four-1701">
        <Masthead1701 page={4} compact />
        <ContinuationIdentity1701 envelope={envelope} />
        <ScheduleSixSpouse1701 envelope={envelope} />
        <PairedSection1701 title="PART VI - Summary of Income Tax Due" rows={PART_SIX_ROWS} envelope={envelope} compact hideHead />
        <IndexedPairedSection1701 title="PART VII - Tax Credits/Payments (attach proof)" labels={PART_SEVEN_LABELS} fieldPrefix="part_vii" envelope={envelope} />
        <PartEight1701 envelope={envelope} />
        <PartNine1701 envelope={envelope} />
        <TaxTables1701 />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader1701() {
  return <header className="government-header-1701"><span>For BIR<br />Use Only</span><span>BCS/<br />Item:</span><span className="government-wordmark-1701"><img src={OFFICIAL_1701_SEAL} alt="Bureau of Internal Revenue seal" /><span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span></span></header>;
}

function Masthead1701({ page, compact = false }: { page: 1 | 2 | 3 | 4; compact?: boolean }) {
  const artwork = OFFICIAL_1701_PDF417[page - 1];
  return <header className={`masthead-1701${compact ? " compact-1701" : ""}`}>
    <div className="form-number-1701"><span>BIR Form No.</span><strong>1701</strong><small>January 2018 (ENCS)</small><b>Page {page}</b></div>
    <div className="form-title-1701"><strong>Annual Income Tax Return</strong><span>Individuals (including MIXED Income Earner), Estates and Trusts</span>{!compact && <em>Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable boxes with an “X”. Two copies must be filed with the BIR and one held by the Tax Filer.</em>}</div>
    <div className="barcode-1701" aria-label={artwork.payload} data-barcode-page={page}>
      <span className="official-pdf417-object-1701" aria-hidden="true">
        <svg
          className="official-pdf417-symbol-1701"
          viewBox={`0 0 ${artwork.columns} ${artwork.rows}`}
          preserveAspectRatio="none"
          shapeRendering="crispEdges"
          focusable="false"
        >
          <path d={artwork.path} />
        </svg>
      </span>
      <small>{artwork.payload}</small>
    </div>
  </header>;
}

function HeaderOptions1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="header-options-1701">
    <div><b>1</b> For the Year <CombValue value={String(envelope.period.taxable_year)} cells={4} /></div>
    <div><b>2</b> Amended Return? <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" /><CheckChoice checked={!bool(envelope, "is_amended")} label="No" /></div>
    <div><b>3</b> Short Period Return? <CheckChoice checked={bool(envelope, "is_short_period")} label="Yes" /><CheckChoice checked={!bool(envelope, "is_short_period")} label="No" /></div>
  </section>;
}

function SectionBand1701({ children }: { children: ReactNode }) {
  return <h2 className="section-band-1701">{children}</h2>;
}

function TaxpayerBackground1701({ envelope }: { envelope: RenderEnvelope }) {
  const filerType = text(envelope, "taxpayer_type");
  const atc = text(envelope, "atc");
  const civil = text(envelope, "civil_status");
  const rate = text(envelope, "tax_rate");
  const deduction = text(envelope, "deduction_method");
  return <section className="background-1701">
    <SectionBand1701>PART I - Background Information of Taxpayer/Filer</SectionBand1701>
    <div className="tin-rdo-1701"><span><b>4</b> Taxpayer Identification Number (TIN)</span><Tin1701 value={envelope.taxpayer.tin} /><span><b>5</b> RDO Code</span><CombValue value={envelope.taxpayer.rdo_code} cells={3} /></div>
    <ChoiceLine1701 number="6" label="Taxpayer Type" choices={[["single_proprietor", "Single Proprietor"], ["professional", "Professional"], ["estate", "Estate"], ["trust", "Trust"], ["compensation_earner", "Compensation Earner"]]} selected={filerType} />
    <AtcChoices1701 number="7" selected={atc} />
    <LabeledComb1701 number="8" label="Taxpayer’s Name (Last Name, First Name, Middle Name)/ESTATE OF/TRUST FAO" value={envelope.taxpayer.name} cells={52} />
    <LabeledComb1701 number="9" label="Registered Address" value={envelope.taxpayer.registered_address} cells={76} twoRows zip={envelope.taxpayer.zip_code} />
    <div className="split-values-1701"><LabeledComb1701 number="10" label="Date of Birth (MM/DD/YYYY)" value={text(envelope, "date_of_birth")} cells={10} /><LabeledComb1701 number="11" label="Email Address" value={envelope.taxpayer.email} cells={43} /></div>
    <div className="split-values-1701 citizenship-row-1701"><LabeledComb1701 number="12" label="Citizenship" value={text(envelope, "citizenship")} cells={24} /><YesNoLine1701 number="13" label="Claiming Foreign Tax Credits?" value={optionalBool(envelope, "claims_foreign_tax_credits")} /><LabeledComb1701 number="14" label="Foreign Tax Number, if applicable" value={text(envelope, "foreign_tax_number")} cells={25} /></div>
    <div className="split-values-1701 contact-civil-1701"><LabeledComb1701 number="15" label="Contact Number" value={envelope.taxpayer.contact_number} cells={20} /><ChoiceLine1701 number="16" label="Civil Status" choices={[["single", "Single"], ["married", "Married"], ["legally_separated", "Legally Separated"], ["widowed", "Widow/er"]]} selected={civil} /></div>
    <div className="two-choice-row-1701 filing-status-row-1701"><YesNoLine1701 number="17" label="If married, spouse has income?" value={optionalBool(envelope, "spouse_has_income")} /><ChoiceLine1701 number="18" label="Filing Status" choices={[["joint", "Joint Filing"], ["separate", "Separate Filing"]]} selected={text(envelope, "joint_filing_status")} /></div>
    <div className="two-choice-row-1701 exempt-special-row-1701"><YesNoLine1701 number="19" label="Income EXEMPT from Income Tax?" value={optionalBool(envelope, "has_exempt_income")} /><YesNoLine1701 number="20" label="Income subject to SPECIAL/PREFERENTIAL RATE?" value={optionalBool(envelope, "has_special_rate_income")} /></div>
    <TaxElection1701 number="21" rate={rate} deduction={deduction} />
  </section>;
}

function SpouseBackground1701({ envelope }: { envelope: RenderEnvelope }) {
  const type = text(envelope, "spouse_type");
  const rate = text(envelope, "spouse_tax_rate");
  const deduction = text(envelope, "spouse_deduction_method");
  return <section className="background-1701 spouse-background-1701">
    <SectionBand1701>PART IV - Background Information of Spouse</SectionBand1701>
    <div className="tin-rdo-1701"><span><b>1</b> Spouse’s Taxpayer Identification Number</span><Tin1701 value={text(envelope, "spouse_tin")} /><span><b>2</b> RDO Code</span><CombValue value={text(envelope, "spouse_rdo_code")} cells={3} /></div>
    <ChoiceLine1701 number="3" label="Filer’s Spouse Type" choices={[["single_proprietor", "Single Proprietor"], ["professional", "Professional"], ["compensation_earner", "Compensation Earner"]]} selected={type} />
    <AtcChoices1701 number="4" selected={text(envelope, "spouse_atc")} />
    <LabeledComb1701 number="5" label="Spouse’s Name (Last Name, First Name, Middle Name)" value={text(envelope, "spouse_name")} cells={56} />
    <div className="split-values-1701 spouse-contact-1701"><LabeledComb1701 number="6" label="Contact Number" value={text(envelope, "spouse_contact_number")} cells={24} /><LabeledComb1701 number="7" label="Citizenship" value={text(envelope, "spouse_citizenship")} cells={30} /></div>
    <div className="split-values-1701 spouse-credit-1701"><YesNoLine1701 number="8" label="Claiming Foreign Tax Credits?" value={optionalBool(envelope, "spouse_claims_foreign_tax_credits")} /><LabeledComb1701 number="9" label="Foreign Tax Number" value={text(envelope, "spouse_foreign_tax_number")} cells={31} /></div>
    <div className="two-choice-row-1701"><YesNoLine1701 number="10" label="Income EXEMPT from Income Tax?" value={optionalBool(envelope, "spouse_has_exempt_income")} /><YesNoLine1701 number="11" label="Income subject to SPECIAL/PREFERENTIAL RATE?" value={optionalBool(envelope, "spouse_has_special_rate_income")} /></div>
    <TaxElection1701 number="12" rate={rate} deduction={deduction} />
  </section>;
}

function ChoiceLine1701({ number, label, choices, selected }: { number: string; label: string; choices: ReadonlyArray<readonly [string, string]>; selected: string }) {
  return <div className="choice-line-1701"><span><b>{number}</b> {label}</span>{choices.map(([value, title]) => <CheckChoice key={value} checked={selected === value} label={title} />)}</div>;
}

function YesNoLine1701({ number, label, value }: { number: string; label: string; value: boolean | undefined }) {
  return <div className="choice-line-1701 yes-no-1701"><span><b>{number}</b> {label}</span><CheckChoice checked={value === true} label="Yes" /><CheckChoice checked={value === false} label="No" /></div>;
}

function AtcChoices1701({ number, selected }: { number: string; selected: string }) {
  const values = [["II012", "Business Income-Graduated IT Rates"], ["II014", "Income from Profession-Graduated IT Rates"], ["II013", "Mixed Income-Graduated IT Rates"], ["II011", "Compensation Income"], ["II015", "Business Income - 8% IT Rate"], ["II017", "Income from Profession - 8% IT Rate"], ["II016", "Mixed Income - 8% IT Rate"]] as const;
  return <div className="atc-choices-1701"><span><b>{number}</b> Alphanumeric Tax Code (ATC)</span>{values.map(([value, label]) => <CheckChoice key={value} checked={selected === value} label={`${value} ${label}`} />)}</div>;
}

function TaxElection1701({ number, rate, deduction }: { number: string; rate: string; deduction: string }) {
  if (number === "21") {
    return <div className="tax-election-1701 taxpayer-election-1701">
      <span className="tax-rate-label-1701"><b>{number}</b><span>Tax<br />Rate*</span><small>(choose one)</small></span>
      <span className="tax-election-choice-1701 graduated-choice-1701">
        <CheckChoice checked={rate === "graduated"} label="Graduated Rates" />
        <small>(Choose Method of Deduction in Item 21A)</small>
      </span>
      <span className="deduction-label-1701"><b>{number}A</b> Method of Deduction (choose one)</span>
      <span className="tax-election-choice-1701 itemized-choice-1701">
        <CheckChoice checked={deduction === "itemized"} label="Itemized Deduction" />
        <small>[Sec. 34(A-J), NIRC]</small>
      </span>
      <span className="tax-election-choice-1701 osd-choice-1701">
        <CheckChoice checked={deduction === "osd"} label="Optional Standard Deduction (OSD)" />
        <small>[40% of Gross Sales/Receipts/Revenues/Fees [Sec. 34(L), NIRC]]</small>
      </span>
      <span className="tax-election-choice-1701 eight-percent-choice-1701">
        <CheckChoice checked={rate === "eight_percent"} label="8% in lieu of Graduated Rates under Sec. 24(A) & Percentage Tax under Sec. 116 of NIRC" />
        <small>(available if gross sales/receipts and other non-operating income do not exceed Three million pesos (P3M))</small>
      </span>
    </div>;
  }
  return <div className="tax-election-1701"><span><b>{number}</b> Tax Rate*<small>(choose one)</small></span><CheckChoice checked={rate === "graduated"} label="Graduated Rates" /><span className="deduction-label-1701"><b>{number}A</b> Method of Deduction (choose one)</span><CheckChoice checked={deduction === "itemized"} label="Itemized Deduction" /><CheckChoice checked={deduction === "osd"} label="Optional Standard Deduction (OSD)" /><CheckChoice checked={rate === "eight_percent"} label="8% in lieu of Graduated Rates under Sec. 24(A) & Percentage Tax under Sec. 116 of NIRC" /></div>;
}

function LabeledComb1701({ number, label, value, cells, twoRows = false, zip }: { number: string; label: string; value: string; cells: number; twoRows?: boolean; zip?: string }) {
  if (twoRows) {
    const characters = Array.from(value);
    if (characters.length > cells) return <div className="labeled-comb-1701 address-1701"><span><b>{number}</b> {label}</span><AdaptiveCombValue value={value} cells={cells} /><span className="address-zip-1701"><b>9A</b> ZIP Code <CombValue value={zip ?? ""} cells={4} /></span></div>;
    const midpoint = Math.ceil(cells / 2);
    return <div className="labeled-comb-1701 address-1701"><span><b>{number}</b> {label}</span><CombValue value={characters.slice(0, midpoint).join("")} cells={midpoint} /><span className="address-second-1701"><CombValue value={characters.slice(midpoint).join("")} cells={cells - midpoint} /><span><b>9A</b> ZIP Code <CombValue value={zip ?? ""} cells={4} /></span></span></div>;
  }
  return <div className="labeled-comb-1701"><span><b>{number}</b> {label}</span><AdaptiveCombValue value={value} cells={cells} /></div>;
}

function Tin1701({ value }: { value: string }) {
  const digits = value.replace(/\D/g, "").slice(0, 14).padEnd(14, " ");
  return <span className="tin-value-1701"><CombValue value={digits.slice(0, 3)} cells={3} /><i>-</i><CombValue value={digits.slice(3, 6)} cells={3} /><i>-</i><CombValue value={digits.slice(6, 9)} cells={3} /><i>-</i><CombValue value={digits.slice(9)} cells={5} /></span>;
}

function optionalBool(envelope: RenderEnvelope, key: string): boolean | undefined {
  const value = field(envelope, key);
  return value?.type === "boolean" ? value.value : undefined;
}

function PairedSection1701({ title, subtitle, rows, envelope, compact = false, descriptions = false, hideHead = false }: { title: string; subtitle?: string; rows: readonly PairedRow[]; envelope: RenderEnvelope; compact?: boolean; descriptions?: boolean; hideHead?: boolean }) {
  return <section className={`paired-section-1701${compact ? " compact-table-1701" : ""}${hideHead ? " no-head-1701" : ""}`}><SectionBand1701>{title}</SectionBand1701>{subtitle && <h3>{subtitle}</h3>}{!hideHead && <PairedHead1701 />}{rows.map(([item, label, key]) => <PairedRow1701 key={key} item={item} label={label} fieldKey={key} envelope={envelope} description={descriptions ? text(envelope, `${key}_description`) : ""} />)}</section>;
}

function PairedHead1701() {
  return <div className="paired-head-1701"><span>Particulars</span><span>A. Taxpayer/Filer</span><span>B. Spouse</span></div>;
}

function PairedRow1701({ item, label, fieldKey, envelope, description = "" }: { item: string; label: ReactNode; fieldKey: string; envelope: RenderEnvelope; description?: string }) {
  return <div className="paired-row-1701"><span><b>{item}</b>{description ? <span className="row-description-1701"><AdaptiveCombValue value={description} cells={34} /></span> : label}</span><Amount1701 envelope={envelope} fieldKey={`${fieldKey}_taxpayer`} /><Amount1701 envelope={envelope} fieldKey={`${fieldKey}_spouse`} /></div>;
}

function Amount1701({ envelope, fieldKey }: { envelope: RenderEnvelope; fieldKey: string }) {
  const value = field(envelope, fieldKey);
  if (!value || value.type !== "decimal") return <span className="amount-1701 blank-amount-1701" aria-label="blank amount"><i /></span>;
  const normalized = Math.round(Object.is(value.value, -0) ? 0 : value.value).toString();
  return <span className="amount-1701"><AdaptiveCombValue value={normalized} cells={9} align="right" /></span>;
}

function Aggregate1701({ envelope }: { envelope: RenderEnvelope }) {
  return <div className="aggregate-1701"><span><b>32</b> Aggregate Amount Payable/(Overpayment) <small>(Sum of Items 31A and 31B)</small></span><Amount1701 envelope={envelope} fieldKey="part_ii_32_aggregate" /></div>;
}

function Overpayment1701({ envelope }: { envelope: RenderEnvelope }) {
  const selected = text(envelope, "overpayment_disposition");
  return <div className="overpayment-1701"><span>If overpayment, mark one (1) box only. (Once the choice is made, the same is irrevocable)</span><CheckChoice checked={selected === "refund"} label="To be refunded" /><CheckChoice checked={selected === "tax_credit_certificate"} label="To be issued a Tax Credit Certificate (TCC)" /><CheckChoice checked={selected === "carry_over"} label="To be carried over as a tax credit for next year/quarter" /></div>;
}

function Declaration1701({ envelope }: { envelope: RenderEnvelope }) {
  const attachments = field(envelope, "number_of_attachments");
  const value = attachments?.type === "decimal" ? Math.round(attachments.value).toString().padStart(2, "0") : "";
  return <section className="declaration-1701"><p>I declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof.</p><div className="signature-1701"><span>Printed Name and Signature of Taxpayer/Authorized Representative</span><span><b>33</b> Number of Attachments <CombValue value={value} cells={2} /></span></div></section>;
}

function PaymentDetails1701({ envelope }: { envelope: RenderEnvelope }) {
  const rows = [[34, "Cash/Bank Debit Memo", "payment_34"], [35, "Check", "payment_35"], [36, "Tax Debit Memo", "payment_36"], [37, "Others (specify below)", "payment_37"]] as const;
  return <section className="payments-1701"><SectionBand1701>PART III - Details of Payment</SectionBand1701><div className="payment-head-1701"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date (MM/DD/YYYY)</span><span>Amount</span></div>{rows.map(([item, label, key]) => <div className="payment-row-1701" key={key}><span><b>{item}</b> {label}</span><AdaptiveCombValue value={text(envelope, `${key}_bank`)} cells={14} /><AdaptiveCombValue value={text(envelope, `${key}_number`)} cells={16} /><AdaptiveCombValue value={text(envelope, `${key}_date`)} cells={10} /><Amount1701 envelope={envelope} fieldKey={`${key}_amount`} /></div>)}<div className="payment-receipt-1701"><span>Machine Validation/Revenue Official Receipt Details<span>{text(envelope, "machine_validation_or_receipt_details")}</span></span><em>Stamp of Receiving Office/AAB and Date of Receipt</em></div></section>;
}

function ContinuationIdentity1701({ envelope }: { envelope: RenderEnvelope }) {
  return <div className="continuation-identity-1701"><span>TIN</span><Tin1701 value={envelope.taxpayer.tin} /><span><b>Tax Filer’s Last Name</b><AdaptiveCombValue value={envelope.taxpayer.name} cells={38} /></span></div>;
}

function ScheduleOne1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-one-1701"><h3>Schedule 1 - Gross Compensation Income and Tax Withheld (Attach Additional Sheet/s, if necessary)</h3><p>On Items 1 and 2, enter the required information for each employer/s and mark (X) whether the information is for the Taxpayer or Spouse. On Item 3A, enter the Total Gross Compensation and Total Tax Withheld for the Taxpayer and on Item 3B, for the Spouse.<strong>(DO NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</strong></p><div className="employer-head-1701"><span>#</span><span>a. Name of Employer</span></div>{[1, 2].map((index) => <div className="employer-row-1701" key={index}><span>{index}<CheckChoice checked={text(envelope, `employer_${index}_owner`) === "taxpayer"} label="Taxpayer" /><CheckChoice checked={text(envelope, `employer_${index}_owner`) === "spouse"} label="Spouse" /></span><EmployerIdentity1701 name={text(envelope, `employer_${index}_name`)} tin={text(envelope, `employer_${index}_tin`)} /></div>)}<div className="employer-continuation-head-1701"><span>(Continuation of Table Above)</span><span>c. Compensation Income</span><span>d. Tax Withheld</span></div>{[1, 2].map((index) => <div className="employer-amount-row-1701" key={index}><span>{index}</span><Amount1701 envelope={envelope} fieldKey={`employer_${index}_compensation`} /><Amount1701 envelope={envelope} fieldKey={`employer_${index}_withheld`} /></div>)}<EmployerTotal1701 item="3A" party="TAXPAYER" incomeKey="schedule_2_4_taxpayer" withheldKey="part_vii_5_taxpayer" envelope={envelope} /><EmployerTotal1701 item="3B" party="SPOUSE" incomeKey="schedule_2_4_spouse" withheldKey="part_vii_5_spouse" envelope={envelope} /></section>;
}

function EmployerIdentity1701({ name, tin }: { name: string; tin: string }) {
  const characters = Array.from(name);
  const exceedsOfficialCombCapacity = characters.length > 76;
  const firstLine = exceedsOfficialCombCapacity ? name : characters.slice(0, 52).join("");
  const continuation = exceedsOfficialCombCapacity ? "" : characters.slice(52).join("");
  return <span className="employer-identity-1701" aria-label={name}><AdaptiveCombValue value={firstLine} cells={52} className="employer-name-primary-1701" /><CombValue value={continuation} cells={24} /><span className="employer-tin-label-1701"><b>b.</b> Employer’s TIN</span><AdaptiveCombValue value={tin} cells={14} className="employer-tin-value-1701" /></span>;
}

function EmployerTotal1701({ item, party, incomeKey, withheldKey, envelope }: { item: "3A" | "3B"; party: "TAXPAYER" | "SPOUSE"; incomeKey: string; withheldKey: string; envelope: RenderEnvelope }) {
  const suffix = item.slice(1);
  return <div className="employer-total-1701"><span><b>{item}</b><span>Gross Compensation Income and Total Tax Withheld for <strong>{party}</strong><small>(To Part V Schedule 2 Item 4{suffix} and Part VII Item 5{suffix})</small></span></span><Amount1701 envelope={envelope} fieldKey={incomeKey} /><Amount1701 envelope={envelope} fieldKey={withheldKey} /></div>;
}

function ScheduleThreeB1701({ envelope }: { envelope: RenderEnvelope }) {
  const [item26, ...remaining] = SCHEDULE_THREE_B_ROWS;
  return <section className="paired-section-1701 compact-table-1701 schedule-three-b-1701"><SectionBand1701>3.B - For 8% Flat Income Tax Rate</SectionBand1701><PairedHead1701 /><PairedRow1701 item={item26[0]} label={item26[1]} fieldKey={item26[2]} envelope={envelope} /><div className="schedule-three-b-subtitle-1701">Add: Other Non-Operating Income (specify below)</div>{remaining.map(([item, label, key]) => <PairedRow1701 key={key} item={item} label={label} fieldKey={key} envelope={envelope} description={item === "27" ? text(envelope, `${key}_description`) : ""} />)}</section>;
}

function ScheduleFour1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-four-1701"><SectionBand1701>Schedule 4 - Ordinary Allowable Itemized Deductions</SectionBand1701>{SCHEDULE_FOUR_LABELS.map((label, index) => <PairedRow1701 key={label} item={String(index + 1)} label={label} fieldKey={`schedule_4_${index + 1}`} envelope={envelope} />)}<div className="schedule-four-subtitle-1701">17 Others (Deductions Subject to Withholding Tax and Other Expenses)</div>{["a", "b", "c", "d"].map((suffix, index) => <PairedRow1701 key={suffix} item={suffix} label={index === 0 ? "Janitorial and Messengerial Services" : index === 1 ? "Professional Fees" : index === 2 ? "Security Services" : <AdaptiveCombValue value={text(envelope, "schedule_4_17d_description")} cells={32} />} fieldKey={`schedule_4_17${suffix}`} envelope={envelope} />)}<PairedRow1701 item="18" label="Total Ordinary Allowable Itemized Deductions" fieldKey="schedule_4_18" envelope={envelope} /></section>;
}

function ScheduleFive1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-five-1701"><SectionBand1701>Schedule 5 - Special Allowable Itemized Deductions</SectionBand1701><SpecialDeductionGroup1701 party="taxpayer" start={1} envelope={envelope} /><SpecialDeductionGroup1701 party="spouse" start={4} envelope={envelope} /></section>;
}

function SpecialDeductionGroup1701({ party, start, envelope }: { party: "taxpayer" | "spouse"; start: number; envelope: RenderEnvelope }) {
  const totalItem = party === "taxpayer" ? 3 : 6;
  return <><div className="special-head-1701"><span>5.{party === "taxpayer" ? "A - Taxpayer/Filer" : "B - Spouse"} — Description</span><span>Legal Basis</span><span>Amount</span></div>{[0, 1].map((offset) => { const key = `schedule_5_${party}_${offset + 1}`; return <div className="special-row-1701" key={key}><span>{start + offset}</span><AdaptiveCombValue value={text(envelope, `${key}_description`)} cells={34} /><AdaptiveCombValue value={text(envelope, `${key}_legal_basis`)} cells={16} /><Amount1701 envelope={envelope} fieldKey={`${key}_amount`} /></div>; })}<div className="special-total-1701"><span>{totalItem} Total Special Allowable Itemized Deductions-{party === "taxpayer" ? "Taxpayer/Filer" : "Spouse"}</span><Amount1701 envelope={envelope} fieldKey={`schedule_5_total_${party}`} /></div></>;
}

function ScheduleSixTaxpayer1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-six-1701"><SectionBand1701>Schedule 6 - Computation of Net Operating Loss Carry Over (NOLCO)</SectionBand1701><h3>6.A - Computation of NOLCO</h3>{[[1, "Gross Income"], [2, "Less: Ordinary Allowable Itemized Deductions"], [3, "Net Operating Loss"]].map(([item, label]) => <PairedRow1701 key={item} item={String(item)} label={label} fieldKey={`schedule_6_${item}`} envelope={envelope} />)}<NolcoTable1701 party="taxpayer" start={4} envelope={envelope} /></section>;
}

function ScheduleSixSpouse1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-six-spouse-1701"><SectionBand1701>(Continuation of Schedule 6)</SectionBand1701><NolcoTable1701 party="spouse" start={9} envelope={envelope} /></section>;
}

function NolcoTable1701({ party, start, envelope }: { party: "taxpayer" | "spouse"; start: number; envelope: RenderEnvelope }) {
  return <div className="nolco-table-1701"><h3>6.A.{party === "taxpayer" ? "1 - Taxpayer/Filer’s" : "2 - Spouse’s"} Detailed Computation of Available NOLCO</h3><div className="nolco-head-1701"><span>Year Incurred</span><span>A. Amount</span><span>B. NOLCO Applied Previous Year/s</span><span>C. NOLCO Expired</span><span>D. NOLCO Applied Current Year</span><span>E. Net Operating Loss (Unapplied)</span></div>{[0, 1, 2, 3].map((offset) => { const key = `schedule_6_${party}_${offset + 1}`; return <div className="nolco-row-1701" key={key}><span>{start + offset}</span><AdaptiveCombValue value={text(envelope, `${key}_year`)} cells={4} /><Amount1701 envelope={envelope} fieldKey={`${key}_amount`} /><Amount1701 envelope={envelope} fieldKey={`${key}_previous`} /><Amount1701 envelope={envelope} fieldKey={`${key}_expired`} /><Amount1701 envelope={envelope} fieldKey={`${key}_current`} /><Amount1701 envelope={envelope} fieldKey={`${key}_unapplied`} /></div>; })}<div className="nolco-total-1701"><span>{start + 4} Total NOLCO - {party === "taxpayer" ? "Taxpayer/Filer" : "Spouse"}</span><Amount1701 envelope={envelope} fieldKey={`schedule_6_total_${party}`} /></div></div>;
}

function IndexedPairedSection1701({ title, labels, fieldPrefix, envelope, descriptions = false }: { title: string; labels: readonly string[]; fieldPrefix: string; envelope: RenderEnvelope; descriptions?: boolean }) {
  return <section className="paired-section-1701 compact-table-1701 indexed-section-1701"><SectionBand1701>{title}</SectionBand1701>{labels.map((label, index) => <PairedRow1701 key={label + index} item={String(index + 1)} label={label} fieldKey={`${fieldPrefix}_${index + 1}`} envelope={envelope} description={descriptions ? text(envelope, `${fieldPrefix}_${index + 1}_description`) : ""} />)}</section>;
}

function PartEight1701({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="part-eight-1701"><SectionBand1701>PART VIII - Tax Relief Availment</SectionBand1701><h3>VIII.A - Special Rate</h3>{PART_EIGHT_A_LABELS.map((label, index) => <PairedRow1701 key={label} item={String(index + 1)} label={label} fieldKey={`part_viii_${index + 1}`} envelope={envelope} />)}<h3>VIII.B - Exempt</h3>{[8, 9, 10].map((item, index) => <PairedRow1701 key={item} item={String(item)} label={["Regular Income Tax Otherwise Due", "Tax Relief on Special Allowable Itemized Deductions", "Total Tax Relief Availment- EXEMPT"][index]} fieldKey={`part_viii_${item}`} envelope={envelope} />)}</section>;
}

function PartNine1701({ envelope }: { envelope: RenderEnvelope }) {
  const row = (index: number, description = false) => <PairedRow1701 key={index} item={String(index)} label={PART_NINE_LABELS[index - 1]} fieldKey={`part_ix_${index}`} envelope={envelope} description={description ? text(envelope, `part_ix_${index}_description`) : ""} />;
  return <section className="paired-section-1701 compact-table-1701 indexed-section-1701 part-nine-1701"><SectionBand1701>PART IX - Reconciliation of Net Income per Books Against Taxable Income</SectionBand1701><PairedHead1701 />{row(1)}<div className="part-nine-subtitle-1701">Add: Non-Deductible Expenses/Taxable Other Income</div>{[2, 3, 4].map((index) => row(index, true))}{row(5)}<div className="part-nine-subtitle-1701">Less: A) Non-Taxable Income and Income Subjected to Final Tax</div>{[6, 7].map((index) => row(index, true))}<div className="part-nine-subtitle-1701">B) Special/Other Allowable Deductions</div>{[8, 9].map((index) => row(index, true))}{row(10)}{row(11)}</section>;
}

function TaxTables1701() {
  const rows2018 = [["Not over P 250,000", "0%"], ["Over P 250,000 but not over P 400,000", "20% of the excess over P 250,000"], ["Over P 400,000 but not over P 800,000", "P 30,000 + 25% of the excess over P 400,000"], ["Over P 800,000 but not over P 2,000,000", "P 130,000 + 30% of the excess over P 800,000"], ["Over P 2,000,000 but not over P 8,000,000", "P 490,000 + 32% of the excess over P 2,000,000"], ["Over P 8,000,000", "P 2,410,000 + 35% of the excess over P 8,000,000"]] as const;
  const rows2023 = [["Not over P 250,000", "0%"], ["Over P 250,000 but not over P 400,000", "15% of the excess over P 250,000"], ["Over P 400,000 but not over P 800,000", "P 22,500 + 20% of the excess over P 400,000"], ["Over P 800,000 but not over P 2,000,000", "P 102,500 + 25% of the excess over P 800,000"], ["Over P 2,000,000 but not over P 8,000,000", "P 402,500 + 30% of the excess over P 2,000,000"], ["Over P 8,000,000", "P 2,202,500 + 35% of the excess over P 8,000,000"]] as const;
  return <section className="tax-tables-1701"><TaxTable1701 title="TABLE 1 - Tax Rates (effective January 1, 2018 to December 31, 2022)" rows={rows2018} /><TaxTable1701 title="TABLE 2 - Tax Rates (effective January 1, 2023 and onwards)" rows={rows2023} /></section>;
}

function TaxTable1701({ title, rows }: { title: string; rows: ReadonlyArray<readonly [string, string]> }) {
  return <div><h3>{title}</h3><div className="tax-table-head-1701"><span>If Taxable Income is:</span><span>Tax Due is:</span></div>{rows.map(([range, due]) => <div className="tax-table-row-1701" key={range}><span>{range}</span><span>{due}</span></div>)}</div>;
}
