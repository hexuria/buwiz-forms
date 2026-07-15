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

  // These are renderer safety capacities, not silent truncation rules. Values
  // that cannot fit the official fixed cells must fail at the untrusted
  // WebView boundary before React starts laying out a printable return.
  maximumCharactersAt(
    stringAt(taxpayer.tin, "envelope.taxpayer.tin").replace(/\D/g, ""),
    14,
    "envelope.taxpayer.tin"
  );
  maximumCharactersAt(taxpayer.name, 40, "envelope.taxpayer.name");
  maximumCharactersAt(taxpayer.rdo_code, 3, "envelope.taxpayer.rdo_code");
  maximumCharactersAt(
    taxpayer.registered_address,
    71,
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
    12,
    "envelope.taxpayer.contact_number"
  );
  maximumCharactersAt(taxpayer.email, 28, "envelope.taxpayer.email");

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

  if (formCode === "2551Q" && formVersion === "2018") {
    assert2551QEnvelope(fields, schedules, period);
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
    26,
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

function invalid(path: string, message: string): never {
  throw new Error(`Invalid render contract at ${path}: ${message}`);
}
