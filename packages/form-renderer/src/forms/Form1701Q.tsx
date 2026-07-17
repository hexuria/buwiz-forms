import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import { Fragment, type ReactNode } from "react";
import {
  AdaptiveCombValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary,
  formatMoneyParts
} from "../components";
import { bool, field, text } from "../values";
import {
  OFFICIAL_1701Q_PDF417_PATHS,
  OFFICIAL_1701Q_PDF417_PAYLOADS,
  OFFICIAL_1701Q_SEAL
} from "./official1701QAssets";
import "./Form1701Q.css";

type AmountRow = readonly [number, ReactNode, string];

const PART_THREE_ROWS: readonly AmountRow[] = [
  [26, <>Tax Due <em>(From Part V, Schedule I-Item 46 OR Schedule II-Item 54)</em></>, "item_26"],
  [27, <>Less: Tax Credits/Payments <em>(From Part V, Schedule III-Item 62)</em></>, "item_27"],
  [28, <>Tax Payable/(Overpayment) <em>(Item 26 Less Item 27) (From Part V, Item 63)</em></>, "item_28"],
  [29, <>Add: Total Penalties <em>(From Part V, Schedule IV-Item 67)</em></>, "item_29"],
  [30, <>Total Amount Payable/(Overpayment) <em>(Sum of Items 28 and 29) (From Part V, Item 68)</em></>, "item_30"]
];

const GRADUATED_ROWS: readonly AmountRow[] = [
  [36, <>Sales/Revenues/Receipts/Fees <small>(net of sales returns, allowances and discounts)</small></>, "item_36"],
  [37, <>Less: Cost of Sales/Services <strong>(applicable only if availing Itemized Deductions)</strong></>, "item_37"],
  [38, <>Gross Income/(Loss) from Operation <small>(Item 36 Less Item 37)</small></>, "item_38"],
  [39, "Total Allowable Itemized Deductions", "item_39"],
  [40, <>Optional Standard Deduction (OSD) <small>(40% of Item 36)</small></>, "item_40"],
  [41, <>Net Income/(Loss) This Quarter <small>(If Itemized: Item 38 Less Item 39; If OSD: Item 38 Less Item 40)</small></>, "item_41"],
  [42, "Taxable Income/(Loss) Previous Quarter/s", "item_42"],
  [43, <>Non-operating Income <small>(specify)</small></>, "item_43"],
  [44, <>Amount Received/Share in Income by a Partner from General Professional Partnership (GPP)</>, "item_44"],
  [45, <>Total Taxable Income/(Loss) To Date <small>(Sum of Items 41 to 44)</small></>, "item_45"],
  [46, <>TAX DUE <small>(Item 45 x Applicable Tax Rate based on Tax Table below) (To Part III, Item 26)</small></>, "item_46"]
];

const EIGHT_PERCENT_ROWS: readonly AmountRow[] = [
  [47, <>Sales/Revenues/Receipts/Fees <small>(net of sales returns, allowances and discounts)</small></>, "item_47"],
  [48, <>Add: Non-operating Income <small>(specify)</small></>, "item_48"],
  [49, <>Total Income for the quarter <small>(Sum of Items 47 and 48)</small></>, "item_49"],
  [50, <>Add: Total Taxable Income/(Loss) Previous Quarter <small>(Item 51 of previous quarter)</small></>, "item_50"],
  [51, <>Cumulative Taxable Income/(Loss) as of This Quarter <small>(Sum of Items 49 and 50)</small></>, "item_51"],
  [52, <>Less: Allowable reduction from gross sales/receipts and other non-operating income of purely self-employed individuals and/or professionals in the amount of P 250,000</>, "item_52"],
  [53, <>Taxable Income/(Loss) To Date <small>(Item 51 Less Item 52)</small></>, "item_53"],
  [54, <>TAX DUE <small>(Item 53 x 8% Tax Rate) (To Part III, Item 26)</small></>, "item_54"]
];

const CREDIT_ROWS: readonly AmountRow[] = [
  [55, "Prior Year’s Excess Credits", "item_55"],
  [56, "Tax Payment/s for the Previous Quarter/s", "item_56"],
  [57, "Creditable Tax Withheld for the Previous Quarter/s", "item_57"],
  [58, "Creditable Tax Withheld per BIR Form No. 2307 for this Quarter", "item_58"],
  [59, "Tax Paid in Return Previously Filed, if this is an Amended Return", "item_59"],
  [60, "Foreign Tax Credits, if applicable", "item_60"],
  [61, <>Other Tax Credits/Payments <small>(specify)</small></>, "item_61"],
  [62, <>Total Tax Credits/Payments <small>(Sum of Items 55 to 61) (To Part III, Item 27)</small></>, "item_62"]
];

const PENALTY_ROWS: readonly AmountRow[] = [
  [64, "Surcharge", "item_64"],
  [65, "Interest", "item_65"],
  [66, "Compromise", "item_66"],
  [67, <>Total Penalties <small>(Sum of Items 64 to 66) (To Part III, Item 29)</small></>, "item_67"]
];

const PAYMENT_ROWS = [
  [32, "Cash/Bank Debit Memo", "payment_32"],
  [33, "Check", "payment_33"],
  [34, "Tax Debit Memo", "payment_34"],
  [35, "Others (specify)", "payment_35"]
] as const;

const PLAIN_DESCRIPTION_ITEMS = new Set([43, 48, 61]);

export function Form1701Q({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("1701Q", "2018");
  if (envelope.schedules.length !== 0) {
    throw new Error("1701Q does not accept repeatable renderer schedules");
  }

  return (
    <main className="form-document" data-form-code="1701Q">
      <FolioPage pageNumber={1} paper="folio" className="form-1701q-page form-1701q-page-one">
        <GovernmentHeader1701Q />
        <Masthead1701Q page={1} />
        <HeaderOptions1701Q envelope={envelope} />
        <TaxpayerBackground1701Q envelope={envelope} />
        <SpouseBackground1701Q envelope={envelope} />
        <PairedAmountSection1701Q title="PART III – TOTAL TAX PAYABLE" rows={PART_THREE_ROWS} envelope={envelope} />
        <AggregateAmount1701Q envelope={envelope} />
        <Declaration1701Q />
        <PaymentDetails1701Q envelope={envelope} />
        <p className="notes-1701q">
          <b>NOTES:</b> * I understand that this choice is irrevocable for this taxable year. However, the 8% Income Tax (IT) Rate option if initially selected shall automatically be<br />
          changed to graduated IT rates when gross sales/receipts and other non-operating income exceed Three million pesos (P3M)<br />
          ** Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)
        </p>
      </FolioPage>

      <FolioPage pageNumber={2} paper="folio" className="form-1701q-page form-1701q-page-two">
        <Masthead1701Q page={2} compact />
        <PageTwoIdentity1701Q envelope={envelope} />
        <PartFive1701Q envelope={envelope} />
        <TaxTables1701Q />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader1701Q() {
  return (
    <header className="government-header-1701q">
      <span>For BIR<br />Use Only</span>
      <span>BCS/<br />Item:</span>
      <span className="government-wordmark-1701q">
        <img src={OFFICIAL_1701Q_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span>
      </span>
    </header>
  );
}

function Masthead1701Q({ page, compact = false }: { page: 1 | 2; compact?: boolean }) {
  const payload = OFFICIAL_1701Q_PDF417_PAYLOADS[page];
  return (
    <header className={`masthead-1701q${compact ? " compact-1701q" : ""}`}>
      <div className="form-number-1701q">
        <span>BIR Form No.</span>
        <strong>1701Q</strong>
        <small>January 2018 (ENCS)</small>
        <b>Page {page}</b>
      </div>
      <div className="form-title-1701q">
        <strong>Quarterly Income Tax Return</strong>
        <span>For Individuals, Estates and Trusts</span>
        {!compact && <em>Enter all required information in CAPITAL LETTERS using BLACK ink. Mark all applicable boxes with an “X”. Two copies must be filed with the BIR and one held by the Tax Filer.</em>}
      </div>
      <div className="barcode-1701q" aria-label={payload} data-barcode-page={page}>
        <span className="official-pdf417-object-1701q" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-1701q"
            viewBox="0 0 120 7"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_1701Q_PDF417_PATHS[page]} />
          </svg>
        </span>
        <small>{payload}</small>
      </div>
    </header>
  );
}

function HeaderOptions1701Q({ envelope }: { envelope: RenderEnvelope }) {
  const quarter = envelope.period.quarter ?? 0;
  return (
    <section className="header-options-1701q" aria-label="Return period and options">
      <div><b>1</b> For the Year <CombValue value={String(envelope.period.taxable_year)} cells={4} /></div>
      <div className="quarter-1701q"><b>2</b> Quarter
        <CheckChoice checked={quarter === 1} label="First" />
        <CheckChoice checked={quarter === 2} label="Second" />
        <CheckChoice checked={quarter === 3} label="Third" />
      </div>
      <div><b>3</b> Amended Return?
        <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "is_amended")} label="No" />
      </div>
      <div><b>4</b> Number of<br />Sheets Attached <CombValue value={text(envelope, "attached_sheets")} cells={2} align="right" /></div>
    </section>
  );
}

function TaxpayerBackground1701Q({ envelope }: { envelope: RenderEnvelope }) {
  const filerType = text(envelope, "filer_type");
  const taxRate = text(envelope, "tax_rate_choice");
  const deduction = text(envelope, "deduction_method");
  const claimsField = field(envelope, "claims_foreign_tax_credit");
  const claimsForeignTaxCredit = claimsField?.type === "boolean" ? claimsField.value : undefined;
  return (
    <section className="part-1701q taxpayer-background-1701q">
      <h2>PART I – BACKGROUND INFORMATION ON TAXPAYER/FILER</h2>
      <div className="tin-rdo-1701q">
        <span><b>5</b> Taxpayer Identification Number (TIN)</span>
        <Tin1701Q value={envelope.taxpayer.tin} />
        <span><b>6</b> RDO Code</span>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <ChoiceRow1701Q number="7" label="Taxpayer/Filer Type" choices={[
        ["single_proprietor", "Single Proprietor"], ["professional", "Professional"], ["estate", "Estate"], ["trust", "Trust"]
      ]} selected={filerType} />
      <ChoiceRow1701Q number="8" label="Alphanumeric Tax Code (ATC)" compact choices={[
        ["II012", "II012 Business Income-Graduated IT Rates"], ["II014", "II014 Income from Profession-Graduated IT Rates"], ["II013", "II013 Mixed Income-Graduated IT Rates"],
        ["II015", "II015 Business Income - 8% IT Rate"], ["II017", "II017 Income from Profession - 8% IT Rate"], ["II016", "II016 Mixed Income - 8% IT Rate"]
      ]} selected={text(envelope, "atc")} />
      <LabelComb1701Q number="9" label={<>Taxpayer/Filer’s Name <em>(Last Name, First Name, Middle Name for Individual) / ESTATE of (First Name, Middle Name, Last Name) / TRUST FAO:(First Name, Middle Name, Last Name)</em></>} value={envelope.taxpayer.name} cells={40} />
      <Address1701Q envelope={envelope} />
      <div className="split-row-1701q three-1701q">
        <LabelComb1701Q number="11" label={<>Date of Birth <em>(MM/DD/YYYY)</em></>} value={text(envelope, "date_of_birth").replace(/\D/g, "")} cells={8} />
        <LabelComb1701Q number="12" label="Email Address" value={envelope.taxpayer.email} cells={30} />
      </div>
      <div className="split-row-1701q citizenship-1701q">
        <LabelComb1701Q number="13" label="Citizenship" value={text(envelope, "citizenship")} cells={16} />
        <LabelComb1701Q number="14" label="Foreign Tax Number (if applicable)" value={text(envelope, "foreign_tax_number")} cells={20} />
        <div className="claiming-credit-1701q"><b>15</b> Claiming Foreign Tax Credits?
          <CheckChoice checked={claimsForeignTaxCredit === true} label="Yes" />
          <CheckChoice checked={claimsForeignTaxCredit === false} label="No" />
        </div>
      </div>
      <TaxElection1701Q number="16" taxRate={taxRate} deduction={deduction} />
    </section>
  );
}

function SpouseBackground1701Q({ envelope }: { envelope: RenderEnvelope }) {
  const spouseType = text(envelope, "spouse_filer_type");
  const claimsField = field(envelope, "spouse_claims_foreign_tax_credit");
  const claimsForeignTaxCredit = claimsField?.type === "boolean" ? claimsField.value : undefined;
  return (
    <section className="part-1701q spouse-background-1701q">
      <h2>PART II – BACKGROUND INFORMATION ON SPOUSE (if applicable)</h2>
      <div className="tin-rdo-1701q">
        <span><b>17</b> Spouse’s TIN</span>
        <Tin1701Q value={text(envelope, "spouse_tin")} />
        <span><b>18</b> RDO Code</span>
        <CombValue value={text(envelope, "spouse_rdo_code")} cells={3} align="right" />
      </div>
      <ChoiceRow1701Q number="19" label="Filer’s Spouse Type" choices={[
        ["single_proprietor", "Single Proprietor"], ["professional", "Professional"], ["compensation_earner", "Compensation Earner"]
      ]} selected={spouseType} />
      <ChoiceRow1701Q number="20" label="ATC" compact choices={[
        ["II012", "II012 Business Income-Graduated IT Rates"], ["II014", "II014 Income from Profession-Graduated IT Rates"], ["II013", "II013 Mixed Income-Graduated IT Rates"], ["II011", "II011 Compensation Income"],
        ["II015", "II015 Business Income - 8% IT Rate"], ["II017", "II017 Income from Profession - 8% IT Rate"], ["II016", "II016 Mixed Income - 8% IT Rate"]
      ]} selected={text(envelope, "spouse_atc")} />
      <LabelComb1701Q number="21" label="Spouse’s Name (Last Name, First Name, Middle Name)" value={text(envelope, "spouse_name")} cells={40} />
      <div className="split-row-1701q citizenship-1701q">
        <LabelComb1701Q number="22" label="Citizenship" value={text(envelope, "spouse_citizenship")} cells={16} />
        <LabelComb1701Q number="23" label="Foreign Tax Number, if applicable" value={text(envelope, "spouse_foreign_tax_number")} cells={20} />
        <div className="claiming-credit-1701q"><b>24</b> Claiming Foreign Tax Credits?
          <CheckChoice checked={claimsForeignTaxCredit === true} label="Yes" />
          <CheckChoice checked={claimsForeignTaxCredit === false} label="No" />
        </div>
      </div>
      <TaxElection1701Q number="25" taxRate={text(envelope, "spouse_tax_rate_choice")} deduction={text(envelope, "spouse_deduction_method")} />
    </section>
  );
}

function ChoiceRow1701Q({ number, label, choices, selected, compact = false }: {
  number: string;
  label: string;
  choices: ReadonlyArray<readonly [string, string]>;
  selected: string;
  compact?: boolean;
}) {
  return (
    <div className={`choice-row-1701q${compact ? " compact-choices-1701q" : ""}`}>
      <span><b>{number}</b> {label}</span>
      {choices.map(([value, choice]) => <CheckChoice key={value} checked={selected === value} label={choice} />)}
    </div>
  );
}

function TaxElection1701Q({ number, taxRate, deduction }: { number: string; taxRate: string; deduction: string }) {
  return (
    <div className="tax-election-1701q">
      <span className="tax-rate-label-1701q"><b>{number}</b> Tax Rate*<small>(choose one, for income from business/profession)</small></span>
      <span className="graduated-choice-1701q"><CheckChoice checked={taxRate === "graduated"} label="Graduated Rates" /><small>per Tax Table- page 2</small><small>(Choose Method of Deduction in Item {number}A)</small></span>
      <span className="deduction-heading-1701q"><b>{number}A</b> Method of Deduction</span>
      <span className="deduction-choices-1701q"><span><CheckChoice checked={deduction === "itemized"} label="Itemized Deduction" /><small>[Sec. 34(A-J), NIRC]</small></span><span><CheckChoice checked={deduction === "osd"} label="Optional Standard Deduction (OSD)" /><small>[40% of Gross Sales/Receipts/Revenues/Fees [Sec. 34(L), NIRC]</small></span></span>
      <span className="eight-percent-choice-1701q"><span><CheckChoice checked={taxRate === "eight_percent"} label="8% on gross sales/receipts & other non-operating income in lieu of Graduated Rates under Sec. 24(A)(2)(a) & Percentage Tax under Sec. 116 of the NIRC, as amended" /><small>(available if gross sales/receipts and other non-operating income do not exceed Three million pesos (P3M))</small></span></span>
    </div>
  );
}

function Address1701Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <div className="address-1701q">
      <div><b>10</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></div>
      <AdaptiveCombValue value={envelope.taxpayer.registered_address.toUpperCase()} cells={40} />
      <div className="address-second-1701q"><AdaptiveCombValue value={text(envelope, "registered_address_2").toUpperCase()} cells={31} /><span><b>10A</b> ZIP Code</span><CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" /></div>
    </div>
  );
}

function LabelComb1701Q({ number, label, value, cells }: { number: string; label: ReactNode; value: string; cells: number }) {
  return (
    <div className="label-comb-1701q" data-item-number={number} data-field-mode="guided" data-cell-capacity={cells}><div><b>{number}</b> {label}</div><AdaptiveCombValue value={value.toUpperCase()} cells={cells} /></div>
  );
}

function Tin1701Q({ value }: { value: string }) {
  const digits = value.replace(/\D/g, "");
  return <span className="tin-1701q"><CombValue value={digits.slice(0, 3)} cells={3} /><i>-</i><CombValue value={digits.slice(3, 6)} cells={3} /><i>-</i><CombValue value={digits.slice(6, 9)} cells={3} /><i>-</i><CombValue value={digits.slice(9, 14)} cells={5} /></span>;
}

function PairedAmountSection1701Q({ title, rows, envelope, graduatedLayout = false }: {
  title: string;
  rows: readonly AmountRow[];
  envelope: RenderEnvelope;
  graduatedLayout?: boolean;
}) {
  return (
    <section className={`paired-section-1701q schedule-${rows[0]?.[0] ?? "empty"}-1701q${graduatedLayout ? " graduated-schedule-1701q" : ""}`}>
      <h2>{title}</h2>
      <div className="paired-header-1701q"><span>Particulars</span><span>A) Taxpayer/Filer</span><span>B) Spouse</span></div>
      {rows.map(([number, label, key]) => (
        <Fragment key={number}>
          {graduatedLayout && number === 39 && <div className="deduction-band-1701q">Less: Allowable Deductions</div>}
          {graduatedLayout && number === 40 && <div className="deduction-or-1701q">OR</div>}
          <PairedAmountRow1701Q number={number} label={label} fieldKey={key} envelope={envelope} />
        </Fragment>
      ))}
    </section>
  );
}

function PairedAmountRow1701Q({ number, label, fieldKey, envelope }: { number: number; label: ReactNode; fieldKey: string; envelope: RenderEnvelope }) {
  const description = text(envelope, `${fieldKey}_description`);
  const hasPlainDescription = PLAIN_DESCRIPTION_ITEMS.has(number);
  return (
    <div className={`paired-row-1701q item-${number}-1701q`}>
      <span><b>{number}</b> {label}{hasPlainDescription && <span className="line-description-1701q" data-field-mode="plain" data-field-name={`${fieldKey}_description`}>{description}</span>}</span>
      <Amount1701Q envelope={envelope} fieldKey={`${fieldKey}_taxpayer`} />
      <Amount1701Q envelope={envelope} fieldKey={`${fieldKey}_spouse`} />
    </div>
  );
}

function Amount1701Q({ envelope, fieldKey, cells = 8 }: { envelope: RenderEnvelope; fieldKey: string; cells?: number }) {
  const value = field(envelope, fieldKey);
  if (!value || value.type !== "decimal") {
    return <span className="amount-1701q blank-amount-1701q" data-field-mode="guided" data-field-name={fieldKey} data-cell-capacity={cells} aria-label="blank amount"><CombValue value="" cells={cells} /></span>;
  }
  const [whole] = formatMoneyParts(value.value);
  return <span className="amount-1701q" data-field-mode="guided" data-field-name={fieldKey} data-cell-capacity={cells}><AdaptiveCombValue value={whole} cells={cells} align="right" /></span>;
}

function AggregateAmount1701Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <div className="aggregate-1701q"><span><b>31</b> Aggregate Amount Payable/(Overpayment) <small>(Sum of Items 30A and 30B)</small></span><Amount1701Q envelope={envelope} fieldKey="item_31" cells={12} /></div>
  );
}

function Declaration1701Q() {
  return (
    <section className="declaration-1701q">
      <p>I declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me, and to the best of my knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as contemplated under the <b>Data Privacy Act of 2012 (R.A. No. 10173)</b> for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter and indicate TIN)</em></p>
      <div><span>Signature and Printed Name of Taxpayer/Authorized Representative/Tax Agent <small>(Indicate Title/Designation and TIN)</small></span></div>
    </section>
  );
}

function PaymentDetails1701Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="payment-details-1701q">
      <h2>PART IV – DETAILS OF PAYMENT</h2>
      <div className="payment-header-1701q"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date (MM/DD/YYYY)</span><span>Amount</span></div>
      {PAYMENT_ROWS.map(([number, label, key]) => (
        <Fragment key={number}>
          {number !== 35 ? (
            <div className="payment-row-1701q">
              <span><b>{number}</b> {label}</span>
              <GuidedPaymentField1701Q fieldName={`${key}_bank`} value={text(envelope, `${key}_bank`)} cells={6} />
              <GuidedPaymentField1701Q fieldName={`${key}_number`} value={text(envelope, `${key}_number`)} cells={11} />
              <GuidedPaymentField1701Q fieldName={`${key}_date`} value={text(envelope, `${key}_date`).replace(/\D/g, "")} cells={8} />
              <Amount1701Q envelope={envelope} fieldKey={`${key}_amount`} />
            </div>
          ) : (
            <>
              <div className="payment-row-1701q payment-row-others-label-1701q">
                <span><b>{number}</b> {label}</span>
              </div>
              <div className="payment-row-1701q payment-row-others-detail-1701q">
                <GuidedPaymentField1701Q fieldName={`${key}_particular`} value={text(envelope, `${key}_particular`)} cells={7} />
                <GuidedPaymentField1701Q fieldName={`${key}_bank`} value={text(envelope, `${key}_bank`)} cells={6} />
                <GuidedPaymentField1701Q fieldName={`${key}_number`} value={text(envelope, `${key}_number`)} cells={11} />
                <GuidedPaymentField1701Q fieldName={`${key}_date`} value={text(envelope, `${key}_date`).replace(/\D/g, "")} cells={8} />
                <Amount1701Q envelope={envelope} fieldKey={`${key}_amount`} />
              </div>
            </>
          )}
        </Fragment>
      ))}
      <div className="payment-receipt-1701q">
        <span>Machine Validation/Revenue Official Receipt Details <em>(if not filed with an Authorized Agent Bank)</em><span className="receipt-value-1701q" data-field-mode="plain" data-field-name="machine_validation_or_receipt_details">{text(envelope, "machine_validation_or_receipt_details")}</span></span>
        <span>Stamp of Receiving Office/AAB and Date of Receipt<br />(RO’s Signature/Bank Teller’s Initial)</span>
      </div>
    </section>
  );
}

function GuidedPaymentField1701Q({ fieldName, value, cells }: { fieldName: string; value: string; cells: number }) {
  return <span className="guided-payment-field-1701q" data-field-mode="guided" data-field-name={fieldName} data-cell-capacity={cells}><AdaptiveCombValue value={value} cells={cells} /></span>;
}

function PageTwoIdentity1701Q({ envelope }: { envelope: RenderEnvelope }) {
  const lastName = text(envelope, "taxpayer_last_name");
  return (
    <div className="page-two-identity-1701q">
      <span className="identity-label-1701q">TIN</span>
      <span className="identity-label-1701q">Taxpayer/Filer’s Last Name</span>
      <CombValue value={envelope.taxpayer.tin.replace(/\D/g, "")} cells={14} />
      <AdaptiveCombValue value={lastName.toUpperCase()} cells={30} />
    </div>
  );
}

function PartFive1701Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-five-1701q">
      <h2>PART V – COMPUTATION OF TAX DUE <small>(DO NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small></h2>
      <div className="part-five-column-header-1701q"><span>Declaration this Quarter</span><span>A) Taxpayer/Filer</span><span>B) Spouse</span></div>
      <div className="part-five-instruction-1701q">If graduated rate, fill in Items 36 to 46; if 8%, fill in Items 47 to 54</div>
      <PairedAmountSection1701Q title="Schedule I – For Graduated IT Rate" rows={GRADUATED_ROWS} envelope={envelope} graduatedLayout />
      <PairedAmountSection1701Q title="Schedule II – For 8% IT Rate" rows={EIGHT_PERCENT_ROWS} envelope={envelope} />
      <PairedAmountSection1701Q title="Schedule III - Tax Credits/Payments" rows={CREDIT_ROWS} envelope={envelope} />
      <div className="single-total-1701q"><span><b>63</b> Tax Payable/(Overpayment) <small>(Item 46 or 54, Less Item 62) (To Part III, Item 28)</small></span><Amount1701Q envelope={envelope} fieldKey="item_63_taxpayer" /><Amount1701Q envelope={envelope} fieldKey="item_63_spouse" /></div>
      <PairedAmountSection1701Q title="Schedule IV - Penalties" rows={PENALTY_ROWS} envelope={envelope} />
      <div className="single-total-1701q final-total-1701q"><span><b>68</b> Total Amount Payable/(Overpayment) <small>(Sum of Items 63 and 67) (To Part III, Item 30)</small></span><Amount1701Q envelope={envelope} fieldKey="item_68_taxpayer" /><Amount1701Q envelope={envelope} fieldKey="item_68_spouse" /></div>
    </section>
  );
}

function TaxTables1701Q() {
  return (
    <section className="tax-tables-1701q">
      <TaxTable1701Q title="TABLE 1 – Tax Rates (effective January 1, 2018 to December 31, 2022)" rows={[
        ["Not over P 250,000", "0%"], ["Over P 250,000 but not over P 400,000", "20% of the excess over P 250,000"], ["Over P 400,000 but not over P 800,000", "P 30,000 + 25% of the excess over P 400,000"], ["Over P 800,000 but not over P 2,000,000", "P 130,000 + 30% of the excess over P 800,000"], ["Over P 2,000,000 but not over P 8,000,000", "P 490,000 + 32% of the excess over P 2,000,000"], ["Over P 8,000,000", "P 2,410,000 + 35% of the excess over P 8,000,000"]
      ]} />
      <TaxTable1701Q title="TABLE 2 – Tax Rates (effective January 1, 2023 and onwards)" rows={[
        ["Not over P 250,000", "0%"], ["Over P 250,000 but not over P 400,000", "15% of the excess over P 250,000"], ["Over P 400,000 but not over P 800,000", "P 22,500 + 20% of the excess over P 400,000"], ["Over P 800,000 but not over P 2,000,000", "P 102,500 + 25% of the excess over P 800,000"], ["Over P 2,000,000 but not over P 8,000,000", "P 402,500 + 30% of the excess over P 2,000,000"], ["Over P 8,000,000", "P 2,202,500 + 35% of the excess over P 8,000,000"]
      ]} />
    </section>
  );
}

function TaxTable1701Q({ title, rows }: { title: string; rows: ReadonlyArray<readonly [string, string]> }) {
  return <div className="tax-table-1701q"><h3>{title}</h3><div className="tax-table-head-1701q"><span>If Taxable Income is:</span><span>Tax Due is:</span></div>{rows.map(([range, due]) => <div key={range}><span>{range}</span><span>{due}</span></div>)}</div>;
}
