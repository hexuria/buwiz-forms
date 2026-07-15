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
import minimumFixture from "../../form-contracts/fixtures/2551q-minimum.json";
import refundFixture from "../../form-contracts/fixtures/2551q-overpayment-refund.json";
import tccFixture from "../../form-contracts/fixtures/2551q-overpayment-tcc.json";
import reliefFixture from "../../form-contracts/fixtures/2551q-tax-relief.json";

const canonicalFixtures = [
  continuationFixture,
  sixRowFixture,
  fiscalFixture,
  item13Fixture,
  minimumFixture,
  refundFixture,
  tccFixture,
  reliefFixture
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

  it("fails closed on official print-field capacity overflow", () => {
    for (const [path, mutate] of [
      ["taxpayer.name", (value: Record<string, any>) => {
        value.taxpayer.name = "N".repeat(41);
      }],
      ["taxpayer.registered_address", (value: Record<string, any>) => {
        value.taxpayer.registered_address = "A".repeat(72);
      }],
      ["taxpayer.contact_number", (value: Record<string, any>) => {
        value.taxpayer.contact_number = "1".repeat(13);
      }],
      ["taxpayer.email", (value: Record<string, any>) => {
        value.taxpayer.email = "E".repeat(29);
      }],
      ["tax_relief_specification", (value: Record<string, any>) => {
        value.fields.tax_relief_specification.value = "R".repeat(27);
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
