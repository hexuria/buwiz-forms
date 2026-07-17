import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import type { ReactNode } from "react";
import {
  AdaptivePlainValue,
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
    <>Total Penalties <em>(Sum of Items 18A to 18C)</em></>,
    "item_18d_total_penalties"
  ],
  [
    "19",
    <>Total Amount of Remittance <em>(Sum of Item 17 and 18D)</em></>,
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
          <b>*NOTE:</b> Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)
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
          <span>Republic of the Philippines</span>
          <span>Department of Finance</span>
          <span>Bureau of Internal Revenue</span>
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
        label={<><b>1</b> For the month of <em>(MM/YYYY)</em></>}
        valueClassName="month-value-0619f"
      >
        <GuidedValue0619F field="1-month" value={monthYear} segments={[2, 4]} />
      </HeaderOption0619F>
      <HeaderOption0619F
        label={<><b>2</b> Due Date <em>(MM/DD/YYYY)</em></>}
        valueClassName="due-date-value-0619f"
      >
        <GuidedValue0619F field="2-due-date" value={text(envelope, "due_date").replace(/\D/g, "")} segments={[2, 2, 4]} />
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
        <GuidedValue0619F field="5-tax-type-code" value={text(envelope, "tax_type_code")} segments={[2]} />
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
  const tin = envelope.taxpayer.tin.replace(/\D/g, "");
  return (
    <section className="part-0619f background-0619f">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-0619f">
        <div><b>6</b> Taxpayer Identification Number (TIN)</div>
        <Tin0619F value={tin} />
        <div><b>7</b> RDO Code</div>
        <GuidedValue0619F field="7-rdo" value={envelope.taxpayer.rdo_code} segments={[3]} />
      </div>
      <LabelValue0619F
        number="8"
        label={<>Withholding Agent’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></>}
        value={envelope.taxpayer.name.toUpperCase()}
        field="8-agent-name"
        segments={[40]}
        className="name-0619f"
      />
      <div className="address-0619f">
        <div className="label-0619f">
          <b>9</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em>
        </div>
        <GuidedValue0619F field="9-address-line-1" value={envelope.taxpayer.registered_address.toUpperCase()} segments={[40]} />
        <div className="address-second-0619f">
          <GuidedValue0619F field="9-address-line-2" value={text(envelope, "registered_address_2").toUpperCase()} segments={[31]} />
          <span><b>9A</b> ZIP Code</span>
          <GuidedValue0619F field="9a-zip" value={envelope.taxpayer.zip_code} segments={[4]} />
        </div>
      </div>
      <div className="contact-category-0619f">
        <div><b>10</b> Contact Number</div>
        <GuidedValue0619F field="10-contact" value={envelope.taxpayer.contact_number.replace(/\D/g, "")} segments={[12]} />
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
        field="12-email"
        segments={[40]}
        className="email-0619f"
      />
    </section>
  );
}

function Tin0619F({ value }: { value: string }) {
  if (Array.from(value).length > 14) {
    return (
      <PlainValue0619F
        field="6-tin"
        value={value}
        className="tin-overflow-plain-0619f"
      />
    );
  }

  return (
    <span
      className="tin-value-0619f"
      data-official-field="6-tin"
      data-field-mode="guided"
      data-guide-count="10"
      data-guide-segments="3-3-3-5"
      aria-label={value}
    >
      {[3, 3, 3, 5].map((segment, index) => {
        const offset = index === 0 ? 0 : index === 1 ? 3 : index === 2 ? 6 : 9;
        return (
          <span className="guided-segment-0619f" key={offset}>
            <CombValue value={value.slice(offset, offset + segment)} cells={segment} />
            {index < 3 && <i aria-hidden="true">-</i>}
          </span>
        );
      })}
    </span>
  );
}

function PlainValue0619F({
  field,
  value,
  align = "left",
  className = ""
}: {
  field: string;
  value: string;
  align?: "left" | "right";
  className?: string;
}) {
  return (
    <span
      className={`plain-field-shell-0619f ${className}`.trim()}
      data-official-field={field}
      data-field-mode="plain"
      data-guide-count="0"
    >
      <AdaptivePlainValue
        value={value}
        align={align}
        className="plain-field-value-0619f"
        maxFontSizePx={10}
        minFontSizePx={6}
        fontStepPx={0.5}
      />
    </span>
  );
}

function GuidedValue0619F({
  field,
  value,
  segments,
  align = "left",
  className = ""
}: {
  field: string;
  value: string;
  segments: readonly number[];
  align?: "left" | "right";
  className?: string;
}) {
  const characters = Array.from(value);
  const capacity = segments.reduce((total, segment) => total + segment, 0);
  if (characters.length > capacity) {
    return (
      <PlainValue0619F
        field={field}
        value={value}
        align={align}
        className={`guided-overflow-plain-0619f ${className}`.trim()}
      />
    );
  }

  let offset = 0;
  const guideCount = segments.reduce((total, segment) => total + segment - 1, 0);
  return (
    <span
      className={`guided-field-0619f ${className}`.trim()}
      data-official-field={field}
      data-field-mode="guided"
      data-guide-count={guideCount}
      data-guide-segments={segments.join("-")}
      aria-label={value}
      style={{ gridTemplateColumns: segments.map((segment) => `${segment}fr`).join(" ") }}
    >
      {segments.map((segment, index) => {
        const segmentValue = characters.slice(offset, offset + segment).join("");
        offset += segment;
        return (
          <span className="guided-segment-0619f" key={`${field}-${index}`}>
            <CombValue value={segmentValue} cells={segment} align={align} />
          </span>
        );
      })}
    </span>
  );
}

function LabelValue0619F({
  number,
  label,
  value,
  field,
  segments,
  className
}: {
  number: string;
  label: ReactNode;
  value: string;
  field: string;
  segments: readonly number[];
  className: string;
}) {
  return (
    <div className={`label-value-0619f ${className}`}>
      <div className="label-0619f"><b>{number}</b> {label}</div>
      <GuidedValue0619F field={field} value={value} segments={segments} />
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
          <span>
            <b>{number}</b>
            <strong
              data-official-field={`${number}-atc`}
              data-field-mode="plain"
              data-guide-count="0"
            >
              {text(envelope, atcKey)}
            </strong>
          </span>
          <span>{label}</span>
          <MoneyComb0619F field={`${number.toLowerCase()}-amount`} value={decimal(envelope, key)} />
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
            <MoneyComb0619F field={`${number.toLowerCase()}-amount`} value={decimal(envelope, key)} />
          </div>
        </div>
      ))}
    </section>
  );
}

function MoneyComb0619F({ field, value }: { field: string; value: number | null }) {
  const [whole, fraction] = value === null ? ["", ""] : formatMoneyParts(value);
  if (value !== null && Array.from(whole).length > 11) {
    return (
      <PlainValue0619F
        field={field}
        value={`${whole}.${fraction}`}
        align="right"
        className="money-overflow-0619f"
      />
    );
  }
  return (
    <span
      className="money-0619f"
      data-official-field={field}
      data-field-mode="guided"
      data-guide-count="11"
      data-guide-segments="11-2"
      aria-label={value === null ? "" : `${whole}.${fraction}`}
    >
      <span className="money-whole-0619f guided-segment-0619f">
        <CombValue value={whole} cells={11} align="right" />
      </span>
      <span className="decimal-separator-0619f">•</span>
      <span className="money-fraction-0619f guided-segment-0619f">
        <CombValue value={fraction} cells={2} align="right" />
      </span>
    </span>
  );
}

function Declaration0619F({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-0619f">
      <p>
        I/We declare under the penalties of perjury that this remittance form has been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter)</em>
      </p>
      <div className="signature-body-0619f">
        <div><span>For Individual:</span><b>Signature over Printed Name of Taxpayer/Authorized Representative/ Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
        <div><span>For Non-Individual:</span><b>Signature over Printed Name of President/Vice President/<br />Authorized Officer or Representative/Tax Agent</b><em>(Indicate Title/Designation and TIN)</em></div>
      </div>
      <div className="signature-footer-0619f">
        <span>Tax Agent Accreditation No./<br />Attorney’s Roll No. <em>(if applicable)</em></span>
        <PlainValue0619F field="tax-agent-accreditation" value={text(envelope, "tax_agent_accreditation_number")} />
        <span>Date of Issue<br /><em>(MM/DD/YYYY)</em></span>
        <PlainValue0619F field="tax-agent-date-of-issue" value={text(envelope, "tax_agent_date_of_issue")} />
        <span>Date of Expiry<br /><em>(MM/DD/YYYY)</em></span>
        <PlainValue0619F field="tax-agent-date-of-expiry" value={text(envelope, "tax_agent_date_of_expiry")} />
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
      <div
        className="machine-validation-0619f"
        data-official-field="machine-validation"
        data-field-mode="plain"
        data-guide-count="0"
      >
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
          <PlainValue0619F
            field="22-bank"
            value={draweeBank}
            className="payment-tax-debit-bank-0619f"
          />
        </span>
        <GuidedValue0619F field="22-number" value={text(envelope, `${prefix}_number`).toUpperCase()} segments={[6]} />
        <GuidedValue0619F field="22-date" value={text(envelope, `${prefix}_date`).replace(/\D/g, "")} segments={[2, 2, 4]} />
        <MoneyComb0619F field="22-amount" value={amount} />
      </div>
    );
  }

  return (
    <div className="payment-row-0619f" data-payment-row={prefix}>
      {number ? (
        <span><b>{number}</b> {label}</span>
      ) : (
        <span className="payment-particular-field-0619f">
          <GuidedValue0619F
            field="23-particular"
            value={text(envelope, "payment_23_particular").toUpperCase()}
            segments={[7]}
          />
        </span>
      )}
      <GuidedValue0619F field={`${number ?? "23"}-bank`} value={draweeBank} segments={[5]} />
      <GuidedValue0619F field={`${number ?? "23"}-number`} value={text(envelope, `${prefix}_number`).toUpperCase()} segments={[6]} />
      <GuidedValue0619F field={`${number ?? "23"}-date`} value={text(envelope, `${prefix}_date`).replace(/\D/g, "")} segments={[2, 2, 4]} />
      <MoneyComb0619F field={`${number ?? "23"}-amount`} value={amount} />
    </div>
  );
}
