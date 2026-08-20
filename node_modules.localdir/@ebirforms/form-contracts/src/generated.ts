/* Generated from Rust RenderEnvelopeV1. Do not edit directly. */

export type RenderValue =
  | {
      type: "text";
      value: string;
      [k: string]: unknown;
    }
  | {
      type: "boolean";
      value: boolean;
      [k: string]: unknown;
    }
  | {
      type: "integer";
      value: number;
      [k: string]: unknown;
    }
  | {
      type: "decimal";
      value: number;
      [k: string]: unknown;
    };
export type RenderAlignment = "left" | "center" | "right";
export type RenderValidationSeverity = "error" | "warning";

/**
 * Canonical eBIRForms renderer contract version 1.0
 */
export interface RenderEnvelopeV1 {
  fields: {
    [k: string]: RenderValue;
  };
  form: RenderFormIdentity;
  locale: string;
  period: RenderPeriod;
  schedules: RenderSchedule[];
  schema_version: string;
  taxpayer: RenderTaxpayer;
  validation: RenderValidationMessage[];
  [k: string]: unknown;
}
export interface RenderFormIdentity {
  code: string;
  version: string;
  [k: string]: unknown;
}
export interface RenderPeriod {
  label: string;
  month?: number | null;
  quarter?: number | null;
  taxable_year: number;
  [k: string]: unknown;
}
export interface RenderSchedule {
  columns: RenderColumn[];
  id: string;
  rows: RenderRow[];
  [k: string]: unknown;
}
export interface RenderColumn {
  alignment: RenderAlignment;
  key: string;
  label: string;
  [k: string]: unknown;
}
export interface RenderRow {
  cells: {
    [k: string]: RenderValue;
  };
  key: string;
  [k: string]: unknown;
}
export interface RenderTaxpayer {
  contact_number: string;
  email: string;
  name: string;
  rdo_code: string;
  registered_address: string;
  tin: string;
  zip_code: string;
  [k: string]: unknown;
}
export interface RenderValidationMessage {
  code: string;
  field_path: string;
  message: string;
  rule_version: string;
  severity: RenderValidationSeverity;
  [k: string]: unknown;
}
