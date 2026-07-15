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

export const BIR_FOLIO: PaperToken = {
  name: "bir-folio",
  widthPt: 612,
  heightPt: 936
};

const FORM_SPECS: Record<string, FormSpec> = {
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
  const spec = FORM_SPECS[`${code}:${revision}`];
  if (!spec) {
    throw new Error(`No HTML form specification for ${code} revision ${revision}`);
  }
  return spec;
}
