import atcReference2551q from "./generated/2551q-atc-reference.json";
import type { RenderEnvelopeV1, RenderValue } from "./generated";

export type {
  RenderAlignment,
  RenderColumn,
  RenderEnvelopeV1,
  RenderFormIdentity,
  RenderPeriod,
  RenderRow,
  RenderSchedule,
  RenderTaxpayer,
  RenderValidationMessage,
  RenderValidationSeverity,
  RenderValue
} from "./generated";
export type { RenderEnvelopeV1 as RenderEnvelope } from "./generated";

export type AtcReferenceEntry = Readonly<{
  code: string;
  description: string;
  rate: number;
}>;

export type Form2551QAtcReference = Readonly<{
  schema_version: 1;
  form_code: "2551Q";
  revision: "2018";
  entries: readonly AtcReferenceEntry[];
}>;

function asForm2551QAtcReference(reference: {
  readonly schema_version: number;
  readonly form_code: string;
  readonly revision: string;
  readonly entries: readonly AtcReferenceEntry[];
}): Form2551QAtcReference {
  if (
    reference.schema_version !== 1 ||
    reference.form_code !== "2551Q" ||
    reference.revision !== "2018"
  ) {
    throw new Error("Invalid generated 2551Q ATC reference metadata");
  }
  return reference as Form2551QAtcReference;
}

/** Exact January 2018 2551Q ATC registry generated from bir-core. */
export const FORM_2551Q_ATC_REFERENCE = asForm2551QAtcReference(atcReference2551q);

export const RENDER_CONTRACT_VERSION = "1.0" as const;

type JsonObject = Record<string, unknown>;
type RenderValueType = RenderValue["type"];

const REQUIRED_0605_FIELDS: Readonly<Record<string, RenderValueType>> = {
  filing_basis: "text",
  quarter: "integer",
  year_end_month: "integer",
  due_date: "text",
  return_period: "text",
  number_of_sheets: "integer",
  atc: "text",
  tax_type_code: "text",
  taxpayer_classification: "text",
  line_of_business: "text",
  manner_of_payment: "text",
  other_manner_description: "text",
  type_of_payment: "text",
  number_of_installments_present: "boolean",
  number_of_installments: "integer",
  item_19_basic_tax_or_payment: "decimal",
  item_20a_surcharge: "decimal",
  item_20b_interest: "decimal",
  item_20c_compromise: "decimal",
  item_20d_total_penalties: "decimal",
  item_21_total_amount_payable: "decimal",
  approval_selection: "text",
  signature_taxpayer_or_representative: "text",
  signature_title_or_position: "text",
  signature_head_of_office: "text",
  payment_23_amount_present: "boolean",
  payment_23_amount: "decimal",
  payment_24_drawee_bank_or_agency: "text",
  payment_24_number: "text",
  payment_24_date: "text",
  payment_24_amount_present: "boolean",
  payment_24_amount: "decimal",
  payment_25_drawee_bank_or_agency: "text",
  payment_25_number: "text",
  payment_25_date: "text",
  payment_25_amount_present: "boolean",
  payment_25_amount: "decimal",
  payment_26_drawee_bank_or_agency: "text",
  payment_26_number: "text",
  payment_26_date: "text",
  payment_26_amount_present: "boolean",
  payment_26_amount: "decimal",
  machine_validation_or_receipt_details: "text"
};

const REQUIRED_2551Q_FIELDS: Readonly<Record<string, RenderValueType>> = {
  tax_period_basis: "text",
  is_amended: "boolean",
  number_of_attached_sheets: "integer",
  tax_relief: "boolean",
  tax_relief_specification: "text",
  item_13_election: "text",
  total_tax_due: "decimal",
  creditable_tax_withheld: "decimal",
  tax_paid_previous: "decimal",
  other_tax_credit: "decimal",
  other_tax_credit_description: "text",
  total_tax_credits: "decimal",
  tax_payable: "decimal",
  surcharge: "decimal",
  interest: "decimal",
  compromise: "decimal",
  total_penalties: "decimal",
  total_amount_payable: "decimal",
  overpayment_disposition: "text"
};

const REQUIRED_2551Q_SCHEDULE_CELLS: Readonly<Record<string, RenderValueType>> = {
  atc: "text",
  description: "text",
  taxable_amount: "decimal",
  tax_rate: "decimal",
  tax_due: "decimal"
};

const REQUIRED_1601C_FIELDS: Readonly<Record<string, RenderValueType>> = {
  is_amended: "boolean",
  any_taxes_withheld: "boolean",
  number_of_sheets: "integer",
  atc: "text",
  line_of_business: "text",
  registered_address_2: "text",
  category_of_agent: "text",
  tax_relief: "boolean",
  tax_relief_specification: "text",
  tax_14_total_compensation: "decimal",
  tax_15_statutory_minimum_wage: "decimal",
  tax_16_holiday_pay: "decimal",
  tax_17_13th_month_pay: "decimal",
  tax_18_de_minimis: "decimal",
  tax_19_sss_gsis: "decimal",
  tax_20_other_name: "text",
  tax_20_other_amount: "decimal",
  tax_21_total_non_taxable: "decimal",
  tax_22_total_taxable: "decimal",
  tax_23_not_subject: "decimal",
  tax_24_net_taxable: "decimal",
  tax_25_total_taxes_withheld: "decimal",
  tax_26_adjustment: "decimal",
  tax_27_taxes_withheld_for_remittance: "decimal",
  tax_28_tax_remitted_previously: "decimal",
  tax_29_other_remittances_name: "text",
  tax_29_other_remittances_amount: "decimal",
  tax_30_total_tax_remittances: "decimal",
  tax_31_tax_still_due: "decimal",
  tax_32_surcharge: "decimal",
  tax_33_interest: "decimal",
  tax_34_compromise: "decimal",
  tax_35_total_penalties: "decimal",
  tax_36_total_amount_payable: "decimal"
};

const REQUIRED_1601C_SCHEDULE_CELLS: Readonly<Record<string, RenderValueType>> = {
  previous_month: "text",
  date_paid: "text",
  drawee_bank_code_or_agency: "text",
  payment_number: "text",
  tax_paid: "decimal",
  should_be_tax_due: "decimal",
  adjustment: "decimal"
};

const REQUIRED_0619E_FIELDS: Readonly<Record<string, RenderValueType>> = {
  is_amended: "boolean",
  any_taxes_withheld: "boolean",
  due_date: "text",
  atc: "text",
  tax_type_code: "text",
  line_of_business: "text",
  registered_address_2: "text",
  withholding_agent_category: "text",
  item_14_amount_of_remittance: "decimal",
  item_15_amount_remitted_previously: "decimal",
  item_16_net_amount_of_remittance: "decimal",
  item_17a_surcharge: "decimal",
  item_17b_interest: "decimal",
  item_17c_compromise: "decimal",
  item_17d_total_penalties: "decimal",
  item_18_total_amount_of_remittance: "decimal",
  tax_agent_accreditation_number: "text",
  tax_agent_date_of_issue: "text",
  tax_agent_date_of_expiry: "text",
  payment_22_particular: "text",
  payment_19_drawee_bank_or_agency: "text",
  payment_19_number: "text",
  payment_19_date: "text",
  payment_19_amount_present: "boolean",
  payment_19_amount: "decimal",
  payment_20_drawee_bank_or_agency: "text",
  payment_20_number: "text",
  payment_20_date: "text",
  payment_20_amount_present: "boolean",
  payment_20_amount: "decimal",
  payment_21_drawee_bank_or_agency: "text",
  payment_21_number: "text",
  payment_21_date: "text",
  payment_21_amount_present: "boolean",
  payment_21_amount: "decimal",
  payment_22_drawee_bank_or_agency: "text",
  payment_22_number: "text",
  payment_22_date: "text",
  payment_22_amount_present: "boolean",
  payment_22_amount: "decimal"
};

const REQUIRED_0619F_FIELDS: Readonly<Record<string, RenderValueType>> = {
  is_amended: "boolean",
  any_taxes_withheld: "boolean",
  due_date: "text",
  item_13_atc: "text",
  item_14_atc: "text",
  tax_type_code: "text",
  line_of_business: "text",
  registered_address_2: "text",
  withholding_agent_category: "text",
  item_13_interest_final_tax_withheld: "decimal",
  item_14_other_final_tax_withheld: "decimal",
  item_15_total: "decimal",
  item_16_remitted_previously: "decimal",
  item_17_net_amount_of_remittance: "decimal",
  item_18a_surcharge: "decimal",
  item_18b_interest: "decimal",
  item_18c_compromise: "decimal",
  item_18d_total_penalties: "decimal",
  item_19_total_amount_of_remittance: "decimal",
  tax_agent_accreditation_number: "text",
  tax_agent_date_of_issue: "text",
  tax_agent_date_of_expiry: "text",
  payment_23_particular: "text",
  payment_20_drawee_bank_or_agency: "text",
  payment_20_number: "text",
  payment_20_date: "text",
  payment_20_amount_present: "boolean",
  payment_20_amount: "decimal",
  payment_21_drawee_bank_or_agency: "text",
  payment_21_number: "text",
  payment_21_date: "text",
  payment_21_amount_present: "boolean",
  payment_21_amount: "decimal",
  payment_22_drawee_bank_or_agency: "text",
  payment_22_number: "text",
  payment_22_date: "text",
  payment_22_amount_present: "boolean",
  payment_22_amount: "decimal",
  payment_23_drawee_bank_or_agency: "text",
  payment_23_number: "text",
  payment_23_date: "text",
  payment_23_amount_present: "boolean",
  payment_23_amount: "decimal"
};

const REQUIRED_1701Q_PREVIEW_FIELDS: Readonly<Record<string, RenderValueType>> = {
  is_amended: "boolean",
  attached_sheets: "text"
};

const REQUIRED_2550Q_PREVIEW_FIELDS: Readonly<Record<string, RenderValueType>> = {
  filing_basis: "text",
  year_end_month: "integer",
  quarter: "integer",
  return_period_from: "text",
  return_period_to: "text",
  is_amended: "boolean",
  is_short_period_return: "boolean",
  is_availing_tax_relief: "boolean",
  tax_relief_details: "text",
  item_19_description: "text",
  item_42_description: "text",
  item_47_description: "text",
  item_56_description: "text",
  signature_taxpayer_or_representative: "text",
  signature_representative_title: "text",
  signature_non_individual_officer: "text",
  tax_agent_accreditation_or_roll_number: "text",
  tax_agent_date_of_issue: "text",
  tax_agent_date_of_expiry: "text",
  payment_check_bank: "text",
  payment_check_number: "text",
  payment_check_date: "text",
  payment_tax_debit_memo_number: "text",
  payment_tax_debit_memo_date: "text",
  payment_other_description: "text",
  payment_other_bank: "text",
  payment_other_number: "text",
  payment_other_date: "text",
  machine_validation_or_receipt_details: "text"
};

const REQUIRED_1701_PREVIEW_FIELDS: Readonly<Record<string, RenderValueType>> = {
  is_amended: "boolean",
  is_short_period: "boolean",
  overpayment_disposition: "text",
  spouse_enabled: "boolean",
  spouse_name: "text",
  spouse_tin: "text",
  citizenship: "text",
  date_of_birth: "text",
  machine_validation_or_receipt_details: "text"
};

const REQUIRED_1702RT_PREVIEW_FIELDS: Readonly<Record<string, RenderValueType>> = {
  filing_basis: "text",
  is_amended: "boolean",
  is_short_period: "boolean",
  deduction_method: "text",
  atc_mcit_selected: "boolean",
  atc_other_selected: "boolean",
  atc_other_code: "text",
  number_of_attachments: "text",
  item_43: "integer",
  item_56: "integer"
};

const REQUIRED_1702MX_PREVIEW_FIELDS: Readonly<Record<string, RenderValueType>> = {
  filing_basis: "text",
  is_amended: "boolean",
  is_short_period: "boolean",
  deduction_method: "text",
  atc_mcit_selected: "boolean",
  atc_other_selected: "boolean",
  atc_other_code: "text",
  number_of_attachments: "text",
  mandatory_attachment_document_kind: "text",
  mandatory_attachment_has_values: "boolean",
  mandatory_attachment_page_count: "integer",
  mandatory_attachment_source_sha256: "text",
  mandatory_attachment_transport_supported: "boolean"
};

/**
 * Runtime validation for the renderer boundary.
 *
 * Generated TypeScript types protect build-time callers, but native WebView
 * injection and calibration fixtures enter as untrusted JSON. Keep this check
 * strict so missing legal values never become confident `false`, blank, or
 * zero values in a printable document.
 */
export function assertRenderEnvelope(value: unknown): asserts value is RenderEnvelopeV1 {
  const envelope = objectAt(value, "envelope");
  if (envelope.schema_version !== RENDER_CONTRACT_VERSION) {
    invalid("envelope.schema_version", `expected ${RENDER_CONTRACT_VERSION}`);
  }

  nonEmptyStringAt(envelope.locale, "envelope.locale");

  const form = objectAt(envelope.form, "envelope.form");
  const formCode = nonEmptyStringAt(form.code, "envelope.form.code");
  const formVersion = nonEmptyStringAt(form.version, "envelope.form.version");

  const taxpayer = objectAt(envelope.taxpayer, "envelope.taxpayer");
  for (const key of [
    "tin",
    "name",
    "rdo_code",
    "registered_address",
    "zip_code",
    "contact_number",
    "email"
  ]) {
    stringAt(taxpayer[key], `envelope.taxpayer.${key}`);
  }

  // Fixed identifier fields remain exact. Human-readable profile fields have
  // larger defensive limits because the renderer switches from official comb
  // cells to an unclipped plain-text box when their values do not fit.
  maximumCharactersAt(
    stringAt(taxpayer.tin, "envelope.taxpayer.tin").replace(/\D/g, ""),
    14,
    "envelope.taxpayer.tin"
  );
  maximumCharactersAt(taxpayer.name, 160, "envelope.taxpayer.name");
  maximumCharactersAt(taxpayer.rdo_code, 3, "envelope.taxpayer.rdo_code");
  maximumCharactersAt(
    taxpayer.registered_address,
    320,
    "envelope.taxpayer.registered_address"
  );
  maximumCharactersAt(taxpayer.zip_code, 4, "envelope.taxpayer.zip_code");
  const contactNumber = stringAt(
    taxpayer.contact_number,
    "envelope.taxpayer.contact_number"
  );
  if (!/^[0-9+().\s-]*$/.test(contactNumber)) {
    invalid(
      "envelope.taxpayer.contact_number",
      "contains unsupported print characters"
    );
  }
  maximumCharactersAt(
    contactNumber.replace(/\D/g, ""),
    32,
    "envelope.taxpayer.contact_number"
  );
  maximumCharactersAt(taxpayer.email, 254, "envelope.taxpayer.email");

  const period = objectAt(envelope.period, "envelope.period");
  integerAt(period.taxable_year, "envelope.period.taxable_year");
  stringAt(period.label, "envelope.period.label");
  optionalIntegerAt(period.month, "envelope.period.month");
  optionalIntegerAt(period.quarter, "envelope.period.quarter");

  const fields = objectAt(envelope.fields, "envelope.fields");
  for (const [key, fieldValue] of Object.entries(fields)) {
    assertRenderValue(fieldValue, `envelope.fields.${key}`);
  }

  const schedules = arrayAt(envelope.schedules, "envelope.schedules");
  const scheduleIds = new Set<string>();
  for (const [scheduleIndex, scheduleValue] of schedules.entries()) {
    const path = `envelope.schedules[${scheduleIndex}]`;
    const schedule = objectAt(scheduleValue, path);
    const id = nonEmptyStringAt(schedule.id, `${path}.id`);
    if (scheduleIds.has(id)) invalid(`${path}.id`, `duplicate schedule id ${id}`);
    scheduleIds.add(id);

    const columns = arrayAt(schedule.columns, `${path}.columns`);
    const columnKeys = new Set<string>();
    for (const [columnIndex, columnValue] of columns.entries()) {
      const columnPath = `${path}.columns[${columnIndex}]`;
      const column = objectAt(columnValue, columnPath);
      const key = nonEmptyStringAt(column.key, `${columnPath}.key`);
      if (columnKeys.has(key)) invalid(`${columnPath}.key`, `duplicate column key ${key}`);
      columnKeys.add(key);
      stringAt(column.label, `${columnPath}.label`);
      if (
        column.alignment !== "left" &&
        column.alignment !== "center" &&
        column.alignment !== "right"
      ) {
        invalid(`${columnPath}.alignment`, "expected left, center, or right");
      }
    }

    const rows = arrayAt(schedule.rows, `${path}.rows`);
    const rowKeys = new Set<string>();
    for (const [rowIndex, rowValue] of rows.entries()) {
      const rowPath = `${path}.rows[${rowIndex}]`;
      const row = objectAt(rowValue, rowPath);
      const key = nonEmptyStringAt(row.key, `${rowPath}.key`);
      if (rowKeys.has(key)) invalid(`${rowPath}.key`, `duplicate row key ${key}`);
      rowKeys.add(key);
      const cells = objectAt(row.cells, `${rowPath}.cells`);
      for (const [cellKey, cellValue] of Object.entries(cells)) {
        assertRenderValue(cellValue, `${rowPath}.cells.${cellKey}`);
      }
    }
  }

  const validation = arrayAt(envelope.validation, "envelope.validation");
  for (const [index, issueValue] of validation.entries()) {
    const path = `envelope.validation[${index}]`;
    const issue = objectAt(issueValue, path);
    stringAt(issue.field_path, `${path}.field_path`);
    stringAt(issue.code, `${path}.code`);
    stringAt(issue.message, `${path}.message`);
    stringAt(issue.rule_version, `${path}.rule_version`);
    if (issue.severity !== "error" && issue.severity !== "warning") {
      invalid(`${path}.severity`, "expected error or warning");
    }
  }

  if (formCode === "0605" && formVersion === "1999") {
    assert0605Envelope(fields, schedules, period);
  } else if (formCode === "0619E" && formVersion === "2018") {
    assert0619EEnvelope(fields, schedules, period);
  } else if (formCode === "0619F" && formVersion === "2018") {
    assert0619FEnvelope(fields, schedules, period);
  } else if (formCode === "1701Q" && formVersion === "2018") {
    assert1701QPreviewEnvelope(fields, schedules, period);
  } else if (formCode === "1701" && formVersion === "2018") {
    assert1701PreviewEnvelope(fields, schedules, period);
  } else if (formCode === "1702RT" && formVersion === "2018C") {
    assert1702RTPreviewEnvelope(fields, schedules, period);
  } else if (formCode === "1702MX" && formVersion === "2018C") {
    assert1702MXPreviewEnvelope(fields, schedules, period);
  } else if (formCode === "2550Q" && formVersion === "2024") {
    assert2550QPreviewEnvelope(fields, schedules, period);
  } else if (formCode === "2551Q" && formVersion === "2018") {
    assert2551QEnvelope(fields, schedules, period);
  } else if (formCode === "1601C" && formVersion === "2018") {
    assert1601CEnvelope(fields, schedules, period);
  }
}

function assertFixedAnnualEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject,
  formId: string,
  requiredFields: Readonly<Record<string, RenderValueType>>
) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", `${formId} year-end month must be between 1 and 12`);
  }
  if (period.quarter !== undefined && period.quarter !== null) {
    invalid("envelope.period.quarter", `${formId} is annual and must not carry a quarter`);
  }
  if (schedules.length !== 0) {
    invalid(
      "envelope.schedules",
      `${formId} uses fixed official schedule capacities; continuation schedules are not certified`
    );
  }
  for (const [key, expectedType] of Object.entries(requiredFields)) {
    assertRequiredField(fields, key, expectedType);
  }
}

function assert1701PreviewEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject
) {
  assertFixedAnnualEnvelope(fields, schedules, period, "1701v2018", REQUIRED_1701_PREVIEW_FIELDS);
}

function assert1702RTPreviewEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject
) {
  assertFixedAnnualEnvelope(
    fields,
    schedules,
    period,
    "1702RTv2018C",
    REQUIRED_1702RT_PREVIEW_FIELDS
  );
}

function assert1702MXPreviewEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject
) {
  assertFixedAnnualEnvelope(
    fields,
    schedules,
    period,
    "1702MXv2018C",
    REQUIRED_1702MX_PREVIEW_FIELDS
  );
  if (renderText(fields.mandatory_attachment_document_kind) !== "separate_two_page_conditional_companion") {
    invalid(
      "envelope.fields.mandatory_attachment_document_kind.value",
      "expected the reviewed separate two-page attachment identity"
    );
  }
  if (renderInteger(fields.mandatory_attachment_page_count) !== 2) {
    invalid("envelope.fields.mandatory_attachment_page_count.value", "expected exactly two pages");
  }
  if (
    renderText(fields.mandatory_attachment_source_sha256) !==
    "36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26"
  ) {
    invalid(
      "envelope.fields.mandatory_attachment_source_sha256.value",
      "does not match the reviewed attachment source"
    );
  }
  const transport = objectAt(
    fields.mandatory_attachment_transport_supported,
    "envelope.fields.mandatory_attachment_transport_supported"
  );
  if (transport.value !== false) {
    invalid(
      "envelope.fields.mandatory_attachment_transport_supported.value",
      "attachment transport must remain fail-closed"
    );
  }
}

function assert2550QPreviewEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject
) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", "2550Q year-end month must be between 1 and 12");
  }
  const quarter = integerAt(period.quarter, "envelope.period.quarter");
  if (quarter < 1 || quarter > 4) {
    invalid("envelope.period.quarter", "2550Q quarter must be between 1 and 4");
  }
  if (schedules.length !== 0) {
    invalid(
      "envelope.schedules",
      "2550Qv2024 uses exact fixed two-row schedule fields and no continuation schedule"
    );
  }
  for (const [key, expectedType] of Object.entries(REQUIRED_2550Q_PREVIEW_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  if (!["calendar", "fiscal"].includes(renderText(fields.filing_basis))) {
    invalid("envelope.fields.filing_basis.value", "expected calendar or fiscal");
  }
  if (
    fields.taxpayer_classification !== undefined &&
    !["micro", "small", "medium", "large"].includes(
      renderText(fields.taxpayer_classification)
    )
  ) {
    invalid(
      "envelope.fields.taxpayer_classification.value",
      "expected micro, small, medium, or large"
    );
  }
  for (const key of ["return_period_from", "return_period_to"]) {
    const value = renderText(fields[key]);
    if (value !== "" && !/^\d{1,2}\/\d{2}\/\d{4}$/.test(value)) {
      invalid(`envelope.fields.${key}.value`, "expected blank or M(M)/DD/YYYY");
    }
  }

  for (const row of [1, 2]) {
    for (const suffix of ["date", "source", "description"]) {
      assertRequiredField(fields, `schedule_1_${row}_${suffix}`, "text");
    }
    for (const suffix of ["from", "to", "agent"]) {
      assertRequiredField(fields, `schedule_3_${row}_${suffix}`, "text");
    }
    for (const suffix of ["from", "to", "miller", "taxpayer", "receipt"]) {
      assertRequiredField(fields, `schedule_4_${row}_${suffix}`, "text");
    }
  }
  for (const key of Object.keys(fields)) {
    if (/^schedule_(?:1|3|4)_[3-9]\d*_/.test(key)) {
      invalid(
        `envelope.fields.${key}`,
        "2550Q exact renderer capacity is two rows; attachment pages are not certified"
      );
    }
  }

  for (const key of [
    "tax_relief_details",
    "item_19_description",
    "item_42_description",
    "item_47_description",
    "item_56_description",
    "signature_taxpayer_or_representative",
    "signature_representative_title",
    "signature_non_individual_officer",
    "machine_validation_or_receipt_details"
  ]) {
    maximumCharactersAt(renderText(fields[key]), 320, `envelope.fields.${key}.value`);
  }
}

function assert0605Envelope(fields: JsonObject, schedules: unknown[], period: JsonObject) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", "0605 persistence slot must be between 1 and 12");
  }
  const quarter = integerAt(period.quarter, "envelope.period.quarter");
  if (quarter < 1 || quarter > 4) {
    invalid("envelope.period.quarter", "0605 quarter must be between 1 and 4");
  }
  if (schedules.length !== 0) {
    invalid("envelope.schedules", "0605v1999 has no repeatable renderer schedule");
  }
  for (const [key, expectedType] of Object.entries(REQUIRED_0605_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  if (!["calendar", "fiscal"].includes(renderText(fields.filing_basis))) {
    invalid("envelope.fields.filing_basis.value", "expected calendar or fiscal");
  }
  const yearEndMonth = renderInteger(fields.year_end_month);
  if (yearEndMonth < 1 || yearEndMonth > 12) {
    invalid("envelope.fields.year_end_month.value", "expected a month from 1 to 12");
  }
  if (!["individual", "non_individual"].includes(renderText(fields.taxpayer_classification))) {
    invalid(
      "envelope.fields.taxpayer_classification.value",
      "expected individual or non_individual"
    );
  }
  if (![
    "",
    "self_assessment",
    "tax_deposit",
    "income_tax_second_installment",
    "penalties",
    "others",
    "assessment_or_deficiency",
    "accounts_receivable_or_delinquent"
  ].includes(renderText(fields.manner_of_payment))) {
    invalid("envelope.fields.manner_of_payment.value", "unsupported Item 17 choice");
  }
  if (!["", "installment", "partial", "full"].includes(renderText(fields.type_of_payment))) {
    invalid("envelope.fields.type_of_payment.value", "unsupported Item 18 choice");
  }
  if (!["none", "xml_option_1", "xml_option_2"].includes(renderText(fields.approval_selection))) {
    invalid(
      "envelope.fields.approval_selection.value",
      "unsupported preserved XML approval flag"
    );
  }

  optionalDateAt(renderText(fields.due_date), "envelope.fields.due_date.value");
  optionalDateAt(renderText(fields.return_period), "envelope.fields.return_period.value");
  maximumCharactersAt(renderText(fields.atc), 16, "envelope.fields.atc.value");
  maximumCharactersAt(
    renderText(fields.tax_type_code),
    8,
    "envelope.fields.tax_type_code.value"
  );
  for (const key of [
    "line_of_business",
    "other_manner_description",
    "signature_taxpayer_or_representative",
    "signature_title_or_position",
    "signature_head_of_office",
    "machine_validation_or_receipt_details"
  ]) {
    maximumCharactersAt(renderText(fields[key]), 320, `envelope.fields.${key}.value`);
  }
  for (const item of ["24", "25", "26"]) {
    maximumCharactersAt(
      renderText(fields[`payment_${item}_drawee_bank_or_agency`]),
      160,
      `envelope.fields.payment_${item}_drawee_bank_or_agency.value`
    );
    maximumCharactersAt(
      renderText(fields[`payment_${item}_number`]),
      160,
      `envelope.fields.payment_${item}_number.value`
    );
    optionalDateAt(
      renderText(fields[`payment_${item}_date`]),
      `envelope.fields.payment_${item}_date.value`
    );
  }
}

function assert1701QPreviewEnvelope(
  fields: JsonObject,
  schedules: unknown[],
  period: JsonObject
) {
  const quarter = integerAt(period.quarter, "envelope.period.quarter");
  if (quarter < 1 || quarter > 3) {
    invalid("envelope.period.quarter", "1701Q quarter must be 1, 2, or 3");
  }
  if (period.month !== undefined) {
    invalid("envelope.period.month", "1701Q does not accept a month");
  }
  if (schedules.length !== 0) {
    invalid("envelope.schedules", "1701Q has no repeatable renderer schedule");
  }
  for (const [key, expectedType] of Object.entries(REQUIRED_1701Q_PREVIEW_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }
  if (fields.filer_type !== undefined && !["single_proprietor", "professional", "estate", "trust"].includes(renderText(fields.filer_type))) {
    invalid("envelope.fields.filer_type.value", "unsupported 1701Q filer type");
  }
  if (fields.atc !== undefined && !["II012", "II013", "II014", "II015", "II016", "II017"].includes(renderText(fields.atc))) {
    invalid("envelope.fields.atc.value", "unsupported 1701Q taxpayer ATC");
  }
  if (fields.tax_rate_choice !== undefined && !["graduated", "eight_percent"].includes(renderText(fields.tax_rate_choice))) {
    invalid("envelope.fields.tax_rate_choice.value", "expected graduated or eight_percent");
  }
  if (fields.deduction_method !== undefined && !["itemized", "osd"].includes(renderText(fields.deduction_method))) {
    invalid("envelope.fields.deduction_method.value", "expected itemized or osd");
  }
  if (!/^\d{1,2}$/.test(renderText(fields.attached_sheets))) {
    invalid("envelope.fields.attached_sheets.value", "expected one or two digits");
  }
}

function assert0619FEnvelope(fields: JsonObject, schedules: unknown[], period: JsonObject) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", "0619F month must be between 1 and 12");
  }
  if (period.quarter !== undefined) {
    invalid("envelope.period.quarter", "0619F does not accept a quarter");
  }
  if (schedules.length !== 0) {
    invalid("envelope.schedules", "0619F has no repeatable renderer schedule");
  }

  for (const [key, expectedType] of Object.entries(REQUIRED_0619F_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  if (renderText(fields.item_13_atc) !== "WMF10") {
    invalid("envelope.fields.item_13_atc.value", "0619F requires fixed Item 13 ATC WMF10");
  }
  if (renderText(fields.item_14_atc) !== "WMF20") {
    invalid("envelope.fields.item_14_atc.value", "0619F requires fixed Item 14 ATC WMF20");
  }
  if (renderText(fields.tax_type_code) !== "WB") {
    invalid("envelope.fields.tax_type_code.value", "0619F requires fixed tax type WB");
  }
  if (!["private", "government"].includes(renderText(fields.withholding_agent_category))) {
    invalid(
      "envelope.fields.withholding_agent_category.value",
      "expected private or government"
    );
  }

  optionalDateAt(renderText(fields.due_date), "envelope.fields.due_date.value");
  for (const key of [
    "line_of_business",
    "registered_address_2",
    "tax_agent_accreditation_number",
    "payment_23_particular"
  ]) {
    maximumCharactersAt(renderText(fields[key]), 160, `envelope.fields.${key}.value`);
  }
  optionalDateAt(
    renderText(fields.tax_agent_date_of_issue),
    "envelope.fields.tax_agent_date_of_issue.value"
  );
  optionalDateAt(
    renderText(fields.tax_agent_date_of_expiry),
    "envelope.fields.tax_agent_date_of_expiry.value"
  );

  for (const item of ["20", "21", "22", "23"]) {
    maximumCharactersAt(
      renderText(fields[`payment_${item}_drawee_bank_or_agency`]),
      160,
      `envelope.fields.payment_${item}_drawee_bank_or_agency.value`
    );
    maximumCharactersAt(
      renderText(fields[`payment_${item}_number`]),
      160,
      `envelope.fields.payment_${item}_number.value`
    );
    optionalDateAt(
      renderText(fields[`payment_${item}_date`]),
      `envelope.fields.payment_${item}_date.value`
    );
  }
}

function assert0619EEnvelope(fields: JsonObject, schedules: unknown[], period: JsonObject) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", "0619E month must be between 1 and 12");
  }
  if (period.quarter !== undefined) {
    invalid("envelope.period.quarter", "0619E does not accept a quarter");
  }
  if (schedules.length !== 0) {
    invalid("envelope.schedules", "0619E has no repeatable renderer schedule");
  }

  for (const [key, expectedType] of Object.entries(REQUIRED_0619E_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  if (renderText(fields.atc) !== "WME10") {
    invalid("envelope.fields.atc.value", "0619E requires fixed ATC WME10");
  }
  if (renderText(fields.tax_type_code) !== "WE") {
    invalid("envelope.fields.tax_type_code.value", "0619E requires fixed tax type WE");
  }
  if (!["private", "government"].includes(renderText(fields.withholding_agent_category))) {
    invalid(
      "envelope.fields.withholding_agent_category.value",
      "expected private or government"
    );
  }

  optionalDateAt(renderText(fields.due_date), "envelope.fields.due_date.value");
  maximumCharactersAt(
    renderText(fields.line_of_business),
    160,
    "envelope.fields.line_of_business.value"
  );
  maximumCharactersAt(
    renderText(fields.registered_address_2),
    160,
    "envelope.fields.registered_address_2.value"
  );
  maximumCharactersAt(
    renderText(fields.tax_agent_accreditation_number),
    160,
    "envelope.fields.tax_agent_accreditation_number.value"
  );
  optionalDateAt(
    renderText(fields.tax_agent_date_of_issue),
    "envelope.fields.tax_agent_date_of_issue.value"
  );
  optionalDateAt(
    renderText(fields.tax_agent_date_of_expiry),
    "envelope.fields.tax_agent_date_of_expiry.value"
  );
  maximumCharactersAt(
    renderText(fields.payment_22_particular),
    160,
    "envelope.fields.payment_22_particular.value"
  );

  for (const item of ["19", "20", "21", "22"]) {
    maximumCharactersAt(
      renderText(fields[`payment_${item}_drawee_bank_or_agency`]),
      160,
      `envelope.fields.payment_${item}_drawee_bank_or_agency.value`
    );
    maximumCharactersAt(
      renderText(fields[`payment_${item}_number`]),
      160,
      `envelope.fields.payment_${item}_number.value`
    );
    optionalDateAt(
      renderText(fields[`payment_${item}_date`]),
      `envelope.fields.payment_${item}_date.value`
    );
  }
}

function assert1601CEnvelope(fields: JsonObject, schedules: unknown[], period: JsonObject) {
  const month = integerAt(period.month, "envelope.period.month");
  if (month < 1 || month > 12) {
    invalid("envelope.period.month", "1601C month must be between 1 and 12");
  }
  if (period.quarter !== undefined) {
    invalid("envelope.period.quarter", "1601C does not accept a quarter");
  }

  for (const [key, expectedType] of Object.entries(REQUIRED_1601C_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  const atc = renderText(fields.atc);
  if (atc !== "WW010") {
    invalid(
      "envelope.fields.atc.value",
      "1601C January 2018 requires the fixed Item 5 ATC WW010"
    );
  }
  maximumCharactersAt(
    renderText(fields.line_of_business),
    160,
    "envelope.fields.line_of_business.value"
  );
  maximumCharactersAt(
    renderText(fields.registered_address_2),
    160,
    "envelope.fields.registered_address_2.value"
  );
  maximumCharactersAt(
    renderText(fields.tax_relief_specification),
    160,
    "envelope.fields.tax_relief_specification.value"
  );
  maximumCharactersAt(
    renderText(fields.tax_20_other_name),
    160,
    "envelope.fields.tax_20_other_name.value"
  );
  maximumCharactersAt(
    renderText(fields.tax_29_other_remittances_name),
    160,
    "envelope.fields.tax_29_other_remittances_name.value"
  );

  const agentCategory = renderText(fields.category_of_agent);
  if (agentCategory !== "P" && agentCategory !== "G") {
    invalid("envelope.fields.category_of_agent.value", "expected P or G");
  }

  if (schedules.length !== 1) {
    invalid("envelope.schedules", "1601C requires exactly one schedule_1");
  }
  const schedule = objectAt(schedules[0], "envelope.schedules[0]");
  if (schedule.id !== "schedule_1") {
    invalid("envelope.schedules[0].id", "1601C requires schedule_1");
  }
  const rows = arrayAt(schedule.rows, "envelope.schedules[0].rows");
  if (rows.length > 3) {
    invalid("envelope.schedules[0].rows", "1601C verified Schedule I capacity is three rows");
  }
  for (const [rowIndex, rowValue] of rows.entries()) {
    const rowPath = `envelope.schedules[0].rows[${rowIndex}]`;
    const cells = objectAt(objectAt(rowValue, rowPath).cells, `${rowPath}.cells`);
    for (const [key, expectedType] of Object.entries(REQUIRED_1601C_SCHEDULE_CELLS)) {
      const cell = cells[key];
      if (cell === undefined) invalid(`${rowPath}.cells.${key}`, "required cell is missing");
      assertRenderValueType(cell, `${rowPath}.cells.${key}`, expectedType);
    }
    maximumCharactersAt(
      renderText(cells.previous_month),
      7,
      `${rowPath}.cells.previous_month.value`
    );
    maximumCharactersAt(renderText(cells.date_paid), 10, `${rowPath}.cells.date_paid.value`);
    maximumCharactersAt(
      renderText(cells.drawee_bank_code_or_agency),
      160,
      `${rowPath}.cells.drawee_bank_code_or_agency.value`
    );
    maximumCharactersAt(
      renderText(cells.payment_number),
      160,
      `${rowPath}.cells.payment_number.value`
    );
  }
}

function assert2551QEnvelope(fields: JsonObject, schedules: unknown[], period: JsonObject) {
  integerAt(period.month, "envelope.period.month");
  integerAt(period.quarter, "envelope.period.quarter");

  for (const [key, expectedType] of Object.entries(REQUIRED_2551Q_FIELDS)) {
    assertRequiredField(fields, key, expectedType);
  }

  maximumCharactersAt(
    renderText(fields.tax_relief_specification),
    160,
    "envelope.fields.tax_relief_specification.value"
  );

  const basis = renderText(fields.tax_period_basis);
  if (basis !== "calendar" && basis !== "fiscal") {
    invalid("envelope.fields.tax_period_basis.value", "expected calendar or fiscal");
  }
  const item13 = renderText(fields.item_13_election);
  if (!["unanswered", "not_applicable", "graduated", "eight_percent"].includes(item13)) {
    invalid("envelope.fields.item_13_election.value", "unexpected Item 13 election");
  }
  const disposition = renderText(fields.overpayment_disposition);
  if (!["none", "refund", "tax_credit_certificate"].includes(disposition)) {
    invalid("envelope.fields.overpayment_disposition.value", "unexpected overpayment disposition");
  }

  if (schedules.length !== 1) {
    invalid("envelope.schedules", "2551Q requires exactly one schedule_1");
  }
  const schedule = objectAt(schedules[0], "envelope.schedules[0]");
  if (schedule.id !== "schedule_1") {
    invalid("envelope.schedules[0].id", "2551Q requires schedule_1");
  }

  const columns = arrayAt(schedule.columns, "envelope.schedules[0].columns");
  const columnKeys = columns.map((column, index) =>
    stringAt(objectAt(column, `envelope.schedules[0].columns[${index}]`).key,
      `envelope.schedules[0].columns[${index}].key`)
  );
  for (const key of Object.keys(REQUIRED_2551Q_SCHEDULE_CELLS)) {
    if (!columnKeys.includes(key)) {
      invalid("envelope.schedules[0].columns", `missing required column ${key}`);
    }
  }

  const rows = arrayAt(schedule.rows, "envelope.schedules[0].rows");
  for (const [rowIndex, rowValue] of rows.entries()) {
    const rowPath = `envelope.schedules[0].rows[${rowIndex}]`;
    const cells = objectAt(objectAt(rowValue, rowPath).cells, `${rowPath}.cells`);
    for (const [key, expectedType] of Object.entries(REQUIRED_2551Q_SCHEDULE_CELLS)) {
      const cell = cells[key];
      if (cell === undefined) invalid(`${rowPath}.cells.${key}`, "required cell is missing");
      assertRenderValueType(cell, `${rowPath}.cells.${key}`, expectedType);
    }
  }

  const subtotal = fields.schedule_1_page_2_subtotal;
  if (rows.length > 6) {
    assertRequiredField(fields, "schedule_1_page_2_subtotal", "decimal");
  } else if (subtotal !== undefined) {
    invalid(
      "envelope.fields.schedule_1_page_2_subtotal",
      "subtotal is allowed only when Schedule 1 exceeds six rows"
    );
  }
}

function assertRequiredField(fields: JsonObject, key: string, expectedType: RenderValueType) {
  const value = fields[key];
  if (value === undefined) invalid(`envelope.fields.${key}`, "required field is missing");
  assertRenderValueType(value, `envelope.fields.${key}`, expectedType);
}

function assertRenderValueType(value: unknown, path: string, expectedType: RenderValueType) {
  const renderValue = objectAt(value, path);
  if (renderValue.type !== expectedType) {
    invalid(`${path}.type`, `expected ${expectedType}`);
  }
  assertRenderValue(renderValue, path);
}

function assertRenderValue(value: unknown, path: string) {
  const renderValue = objectAt(value, path);
  switch (renderValue.type) {
    case "text":
      stringAt(renderValue.value, `${path}.value`);
      return;
    case "boolean":
      if (typeof renderValue.value !== "boolean") {
        invalid(`${path}.value`, "expected boolean");
      }
      return;
    case "integer":
      integerAt(renderValue.value, `${path}.value`);
      return;
    case "decimal":
      finiteNumberAt(renderValue.value, `${path}.value`);
      return;
    default:
      invalid(`${path}.type`, "expected text, boolean, integer, or decimal");
  }
}

function renderText(value: unknown): string {
  return stringAt(objectAt(value, "render text").value, "render text.value");
}

function renderInteger(value: unknown): number {
  return integerAt(objectAt(value, "render integer").value, "render integer.value");
}

function objectAt(value: unknown, path: string): JsonObject {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    invalid(path, "expected object");
  }
  return value as JsonObject;
}

function arrayAt(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) invalid(path, "expected array");
  return value;
}

function stringAt(value: unknown, path: string): string {
  if (typeof value !== "string") invalid(path, "expected string");
  return value;
}

function nonEmptyStringAt(value: unknown, path: string): string {
  const text = stringAt(value, path);
  if (text.length === 0) invalid(path, "expected non-empty string");
  return text;
}

function maximumCharactersAt(value: unknown, maximum: number, path: string): string {
  const text = stringAt(value, path);
  const count = Array.from(text).length;
  if (count > maximum) {
    invalid(path, `requires ${count} characters but renderer capacity is ${maximum}`);
  }
  return text;
}

function finiteNumberAt(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    invalid(path, "expected finite number");
  }
  return value;
}

function integerAt(value: unknown, path: string): number {
  const integer = finiteNumberAt(value, path);
  if (!Number.isSafeInteger(integer)) invalid(path, "expected safe integer");
  return integer;
}

function optionalIntegerAt(value: unknown, path: string) {
  if (value !== undefined && value !== null) integerAt(value, path);
}

function optionalDateAt(value: string, path: string) {
  if (value !== "" && !/^\d{2}\/\d{2}\/\d{4}$/.test(value)) {
    invalid(path, "expected blank or MM/DD/YYYY");
  }
}

function invalid(path: string, message: string): never {
  throw new Error(`Invalid render contract at ${path}: ${message}`);
}
