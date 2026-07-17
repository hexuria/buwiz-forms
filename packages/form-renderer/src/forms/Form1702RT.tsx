import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import { Fragment, type ReactNode } from "react";
import {
  AdaptiveCombValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary
} from "../components";
import { bool, integer, text } from "../values";
import {
  OFFICIAL_1702RT_PDF417_PATHS,
  OFFICIAL_1702RT_PDF417_PAYLOADS,
  OFFICIAL_1702RT_SEAL
} from "./official1702RTAssets";
import "./Form1702RT.css";

type AmountRow1702RT = Readonly<{
  item: number;
  label: ReactNode;
  field: string;
  emphasis?: boolean;
  descriptionField?: string;
}>;

const PART_TWO_ROWS: readonly AmountRow1702RT[] = [
  { item: 14, label: <>Tax Due <em>(From Part IV Item 43)</em></>, field: "item_14" },
  { item: 15, label: <>Less: Total Tax Credits/Payments <em>(From Part IV Item 55)</em></>, field: "item_15" },
  { item: 16, label: <>Net Tax Payable / (Overpayment) <em>(Item 14 Less Item 15) (From Part IV Item 56)</em></>, field: "item_16" },
  { item: 17, label: "Surcharge", field: "item_17" },
  { item: 18, label: "Interest", field: "item_18" },
  { item: 19, label: "Compromise", field: "item_19" },
  { item: 20, label: <>Total Penalties <em>(Sum of Items 17 to 19)</em></>, field: "item_20" },
  { item: 21, label: <>TOTAL AMOUNT PAYABLE / (Overpayment) <em>(Sum of Items 16 and 20)</em></>, field: "item_21", emphasis: true }
];

const PART_FOUR_ROWS: readonly AmountRow1702RT[] = [
  { item: 27, label: "Sales/Receipts/Revenues/Fees", field: "item_27" },
  { item: 28, label: "Less: Sales Returns, Allowances and Discounts", field: "item_28" },
  { item: 29, label: <>Net Sales/Receipts/Revenues/Fees <em>(Item 27 Less Item 28)</em></>, field: "item_29" },
  { item: 30, label: "Less: Cost of Sales/Services", field: "item_30" },
  { item: 31, label: <>Gross Income from Operation <em>(Item 29 Less Item 30)</em></>, field: "item_31" },
  { item: 32, label: "Add: Other Taxable Income Not Subjected to Final Tax", field: "item_32" },
  { item: 33, label: <>Total Taxable Income <em>(Sum of Items 31 and 32)</em></>, field: "item_33" },
  { item: 34, label: <>Ordinary Allowable Itemized Deductions <em>(From Part VI Schedule I Item 18)</em></>, field: "item_34" },
  { item: 35, label: <>Special Allowable Itemized Deductions <em>(From Part VI Schedule II Item 5)</em></>, field: "item_35" },
  { item: 36, label: <>N O L C O <span className="nolco-source-note-1702rt"><em>[Only for those taxable under Sec. 27 (A to C); Sec. 28(A)(1)(A)(6)(b) of the Tax Code, as amended]</em><em>(From Part VI Schedule III Item 8)</em></span></>, field: "item_36" },
  { item: 37, label: <>Total Deductions <em>(Sum of Items 34 to 36)</em></>, field: "item_37" },
  { item: 38, label: <>Optional Standard Deduction (OSD) <em>(40% of Item 33)</em></>, field: "item_38" },
  { item: 39, label: <>Net Taxable Income/(Loss) <em>(If Itemized: Item 33 Less Item 37; If OSD: Item 33 Less Item 38)</em></>, field: "item_39" },
  { item: 41, label: <>Income Tax Due other than Minimum Corporate Income Tax (MCIT) <em>(Item 39 x Item 40)</em></>, field: "item_41" },
  { item: 42, label: <>MCIT Due <em>(2% of Item 33)</em></>, field: "item_42" },
  { item: 43, label: <>Tax Due <em>(Normal Income Tax Due in Item 41 OR the MCIT Due in Item 42, whichever is higher) (To Part II Item 14)</em></>, field: "item_43", emphasis: true },
  { item: 44, label: "Prior Year’s Excess Credits other than MCIT", field: "item_44" },
  { item: 45, label: "Income Tax Payment under MCIT from Previous Quarter/s", field: "item_45" },
  { item: 46, label: "Income Tax Payment under Regular/Normal Rate from Previous Quarter/s", field: "item_46" },
  { item: 47, label: "Excess MCIT Applied this Current Taxable Year (From Part VI Schedule IV Item 4)", field: "item_47" },
  { item: 48, label: "Creditable Tax Withheld from Previous Quarter/s per BIR Form No. 2307", field: "item_48" },
  { item: 49, label: "Creditable Tax Withheld per BIR Form No. 2307 for the 4th Quarter", field: "item_49" },
  { item: 50, label: "Foreign Tax Credits, if applicable", field: "item_50" },
  { item: 51, label: "Tax Paid in Return Previously Filed, if this is an Amended Return", field: "item_51" },
  { item: 52, label: "Special Tax Credits (To Part V Item 58)", field: "item_52" },
  { item: 53, label: "Other Tax Credits/Payments (specify)", field: "item_53", descriptionField: "item_53_description" },
  { item: 54, label: "", field: "item_54", descriptionField: "item_54_description" },
  { item: 55, label: <>Total Tax Credits/Payments <em>(Sum of Items 44 to 54) (To Part II Item 15)</em></>, field: "item_55" },
  { item: 56, label: <>Net Tax Payable / (Overpayment) <em>(Item 43 Less Item 55) (To Part II Item 16)</em></>, field: "item_56", emphasis: true }
];

const PART_FIVE_ROWS: readonly AmountRow1702RT[] = [
  { item: 57, label: "Special Allowable Itemized Deductions (Item 35 of Part IV x Applicable Income Tax Rate)", field: "item_57" },
  { item: 58, label: "Add: Special Tax Credits (From Part IV Item 52)", field: "item_58" },
  { item: 59, label: <>Total Tax Relief Availment <em>(Sum of Items 57 and 58)</em></>, field: "item_59", emphasis: true }
];

const SCHEDULE_ONE_FIXED = [
  "Amortizations", "Bad Debts", "Charitable and Other Contributions", "Depletion",
  "Depreciation", "Entertainment, Amusement and Recreation", "Fringe Benefits", "Interest",
  "Losses", "Pension Trusts", "Rental", "Research and Development",
  "Salaries, Wages and Allowances", "SSS, GSIS, Philhealth, HDMF and Other Contributions",
  "Taxes and Licenses", "Transportation and Travel"
] as const;

export function Form1702RT({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("1702RT", "2018C");
  if (envelope.schedules.length !== 0) {
    throw new Error("1702RTv2018C uses fixed official schedule capacities and no continuation renderer schedule");
  }

  return (
    <main className="form-document" data-form-code="1702RT">
      <FolioPage pageNumber={1} paper="folio" className="form-1702rt-page form-1702rt-page-one">
        <GovernmentHeader1702RT />
        <Masthead1702RT page={1} />
        <HeaderOptions1702RT envelope={envelope} />
        <PartOne1702RT envelope={envelope} />
        <PartTwo1702RT envelope={envelope} />
        <Declaration1702RT envelope={envelope} />
        <PartThree1702RT envelope={envelope} />
      </FolioPage>

      <FolioPage pageNumber={2} paper="folio" className="form-1702rt-page form-1702rt-page-two">
        <Masthead1702RT page={2} />
        <PageIdentity1702RT envelope={envelope} />
        <PartFour1702RT envelope={envelope} />
        <PartFive1702RT envelope={envelope} />
      </FolioPage>

      <FolioPage pageNumber={3} paper="folio" className="form-1702rt-page form-1702rt-page-three">
        <Masthead1702RT page={3} />
        <PageIdentity1702RT envelope={envelope} />
        <section className="schedules-1702rt schedules-page-three-1702rt">
          <PartSixHeading1702RT />
          <ScheduleOne1702RT envelope={envelope} />
          <ScheduleTwo1702RT envelope={envelope} />
        </section>
      </FolioPage>

      <FolioPage pageNumber={4} paper="folio" className="form-1702rt-page form-1702rt-page-four">
        <Masthead1702RT page={4} />
        <PageIdentity1702RT envelope={envelope} />
        <section className="schedules-1702rt schedules-page-four-1702rt">
          <ScheduleThree1702RT envelope={envelope} />
          <ScheduleFour1702RT envelope={envelope} />
          <ScheduleFive1702RT envelope={envelope} />
        </section>
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader1702RT() {
  return (
    <header className="government-header-1702rt">
      <span>For BIR<br />Use Only:</span>
      <span>BCS/<br />Item:</span>
      <span className="government-wordmark-1702rt">
        <img src={OFFICIAL_1702RT_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span>
      </span>
    </header>
  );
}

function Masthead1702RT({ page }: { page: 1 | 2 | 3 | 4 }) {
  const payload = OFFICIAL_1702RT_PDF417_PAYLOADS[page];
  return (
    <header className={`masthead-1702rt masthead-1702rt-page-${page}${page > 1 ? " compact-1702rt" : ""}`}>
      <div className="form-number-1702rt">
        <span>BIR Form No.</span>
        <strong>1702-RT</strong>
        <small>January 2018 (ENCS)</small>
        <b>Page {page}</b>
      </div>
      <div className="form-title-1702rt">
        <strong>Annual Income Tax Return</strong>
        <span>Corporation, Partnership and Other Non-Individual Taxpayer<br />Subject Only to REGULAR Income Tax Rate</span>
        {page === 1 && <em>Enter all required information in CAPITAL LETTERS. Mark applicable boxes with an “X”.<br />Two copies MUST be filed with the BIR and one held by the taxpayer.</em>}
      </div>
      <div className="barcode-1702rt" aria-label={payload} data-barcode-page={page}>
        <span className="official-pdf417-object-1702rt" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-1702rt"
            viewBox="0 0 120 8"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_1702RT_PDF417_PATHS[page]} />
          </svg>
        </span>
        <small>{payload}</small>
      </div>
    </header>
  );
}

function HeaderOptions1702RT({ envelope }: { envelope: RenderEnvelope }) {
  const basis = text(envelope, "filing_basis");
  const otherAtcSelected = bool(envelope, "atc_other_selected");
  const otherAtcCode = text(envelope, "atc_other_code").trim();
  const otherAtcDescription = text(envelope, "atc_other_description").trim();
  if (otherAtcSelected && (!otherAtcCode || !otherAtcDescription)) {
    throw new Error("1702RTv2018C alternate Item 5 ATC requires a reviewed code and description");
  }
  return (
    <section className="header-options-1702rt" aria-label="Return period and options">
      <div className="basis-1702rt">
        <div className="basis-item-one-1702rt">
          <span><b>1</b> For</span>
          <span><CheckChoice checked={basis === "calendar"} label="Calendar" /><CheckChoice checked={basis === "fiscal"} label="Fiscal" /></span>
        </div>
        <div className="basis-item-two-1702rt">
          <span><b>2</b> Year Ended <em>(MM/20YY)</em></span>
          <YearEnded1702RT month={envelope.period.month} taxableYear={envelope.period.taxable_year} />
        </div>
      </div>
      <BinaryHeaderChoice1702RT number={3} label="Amended Return?" checked={bool(envelope, "is_amended")} />
      <BinaryHeaderChoice1702RT number={4} label="Short Period Return?" checked={bool(envelope, "is_short_period")} />
      <div className="atc-options-1702rt">
        <b>5</b><span>Alphanumeric Tax Code (ATC)</span>
        <span className="atc-row-1702rt"><span>IC 055</span><span>Minimum Corporate Income Tax (MCIT)</span><span className={bool(envelope, "atc_mcit_selected") ? "check-box checked" : "check-box"}>{bool(envelope, "atc_mcit_selected") ? "X" : ""}</span></span>
        <span className="atc-row-1702rt"><span className="plain-value-1702rt atc-code-value-1702rt" data-official-field-mode="plain" aria-label={otherAtcCode}>{otherAtcCode}</span><span className="atc-description-1702rt" data-official-field-mode="plain" aria-label={otherAtcDescription}>{otherAtcDescription}</span><span className={otherAtcSelected ? "check-box checked" : "check-box"}>{otherAtcSelected ? "X" : ""}</span></span>
      </div>
    </section>
  );
}

function BinaryHeaderChoice1702RT({ number, label, checked }: { number: number; label: string; checked: boolean }) {
  return <div className="binary-header-1702rt"><span><b>{number}</b>{label}</span><span><CheckChoice checked={checked} label="Yes" /><CheckChoice checked={!checked} label="No" /></span></div>;
}

function PartOne1702RT({ envelope }: { envelope: RenderEnvelope }) {
  const deduction = text(envelope, "deduction_method");
  return (
    <section className="part-1702rt part-one-1702rt">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-1702rt"><span><b>6</b> Taxpayer Identification Number (TIN)</span><Tin1702RT value={envelope.taxpayer.tin} /><span><b>7</b> RDO Code</span><CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" /></div>
      <StackedIdentityField1702RT number="8" label="Registered Name (Enter only 1 letter per box using CAPITAL LETTERS)" values={[1, 2, 3].map((line) => text(envelope, `registered_name_line_${line}`))} cells={38} />
      <div className="address-block-1702rt" data-official-field-mode="guided" data-line-capacities="38,38,30">
        <span><b>9</b> Registered Address <em>(Indicate complete address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></span>
        {[1, 2].map((line) => <AdaptiveCombValue key={line} value={text(envelope, `registered_address_line_${line}`)} cells={38} />)}
        <div className="address-last-1702rt"><AdaptiveCombValue value={text(envelope, "registered_address_line_3")} cells={30} /><span><b>9A</b> ZIP Code</span><CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" /></div>
      </div>
      <div className="split-identity-1702rt">
        <div className="labeled-comb-1702rt"><span><b>10</b>Date of Incorporation/Organization</span><OfficialDate1702RT value={text(envelope, "incorporation_date")} /></div>
        <LabeledComb1702RT number="11" label="Contact Number" value={envelope.taxpayer.contact_number} cells={12} />
      </div>
      <LabeledComb1702RT number="12" label="Email Address" value={envelope.taxpayer.email} cells={32} />
      <div className="deduction-choice-1702rt"><b>13</b><span>Method of Deductions</span><CheckChoice checked={deduction === "itemized"} label="Itemized Deductions [Section 34 (A-J), NIRC]" /><CheckChoice checked={deduction === "osd"} label="Optional Standard Deduction (OSD) – 40% of Gross Income [Section 34(L) NIRC, as amended]" /></div>
    </section>
  );
}

function PartTwo1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-1702rt part-two-1702rt">
      <h2>Part II – Total Tax Payable <small>(Do NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small></h2>
      {PART_TWO_ROWS.map((row) => <Fragment key={row.item}>
        {row.item === 17 && <div className="subband-1702rt">Add: Penalties</div>}
        <AmountRow1702RT envelope={envelope} row={row} />
      </Fragment>)}
      <div className="overpayment-title-1702rt">If overpayment, mark one (1) box only. (Once the choice is made, the same is irrevocable)</div>
      <div className="overpayment-options-1702rt"><CheckChoice checked={text(envelope, "overpayment_disposition") === "refund"} label="To be refunded" /><CheckChoice checked={text(envelope, "overpayment_disposition") === "tcc"} label="To be issued a Tax Credit Certificate (TCC)" /><CheckChoice checked={text(envelope, "overpayment_disposition") === "carry_over"} label="To be carried over as tax credit for next year/quarter" /></div>
    </section>
  );
}

function Declaration1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-1702rt">
      <p>We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by us, and to the best of our knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. (If signed by an Authorized Representative, indicate TIN and attach authorization letter)</p>
      <div className="signature-body-1702rt">
        <SignaturePanel1702RT signature={text(envelope, "president_signature")} title={text(envelope, "president_signatory_title")} tin={text(envelope, "president_signatory_tin")} label="Signature over Printed Name of President/Principal Officer/ Authorized Representative" />
        <SignaturePanel1702RT signature={text(envelope, "treasurer_signature")} title={text(envelope, "treasurer_signatory_title")} tin={text(envelope, "treasurer_signatory_tin")} label="Signature over Printed Name of Treasurer/ Assistant Treasurer" />
        <div className="attachments-1702rt"><b>22</b><span>Number of<br />Attachments</span><AdaptiveCombValue value={text(envelope, "number_of_attachments")} cells={3} align="right" /></div>
      </div>
    </section>
  );
}

function SignaturePanel1702RT({ signature, title, tin, label }: { signature: string; title: string; tin: string; label: string }) {
  return <div className="signature-panel-1702rt"><span className="signature-space-1702rt">{signature}</span><span>{label}</span><span className="signature-details-1702rt"><span>Title of Signatory<br /><b data-signatory-binding="title">{title}</b></span><span>TIN<br /><b>{tin}</b></span></span></div>;
}

function PartThree1702RT({ envelope }: { envelope: RenderEnvelope }) {
  const rows = [[23, "Cash/Bank Debit Memo"], [24, "Check"]] as const;
  return (
    <section className="part-three-1702rt">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-grid-1702rt payment-header-1702rt"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date (MM/DD/YYYY)</span><span>Amount</span></div>
      {rows.map(([item, label]) => <div key={item} className="payment-grid-1702rt payment-row-1702rt" data-payment-item={item} data-payment-fields="bank,number,date,amount"><span><b>{item}</b>{label}</span><PaymentTextCell1702RT value={text(envelope, `payment_${item}_bank`)} cells={5} /><PaymentTextCell1702RT value={text(envelope, `payment_${item}_number`)} cells={7} /><PaymentDate1702RT value={text(envelope, `payment_${item}_date`)} /><Money1702RT envelope={envelope} fieldKey={`payment_${item}_amount`} /></div>)}
      <div className="payment-grid-1702rt payment-row-1702rt payment-row-tax-debit-1702rt" data-payment-item="25" data-payment-fields="number,date,amount"><span><b>25</b>Tax Debit Memo</span><PaymentTextCell1702RT value={text(envelope, "payment_25_number")} cells={7} /><PaymentDate1702RT value={text(envelope, "payment_25_date")} /><Money1702RT envelope={envelope} fieldKey="payment_25_amount" /></div>
      <div className="payment-other-label-1702rt" data-payment-item-label="26"><b>26</b>Others <em>(Specify below)</em></div>
      <div className="payment-grid-1702rt payment-other-1702rt" data-payment-item="26" data-payment-fields="specification,bank,number,date,amount"><PaymentPlainTextCell1702RT value={text(envelope, "payment_26_specification")} /><PaymentPlainTextCell1702RT value={text(envelope, "payment_26_bank")} /><PaymentTextCell1702RT value={text(envelope, "payment_26_number")} cells={7} /><PaymentDate1702RT value={text(envelope, "payment_26_date")} /><Money1702RT envelope={envelope} fieldKey="payment_26_amount" /></div>
      <div className="payment-validation-1702rt"><span>Machine Validation/Revenue Official Receipt Details [if not filed with an Authorized Agent Bank (AAB)]</span><span>Stamp of Receiving Office/AAB and Date of Receipt <em>(RO’s Signature/Bank Teller’s Initial)</em></span></div>
    </section>
  );
}

function PageIdentity1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="page-identity-1702rt"><div><span>Taxpayer Identification Number (TIN)</span><Tin1702RT value={envelope.taxpayer.tin} /></div><div data-official-field-mode="guided" data-cell-capacity="24"><span>Registered Name</span><AdaptiveCombValue value={envelope.taxpayer.name} cells={24} /></div></section>;
}

function PartFour1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-1702rt part-four-1702rt">
      <h2>Part IV – Computation of Tax <small>(Do NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small></h2>
      {PART_FOUR_ROWS.map((row) => <Fragment key={row.item}>
        {row.item === 34 && <div className="subband-1702rt">Less: Deductions Allowable under Existing Law</div>}
        {row.item === 38 && <div className="subband-1702rt centered-subband-1702rt">OR [in case taxable under Sec 27(A) &amp; 28(A)(1)]</div>}
        {row.item === 44 && <div className="subband-1702rt">Less: Tax Credits/Payments (attach proof)</div>}
        {row.item === 53 && <div className="subband-1702rt">Other Tax Credits/Payments (specify)</div>}
        <AmountRow1702RT envelope={envelope} row={row} />
        {row.item === 39 && <div className="rate-row-1702rt"><span><b>40</b> Applicable Income Tax Rate</span><span>{integer(envelope, "item_40_rate_percent") || ""}</span><b>%</b></div>}
      </Fragment>)}
    </section>
  );
}

function PartFive1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="part-1702rt part-five-1702rt"><h2>Part V – Tax Relief Availment</h2>{PART_FIVE_ROWS.map((row) => <AmountRow1702RT key={row.item} envelope={envelope} row={row} />)}</section>;
}

function PartSixHeading1702RT() {
  return <h2 className="part-six-heading-1702rt">Part VI – Schedules <small>(Do NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small></h2>;
}

function ScheduleOne1702RT({ envelope }: { envelope: RenderEnvelope }) {
  const rows = SCHEDULE_ONE_FIXED.map((label, index) => ({ item: index + 1, label, field: `schedule_1_item_${index + 1}` }));
  return <section className="schedule-1702rt schedule-one-1702rt" data-other-description-capacity="23"><h3>Schedule I – Ordinary Allowable Itemized Deductions <small>(attach additional sheet/s, if necessary)</small></h3>
    {rows.map((row) => <ScheduleAmountRow1702RT key={row.item} envelope={envelope} item={String(row.item)} label={row.label} field={row.field} />)}
    <div className="schedule-subband-1702rt">17 Others (Deductions Subject to Withholding Tax and Other Expenses) [Specify below; Add additional sheet(s), if necessary]</div>
    {["a", "b", "c"].map((suffix, index) => <ScheduleAmountRow1702RT key={suffix} envelope={envelope} item={suffix} label={["Janitorial and Messengerial Services", "Professional Fees", "Security Services"][index]} field={`schedule_1_item_17${suffix}`} />)}
    {["d", "e", "f", "g", "h", "i"].map((suffix) => <ScheduleAmountRow1702RT key={suffix} envelope={envelope} item={suffix} label={<AdaptiveCombValue value={text(envelope, `schedule_1_item_17${suffix}_description`)} cells={23} />} field={`schedule_1_item_17${suffix}_amount`} />)}
    <ScheduleAmountRow1702RT envelope={envelope} item="18" label={<>Total Ordinary Allowable Itemized Deductions <em>(Sum of Items 1 to 17i) (To Part IV Item 34)</em></>} field="schedule_1_item_18" emphasis />
  </section>;
}

function ScheduleTwo1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-1702rt schedule-two-1702rt" data-description-capacity="15" data-legal-basis-capacity="10"><h3>Schedule II – Special Allowable Itemized Deductions <small>(attach additional sheet/s, if necessary)</small></h3>
    <div className="schedule-two-grid-1702rt schedule-table-header-1702rt"><span /><span>Description</span><span>Legal Basis</span><span>Amount</span></div>
    {[1, 2, 3, 4].map((item) => <div key={item} className="schedule-two-grid-1702rt"><span>{item}</span><AdaptiveCombValue value={text(envelope, `schedule_2_item_${item}_description`)} cells={15} /><AdaptiveCombValue value={text(envelope, `schedule_2_item_${item}_legal_basis`)} cells={10} /><Money1702RT envelope={envelope} fieldKey={`schedule_2_item_${item}_amount`} /></div>)}
    <div className="schedule-two-total-1702rt"><span>5 Total Special Allowable Itemized Deductions (Sum of Items 1 to 4) (To Part IV Item 35)</span><Money1702RT envelope={envelope} fieldKey="schedule_2_item_5" /></div>
  </section>;
}

function ScheduleThree1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-1702rt schedule-three-1702rt"><h3>Schedule III – Computation of Net Operating Loss Carry Over (NOLCO)</h3>
    <ScheduleAmountRow1702RT envelope={envelope} item="1" label="Gross Income (From Part IV Item 33)" field="schedule_3_item_1" />
    <ScheduleAmountRow1702RT envelope={envelope} item="2" label="Less: Ordinary Allowable Itemized Deductions (From Part VI Schedule I Item 18)" field="schedule_3_item_2" />
    <ScheduleAmountRow1702RT envelope={envelope} item="3" label="Net Operating Loss (Item 1 Less Item 2) (To Schedule IIIA, Item 7A)" field="schedule_3_item_3" />
    <h3>Schedule IIIA – Computation of Available Net Operating Loss Carry Over (NOLCO)</h3>
    <div className="nolco-primary-heading-1702rt">
      <span className="nolco-primary-title-1702rt">Net Operating Loss</span>
      <span className="nolco-primary-year-heading-1702rt">Year Incurred</span>
      <span className="nolco-primary-amount-heading-1702rt">A) Amount</span>
      <span className="nolco-primary-previous-heading-1702rt">B) NOLCO Applied Previous Year/s</span>
    </div>
    {[4, 5, 6, 7].map((item) => <div key={item} className="nolco-primary-row-1702rt" data-nolco-item={item}>
      <span>{item}</span>
      <span className="nolco-dead-cell-1702rt" aria-hidden="true" />
      <AdaptiveCombValue value={text(envelope, `schedule_3_item_${item}_year`)} cells={4} />
      <span className="nolco-dead-cell-1702rt" aria-hidden="true" />
      <Money1702RT envelope={envelope} fieldKey={`schedule_3_item_${item}_amount`} />
      <span className="nolco-dead-cell-1702rt" aria-hidden="true" />
      <Money1702RT envelope={envelope} fieldKey={`schedule_3_item_${item}_applied_previous`} />
    </div>)}
    <div className="continuation-title-1702rt">Continuation of Schedule IIIA (Item numbers continue from table above)</div>
    <div className="nolco-continuation-grid-1702rt nolco-heading-1702rt"><span /><span>C) NOLCO Expired</span><span>D) NOLCO Applied Current Year</span><span>E) Net Operating Loss (Unapplied) [ E = A Less (B + C + D) ]</span></div>
    {[4, 5, 6, 7].map((item) => <div key={item} className="nolco-continuation-grid-1702rt"><span>{item}</span><Money1702RT envelope={envelope} fieldKey={`schedule_3_item_${item}_expired`} /><Money1702RT envelope={envelope} fieldKey={`schedule_3_item_${item}_applied_current`} /><Money1702RT envelope={envelope} fieldKey={`schedule_3_item_${item}_unapplied`} /></div>)}
    <div className="schedule-three-total-1702rt"><span>8 Total NOLCO <em>(Sum of Items 4D to 7D) (To Part IV, Item 36)</em></span><Money1702RT envelope={envelope} fieldKey="schedule_3_item_8" /><span /></div>
  </section>;
}

function ScheduleFour1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-1702rt schedule-four-1702rt"><h3>Schedule IV – Computation of Minimum Corporate Income Tax (MCIT)</h3>
    <div className="mcit-grid-1702rt mcit-heading-1702rt"><span /><span>Year</span><span>A) Normal Income Tax as Adjusted</span><span>B) MCIT</span><span>C) Excess MCIT over Normal Income Tax</span></div>
    {[1, 2, 3].map((item) => <div key={item} className="mcit-grid-1702rt"><span>{item}</span><AdaptiveCombValue value={text(envelope, `schedule_4_item_${item}_year`)} cells={4} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_normal_tax`} cells={11} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_mcit`} cells={11} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_excess`} cells={11} /></div>)}
    <div className="continuation-title-1702rt">Continuation of Schedule IV (Item numbers continue from table above)</div>
    <div className="mcit-continuation-grid-1702rt mcit-heading-1702rt"><span /><span>D) Excess MCIT Applied/Used in Previous Years</span><span>E) Expired Portion of Excess MCIT</span><span>F) Excess MCIT Applied this Current Taxable Year</span><span>G) Balance of Excess MCIT Allowable as Tax Credit for Succeeding Year/s</span></div>
    {[1, 2, 3].map((item) => <div key={item} className="mcit-continuation-grid-1702rt"><span>{item}</span><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_applied_previous`} cells={9} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_expired`} cells={9} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_applied_current`} cells={10} /><Money1702RT envelope={envelope} fieldKey={`schedule_4_item_${item}_balance`} cells={9} /></div>)}
    <div className="schedule-four-total-1702rt"><span>4 Total Excess MCIT Applied (Sum of Items 1F to 3F) (To Part IV Item 47)</span><Money1702RT envelope={envelope} fieldKey="schedule_4_item_4" cells={10} /><span /></div>
  </section>;
}

function ScheduleFive1702RT({ envelope }: { envelope: RenderEnvelope }) {
  return <section className="schedule-1702rt schedule-five-1702rt" data-description-capacity="24"><h3>Schedule V – Reconciliation of Net Income per Books Against Taxable Income <small>(attach additional sheet/s, if necessary)</small></h3>
    <ScheduleAmountRow1702RT envelope={envelope} item="1" label="Net Income/(Loss) per Books" field="schedule_5_item_1" />
    <div className="schedule-subband-1702rt">Add: Non-Deductible Expenses/Taxable Other Income</div>
    {[2, 3].map((item) => <ScheduleAmountRow1702RT key={item} envelope={envelope} item={String(item)} label={<AdaptiveCombValue value={text(envelope, `schedule_5_item_${item}_description`)} cells={24} />} field={`schedule_5_item_${item}_amount`} />)}
    <ScheduleAmountRow1702RT envelope={envelope} item="4" label="Total (Sum of Items 1 to 3)" field="schedule_5_item_4" />
    <div className="schedule-subband-1702rt">Less: A) Non-Taxable Income and Income Subjected to Final Tax</div>
    {[5, 6].map((item) => <ScheduleAmountRow1702RT key={item} envelope={envelope} item={String(item)} label={<AdaptiveCombValue value={text(envelope, `schedule_5_item_${item}_description`)} cells={24} />} field={`schedule_5_item_${item}_amount`} />)}
    <div className="schedule-subband-1702rt">B) Special Deductions</div>
    {[7, 8].map((item) => <ScheduleAmountRow1702RT key={item} envelope={envelope} item={String(item)} label={<AdaptiveCombValue value={text(envelope, `schedule_5_item_${item}_description`)} cells={24} />} field={`schedule_5_item_${item}_amount`} />)}
    <ScheduleAmountRow1702RT envelope={envelope} item="9" label="Total (Sum of Items 5 to 8)" field="schedule_5_item_9" />
    <ScheduleAmountRow1702RT envelope={envelope} item="10" label="Net Taxable Income/(Loss) (Item 4 Less Item 9)" field="schedule_5_item_10" emphasis />
  </section>;
}

function AmountRow1702RT({ envelope, row }: { envelope: RenderEnvelope; row: AmountRow1702RT }) {
  if (row.descriptionField) {
    return <div className="amount-row-1702rt amount-description-row-1702rt" data-description-capacity="23"><span><b>{row.item}</b></span><AdaptiveCombValue value={text(envelope, row.descriptionField)} cells={23} /><span className="amount-description-dead-cell-1702rt" aria-hidden="true" /><Money1702RT envelope={envelope} fieldKey={row.field} /></div>;
  }
  return <div className={`amount-row-1702rt${row.emphasis ? " emphasis-1702rt" : ""}`}><span><b>{row.item}</b>{row.descriptionField ? <AdaptiveCombValue value={text(envelope, row.descriptionField)} cells={28} /> : row.label}</span><Money1702RT envelope={envelope} fieldKey={row.field} /></div>;
}

function ScheduleAmountRow1702RT({ envelope, item, label, field, emphasis = false }: { envelope: RenderEnvelope; item: string; label: ReactNode; field: string; emphasis?: boolean }) {
  return <div className={`schedule-amount-row-1702rt${emphasis ? " emphasis-1702rt" : ""}`}><span>{item}</span><span>{label}</span><Money1702RT envelope={envelope} fieldKey={field} /></div>;
}

function Money1702RT({ envelope, fieldKey, cells = 12 }: { envelope: RenderEnvelope; fieldKey: string; cells?: number }) {
  const value = integer(envelope, fieldKey);
  return <span className="money-1702rt" data-official-field-mode="guided" data-cell-capacity={cells} aria-label={String(value)}><AdaptiveCombValue value={value === 0 ? "" : String(value)} cells={cells} align="right" /></span>;
}

function YearEnded1702RT({ month, taxableYear }: { month: number | null | undefined; taxableYear: number }) {
  if (month === null || month === undefined || month < 1 || month > 12) {
    throw new Error("1702RTv2018C Item 2 requires a year-end month from 1 to 12");
  }
  if (taxableYear < 2000 || taxableYear > 2099) {
    throw new Error("1702RTv2018C Item 2 MM/20YY cannot represent a taxable year outside 2000 through 2099");
  }
  const monthText = String(month).padStart(2, "0");
  const yearText = String(taxableYear).slice(-2);
  return <span className="year-ended-1702rt" data-official-date-format="MM/20YY" aria-label={`${monthText}/${taxableYear}`}><CombValue value={monthText} cells={2} /><span className="year-literal-1702rt">/20</span><CombValue value={yearText} cells={2} /></span>;
}

function OfficialDate1702RT({ value }: { value: string }) {
  const { normalized, match } = officialDateParts1702RT(value);
  return <span className="official-date-1702rt" data-official-date-format="MM/DD/YYYY" aria-label={normalized}><CombValue value={match?.[1] ?? ""} cells={2} /><span className="date-separator-1702rt">/</span><CombValue value={match?.[2] ?? ""} cells={2} /><span className="date-separator-1702rt">/</span><CombValue value={match?.[3] ?? ""} cells={4} /></span>;
}

function PaymentDate1702RT({ value }: { value: string }) {
  const { normalized, match } = officialDateParts1702RT(value);
  return <span className="payment-date-1702rt" data-official-date-format="MM/DD/YYYY" data-official-field-mode="guided" data-segment-capacities="2,2,4" aria-label={normalized}><CombValue value={match?.[1] ?? ""} cells={2} /><CombValue value={match?.[2] ?? ""} cells={2} /><CombValue value={match?.[3] ?? ""} cells={4} /></span>;
}

function officialDateParts1702RT(value: string) {
  const normalized = value.trim();
  const match = normalized.match(/^(\d{2})\/(\d{2})\/(\d{4})$/);
  if (normalized && !match) {
    throw new Error(`1702RTv2018C date value ${normalized} must use MM/DD/YYYY`);
  }
  return { normalized, match };
}

function PaymentTextCell1702RT({ value, cells }: { value: string; cells: number }) {
  return <span className="payment-value-1702rt" data-official-field-mode="guided" data-cell-capacity={cells} aria-label={value}><AdaptiveCombValue value={value} cells={cells} /></span>;
}

function PaymentPlainTextCell1702RT({ value }: { value: string }) {
  return <span className="payment-value-1702rt" data-official-field-mode="plain" aria-label={value}><span className="plain-value-1702rt">{value}</span></span>;
}

function Tin1702RT({ value }: { value: string }) {
  const digits = value.replace(/\D/g, "").slice(0, 14).padEnd(14, " ");
  return <span className="tin-1702rt"><CombValue value={digits.slice(0, 3)} cells={3} /><b>–</b><CombValue value={digits.slice(3, 6)} cells={3} /><b>–</b><CombValue value={digits.slice(6, 9)} cells={3} /><b>–</b><CombValue value={digits.slice(9, 14)} cells={5} /></span>;
}

function StackedIdentityField1702RT({ number, label, values, cells }: { number: string; label: ReactNode; values: string[]; cells: number }) {
  return <div className="stacked-field-1702rt" data-official-field-mode="guided" data-line-capacity={cells}><span><b>{number}</b>{label}</span>{values.map((value, index) => <AdaptiveCombValue key={index} value={value} cells={cells} />)}</div>;
}

function LabeledComb1702RT({ number, label, value, cells }: { number: string; label: ReactNode; value: string; cells: number }) {
  return <div className="labeled-comb-1702rt" data-official-field-mode="guided" data-cell-capacity={cells}><span><b>{number}</b>{label}</span><AdaptiveCombValue value={value} cells={cells} /></div>;
}
