import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import allLinesFixture from "../../form-contracts/fixtures/1701q-all-lines.json";
import longFixture from "../../form-contracts/fixtures/1701q-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1701q-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1701q-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1701q-validation-edge.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  allLinesFixture
] as const;

describe("1701Q:2018 experimental preview contract", () => {
  it("accepts every Rust fixture and renders exactly two 612x936 pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(2);
      expect(markup.match(/data-paper="legal"/g)).toHaveLength(2);
      expect(markup).toContain("PART V – COMPUTATION OF TAX DUE");
      expect(markup).toContain("Schedule IV – Penalties");
    }
  });

  it("maps Rust-owned calculations and keeps unentered schedule lines blank", () => {
    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.item_26_taxpayer).toEqual({
      type: "decimal",
      value: 7500
    });
    expect(minimum.fields.item_36_taxpayer).toEqual({
      type: "decimal",
      value: 500000
    });
    expect(minimum.fields.item_47_taxpayer).toBeUndefined();
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: minimum })
    );
    expect(markup).toContain("blank-amount-1701q");
  });

  it("leaves unresolved optional choices unmarked instead of inventing No", () => {
    const fixture = structuredClone(validationEdgeFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const item15 = markup.match(/Claiming Foreign Tax Credits\?[\s\S]{0,300}/)?.[0];
    expect(item15).toBeDefined();
    expect(item15).not.toContain("check-box checked");
  });

  it("preserves long valid identity values with reviewed plain-box mode", () => {
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
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(2);
  });

  it("fails closed on invalid quarter, mutable filer choice, or schedules", () => {
    const quarter = structuredClone(normalFixture) as Record<string, any>;
    quarter.period.quarter = 4;
    expect(() => assertRenderEnvelope(quarter)).toThrow(
      "1701Q quarter must be 1, 2, or 3"
    );

    const filer = structuredClone(normalFixture) as Record<string, any>;
    filer.fields.filer_type.value = "corporation";
    expect(() => assertRenderEnvelope(filer)).toThrow(
      "unsupported 1701Q filer type"
    );

    const schedule = structuredClone(normalFixture) as Record<string, any>;
    schedule.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => assertRenderEnvelope(schedule)).toThrow(
      "1701Q has no repeatable renderer schedule"
    );
  });
});
