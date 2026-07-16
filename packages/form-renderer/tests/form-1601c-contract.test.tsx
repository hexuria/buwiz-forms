import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import capacityFixture from "../../form-contracts/fixtures/1601c-3-rows.json";
import longFixture from "../../form-contracts/fixtures/1601c-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1601c-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1601c-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1601c-validation-edge.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  capacityFixture
] as const;

describe("1601C:2018 runtime render contract", () => {
  it("accepts and renders every Rust-generated fixture as exactly two folio pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(2);
      expect(markup).toContain("Part IV - Schedule");
    }
  });

  it("prints Rust-owned schedule values and Item 26 without calculating in React", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain('data-row-key="schedule-1-1"');
    expect(markup).toContain("PAY-1601C-001");
    expect(markup).toContain('data-item="26"');
    expect(fixture.fields.tax_26_adjustment).toEqual({ type: "decimal", value: 1100 });
  });

  it("renders long profile and schedule values through reviewed plain boxes", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email
    ]) {
      expect(markup).toContain(`aria-label="${value.toUpperCase()}"`);
    }
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(4);
  });

  it("fails closed on missing legal fields or rows beyond verified capacity", () => {
    const missing = structuredClone(normalFixture) as Record<string, any>;
    delete missing.fields.tax_36_total_amount_payable;
    expect(() => assertRenderEnvelope(missing)).toThrow(
      "tax_36_total_amount_payable"
    );

    const overflow = structuredClone(capacityFixture) as Record<string, any>;
    overflow.schedules[0].rows.push(structuredClone(overflow.schedules[0].rows[0]));
    overflow.schedules[0].rows[3].key = "schedule-1-4";
    expect(() => assertRenderEnvelope(overflow)).toThrow(
      "verified Schedule I capacity is three rows"
    );
  });
});
