import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import longFixture from "../../form-contracts/fixtures/0605-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/0605-minimum.json";
import normalFixture from "../../form-contracts/fixtures/0605-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/0605-validation-edge.json";
import variantFixture from "../../form-contracts/fixtures/0605-variant.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  variantFixture
] as const;

describe("0605:1999 runtime render contract", () => {
  it("accepts every Rust fixture and renders exactly two BIR Folio pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(2);
      expect(markup.match(/data-paper="folio"/g)).toHaveLength(2);
      expect(markup).toContain("Part III");
      expect(markup).toContain("Guidelines and Instructions");
    }
  });

  it("prints Rust-owned dates, reviewed codes, choices, and totals", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );

    expect(fixture.fields.atc).toEqual({ type: "text", value: "II011" });
    expect(fixture.fields.tax_type_code).toEqual({ type: "text", value: "IT" });
    expect(fixture.fields.item_20d_total_penalties).toEqual({
      type: "decimal",
      value: 4250
    });
    expect(fixture.fields.item_21_total_amount_payable).toEqual({
      type: "decimal",
      value: 129250
    });
    expect(markup).toContain("Taxpayer Identification No.");
    expect(markup).toContain("Total Amount Payable");
  });

  it("renders the official page-two reference tables semantically", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: normalFixture as RenderEnvelope })
    );
    expect(markup).toContain("NATURE OF PAYMENT");
    expect(markup).toContain("FP 010 - FP 930");
    expect(markup).toContain("Fines and Penalties");
    expect(markup).toContain("WITHHOLDING TAX-COMPENSATION");
    expect(markup).toContain("Who Shall File");
    expect(markup).not.toContain("barcode");
  });

  it("preserves long legal values with reviewed plain-box mode", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain(`aria-label="${fixture.taxpayer.name.toUpperCase()}"`);
    expect(markup).toContain(
      `aria-label="${fixture.taxpayer.registered_address.toUpperCase()}"`
    );
    expect(markup).toContain(
      "aria-label=\"SOFTWARE DEVELOPMENT, INFORMATION TECHNOLOGY CONSULTING"
    );
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(4);
  });

  it("distinguishes official blank payment amounts from entered zero", () => {
    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.payment_23_amount_present).toEqual({
      type: "boolean",
      value: false
    });

    const enteredZero = structuredClone(minimumFixture) as Record<string, any>;
    enteredZero.fields.payment_24_amount_present.value = true;
    enteredZero.fields.payment_24_amount.value = 0;
    expect(() => assertRenderEnvelope(enteredZero)).not.toThrow();
  });

  it("fails closed on missing fields, invented schedules, or invalid choices", () => {
    const missing = structuredClone(normalFixture) as Record<string, any>;
    delete missing.fields.item_21_total_amount_payable;
    expect(() => assertRenderEnvelope(missing)).toThrow(
      "item_21_total_amount_payable"
    );

    const schedule = structuredClone(normalFixture) as Record<string, any>;
    schedule.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => assertRenderEnvelope(schedule)).toThrow(
      "0605v1999 has no repeatable renderer schedule"
    );

    const basis = structuredClone(normalFixture) as Record<string, any>;
    basis.fields.filing_basis.value = "invented";
    expect(() => assertRenderEnvelope(basis)).toThrow("expected calendar or fiscal");
  });
});
