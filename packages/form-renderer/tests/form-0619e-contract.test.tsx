import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import longFixture from "../../form-contracts/fixtures/0619e-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/0619e-minimum.json";
import normalFixture from "../../form-contracts/fixtures/0619e-normal.json";
import paymentFixture from "../../form-contracts/fixtures/0619e-payment.json";
import validationEdgeFixture from "../../form-contracts/fixtures/0619e-validation-edge.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  paymentFixture
] as const;

describe("0619E:2018 runtime render contract", () => {
  it("accepts every Rust fixture and renders exactly one Letter page", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(1);
      expect(markup).toContain('data-paper="letter"');
      expect(markup).toContain("Part III – Details of Payment");
    }
  });

  it("prints fixed codes and Rust-owned totals without calculating in React", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain("WME10");
    expect(markup).toContain("WE");
    expect(markup).toContain('data-item="16"');
    expect(markup).toContain('data-item="17D"');
    expect(fixture.fields.item_16_net_amount_of_remittance).toEqual({
      type: "decimal",
      value: 120000
    });
    expect(fixture.fields.item_18_total_amount_of_remittance).toEqual({
      type: "decimal",
      value: 124250
    });
  });

  it("renders long legal values through reviewed plain-box mode", () => {
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

  it("preserves all four fixed payment rows and official blank amounts", () => {
    const payment = structuredClone(paymentFixture) as RenderEnvelope;
    const paymentMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: payment })
    );
    for (const reference of ["BDM-001", "CHECK-002", "TDM-003", "OTHER-004"]) {
      expect(paymentMarkup).toContain(reference);
    }

    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.payment_19_amount_present).toEqual({
      type: "boolean",
      value: false
    });
  });

  it("fails closed on missing fields, mutable fixed codes, or schedules", () => {
    const missing = structuredClone(normalFixture) as Record<string, any>;
    delete missing.fields.item_18_total_amount_of_remittance;
    expect(() => assertRenderEnvelope(missing)).toThrow(
      "item_18_total_amount_of_remittance"
    );

    const wrongAtc = structuredClone(normalFixture) as Record<string, any>;
    wrongAtc.fields.atc.value = "WME99";
    expect(() => assertRenderEnvelope(wrongAtc)).toThrow("fixed ATC WME10");

    const schedule = structuredClone(normalFixture) as Record<string, any>;
    schedule.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => assertRenderEnvelope(schedule)).toThrow(
      "0619E has no repeatable renderer schedule"
    );
  });
});
