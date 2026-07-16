import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import longFixture from "../../form-contracts/fixtures/1702rt-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1702rt-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1702rt-normal.json";
import capacityFixture from "../../form-contracts/fixtures/1702rt-schedule-capacity.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1702rt-validation-edge.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  capacityFixture
] as const;

describe("1702RT:2018C experimental preview contract", () => {
  it("accepts every Rust fixture and renders exactly four 612x936 pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(4);
      expect(markup.match(/data-paper="folio"/g)).toHaveLength(4);
      expect(markup).toContain("Part IV – Computation of Tax");
      expect(markup).toContain("Schedule V – Reconciliation of Net Income per Books Against Taxable Income");
    }
  });

  it("preserves Rust-owned whole-peso calculations as exact integer values", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    expect(fixture.fields.item_43).toEqual({ type: "integer", value: 358500 });
    expect(fixture.fields.item_55).toEqual({ type: "integer", value: 65000 });
    expect(fixture.fields.item_56).toEqual({ type: "integer", value: 293500 });
    expect(fixture.fields.schedule_1_item_18).toEqual({ type: "integer", value: 430000 });
    expect(fixture.schedules).toEqual([]);
  });

  it("renders the exact page-one Item 2, date, declaration, and title semantics", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );

    expect(markup).toContain('data-official-date-format="MM/20YY" aria-label="12/2026"');
    expect(markup).toContain('class="year-literal-1702rt">/20</span>');
    expect(markup.match(/data-official-date-format="MM\/DD\/YYYY"/g)).toHaveLength(5);
    expect(markup.match(/class="date-separator-1702rt">\/<\/span>/g)).toHaveLength(10);
    for (const date of ["12/10/2019", "04/15/2027", "04/16/2027", "04/17/2027"]) {
      expect(markup).toContain(`aria-label="${date}"`);
    }
    expect(markup).toContain(
      "(If signed by an Authorized Representative, indicate TIN and attach authorization letter)"
    );
    expect(markup.match(/data-signatory-binding="title"/g)).toHaveLength(2);
    expect(markup).toContain('data-signatory-binding="title">PRESIDENT</b>');
    expect(markup).toContain('data-signatory-binding="title">TREASURER</b>');
  });

  it("renders Items 25 and 26 with only their source-proven payment cells", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );

    expect(fixture.fields).not.toHaveProperty("payment_25_bank");
    expect(fixture.fields).not.toHaveProperty("payment_25_specification");
    expect(markup).toContain("payment-row-tax-debit-1702rt");
    expect(markup).toContain('data-payment-item="25" data-payment-fields="number,date,amount"');
    expect(markup).toContain('aria-label="TDM-1702RT-025"');
    expect(markup).toContain('data-payment-item-label="26"');
    expect(markup).toContain(
      'data-payment-item="26" data-payment-fields="specification,bank,number,date,amount"'
    );
    expect(markup).toContain('aria-label="OTHER REVIEWED PAYMENT"');
    expect(markup).toContain('aria-label="AUTHORIZED AGENT BANK 026"');
    expect(markup.indexOf('data-payment-item-label="26"')).toBeLessThan(
      markup.indexOf('data-payment-item="26"')
    );
  });

  it("prints only a Rust-reviewed alternate ATC description and fails closed otherwise", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(fixture.fields.atc_other_code).toEqual({ type: "text", value: "IC010" });
    expect(fixture.fields.atc_other_description).toEqual({
      type: "text",
      value: "CORPORATION IN GENERAL - JAN 1, 2009 (2009)"
    });
    expect(markup).toContain(
      'aria-label="CORPORATION IN GENERAL - JAN 1, 2009 (2009)"'
    );

    const missingEvidence = structuredClone(fixture) as Record<string, any>;
    missingEvidence.fields.atc_other_description = { type: "text", value: "" };
    expect(() => renderToStaticMarkup(
      createElement(FormDocument, { envelope: missingEvidence as RenderEnvelope })
    )).toThrow("requires a reviewed code and description");
  });

  it("fails closed instead of stripping malformed or unrepresentable dates", () => {
    const malformedDate = structuredClone(normalFixture) as Record<string, any>;
    malformedDate.fields.payment_23_date = { type: "text", value: "04152027" };
    expect(() => renderToStaticMarkup(
      createElement(FormDocument, { envelope: malformedDate as RenderEnvelope })
    )).toThrow("must use MM/DD/YYYY");

    const unrepresentableYear = structuredClone(normalFixture) as Record<string, any>;
    unrepresentableYear.period.taxable_year = 2100;
    expect(() => renderToStaticMarkup(
      createElement(FormDocument, { envelope: unrepresentableYear as RenderEnvelope })
    )).toThrow("MM/20YY cannot represent");
  });

  it("keeps unresolved ATC, deduction method, and rate visibly blank", () => {
    const fixture = structuredClone(validationEdgeFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(fixture.validation.length).toBeGreaterThan(3);
    expect(fixture.fields.deduction_method).toEqual({ type: "text", value: "unresolved" });
    const atcRegion = markup.match(/Alphanumeric Tax Code \(ATC\)[\s\S]{0,900}/)?.[0];
    expect(atcRegion).toBeDefined();
    expect(atcRegion).not.toContain("check-box checked");
  });

  it("preserves long valid identity and schedule descriptions in plain-box mode", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email,
      "A VALID OTHER DEDUCTION DESCRIPTION LONGER THAN THE OFFICIAL COMB CAPACITY",
      "REVIEWED SPECIAL LAW LEGAL BASIS WITH A LONG CAPTION"
    ]) {
      expect(markup).toContain(`aria-label="${value}"`);
    }
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(5);
  });

  it("renders every reviewed fixed-capacity schedule row without inventing continuation pages", () => {
    const fixture = structuredClone(capacityFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(fixture.fields.schedule_1_item_17i_description).toEqual({
      type: "text",
      value: "OTHER DEDUCTION 6"
    });
    expect(fixture.fields.schedule_2_item_4_description).toEqual({
      type: "text",
      value: "SPECIAL DEDUCTION 4"
    });
    expect(fixture.fields.schedule_5_item_3_description).toEqual({
      type: "text",
      value: "NON-DEDUCTIBLE EXPENSE 2"
    });
    expect(markup.match(/data-page-number=/g)).toHaveLength(4);
  });

  it("fails closed when a continuation renderer schedule is injected", () => {
    const fixture = structuredClone(normalFixture) as Record<string, any>;
    fixture.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture as RenderEnvelope })
    )).toThrow("1702RTv2018C uses fixed official schedule capacities");
  });
});
