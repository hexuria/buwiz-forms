import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import { Fragment, type ReactNode } from "react";
import {
  AdaptiveCombValue,
  AdaptivePlainValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary,
  formatMoneyParts
} from "../components";
import { bool, field, integer, text } from "../values";
import {
  OFFICIAL_2550Q_PDF417_PATHS,
  OFFICIAL_2550Q_PDF417_PAYLOADS,
  OFFICIAL_2550Q_SEAL
} from "./official2550QAssets";
import "./Form2550Q.css";

const GUIDED_OVERFLOW_MAX_FONT_PX = 9.6;
const GUIDED_OVERFLOW_MIN_FONT_PX = 8;
const PLAIN_FIELD_MAX_FONT_PX = 8.25;
const PLAIN_FIELD_MIN_FONT_PX = 6.75;
const ADAPTIVE_FONT_STEP_PX = 0.5;

type AmountRow = Readonly<{
  item: number;
  label: ReactNode;
  key: string;
  emphasis?: "subtotal" | "total";
}>;

const PART_TWO_ROWS: readonly AmountRow[] = [
  { item: 15, label: <>Net VAT Payable/(Excess Input Tax) <em>(From Part IV, Item 61)</em></>, key: "item_15" },
  { item: 16, label: <>Creditable VAT Withheld <em>(From Part V - Schedule 3, Column D)</em></>, key: "item_16" },
  { item: 17, label: <>Advance VAT Payments <em>(From Part V - Schedule 4)</em></>, key: "item_17" },
  { item: 18, label: "VAT paid in return previously filed, if this is an amended return", key: "item_18" },
  { item: 19, label: <>Other Credits/Payment <em>(Specify)</em></>, key: "item_19" },
  { item: 20, label: <>Total Tax Credits/Payment <em>(Sum of Items 16 to 19)</em></>, key: "item_20", emphasis: "subtotal" },
  { item: 21, label: <>Tax Still Payable/(Excess Credits) <em>(Item 15 Less Item 20)</em></>, key: "item_21", emphasis: "subtotal" },
  { item: 22, label: "Surcharge", key: "item_22" },
  { item: 23, label: "Interest", key: "item_23" },
  { item: 24, label: "Compromise", key: "item_24" },
  { item: 25, label: <>Total Penalties <em>(Sum of Items 22 to 24)</em></>, key: "item_25", emphasis: "subtotal" },
  { item: 26, label: <>TOTAL AMOUNT PAYABLE/(Excess Credits) <em>(Sum of Items 21 and 25)</em></>, key: "item_26", emphasis: "total" }
];

const PART_FOUR_ROWS: readonly Readonly<{
  item: number;
  label: ReactNode;
  a?: string;
  b?: string;
  disabledA?: boolean;
  disabledB?: boolean;
  emphasis?: "subtotal" | "total";
}>[] = [
  { item: 31, label: "VATable Sales", a: "item_31a", b: "item_31b" },
  { item: 32, label: "Zero-Rated Sales", a: "item_32a", disabledB: true },
  { item: 33, label: "Exempt Sales", a: "item_33a", disabledB: true },
  { item: 34, label: <>Total Sales &amp; Output Tax Due <em>(Sum of Items 31A to 33A) / (Item 31B)</em></>, a: "item_34a", b: "item_34b", emphasis: "subtotal" },
  { item: 35, label: "Less: Output VAT on Uncollected Receivables", disabledA: true, b: "item_35b" },
  { item: 36, label: "Add: Output VAT on Recovered Uncollected Receivables Previously Deducted", disabledA: true, b: "item_36b" },
  { item: 37, label: <>Total Adjusted Output Tax Due <em>(Item 34B Less Item 35B Add Item 36B)</em></>, disabledA: true, b: "item_37b", emphasis: "subtotal" },
  { item: 38, label: "Input Tax Carried Over from Previous Quarter", disabledA: true, b: "item_38b" },
  { item: 39, label: <>Input Tax Deferred on Capital Goods Exceeding P1 Million from Previous Quarter <em>(From Part V - Schedule 1 Col E)</em></>, disabledA: true, b: "item_39b" },
  { item: 40, label: "Transitional Input Tax", disabledA: true, b: "item_40b" },
  { item: 41, label: "Presumptive Input Tax", disabledA: true, b: "item_41b" },
  { item: 42, label: <>Others <em>(Specify)</em></>, disabledA: true, b: "item_42b" },
  { item: 43, label: <>Total <em>(Sum of Items 38B to 42B)</em></>, disabledA: true, b: "item_43b", emphasis: "subtotal" },
  { item: 44, label: "Domestic Purchases", a: "item_44a", b: "item_44b" },
  { item: 45, label: "Services Rendered by Non-Residents", a: "item_45a", b: "item_45b" },
  { item: 46, label: "Importations", a: "item_46a", b: "item_46b" },
  { item: 47, label: <>Others <em>(Specify)</em></>, a: "item_47a", b: "item_47b" },
  { item: 48, label: "Domestic Purchases with No Input Tax", a: "item_48a", disabledB: true },
  { item: 49, label: "VAT- Exempt Importations", a: "item_49a", disabledB: true },
  { item: 50, label: <>Total Current Purchases/Input Tax <em>(Sum of Items 44A to 49A)/(Sum of Items 44B to 47B)</em></>, a: "item_50a", b: "item_50b", emphasis: "subtotal" },
  { item: 51, label: <>Total Available Input Tax <em>(Sum of Items 43B and 50B)</em></>, disabledA: true, b: "item_51b", emphasis: "subtotal" },
  { item: 52, label: <>Input Tax on Purchases/Importation of Capital Goods exceeding P1 Million deferred for the succeeding period <em>(From Part V Schedule 1, Column I)</em></>, disabledA: true, b: "item_52b" },
  { item: 53, label: <>Input Tax Attributable to VAT Exempt Sales <em>(From Part V - Schedule 2)</em></>, disabledA: true, b: "item_53b" },
  { item: 54, label: "VAT Refund/TCC Claimed", disabledA: true, b: "item_54b" },
  { item: 55, label: "Input VAT on Unpaid Payables", disabledA: true, b: "item_55b" },
  { item: 56, label: <>Others <em>(Specify)</em></>, disabledA: true, b: "item_56b" },
  { item: 57, label: <>Total Deductions from Input Tax <em>(Sum of Items 52B to 56B)</em></>, disabledA: true, b: "item_57b", emphasis: "subtotal" },
  { item: 58, label: "Add: Input VAT on Settled Unpaid Payables Previously Deducted", disabledA: true, b: "item_58b" },
  { item: 59, label: <>Adjusted Deductions from Input Tax <em>(Sum of Items 57B and 58B)</em></>, disabledA: true, b: "item_59b", emphasis: "subtotal" },
  { item: 60, label: <>Total Allowable Input Tax <em>(Item 51B Less Item 59B)</em></>, disabledA: true, b: "item_60b", emphasis: "subtotal" },
  { item: 61, label: <>Net VAT Payable/(Excess Input Tax) <em>(Item 37B Less Item 60B) (To Part II, Item 15)</em></>, disabledA: true, b: "item_61b", emphasis: "total" }
];

export function Form2550Q({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("2550Q", "2024");
  if (envelope.schedules.length !== 0) {
    throw new Error("2550Q uses exact fixed two-row schedule fields, not continuation schedules");
  }

  return (
    <main className="form-document" data-form-code="2550Q">
      <FolioPage pageNumber={1} paper="legal" className="form-2550q-page form-2550q-page-one">
        <GovernmentHeader2550Q />
        <Masthead2550Q page={1} />
        <HeaderOptions2550Q envelope={envelope} />
        <PartOne2550Q envelope={envelope} />
        <PartTwo2550Q envelope={envelope} />
        <Declaration2550Q envelope={envelope} />
        <PartThree2550Q envelope={envelope} />
        <p className="privacy-note-2550q">*NOTE: The BIR Data Privacy Policy is in the BIR website (www.bir.gov.ph)</p>
      </FolioPage>

      <FolioPage pageNumber={2} paper="legal" className="form-2550q-page form-2550q-page-two">
        <Masthead2550Q page={2} compact />
        <PageTwoIdentity2550Q envelope={envelope} />
        <PartFour2550Q envelope={envelope} />
        <PartFive2550Q envelope={envelope} />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader2550Q() {
  return (
    <header className="government-header-2550q">
      <span>For BIR<br />Use Only</span>
      <span>BCS/<br />Item:</span>
      <span className="government-wordmark-2550q">
        <img src={OFFICIAL_2550Q_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span>
      </span>
    </header>
  );
}

function Masthead2550Q({ page, compact = false }: { page: 1 | 2; compact?: boolean }) {
  const payload = OFFICIAL_2550Q_PDF417_PAYLOADS[page];
  return (
    <header className={`masthead-2550q${compact ? " compact-2550q" : ""}`}>
      <div className="form-number-2550q">
        <span>BIR Form No.</span>
        <strong>2550Q</strong>
        <small>April 2024 (ENCS)</small>
        <b>Page {page}</b>
      </div>
      <div className="form-title-2550q">
        <strong>Quarterly Value-Added Tax<br />(VAT) Return</strong>
        {!compact && <em>Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable boxes with an “X”. Two copies MUST be filed with the BIR and one held by the Taxpayer.</em>}
      </div>
      <div className="barcode-2550q" aria-label={payload} data-barcode-page={page}>
        <span className="official-pdf417-object-2550q" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-2550q"
            viewBox="0 0 120 7"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_2550Q_PDF417_PATHS[page]} />
          </svg>
        </span>
        <small>{payload}</small>
      </div>
    </header>
  );
}

function HeaderOptions2550Q({ envelope }: { envelope: RenderEnvelope }) {
  const basis = text(envelope, "filing_basis");
  const quarter = integer(envelope, "quarter");
  const yearEnded = `${String(integer(envelope, "year_end_month")).padStart(2, "0")}${envelope.period.taxable_year}`;
  return (
    <section className="header-options-2550q" aria-label="Return period and options">
      <div className="header-row-2550q header-row-one-2550q">
        <div className="header-cell-2550q filing-basis-2550q">
          <span><b>1</b> For the</span>
          <span><CheckChoice checked={basis === "calendar"} label="Calendar" /><CheckChoice checked={basis === "fiscal"} label="Fiscal" /></span>
        </div>
        <div className="header-cell-2550q year-ended-2550q">
          <span><b>2</b> Year Ended <em>(MM/YYYY)</em></span>
          <CombValue value={yearEnded} cells={6} align="right" />
        </div>
        <div className="header-cell-2550q quarter-2550q"><span><b>3</b> Quarter</span>
          <span>{[1, 2, 3, 4].map((value) => <CheckChoice key={value} checked={quarter === value} label={`${value}${value === 1 ? "st" : value === 2 ? "nd" : value === 3 ? "rd" : "th"}`} />)}</span>
        </div>
      </div>
      <div className="header-row-2550q header-row-two-2550q">
        <div className="header-cell-2550q return-period-2550q">
          <span><b>4</b> Return Period <em>(MM/DD/YYYY)</em></span>
          <div className="return-period-values-2550q">
            <span>From</span><AdaptiveGuidedValue2550Q value={text(envelope, "return_period_from").replace(/\D/g, "")} cells={8} />
            <span>To</span><AdaptiveGuidedValue2550Q value={text(envelope, "return_period_to").replace(/\D/g, "")} cells={8} />
          </div>
        </div>
        <div className="header-cell-2550q binary-option-2550q">
          <span><b>5</b> Amended Return?</span>
          <span><CheckChoice checked={bool(envelope, "is_amended")} label="Yes" /><CheckChoice checked={!bool(envelope, "is_amended")} label="No" /></span>
        </div>
        <div className="header-cell-2550q binary-option-2550q">
          <span><b>6</b> Short Period Return?</span>
          <span><CheckChoice checked={bool(envelope, "is_short_period_return")} label="Yes" /><CheckChoice checked={!bool(envelope, "is_short_period_return")} label="No" /></span>
        </div>
      </div>
    </section>
  );
}

function PartOne2550Q({ envelope }: { envelope: RenderEnvelope }) {
  const classification = text(envelope, "taxpayer_classification");
  return (
    <section className="part-2550q part-one-2550q">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-2550q">
        <span><b>7</b> Taxpayer Identification Number (TIN)</span>
        <Tin2550Q value={envelope.taxpayer.tin} />
        <span><b>8</b> RDO Code</span>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <FullWidthField2550Q item="9" label={<>Taxpayer’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></>} value={envelope.taxpayer.name} cells={40} className="name-field-2550q" />
      <Address2550Q envelope={envelope} />
      <div className="split-row-2550q">
        <FullWidthField2550Q item="11" label={<>Contact Number <em>(Landline/Cellphone No.)</em></>} value={envelope.taxpayer.contact_number} cells={13} />
        <FullWidthField2550Q item="12" label="Email Address" value={envelope.taxpayer.email} cells={27} />
      </div>
      <div className="classification-2550q">
        <span><b>13</b> Taxpayer Classification</span>
        {[["micro", "Micro"], ["small", "Small"], ["medium", "Medium"], ["large", "Large"]].map(([value, label]) => <CheckChoice key={value} checked={classification === value} label={label} />)}
      </div>
      <div className="tax-relief-2550q">
        <span><b>14</b> Are you availing of tax relief under Special Law or International Tax Treaty?</span>
        <CheckChoice checked={bool(envelope, "is_availing_tax_relief")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "is_availing_tax_relief")} label="No" />
        <span className="tax-relief-detail-2550q"><span><b>14A</b> If yes, specify</span><AdaptiveGuidedValue2550Q value={text(envelope, "tax_relief_details")} cells={17} /></span>
      </div>
    </section>
  );
}

function PartTwo2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-2550q part-two-2550q">
      <h2>Part II – Total Tax Payable</h2>
      {PART_TWO_ROWS.map((row) => (
        <Fragment key={row.item}>
          <div className={`amount-row-2550q ${row.emphasis ? `${row.emphasis}-2550q` : ""}`} data-item-number={row.item}>
            <span className="amount-label-2550q">{row.item === 22 && <span className="penalty-prefix-2550q">Add: Penalties</span>}<b>{row.item}</b>{row.label}{row.item === 19 && <InlineSpecify2550Q value={text(envelope, "item_19_description")} />}</span>
            <MoneyCell2550Q envelope={envelope} fieldKey={row.key} cells={12} />
          </div>
          {row.item === 15 && <div className="part-two-subband-2550q">Less: Tax Credits/Payments</div>}
        </Fragment>
      ))}
    </section>
  );
}

function Declaration2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-2550q">
      <p>I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give my/our consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter and indicate TIN)</em></p>
      <div className="signature-panels-2550q">
        <div><span className="signature-entry-2550q"><span>For Individual:</span><PlainValue2550Q value={text(envelope, "signature_taxpayer_or_representative")} capacity={54} /><PlainValue2550Q value={text(envelope, "signature_representative_title")} capacity={42} /></span><span className="signature-caption-2550q">Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent{" "}<small>(Indicate Title/Designation and TIN)</small></span></div>
        <div><span className="signature-entry-2550q"><span>For Non-Individual:</span><PlainValue2550Q value={text(envelope, "signature_non_individual_officer")} capacity={50} /></span><span className="signature-caption-2550q">Signature over Printed Name of President/Vice President/Authorized Officer or Representative/Tax Agent{" "}<small>(Indicate Title/Designation and TIN)</small></span></div>
      </div>
      <div className="credential-row-2550q">
        <span><span>Tax Agent Accreditation No./Attorney’s Roll No. <em>(If applicable)</em></span><ValueLine2550Q value={text(envelope, "tax_agent_accreditation_or_roll_number")} /></span>
        <span><span>Date of Issue <em>(MM/DD/YYYY)</em></span><ValueLine2550Q value={text(envelope, "tax_agent_date_of_issue")} /></span>
        <span><span>Expiry Date <em>(MM/DD/YYYY)</em></span><ValueLine2550Q value={text(envelope, "tax_agent_date_of_expiry")} /></span>
      </div>
    </section>
  );
}

function PartThree2550Q({ envelope }: { envelope: RenderEnvelope }) {
  const rows = [
    [27, "Cash/Bank Debit Advice", "", "", "", "payment_cash_or_bank_debit_advice_amount"],
    [28, "Check", "payment_check_bank", "payment_check_number", "payment_check_date", "payment_check_amount"],
    [29, "Tax Debit Memo", "", "payment_tax_debit_memo_number", "payment_tax_debit_memo_date", "payment_tax_debit_memo_amount"]
  ] as const;
  return (
    <section className="part-three-2550q">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-header-2550q"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date <em>(MM/DD/YYY)</em></span><span>Amount</span></div>
      {rows.map(([item, label, bank, number, date, amount]) => (
        <div className="payment-row-2550q" key={item}>
          <span><b>{item}</b> {label}</span><AdaptiveGuidedValue2550Q value={bank ? text(envelope, bank) : ""} cells={5} /><AdaptiveGuidedValue2550Q value={number ? text(envelope, number) : ""} cells={6} /><AdaptiveGuidedValue2550Q value={date ? text(envelope, date) : ""} cells={8} /><MoneyCell2550Q envelope={envelope} fieldKey={amount} cells={12} />
        </div>
      ))}
      <div className="payment-other-label-2550q"><b>30</b> Others (Specify below)</div>
      <div className="payment-row-2550q payment-other-values-2550q">
        <AdaptiveGuidedValue2550Q value={text(envelope, "payment_other_description")} cells={6} /><AdaptiveGuidedValue2550Q value={text(envelope, "payment_other_bank")} cells={5} /><AdaptiveGuidedValue2550Q value={text(envelope, "payment_other_number")} cells={6} /><AdaptiveGuidedValue2550Q value={text(envelope, "payment_other_date")} cells={8} /><MoneyCell2550Q envelope={envelope} fieldKey="payment_other_amount" cells={12} />
      </div>
      <div className="receipt-validation-row-2550q">
        <div className="machine-validation-2550q">Machine Validation/Revenue Official Receipt (ROR) Details (if not filed with an Authorized Agent Bank) <PlainValue2550Q value={text(envelope, "machine_validation_or_receipt_details")} capacity={72} /></div>
        <div className="receiving-stamp-2550q">Stamp of Receiving Office/AAB and Date of Receipt<br /><em>(RO’s Signature/Bank Teller’s Initial)</em></div>
      </div>
    </section>
  );
}

function PageTwoIdentity2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <div className="page-two-identity-2550q">
      <div><span>TIN</span><Tin2550Q value={envelope.taxpayer.tin} /></div>
      <div><span>Taxpayer’s Last Name (if Individual)/ Registered Name (if Non-Individual)</span><AdaptiveGuidedValue2550Q value={envelope.taxpayer.name} cells={26} /></div>
    </div>
  );
}

function PartFour2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-four-2550q">
      <h2>Part IV – Details of VAT Computation</h2>
      <div className="part-four-header-2550q"><span>Total Sales and Output Tax</span><span>A. Sales for the Quarter (Exclusive of VAT)</span><span>B. Output Tax for the Quarter</span></div>
      {PART_FOUR_ROWS.map((row) => (
        <Fragment key={row.item}>
          {row.item === 38 && <PartFourSubheader2550Q first="Less: Allowable Input Tax" third="B. Input Tax" />}
          {row.item === 44 && <PartFourSubheader2550Q first="Current Transactions" second="A. Purchases" third="B. Input Tax" />}
          {row.item === 52 && <PartFourSubheader2550Q first="Less: Adjustment/Deductions from Input Tax" third="B. Input Tax" />}
          <div className={`part-four-row-2550q item-${row.item}-2550q ${row.emphasis ? `${row.emphasis}-2550q` : ""} ${row.disabledA && row.b ? "single-b-2550q" : ""}`}>
            <span><b>{row.item}</b>{row.label}{row.item === 42 && <InlineSpecify2550Q value={text(envelope, "item_42_description")} capacity={18} />}{row.item === 47 && <InlineSpecify2550Q value={text(envelope, "item_47_description")} capacity={18} />}{row.item === 56 && <InlineSpecify2550Q value={text(envelope, "item_56_description")} capacity={18} />}</span>
            <span className={row.disabledA ? "disabled-cell-2550q" : ""}>{row.a && <MoneyCell2550Q envelope={envelope} fieldKey={row.a} cells={12} />}</span>
            <span className={row.disabledB ? "disabled-cell-2550q" : ""}>{row.b && <MoneyCell2550Q envelope={envelope} fieldKey={row.b} cells={12} />}</span>
          </div>
        </Fragment>
      ))}
    </section>
  );
}

function PartFourSubheader2550Q({ first, second = "", third }: { first: string; second?: string; third: string }) {
  return <div className={`part-four-subheader-2550q ${second ? "current-transactions-2550q" : "wide-label-2550q"}`.trim()}><span>{first}</span><span>{second}</span><span>{third}</span></div>;
}

function PartFive2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-five-2550q">
      <h2>Part V – Schedules</h2>
      <ScheduleOne2550Q envelope={envelope} />
      <ScheduleTwo2550Q envelope={envelope} />
      <ScheduleThree2550Q envelope={envelope} />
      <ScheduleFour2550Q envelope={envelope} />
    </section>
  );
}

function ScheduleOne2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-2550q schedule-one-2550q">
      <h3>Schedule 1 – Amortized Input Tax from Capital Goods <small>(Attach additional sheet/s, if necessary)</small></h3>
      <div className="schedule-one-grid-2550q schedule-header-2550q"><span>Date Purchased/<br />Imported<br /><em>(MM/DD/YYYY)</em><small className="schedule-column-letter-2550q">(A)</small></span><span>Source<br />Code*<small className="schedule-column-letter-2550q">(B)</small></span><span>Description<small className="schedule-column-letter-2550q">(C)</small></span><span>Amount of Purchases/<br />Importation of Capital<br />Goods Exceeding P 1 M<small className="schedule-column-letter-2550q">(D)</small></span><span>Balance of Input Tax from Previous Period<small className="schedule-column-letter-2550q">(E)</small></span><span>Estimated Life<br />(in months)<small className="schedule-column-letter-2550q">(F)</small></span><span>Recognized Life<br />(in Months)<br /><small>Remaining Life</small><small className="schedule-column-letter-2550q">(G)</small></span><span>Allowable Input Tax<br />for the Period**<small className="schedule-column-letter-2550q">(H)</small></span><span>Balance of Input Tax to<br />be carried to Next<br />Period (E) Less (H)<small className="schedule-column-letter-2550q">(I)</small></span></div>
      {[1, 2].map((row) => <div className="schedule-one-grid-2550q" key={row}><PlainValue2550Q value={text(envelope, `schedule_1_${row}_date`)} capacity={10} /><PlainValue2550Q value={text(envelope, `schedule_1_${row}_source`)} capacity={1} /><PlainValue2550Q value={text(envelope, `schedule_1_${row}_description`)} capacity={22} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_1_${row}_amount`} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_1_${row}_input_tax`} /><PlainValue2550Q value={IntegerOrBlank2550Q(envelope, `schedule_1_${row}_estimated_life_months`)} capacity={3} /><PlainValue2550Q value={IntegerOrBlank2550Q(envelope, `schedule_1_${row}_recognized_life_months`)} capacity={3} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_1_${row}_allowable_input_tax`} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_1_${row}_balance`} /></div>)}
      <div className="schedule-one-total-2550q"><span>Total (Column E - To Part IV, Item 39B)/(Column I - To Part IV, Item 52B)</span><ScheduleMoney2550Q envelope={envelope} fieldKey="item_39b" /><ScheduleMoney2550Q envelope={envelope} fieldKey="item_52b" /></div>
      <div className="schedule-one-note-2550q"><span>* D for Domestic Purchase; I for Importation</span><span>**E divided by G multiplied by the Number of months in use during the quarter</span></div>
    </section>
  );
}

function ScheduleTwo2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-2550q schedule-two-2550q">
      <h3>Schedule 2 – Input Tax Attributable to VAT Exempt Sales</h3>
      <div className="schedule-two-body-2550q">
        <div className="schedule-two-formula-2550q">
          <span className="schedule-two-direct-label-2550q">Input Tax directly attributable to VAT Exempt Sale</span>
          <span className="schedule-two-ratable-label-2550q">Add:&nbsp; Ratable portion of Input Tax not directly attributable to any activity:</span>
          <span className="schedule-two-equation-2550q">
            <span className="schedule-two-fraction-2550q">
              <span><span>VAT Exempt Sale</span><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_exempt_sales" /></span>
              <span><span>Total Sales</span><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_total_sales" /></span>
            </span>
            <b aria-hidden="true">X</b>
            <span className="schedule-two-indirect-2550q"><span>Amount of Input Tax not directly attributable</span><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_indirect_input_tax" /></span>
          </span>
          <span className="schedule-two-total-label-2550q">Total Input Tax attributable to Exempt Sale <em>(To Part IV, Item 53)</em></span>
        </div>
        <div className="schedule-two-results-2550q"><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_direct_input_tax" /><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_ratable_input_tax" /><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_2_total_attributable_input_tax" /></div>
      </div>
    </section>
  );
}

function ScheduleThree2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-2550q schedule-three-2550q">
      <h3>Schedule 3 – Creditable VAT Withheld <small>(Attach additional sheet/s, if necessary)</small></h3>
      <div className="schedule-three-grid-2550q schedule-header-2550q"><span>(A) Period Covered</span><span>(B) Name of Withholding Agent</span><span>(C) Income Payment</span><span>(D) Total Tax Withheld</span></div>
      {[1, 2].map((row) => <div className="schedule-three-grid-2550q" key={row}><PlainValue2550Q value={dateRange2550Q(envelope, `schedule_3_${row}_from`, `schedule_3_${row}_to`)} capacity={21} /><PlainValue2550Q value={text(envelope, `schedule_3_${row}_agent`)} capacity={48} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_3_${row}_income_payment`} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_3_${row}_tax_withheld`} /></div>)}
      <div className="schedule-three-grid-2550q schedule-total-2550q"><span>Total (Column D - To Part II, Item 16)</span><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_3_total_income_payment" /><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_3_total_tax_withheld" /></div>
    </section>
  );
}

function ScheduleFour2550Q({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-2550q schedule-four-2550q">
      <h3>Schedule 4 – Advance VAT Payment <small>(Attach additional sheet/s, if necessary)</small></h3>
      <div className="schedule-four-grid-2550q schedule-header-2550q"><span>(A) Period Covered</span><span>(B) Name of Miller</span><span>(C) Name of Taxpayer</span><span>(D) Official Receipt Number</span><span>(E) Amount Paid</span></div>
      {[1, 2].map((row) => <div className="schedule-four-grid-2550q" key={row}><PlainValue2550Q value={dateRange2550Q(envelope, `schedule_4_${row}_from`, `schedule_4_${row}_to`)} capacity={21} /><PlainValue2550Q value={text(envelope, `schedule_4_${row}_miller`)} capacity={24} /><PlainValue2550Q value={text(envelope, `schedule_4_${row}_taxpayer`)} capacity={24} /><PlainValue2550Q value={text(envelope, `schedule_4_${row}_receipt`)} capacity={20} /><ScheduleMoney2550Q envelope={envelope} fieldKey={`schedule_4_${row}_amount`} /></div>)}
      <div className="schedule-four-grid-2550q schedule-total-2550q"><span>Total (To Part II, Item 17)</span><ScheduleMoney2550Q envelope={envelope} fieldKey="schedule_4_total_amount" /></div>
    </section>
  );
}

function FullWidthField2550Q({ item, label, value, cells, className = "" }: { item: string; label: ReactNode; value: string; cells: number; className?: string }) {
  return <div className={`full-field-2550q ${className}`.trim()}><span><b>{item}</b>{label}</span><AdaptiveGuidedValue2550Q value={value} cells={cells} /></div>;
}

function Address2550Q({ envelope }: { envelope: RenderEnvelope }) {
  const characters = Array.from(envelope.taxpayer.registered_address);
  return <div className="address-2550q"><span><b>10</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></span><div className="address-value-2550q">{characters.length <= 71 ? <div className="address-comb-lines-2550q"><CombValue value={characters.slice(0, 40).join("")} cells={40} /><CombValue value={characters.slice(40).join("")} cells={31} /></div> : <AdaptiveGuidedValue2550Q value={envelope.taxpayer.registered_address} cells={71} />}<span className="zip-2550q"><span className="zip-label-2550q"><b>10A</b> ZIP Code</span><CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" /></span></div></div>;
}

function Tin2550Q({ value }: { value: string }) {
  const digits = value.replace(/\D/g, "").slice(0, 14).padEnd(14, " ");
  return <span className="tin-comb-2550q"><CombValue value={digits.slice(0, 3)} cells={3} /><span className="tin-separator-2550q">-</span><CombValue value={digits.slice(3, 6)} cells={3} /><span className="tin-separator-2550q">-</span><CombValue value={digits.slice(6, 9)} cells={3} /><span className="tin-separator-2550q">-</span><CombValue value={digits.slice(9)} cells={5} /></span>;
}

function InlineSpecify2550Q({ value, capacity = 34 }: { value: string; capacity?: number }) {
  return <PlainValue2550Q value={value} capacity={capacity} className="inline-specify-2550q" />;
}

function ValueLine2550Q({ value }: { value: string }) {
  return <span className="value-line-2550q">{value}</span>;
}

function MoneyCell2550Q({ envelope, fieldKey, cells }: { envelope: RenderEnvelope; fieldKey: string; cells: number }) {
  const value = field(envelope, fieldKey);
  if (value?.type !== "decimal") return <span className="money-cell-2550q blank-money-2550q"><AdaptiveGuidedValue2550Q value="" cells={cells} align="right" /><span className="decimal-cell-2550q">.</span><CombValue value="" cells={2} align="right" /></span>;
  const [whole, decimal] = formatMoneyParts(value.value);
  return <span className="money-cell-2550q"><AdaptiveGuidedValue2550Q value={whole} cells={cells} align="right" /><span className="decimal-cell-2550q">.</span><CombValue value={decimal} cells={2} align="right" /></span>;
}

function ScheduleMoney2550Q({ envelope, fieldKey }: { envelope: RenderEnvelope; fieldKey: string }) {
  const value = field(envelope, fieldKey);
  return <span className="schedule-money-2550q">{value?.type === "decimal" ? value.value.toFixed(2) : ""}</span>;
}

function IntegerOrBlank2550Q(envelope: RenderEnvelope, key: string): string {
  const value = field(envelope, key);
  return value?.type === "integer" ? String(value.value) : "";
}

function dateRange2550Q(envelope: RenderEnvelope, fromKey: string, toKey: string): string {
  const from = text(envelope, fromKey);
  const to = text(envelope, toKey);
  return from && to ? `${from} – ${to}` : from || to;
}

function PlainValue2550Q({ value, capacity, className = "" }: { value: string; capacity: number; className?: string }) {
  return (
    <AdaptivePlainValue
      value={value}
      cellCapacity={capacity}
      className={`plain-value-2550q ${className}`.trim()}
      maxFontSizePx={PLAIN_FIELD_MAX_FONT_PX}
      minFontSizePx={PLAIN_FIELD_MIN_FONT_PX}
      fontStepPx={ADAPTIVE_FONT_STEP_PX}
    />
  );
}

function AdaptiveGuidedValue2550Q({
  value,
  cells,
  align = "left"
}: {
  value: string;
  cells: number;
  align?: "left" | "right";
}) {
  return (
    <AdaptiveCombValue
      value={value}
      cells={cells}
      align={align}
      fitToField
      maxFontSizePx={GUIDED_OVERFLOW_MAX_FONT_PX}
      minFontSizePx={GUIDED_OVERFLOW_MIN_FONT_PX}
      fontStepPx={ADAPTIVE_FONT_STEP_PX}
    />
  );
}
