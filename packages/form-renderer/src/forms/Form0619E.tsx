import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import type { ReactNode } from "react";
import {
  AdaptiveCombValue,
  CheckChoice,
  CombValue,
  FolioPage,
  ValidationSummary,
  formatMoneyParts
} from "../components";
import { bool, decimal, text } from "../values";
import {
  OFFICIAL_0619E_BARCODE_PAGE_ONE,
  OFFICIAL_0619E_SEAL
} from "./official0619EAssets";
import "./Form0619E.css";

const REMITTANCE_LINES = [
  ["14", "Amount of Remittance", "item_14_amount_of_remittance"],
  [
    "15",
    "Less: Amount Remitted from Previously Filed Form, if this is an amended form",
    "item_15_amount_remitted_previously"
  ],
  [
    "16",
    "Net Amount of Remittance (Item 14 Less Item 15)",
    "item_16_net_amount_of_remittance"
  ],
  ["17A", "Surcharge", "item_17a_surcharge"],
  ["17B", "Interest", "item_17b_interest"],
  ["17C", "Compromise", "item_17c_compromise"],
  [
    "17D",
    "Total Penalties (Sum of Items 17A to 17C)",
    "item_17d_total_penalties"
  ],
  [
    "18",
    "Total Amount of Remittance (Sum of Items 16 and 17D)",
    "item_18_total_amount_of_remittance"
  ]
] as const;

const PAYMENT_ROWS = [
  ["19", "Cash/Bank Debit Memo", "payment_19"],
  ["20", "Check", "payment_20"],
  ["21", "Tax Debit Memo", "payment_21"],
  ["22", "Others", "payment_22"]
] as const;

export function Form0619E({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("0619E", "2018");
  if (envelope.schedules.length !== 0) {
    throw new Error("0619E does not accept repeatable renderer schedules");
  }

  return (
    <main className="form-document" data-form-code="0619E">
      <FolioPage pageNumber={1} paper="letter" className="form-0619e-page-one">
        <GovernmentHeader0619E />
        <Masthead0619E />
        <HeaderOptions0619E envelope={envelope} />
        <BackgroundInformation0619E envelope={envelope} />
        <TaxRemittance0619E envelope={envelope} />
        <Declaration0619E envelope={envelope} />
        <PaymentDetails0619E envelope={envelope} />
        <p className="privacy-note-0619e">
          *NOTE: Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)
        </p>
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function GovernmentHeader0619E() {
  return (
    <header className="government-header-0619e">
      <span className="bir-only-0619e">For BIR<br />Use Only</span>
      <span className="bcs-0619e">BCS/<br />Item:</span>
      <span className="government-wordmark-0619e">
        <img src={OFFICIAL_0619E_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>
          Republic of the Philippines<br />
          Department of Finance<br />
          Bureau of Internal Revenue
        </span>
      </span>
    </header>
  );
}

function Masthead0619E() {
  return (
    <header className="masthead-0619e">
      <div className="form-number-0619e">
        <span>BIR Form No.</span>
        <strong>0619-E</strong>
        <small>January 2018</small>
        <b>Page 1</b>
      </div>
      <div className="form-title-0619e">
        <strong>Monthly Remittance Form</strong>
        <span>of Creditable Income Taxes Withheld (Expanded)</span>
        <em>
          Enter all <i>required information</i> in <i>CAPITAL LETTERS</i> using <i>BLACK ink</i>. Mark all
          applicable boxes with an “X”.<br />Two copies MUST be filed with the BIR and one held by the Taxpayer.
        </em>
      </div>
      <div className="barcode-0619e" aria-label="0619-E 01/18 P1">
        <img src={OFFICIAL_0619E_BARCODE_PAGE_ONE} alt="" aria-hidden="true" />
      </div>
    </header>
  );
}

function HeaderOptions0619E({ envelope }: { envelope: RenderEnvelope }) {
  const monthYear = `${String(envelope.period.month).padStart(2, "0")}${String(envelope.period.taxable_year).padStart(4, "0")}`;
  return (
    <section className="header-options-0619e" aria-label="Return period and filing options">
      <HeaderOption0619E label={<><b>1</b> For the Month of <em>(MM/YYYY)</em></>}>
        <CombValue value={monthYear} cells={6} align="right" />
      </HeaderOption0619E>
      <HeaderOption0619E label={<><b>2</b> Due Date <em>(MM/DD/YYYY)</em></>}>
        <AdaptiveCombValue value={text(envelope, "due_date").replace(/\D/g, "")} cells={8} align="right" />
      </HeaderOption0619E>
      <HeaderOption0619E label={<><b>3</b> Amended Form?</>}>
        <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "is_amended")} label="No" />
      </HeaderOption0619E>
      <HeaderOption0619E label={<><b>4</b> Any Taxes Withheld?</>}>
        <CheckChoice checked={bool(envelope, "any_taxes_withheld")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "any_taxes_withheld")} label="No" />
      </HeaderOption0619E>
      <HeaderOption0619E label={<><b>5</b> ATC</>} valueClassName="code-value-0619e">
        {text(envelope, "atc")}
      </HeaderOption0619E>
      <HeaderOption0619E label={<><b>6</b> Tax Type Code</>} valueClassName="code-value-0619e">
        {text(envelope, "tax_type_code")}
      </HeaderOption0619E>
    </section>
  );
}

function HeaderOption0619E({
  label,
  children,
  valueClassName = ""
}: {
  label: ReactNode;
  children: ReactNode;
  valueClassName?: string;
}) {
  return (
    <div className="header-option-0619e">
      <div>{label}</div>
      <span className={valueClassName}>{children}</span>
    </div>
  );
}

function BackgroundInformation0619E({ envelope }: { envelope: RenderEnvelope }) {
  const category = text(envelope, "withholding_agent_category");
  const tinSegments = [
    printableTinSegment0619E(envelope, "printable_tin_segment_1", 3),
    printableTinSegment0619E(envelope, "printable_tin_segment_2", 3),
    printableTinSegment0619E(envelope, "printable_tin_segment_3", 3),
    printableTinSegment0619E(envelope, "printable_tin_branch", 5)
  ] as const;
  return (
    <section className="part-0619e background-0619e">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-0619e">
        <div><b>7</b> Taxpayer Identification Number (TIN)</div>
        <Tin0619E segments={tinSegments} />
        <div><b>8</b> RDO Code</div>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <LabelValue0619E
        number="9"
        label={<>Withholding Agent’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></>}
        value={envelope.taxpayer.name.toUpperCase()}
        cells={40}
        className="name-0619e"
      />
      <div className="address-0619e">
        <div className="label-0619e">
          <b>10</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em>
        </div>
        <AdaptiveCombValue value={envelope.taxpayer.registered_address.toUpperCase()} cells={40} />
        <div className="address-second-0619e">
          <AdaptiveCombValue value={text(envelope, "registered_address_2").toUpperCase()} cells={31} />
          <span><b>10A</b> ZIP Code</span>
          <CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" />
        </div>
      </div>
      <div className="contact-category-0619e">
        <div><b>11</b> Contact Number</div>
        <AdaptiveCombValue value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} />
        <div><b>12</b> Category of Withholding Agent</div>
        <span className="category-choices-0619e">
          <CheckChoice checked={category === "private"} label="Private" />
          <CheckChoice checked={category === "government"} label="Government" />
        </span>
      </div>
      <LabelValue0619E
        number="13"
        label="Email Address"
        value={envelope.taxpayer.email.toUpperCase()}
        cells={40}
        className="email-0619e"
      />
    </section>
  );
}

function printableTinSegment0619E(
  envelope: RenderEnvelope,
  key: string,
  digitCount: 3 | 5
) {
  const value = text(envelope, key);
  if (value.length !== digitCount || !/^\d+$/.test(value)) {
    throw new Error(
      `0619E ${key} must contain exactly ${digitCount} Rust-normalized digits`
    );
  }
  return value;
}

function Tin0619E({
  segments
}: {
  segments: readonly [string, string, string, string];
}) {
  const [segment1, segment2, segment3, branch] = segments;
  return (
    <span
      className="tin-value-0619e"
      aria-label={`${segment1}-${segment2}-${segment3}-${branch}`}
    >
      <CombValue value={segment1} cells={3} />
      <i>-</i>
      <CombValue value={segment2} cells={3} />
      <i>-</i>
      <CombValue value={segment3} cells={3} />
      <i>-</i>
      <CombValue value={branch} cells={5} />
    </span>
  );
}

function LabelValue0619E({
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
    <div className={`label-value-0619e ${className}`}>
      <div className="label-0619e"><b>{number}</b> {label}</div>
      <AdaptiveCombValue value={value} cells={cells} />
    </div>
  );
}

function TaxRemittance0619E({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-0619e remittance-0619e">
      <h2>Part II – Tax Remittance</h2>
      {REMITTANCE_LINES.map(([number, label, key], index) => (
        <div key={number}>
          {index === 3 && <div className="penalties-heading-0619e"><b>17</b> Add: Penalties</div>}
          <div
            className={`remittance-row-0619e item-${number.toLowerCase()}-0619e ${["16", "17D", "18"].includes(number) ? "computed" : ""}`}
            data-item={number}
          >
            <div><b>{number}</b><span>{label}</span></div>
            <MoneyComb0619E value={decimal(envelope, key)} />
          </div>
        </div>
      ))}
    </section>
  );
}

function MoneyComb0619E({ value }: { value: number | null }) {
  if (value === null) {
    return (
      <span className="money-0619e">
        <CombValue value="" cells={11} />
        <span className="decimal-separator-0619e">•</span>
        <CombValue value="" cells={2} />
      </span>
    );
  }
  const [whole, fraction] = formatMoneyParts(value);
  if (Array.from(whole).length > 11) {
    return (
      <AdaptiveCombValue
        value={`${whole}.${fraction}`}
        cells={14}
        align="right"
        className="money-overflow-0619e"
      />
    );
  }
  return (
    <span className="money-0619e">
      <CombValue value={whole} cells={11} align="right" />
      <span className="decimal-separator-0619e">•</span>
      <CombValue value={fraction} cells={2} align="right" />
    </span>
  );
}

function Declaration0619E({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-0619e">
      <p>
        I/We declare under the penalties of perjury that this remittance form has been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give my/our consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter)</em>
      </p>
      <div className="signature-body-0619e">
        <div><span>For Individual:</span><b>Signature over Printed Name of Taxpayer/Authorized Representative/ Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
        <div><span>For Non-Individual:</span><b>Signature over Printed Name of President/Vice President/<br />Authorized Officer or Representative/Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
      </div>
      <div className="signature-footer-0619e">
        <span>Tax Agent Accreditation No./<br />Attorney’s Roll No. <em>(if applicable)</em></span>
        <AdaptiveCombValue value={text(envelope, "tax_agent_accreditation_number")} cells={18} />
        <span>Date of Issue<br /><em>(MM/DD/YYYY)</em></span>
        <AdaptiveCombValue value={text(envelope, "tax_agent_date_of_issue").replace(/\D/g, "")} cells={8} />
        <span>Date of Expiry<br /><em>(MM/DD/YYYY)</em></span>
        <AdaptiveCombValue value={text(envelope, "tax_agent_date_of_expiry").replace(/\D/g, "")} cells={8} />
      </div>
    </section>
  );
}

function PaymentDetails0619E({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="payment-0619e">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-head-0619e">
        <span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date <em>(MM/DD/YYYY)</em></span><span>Amount</span>
      </div>
      {PAYMENT_ROWS.slice(0, 3).map(([number, label, prefix]) => (
        <PaymentRow0619E key={number} envelope={envelope} number={number} label={label} prefix={prefix} />
      ))}
      <div className="payment-other-label-0619e"><b>22</b> Others (specify below)</div>
      <PaymentRow0619E envelope={envelope} prefix="payment_22" />
      <div className="machine-validation-0619e">
        <span>Machine Validation/Revenue Official Receipt Details <small>(if not filed with an Authorized Agent Bank)</small></span>
        <span>Stamp of Receiving Office/AAB and Date of Receipt<br /><em>(RO’s Signature/Bank Teller’s Initial)</em></span>
      </div>
    </section>
  );
}

function PaymentRow0619E({
  envelope,
  prefix,
  number,
  label
}: {
  envelope: RenderEnvelope;
  prefix: string;
  number?: string;
  label?: string;
}) {
  const amount = bool(envelope, `${prefix}_amount_present`)
    ? decimal(envelope, `${prefix}_amount`)
    : null;
  return (
    <div className="payment-row-0619e" data-payment-row={prefix}>
      <span>
        {number ? <><b>{number}</b> {label}</> : text(envelope, "payment_22_particular").toUpperCase()}
      </span>
      <AdaptiveCombValue value={text(envelope, `${prefix}_drawee_bank_or_agency`).toUpperCase()} cells={5} />
      <AdaptiveCombValue value={text(envelope, `${prefix}_number`).toUpperCase()} cells={6} />
      <AdaptiveCombValue value={text(envelope, `${prefix}_date`).replace(/\D/g, "")} cells={8} />
      <MoneyComb0619E value={amount} />
    </div>
  );
}
