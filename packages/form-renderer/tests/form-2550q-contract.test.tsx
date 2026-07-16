import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import capacityFixture from "../../form-contracts/fixtures/2550q-two-row-capacity.json";
import longFixture from "../../form-contracts/fixtures/2550q-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/2550q-minimum.json";
import normalFixture from "../../form-contracts/fixtures/2550q-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/2550q-validation-edge.json";
import { FormDocument } from "../src/FormDocument";

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  capacityFixture
] as const;

describe("2550Q:2024 exact semantic preview contract", () => {
  it("accepts every Rust fixture and renders exactly two 612x1008 pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(2);
      expect(markup.match(/data-paper="legal"/g)).toHaveLength(2);
      expect(markup).toContain("Part IV – Details of VAT Computation");
      expect(markup).toContain("Schedule 1 — Amortized Input Tax from Capital Goods");
    }
  });

  it("prints the official Item 4, Item 5, and Item 6 choices without shifting labels", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain("Return Period");
    expect(markup).toContain("Amended Return?");
    expect(markup).toContain("Short Period Return?");
    expect(markup).toContain("04/24ENCS P1");
    expect(markup).toContain("04/24ENCS P2");
  });

  it("uses Rust-owned signed amounts and preserves unentered optional amounts as blank", () => {
    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.item_61b).toEqual({ type: "decimal", value: 0 });
    expect(minimum.fields.payment_check_amount).toBeUndefined();
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: minimum })
    );
    expect(markup).toContain("blank-money-2550q");
  });

  it("renders both exact official schedule rows without inventing continuation pages", () => {
    const fixture = structuredClone(capacityFixture) as RenderEnvelope;
    expect(fixture.fields.schedule_1_2_allowable_input_tax).toEqual({
      type: "decimal",
      value: 1440
    });
    expect(fixture.fields.schedule_3_2_tax_withheld).toEqual({
      type: "decimal",
      value: 1000
    });
    expect(fixture.fields.schedule_4_2_amount).toEqual({
      type: "decimal",
      value: 750
    });
    expect(fixture.schedules).toHaveLength(0);
  });

  it("keeps long valid identity, description, and schedule text in reviewed plain boxes", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    for (const value of [
      fixture.taxpayer.name,
      fixture.taxpayer.registered_address,
      fixture.taxpayer.email,
      (fixture.fields.item_19_description as { value: string }).value,
      (fixture.fields.schedule_1_1_description as { value: string }).value,
      (fixture.fields.schedule_4_1_receipt as { value: string }).value
    ]) {
      expect(markup).toContain(`aria-label="${value.replaceAll("&", "&amp;").replaceAll('"', "&quot;")}"`);
    }
    expect(markup.match(/data-overflow-mode="plain"/g)?.length).toBeGreaterThan(5);
  });

  it("fails closed on invalid period, continuation schedule, or a third fixed row", () => {
    const month = structuredClone(normalFixture) as Record<string, any>;
    month.period.month = 13;
    expect(() => assertRenderEnvelope(month)).toThrow(
      "2550Q year-end month must be between 1 and 12"
    );

    const schedule = structuredClone(normalFixture) as Record<string, any>;
    schedule.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => assertRenderEnvelope(schedule)).toThrow(
      "2550Qv2024 uses exact fixed two-row schedule fields"
    );

    const thirdRow = structuredClone(normalFixture) as Record<string, any>;
    thirdRow.fields.schedule_3_3_agent = { type: "text", value: "UNREVIEWED" };
    expect(() => assertRenderEnvelope(thirdRow)).toThrow(
      "2550Q exact renderer capacity is two rows"
    );
  });
});
