import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FormDocument } from "../src/FormDocument";
import continuationFixture from "../../form-contracts/fixtures/2551q-10-rows.json";
import sixRowFixture from "../../form-contracts/fixtures/2551q-6-rows.json";
import fiscalFixture from "../../form-contracts/fixtures/2551q-fiscal-period.json";
import item13Fixture from "../../form-contracts/fixtures/2551q-item13-eight-percent.json";
import longValuesFixture from "../../form-contracts/fixtures/2551q-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/2551q-minimum.json";
import normalFixture from "../../form-contracts/fixtures/2551q-normal.json";
import refundFixture from "../../form-contracts/fixtures/2551q-overpayment-refund.json";
import tccFixture from "../../form-contracts/fixtures/2551q-overpayment-tcc.json";
import reliefFixture from "../../form-contracts/fixtures/2551q-tax-relief.json";
import validationEdgeFixture from "../../form-contracts/fixtures/2551q-validation-edge.json";

const canonicalFixtures = [
  continuationFixture,
  sixRowFixture,
  fiscalFixture,
  item13Fixture,
  longValuesFixture,
  minimumFixture,
  normalFixture,
  refundFixture,
  tccFixture,
  reliefFixture,
  validationEdgeFixture
] as const;

function validEnvelope(): Record<string, any> {
  return structuredClone(continuationFixture) as Record<string, any>;
}

describe("2551Q runtime render contract", () => {
  it("accepts every Rust-generated canonical fixture", () => {
    for (const fixture of canonicalFixtures) {
      expect(() => assertRenderEnvelope(structuredClone(fixture))).not.toThrow();
    }
  });

  it("actually renders every Rust-generated canonical fixture", () => {
    for (const fixture of canonicalFixtures) {
      const markup = renderToStaticMarkup(
        createElement(FormDocument, {
          envelope: structuredClone(fixture) as RenderEnvelope
        })
      );
      const expectedPageCount = fixture.schedules[0].rows.length > 6 ? 3 : 2;
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(expectedPageCount);
    }
  });

  it("keeps official header Item 5 and background Item 13 in the rendered form", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, {
        envelope: structuredClone(item13Fixture) as RenderEnvelope
      })
    );

    expect(markup).toContain('<div class="option-label"><b>5</b> Number of Sheet/s</div>');
    expect(markup).toContain('<div class="income-rate-field"><b>13</b>');
    expect(markup).toContain('8% income tax rate on gross sales/receipts/others');
  });

  it("rejects a missing required legal field instead of defaulting it", () => {
    const envelope = validEnvelope();
    delete envelope.fields.is_amended;

    expect(() => assertRenderEnvelope(envelope)).toThrow(
      "envelope.fields.is_amended"
    );

    const missingQuarter = validEnvelope();
    delete missingQuarter.period.quarter;
    expect(() => assertRenderEnvelope(missingQuarter)).toThrow(
      "envelope.period.quarter"
    );
  });

  it("rejects wrong field types and non-finite decimals", () => {
    const wrongType = validEnvelope();
    wrongType.fields.tax_relief = { type: "text", value: "false" };
    expect(() => assertRenderEnvelope(wrongType)).toThrow("expected boolean");

    const nonFinite = validEnvelope();
    nonFinite.fields.total_tax_due.value = Number.NaN;
    expect(() => assertRenderEnvelope(nonFinite)).toThrow("expected finite number");
  });

  it("accepts values that overflow official combs and renders plain text boxes", () => {
    const envelope = validEnvelope();
    envelope.taxpayer.name = "N".repeat(41);
    envelope.taxpayer.registered_address = "A".repeat(72);
    envelope.taxpayer.contact_number = "1".repeat(13);
    envelope.taxpayer.email = "E".repeat(29);
    envelope.fields.tax_relief_specification.value = "R".repeat(27);

    expect(() => assertRenderEnvelope(envelope)).not.toThrow();
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: envelope as RenderEnvelope })
    );
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThanOrEqual(6);
    expect(markup).toContain(`aria-label="${"A".repeat(72)}"`);
  });

  it("renders the Rust-owned long-value fixture without truncation", () => {
    const fixture = structuredClone(longValuesFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const relief = fixture.fields.tax_relief_specification;
    if (relief.type !== "text") throw new Error("fixture relief must be text");

    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email,
      relief.value
    ]) {
      expect(markup).toContain(`aria-label="${value.toUpperCase()}"`);
    }
  });

  it("fails closed beyond defensive document-rendering limits", () => {
    for (const [path, mutate] of [
      ["taxpayer.name", (value: Record<string, any>) => {
        value.taxpayer.name = "N".repeat(161);
      }],
      ["taxpayer.registered_address", (value: Record<string, any>) => {
        value.taxpayer.registered_address = "A".repeat(321);
      }],
      ["taxpayer.contact_number", (value: Record<string, any>) => {
        value.taxpayer.contact_number = "1".repeat(33);
      }],
      ["taxpayer.email", (value: Record<string, any>) => {
        value.taxpayer.email = "E".repeat(255);
      }],
      ["tax_relief_specification", (value: Record<string, any>) => {
        value.fields.tax_relief_specification.value = "R".repeat(161);
      }]
    ] as const) {
      const envelope = validEnvelope();
      mutate(envelope);
      expect(() => assertRenderEnvelope(envelope), path).toThrow("renderer capacity");
    }
  });

  it("requires every typed Schedule 1 cell and stable unique row key", () => {
    const missingCell = validEnvelope();
    delete missingCell.schedules[0].rows[0].cells.tax_due;
    expect(() => assertRenderEnvelope(missingCell)).toThrow(
      "rows[0].cells.tax_due"
    );

    const duplicateKey = validEnvelope();
    duplicateKey.schedules[0].rows[1].key = duplicateKey.schedules[0].rows[0].key;
    expect(() => assertRenderEnvelope(duplicateKey)).toThrow("duplicate row key");
  });

  it("requires the Rust-owned subtotal exactly when continuation exists", () => {
    const missingSubtotal = validEnvelope();
    delete missingSubtotal.fields.schedule_1_page_2_subtotal;
    expect(() => assertRenderEnvelope(missingSubtotal)).toThrow(
      "schedule_1_page_2_subtotal"
    );

    const base = validEnvelope();
    base.schedules[0].rows = base.schedules[0].rows.slice(0, 6);
    expect(() => assertRenderEnvelope(base)).toThrow(
      "subtotal is allowed only"
    );
  });
});
