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
  OFFICIAL_0619F_PDF417_PAGE_ONE_PATH,
  OFFICIAL_0619F_PDF417_PAYLOAD,
  OFFICIAL_0619F_SEAL
} from "./official0619FAssets";
import "./Form0619F.css";

const FINAL_TAX_LINES = [
  [
    "13",
    "item_13_atc",
    "Remittance of Final Income Taxes Withheld on Interest Paid on Deposits and Yield on Deposit Substitutes/Trusts/Etc.",
    "item_13_interest_final_tax_withheld"
  ],
  [
    "14",
    "item_14_atc",
    "Remittance of Final Income Taxes Withheld on Other Final Income Taxes",
    "item_14_other_final_tax_withheld"
  ]
] as const;

const REMITTANCE_LINES = [
  ["15", "Total (Sum of Items 13 and 14)", "item_15_total"],
  [
    "16",
    "Less: Amount Remitted from Previously Filed Form, if this is an amended form",
    "item_16_remitted_previously"
  ],
  ["17", "Net Amount of Remittance (Item 15 Less Item 16)", "item_17_net_amount_of_remittance"],
  ["18A", "Surcharge", "item_18a_surcharge"],
  ["18B", "Interest", "item_18b_interest"],
  ["18C", "Compromise", "item_18c_compromise"],
  [
    "18D",
    "Total Penalties (Sum of Items 18A to 18C)",
    "item_18d_total_penalties"
  ],
  [
    "19",
    "Total Amount of Remittance (Sum of Item 17 and 18D)",
    "item_19_total_amount_of_remittance"
  ]
] as const;

const PAYMENT_ROWS = [
  ["20", "Cash/Bank Debit Memo", "payment_20"],
  ["21", "Check", "payment_21"],
  ["22", "Tax Debit Memo", "payment_22"],
  ["23", "Others", "payment_23"]
] as const;

export function Form0619F({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("0619F", "2018");
  if (envelope.schedules.length !== 0) {
    throw new Error("0619F does not accept repeatable renderer schedules");
  }

  return (
    <main className="form-document" data-form-code="0619F">
      <FolioPage pageNumber={1} paper="letter" className="form-0619f-page-one">
        <GovernmentHeader0619F />
        <Masthead0619F />
        <HeaderOptions0619F envelope={envelope} />
        <BackgroundInformation0619F envelope={envelope} />
        <TaxRemittance0619F envelope={envelope} />
        <Declaration0619F envelope={envelope} />
        <PaymentDetails0619F envelope={envelope} />
        <p className="privacy-note-0619f">
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

function GovernmentHeader0619F() {
  return (
    <header className="government-header-0619f">
      <span className="bir-only-0619f">For BIR<br />Use Only</span>
      <span className="bcs-0619f">BCS/<br />Item:</span>
      <span className="government-wordmark-0619f">
        <img src={OFFICIAL_0619F_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>
          Republic of the Philippines<br />
          Department of Finance<br />
          Bureau of Internal Revenue
        </span>
      </span>
    </header>
  );
}

function Masthead0619F() {
  return (
    <header className="masthead-0619f">
      <div className="form-number-0619f">
        <span>BIR Form No.</span>
        <strong>0619-F</strong>
        <small>January 2018</small>
        <b>Page 1</b>
      </div>
      <div className="form-title-0619f">
        <strong>Monthly Remittance Form</strong>
        <span>of Final Income Taxes Withheld</span>
        <em>
          Enter all <i>required information</i> in <i>CAPITAL LETTERS</i> using <i>BLACK ink</i>. Mark all
          applicable boxes with an “X”.<br />Two copies MUST be filed with the BIR and one held by the Taxpayer.
        </em>
      </div>
      <div className="barcode-0619f" aria-label={OFFICIAL_0619F_PDF417_PAYLOAD}>
        <span className="official-pdf417-object-0619f" aria-hidden="true">
          <svg
            className="official-pdf417-symbol-0619f"
            viewBox="0 0 120 7"
            preserveAspectRatio="none"
            shapeRendering="crispEdges"
            focusable="false"
          >
            <path d={OFFICIAL_0619F_PDF417_PAGE_ONE_PATH} />
          </svg>
        </span>
        <small>{OFFICIAL_0619F_PDF417_PAYLOAD}</small>
      </div>
    </header>
  );
}

function HeaderOptions0619F({ envelope }: { envelope: RenderEnvelope }) {
  const monthYear = `${String(envelope.period.month).padStart(2, "0")}${String(envelope.period.taxable_year).padStart(4, "0")}`;
  return (
    <section className="header-options-0619f" aria-label="Return period and filing options">
      <HeaderOption0619F
        label={<><b>1</b> For the Month of <em>(MM/YYYY)</em></>}
        valueClassName="month-value-0619f"
      >
        <CombValue value={monthYear} cells={6} align="right" />
      </HeaderOption0619F>
      <HeaderOption0619F
        label={<><b>2</b> Due Date <em>(MM/DD/YYYY)</em></>}
        valueClassName="due-date-value-0619f"
      >
        <AdaptiveCombValue value={text(envelope, "due_date").replace(/\D/g, "")} cells={8} align="right" />
      </HeaderOption0619F>
      <HeaderOption0619F label={<><b>3</b> Amended Form?</>}>
        <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "is_amended")} label="No" />
      </HeaderOption0619F>
      <HeaderOption0619F label={<><b>4</b> Any Taxes Withheld?</>}>
        <CheckChoice checked={bool(envelope, "any_taxes_withheld")} label="Yes" />
        <CheckChoice checked={!bool(envelope, "any_taxes_withheld")} label="No" />
      </HeaderOption0619F>
      <HeaderOption0619F label={<><b>5</b> Tax Type Code**</>} valueClassName="code-value-0619f">
        <CombValue value={text(envelope, "tax_type_code")} cells={2} />
      </HeaderOption0619F>
    </section>
  );
}

function HeaderOption0619F({
  label,
  children,
  valueClassName = ""
}: {
  label: ReactNode;
  children: ReactNode;
  valueClassName?: string;
}) {
  return (
    <div className="header-option-0619f">
      <div>{label}</div>
      <span className={valueClassName}>{children}</span>
    </div>
  );
}

function BackgroundInformation0619F({ envelope }: { envelope: RenderEnvelope }) {
  const category = text(envelope, "withholding_agent_category");
  const tin = envelope.taxpayer.tin.replace(/\D/g, "").padEnd(14);
  return (
    <section className="part-0619f background-0619f">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-0619f">
        <div><b>6</b> Taxpayer Identification Number (TIN)</div>
        <Tin0619F value={tin} />
        <div><b>7</b> RDO Code</div>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <LabelValue0619F
        number="8"
        label={<>Withholding Agent’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></>}
        value={envelope.taxpayer.name.toUpperCase()}
        cells={40}
        className="name-0619f"
      />
      <div className="address-0619f">
        <div className="label-0619f">
          <b>9</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em>
        </div>
        <AdaptiveCombValue value={envelope.taxpayer.registered_address.toUpperCase()} cells={40} />
        <div className="address-second-0619f">
          <AdaptiveCombValue value={text(envelope, "registered_address_2").toUpperCase()} cells={31} />
          <span><b>9A</b> ZIP Code</span>
          <CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" />
        </div>
      </div>
      <div className="contact-category-0619f">
        <div><b>10</b> Contact Number</div>
        <AdaptiveCombValue value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} />
        <div><b>11</b> Category of Withholding Agent</div>
        <span className="category-choices-0619f">
          <CheckChoice checked={category === "private"} label="Private" />
          <CheckChoice checked={category === "government"} label="Government" />
        </span>
      </div>
      <LabelValue0619F
        number="12"
        label="Email Address"
        value={envelope.taxpayer.email.toUpperCase()}
        cells={40}
        className="email-0619f"
      />
    </section>
  );
}

function Tin0619F({ value }: { value: string }) {
  return (
    <span className="tin-value-0619f">
      <CombValue value={value.slice(0, 3)} cells={3} />
      <i>-</i>
      <CombValue value={value.slice(3, 6)} cells={3} />
      <i>-</i>
      <CombValue value={value.slice(6, 9)} cells={3} />
      <i>-</i>
      <CombValue value={value.slice(9, 14)} cells={5} />
    </span>
  );
}

function LabelValue0619F({
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
    <div className={`label-value-0619f ${className}`}>
      <div className="label-0619f"><b>{number}</b> {label}</div>
      <AdaptiveCombValue value={value} cells={cells} />
    </div>
  );
}

function TaxRemittance0619F({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-0619f remittance-0619f">
      <h2>Part II – Tax Remittance</h2>
      <div className="remittance-head-0619f">
        <span>ATC</span><span>Description</span><span>Amount for Remittance</span>
      </div>
      {FINAL_TAX_LINES.map(([number, atcKey, label, key]) => (
        <div className={`final-tax-row-0619f item-${number}-0619f`} data-item={number} key={number}>
          <span><b>{number}</b><strong>{text(envelope, atcKey)}</strong></span>
          <span>{label}</span>
          <MoneyComb0619F value={decimal(envelope, key)} />
        </div>
      ))}
      {REMITTANCE_LINES.map(([number, label, key], index) => (
        <div key={number}>
          {index === 3 && <div className="penalties-heading-0619f"><b>18</b> Add: Penalties</div>}
          <div
            className={`remittance-row-0619f item-${number.toLowerCase()}-0619f ${["15", "17", "18D", "19"].includes(number) ? "computed" : ""}`}
            data-item={number}
          >
            <div><b>{number}</b><span>{label}</span></div>
            <MoneyComb0619F value={decimal(envelope, key)} />
          </div>
        </div>
      ))}
    </section>
  );
}

function MoneyComb0619F({ value }: { value: number | null }) {
  if (value === null) {
    return (
      <span className="money-0619f">
        <CombValue value="" cells={11} />
        <span className="decimal-separator-0619f">•</span>
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
        className="money-overflow-0619f"
      />
    );
  }
  return (
    <span className="money-0619f">
      <CombValue value={whole} cells={11} align="right" />
      <span className="decimal-separator-0619f">•</span>
      <CombValue value={fraction} cells={2} align="right" />
    </span>
  );
}

function Declaration0619F({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-0619f">
      <p>
        I/We declare under the penalties of perjury that this remittance form has been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give my/our consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter)</em>
      </p>
      <div className="signature-body-0619f">
        <div><span>For Individual:</span><b>Signature over Printed Name of Taxpayer/Authorized Representative/ Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
        <div><span>For Non-Individual:</span><b>Signature over Printed Name of President/Vice President/<br />Authorized Officer or Representative/Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
      </div>
      <div className="signature-footer-0619f">
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

function PaymentDetails0619F({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="payment-0619f">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-head-0619f">
        <span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>Date <em>(MM/DD/YYYY)</em></span><span>Amount</span>
      </div>
      {PAYMENT_ROWS.slice(0, 3).map(([number, label, prefix]) => (
        <PaymentRow0619F key={number} envelope={envelope} number={number} label={label} prefix={prefix} />
      ))}
      <div className="payment-other-label-0619f"><b>23</b> Others (specify below)</div>
      <PaymentRow0619F envelope={envelope} prefix="payment_23" />
      <div className="machine-validation-0619f">
        <span>Machine Validation/Revenue Official Receipt Details <small>(if not filed with an Authorized Agent Bank)</small></span>
        <span>Stamp of Receiving Office/AAB and Date of Receipt<br /><em>(RO’s Signature/Bank Teller’s Initial)</em></span>
      </div>
    </section>
  );
}

function PaymentRow0619F({
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
  const draweeBank = text(envelope, `${prefix}_drawee_bank_or_agency`).toUpperCase();

  if (number === "22") {
    return (
      <div
        className="payment-row-0619f payment-tax-debit-row-0619f"
        data-payment-row={prefix}
      >
        <span>
          <b>{number}</b> {label}
          {draweeBank && (
            <small className="payment-tax-debit-bank-0619f">{draweeBank}</small>
          )}
        </span>
        <AdaptiveCombValue value={text(envelope, `${prefix}_number`).toUpperCase()} cells={6} />
        <AdaptiveCombValue value={text(envelope, `${prefix}_date`).replace(/\D/g, "")} cells={8} />
        <MoneyComb0619F value={amount} />
      </div>
    );
  }

  return (
    <div className="payment-row-0619f" data-payment-row={prefix}>
      {number ? (
        <span><b>{number}</b> {label}</span>
      ) : (
        <span className="payment-particular-field-0619f">
          <AdaptiveCombValue
            value={text(envelope, "payment_23_particular").toUpperCase()}
            cells={7}
          />
        </span>
      )}
      <AdaptiveCombValue value={draweeBank} cells={5} />
      <AdaptiveCombValue value={text(envelope, `${prefix}_number`).toUpperCase()} cells={6} />
      <AdaptiveCombValue value={text(envelope, `${prefix}_date`).replace(/\D/g, "")} cells={8} />
      <MoneyComb0619F value={amount} />
    </div>
  );
}
