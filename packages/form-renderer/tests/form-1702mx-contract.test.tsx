import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import capacityFixture from "../../form-contracts/fixtures/1702mx-fixed-capacity.json";
import longFixture from "../../form-contracts/fixtures/1702mx-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1702mx-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1702mx-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1702mx-validation-edge.json";
import { Form1702MX } from "../src/forms/Form1702MX";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  capacityFixture
] as const;

describe("1702MX:2018C semantic HTML contract", () => {
  it("accepts every Rust fixture and renders exactly four 612x936 base-return pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(Form1702MX, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(4);
      expect(markup.match(/data-paper="folio"/g)).toHaveLength(4);
      expect(markup).toContain("Schedule 1 – Basis of Tax Relief");
      expect(markup).toContain("Schedule 10 – Reconciliation of Net Income");
    }
  });

  it("never appends the distinct two-page mandatory attachment", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    fixture.fields.mandatory_attachment_has_values = { type: "boolean", value: true };
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    );
    expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(4);
    expect(markup).toContain("separate two-page document");
    expect(fixture.fields.mandatory_attachment_source_sha256).toEqual({
      type: "text",
      value: "36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26"
    });
    expect(fixture.fields.mandatory_attachment_transport_supported).toEqual({
      type: "boolean",
      value: false
    });
  });

  it("preserves long identity and schedule values with reviewed plain-box mode", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    );
    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email,
      (fixture.fields.schedule_3_item_30_description as { value: string }).value,
      (fixture.fields.schedule_6_row_1_description as { value: string }).value
    ]) {
      expect(markup).toContain(value.toUpperCase());
    }
    expect(markup).toContain("data-overflow-mode=\"plain\"");
  });

  it("prints all fixed official capacities without TypeScript calculation", () => {
    const fixture = structuredClone(capacityFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    );
    expect(markup).toContain("SPECIAL DEDUCTION ROW 4");
    expect(markup).toContain("2023");
    expect(markup).toContain("RECONCILIATION ITEM 10");
    expect(markup).toContain("75,000");
  });

  it("fails closed if an undeclared continuation schedule reaches the component", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    fixture.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    )).toThrow("uses fixed official schedule rows");
  });
});
