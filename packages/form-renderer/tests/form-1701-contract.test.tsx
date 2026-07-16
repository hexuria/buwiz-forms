import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import capacityFixture from "../../form-contracts/fixtures/1701-fixed-capacity.json";
import longFixture from "../../form-contracts/fixtures/1701-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1701-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1701-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1701-validation-edge.json";
import { Form1701 } from "../src/forms/Form1701";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  capacityFixture
] as const;

describe("1701:2018 semantic HTML preview contract", () => {
  it("accepts every Rust fixture and renders exactly four 612x936 pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(Form1701, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(4);
      expect(markup.match(/data-paper="folio"/g)).toHaveLength(4);
      expect(markup).toContain("PART IV - Background Information of Spouse");
      expect(markup).toContain("Schedule 6 - Computation of Net Operating Loss Carry Over");
      expect(markup).toContain("PART IX - Reconciliation of Net Income per Books Against Taxable Income");
    }
  });

  it("maps only Rust-owned calculations and keeps unentered cells blank", () => {
    const fixture = structuredClone(minimumFixture) as RenderEnvelope;
    expect(fixture.fields.schedule_3_8_taxpayer).toEqual({
      type: "decimal",
      value: 750000
    });
    expect(fixture.fields.schedule_3_17_taxpayer).toEqual({
      type: "decimal",
      value: 300000
    });
    expect(fixture.fields.schedule_4_1_taxpayer).toBeUndefined();
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    expect(markup).toContain("blank-amount-1701");
  });

  it("renders unresolved choices unmarked instead of inventing No", () => {
    const fixture = structuredClone(validationEdgeFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    const foreignCredit = markup.match(/Claiming Foreign Tax Credits\?[\s\S]{0,320}/)?.[0];
    expect(foreignCredit).toBeDefined();
    expect(foreignCredit).not.toContain("check-box checked");
  });

  it("preserves long valid identity and description values in reviewed plain boxes", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email,
      (fixture.fields.schedule_3_20_description as { value: string }).value
    ]) {
      expect(markup).toContain(`aria-label="${value}"`);
    }
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(3);
  });

  it("renders every reviewed fixed-capacity row without creating continuation schedules", () => {
    const fixture = structuredClone(capacityFixture) as RenderEnvelope;
    expect(fixture.schedules).toEqual([]);
    expect(fixture.fields.schedule_5_taxpayer_2_description).toEqual({
      type: "text",
      value: "SPECIAL TAXPAYER DEDUCTION 2"
    });
    expect(fixture.fields.schedule_5_spouse_2_description).toEqual({
      type: "text",
      value: "SPECIAL SPOUSE DEDUCTION 2"
    });
    expect(fixture.fields.schedule_6_spouse_4_year).toEqual({
      type: "text",
      value: "2024"
    });
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    expect(markup.match(/class="special-row-1701"/g)).toHaveLength(4);
    expect(markup.match(/class="nolco-row-1701"/g)).toHaveLength(8);
  });
});
