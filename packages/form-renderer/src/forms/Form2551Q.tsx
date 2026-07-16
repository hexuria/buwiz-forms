import {
  FORM_2551Q_ATC_REFERENCE,
  type AtcReferenceEntry,
  type RenderEnvelope,
  type RenderRow
} from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import {
  AdaptiveCombValue,
  CheckChoice,
  CombValue,
  FolioPage,
  formatMoneyParts,
  ValidationSummary
} from "../components";
import { paginateSchedule, type SchedulePage } from "../pagination";
import { bool, cellDecimal, cellText, decimal, integer, text } from "../values";
import {
  OFFICIAL_2551Q_BARCODE,
  OFFICIAL_2551Q_PAGE_TWO_BARCODE,
  OFFICIAL_2551Q_SEAL
} from "./official2551QAssets";

type AtcReferenceRow =
  | {
      kind: "entry";
      atc: string;
      description: string;
      rate: string;
      section?: string;
      lines?: [string, string];
    }
  | {
      kind: "category";
      description: string;
      section: string;
    }
  | {
      kind: "note";
      lines: [string, string];
    };

const LENDING_NOTE: AtcReferenceRow = {
  kind: "note",
  lines: [
    "1) On interest, commissions and discounts from lending activities as well as income from financial leasing,",
    "on the basis of remaining maturities of instruments from which such receipts are derived"
  ]
};

// Categories and explanatory notes are page-two layout structure. Every legal
// ATC code, description, rate, and ordering comes from the Rust-generated
// registry imported above.
const ATC_ROWS_BEFORE: Readonly<Record<string, readonly AtcReferenceRow[]>> = {
  PT105: [
    {
      kind: "category",
      description: "Tax on Banks and Non-Bank Financial Intermediaries Performing Quasi-Banking Functions",
      section: "Sec. 121"
    },
    LENDING_NOTE
  ],
  PT113: [
    {
      kind: "category",
      description: "Tax on Other Non-Bank Financial Intermediaries not Performing Quasi-Banking Functions",
      section: "Sec. 122"
    },
    LENDING_NOTE
  ],
  PT130: [
    {
      kind: "category",
      description: "Agents of Foreign Insurance Companies",
      section: "Sec. 124"
    }
  ]
};

const ATC_REFERENCE: AtcReferenceRow[] = FORM_2551Q_ATC_REFERENCE.entries.flatMap(
  (entry) => [...(ATC_ROWS_BEFORE[entry.code] ?? []), atcReferenceRow(entry)]
);

function atcReferenceRow(entry: AtcReferenceEntry): AtcReferenceRow {
  const sectionMatch = /^(.*) \[(Sec\. [^\]]+)\]$/.exec(entry.description);
  const description = sectionMatch?.[1] ?? entry.description;
  const section = sectionMatch?.[2];
  const splitAt = entry.code === "PT150" ? description.indexOf(" boxes,") : -1;
  const lines: [string, string] | undefined = splitAt >= 0
    ? [description.slice(0, splitAt), description.slice(splitAt + 1)]
    : undefined;
  return {
    kind: "entry",
    atc: entry.code.replace(/^PT/, "PT "),
    description,
    rate: formatAtcRate(entry.rate),
    ...(section ? { section } : {}),
    ...(lines ? { lines } : {})
  };
}

export function formatAtcRate(rate: number): string {
  // JSON carries the Rust-owned rate as a decimal number. Normalize the
  // display value so binary floating-point artifacts (for example,
  // 0.07 * 100) cannot widen the fixed official tax-rate column.
  return `${Number((rate * 100).toFixed(6))}%`;
}

export function Form2551Q({ envelope }: { envelope: RenderEnvelope }) {
  const spec = getFormSpec("2551Q", "2018");
  const schedule = envelope.schedules.find((item) => item.id === "schedule_1");
  if (!schedule) throw new Error("2551Q requires schedule_1");
  const pages = paginateSchedule(schedule, spec.schedules.schedule_1);
  const pageTwoSubtotal = pages[0]?.summaryKind === "page_2_subtotal"
    ? requiredDecimal(envelope, "schedule_1_page_2_subtotal")
    : null;

  return (
    <main className="form-document" data-form-code="2551Q">
      <FolioPage pageNumber={1} className="form-2551q-page-one">
        <OfficialPageOne envelope={envelope} />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}

      {pages.map((page) => (
        <FolioPage
          key={`schedule-1-${page.pageIndex}`}
          pageNumber={page.pageIndex + 2}
          className={page.isContinuation ? "form-2551q-continuation continuation-page" : "form-2551q-page-two"}
        >
          <PageTwoMasthead pageNumber={page.pageIndex + 2} />
          <PageTwoIdentity envelope={envelope} />
          <OfficialSchedule
            rows={page.rows}
            startRowIndex={page.startRowIndex}
            isContinuation={page.isContinuation}
            summaryKind={page.summaryKind}
            pageTwoSubtotal={pageTwoSubtotal}
            totalTaxDue={decimal(envelope, "total_tax_due")}
          />
          {!page.isContinuation && <AtcReferenceTable />}
        </FolioPage>
      ))}
    </main>
  );
}

function PageTwoMasthead({ pageNumber }: { pageNumber: number }) {
  const barcodeText = `2551Q 01/18ENCS P${pageNumber}`;

  return (
    <header className="page-two-masthead">
      <div className="page-two-form-number">
        <span>BIR Form No.</span>
        <strong>2551Q</strong>
        <small>January 2018 (ENCS)</small>
        <b>Page {pageNumber}</b>
      </div>
      <div className="page-two-form-title">
        <strong>Quarterly Percentage Tax Return</strong>
      </div>
      <div className="page-two-barcode" aria-label={barcodeText}>
        <img src={OFFICIAL_2551Q_PAGE_TWO_BARCODE} alt="" aria-hidden="true" />
      </div>
    </header>
  );
}

function PageTwoIdentity({ envelope }: { envelope: RenderEnvelope }) {
  const tin = requireOfficialCellCapacity(
    envelope.taxpayer.tin.replace(/\D/g, ""),
    14,
    "taxpayer.tin"
  );
  const taxpayerName = envelope.taxpayer.name.toUpperCase();

  return (
    <section className="page-two-identity" aria-label="Taxpayer identity repeated on Schedule 1">
      <div className="page-two-identity-label">TIN</div>
      <div className="page-two-identity-label taxpayer-name-label">
        Taxpayer’s Last Name <em>(if Individual)</em> / Registered Name <em>(if Non-Individual)</em>
      </div>
      <CombValue value={tin} cells={14} />
      <AdaptiveCombValue value={taxpayerName} cells={26} />
    </section>
  );
}

function OfficialSchedule({
  rows,
  startRowIndex,
  isContinuation,
  summaryKind,
  pageTwoSubtotal,
  totalTaxDue
}: {
  rows: RenderRow[];
  startRowIndex: number;
  isContinuation: boolean;
  summaryKind: SchedulePage["summaryKind"];
  pageTwoSubtotal: number | null;
  totalTaxDue: number;
}) {
  const slotCount = isContinuation ? 12 : 6;
  const visualRows: Array<RenderRow | null> = [...rows];
  while (visualRows.length < slotCount) visualRows.push(null);
  const isFinal = summaryKind === "final_total";
  if (summaryKind === "page_2_subtotal" && pageTwoSubtotal === null) {
    throw new Error("2551Q overflow requires Rust-owned schedule_1_page_2_subtotal");
  }

  return (
    <section
      className={`official-schedule ${isContinuation ? "official-schedule-continuation" : "official-schedule-base"}`}
      data-schedule-page={isContinuation ? "continuation" : "base"}
    >
      <h2>
        <strong>Schedule 1 – Computation of Tax</strong>
        <em>{isContinuation ? "(Continuation)" : "(Attach additional sheet/s, if necessary)"}</em>
      </h2>
      <div className="official-schedule-head" role="row">
        <span><span>Alphanumeric Tax<br />Code <em>(ATC)</em></span></span>
        <span>Taxable Amount</span>
        <span><span>Tax<br />Rate</span></span>
        <span>Tax Due</span>
      </div>
      <div className="official-schedule-body">
        {visualRows.map((row, index) => (
          <ScheduleRow
            key={row?.key ?? `visual-empty-${startRowIndex + index + 1}`}
            row={row}
            number={startRowIndex + index + 1}
          />
        ))}
      </div>
      <div
        className={`official-schedule-total ${summaryKind === "page_2_subtotal" ? "is-subtotal" : isFinal ? "is-final" : "continues"}`}
        data-final-total={isFinal ? "true" : "false"}
        data-summary-kind={summaryKind}
        data-summary-value={summaryKind === "final_total" ? totalTaxDue : summaryKind === "page_2_subtotal" ? pageTwoSubtotal : undefined}
      >
        <div className="official-schedule-total-label">
          {summaryKind === "final_total" ? (
            <>
              <b>7&nbsp; Total Tax Due</b>
              <em>{isContinuation ? "(Sum of all Schedule 1 items)(To Part II Item 14)" : "(Sum of Items 1 to 6)(To Part II Item 14)"}</em>
            </>
          ) : summaryKind === "page_2_subtotal" ? (
            <>
              <b>7&nbsp; Subtotal carried to Schedule 1 continuation</b>
              <em>(Sum of Items 1 to 6)(Carry forward only)</em>
            </>
          ) : (
            <>
              <b>Schedule 1 continues on the following page</b>
              <em>(Total Tax Due is printed only on the final Schedule 1 page)</em>
            </>
          )}
        </div>
        {summaryKind === "final_total" ? (
          <ScheduleMoney value={totalTaxDue} trailingCell />
        ) : summaryKind === "page_2_subtotal" ? (
          <ScheduleMoney value={pageTwoSubtotal} trailingCell />
        ) : (
          <span className="schedule-carry-cell" />
        )}
      </div>
    </section>
  );
}

function requiredDecimal(envelope: RenderEnvelope, key: string): number {
  const value = envelope.fields[key];
  if (value?.type !== "decimal") {
    throw new Error(`2551Q render contract requires decimal field ${key}`);
  }
  return value.value;
}

function ScheduleRow({ row, number }: { row: RenderRow | null; number: number }) {
  const rowProps = row ? { "data-row-key": row.key } : { "data-visual-placeholder": "true" };

  return (
    <div className="official-schedule-row" data-row-slot={number} {...rowProps}>
      <b className="official-schedule-row-number">{number}</b>
      <CombValue value={row ? cellText(row.cells.atc).replace(/\s/g, "").toUpperCase() : ""} cells={5} />
      <ScheduleMoney
        value={row ? cellDecimal(row.cells.taxable_amount) : null}
        integerCells={11}
      />
      <ScheduleRate value={row ? cellDecimal(row.cells.tax_rate) : null} />
      <ScheduleMoney
        value={row ? cellDecimal(row.cells.tax_due) : null}
        integerCells={7}
        trailingCell
      />
    </div>
  );
}

function ScheduleMoney({
  value,
  integerCells = 7,
  trailingCell = false
}: {
  value: number | null;
  integerCells?: number;
  trailingCell?: boolean;
}) {
  const [whole, fraction] = value === null ? ["", ""] : formatMoneyParts(value);

  return (
    <span className={`schedule-money ${trailingCell ? "schedule-money-with-trailer" : ""}`}>
      <VisualIntegerComb value={whole} valueCells={integerCells} />
      <span className="schedule-decimal-separator">.</span>
      <CombValue value={fraction} cells={2} align="right" />
      {trailingCell && <span className="schedule-money-trailer" />}
    </span>
  );
}

function VisualIntegerComb({
  value,
  valueCells
}: {
  value: string;
  valueCells: number;
}) {
  const visualCells = 12;
  const leadingCells = visualCells - valueCells;
  if (leadingCells < 0) {
    throw new Error("2551Q money layout cannot exceed 12 visual integer cells");
  }

  return (
    <span className={`visual-integer-comb visual-integer-comb-leading-${leadingCells}`}>
      {leadingCells > 0 && (
        <span className="visual-leading-cells" aria-hidden="true">
          {Array.from({ length: leadingCells }, (_, index) => <span key={index} />)}
        </span>
      )}
      <CombValue value={value} cells={valueCells} align="right" />
    </span>
  );
}

function ScheduleRate({ value }: { value: number | null }) {
  const percent = value === null ? "" : String(Math.round(value * 100));
  return (
    <span className="schedule-rate">
      <CombValue value={percent} cells={2} align="right" />
      <b>%</b>
    </span>
  );
}

function AtcReferenceTable() {
  return (
    <table className="official-atc-table">
      <caption>Table 1 – Alphanumeric Tax Code (ATC)</caption>
      <colgroup><col /><col /><col /></colgroup>
      <thead>
        <tr><th>ATC</th><th>Percentage Tax On</th><th>Tax Rate</th></tr>
      </thead>
      <tbody>
        {ATC_REFERENCE.map((row, index) => {
          if (row.kind === "category") {
            return (
              <tr className="atc-category" key={`${row.description}-${row.section}`}>
                <th colSpan={3}>{row.description} <em>({row.section})</em></th>
              </tr>
            );
          }
          if (row.kind === "note") {
            return (
              <tr className="atc-note" data-atc-note={index} key={`note-${index}`}>
                <td />
                <td><span>{row.lines[0]}</span><span>{row.lines[1]}</span></td>
                <td />
              </tr>
            );
          }
          return (
            <tr className={row.lines ? "atc-entry atc-entry-multiline" : "atc-entry"} key={row.atc}>
              <td>{row.atc}</td>
              <td>
                {row.lines ? (
                  <><span>{row.lines[0]}</span><span>{row.lines[1]} {row.section && <em>({row.section})</em>}</span></>
                ) : (
                  <>{row.description} {row.section && <em>({row.section})</em>}</>
                )}
              </td>
              <td>{row.rate}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function OfficialPageOne({ envelope }: { envelope: RenderEnvelope }) {
  const yearEnded = `${String(envelope.period.month ?? 12).padStart(2, "0")}${String(envelope.period.taxable_year).padStart(4, "0")}`;
  const isFiscal = text(envelope, "tax_period_basis") === "fiscal";
  const item13Election = text(envelope, "item_13_election");
  const overpaymentDisposition = text(envelope, "overpayment_disposition");

  return (
    <>
      <header className="official-government-header">
        <div className="bir-use-only">For BIR<br />Use Only</div>
        <div className="bcs-item">BCS/<br />Item</div>
        <div className="government-wordmark">
          <img
            className="government-seal"
            src={OFFICIAL_2551Q_SEAL}
            alt="Bureau of Internal Revenue seal"
          />
          <strong>Republic of the Philippines<br />Department of Finance<br />Bureau of Internal Revenue</strong>
        </div>
      </header>

      <header className="official-masthead">
        <div className="official-form-number">
          <span>BIR Form No.</span>
          <strong>2551Q</strong>
          <small>January 2018 (ENCS)</small>
          <b>Page 1</b>
        </div>
        <div className="official-form-title">
          <strong>Quarterly Percentage Tax Return</strong>
          <em>Enter all required information in CAPITAL LETTERS using BLACK ink. Mark applicable<br />boxes with an “X”.&nbsp; Two copies MUST be filed with the BIR and one held by the Taxpayer.</em>
        </div>
        <div className="official-barcode" aria-label="2551Q 01/18ENCS P1">
          <img src={OFFICIAL_2551Q_BARCODE} alt="" aria-hidden="true" />
        </div>
      </header>

      <section className="official-header-options" aria-label="Return period and filing options">
        <div className="option-block filing-basis">
          <div className="option-label"><b>1</b> For the</div>
          <div className="option-choices">
            <CheckChoice checked={!isFiscal} label="Calendar" />
            <CheckChoice checked={isFiscal} label="Fiscal" />
          </div>
          <div className="option-label year-label"><b>2</b> Year Ended <em>(MM/YYYY)</em></div>
          <CombValue value={yearEnded} cells={6} align="right" />
        </div>
        <div className="option-block quarter-options">
          <div className="option-label"><b>3</b> Quarter</div>
          <div className="option-choices">
            {[1, 2, 3, 4].map((quarter) => (
              <CheckChoice
                key={quarter}
                checked={envelope.period.quarter === quarter}
                label={`${quarter}${quarter === 1 ? "st" : quarter === 2 ? "nd" : quarter === 3 ? "rd" : "th"}`}
              />
            ))}
          </div>
        </div>
        <div className="option-block amended-options">
          <div className="option-label"><b>4</b> Amended Return?</div>
          <div className="option-choices">
            <CheckChoice checked={bool(envelope, "is_amended")} label="Yes" />
            <CheckChoice checked={!bool(envelope, "is_amended")} label="No" />
          </div>
        </div>
        <div className="option-block sheets-options">
          <div className="option-label"><b>5</b> Number of Sheet/s</div>
          <div className="sheets-value"><span>Attached</span><CombValue value={String(integer(envelope, "number_of_attached_sheets")).padStart(2, "0")} cells={2} align="right" /></div>
        </div>
      </section>

      <BackgroundInformation envelope={envelope} item13Election={item13Election} />
      <TaxPayable envelope={envelope} overpaymentDisposition={overpaymentDisposition} />
      <OfficialDeclaration />
      <OfficialPaymentDetails />
      <p className="privacy-note"><b>*NOTE:</b> Please read the BIR Data Privacy Policy found in the BIR website (www.bir.gov.ph)</p>
    </>
  );
}

function BackgroundInformation({
  envelope,
  item13Election
}: {
  envelope: RenderEnvelope;
  item13Election: string;
}) {
  const tin = requireOfficialCellCapacity(
    envelope.taxpayer.tin.replace(/\D/g, ""),
    14,
    "taxpayer.tin"
  ).padEnd(14);
  const taxpayerName = envelope.taxpayer.name.toUpperCase();
  const address = envelope.taxpayer.registered_address.toUpperCase();
  const addressFitsOfficialComb = Array.from(address).length <= 71;
  const [addressLineOne, addressLineTwo] = addressFitsOfficialComb
    ? splitOfficialCombRows(address, 40, 31, "taxpayer.registered_address")
    : [address, ""];

  return (
    <section className="official-part background-information">
      <h2>Part I – Background Information</h2>
      <div className="tin-rdo-row">
        <div className="field-label"><b>6</b> Taxpayer Identification Number (TIN)</div>
        <CombValue value={tin.slice(0, 3)} cells={3} />
        <span className="tin-separator">-</span>
        <CombValue value={tin.slice(3, 6)} cells={3} />
        <span className="tin-separator">-</span>
        <CombValue value={tin.slice(6, 9)} cells={3} />
        <span className="tin-separator">-</span>
        <CombValue value={tin.slice(9, 14)} cells={5} />
        <div className="rdo-label"><b>7</b> RDO Code</div>
        <CombValue value={envelope.taxpayer.rdo_code} cells={3} align="right" />
      </div>
      <div className="full-width-field name-field">
        <div className="field-label"><b>8</b> Taxpayer’s Name <em>(Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)</em></div>
        <AdaptiveCombValue value={taxpayerName} cells={40} />
      </div>
      <div className="full-width-field address-field">
        <div className="field-label"><b>9</b> Registered Address <em>(Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)</em></div>
        {addressFitsOfficialComb ? (
          <CombValue value={addressLineOne} cells={40} />
        ) : (
          <AdaptiveCombValue
            value={addressLineOne}
            cells={71}
            className="address-overflow-value"
          />
        )}
        <div className="address-continuation">
          {addressFitsOfficialComb ? (
            <CombValue value={addressLineTwo} cells={31} />
          ) : (
            <span className="adaptive-address-spacer" aria-hidden="true" />
          )}
          <div className="zip-label"><b>9A</b> ZIP Code</div>
          <CombValue value={envelope.taxpayer.zip_code} cells={4} align="right" />
        </div>
      </div>
      <div className="contact-email-field">
        <div className="field-label"><b>10</b> Contact Number <em>(Landline/Cellphone No.)</em></div>
        <div className="field-label"><b>11</b> Email Address</div>
        <AdaptiveCombValue value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} />
        <AdaptiveCombValue value={envelope.taxpayer.email.toUpperCase()} cells={28} />
      </div>
      <div className="tax-relief-field">
        <div className="field-label"><b>12</b> Are you availing of tax relief under<br />Special Law or International Tax Treaty?</div>
        <div className="relief-choices">
          <CheckChoice checked={bool(envelope, "tax_relief")} label="Yes" />
          <CheckChoice checked={!bool(envelope, "tax_relief")} label="No" />
        </div>
        <div className="tax-relief-spec"><b>12A</b> If yes, specify</div>
        <AdaptiveCombValue value={text(envelope, "tax_relief_specification").toUpperCase()} cells={26} />
      </div>
      <div className="income-rate-field">
        <b>13</b>
        <div className="income-rate-question">
          Only for individual taxpayers whose sales/receipts are subject to Percentage Tax under Section 116 of the Tax Code, as amended:<br />
          What income tax rates are you availing? <em>(choose one)</em>
        </div>
        <i className="income-rate-note">(To be filled out only on the initial<br />quarter of the taxable year)</i>
        <div className="income-rate-choice graduated">
          <CheckChoice checked={item13Election === "graduated" || item13Election === "graduated_income_tax"} label="Graduated income tax rate on net taxable income" />
        </div>
        <div className="income-rate-choice eight-percent">
          <CheckChoice checked={item13Election === "eight_percent" || item13Election === "8_percent"} label="8% income tax rate on gross sales/receipts/others" />
        </div>
      </div>
    </section>
  );
}

function TaxPayable({
  envelope,
  overpaymentDisposition
}: {
  envelope: RenderEnvelope;
  overpaymentDisposition: string;
}) {
  const otherTaxCreditDescription = text(envelope, "other_tax_credit_description");

  return (
    <section className="official-part tax-payable">
      <h2>Part II – Total Tax Payable</h2>
      <OfficialTaxLine number={14} label={<>Total Tax Due <em>(From Schedule 1 Item 7)</em></>} value={decimal(envelope, "total_tax_due")} />
      <div className="tax-subheading">Less: Tax Credit/Payment <em>(attach proof)</em></div>
      <OfficialTaxLine number={15} indent label="Creditable Percentage Tax Withheld per BIR Form No. 2307" value={decimal(envelope, "creditable_tax_withheld")} />
      <OfficialTaxLine number={16} indent label="Tax Paid in Return Previously Filed, if this is an Amended Return" value={decimal(envelope, "tax_paid_previous")} />
      <OfficialTaxLine
        number={17}
        indent
        label={<>Other Tax Credit/Payment <em>(specify)</em>{otherTaxCreditDescription && <span className="tax-credit-description">: {otherTaxCreditDescription}</span>}</>}
        value={decimal(envelope, "other_tax_credit")}
        className="specify-line"
      />
      <OfficialTaxLine number={18} label={<>Total Tax Credits/Payments <em>(Sum of Items 15 to 17)</em></>} value={decimal(envelope, "total_tax_credits")} />
      <OfficialTaxLine number={19} label={<>Tax Still Payable/(Overpayment) <em>(Item 14 Less Item 18)</em></>} value={decimal(envelope, "tax_payable")} />
      <div className="tax-subheading penalties">Add: Penalties</div>
      <OfficialTaxLine number={20} indent label="Surcharge" value={decimal(envelope, "surcharge")} />
      <OfficialTaxLine number={21} indent label="Interest" value={decimal(envelope, "interest")} />
      <OfficialTaxLine number={22} indent label="Compromise" value={decimal(envelope, "compromise")} />
      <OfficialTaxLine number={23} label={<>Total Penalties <em>(Sum of Items 20 to 22)</em></>} value={decimal(envelope, "total_penalties")} />
      <OfficialTaxLine number={24} label={<>TOTAL AMOUNT PAYABLE/(Overpayment) <em>(Sum of Items 19 and 23)</em></>} value={decimal(envelope, "total_amount_payable")} strong />
      <div className="overpayment-options">
        <span>If overpayment, mark one box only:</span>
        <CheckChoice checked={overpaymentDisposition === "refund"} label="To be refunded" />
        <CheckChoice checked={overpaymentDisposition === "tax_credit_certificate" || overpaymentDisposition === "tcc"} label="To be issued a Tax Credit Certificate" />
      </div>
    </section>
  );
}

function OfficialTaxLine({
  number,
  label,
  value,
  strong = false,
  indent = false,
  className = ""
}: {
  number: number;
  label: React.ReactNode;
  value: number;
  strong?: boolean;
  indent?: boolean;
  className?: string;
}) {
  return (
    <div data-item={number} className={`official-tax-line ${strong ? "strong" : ""} ${indent ? "indented" : ""} ${className}`}>
      <div className="tax-line-label"><b>{number}</b> <span>{label}</span></div>
      <OfficialMoneyValue value={value} />
    </div>
  );
}

function OfficialMoneyValue({ value }: { value: number }) {
  const [whole, fraction] = formatMoneyParts(value);
  return (
    <span className="money-value official-money-value">
      <span className="official-money-integer">
        <span className="official-money-leading-cell" aria-hidden="true" />
        <CombValue value={whole} cells={11} align="right" />
      </span>
      <span className="decimal-separator">.</span>
      <CombValue value={fraction} cells={2} align="right" />
    </span>
  );
}

function OfficialDeclaration() {
  return (
    <section className="official-declaration">
      <p>
        I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct<br />
        pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as<br />
        contemplated under the “Data Privacy Act of 2012 (R.A. No. 10173)” for legitimate and lawful purposes. <em>(If Authorized Representative, attach authorization letter)</em>
      </p>
      <div className="official-signature-grid">
        <div>
          <span className="signature-space">For Individual:</span>
          <span className="signature-caption"><b>Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent</b><em>(Indicate title/designation and TIN)</em></span>
        </div>
        <div>
          <span className="signature-space">For Non-Individual:</span>
          <span className="signature-caption"><b>Signature over Printed Name of President/Vice President/<br />Authorized Officer or Representative/Tax Agent</b><em>(Indicate title/designation and TIN)</em></span>
        </div>
      </div>
      <div className="tax-agent-strip">
        <span>Tax Agent Accreditation No./<br />Attorney’s Roll No. <em>(If applicable)</em></span>
        <span />
        <span>Date of Issue<br /><em>(MM/DD/YYYY)</em></span>
        <span />
        <span>Expiry Date<br /><em>(MM/DD/YYYY)</em></span>
        <span />
      </div>
    </section>
  );
}

function OfficialPaymentDetails() {
  return (
    <section className="official-payment-details">
      <h2>Part III – Details of Payment</h2>
      <div className="payment-grid payment-headings">
        <b>Particulars</b><b>Drawee Bank/<br />Agency</b><b>Number</b><b>Date <em>(MM/DD/YYYY)</em></b><b>Amount</b>
      </div>
      {["Cash/Bank Debit Memo", "Check", "Tax Debit Memo"].map((label, index) => (
        <div className={`payment-grid payment-row payment-row-${25 + index}`} key={label}>
          <span><b>{25 + index}</b> {label}</span><CombValue value="" cells={8} /><CombValue value="" cells={8} /><CombValue value="" cells={8} /><BlankMoneyValue />
        </div>
      ))}
      <div className="payment-other-label"><b>28</b> Others <em>(Specify below)</em></div>
      <div className="payment-grid payment-other-row"><span /><CombValue value="" cells={8} /><CombValue value="" cells={8} /><CombValue value="" cells={8} /><BlankMoneyValue /></div>
      <div className="machine-validation">
        <span>Machine Validation/Revenue Official Receipt (ROR) Details <em>(if not filed with an Authorized Agent Bank)</em></span>
        <span><em>Stamp of receiving Office/AAB and Date of Receipt<br />(RO’s Signature/Bank Teller’s Initial)</em></span>
      </div>
    </section>
  );
}

function BlankMoneyValue() {
  return (
    <span className="money-value blank-money-value">
      <CombValue value="" cells={13} />
      <span className="decimal-separator">.</span>
      <CombValue value="" cells={2} />
    </span>
  );
}

export function requireOfficialCellCapacity(
  value: string,
  cells: number,
  fieldName: string
): string {
  const characterCount = Array.from(value).length;
  if (characterCount > cells) {
    throw new Error(
      `${fieldName} requires ${characterCount} cells but the official 2551Q field allows ${cells}`
    );
  }
  return value;
}

export function splitOfficialCombRows(
  value: string,
  firstRowCells: number,
  secondRowCells: number,
  fieldName: string
): [string, string] {
  requireOfficialCellCapacity(value, firstRowCells + secondRowCells, fieldName);
  const characters = Array.from(value);
  if (characters.length <= firstRowCells) return [value, ""];

  const minimumSplit = characters.length - secondRowCells;
  for (let index = firstRowCells - 1; index >= minimumSplit - 1; index -= 1) {
    if (characters[index] === " ") {
      const split = index + 1;
      return [characters.slice(0, split).join(""), characters.slice(split).join("")];
    }
  }

  if (characters[firstRowCells] === " ") {
    return [
      characters.slice(0, firstRowCells).join(""),
      characters.slice(firstRowCells).join("")
    ];
  }

  // A legal in-capacity address must never lose characters merely because it
  // contains an unusually long token. Hard-split only as the final fallback;
  // the official comb cells still make the boundary explicit.
  return [
    characters.slice(0, firstRowCells).join(""),
    characters.slice(firstRowCells).join("")
  ];
}
