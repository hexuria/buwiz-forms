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
import { bool, decimal, integer, text } from "../values";
import { OFFICIAL_0605_SEAL } from "./official0605Assets";
import "./Form0605.css";

const MANNER_CHOICES = [
  ["self_assessment", "Self-Assessment"],
  ["tax_deposit", "Tax Deposit/Advance Payment"],
  ["income_tax_second_installment", "Income Tax Second Installment (Individual)"],
  ["penalties", "Penalties"],
  ["assessment_or_deficiency", "Preliminary/Final Assessment/Deficiency Tax"],
  ["accounts_receivable_or_delinquent", "Accounts Receivable/Delinquent Account"]
] as const;

const ATC_ROWS = [
  [["II 011", "Pure Compensation Income"], ["", "Sweetened Products"], ["XP060", "Premium (Unleaded) Gasoline"]],
  [["II 012", "Pure Business Income"], ["XB010", "Sweetened Juice Drinks"], ["XP080", "Regular Gasoline"]],
  [["II 013", "Mixed (Compensation and Business)"], ["XB020", "Sweetened Tea"], ["XP090 & XP100", "Naptha & Other Similar Products"]],
  [["MC 180", "VAT/Non-VAT Registration Fee"], ["XB030", "Carbonated Beverages"], ["XP110", "Aviation Gasoline"]],
  [["MC 190", "Travel Tax"], ["XB040", "Flavored Water"], ["XP140", "Diesel Gas"]],
  [["MC 090", "Tin Card Fees"], ["XB050", "Energy and Sports Drinks"], ["XP180", "Bunker Fuel Oil"]],
  [["MC010 & MC020", "Tax Amnesty"], ["XB060", "Powdered Drinks not Classified as Milk, Juice, Tea and Coffee"], ["XP120", "Avturbo Jet Fuel"]],
  [["MC 040", "Income from Forfeited Properties"], ["XB070", "Cereal and Grain Beverages"], ["XP130 & XP131", "Kerosene"]],
  [["MC 050", "Proceeds from Sale of Rent Estate"], ["XB080", "Other Non-Alcoholic Beverages that Contain Added Sugar"], ["XP170", "Asphalts"]],
  [["MC 060", "Energy Tax on Electric Power Consumption"], ["XB090", "Using Purely High Fructose Corn Syrup"], ["XP150 & XP160", "LPG Gas"]],
  [["MC 031", "Deficiency Tax"], ["XB100", "Using Purely Coconut Sap Sugar & Purely Steviol Glycosides"], ["XP010, XP020 & XP190", "Basetocks, Lubes and Greases"]],
  [["MC 030", "Delinquent Accounts/Accounts Receivable"], ["", "Invasive Cosmetic Products"], ["XP040", "Waxes and Petrolatum"]],
  [["FP 010 - FP 930", "Fines and Penalties"], ["XC010", "Performance of Services on Invasive Cosmetic Procedures"], ["XP030", "Processed Gas"]],
  [["MC 200", "Others"], ["", "Tobacco Products"], ["", "Miscellaneous Products/Articles"]],
  [["MC 210", "Miscellaneous Taxes -Other Tax Revenue"], ["XT010 & XT020", "Smoking and Chewing Tobacco"], ["XG020-XG090", "Automobiles"]],
  [["MC 220 - MC 240", "Advance Payment on Privilege Store"], ["XT030", "Cigars"], ["XG100-XG120", "Non Essential Goods"]],
  [["VM 160", "VAT on Manufacturing - Sugar"], ["XT040", "Cigarettes Packed By Hand"], ["", "Mineral Products"]],
  [["IC 080", "Income Tax on International Carriers"], ["XT050-XT130", "Cigarettes Packed By Machine"], ["XM010", "Coal & Coke"]],
  [["PT 041", "Percentage Tax on International Carriers"], ["", "Tobacco Inspection Fees"], ["XM020", "Non Metallic & Quarry Resources"]],
  [["DS 010", "Documentary Stamp Tax - General"], ["XT080", "Cigars"], ["XM030", "Gold and Chromite"]],
  [["", "Excise Tax on Goods"], ["XT090", "Cigarettes"], ["XM040", "Copper & Other Metallic Minerals"]],
  [["", "Alcohol Products"], ["XT100 & XT110", "Leaf Tobacco & Other Manufactured Tobacco"], ["XM050", "Indigenous Petroleum"]],
  [["XA010-XA040", "Distilled Spirits"], ["XT120", "Monitoring Fees"], ["XM051", "Others"]],
  [["XA061-XA090", "Wines"], ["", "Petroleum Products"], ["", ""]],
  [["XA051-XA053", "Fermented Liquor"], ["XP070", "Premium (Leaded) Gasoline"], ["", ""]]
] as const;

const TAX_TYPE_ROWS = [
  [["RF", "REGISTRATION FEE"], ["CS", "CAPITAL GAINS TAX - Stocks"], ["WC", "WITHHOLDING TAX-COMPENSATION"]],
  [["TR", "TRAVEL TAX-PTA"], ["ES", "ESTATE TAX"], ["WE", "WITHHOLDING TAX-EXPANDED"]],
  [["ET", "ENERGY TAX"], ["DN", "DONOR'S TAX"], ["WF", "WITHHOLDING TAX-FINAL"]],
  [["QP", "QUALIFYING FEES-PAGCOR"], ["VT", "VALUE-ADDED TAX"], ["WG", "WITHHOLDING TAX - VAT AND OTHER PERCENTAGE TAXES"]],
  [["MC", "MISCELLANEOUS TAX"], ["PT", "PERCENTAGE TAX"], ["WO", "WITHHOLDING TAX-OTHERS (ONE-TIME TRANSACTION NOT SUBJECT TO CAPITAL GAINS TAX)"]],
  [["XV", "EXCISE-AD VALOREM"], ["ST", "PERCENTAGE TAX - STOCKS"], ["WO", "WITHHOLDING TAX-OTHERS (ONE-TIME TRANSACTION NOT SUBJECT TO CAPITAL GAINS TAX)"]],
  [["XS", "EXCISE-SPECIFIC"], ["SO", "PERCENTAGE TAX - STOCKS (IPO)"], ["", ""]],
  [["XF", "TOBACCO INSPECTION AND MONITORING FEES"], ["SL", "PERCENTAGE TAX - SPECIAL LAWS"], ["", ""]],
  [["IT", "INCOME TAX"], ["DS", "DOCUMENTARY STAMP TAX"], ["WR", "WITHHOLDING TAX - FRINGE BENEFITS"]],
  [["CG", "CAPITAL GAINS TAX - Real Property"], ["WB", "WITHHOLDING TAX-BANKS AND OTHER FINANCIAL INSTITUTIONS"], ["WW", "WITHHOLDING TAX - PERCENTAGE TAX ON WINNING AND PRIZES"]]
] as const;

export function Form0605({ envelope }: { envelope: RenderEnvelope }) {
  getFormSpec("0605", "1999");
  if (envelope.schedules.length !== 0) {
    throw new Error("0605v1999 does not accept repeatable renderer schedules");
  }

  return (
    <main className="form-document" data-form-code="0605">
      <FolioPage pageNumber={1} paper="folio" className="form-0605-page form-0605-page-one">
        <PageOnePreamble0605 />
        <Masthead0605 />
        <Instruction0605 />
        <HeaderFields0605 envelope={envelope} />
        <BackgroundInformation0605 envelope={envelope} />
        <Computation0605 envelope={envelope} />
        <Declaration0605 envelope={envelope} />
        <PaymentDetails0605 envelope={envelope} />
        <p className="classification-note-0605">Taxpayer Classification:&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; I - Individual&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; N - Non-Individual</p>
      </FolioPage>
      <FolioPage pageNumber={2} paper="folio" className="form-0605-page form-0605-page-two">
        <PageTwoReferenceTables0605 />
        <Instructions0605 />
      </FolioPage>

      {envelope.validation.length > 0 && (
        <div className="preview-validation" aria-live="polite">
          <ValidationSummary issues={envelope.validation} />
        </div>
      )}
    </main>
  );
}

function PageOnePreamble0605() {
  return (
    <div className="preamble-0605">
      <span>(To be filled up the BIR)</span>
      <div><b>►&nbsp; DLN:</b><b>►&nbsp; PSIC:</b><b>►&nbsp; PSOC:</b></div>
    </div>
  );
}

function Masthead0605() {
  return (
    <header className="masthead-0605">
      <div className="government-0605">
        <img src={OFFICIAL_0605_SEAL} alt="Bureau of Internal Revenue seal" />
        <span>Republika ng Pilipinas<br />Kagawaran ng Pananalapi<br />Kawanihan ng Rentas Internas</span>
      </div>
      <h1>Payment Form</h1>
      <div className="form-number-0605"><span>BIR Form No.</span><strong>0605</strong><small>July 1999 (ENCS)</small></div>
    </header>
  );
}

function Instruction0605() {
  return <p className="fill-instruction-0605">Fill in all applicable spaces.&nbsp; Mark all appropriate boxes with an "X"</p>;
}

function HeaderFields0605({ envelope }: { envelope: RenderEnvelope }) {
  const basis = text(envelope, "filing_basis");
  const quarter = integer(envelope, "quarter");
  const yearEnded = `${String(integer(envelope, "year_end_month")).padStart(2, "0")}${String(envelope.period.taxable_year).padStart(4, "0")}`;
  return (
    <section className="header-fields-0605" aria-label="Items 1 to 8">
      <div className="period-block-0605">
        <div className="basis-row-0605"><Label number="1">For the</Label><CheckChoice checked={basis === "calendar"} label="Calendar" /><CheckChoice checked={basis === "fiscal"} label="Fiscal" /></div>
        <div className="year-row-0605"><Label number="2">Year Ended <em>(MM / YYYY)</em></Label><AdaptiveCombValue value={yearEnded} cells={6} /></div>
      </div>
      <div className="quarter-block-0605"><Label number="3">Quarter</Label><div>{[1, 2, 3, 4].map((value) => <CheckChoice key={value} checked={quarter === value} label={`${value}${value === 1 ? "st" : value === 2 ? "nd" : value === 3 ? "rd" : "th"}`} />)}</div></div>
      <HeaderValue0605 number="4" label="Due Date ( MM / DD / YYYY)"><AdaptiveCombValue value={text(envelope, "due_date").replace(/\D/g, "")} cells={8} /></HeaderValue0605>
      <HeaderValue0605 number="5" label={<>No. of Sheets<br />Attached</>}><AdaptiveCombValue value={String(integer(envelope, "number_of_sheets")).padStart(2, "0")} cells={2} align="right" /></HeaderValue0605>
      <HeaderValue0605 number="6" label="A T C"><AdaptiveCombValue value={text(envelope, "atc")} cells={10} /></HeaderValue0605>
      <HeaderValue0605 number="7" label="Return Period ( MM / DD / YYYY)"><AdaptiveCombValue value={text(envelope, "return_period").replace(/\D/g, "")} cells={8} /></HeaderValue0605>
      <HeaderValue0605 number="8" label="Tax Type Code"><AdaptiveCombValue value={text(envelope, "tax_type_code")} cells={4} /></HeaderValue0605>
      <div className="bcs-item-0605"><span>BCS No./Item No. (To be filled up by the BIR)</span><i /></div>
    </section>
  );
}

function Label({ number, children }: { number: string; children: ReactNode }) {
  return <span className="item-label-0605"><b>{number}</b><i>►</i>{children}</span>;
}

function HeaderValue0605({ number, label, children }: { number: string; label: ReactNode; children: ReactNode }) {
  return <div className={`header-value-0605 item-${number}-0605`}><Label number={number}>{label}</Label><div>{children}</div></div>;
}

function BackgroundInformation0605({ envelope }: { envelope: RenderEnvelope }) {
  const classification = text(envelope, "taxpayer_classification");
  const manner = text(envelope, "manner_of_payment");
  const paymentType = text(envelope, "type_of_payment");
  const tin = envelope.taxpayer.tin.replace(/\D/g, "").padEnd(14);
  return (
    <section className="part-one-0605">
      <h2><span>Part I</span><b>Background Information</b></h2>
      <div className="identity-row-0605">
        <HeaderValue0605 number="9" label="Taxpayer Identification No."><Tin0605 value={tin} /></HeaderValue0605>
        <HeaderValue0605 number="10" label="RDO Code"><CombValue value={envelope.taxpayer.rdo_code} cells={3} /></HeaderValue0605>
        <div className="classification-0605"><Label number="11">Taxpayer Classification</Label><span><CheckChoice checked={classification === "individual"} label="I" /><CheckChoice checked={classification === "non_individual"} label="N" /></span></div>
        <HeaderValue0605 number="12" label="Line of Business/Occupation"><AdaptiveCombValue value={text(envelope, "line_of_business")} cells={32} /></HeaderValue0605>
      </div>
      <div className="name-phone-0605">
        <div><Label number="13">Taxpayer's Name</Label><AdaptiveCombValue value={envelope.taxpayer.name.toUpperCase()} cells={52} /><small>(Last Name, First Name, Middle Name for Individuals) / (Registered Name for Non-Individuals)</small></div>
        <HeaderValue0605 number="14" label="Telephone Number"><AdaptiveCombValue value={envelope.taxpayer.contact_number.replace(/\D/g, "")} cells={12} /></HeaderValue0605>
      </div>
      <div className="address-zip-0605">
        <div><Label number="15">Registered Address</Label><AdaptiveCombValue value={envelope.taxpayer.registered_address.toUpperCase()} cells={52} /></div>
        <HeaderValue0605 number="16" label="Zip Code"><CombValue value={envelope.taxpayer.zip_code} cells={4} /></HeaderValue0605>
      </div>
      <div className="payment-choice-head-0605"><span>►&nbsp;&nbsp;17&nbsp; Manner of Payment</span><span>►&nbsp;&nbsp;18&nbsp; Type of Payment</span></div>
      <div className="payment-choices-0605">
        <div className="manner-0605">
          <h3>Voluntary Payment</h3><h3>Per Audit/Delinquent Account</h3>
          {MANNER_CHOICES.map(([value, label]) => <CheckChoice key={value} checked={manner === value} label={label} />)}
          <div className="other-manner-0605"><CheckChoice checked={manner === "others"} label="Others (Specify)" /><AdaptiveCombValue value={text(envelope, "other_manner_description")} cells={34} /></div>
        </div>
        <div className="type-choices-0605">
          <CheckChoice checked={paymentType === "installment"} label="Installment" />
          <div><span className="check-box" aria-hidden="true" /><span>No. of Installment</span><AdaptiveCombValue value={bool(envelope, "number_of_installments_present") ? String(integer(envelope, "number_of_installments")) : ""} cells={3} /></div>
          <CheckChoice checked={paymentType === "partial"} label="Partial Payment" />
          <CheckChoice checked={paymentType === "full"} label="Full Payment" />
        </div>
      </div>
    </section>
  );
}

function Tin0605({ value }: { value: string }) {
  return <span className="tin-0605"><CombValue value={value.slice(0, 3)} cells={3} /><i /><CombValue value={value.slice(3, 6)} cells={3} /><i /><CombValue value={value.slice(6, 9)} cells={3} /><i /><CombValue value={value.slice(9, 14)} cells={5} /></span>;
}

function Computation0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-two-0605">
      <h2><span>Part II</span><b>►&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; Computation of Tax</b></h2>
      <div className="item-19-row-0605"><span><b>19</b> Basic Tax / Deposit / Advance Payment</span><Money0605 value={decimal(envelope, "item_19_basic_tax_or_payment")} /></div>
      <div className="penalties-row-0605"><span><b>20</b> Add: Penalties</span><label><span>Surcharge</span><small>20A</small><Money0605 value={decimal(envelope, "item_20a_surcharge")} /></label><label><span>Interest</span><small>20B</small><Money0605 value={decimal(envelope, "item_20b_interest")} /></label><label><span>Compromise</span><small>20C</small><Money0605 value={decimal(envelope, "item_20c_compromise")} /></label><label className="total-penalty-0605"><small>20D</small><Money0605 value={decimal(envelope, "item_20d_total_penalties")} /></label></div>
      <div className="item-21-row-0605"><span><b>21</b> Total Amount Payable&nbsp; (Sum of Items 19 &amp; 20D)</span><Money0605 value={decimal(envelope, "item_21_total_amount_payable")} /></div>
    </section>
  );
}

function Money0605({ value, present = true }: { value: number; present?: boolean }) {
  if (!present) return <span className="money-0605"><CombValue value="" cells={12} /><i>•</i><CombValue value="" cells={2} /></span>;
  const [whole, fraction] = formatMoneyParts(value);
  if (Array.from(whole).length > 12) return <AdaptiveCombValue value={`${whole}.${fraction}`} cells={15} align="right" className="money-overflow-0605" />;
  return <span className="money-0605"><CombValue value={whole} cells={12} align="right" /><i>•</i><CombValue value={fraction} cells={2} align="right" /></span>;
}

function Declaration0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-0605">
      <div className="voluntary-signature-0605"><h3>For Voluntary Payment</h3><p>I declare, under the penalties of perjury, that this document has been made in good faith, verified by me, and to the best of my knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof.</p><div className="signature-lines-0605"><SignatureLine0605 number="22A" value={text(envelope, "signature_taxpayer_or_representative")} label="Signature over Printed Name of Taxpayer /Authorized Representative" /><SignatureLine0605 value={text(envelope, "signature_title_or_position")} label="Title/Position of Signatory" /></div></div>
      <div className="deficiency-signature-0605"><h3>For Payment of Deficiency Taxes<br />From Audit/Investigation/<br />Delinquent Accounts</h3><p>APPROVED BY:</p><SignatureLine0605 number="22B" value={text(envelope, "signature_head_of_office")} label="Signature over Printed Name of Head of Office" /></div>
      <div className="receiving-stamp-0605">Stamp of Receiving<br />Office<br /><br />and Date of Receipt</div>
    </section>
  );
}

function SignatureLine0605({ number, value, label }: { number?: string; value: string; label: string }) {
  const fontSize = Math.max(3.4, Math.min(5.4, 5.4 * 27 / Math.max(27, Array.from(value).length)));
  return <div className="signature-line-0605">{number && <b>{number}</b>}<span className="adaptive-plain-value" data-cell-capacity="44" data-overflow-mode="plain" aria-label={value} style={{ fontSize: `${fontSize}pt` }}>{value}</span><span>{label}</span></div>;
}

function PaymentDetails0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-three-0605">
      <h2><span>Part III</span><b>Details of Payment</b></h2>
      <div className="payment-table-head-0605"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>MM</span><span>DD</span><span>YYYY</span><span>Amount</span></div>
      <div className="cash-payment-0605"><span><b>23</b> Cash/Bank<br />&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;Debit Memo</span><Money0605 value={decimal(envelope, "payment_23_amount")} present={bool(envelope, "payment_23_amount_present")} /></div>
      <PaymentRow0605 envelope={envelope} number="24" label="Check" prefix="payment_24" bankItem="24A" numberItem="24B" dateItem="24C" amountItem="24D" />
      <PaymentRow0605 envelope={envelope} number="25" label={<>Tax Debit<br />Memo</>} prefix="payment_25" numberItem="25A" dateItem="25B" amountItem="25C" hideBank />
      <PaymentRow0605 envelope={envelope} number="26" label="Others" prefix="payment_26" bankItem="26A" numberItem="26B" dateItem="26C" amountItem="26D" />
      <div className="machine-validation-0605"><span>Machine Validation/Revenue Official Receipt Details (If not filed with the bank)</span><span className="adaptive-plain-value" data-cell-capacity="100" data-overflow-mode="plain" aria-label={text(envelope, "machine_validation_or_receipt_details")}>{text(envelope, "machine_validation_or_receipt_details")}</span></div>
    </section>
  );
}

function PaymentRow0605({ envelope, number, label, prefix, bankItem, numberItem, dateItem, amountItem, hideBank = false }: { envelope: RenderEnvelope; number: string; label: ReactNode; prefix: string; bankItem?: string; numberItem: string; dateItem: string; amountItem: string; hideBank?: boolean }) {
  const date = text(envelope, `${prefix}_date`).replace(/\D/g, "");
  return (
    <div className={`payment-row-0605 ${hideBank ? "no-bank-0605" : ""}`}>
      <span><b>{number}</b> {label}</span>
      {!hideBank && <FieldWithItem0605 item={bankItem} value={text(envelope, `${prefix}_drawee_bank_or_agency`)} cells={22} className="payment-bank-0605" />}
      <FieldWithItem0605 item={numberItem} value={text(envelope, `${prefix}_number`)} cells={18} className="payment-number-0605" />
      <FieldWithItem0605 item={dateItem} value={date} cells={8} className="payment-date-0605" />
      <div className="payment-amount-0605"><small>{amountItem} ►</small><Money0605 value={decimal(envelope, `${prefix}_amount`)} present={bool(envelope, `${prefix}_amount_present`)} /></div>
    </div>
  );
}

function FieldWithItem0605({ item, value, cells, className = "" }: { item?: string; value: string; cells: number; className?: string }) {
  return <div className={`payment-field-0605 ${className}`}><small>{item} ►</small><AdaptiveCombValue value={value} cells={cells} /></div>;
}

function PageTwoReferenceTables0605() {
  return (
    <section className="reference-tables-0605">
      <p>BIR Form 0605 (ENCS) - PAGE 2</p>
      <table className="atc-table-0605"><thead><tr>{[0, 1, 2].flatMap((index) => [<th key={`code-${index}`}>ATC</th>, <th key={`desc-${index}`}>NATURE OF PAYMENT</th>])}</tr></thead><tbody>{ATC_ROWS.map((row, rowIndex) => <tr key={rowIndex}>{row.flatMap(([code, description], groupIndex) => [<td key={`c-${groupIndex}`}>{code}</td>, <td key={`d-${groupIndex}`}>{description}</td>])}</tr>)}</tbody></table>
      <h2>T A X&nbsp;&nbsp; T Y P E</h2>
      <table className="tax-type-table-0605"><thead><tr>{[0, 1, 2].flatMap((index) => [<th key={`code-${index}`}>Code</th>, <th key={`desc-${index}`}>Description</th>])}</tr></thead><tbody>{TAX_TYPE_ROWS.map((row, rowIndex) => <tr key={rowIndex}>{row.flatMap(([code, description], groupIndex) => [<td key={`c-${groupIndex}`}>{code}</td>, <td key={`d-${groupIndex}`}>{description}</td>])}</tr>)}</tbody></table>
    </section>
  );
}

function Instructions0605() {
  return (
    <section className="instructions-0605">
      <h2>BIR Form No. 0605 - Payment Form<br />Guidelines and Instructions</h2>
      <div>
        <article><h3>Who Shall File</h3><p>Every taxpayer shall use this form, in triplicate, to pay taxes and fees which do not require the use of a tax return such as second installment payment for income tax, deficiency tax, delinquency tax, registration fees, penalties, advance payments, deposits, installment payments, etc.</p><h3>When and Where to File and Pay</h3><p>This form shall be accomplished:<br />1. Everytime a tax payment or penalty is due or an advance payment is to be made;<br />2. Upon receipt of a demand letter/assessment notice and/or collection letter from the BIR; and<br />3. Upon payment of annual registration fee for new business and for renewals on or before January 31 of every year.</p><p>This form shall be filed and the tax shall be paid with any Authorized Agent Bank (AAB) under the jurisdiction of the Revenue District Office where the taxpayer is required to register/conducting business/producing articles subject to excise tax/having taxable transactions. In places where there are no AABs, this form shall be filed and the tax shall be paid directly with the Revenue Collection Officer or duly Authorized City or Municipal Treasurer of the Revenue District Office where the taxpayer is required to register/conducting business/producing articles subject to excise tax/having taxable transactions, who shall issue Revenue Official Receipt (BIR Form No. 2524) therefor.</p><p>Where the return is filed with an AAB, the lower portion of the return must be properly machine-validated and stamped by the Authorized Agent Bank to serve as the receipt of payment. The machine validation shall reflect the date of payment, amount paid and transaction code, and the stamp mark shall show the name of the bank, branch code, teller’s name and teller’s initial. The AAB shall also issue an official receipt or bank debit advice or credit document, whichever is applicable, as additional proof of payment.</p></article>
        <article><p>One set of form shall be filled-up for each kind of tax and for each taxable period.</p><h3>Note:</h3><p>• All background information must be properly filled-up.</p><p>• For voluntary payment of taxes, BIR Form 0605 shall be signed by the taxpayer or his authorized representative.</p><p>• For payment of deficiency taxes at the Revenue District Office level and other investigating offices prior to the issuance of Preliminary Assessment Notice (PAN)/Final Assessment Notice (FAN), BIR Form 0605 shall be approved and signed by the Revenue District Officer (RDO) or Head of the investigating offices.</p><p>• For payment of deficiency taxes with issued Preliminary Assessment Notice/Final Assessment Notice, BIR Form 0605 shall be signed by the taxpayer or his authorized representative.</p><p>• The last 3 digits of the 12-digit TIN refers to the branch code.</p><h3>Attachments<br />For Voluntary Payment:</h3><p>1. Duly approved Tax Debit Memo, if applicable;<br />2. Xerox copy of the return (ITR)/ Reminder Letter in case of payment of second installment on income tax.</p><h3>For Payment of Deficiency Taxes from Audit/ Delinquent Accounts:</h3><p>1. Duly approved Tax Debit Memo, if applicable;<br />2. Preliminary Assessment Notice (PAN)/ Final Assessment Notice (FAN)’ Letter of Demand;<br />3. Post Reporting Notice, if applicable;<br />4. Collection letter of Delinquent/ Accounts Receivable.</p><b className="encs-0605">ENCS</b></article>
      </div>
    </section>
  );
}
