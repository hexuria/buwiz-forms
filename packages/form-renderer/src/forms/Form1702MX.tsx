import type { RenderEnvelope, RenderValue } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import { Fragment, type ReactNode } from "react";
import {
  AdaptiveCombValue,
  AdaptivePlainValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary
} from "../components";
import { bool, text } from "../values";
import {
  OFFICIAL_1702MX_PDF417_PATHS,
  OFFICIAL_1702MX_PDF417_PAYLOADS,
  OFFICIAL_1702MX_SEAL
} from "./official1702MXAssets";
import "./Form1702MX.css";

const SCHEDULE_TWO_LABELS: readonly ReactNode[] = [
  <>Sales/Receipts/Revenues/Fees <small>(From all Part V-Schedule B Item 1, if applicable)</small></>,
  "Less: Sales Returns, Allowances and Discounts",
  "Net Sales/Receipts/Revenues/Fees (Item 1 Less Item 2)",
  "Less: Cost of Sales/Services",
  "Gross Income from Operation (Item 3 Less Item 4)",
  "Add: Other Taxable Income not subjected to Final Tax",
  "Total Taxable Income (Sum of Items 5 and 6)",
  "Ordinary Allowable Itemized Deductions",
  "Special Allowable Itemized Deductions",
  "NOLCO",
  "Total Itemized Deductions (Sum of Items 8 to 10)",
  "Optional Standard Deduction (OSD) (40% of Item 7)",
  "Net Taxable Income/(Loss)",
  "Applicable Income Tax Rate",
  "Income Tax Due other than MCIT",
  "Less: Share of Other Government Agency, if remitted directly",
  "Net Income Tax Due to National Government",
  "MCIT (2% of Gross Income in Item 7)",
  "Total Income Tax Due / (Overpayment)"
];

const SCHEDULE_THREE_LABELS: readonly ReactNode[] = [
  "Prior Year’s Excess Credits other than MCIT",
  "Income Tax Payment under MCIT from Previous Quarter/s",
  "Income Tax Payment Under Regular Rate from Previous Quarter/s",
  "Excess MCIT Applied this Current Taxable Year",
  "Creditable Tax Withheld from Previous Quarter/s per BIR Form No. 2307",
  "Creditable Tax Withheld per BIR Form No. 2307 for the 4th Quarter",
  "Foreign Tax Credits, if applicable",
  "Tax Paid in Return Previously Filed, if this is an Amended Return",
  "Income Tax Payments under Special Rate from Previous Quarter/s",
  "Special Tax Credits (To Part IV-Schedule 4 Item 6)",
  "Other Tax Credits/Payments (specify)",
  "Other Tax Credits/Payments (specify)",
  "Total Tax Credits/Payments (Sum of Items 20 to 31)",
  "Net Tax Payable / (Overpayment) (Item 19 Less Item 32)"
];

type RegimeColumn1702MX = "exempt" | "special" | "regular" | "total";

const REGIME_COLUMNS_1702MX: readonly (readonly [RegimeColumn1702MX, string])[] = [
  ["exempt", "exempt"],
  ["special", "special"],
  ["regular", "regular"],
  ["total", "total"]
];

// Exact full-cell applicability map from the pinned official page 2 PDF
// (SHA-256 81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2).
const SCHEDULE_TWO_UNAVAILABLE_COLUMNS_1702MX: Readonly<Record<number, readonly RegimeColumn1702MX[]>> = {
  10: ["exempt"],
  12: ["exempt", "special"],
  14: ["total"],
  16: ["exempt"],
  17: ["exempt"],
  18: ["exempt", "special"],
  19: ["exempt"]
};

const SCHEDULE_THREE_UNAVAILABLE_COLUMNS_1702MX: Readonly<Record<number, readonly RegimeColumn1702MX[]>> = {
  21: ["exempt", "special"],
  23: ["exempt", "special"]
};

const SCHEDULE_FOUR_LABELS: readonly ReactNode[] = [
  "Regular Income Tax Otherwise Due",
  "Tax Relief on Special Allowable Itemized Deductions",
  "Sub-Total – Tax Relief (Sum of Items 1 and 2)",
  "Less: Income Tax Due",
  "Tax Relief Availment before Special Tax Credit",
  "Add: Special Tax Credit, if any",
  "Total Tax Relief Availment (Sum of Items 5 and 6)"
];

const SCHEDULE_FIVE_LABELS: readonly ReactNode[] = [
  "Amortizations",
  "Bad Debts",
  "Charitable and Other Contributions",
  "Depletion",
  "Depreciation",
  "Entertainment, Amusement and Recreation",
  "Fringe Benefits",
  "Interest",
  "Losses",
  "Pension Trusts",
  "Rental",
  "Research and Development",
  "Salaries, Wages and Allowances",
  "SSS, GSIS, Philhealth, HDMF and Other Contributions",
  "Taxes and Licenses",
  "Transportation and Travel",
  "a. Janitorial and Messengerial Services",
  "b. Professional Fees",
  "c. Security Services",
  "d.",
  "e.",
  "f.",
  "g.",
  "h.",
  "i.",
  "Total Ordinary Allowable Itemized Deductions"
];

export function Form1702MX({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("1702MX", "2018C");
  if (envelope.schedules.length !== 0) {
    throw new Error("1702MXv2018C uses fixed official schedule rows and accepts no continuation schedule");
  }

  const attachmentHasValues = bool(envelope, "mandatory_attachment_has_values");

  return (
    <main className="form-document" data-form-code="1702MX">
      <FolioPage pageNumber={1} className="form-1702mx-page form-1702mx-page-one">
        <GovernmentHeader1702MX />
        <Masthead1702MX page={1} />
        <HeaderOptions1702MX envelope={envelope} />
        <PartOne1702MX envelope={envelope} />
        <PartTwo1702MX envelope={envelope} />
        <Declaration1702MX envelope={envelope} />
        <PartThree1702MX envelope={envelope} />
      </FolioPage>

      <FolioPage pageNumber={2} className="form-1702mx-page form-1702mx-page-two">
        <Masthead1702MX page={2} compact />
        <PageIdentity1702MX envelope={envelope} />
        <PartFourHeading1702MX envelope={envelope} />
        <ScheduleOne1702MX envelope={envelope} />
        <RegimeTable1702MX
          className="schedule-two-1702mx"
          title="Schedule 2 – Computation of Income Tax per Tax Regime"
          labels={SCHEDULE_TWO_LABELS}
          startNumber={1}
          fieldPrefix="schedule_2"
          envelope={envelope}
          groupsBefore={{
            7: "Less: Deductions Allowable under Existing Law",
            11: "OR [in case taxable under Sec 27(A) & 28(A)(1)]"
          }}
          unavailableColumnsByNumber={SCHEDULE_TWO_UNAVAILABLE_COLUMNS_1702MX}
        />
        <RegimeTable1702MX
          className="schedule-three-1702mx"
          title="Schedule 3 – Tax Credits/Payments (attach proof)"
          labels={SCHEDULE_THREE_LABELS}
          startNumber={20}
          fieldPrefix="schedule_3"
          envelope={envelope}
          labelOverride={(index, label) => scheduleThreeLabel(envelope, index, label)}
          unavailableColumnsByNumber={SCHEDULE_THREE_UNAVAILABLE_COLUMNS_1702MX}
        />
      </FolioPage>

      <FolioPage pageNumber={3} className="form-1702mx-page form-1702mx-page-three">
        <Masthead1702MX page={3} compact />
        <PageIdentity1702MX envelope={envelope} />
        <RegimeTable1702MX
          className="schedule-four-1702mx"
          title="Schedule 4 – Tax Relief Availment"
          labels={SCHEDULE_FOUR_LABELS}
          startNumber={1}
          fieldPrefix="schedule_4"
          envelope={envelope}
        />
        <ScheduleFive1702MX envelope={envelope} />
        <ScheduleSix1702MX envelope={envelope} />
        <NolcoComputation1702MX
          className="schedule-seven-1702mx"
          title="Schedule 7 – Computation of Net Operating Loss Carry Over (NOLCO) for Regular Rate"
          prefix="schedule_7"
          envelope={envelope}
        />
      </FolioPage>

      <FolioPage pageNumber={4} className="form-1702mx-page form-1702mx-page-four">
        <Masthead1702MX page={4} compact />
        <PageIdentity1702MX envelope={envelope} />
        <NolcoTable1702MX
          className="schedule-seven-one-1702mx"
          title="Schedule 7.1 – Computation of Available Net Operating Loss Carry Over (NOLCO) for Regular Rate"
          prefix="schedule_7_1"
          envelope={envelope}
        />
        <NolcoComputation1702MX
          className="schedule-eight-1702mx"
          title={(
            <>
              <span>
                Schedule 8 – Computation of Net Operating Loss Carry Over (NOLCO) for Special Rate{" "}
                <strong>(except those availing fiscal incentives)</strong>
              </span>
              <small>(Attach Additional Sheet/s, if necessary)</small>
            </>
          )}
          prefix="schedule_8"
          envelope={envelope}
        />
        <NolcoTable1702MX
          className="schedule-eight-one-1702mx"
          title="Schedule 8.1 – Computation of Available Net Operating Loss Carry Over (NOLCO) for Special Rate"
          prefix="schedule_8_1"
          envelope={envelope}
        />
        <ScheduleNine1702MX envelope={envelope} />
        <ScheduleTen1702MX envelope={envelope} />
      </FolioPage>

      {(attachmentHasValues || bool(envelope, "relief_multiple_activities")) && (
        <aside className="attachment-blocker-1702mx" role="alert">
          The reviewed mandatory attachment is a separate two-page document. Its transport
          contract is not implemented, so it was not appended to this four-page return.
        </aside>
      )}
      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader1702MX() {
  return (
    <header className="government-header-1702mx">
      <span>For BIR<br />Use Only:</span>
      <span>BCS/<br />Item:</span>
      <span className="government-wordmark-1702mx">
        <img src={OFFICIAL_1702MX_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</span>
      </span>
    </header>
  );
}

function Masthead1702MX({ page, compact = false }: { page: 1 | 2 | 3 | 4; compact?: boolean }) {
  const payload = OFFICIAL_1702MX_PDF417_PAYLOADS[page];
  return (
    <header className={`masthead-1702mx${compact ? " compact-1702mx" : ""}`}>
      <div className="form-number-1702mx">
        <span>BIR Form No.</span>
        <strong>1702-MX</strong>
        <small>January 2018 (ENCS)</small>
        <b>Page {page}</b>
      </div>
      <div className="form-title-1702mx">
        <strong>Annual Income Tax Return</strong>
        <span>Corporation, Partnership and Other Non-Individual<br />with MIXED Income Subject to Multiple Income Tax Rates or<br />with Income Subject to SPECIAL/PREFERENTIAL RATE</span>
        {!compact && (
          <em>
            Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable
            boxes with an “X”. Two copies MUST be filed with the BIR and one held by the taxpayer.
          </em>
        )}
      </div>
      <div className="barcode-1702mx" aria-label={payload} data-barcode-page={page}>
        <span className="official-pdf417-object-1702mx" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-1702mx"
            viewBox="0 0 120.5 8"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_1702MX_PDF417_PATHS[page]} />
          </svg>
        </span>
        <small>{payload}</small>
      </div>
    </header>
  );
}

function HeaderOptions1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const basis = text(envelope, "filing_basis");
  return (
    <section className="header-options-1702mx">
      <div className="period-options-1702mx">
        <span className="basis-row-1702mx"><b>1</b> For <CheckChoice checked={basis === "calendar"} label="Calendar" /><CheckChoice checked={basis === "fiscal"} label="Fiscal" /></span>
        <span className="year-row-1702mx"><b>2</b> Year Ended <small>(MM/20YY)</small><CombValue value={String(envelope.period.month ?? "").padStart(2, "0")} cells={2} /><i>/20</i><CombValue value={String(envelope.period.taxable_year).slice(-2)} cells={2} /></span>
      </div>
      <BinaryHeader1702MX number="3" label="Amended Return?" checked={bool(envelope, "is_amended")} />
      <BinaryHeader1702MX number="4" label="Short Period Return?" checked={bool(envelope, "is_short_period")} />
      <div className="atc-options-1702mx">
        <span><b>5</b> Alphanumeric Tax Code (ATC)</span>
        <span><span className="atc-code-box-1702mx">IC 055</span><span className="atc-description-box-1702mx">Minimum Corporate Income Tax (MCIT)</span><CheckChoice checked={bool(envelope, "atc_mcit_selected")} label="" /></span>
        <span><PlainValue1702MX value={text(envelope, "atc_other_code")} className="atc-code-box-1702mx" /><span className="atc-description-box-1702mx" /><CheckChoice checked={bool(envelope, "atc_other_selected")} label="" /></span>
      </div>
    </section>
  );
}

function BinaryHeader1702MX({ number, label, checked }: { number: string; label: string; checked: boolean }) {
  return (
    <div className="binary-header-1702mx">
      <span><b>{number}</b> {label}</span>
      <span><CheckChoice checked={checked} label="Yes" /><CheckChoice checked={!checked} label="No" /></span>
    </div>
  );
}

function PartOne1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const deduction = text(envelope, "deduction_method");
  return (
    <section className="part-one-1702mx ruled-section-1702mx">
      <SectionHeading1702MX>Part I – Background Information</SectionHeading1702MX>
      <div className="tin-rdo-1702mx">
        <span><b>6</b> Taxpayer Identification Number (TIN)</span>
        <Tin1702MX value={envelope.taxpayer.tin} />
        <span><b>7</b> RDO Code</span>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} />
      </div>
      <LabeledCombLines1702MX number="8" label="Registered Name (Enter only 1 letter per box using CAPITAL LETTERS)" value={envelope.taxpayer.name.toUpperCase()} cells={114} lines={3} />
      <Address1702MX envelope={envelope} />
      <div className="identity-split-1702mx">
        <LabeledCombLines1702MX number="10" label="Date of Incorporation/Organization (MM/DD/YYYY)" value={text(envelope, "incorporation_date").replace(/\D/g, "")} cells={8} lines={1} />
        <LabeledCombLines1702MX number="11" label="Contact Number" value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} lines={1} />
      </div>
      <LabeledCombLines1702MX number="12" label="Email Address" value={envelope.taxpayer.email.toUpperCase()} cells={40} lines={1} compact />
      <div className="deduction-method-1702mx">
        <span><b>13</b> Method of Deduction</span>
        <CheckChoice checked={deduction === "itemized"} label="Itemized Deduction [Section 34 (A-J), NIRC]" />
        <CheckChoice checked={deduction === "osd"} label="Optional Standard Deduction (OSD)–40% of Gross Income [Section 34(L) NIRC, as amended]" />
      </div>
    </section>
  );
}

function Address1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const characters = Array.from(envelope.taxpayer.registered_address.toUpperCase());
  const lineCapacities = [38, 38, 30] as const;
  const capacity = lineCapacities.reduce((total, lineCapacity) => total + lineCapacity, 0);
  const usesPlainBox = characters.length > capacity;
  let characterOffset = 0;
  return (
    <div
      className="address-field-1702mx"
      data-field-mode={usesPlainBox ? "plain" : "guided"}
      data-cell-capacity={capacity}
      data-line-cell-capacity={lineCapacities.join(",")}
    >
      <span><b>9</b> Registered Address <em>(Indicate complete address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></span>
      <div className="address-comb-1702mx">
        {usesPlainBox ? (
          <AdaptivePlainValue
            value={characters.join("")}
            cellCapacity={capacity}
            className="address-plain-value-1702mx"
            maxFontSizePx={9.333}
            minFontSizePx={5.333}
            fontStepPx={.5}
          />
        ) : lineCapacities.map((lineCapacity, index) => {
          const lineValue = characters
            .slice(characterOffset, characterOffset + lineCapacity)
            .join("");
          characterOffset += lineCapacity;
          return <CombValue key={index} value={lineValue} cells={lineCapacity} />;
        })}
        <span className="zip-field-1702mx"><span className="zip-label-1702mx"><b>9A</b> ZIP Code</span><CombValue value={envelope.taxpayer.zip_code} cells={4} /></span>
      </div>
    </div>
  );
}

function PartTwo1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const rows = [
    [14, "Total Tax Due/(Overpayment) (From Part IV-Schedule 2 Item 19D)", "item_14"],
    [15, "Less: Total Tax Credits/Payments (From Part IV-Schedule 3 Item 32D)", "item_15"],
    [16, "Net Tax Payable / (Overpayment) (Item 14 Less Item 15) (From Part IV Item 33D)", "item_16"],
    [17, "Surcharge", "item_17"],
    [18, "Interest", "item_18"],
    [19, "Compromise", "item_19"],
    [20, "Total Penalties (Sum of Items 17 to 19)", "item_20"],
    [21, "TOTAL AMOUNT PAYABLE / (Overpayment) (Sum of Items 16 and 20)", "item_21"]
  ] as const;
  const disposition = text(envelope, "overpayment_disposition");
  return (
    <section className="part-two-1702mx ruled-section-1702mx">
      <SectionHeading1702MX>Part II – Total Tax Payable <small>(DO NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small></SectionHeading1702MX>
      {rows.map(([number, label, key], index) => (
        <Fragment key={number}>
          {number === 17 && <div className="penalty-heading-1702mx">Add: Penalties</div>}
          <AmountRow1702MX number={number} label={label} value={amount(envelope, key)} className={index >= 3 && index <= 5 ? "penalty-row-1702mx" : ""} />
        </Fragment>
      ))}
      <div className="overpayment-row-1702mx">
        <span>If overpayment, mark one (1) box only. (Once the choice is made, the same is irrevocable)</span>
        <span><CheckChoice checked={disposition === "refund"} label="To be refunded" /><CheckChoice checked={disposition === "tcc"} label="To be issued a Tax Credit Certificate (TCC)" /><CheckChoice checked={disposition === "carry_over"} label="To be carried over as tax credit for next year/quarter" /></span>
      </div>
    </section>
  );
}

function Declaration1702MX({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-1702mx">
      <p>We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by us, and to the best of our knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. (If signed by an Authorized Representative, indicate TIN and attach authorization letter)</p>
      <div className="signature-area-1702mx">
        <Signatory1702MX
          name={text(envelope, "authorized_representative")}
          label="Signature over Printed Name of President/Principal Officer/Authorized Representative"
          title={text(envelope, "president_title")}
          tin={text(envelope, "president_tin")}
        />
        <Signatory1702MX
          name={text(envelope, "treasurer")}
          label="Signature over Printed Name of Treasurer/Assistant Treasurer"
          title={text(envelope, "treasurer_title")}
          tin={text(envelope, "treasurer_tin")}
        />
        <div className="attachments-cell-1702mx"><span><b>22</b> Number of<br />Attachments</span><CombValue value={text(envelope, "number_of_attachments")} cells={2} align="right" /></div>
      </div>
    </section>
  );
}

function Signatory1702MX({ name, label, title, tin }: { name: string; label: string; title: string; tin: string }) {
  return (
    <div className="signatory-1702mx">
      <span className="signature-space-1702mx"><PlainValue1702MX value={name} /></span>
      <span className="signature-label-1702mx">{label}</span>
      <span className="signature-footer-1702mx">
        <span>Title of Signatory</span>
        <PlainValue1702MX value={title} />
        <span>TIN</span>
        <PlainValue1702MX value={tin} />
      </span>
    </div>
  );
}

function PartThree1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const labels = ["23 Cash/Bank Debit Memo", "24 Check", "25 Tax Debit Memo"];
  return (
    <section className="part-three-1702mx ruled-section-1702mx">
      <SectionHeading1702MX>Part III – Details of Payment</SectionHeading1702MX>
      <div className="payment-grid-1702mx payment-header-1702mx"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date (MM/DD/YYYY)</span><span>Amount</span></div>
      {labels.map((label, index) => {
        const prefix = `payment_${index + 23}`;
        const combined = text(envelope, `${prefix}_date_or_amount`);
        return <PaymentEntry1702MX key={label} label={label} drawee={text(envelope, `${prefix}_drawee`)} number={text(envelope, `${prefix}_number`)} date={combined} />;
      })}
      <div className="payment-others-heading-1702mx">26 Others <em>(specify below)</em></div>
      <PaymentEntry1702MX
        label=""
        drawee={text(envelope, "payment_26_drawee")}
        number={text(envelope, "payment_26_number")}
        date={text(envelope, "payment_26_date_or_amount")}
      />
      <div className="validation-receipt-1702mx"><span>Machine Validation/Revenue Official Receipt Details [if not filed with an Authorized Agent Bank (AAB)]</span><span>Stamp of Receiving Office/AAB and Date of Receipt (RO’s Signature/Bank Teller’s Initial)</span></div>
    </section>
  );
}

function PaymentEntry1702MX({ label, drawee, number, date }: { label: string; drawee: string; number: string; date: string }) {
  return (
    <div className="payment-grid-1702mx payment-entry-1702mx">
      <span>{label}</span>
      <PlainValue1702MX value={drawee} className="payment-value-1702mx" />
      <PlainValue1702MX value={number} className="payment-value-1702mx" />
      <PlainValue1702MX value={date} className="payment-value-1702mx" />
      <span className="payment-amount-1702mx" />
    </div>
  );
}

function PageIdentity1702MX({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="page-identity-1702mx">
      <span><b>Taxpayer Identification Number (TIN)</b><Tin1702MX value={envelope.taxpayer.tin} /></span>
      <span><b>Registered Name</b><AdaptiveCombValue value={envelope.taxpayer.name.toUpperCase()} cells={48} /></span>
    </section>
  );
}

function PartFourHeading1702MX({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-four-heading-1702mx">
      <SectionHeading1702MX>Part IV – Schedules</SectionHeading1702MX>
      {/*
        * Instruction copy reproduced verbatim from the pinned official PDF, not
        * paraphrased. The renderer previously shortened both clauses and
        * dropped "(mark appropriate box)" entirely, so the printed form said
        * something the official document does not. Verified against
        * references/1702mx-2018c-official-static-text.json (PDF text layer).
        */}
      <div className="instructions-1702mx">
        <span className="instructions-lead-1702mx">
          <b>Instructions:</b>{" "}
          <CheckChoice
            checked={bool(envelope, "relief_single_activity")}
            label="A. Only one activity/project under EXEMPT and/or SPECIAL Tax Regimes, fill-out the applicable columns below."
          />
        </span>
        <span className="instructions-mark-1702mx">(mark appropriate box)</span>
        <CheckChoice
          checked={bool(envelope, "relief_multiple_activities")}
          label="B. Two or more activities/projects under EXEMPT and/or SPECIAL Tax Regimes, accomplish Part V-Mandatory Attachments per activity and reflect consolidated amounts from Part V on the corresponding columns below."
        />
      </div>
    </section>
  );
}

function ScheduleOne1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const rows = [
    ["1", "Investment Promotion Agency (IPA)/Implementing Government Entity"],
    ["2", "Legal Basis"],
    ["3", "Registered Activity/Program (Reg. No.)"],
    ["4", "Special Tax Rate"],
    ["5", "Effectivity Date of Tax Relief/Exemption FROM (MM/DD/YYYY)"],
    ["6", "Expiration Date of Tax Relief/Exemption TO (MM/DD/YYYY)"]
  ] as const;
  return (
    <section className="schedule-one-1702mx schedule-block-1702mx">
      <ScheduleTitle1702MX>Schedule 1 – Basis of Tax Relief</ScheduleTitle1702MX>
      <div className="relief-grid-1702mx relief-header-1702mx"><span>Particulars</span><span>A. Exempt</span><span>B. Special</span><span>C. Special Tax Relief<br />(Under Regular/Normal Rate)</span></div>
      {rows.map(([number, label]) => {
        const rowNumber = Number(number);
        return (
          <div
            key={number}
            className={`relief-grid-1702mx relief-row-1702mx relief-row-${number}-1702mx`}
            data-relief-row={number}
          >
            <span>{number} {label}</span>
            {rowNumber <= 3 ? (
              <>
                <ReliefCharacterField1702MX column="exempt" cells={10} />
                <ReliefCharacterField1702MX column="special" cells={10} />
                <ReliefCharacterField1702MX column="regular-relief" cells={9} />
              </>
            ) : rowNumber === 4 ? (
              <>
                <ReliefUnavailableField1702MX column="exempt" />
                <ReliefRateField1702MX value={amount(envelope, "relief_special_tax_rate")} />
                <ReliefUnavailableField1702MX column="regular-relief" />
              </>
            ) : (
              <>
                <ReliefDateField1702MX column="exempt" cells={10} unavailable={[0, 9]} />
                <ReliefDateField1702MX column="special" cells={10} unavailable={[0, 9]} />
                <ReliefDateField1702MX column="regular-relief" cells={9} unavailable={[0]} />
              </>
            )}
          </div>
        );
      })}
    </section>
  );
}

type ReliefColumn1702MX = "exempt" | "special" | "regular-relief";

function ReliefCharacterField1702MX({ column, cells }: { column: ReliefColumn1702MX; cells: number }) {
  return (
    <span
      className="relief-field-1702mx relief-character-field-1702mx"
      data-relief-column={column}
      data-guide-cells={cells}
    >
      {Array.from({ length: cells }, (_, index) => <i key={index} aria-hidden="true" />)}
    </span>
  );
}

function ReliefUnavailableField1702MX({ column }: { column: ReliefColumn1702MX }) {
  return (
    <span
      className="relief-field-1702mx relief-unavailable-field-1702mx"
      data-relief-column={column}
      data-unavailable-field="true"
    />
  );
}

function ReliefDateField1702MX({
  column,
  cells,
  unavailable
}: {
  column: ReliefColumn1702MX;
  cells: number;
  unavailable: readonly number[];
}) {
  const unavailableCells = new Set(unavailable);
  return (
    <span
      className="relief-field-1702mx relief-date-field-1702mx"
      data-relief-column={column}
      data-guide-cells={cells}
      data-applicable-cells={cells - unavailable.length}
      data-unavailable-cells={unavailable.length}
    >
      {Array.from({ length: cells }, (_, index) => (
        <i
          key={index}
          className={unavailableCells.has(index) ? "relief-cell-unavailable-1702mx" : undefined}
          aria-hidden="true"
        />
      ))}
    </span>
  );
}

function ReliefRateField1702MX({ value }: { value: number | null }) {
  const rate = value === null ? "" : value.toFixed(1);
  const [whole = "", fraction = ""] = rate.split(".");
  const wholeDigits = whole.slice(-2).padStart(2, " ");
  const values = [...wholeDigits, ".", fraction.slice(0, 1), "%"];
  return (
    <span
      className="relief-field-1702mx relief-rate-field-1702mx"
      data-relief-column="special"
      data-field-cells="10"
      data-applicable-character-cells="3"
      data-unavailable-cells="5"
      data-static-cells="2"
      data-rate-format="00.0%"
    >
      <i className="relief-cell-unavailable-1702mx relief-rate-prefix-1702mx" aria-hidden="true" />
      {values.map((cellValue, index) => (
        <i
          key={index}
          className={index === 2
              ? "relief-rate-decimal-1702mx"
              : index === 4
                ? "relief-rate-percent-1702mx"
                : undefined}
          aria-hidden={cellValue === "." || cellValue === "%" ? undefined : "true"}
        >
          {cellValue}
        </i>
      ))}
    </span>
  );
}

function RegimeTable1702MX({
  className,
  title,
  labels,
  startNumber,
  fieldPrefix,
  envelope,
  labelOverride,
  numberOverride,
  groupsBefore = {},
  unavailableColumnsByNumber = {}
}: {
  className: string;
  title: string;
  labels: readonly ReactNode[];
  startNumber: number;
  fieldPrefix: string;
  envelope: RenderEnvelope;
  labelOverride?: (index: number, label: ReactNode) => ReactNode;
  numberOverride?: (index: number, fallback: number) => string | number;
  groupsBefore?: Readonly<Record<number, string>>;
  unavailableColumnsByNumber?: Readonly<Record<number, readonly RegimeColumn1702MX[]>>;
}) {
  return (
    <section className={`${className} schedule-block-1702mx regime-table-1702mx`}>
      <ScheduleTitle1702MX>{title}</ScheduleTitle1702MX>
      <div className="regime-grid-1702mx regime-header-1702mx"><span>Description</span><span>A. Total Exempt</span><span>B. Total Special</span><span>C. Total Regular</span><span>D. Total All Columns</span></div>
      {labels.map((label, index) => {
        const fallbackNumber = startNumber + index;
        const itemNumber = numberOverride?.(index, fallbackNumber) ?? fallbackNumber;
        const prefix = `${fieldPrefix}_item_${index + 1}`;
        const unavailableColumns = new Set(unavailableColumnsByNumber[fallbackNumber] ?? []);
        return [
          groupsBefore[index] ? (
            <div key={`group-${index}`} className={`regime-group-row-1702mx group-before-${fallbackNumber}-1702mx`}>
              {groupsBefore[index]}
            </div>
          ) : null,
          (
            <div key={String(itemNumber)} className={`regime-grid-1702mx regime-row-1702mx item-${fallbackNumber}-1702mx`}>
              <span>{itemNumber} {labelOverride ? labelOverride(index, label) : label}</span>
              {REGIME_COLUMNS_1702MX.map(([column, fieldSuffix]) => (
                <RegimeAmountCell1702MX
                  key={column}
                  column={column}
                  unavailable={unavailableColumns.has(column)}
                  unavailableBlockContinues={
                    unavailableColumns.has(column)
                    && (unavailableColumnsByNumber[fallbackNumber + 1] ?? []).includes(column)
                  }
                  value={amount(envelope, `${prefix}_${fieldSuffix}`)}
                />
              ))}
            </div>
          )
        ];
      })}
    </section>
  );
}

function RegimeAmountCell1702MX({
  column,
  unavailable,
  unavailableBlockContinues,
  value
}: {
  column: RegimeColumn1702MX;
  unavailable: boolean;
  unavailableBlockContinues: boolean;
  value: number | null;
}) {
  if (!unavailable) return <AmountCell1702MX value={value} />;
  return (
    <span
      aria-hidden="true"
      className="amount-cell-1702mx regime-unavailable-cell-1702mx"
      data-regime-column={column}
      data-unavailable-block-continues={unavailableBlockContinues ? "true" : undefined}
      data-unavailable-cell="true"
    />
  );
}

function ScheduleFive1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const amountRow = (index: number, number: ReactNode, label: ReactNode) => {
    const prefix = `schedule_5_item_${index + 1}`;
    return (
      <div
        key={`schedule-5-${index}`}
        className={`regime-grid-1702mx regime-row-1702mx schedule-five-row-1702mx item-${index + 1}-1702mx`}
      >
        <span>{number} {label}</span>
        <AmountCell1702MX value={amount(envelope, `${prefix}_exempt`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_special`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_regular`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_total`)} />
      </div>
    );
  };

  return (
    <section className="schedule-five-1702mx schedule-block-1702mx regime-table-1702mx">
      <ScheduleTitle1702MX>
        <span>Schedule 5 – Ordinary Allowable Itemized Deductions (attach additional sheet/s, if necessary)</span>
        <small>(If with only one activity, fill-out the applicable columns below: if with two or more activities, amount for each expense shall come from all of Part V-Schedule D)</small>
      </ScheduleTitle1702MX>
      {SCHEDULE_FIVE_LABELS.slice(0, 16).map((label, index) => amountRow(index, index + 1, label))}
      <div className="schedule-five-group-row-1702mx">
        17 Others (Deductions Subject to Withholding Tax and Other Expenses) [Specify below; Add additional sheet(s), if necessary]
      </div>
      {SCHEDULE_FIVE_LABELS.slice(16, 25).map((label, offset) => {
        const index = offset + 16;
        let displayedLabel = label;
        if (offset < 3 && typeof label === "string") {
          displayedLabel = label.replace(/^[a-c]\.\s*/, "");
        } else if (offset >= 3) {
          displayedLabel = (
            <PlainValue1702MX
              value={text(envelope, `schedule_5_other_description_${offset - 2}`)}
            />
          );
        }
        return amountRow(index, `${String.fromCharCode(97 + offset)}.`, displayedLabel);
      })}
      <div className="regime-grid-1702mx regime-row-1702mx schedule-five-row-1702mx schedule-five-total-row-1702mx item-26-1702mx">
        <span><b>18 Total Ordinary Allowable Itemized Deductions</b><small>(Sum of Items 1 to 17i) (To Part IV-Schedule 2 Item 8)</small></span>
        <AmountCell1702MX value={amount(envelope, "schedule_5_item_26_exempt")} />
        <AmountCell1702MX value={amount(envelope, "schedule_5_item_26_special")} />
        <AmountCell1702MX value={amount(envelope, "schedule_5_item_26_regular")} />
        <AmountCell1702MX value={amount(envelope, "schedule_5_item_26_total")} />
      </div>
    </section>
  );
}

function ScheduleSix1702MX({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-six-1702mx schedule-block-1702mx regime-table-1702mx">
      <ScheduleTitle1702MX>Schedule 6 – Special Allowable Itemized Deductions (attach additional sheet/s, if necessary)</ScheduleTitle1702MX>
      <div className="special-grid-1702mx regime-header-1702mx"><span>Description</span><span>Legal Basis</span><span>A. Total Exempt</span><span>B. Total Special</span><span>C. Total Regular</span><span>D. Total All Columns</span></div>
      {[1, 2, 3, 4].map((item) => {
        const prefix = `schedule_6_row_${item}`;
        return <div key={item} className="special-grid-1702mx"><span>{item} <PlainValue1702MX value={text(envelope, `${prefix}_description`)} /></span><PlainValue1702MX value={text(envelope, `${prefix}_legal_basis`)} fixedFontSizePt={4} /><AmountCell1702MX value={amount(envelope, `${prefix}_exempt`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_special`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_regular`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_total`)} /></div>;
      })}
      <div className="special-grid-1702mx total-row-1702mx"><span>5 Total Special Allowable Itemized Deductions</span><span /><AmountCell1702MX value={amount(envelope, "schedule_6_item_5_exempt")} /><AmountCell1702MX value={amount(envelope, "schedule_6_item_5_special")} /><AmountCell1702MX value={amount(envelope, "schedule_6_item_5_regular")} /><AmountCell1702MX value={amount(envelope, "schedule_6_item_5_total")} /></div>
    </section>
  );
}

function NolcoComputation1702MX({ className, title, prefix, envelope }: { className: string; title: ReactNode; prefix: string; envelope: RenderEnvelope }) {
  const rows: ReactNode[] = prefix === "schedule_8"
    ? [
        <>Gross Income <small>(From Part IV-Schedule 2 Item 7B)</small></>,
        <>Less: Ordinary Allowable Itemized Deductions <small>(From Part IV-Schedule 2 Item 8B)</small></>,
        <>Net Operating Loss (Item 1 Less Item 2) <small>(To Part IV-Schedule 8.1 Item 7A)</small></>
      ]
    : [
        "Gross Income",
        "Less: Ordinary Allowable Itemized Deductions",
        "Net Operating Loss (Item 1 Less Item 2)"
      ];
  return (
    <section className={`${className} schedule-block-1702mx nolco-computation-1702mx`}>
      <ScheduleTitle1702MX>{title}</ScheduleTitle1702MX>
      {rows.map((label, index) => <AmountRow1702MX key={index} number={index + 1} label={label} value={amount(envelope, `${prefix}_item_${index + 1}`)} />)}
    </section>
  );
}

function NolcoTable1702MX({ className, title, prefix, envelope }: { className: string; title: string; prefix: string; envelope: RenderEnvelope }) {
  const totalDestination = prefix === "schedule_7_1" ? "10C" : "10B";
  return (
    <section className={`${className} schedule-block-1702mx nolco-table-1702mx`}>
      <ScheduleTitle1702MX>
        <span>{title}</span>
        <small>DO NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up)</small>
      </ScheduleTitle1702MX>
      <div className="nolco-grid-1702mx nolco-header-1702mx">
        <span className="nolco-loss-heading-1702mx">Net Operating Loss</span>
        <span className="nolco-year-heading-1702mx">Year<br />Incurred</span>
        <span className="nolco-amount-heading-1702mx">A. Amount</span>
        <span className="nolco-b-heading-1702mx">B. NOLCO Applied<br />Previous Year/s</span>
        <span className="nolco-c-heading-1702mx">C. NOLCO Expired</span>
        <span className="nolco-d-heading-1702mx">D. NOLCO Applied<br />Current Year</span>
        <span className="nolco-e-heading-1702mx">E. Net Operating Loss<br />(Unapplied)<br />[(E) = A - (B+C+D)]</span>
      </div>
      {[4, 5, 6, 7].map((item) => {
        const row = `${prefix}_row_${item}`;
        return <div key={item} className="nolco-grid-1702mx nolco-data-row-1702mx"><span className="nolco-year-cell-1702mx"><b>{item}</b><PlainValue1702MX value={text(envelope, `${row}_year`)} /></span><AmountCell1702MX value={amount(envelope, `${row}_amount`)} /><AmountCell1702MX value={amount(envelope, `${row}_applied_previous_years`)} /><AmountCell1702MX value={amount(envelope, `${row}_expired`)} /><AmountCell1702MX value={amount(envelope, `${row}_applied_current_year`)} /><AmountCell1702MX value={amount(envelope, `${row}_unapplied`)} /></div>;
      })}
      <div className="nolco-total-row-1702mx total-row-1702mx">
        <b>8</b>
        <span>
          Total NOLCO <small>(Sum of Items 4D to 7D) (To Part IV-Schedule 2 Item {totalDestination})</small>
        </span>
        <AmountCell1702MX value={amount(envelope, `${prefix}_item_8_total`)} />
        <span aria-hidden="true" />
      </div>
    </section>
  );
}

function ScheduleNine1702MX({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="schedule-nine-1702mx schedule-block-1702mx">
      <ScheduleTitle1702MX>Schedule 9 – Computation of Minimum Corporate Income Tax (MCIT)</ScheduleTitle1702MX>
      <div className="mcit-first-grid-1702mx regime-header-1702mx"><span>Year</span><span>A) Normal Income Tax as Adjusted</span><span>B) MCIT</span><span>C) Excess MCIT over Normal Income Tax</span></div>
      {[1, 2, 3].map((item) => {
        const prefix = `schedule_9_row_${item}`;
        return <div key={item} className="mcit-first-grid-1702mx mcit-data-row-1702mx"><span className="mcit-year-cell-1702mx"><b>{item}</b><PlainValue1702MX value={text(envelope, `${prefix}_year`)} /></span><AmountCell1702MX value={amount(envelope, `${prefix}_normal_income_tax`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_mcit`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_excess_mcit`)} /></div>;
      })}
      <ScheduleTitle1702MX>Continuation of Schedule 9 <small>(Item numbers continue from table above)</small></ScheduleTitle1702MX>
      <div className="mcit-second-grid-1702mx regime-header-1702mx"><span>D) Excess MCIT Applied/<br />Used in Previous Years</span><span>E) Expired Portion of<br />Excess MCIT</span><span>F) Excess MCIT Applied this<br />Current Taxable Year</span><span>G) Balance of Excess MCIT Allowable<br />as Tax Credit for Succeeding Year/s<br />[ G = C Less (D + E + F) ]</span></div>
      {[1, 2, 3].map((item) => {
        const prefix = `schedule_9_row_${item}`;
        return <div key={item} className="mcit-second-grid-1702mx mcit-continuation-row-1702mx"><span className="mcit-numbered-amount-1702mx"><b>{item}</b><AmountCell1702MX value={amount(envelope, `${prefix}_applied_previous_years`)} /></span><AmountCell1702MX value={amount(envelope, `${prefix}_expired`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_applied_current_year`)} /><AmountCell1702MX value={amount(envelope, `${prefix}_balance`)} /></div>;
      })}
      <div className="mcit-total-row-1702mx total-row-1702mx">
        <span><b>4</b> Total Excess MCIT Applied <small>(Sum of Items 1F to 3F) (To Part IV-Schedule 3 Item 23)</small></span>
        <AmountCell1702MX value={amount(envelope, "schedule_9_item_4_total")} />
        <span aria-hidden="true" />
      </div>
    </section>
  );
}

function ScheduleTen1702MX({ envelope }: { envelope: RenderEnvelope }) {
  const row = (item: number, defaultLabel = "") => {
    const prefix = `schedule_10_item_${item}`;
    const description = text(envelope, `schedule_10_description_${item}`);
    const acceptsDescription = [2, 3, 5, 6, 7, 8].includes(item);
    return (
      <div key={item} className={`regime-grid-1702mx regime-row-1702mx schedule-ten-row-1702mx item-${item}-1702mx`}>
        <span>
          {item}{" "}
          {acceptsDescription
            ? <PlainValue1702MX value={description} className="schedule-ten-description-1702mx" />
            : description
              ? <PlainValue1702MX value={description} />
              : defaultLabel}
        </span>
        <AmountCell1702MX value={amount(envelope, `${prefix}_exempt`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_special`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_regular`)} />
        <AmountCell1702MX value={amount(envelope, `${prefix}_total`)} />
      </div>
    );
  };

  return (
    <section className="schedule-ten-1702mx schedule-block-1702mx regime-table-1702mx">
      <ScheduleTitle1702MX>Schedule 10 – Reconciliation of Net Income per Books Against Taxable Income <small>(attach additional sheet/s, if necessary)</small></ScheduleTitle1702MX>
      <div className="regime-grid-1702mx regime-header-1702mx"><span>Particulars</span><span>A. Total Exempt</span><span>B. Total Special</span><span>C. Total Regular</span><span>D. Total All Columns</span></div>
      {row(1, "Net Income/(Loss) per Books")}
      <div className="schedule-ten-group-row-1702mx">Add: Non-Deductible Expenses/Taxable Other Income <small>(specify below)</small></div>
      {row(2)}
      {row(3)}
      {row(4, "Total (Sum of Items 1 to 3)")}
      <div className="schedule-ten-group-row-1702mx">Less: A) Non-Taxable Income and Income Subjected to Final Tax <small>(specify below)</small></div>
      {row(5)}
      {row(6)}
      <div className="schedule-ten-group-row-1702mx schedule-ten-group-b-1702mx">B) Special Deductions <small>(specify below)</small></div>
      {row(7)}
      {row(8)}
      {row(9, "Total (Sum of Items 5 to 8)")}
      {row(10, "Net Taxable Income/(Loss) (Item 4 Less Item 9)")}
    </section>
  );
}

function SectionHeading1702MX({ children }: { children: ReactNode }) {
  return <h2 className="section-heading-1702mx">{children}</h2>;
}

function ScheduleTitle1702MX({ children }: { children: ReactNode }) {
  return <h3 className="schedule-title-1702mx">{children}</h3>;
}

function Tin1702MX({ value }: { value: string }) {
  const digits = value.replace(/\D/g, "");
  return (
    <span className="tin-comb-1702mx">
      <CombValue value={digits.slice(0, 3)} cells={3} />
      <i>-</i>
      <CombValue value={digits.slice(3, 6)} cells={3} />
      <i>-</i>
      <CombValue value={digits.slice(6, 9)} cells={3} />
      <i>-</i>
      <CombValue value={digits.slice(9, 14)} cells={5} />
    </span>
  );
}

function LabeledCombLines1702MX({ number, label, value, cells, lines, compact = false }: { number: string; label: ReactNode; value: string; cells: number; lines: number; compact?: boolean }) {
  const characters = Array.from(value);
  const baseLineCapacity = Math.floor(cells / lines);
  const remainder = cells % lines;
  const lineCapacities = Array.from(
    { length: lines },
    (_, index) => baseLineCapacity + (index < remainder ? 1 : 0)
  );
  let characterOffset = 0;
  const usesPlainBox = characters.length > cells;
  return (
    <div
      className={`labeled-comb-1702mx${compact ? " compact-field-1702mx" : ""}`}
      data-item-number={number}
      data-field-mode={usesPlainBox ? "plain" : "guided"}
      data-cell-capacity={cells}
      data-line-count={lines}
      data-line-cell-capacity={lineCapacities.join(",")}
    >
      <span><b>{number}</b> {label}</span>
      <span className="comb-lines-1702mx" style={{ gridTemplateRows: `repeat(${lines}, 1fr)` }}>
        {usesPlainBox ? (
          <AdaptivePlainValue
            value={value}
            cellCapacity={cells}
            className="plain-lines-value-1702mx"
            maxFontSizePx={9.333}
            minFontSizePx={5.333}
            fontStepPx={.5}
          />
        ) : lineCapacities.map((lineCapacity, index) => {
          const lineValue = characters
            .slice(characterOffset, characterOffset + lineCapacity)
            .join("");
          characterOffset += lineCapacity;
          return <CombValue key={index} value={lineValue} cells={lineCapacity} />;
        })}
      </span>
    </div>
  );
}

function AmountRow1702MX({ number, label, value, className = "" }: { number: number; label: ReactNode; value: number | null; className?: string }) {
  return <div className={`amount-row-1702mx ${className}`}><span><b>{number}</b> {label}</span><AmountCell1702MX value={value} /></div>;
}

function AmountCell1702MX({ value }: { value: number | null }) {
  return <span className={value === null ? "amount-cell-1702mx blank-amount-1702mx" : "amount-cell-1702mx"}>{value === null ? "" : formatWhole(value)}</span>;
}

function PlainValue1702MX({ value, fixedFontSizePt, className = "" }: { value: string; fixedFontSizePt?: number; className?: string }) {
  const fixedFontSizePx = fixedFontSizePt === undefined
    ? undefined
    : fixedFontSizePt * 4 / 3;
  return (
    <AdaptivePlainValue
      value={value}
      className={`plain-value-1702mx${className ? ` ${className}` : ""}`}
      maxFontSizePx={fixedFontSizePx ?? 9.333}
      minFontSizePx={fixedFontSizePx ?? 5.333}
      fontStepPx={.5}
    />
  );
}

function scheduleThreeLabel(envelope: RenderEnvelope, index: number, label: ReactNode): ReactNode {
  if (index === 10) {
    return <>Other Tax Credits/Payments: <PlainValue1702MX value={text(envelope, "schedule_3_item_30_description")} /></>;
  }
  if (index === 11) {
    return <>Other Tax Credits/Payments: <PlainValue1702MX value={text(envelope, "schedule_3_item_31_description")} /></>;
  }
  return label;
}

function amount(envelope: RenderEnvelope, key: string): number | null {
  const value: RenderValue | undefined = envelope.fields[key];
  if (!value) return null;
  if (value.type === "integer" || value.type === "decimal") return value.value;
  throw new Error(`Expected numeric Rust-owned 1702MX field ${key}`);
}

function formatWhole(value: number): string {
  if (!Number.isFinite(value)) throw new Error("1702MX amount must be finite");
  return Math.trunc(Object.is(value, -0) ? 0 : value).toLocaleString("en-US");
}

function formatPercent(value: number | null): string {
  return value === null ? "" : `${value.toFixed(2)}%`;
}
