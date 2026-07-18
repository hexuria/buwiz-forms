import type { RenderEnvelope, RenderRow } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import type { ReactNode } from "react";
import {
  AdaptiveCombValue,
  AdaptivePlainValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary,
  formatMoneyParts
} from "../components";
import { bool, cellDecimal, cellText, decimal, integer, text } from "../values";
import {
  OFFICIAL_1601C_PDF417_PATHS,
  OFFICIAL_1601C_PDF417_PAYLOADS,
  OFFICIAL_1601C_SEAL
} from "./official1601CAssets";
import "./Form1601C.css";

const TAX_LINES = [
  ["14", "Total Amount of Compensation", "tax_14_total_compensation"],
  ["15", <>Statutory Minimum Wage for Minimum Wage Earners <em>(MWEs)</em></>, "tax_15_statutory_minimum_wage"],
  ["16", <>Holiday Pay, Overtime Pay, Night Shift Differential Pay, Hazard Pay <em>(for MWEs only)</em></>, "tax_16_holiday_pay"],
  ["17", <>13<sup>th</sup> Month Pay and Other Benefits</>, "tax_17_13th_month_pay"],
  ["18", "De Minimis Benefits", "tax_18_de_minimis"],
  ["19", <>SSS, GSIS, PHIC, HDMF Mandatory Contributions &amp; Union Dues <em>(employee’s share only)</em></>, "tax_19_sss_gsis"],
  ["20", <>Other Non-Taxable Compensation <em>(specify)</em></>, "tax_20_other_amount"],
  ["21", <>Total Non-Taxable Compensation <em>(Sum of Items 15 to 20)</em></>, "tax_21_total_non_taxable"],
  ["22", <>Total Taxable Compensation <em>(Item 14 Less Item 21)</em></>, "tax_22_total_taxable"],
  ["23", <>Less: Taxable compensation not subject to withholding tax <em>(for employees, other than MWEs, receiving P250,000 &amp; below for the year)</em></>, "tax_23_not_subject"],
  ["24", <>Net Taxable Compensation <em>(Item 22 Less Item 23)</em></>, "tax_24_net_taxable"],
  ["25", "Total Taxes Withheld", "tax_25_total_taxes_withheld"],
  ["26", <>Add/(Less): Adjustment of Taxes Withheld from Previous Month/s <em>(From Part IV-Schedule 1, Item 4)</em></>, "tax_26_adjustment"],
  ["27", <>Taxes Withheld for Remittance <em>(Sum of Items 25 and 26)</em></>, "tax_27_taxes_withheld_for_remittance"],
  ["28", "Less: Tax Remitted in Return Previously Filed, if this is an amended return", "tax_28_tax_remitted_previously"],
  ["29", <>Other Remittances Made <em>(specify)</em></>, "tax_29_other_remittances_amount"],
  ["30", <>Total Tax Remittances Made <em>(Sum of Items 28 and 29)</em></>, "tax_30_total_tax_remittances"],
  ["31", <><strong>Tax Still Due</strong>/(Over-remittance) <em>(Item 27 Less Item 30)</em></>, "tax_31_tax_still_due"],
  ["32", "Surcharge", "tax_32_surcharge"],
  ["33", "Interest", "tax_33_interest"],
  ["34", "Compromise", "tax_34_compromise"],
  ["35", <>Total Penalties <em>(Sum of Items 32 to 34)</em></>, "tax_35_total_penalties"],
  ["36", <><strong>TOTAL AMOUNT STILL DUE</strong>/(Over-remittance) <em>(Sum of Items 31 and 35)</em></>, "tax_36_total_amount_payable"]
] as const;

export function Form1601C({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("1601C", "2018");
  const schedule = envelope.schedules.find((item) => item.id === "schedule_1");
  if (!schedule) throw new Error("1601C requires schedule_1");
  if (schedule.rows.length > 3) {
    throw new Error("1601C Schedule I exceeds the verified three-row XML capacity");
  }

  return (
    <main className="form-document" data-form-code="1601C">
      <FolioPage pageNumber={1} className="form-1601c-page-one">
        <PageOne envelope={envelope} />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}

      <FolioPage pageNumber={2} className="form-1601c-page-two">
        <PageTwo envelope={envelope} rows={schedule.rows} />
      </FolioPage>
    </main>
  );
}

function PageOne({ envelope }: { envelope: RenderEnvelope }) {
  const monthYear = `${String(envelope.period.month).padStart(2, "0")}${String(envelope.period.taxable_year).padStart(4, "0")}`;
  const category = text(envelope, "category_of_agent");

  return (
    <>
      <GovernmentHeader1601C />
      <Masthead1601C page={1} />
      <HeaderOptions envelope={envelope} monthYear={monthYear} />
      <BackgroundInformation envelope={envelope} category={category} />
      <ComputationOfTax envelope={envelope} />
      <Declaration1601C />
      <PaymentDetails1601C />
      <p className="privacy-note-1601c"><b>* NOTE:</b> Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)</p>
    </>
  );
}

function GovernmentHeader1601C() {
  return (
    <header className="government-header-1601c">
      <span className="bir-only-1601c">For BIR<br />Use Only</span>
      <span className="bcs-1601c">BCS/<br />Item:</span>
      <span className="government-wordmark-1601c">
        <img src={OFFICIAL_1601C_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span>
      </span>
    </header>
  );
}

function Masthead1601C({ page }: { page: 1 | 2 }) {
  const payload = OFFICIAL_1601C_PDF417_PAYLOADS[page];
  return (
    <header className={`masthead-1601c masthead-1601c-page-${page}`}>
      <div className="form-number-1601c">
        <span>BIR Form No.</span>
        <strong>1601-C</strong>
        <small>January 2018 (ENCS)</small>
        <b>Page {page}</b>
      </div>
      <div className="form-title-1601c">
        <strong>Monthly Remittance Return</strong>
        <span>of Income Taxes Withheld on Compensation</span>
        {page === 1 && (
          <em>Enter all required information in CAPITAL LETTERS using BLACK ink. Mark all applicable boxes with<br />an “X”. Two copies MUST be filed with the BIR and one held by the Taxpayer.</em>
        )}
      </div>
      <div className="barcode-1601c" aria-label={payload} data-barcode-page={page}>
        <span className="official-pdf417-object-1601c" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-1601c"
            viewBox="0 0 120 7"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_1601C_PDF417_PATHS[page]} />
          </svg>
        </span>
        <small>{payload}</small>
      </div>
    </header>
  );
}

function HeaderOptions({ envelope, monthYear }: { envelope: RenderEnvelope; monthYear: string }) {
  return (
    <section className="header-options-1601c" aria-label="Return period and filing options">
      <HeaderOption label={<><b>1</b> For the Month <em>(MM/YYYY)</em></>}>
        <CombValue value={monthYear} cells={6} align="right" />
      </HeaderOption>
      <HeaderOption label={<><b>2</b> Amended Return?</>}>
        <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "is_amended")} label="No" />
      </HeaderOption>
      <HeaderOption label={<><b>3</b> Any Taxes Withheld?</>}>
        <CheckChoice checked={bool(envelope, "any_taxes_withheld")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "any_taxes_withheld")} label="No" />
      </HeaderOption>
      <HeaderOption label={<><b>4</b> Number of Sheet/s Attached</>}>
        <AdaptiveComb1601C value={String(integer(envelope, "number_of_sheets"))} cells={2} align="right" />
      </HeaderOption>
      <HeaderOption label={<><b>5</b> ATC</>}>
        <AdaptivePlainValue value={text(envelope, "atc")} className="atc-plain-1601c" />
      </HeaderOption>
    </section>
  );
}

function HeaderOption({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="header-option-1601c">
      <div>{label}</div>
      <span>{children}</span>
    </div>
  );
}

function BackgroundInformation({
  envelope,
  category
}: {
  envelope: RenderEnvelope;
  category: string;
}) {
  const tin = requireCellCapacity(envelope.taxpayer.tin.replace(/\D/g, ""), 14, "taxpayer.tin").padEnd(14);
  const addressLineTwo = text(envelope, "registered_address_2").toUpperCase();
  const relief = bool(envelope, "tax_relief");

  return (
    <section className="part-1601c background-1601c">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-1601c">
        <div><b>6</b> Taxpayer Identification Number (TIN)</div>
        <Tin1601C value={tin} />
        <div><b>7</b> RDO Code</div>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <LabelValue1601C
        number="8"
        label={<>Withholding Agent’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></>}
        value={envelope.taxpayer.name.toUpperCase()}
        cells={40}
        className="name-1601c"
      />
      <div className="address-1601c">
        <div className="label-1601c"><b>9</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></div>
        <AdaptiveComb1601C value={envelope.taxpayer.registered_address.toUpperCase()} cells={40} />
        <div className="address-second-1601c">
          <AdaptiveComb1601C value={addressLineTwo} cells={31} />
          <span><b>9A</b> ZIP Code</span>
          <CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" />
        </div>
      </div>
      <div className="contact-category-1601c">
        <div><b>10</b> Contact Number</div>
        <AdaptiveComb1601C value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} />
        <div><b>11</b> Category of Withholding Agent</div>
        <span className="category-choices-1601c">
          <CheckChoice checked={category === "P"} label="Private" />
          <CheckChoice checked={category === "G"} label="Government" />
        </span>
      </div>
      <LabelValue1601C number="12" label="Email Address" value={envelope.taxpayer.email.toUpperCase()} cells={34} className="email-1601c" />
      <div className="tax-relief-1601c">
        <div><b>13</b><span>Are there payees availing of tax relief under<br />Special Law or International Tax Treaty?</span></div>
        <span className="relief-choices-1601c">
          <CheckChoice checked={relief} label="Yes" />
          <CheckChoice checked={!relief} label="No" />
        </span>
        <div><b>13A</b> If yes, specify</div>
        <AdaptiveComb1601C value={text(envelope, "tax_relief_specification").toUpperCase()} cells={24} />
      </div>
    </section>
  );
}

function Tin1601C({ value }: { value: string }) {
  return (
    <span className="tin-value-1601c">
      <CombValue value={value.slice(0, 3)} cells={3} />
      <i>/</i>
      <CombValue value={value.slice(3, 6)} cells={3} />
      <i>/</i>
      <CombValue value={value.slice(6, 9)} cells={3} />
      <i>/</i>
      <CombValue value={value.slice(9, 14)} cells={5} />
    </span>
  );
}

function LabelValue1601C({
  number,
  label,
  value,
  cells,
  className
}: {
  number: string;
  label: ReactNode;
  value: string;
  cells: number;
  className: string;
}) {
  return (
    <div className={`label-value-1601c ${className}`}>
      <div className="label-1601c"><b>{number}</b> {label}</div>
      <AdaptiveComb1601C value={value} cells={cells} />
    </div>
  );
}

function ComputationOfTax({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-1601c computation-1601c">
      <h2>Part II – Computation of Tax</h2>
      {TAX_LINES.map(([number, label, key]) => (
        <TaxLine1601C
          key={number}
          number={number}
          label={label}
          value={decimal(envelope, key)}
          description={
            number === "20"
              ? text(envelope, "tax_20_other_name")
              : number === "29"
                ? text(envelope, "tax_29_other_remittances_name")
                : undefined
          }
        />
      ))}
    </section>
  );
}

function TaxLine1601C({
  number,
  label,
  value,
  description
}: {
  number: string;
  label: ReactNode;
  value: number;
  description?: string;
}) {
  const subheading = number === "15" ? "Less: Non-Taxable/Exempt Compensation" : null;

  return (
    <>
      {subheading && <div className={`tax-subheading-1601c tax-subheading-${number}`}>{subheading}</div>}
      <div
        className={`tax-line-1601c tax-line-${number} ${["21", "22", "24", "27", "30", "31", "35", "36"].includes(number) ? "computed" : ""}`}
        data-item={number}
      >
        <div>
          <b>{number}</b>
          <span>{label}</span>
          {description !== undefined && (
            <span className="inline-description-1601c">
              <PlainLine1601C value={description.toUpperCase()} cells={36} />
            </span>
          )}
        </div>
        <MoneyComb1601C value={value} />
      </div>
    </>
  );
}

function PlainLine1601C({ value, cells }: { value: string; cells: number }) {
  return (
    <AdaptivePlainValue value={value} cellCapacity={cells} className="inline-plain-1601c" />
  );
}

function MoneyComb1601C({ value }: { value: number | null }) {
  const [whole, fraction] = value === null ? ["", ""] : formatMoneyParts(value);
  if (Array.from(whole).length > 11) {
    return <AdaptiveComb1601C value={`${whole}.${fraction}`} cells={14} align="right" className="money-overflow-1601c" />;
  }
  return (
    <span className="money-1601c">
      <CombValue value={whole} cells={11} align="right" />
      <span>.</span>
      <CombValue value={fraction} cells={2} align="right" />
      <i aria-hidden="true" />
    </span>
  );
}

function Declaration1601C() {
  return (
    <section className="declaration-1601c">
      <p>I/We declare under the penalties of perjury that this remittance return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter)</em></p>
      <div className="signature-body-1601c">
        <div><span>For Individual:</span><b>Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
        <div><span>For Non-Individual:</span><b>Signature over Printed Name of President/Vice President/<br />Authorized Officer or Representative/Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
      </div>
      <div className="agent-strip-1601c">
        <span>Tax Agent Accreditation No./<br />Attorney’s Roll No. <em>(if applicable)</em></span>
        <span />
        <span>Date of Issue<br /><em>(MM/DD/YYYY)</em></span>
        <span />
        <span>Date of Expiry<br /><em>(MM/DD/YYYY)</em></span>
        <span />
      </div>
    </section>
  );
}

function PaymentDetails1601C() {
  return (
    <section className="payment-1601c">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-head-1601c"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date <em>(MM/DD/YYYY)</em></span><span>Amount</span></div>
      {[["37", "Cash/Bank Debit Memo"], ["38", "Check"]].map(([number, label]) => (
        <div className="payment-row-1601c" data-payment-line={number} key={number}>
          <span>{number} {label}</span><CombValue value="" cells={5} /><CombValue value="" cells={6} /><CombValue value="" cells={8} /><MoneyComb1601C value={null} />
        </div>
      ))}
      <div className="payment-row-1601c payment-tax-debit-1601c" data-payment-line="39">
        <span>39 Tax Debit Memo</span><CombValue value="" cells={6} /><CombValue value="" cells={8} /><MoneyComb1601C value={null} />
      </div>
      <div className="payment-other-label-1601c">40 Others (specify below)</div>
      <div className="payment-row-1601c payment-other-1601c" data-payment-line="40"><CombValue value="" cells={7} /><CombValue value="" cells={5} /><CombValue value="" cells={6} /><CombValue value="" cells={8} /><MoneyComb1601C value={null} /></div>
      <div className="machine-validation-1601c"><span>Machine Validation/Revenue Official Receipt Details <small>(if not filed with an Authorized Agent Bank)</small></span><span>Stamp of Receiving Office/AAB and Date of Receipt<br /><em>(RO’s Signature/Bank Teller’s Initial)</em></span></div>
    </section>
  );
}

function PageTwo({ envelope, rows }: { envelope: RenderEnvelope; rows: RenderRow[] }) {
  const visualRows: Array<RenderRow | null> = [...rows];
  while (visualRows.length < 3) visualRows.push(null);
  return (
    <>
      <Masthead1601C page={2} />
      <div className="identity-1601c-page-two">
        <span className="identity-label-1601c">TIN</span><span className="identity-label-1601c">Withholding Agent’s Name <em>(Last Name for Individual OR Registered Name for Non-Individual)</em></span>
        <AdaptiveComb1601C value={envelope.taxpayer.tin.replace(/\D/g, "")} cells={14} />
        <AdaptiveComb1601C value={envelope.taxpayer.name.toUpperCase()} cells={38} />
      </div>
      <section className="schedule-1601c">
        <h2>Part IV - Schedule</h2>
        <h3>Schedule I – Adjustment of Taxes Withheld on Compensation from Previous Months <em>(attach additional sheet/s, if necessary)</em></h3>
        <div className="schedule-top-head-1601c">
          <span>Previous Month/s<br /><em>(MM/YYYY)</em><b>1</b></span>
          <span>Date Paid<br /><em>(MM/DD/YYYY)</em><b>2</b></span>
          <span>Drawee Bank/Bank<br />Code/Agency<b>3</b></span>
          <span>Number<b>4</b></span>
          <span>
            <span className="schedule-tax-paid-label-1601c">
              Tax Paid <em>(Excluding Penalties for the Month)</em>
            </span>
            <b>5</b>
          </span>
        </div>
        {visualRows.map((row, index) => <ScheduleTopRow1601C key={row?.key ?? `schedule-1-empty-${index + 1}`} row={row} number={index + 1} />)}
        <div className="schedule-bottom-head-1601c"><span>Should be Tax Due for the Month<b>6</b></span><span>Adjustments<b>7 = (6 less 5)</b></span></div>
        {visualRows.map((row, index) => <ScheduleBottomRow1601C key={row?.key ?? `schedule-1-bottom-empty-${index + 1}`} row={row} number={index + 1} />)}
        <div className="schedule-total-1601c"><b>4</b><span>Total Adjustment <em>(Sum of Items 1 to 3) (To Part II, Item 26)</em></span><MoneyComb1601C value={decimal(envelope, "tax_26_adjustment")} /></div>
      </section>
      <Guidelines1601C />
    </>
  );
}

function ScheduleTopRow1601C({ row, number }: { row: RenderRow | null; number: number }) {
  const props = row ? { "data-row-key": row.key } : { "data-visual-placeholder": "true" };
  return (
    <div className="schedule-top-row-1601c" {...props}>
      <b>{number}</b>
      <AdaptiveComb1601C value={row ? cellText(row.cells.previous_month) : ""} cells={6} />
      <AdaptiveComb1601C value={row ? cellText(row.cells.date_paid) : ""} cells={8} />
      <AdaptiveComb1601C value={row ? cellText(row.cells.drawee_bank_code_or_agency) : ""} cells={5} />
      <AdaptiveComb1601C value={row ? cellText(row.cells.payment_number) : ""} cells={6} />
      <MoneyComb1601C value={row ? cellDecimal(row.cells.tax_paid) : null} />
    </div>
  );
}

function AdaptiveComb1601C({
  value,
  cells,
  align = "left",
  className = ""
}: {
  value: string;
  cells: number;
  align?: "left" | "right";
  className?: string;
}) {
  return (
    <AdaptiveCombValue
      value={value}
      cells={cells}
      align={align}
      className={className}
      fitToField
      maxFontSizePx={9.6}
      minFontSizePx={8}
      fontStepPx={0.5}
    />
  );
}

function ScheduleBottomRow1601C({ row, number }: { row: RenderRow | null; number: number }) {
  return (
    <div className="schedule-bottom-row-1601c">
      <b>{number}</b>
      <MoneyComb1601C value={row ? cellDecimal(row.cells.should_be_tax_due) : null} />
      <MoneyComb1601C value={row ? cellDecimal(row.cells.adjustment) : null} />
    </div>
  );
}

function Guidelines1601C() {
  return (
    <section className="guidelines-1601c">
      <h2>Guidelines and Instructions for BIR Form No. 1601-C <small>[January 2018 (ENCS)]</small></h2>
      <h3>Monthly Remittance Return of Income Taxes Withheld on Compensation</h3>
      <div className="guideline-columns-1601c">
        <div>
          <h4>Who Shall File</h4>
          <p>This monthly remittance return shall be filed in triplicate by every withholding agent (WA)/payor required to deduct and withhold taxes on compensation paid to employees.</p>
          <p>If the person required to withhold and pay/remit the tax is a corporation, the return shall be made in the name of the corporation and shall be signed and verified by the president, vice-president, or any authorized officer.</p>
          <p>If the Government of the Philippines or any of its agencies, political subdivisions or instrumentalities, or a government-owned or controlled corporation, is the withholding agent/payor, the return shall be accomplished and signed by the officer or employee having control of disbursement of income payments or other officer or employee appropriately designated for the purpose.</p>
          <p>With respect to a fiduciary, the return shall be made in the name of the individual, estate or trust for which such fiduciary acts and shall be signed and verified by such fiduciary. In case of two or more joint fiduciaries, the return shall be signed and verified by one of such fiduciaries.</p>
          <p>Authorized Representative and Accredited Tax Agent filing, in behalf of the taxpayer, shall also use this return to pay/remit the creditable taxes withheld.</p>
          <p>In the case of hazardous employment, the employer in the private sector shall attach to BIR Form No. 1601-C, for return periods March, June, September and December a copy of the list submitted to the Department of Labor &amp; Employment Regional/Provincial Offices–Operations Division/Unit. The list shall show the names of the Minimum Wage Earners who received the hazard pay, period of employment, amount of hazard pay per month and justification for payment of hazard pay as certified by said DOLE/allied agency that the hazard pay is justifiable. In the same manner, for the aforementioned return periods, employer in the public sector shall attach a copy of Department of Budget and Management (DBM) circular/s or equivalent, as to who are allowed to receive hazard pay.</p>
          <h4>When and Where to File and Pay/Remit</h4>
          <p>The return shall be filed and the tax paid/remitted on or before the tenth (10<sup>th</sup>) day of the month following the month in which withholding was made except for taxes withheld for December which shall be filed and paid/remitted on or before January 15 of the succeeding year.</p>
          <p>Provided, however, that with respect to non-large and large taxpayers who shall file through the Electronic Filing and Payment System (eFPS), the deadline for electronically filing the return and paying/remitting the taxes due thereon shall be in accordance with the provisions of existing applicable revenue issuances.</p>
          <p>The return shall be filed and the tax paid/remitted with the Authorized Agent Bank (AAB) of the Revenue District Office (RDO) having jurisdiction over the withholding agent’s place of business/office. In places where there are no Authorized Agent Banks, the return shall be filed and the tax paid/remitted with the Revenue Collection Officer (RCO) of the RDO having jurisdiction over the WA’s place of business/office, who will issue an Electronic Revenue Official Receipt (eROR) therefor.</p>
          <p>When the return is filed with an AAB, taxpayer must accomplish and submit BIR-prescribed deposit slip, which the bank teller shall machine validate as evidence that payment/remittance was received by the AAB. The AAB receiving the tax return shall stamp mark the word “Received” on the return and also machine validate the return as proof of filing and payment/remittance of the tax by the taxpayer. The machine validation shall reflect the date of payment/remittance, amount paid/remitted and transactions code, the name of the bank, branch code, teller’s code and teller’s initial. Bank debit memo number and date should be indicated in the return for taxpayers paying/remitting under the bank debit system.</p>
          <p>Payment/Remittance may also be made thru the epayment channels of AABs thru either their online facility, credit/debit/prepaid cards, and mobile payments.</p>
          <p>A taxpayer may file a separate return for the head office and for each branch or place of business/office or a consolidated return for the head office and all the branches/offices. In the case of large taxpayers only one consolidated return is required.</p>
          <h4>Penalties</h4>
          <p>There shall be imposed and collected as part of the tax:</p>
          <ol>
            <li>A surcharge of twenty-five percent (25%) for the following violations:
              <ol type="a">
                <li>Failure to file any return and pay the amount of tax or installment due on or before the due date;</li>
                <li>Filing a return with a person or office other than those with whom it is required to be filed, unless otherwise authorized by the Commissioner;</li>
                <li>Failure to pay the full or part of the amount of tax shown on the return, or the full amount of tax due for which no return is required to be filed on or before the due date;</li>
                <li>Failure to pay the deficiency tax within the time prescribed for its payment in the notice of assessment.</li>
              </ol>
            </li>
            <li>A surcharge of fifty percent (50%) of the tax or of the deficiency tax, in case any payment has been made before the discovery of the falsity or fraud, for each of the following violations:</li>
          </ol>
        </div>
        <div>
          <ol type="a" className="continued-guidelines-1601c">
            <li>Willful neglect to file the return within the period prescribed by the Code or by rules and regulations; or</li>
            <li>A false or fraudulent return is willfully made.</li>
          </ol>
          <ol start={3}>
            <li>Interest at the rate of double the legal interest rate for loans or forbearance of any money in the absence of an express stipulation as set by the Bangko Sentral ng Pilipinas from the date prescribed for payment until the amount is fully remitted: Provided, That in no case shall the deficiency and the delinquency interest prescribed under Section 249 Subsections (B) and (C) of the National Internal Revenue Code, as amended, be imposed simultaneously.</li>
            <li>Compromise penalty as provided under applicable rules and regulations.</li>
          </ol>
          <h4>Violation of Withholding Tax Provisions</h4>
          <p>Any person required to withhold, account for, and pay/remit any tax imposed by the National Internal Revenue Code (NIRC), as amended, or who willfully fails to withhold such tax, or account for and pay/remit such tax, or aids or abets in any manner to evade any such tax or the payment/remittance thereof, shall, in addition to other penalties provided for under the Law, be liable upon conviction to a penalty equal to the total amount of the tax not withheld, or not accounted for and paid/remitted.</p>
          <p>Any person required under the NIRC, as amended, or by rules and regulations promulgated thereunder to pay/remit any tax, make a return, keep any record, or supply correct and accurate information, who willfully fails to pay/remit such tax, make such return, keep such record, or supply such correct and accurate information, or withhold or pay/remit taxes withheld, or refund excess taxes withheld on compensation, at the time or times required by law or rules and regulations shall, in addition to the other penalties provided by law, upon conviction thereof, be punished by a fine of not less than ten thousand pesos (₱10,000.00) and suffer imprisonment of not less than one (1) year but not more than ten (10) years.</p>
          <p>Every officer or employee of the Government of the Republic of the Philippines or any of its agencies and instrumentalities, its political subdivisions, as well as government-owned or controlled corporations, including the Bangko Sentral ng Pilipinas, who, under the provisions of the NIRC, as amended, or regulations promulgated thereunder, is charged with the duty to deduct and withhold any internal revenue tax and to pay/remit the same in accordance with the provisions of the NIRC, as amended, and other laws and who is found guilty of any offense herein below specified shall, upon conviction of each act or omission, be fined in a sum not less than five thousand pesos (₱5,000) but not more than fifty thousand pesos (₱50,000) or imprisoned for a term of not less than six (6) months and one day but not more than two (2) years, or both:</p>
          <ol type="a">
            <li>Those who fail or cause the failure to deduct and withhold any internal revenue tax under any of the withholding tax laws and implementing regulations;</li>
            <li>Those who fail or cause the failure to pay/remit taxes deducted and withheld within the time prescribed by law, and implementing regulations; and</li>
            <li>Those who fail or cause the failure to file a return or statement within the time prescribed, or render or furnish a false or fraudulent return or statement required under the withholding tax laws and regulations.</li>
          </ol>
          <p>If the withholding agent is the Government or any of its agencies, political subdivisions or instrumentalities, or a government-owned or controlled corporation, the employee thereof responsible for the withholding and payment/remittance of tax shall be personally liable for the additions to the tax prescribed by the NIRC, as amended.</p>
          <h4>Required Attachments:</h4>
          <ol>
            <li>For Private Sector, copy of the list submitted to the DOLE Regional/Provincial Offices – Operations Division/Unit.</li>
            <li>For Public Sector, copy of Department of Budget and Management (DBM) circular/s or equivalent.</li>
          </ol>
          <h4 className="note-heading-1601c">Note: <span>All background information must be properly filled out.</span></h4>
          <ul className="note-list-1601c">
            <li>The last 5 digits of the 14-digit TIN refers to the branch code</li>
            <li>All returns filed by an accredited tax agent on behalf of a taxpayer shall bear the following information:
              <div className="note-numbering-1601c">
                <p><b>A.</b> For Individual (CPAs, members of GPPs, and others)</p>
                <p><b>a.1</b> Taxpayer Identification Number (TIN); and</p>
                <p><b>a.2</b> BIR Accreditation Number, Date of Issue, and Date of Expiry.</p>
                <p><b>B.</b> For members of the Philippine Bar (Lawyers)</p>
                <p><b>b.1</b> Taxpayer Identification Number (TIN);</p>
                <p><b>b.2</b> Attorney’s Roll Number;</p>
                <p><b>b.3</b> Mandatory Continuing Legal Education (MCLE) Compliance Number; and</p>
                <p><b>b.4</b> BIR Accreditation Number, Date of Issue, and Date of Expiry.</p>
              </div>
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}

function requireCellCapacity(value: string, cells: number, path: string): string {
  if (Array.from(value).length > cells) {
    throw new Error(`${path} requires ${Array.from(value).length} cells but 1601C allows ${cells}`);
  }
  return value;
}
