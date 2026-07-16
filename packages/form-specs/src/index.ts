export interface PaperToken {
  name: "bir-folio" | "letter" | "legal";
  widthPt: number;
  heightPt: number;
}

export interface SchedulePolicy {
  minimumRows: number;
  firstPageRows: number;
  continuationPageRows: number;
  repeatHeader: boolean;
  finalTotalsOnLastPage: boolean;
}

export interface FormSpec {
  code: string;
  revision: string;
  title: string;
  paper: PaperToken;
  expectedBasePageCount: number;
  schedules: Record<string, SchedulePolicy>;
}

export const FORM_MIGRATION_SCHEMA_VERSION = 3 as const;

export type HtmlFormRoute = "disabled" | "experimental" | "html_only";

export interface FormMigrationCapabilities {
  typed_model: boolean;
  xml_round_trip: boolean;
  formula_evidence: boolean;
  persistence: boolean;
  queue_submission: boolean;
  editor: boolean;
  render_contract: boolean;
  html_component: boolean;
  html_spec: boolean;
  pagination: boolean;
  visual_parity: boolean;
  native_preview: boolean;
  native_print: boolean;
  pdf_export: boolean;
  packaged_offline: boolean;
}

export interface FormMigrationRecordV3 {
  code: string;
  revision: string;
  form_id: string;
  support_level: "ImplementedInApp" | "ScaffoldOnly";
  route: HtmlFormRoute;
  capabilities: FormMigrationCapabilities;
  release_ready: boolean;
}

export interface FormMigrationManifestV3 {
  schema_version: typeof FORM_MIGRATION_SCHEMA_VERSION;
  forms: FormMigrationRecordV3[];
}

export type FormKey = `${string}:${string}`;

export function formKey(code: string, revision: string): FormKey {
  return `${code}:${revision}`;
}

export const BIR_FOLIO: PaperToken = {
  name: "bir-folio",
  widthPt: 612,
  heightPt: 936
};

export const BIR_LETTER: PaperToken = {
  name: "letter",
  widthPt: 612,
  heightPt: 792
};

export const BIR_LEGAL: PaperToken = {
  name: "legal",
  widthPt: 612,
  heightPt: 1008
};

export const FORM_SPEC_REGISTRY: Readonly<Record<FormKey, FormSpec>> = {
  "0605:1999": {
    code: "0605",
    revision: "1999",
    title: "Payment Form",
    paper: BIR_FOLIO,
    expectedBasePageCount: 2,
    schedules: {}
  },
  "0619E:2018": {
    code: "0619E",
    revision: "2018",
    title: "Monthly Remittance Form of Creditable Income Taxes Withheld (Expanded)",
    paper: BIR_LETTER,
    expectedBasePageCount: 1,
    schedules: {}
  },
  "0619F:2018": {
    code: "0619F",
    revision: "2018",
    title: "Monthly Remittance Form of Final Income Taxes Withheld",
    paper: BIR_LETTER,
    expectedBasePageCount: 1,
    schedules: {}
  },
  "1601C:2018": {
    code: "1601C",
    revision: "2018",
    title: "Monthly Remittance Return of Income Taxes Withheld on Compensation",
    paper: BIR_FOLIO,
    expectedBasePageCount: 2,
    schedules: {
      schedule_1: {
        minimumRows: 3,
        firstPageRows: 3,
        continuationPageRows: 3,
        repeatHeader: true,
        finalTotalsOnLastPage: true
      }
    }
  },
  "1701Q:2018": {
    code: "1701Q",
    revision: "2018",
    title: "Quarterly Income Tax Return for Individuals, Estates and Trusts",
    paper: BIR_FOLIO,
    expectedBasePageCount: 2,
    schedules: {}
  },
  "1701:2018": {
    code: "1701",
    revision: "2018",
    title: "Annual Income Tax Return for Individuals, Estates and Trusts",
    paper: BIR_FOLIO,
    expectedBasePageCount: 4,
    schedules: {}
  },
  "1702RT:2018C": {
    code: "1702RT",
    revision: "2018C",
    title: "Annual Income Tax Return for Corporation, Partnership and Other Non-Individual Taxpayer Subject Only to Regular Income Tax Rate",
    paper: BIR_FOLIO,
    expectedBasePageCount: 4,
    schedules: {}
  },
  "1702MX:2018C": {
    code: "1702MX",
    revision: "2018C",
    title: "Annual Income Tax Return for Corporation, Partnership and Other Non-Individual Taxpayer with Mixed Income Subject to Multiple Tax Rates",
    paper: BIR_FOLIO,
    expectedBasePageCount: 4,
    schedules: {}
  },
  "2550Q:2024": {
    code: "2550Q",
    revision: "2024",
    title: "Quarterly Value-Added Tax Return",
    paper: BIR_LEGAL,
    expectedBasePageCount: 2,
    schedules: {}
  },
  "2551Q:2018": {
    code: "2551Q",
    revision: "2018",
    title: "Quarterly Percentage Tax Return",
    paper: BIR_FOLIO,
    expectedBasePageCount: 2,
    schedules: {
      schedule_1: {
        minimumRows: 6,
        firstPageRows: 6,
        continuationPageRows: 12,
        repeatHeader: true,
        finalTotalsOnLastPage: true
      }
    }
  }
};

export function getFormSpec(code: string, revision: string): FormSpec {
  const spec = FORM_SPEC_REGISTRY[formKey(code, revision)];
  if (!spec) {
    throw new Error(`No HTML form specification for ${code} revision ${revision}`);
  }
  return spec;
}

export function hasFormSpec(code: string, revision: string): boolean {
  return formKey(code, revision) in FORM_SPEC_REGISTRY;
}

export function listFormSpecs(): readonly FormSpec[] {
  return Object.values(FORM_SPEC_REGISTRY);
}
