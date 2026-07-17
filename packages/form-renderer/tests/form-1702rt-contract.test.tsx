import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import longFixture from "../../form-contracts/fixtures/1702rt-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1702rt-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1702rt-normal.json";
import capacityFixture from "../../form-contracts/fixtures/1702rt-schedule-capacity.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1702rt-validation-edge.json";
import { FormDocument } from "../src/FormDocument";
import {
  OFFICIAL_1702RT_PDF417_MATRICES,
  OFFICIAL_1702RT_PDF417_PATHS,
  OFFICIAL_1702RT_PDF417_PAYLOADS
} from "../src/forms/official1702RTAssets";

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

  it("renders the exact native seal and all eight rows of every audited PDF417", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const expectedMatrixHashes = [
      "362d6c13fc51ae71f86168da68bd2dfeadee77d82a0c68f5234a006adfaf6200",
      "aaf9a1d313b3804a9ab4507407a98d8d5b32eb4711e8779cae8f1e2e7dbdc849",
      "3ad58b30bbd02dab3dcfe7b896c861669a6f21693f60a04250e9c4e83000fa5f",
      "94b3aaadd3c8dcfbfca4ba4fb64347ceef5d1a7693f7d7d96a9f4d1e9bf5a8db"
    ];
    const expectedPathHashes = [
      "82ad803cb29fb6886dd1bd0158c9528df4960c07016f7d807a1a54e6b761eb04",
      "352ade9e021b50d016521709013b6d278b130bf8cb2eaaec0a599e311cfd1dfc",
      "f8ca0babf7a0c25535927f096d9f7beeb68aaa9d1f4962c62b85158b0255f1b2",
      "e52b63b2670d75324b1724a2ae3cc627c024197c5dc20288419f1a2eb9525d4b"
    ];
    const pages = [1, 2, 3, 4] as const;
    const matrixHashes = pages.map((page) => {
      const rows = OFFICIAL_1702RT_PDF417_MATRICES[page];
      expect(rows).toHaveLength(8);
      expect(rows.every((row) => row.length === 120)).toBe(true);
      return createHash("sha256").update(rows.join("")).digest("hex");
    });
    const pathHashes = pages.map((page) =>
      createHash("sha256").update(OFFICIAL_1702RT_PDF417_PATHS[page]).digest("hex")
    );
    const sealBytes = readFileSync(fileURLToPath(
      new URL("../src/forms/assets/1702rt-seal.png", import.meta.url)
    ));

    expect(matrixHashes).toEqual(expectedMatrixHashes);
    expect(pathHashes).toEqual(expectedPathHashes);
    expect(createHash("sha256").update(sealBytes).digest("hex")).toBe(
      "50d1fc573146e251138b78074b5790dd569f6dbde335feea908334adef4dd7b0"
    );
    expect(markup.match(/viewBox="0 0 120 8"/g)).toHaveLength(4);
    expect(markup.match(/official-pdf417-object-1702rt/g)).toHaveLength(4);
    expect(markup.match(/<img /g)).toHaveLength(1);
    expect(markup).not.toContain("1702rt-barcode-page-");
    for (const page of pages) {
      const payload = OFFICIAL_1702RT_PDF417_PAYLOADS[page];
      expect(markup).toContain(`aria-label="${payload}"`);
      expect(markup).toContain(`<small>${payload}</small>`);
    }
  });

  it("renders the exact page-one Item 2, date, declaration, and title semantics", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );

    expect(markup).toContain('data-official-date-format="MM/20YY" aria-label="12/2026"');
    expect(markup).toContain('class="year-literal-1702rt">/20</span>');
    expect(markup.match(/data-official-date-format="MM\/DD\/YYYY"/g)).toHaveLength(5);
    expect(markup.match(/class="date-separator-1702rt">\/<\/span>/g)).toHaveLength(2);
    expect(markup.match(/class="payment-date-1702rt"/g)).toHaveLength(4);
    expect(markup.match(/data-segment-capacities="2,2,4"/g)).toHaveLength(4);
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

  it("preserves the official four-page order and audited field modes and capacities", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const orderedAnchors = [
      'data-page-number="1"',
      "Part I – Background Information",
      "Part III – Details of Payment",
      'data-page-number="2"',
      "Part IV – Computation of Tax",
      "Part V – Tax Relief Availment",
      'data-page-number="3"',
      "Schedule I – Ordinary Allowable Itemized Deductions",
      "Schedule II – Special Allowable Itemized Deductions",
      'data-page-number="4"',
      "Schedule III – Computation of Net Operating Loss Carry Over (NOLCO)",
      "Schedule V – Reconciliation of Net Income per Books Against Taxable Income"
    ];
    for (let index = 1; index < orderedAnchors.length; index += 1) {
      expect(markup.indexOf(orderedAnchors[index - 1])).toBeLessThan(
        markup.indexOf(orderedAnchors[index])
      );
    }

    expect(markup).toContain('class="stacked-field-1702rt" data-official-field-mode="guided" data-line-capacity="38"');
    expect(markup).toContain('class="address-block-1702rt" data-official-field-mode="guided" data-line-capacities="38,38,30"');
    expect(markup).toContain('data-cell-capacity="12"><span><b>11</b>Contact Number');
    expect(markup).toContain('data-cell-capacity="32"><span><b>12</b>Email Address');
    expect(markup).toContain('data-official-field-mode="plain" aria-label="IC010"');
    expect(markup.match(/class="payment-value-1702rt" data-official-field-mode="guided" data-cell-capacity="5"/g)).toHaveLength(2);
    expect(markup.match(/class="payment-value-1702rt" data-official-field-mode="guided" data-cell-capacity="7"/g)).toHaveLength(4);
    expect(markup).toContain('data-official-field-mode="plain" aria-label="OTHER REVIEWED PAYMENT"');
    expect(markup).toContain('data-official-field-mode="plain" aria-label="AUTHORIZED AGENT BANK 026"');
    expect(markup).toContain('data-other-description-capacity="23"');
    expect(markup).toContain('data-description-capacity="15" data-legal-basis-capacity="10"');
    expect(markup).toContain('data-description-capacity="24"');
  });

  it("keeps the official monetary guides for representable values and switches overflow to one plain field", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    fixture.fields.item_14 = { type: "integer", value: 1_234_567_890_123 };
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const itemFourteen = markup.match(/aria-label="1234567890123"[\s\S]{0,260}/)?.[0];
    expect(itemFourteen).toBeDefined();
    expect(itemFourteen).toContain('data-cell-capacity="12"');
    expect(itemFourteen).toContain('data-overflow-mode="plain"');
    expect(itemFourteen).toContain("1234567890123");
  });

  it("includes the official Part IV eligibility and cross-part references", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: structuredClone(minimumFixture) as RenderEnvelope })
    );
    expect(markup).toContain("[Only for those taxable under Sec. 27 (A to C); Sec. 28(A)(1)(A)(6)(b) of the Tax Code, as amended]");
    expect(markup).toContain("(To Part II Item 14)");
    expect(markup).toContain("(To Part II Item 15)");
    expect(markup).toContain("(To Part II Item 16)");
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
