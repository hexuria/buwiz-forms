import type { RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormSpec } from "@ebirforms/form-specs";
import type { CSSProperties, ReactNode } from "react";
import {
  AdaptivePlainValue,
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
  [["MC 180", "VAT/Non-VAT Registration Fee"], ["XB030", "Carbonated Beverages"], ["XP 110", "Aviation Gasoline"]],
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
  [["MC", "MISCELLANEOUS TAX"], ["PT", "PERCENTAGE TAX"], ["", ""]],
  [["XV", "EXCISE-AD VALOREM"], ["ST", "PERCENTAGE TAX - STOCKS"], ["WO", "WITHHOLDING TAX-OTHERS (ONE-TIME TRANSACTION NOT SUBJECT TO CAPITAL GAINS TAX)"]],
  [["XS", "EXCISE-SPECIFIC"], ["SO", "PERCENTAGE TAX - STOCKS (IPO)"], ["", ""]],
  [["XF", "TOBACCO INSPECTION AND MONITORING FEES"], ["SL", "PERCENTAGE TAX - SPECIAL LAWS"], ["", ""]],
  [["IT", "INCOME TAX"], ["DS", "DOCUMENTARY STAMP TAX"], ["WR", "WITHHOLDING TAX - FRINGE BENEFITS"]],
  [["CG", "CAPITAL GAINS TAX - Real Property"], ["WB", "WITHHOLDING TAX-BANKS AND OTHER FINANCIAL INSTITUTIONS"], ["WW", "WITHHOLDING TAX - PERCENTAGE TAX ON WINNING AND PRIZES"]]
] as const;

// Poppler identifies exactly these four official page-two entries as the
// embedded Arial Narrow subset (font id 3); the remaining ATC rows use Arial.
// The renderer keeps its bundled deterministic Arimo face and reproduces the
// reviewed narrow widths with per-entry horizontal scales instead of relying
// on a platform-installed Arial Narrow font.
const ATC_NARROW_SCALE_BY_DESCRIPTION: Readonly<Record<string, number>> = {
  "Powdered Drinks not Classified as Milk, Juice, Tea and Coffee": 0.716,
  "Other Non-Alcoholic Beverages that Contain Added Sugar": 0.772,
  "Using Purely Coconut Sap Sugar & Purely Steviol Glycosides": 0.712,
  "Performance of Services on Invasive Cosmetic Procedures": 0.772
};

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
      <div><b>DLN:</b><b>PSIC:</b><b>PSOC:</b></div>
    </div>
  );
}

function Masthead0605() {
  return (
    <header className="masthead-0605">
      <div className="government-0605">
        <img
          className="government-seal-0605"
          src={OFFICIAL_0605_SEAL}
          alt="Bureau of Internal Revenue seal"
          data-official-pdf-object="13 0"
        />
        <span className="government-copy-0605">
          <span className="government-line-one-0605">Republika ng Pilipinas</span>
          <span className="government-line-two-0605">Kagawaran ng Pananalapi</span>
          <span className="government-line-three-0605">Kawanihan ng Rentas Internas</span>
        </span>
      </div>
      <h1>Payment Form</h1>
      <div className="form-number-0605">
        <span>BIR Form No.</span>
        <strong>0605</strong>
        <small>July 1999 (ENCS)</small>
      </div>
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
        <div className="year-row-0605"><Label number="2">Year Ended <em>(MM / YYYY)</em></Label><GuidedValue0605 field="2-year-ended" value={yearEnded} segments={[2, 4]} /></div>
      </div>
      <div className="quarter-block-0605"><Label number="3">Quarter</Label><div>{[1, 2, 3, 4].map((value) => <CheckChoice key={value} checked={quarter === value} label={`${value}${value === 1 ? "st" : value === 2 ? "nd" : value === 3 ? "rd" : "th"}`} />)}</div></div>
      <HeaderValue0605 number="4" label="Due Date ( MM / DD / YYYY)"><GuidedValue0605 field="4-due-date" value={text(envelope, "due_date").replace(/\D/g, "")} segments={[2, 2, 4]} /></HeaderValue0605>
      <HeaderValue0605 number="5" label={<>No. of Sheets<br />Attached</>}><GuidedValue0605 field="5-sheets" value={String(integer(envelope, "number_of_sheets")).padStart(2, "0")} segments={[2]} align="right" /></HeaderValue0605>
      <HeaderValue0605 number="6" label="A T C"><PlainValue0605 field="6-atc" value={text(envelope, "atc")} /></HeaderValue0605>
      <HeaderValue0605 number="7" label="Return Period ( MM / DD / YYYY)"><GuidedValue0605 field="7-return-period" value={text(envelope, "return_period").replace(/\D/g, "")} segments={[2, 2, 4]} /></HeaderValue0605>
      <HeaderValue0605 number="8" label="Tax Type Code"><GuidedValue0605 field="8-tax-type" value={text(envelope, "tax_type_code")} segments={[2]} /></HeaderValue0605>
      <div className="bcs-item-0605"><span>BCS No./Item No. (To be filled up by the BIR)</span><i /></div>
    </section>
  );
}

function Label({ number, children }: { number: string; children: ReactNode }) {
  return <span className="item-label-0605"><b>{number}</b><i>{"\u00a0"}</i>{children}</span>;
}

function HeaderValue0605({ number, label, children }: { number: string; label: ReactNode; children: ReactNode }) {
  return <div className={`header-value-0605 item-${number}-0605`}><Label number={number}>{label}</Label><div>{children}</div></div>;
}

function PlainValue0605({
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
      className={`plain-field-shell-0605 ${className}`.trim()}
      data-official-field={field}
      data-field-mode="plain"
      data-guide-count="0"
    >
      <AdaptivePlainValue
        value={value}
        align={align}
        className="plain-field-value-0605"
        maxFontSizePx={10}
        minFontSizePx={6}
        fontStepPx={0.5}
      />
    </span>
  );
}

function GuidedValue0605({
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
      <PlainValue0605
        field={field}
        value={value}
        align={align}
        className={`guided-overflow-plain-0605 ${className}`.trim()}
      />
    );
  }

  let offset = 0;
  const guideCount = segments.reduce((total, segment) => total + segment - 1, 0);
  return (
    <span
      className={`guided-field-0605 ${className}`.trim()}
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
          <span className="guided-segment-0605" key={`${field}-${index}`}>
            <CombValue value={segmentValue} cells={segment} align={align} />
          </span>
        );
      })}
    </span>
  );
}

function BackgroundInformation0605({ envelope }: { envelope: RenderEnvelope }) {
  const classification = text(envelope, "taxpayer_classification");
  const manner = text(envelope, "manner_of_payment");
  const paymentType = text(envelope, "type_of_payment");
  const tin = envelope.taxpayer.tin.replace(/\D/g, "");
  return (
    <section className="part-one-0605">
      <h2><span>Part I</span><b>Background Information</b></h2>
      <div className="identity-row-0605">
        <HeaderValue0605 number="9" label="Taxpayer Identification No."><Tin0605 value={tin} /></HeaderValue0605>
        <HeaderValue0605 number="10" label="RDO Code"><GuidedValue0605 field="10-rdo" value={envelope.taxpayer.rdo_code} segments={[3]} /></HeaderValue0605>
        <div className="classification-0605"><Label number="11">Taxpayer Classification</Label><span><CheckChoice checked={classification === "individual"} label="I" /><CheckChoice checked={classification === "non_individual"} label="N" /></span></div>
        <HeaderValue0605 number="12" label="Line of Business/Occupation"><PlainValue0605 field="12-line-of-business" value={text(envelope, "line_of_business")} /></HeaderValue0605>
      </div>
      <div className="name-phone-0605">
        <div><Label number="13">Taxpayer's Name</Label><PlainValue0605 field="13-taxpayer-name" value={envelope.taxpayer.name.toUpperCase()} /><small>(Last Name, First Name, Middle Name for Individuals) / (Registered Name for Non-Individuals)</small></div>
        <HeaderValue0605 number="14" label="Telephone Number"><GuidedValue0605 field="14-telephone" value={envelope.taxpayer.contact_number.replace(/\D/g, "")} segments={[7]} /></HeaderValue0605>
      </div>
      <div className="address-zip-0605">
        <div><Label number="15">Registered Address</Label><PlainValue0605 field="15-registered-address" value={envelope.taxpayer.registered_address.toUpperCase()} /></div>
        <HeaderValue0605 number="16" label="Zip Code"><GuidedValue0605 field="16-zip" value={envelope.taxpayer.zip_code} segments={[4]} /></HeaderValue0605>
      </div>
      <div className="payment-choice-head-0605"><span>17&nbsp; Manner of Payment</span><span>18&nbsp; Type of Payment</span></div>
      <div className="payment-choices-0605">
        <div className="manner-0605">
          <h3>Voluntary Payment</h3><h3>Per Audit/Delinquent Account</h3>
          {MANNER_CHOICES.map(([value, label]) => <CheckChoice key={value} checked={manner === value} label={label} />)}
          <div className="other-manner-0605"><CheckChoice checked={manner === "others"} label="Others (Specify)" /><PlainValue0605 field="17-other-manner" value={text(envelope, "other_manner_description")} /></div>
        </div>
        <div className="type-choices-0605">
          <CheckChoice checked={paymentType === "installment"} label="Installment" />
          <div>
            <span
              className={bool(envelope, "number_of_installments_present") ? "check-box checked" : "check-box"}
              aria-hidden="true"
            >
              {bool(envelope, "number_of_installments_present") ? "X" : ""}
            </span>
            <span>No. of Installment</span>
            <PlainValue0605
              field="18-installment-count"
              value={bool(envelope, "number_of_installments_present") ? String(integer(envelope, "number_of_installments")) : ""}
              align="right"
              className="installment-count-0605"
            />
          </div>
          <CheckChoice checked={paymentType === "partial"} label="Partial Payment" />
          <CheckChoice checked={paymentType === "full"} label="Full Payment" />
        </div>
      </div>
    </section>
  );
}

// REVIEWER-AUTHORISED DIVERGENCE FROM THE PINNED 1999 GEOMETRY.
// The pinned July-1999 PDF prints item 9 as FOUR groups of three = 12 character
// cells (000-000-000-000): its page-two guideline even states "The last 3
// digits of the 12-digit TIN refers to the branch code", and the official
// comb ticks confirm 12 cells (per-group character ticks at x=43.68/57.12,
// 95.64/109.08, 148.32/161.76, 197.16/211.32 pt with grey separators between
// the four groups). The reviewer has deliberately widened the final group to
// FIVE cells (3-3-3-5 = 14 cells, the modern 000-000-000-00000 format) because
// real TINs now carry a 5-digit branch code that the pinned 12-cell field can
// no longer hold — the shipped fixtures all carry a 14-digit TIN. This is NOT a
// match to the pinned form; the field-guide inventory records the same reason.
const TIN_SEGMENTS_0605 = [3, 3, 3, 5] as const;

function Tin0605({ value }: { value: string }) {
  const characters = Array.from(value);
  const capacity = TIN_SEGMENTS_0605.reduce((total, segment) => total + segment, 0);
  if (characters.length > capacity) {
    return <PlainValue0605 field="9-tin" value={value} className="tin-overflow-plain-0605" />;
  }
  const guideCount = TIN_SEGMENTS_0605.reduce((total, segment) => total + segment - 1, 0);
  let offset = 0;
  return (
    <span
      className="tin-0605"
      data-official-field="9-tin"
      data-field-mode="guided"
      data-guide-count={guideCount}
      data-guide-segments={TIN_SEGMENTS_0605.join("-")}
      aria-label={value}
    >
      {TIN_SEGMENTS_0605.map((segment, index) => {
        const segmentValue = characters.slice(offset, offset + segment).join("");
        offset += segment;
        return (
          <span className="guided-segment-0605" key={index}>
            <CombValue value={segmentValue} cells={segment} />
            {index < TIN_SEGMENTS_0605.length - 1 && <i aria-hidden="true" />}
          </span>
        );
      })}
    </span>
  );
}

function Computation0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-two-0605">
      <h2><span>Part II</span><b>Computation of Tax</b></h2>
      <div className="item-19-row-0605"><span><b>19</b> Basic Tax / Deposit / Advance Payment</span><small className="item-caption-0605">19</small><Money0605 field="19-basic-tax" value={decimal(envelope, "item_19_basic_tax_or_payment")} /></div>
      <div className="penalties-row-0605"><span><b>20</b> Add: Penalties</span><label><span>Surcharge</span><small>20A</small><Money0605 field="20a-surcharge" value={decimal(envelope, "item_20a_surcharge")} /></label><label><span>Interest</span><small>20B</small><Money0605 field="20b-interest" value={decimal(envelope, "item_20b_interest")} /></label><label><span>Compromise</span><small>20C</small><Money0605 field="20c-compromise" value={decimal(envelope, "item_20c_compromise")} /></label><label className="total-penalty-0605"><small>20D</small><Money0605 field="20d-total-penalties" value={decimal(envelope, "item_20d_total_penalties")} /></label></div>
      <div className="item-21-row-0605"><span><b>21</b> Total Amount Payable&nbsp; (Sum of Items 19 &amp; 20D)</span><small className="item-caption-0605">21</small><Money0605 field="21-total-payable" value={decimal(envelope, "item_21_total_amount_payable")} /></div>
    </section>
  );
}

function Money0605({ field, value, present = true }: { field: string; value: number; present?: boolean }) {
  const [whole, fraction] = formatMoneyParts(value);
  if (present && Array.from(whole).length > 12) {
    return (
      <span className="money-0605 plain-money-field-0605" data-official-field={field} data-field-mode="plain" data-guide-count="0">
        <AdaptivePlainValue value={`${whole}.${fraction}`} align="right" className="money-overflow-0605" />
      </span>
    );
  }
  return (
    <span className="money-0605 plain-money-field-0605" data-official-field={field} data-field-mode="plain" data-guide-count="0">
      <span className="money-whole-0605">{present ? whole : ""}</span>
      <i>•</i>
      <span className="money-fraction-0605">{present ? fraction : ""}</span>
    </span>
  );
}

function Declaration0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="declaration-0605">
      <div className="voluntary-signature-0605"><h3>For Voluntary Payment</h3><p>I declare, under the penalties of perjury, that this document has been made in good faith, verified by me, and to the best of my knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof.</p><div className="signature-lines-0605"><SignatureLine0605 field="22a-taxpayer-signature" number="22A" value={text(envelope, "signature_taxpayer_or_representative")} label="Signature over Printed Name of Taxpayer /Authorized Representative" /><SignatureLine0605 field="22a-title-position" value={text(envelope, "signature_title_or_position")} label="Title/Position of Signatory" /></div></div>
      <div className="deficiency-signature-0605"><h3>For Payment of Deficiency Taxes<br />From Audit/Investigation/<br />Delinquent Accounts</h3><p>APPROVED BY:</p><SignatureLine0605 field="22b-head-of-office-signature" number="22B" value={text(envelope, "signature_head_of_office")} label="Signature over Printed Name of Head of Office" /></div>
      <div className="receiving-stamp-0605">Stamp of Receiving<br />Office<br /><br />and Date of Receipt</div>
    </section>
  );
}

function SignatureLine0605({ field, number, value, label }: { field: string; number?: string; value: string; label: string }) {
  return <div className="signature-line-0605">{number && <b>{number}</b>}<PlainValue0605 field={field} value={value} className="signature-value-0605" /><span>{label}</span></div>;
}

function PaymentDetails0605({ envelope }: { envelope: RenderEnvelope }) {
  return (
    <section className="part-three-0605">
      <h2><span>Part III</span><b>Details of Payment</b></h2>
      <div className="payment-table-head-0605"><span>Particulars</span><span>Drawee Bank/Agency</span><span>Number</span><span>MM</span><span>DD</span><span>YYYY</span><span>Amount</span></div>
      <div className="cash-payment-0605" data-payment-row="payment_23"><span><b>23</b> Cash/Bank<br />&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;Debit Memo</span><Money0605 field="23-amount" value={decimal(envelope, "payment_23_amount")} present={bool(envelope, "payment_23_amount_present")} /></div>
      <PaymentRow0605 envelope={envelope} number="24" label="Check" prefix="payment_24" bankItem="24A" numberItem="24B" dateItem="24C" amountItem="24D" />
      <PaymentRow0605 envelope={envelope} number="25" label={<>Tax Debit<br />Memo</>} prefix="payment_25" numberItem="25A" dateItem="25B" amountItem="25C" hideBank />
      <PaymentRow0605 envelope={envelope} number="26" label="Others" prefix="payment_26" bankItem="26A" numberItem="26B" dateItem="26C" amountItem="26D" />
      <div className="machine-validation-0605"><span>Machine Validation/Revenue Official Receipt Details (If not filed with the bank)</span><span className="machine-validation-value-0605" data-official-field="machine-validation" data-field-mode="plain" data-guide-count="0"><AdaptivePlainValue value={text(envelope, "machine_validation_or_receipt_details")} className="machine-validation-text-0605" maxFontSizePx={9} minFontSizePx={6} fontStepPx={0.5} /></span></div>
    </section>
  );
}

function PaymentRow0605({ envelope, number, label, prefix, bankItem, numberItem, dateItem, amountItem, hideBank = false }: { envelope: RenderEnvelope; number: string; label: ReactNode; prefix: string; bankItem?: string; numberItem: string; dateItem: string; amountItem: string; hideBank?: boolean }) {
  const date = text(envelope, `${prefix}_date`).replace(/\D/g, "");
  return (
    <div className={`payment-row-0605 ${hideBank ? "no-bank-0605" : ""}`} data-payment-row={prefix}>
      <span><b>{number}</b> {label}</span>
      {!hideBank && <FieldWithItem0605 field={`${number.toLowerCase()}a-bank`} item={bankItem} value={text(envelope, `${prefix}_drawee_bank_or_agency`)} className="payment-bank-0605" />}
      <FieldWithItem0605 field={`${number.toLowerCase()}${hideBank ? "a" : "b"}-number`} item={numberItem} value={text(envelope, `${prefix}_number`)} className="payment-number-0605" />
      <FieldWithItem0605 field={`${number.toLowerCase()}${hideBank ? "b" : "c"}-date`} item={dateItem} value={date} segments={[2, 2, 4]} className="payment-date-0605" />
      <div className="payment-amount-0605"><small>{amountItem}</small><Money0605 field={`${number.toLowerCase()}${hideBank ? "c" : "d"}-amount`} value={decimal(envelope, `${prefix}_amount`)} present={bool(envelope, `${prefix}_amount_present`)} /></div>
    </div>
  );
}

function FieldWithItem0605({ field, item, value, segments, className = "" }: { field: string; item?: string; value: string; segments?: readonly number[]; className?: string }) {
  return (
    <div className={`payment-field-0605 ${className}`}>
      <small>{item}</small>
      {segments
        ? <GuidedValue0605 field={field} value={value} segments={segments} />
        : <PlainValue0605 field={field} value={value} />}
    </div>
  );
}

function PageTwoReferenceTables0605() {
  return (
    <section className="reference-tables-0605">
      <p>BIR Form 0605 (ENCS) - PAGE 2</p>
      <table className="atc-table-0605">
        <thead>
          <tr>{[0, 1, 2].flatMap((index) => [<th key={`code-${index}`}>ATC</th>, <th key={`desc-${index}`}>NATURE OF PAYMENT</th>])}</tr>
        </thead>
        <tbody>
          {ATC_ROWS.map((_, rowIndex) => (
            <tr key={rowIndex}>
              {[0, 1, 2].map((groupIndex) => (
                <AtcCellPair0605
                  key={groupIndex}
                  groupIndex={groupIndex}
                  rowIndex={rowIndex}
                />
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <h2>T A X&nbsp;&nbsp; T Y P E</h2>
      <table className="tax-type-table-0605"><thead><tr>{[0, 1, 2].flatMap((index) => [<th key={`code-${index}`}>Code</th>, <th key={`desc-${index}`}>Description</th>])}</tr></thead><tbody>{TAX_TYPE_ROWS.map((row, rowIndex) => <tr key={rowIndex}>{row.flatMap(([code, description], groupIndex) => [<td key={`c-${groupIndex}`}>{code}</td>, <td key={`d-${groupIndex}`}>{description}</td>])}</tr>)}</tbody></table>
    </section>
  );
}

function AtcCellPair0605({ rowIndex, groupIndex }: { rowIndex: number; groupIndex: number }) {
  // The official right-hand XP010/XP020/XP190 entry occupies two physical
  // rows. Subsequent right-hand codes therefore advance one row later than
  // the left and middle columns. The two halves intentionally remain separate
  // table cells with a suppressed shared rule: a literal rowspan makes the
  // browser report its first tr as overflowing even though the cells fit the
  // two official rows.
  const sourceRow = groupIndex === 2 && rowIndex >= 12 ? rowIndex - 1 : rowIndex;
  const [sourceCode, sourceDescription] = ATC_ROWS[sourceRow][groupIndex];
  let code: string = sourceCode;
  let description: string = sourceDescription;
  const mergedStart = groupIndex === 2 && rowIndex === 10;
  const mergedEnd = groupIndex === 2 && rowIndex === 11;
  if (mergedStart) {
    code = "XP010, XP020 &";
    description = "Basetocks, Lubes and";
  } else if (mergedEnd) {
    code = "XP190";
    description = "Greases";
  }
  const isCategory = code.length === 0 && description.length > 0;
  // Official page-two ATC group-header indentation (contract 0605-1999 page 2
  // text_runs; ordinary description x0 = 80.64 / 280.61 / 480.58pt in the three
  // description columns). Verified against references/0605-1999-page-2-chromium.png
  // (pixel scan: Mineral +14.5, Alcohol +14.0, Miscellaneous +3.5, Excise +0):
  //  - the product-group sub-headers indent +14.28pt (Sweetened / Invasive
  //    Cosmetic / Tobacco Products / Tobacco Inspection Fees / Petroleum /
  //    Mineral / Alcohol Products all sit at x0 94.92 / 294.89 / 494.86);
  //  - "Excise Tax on Goods" is the OUTER top-level category and stays FLUSH at
  //    the ordinary x0 (80.64). The contract refutes any deeper indent for it:
  //    the product groups nest one level BENEATH it (e.g. Alcohol Products, the
  //    row directly below, indents +14.28 while Excise does not);
  //  - "Miscellaneous Products/Articles" carries only the official's two leading
  //    spaces (~3.68pt) — a shallower indent than the +14.28pt sub-headers, and
  //    HTML collapses the leading spaces so the indent must come from CSS.
  const categoryIndentClass = !isCategory
    ? ""
    : description === "Excise Tax on Goods"
      ? ""
      : description === "Miscellaneous Products/Articles"
        ? "atc-category-subindent-0605"
        : "atc-category-indent-0605";
  const narrowScale = ATC_NARROW_SCALE_BY_DESCRIPTION[description];
  const mergedClass = mergedStart
    ? "atc-merged-start-0605"
    : mergedEnd
      ? "atc-merged-end-0605"
      : "";

  return (
    <>
      <td className={mergedClass || undefined}>{code}</td>
      <td
        className={[
          mergedClass,
          isCategory ? "atc-category-0605" : "",
          categoryIndentClass,
          narrowScale ? "atc-narrow-0605" : ""
        ].filter(Boolean).join(" ") || undefined}
        data-official-font-role={narrowScale ? "Arial Narrow" : undefined}
        style={narrowScale
          ? ({ "--atc-narrow-scale": String(narrowScale) } as CSSProperties)
          : undefined}
      >
        {narrowScale ? <span>{description}</span> : description}
      </td>
    </>
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
