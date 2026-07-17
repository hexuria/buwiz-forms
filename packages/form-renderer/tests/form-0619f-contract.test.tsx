import {
  assertRenderEnvelope,
  type RenderEnvelope
} from "@ebirforms/form-contracts";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import longFixture from "../../form-contracts/fixtures/0619f-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/0619f-minimum.json";
import normalFixture from "../../form-contracts/fixtures/0619f-normal.json";
import allPaymentsFixture from "../../form-contracts/fixtures/0619f-all-payments.json";
import validationEdgeFixture from "../../form-contracts/fixtures/0619f-validation-edge.json";
import fieldGuideInventory from "../references/0619f-2018-field-guide-inventory.json";
import { FormDocument } from "../src/FormDocument";
import {
  OFFICIAL_0619F_PDF417_PAGE_ONE_PATH,
  OFFICIAL_0619F_PDF417_PAYLOAD
} from "../src/forms/official0619FAssets";

const HERE = path.dirname(fileURLToPath(import.meta.url));

function pdf417ModuleDigest(pathData: string): {
  digest: string;
  blackModules: number;
} {
  const modules = Array<boolean>(120 * 7).fill(false);
  const command = /M(\d+) (\d+)h(\d+)v1H(\d+)z/g;
  const consumed: string[] = [];
  let blackModules = 0;

  for (const match of pathData.matchAll(command)) {
    consumed.push(match[0]);
    const x = Number(match[1]);
    const y = Number(match[2]);
    const width = Number(match[3]);
    expect(Number(match[4])).toBe(x);
    expect(y).toBeGreaterThanOrEqual(0);
    expect(y).toBeLessThan(7);
    expect(x + width).toBeLessThanOrEqual(120);

    for (let column = x; column < x + width; column += 1) {
      const index = y * 120 + column;
      expect(modules[index]).toBe(false);
      modules[index] = true;
      blackModules += 1;
    }
  }

  expect(consumed.join("")).toBe(pathData);
  const bits = modules.map((module) => module ? "1" : "0").join("");
  return {
    digest: createHash("sha256").update(bits).digest("hex"),
    blackModules
  };
}

const fixtures = [
  minimumFixture,
  normalFixture,
  longFixture,
  validationEdgeFixture,
  allPaymentsFixture
] as const;

describe("0619F:2018 runtime render contract", () => {
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
    expect(markup).toContain("WMF10");
    expect(markup).toContain("WMF20");
    expect(fixture.fields.tax_type_code).toEqual({ type: "text", value: "WB" });
    expect(markup).toContain('data-item="17"');
    expect(markup).toContain('data-item="18D"');
    expect(fixture.fields.item_17_net_amount_of_remittance).toEqual({
      type: "decimal",
      value: 120000
    });
    expect(fixture.fields.item_19_total_amount_of_remittance).toEqual({
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
    expect(markup).toContain("money-overflow-0619f");
  });

  it("preserves all four fixed payment rows and official blank amounts", () => {
    const payment = structuredClone(allPaymentsFixture) as RenderEnvelope;
    const paymentMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: payment })
    );
    for (const reference of ["BDM-001", "CHECK-002", "TDM-003", "OTHER-004"]) {
      expect(paymentMarkup).toContain(reference);
    }
    expect(paymentMarkup).toContain("payment-tax-debit-row-0619f");
    expect(paymentMarkup).toContain('data-official-field="22-bank"');
    expect(paymentMarkup).toContain('aria-label="BIR"');
    expect(paymentMarkup).toContain("payment-particular-field-0619f");
    expect(paymentMarkup).toContain("REVENUE COLLECTION OFFICER");

    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.payment_20_amount_present).toEqual({
      type: "boolean",
      value: false
    });
  });

  it("preserves the exact official PDF417 active matrix and live caption", () => {
    expect(pdf417ModuleDigest(OFFICIAL_0619F_PDF417_PAGE_ONE_PATH)).toEqual({
      digest: "b72d83aba3dd6189e71a96019fbc57c5e8bcd7ec2465228fda8d74154308150b",
      blackModules: 476
    });

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain(`aria-label="${OFFICIAL_0619F_PDF417_PAYLOAD}"`);
    expect(markup).toContain('class="official-pdf417-object-0619f"');
    expect(markup).toContain('viewBox="0 0 120 7"');
    expect(markup).toContain('shape-rendering="crispEdges"');
    expect(markup).toContain(`<small>${OFFICIAL_0619F_PDF417_PAYLOAD}</small>`);
    expect(markup).not.toContain("0619f-barcode-page-1.png");
  });

  it("embeds the exact official object-derived grayscale seal", () => {
    const sealPath = path.resolve(
      HERE,
      "../src/forms/assets/0619f-seal.png"
    );
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "42909b9601489a09f4bbcd9a2e0502bb8b0c839617c6b1be3c4e41d2a88c4954"
    );
  });

  it("encodes the pinned 2018 plain fields and exact short-guide counts", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: normalFixture as RenderEnvelope })
    );
    const inventory = new Map(
      fieldGuideInventory.fields.map((field) => [field.field, field])
    );

    for (const field of [
      "13-atc",
      "14-atc",
      "tax-agent-accreditation",
      "tax-agent-date-of-issue",
      "tax-agent-date-of-expiry",
      "22-bank",
      "machine-validation"
    ]) {
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="plain"');
      expect(tag, field).toContain('data-guide-count="0"');
      expect(inventory.get(field)?.short_tick_count, field).toBe(0);
    }

    for (const field of [
      "1-month",
      "2-due-date",
      "5-tax-type-code",
      "6-tin",
      "7-rdo",
      "8-agent-name",
      "9-address-line-1",
      "9-address-line-2",
      "9a-zip",
      "10-contact",
      "12-email",
      "13-amount",
      "14-amount",
      "15-amount",
      "16-amount",
      "17-amount",
      "18a-amount",
      "18b-amount",
      "18c-amount",
      "18d-amount",
      "19-amount",
      "20-date",
      "20-amount",
      "21-bank",
      "21-number",
      "21-date",
      "21-amount",
      "22-number",
      "22-date",
      "22-amount",
      "23-particular",
      "23-bank",
      "23-number",
      "23-date",
      "23-amount"
    ]) {
      const evidence = inventory.get(field);
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="guided"');
      expect(tag, field).toContain(
        `data-guide-segments="${evidence?.segments?.join("-")}"`
      );
      expect(tag, field).toContain(
        `data-guide-count="${evidence?.short_tick_count}"`
      );
    }

    for (const field of ["20-bank", "20-number"]) {
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="plain"');
      expect(tag, field).toContain('data-guide-count="0"');
    }
  });

  it("keeps exact-capacity guides and switches the whole over-capacity field to measured plain mode", () => {
    const exact = structuredClone(normalFixture) as RenderEnvelope;
    exact.taxpayer.name = "A".repeat(40);
    const exactMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: exact })
    );
    expect(officialFieldTag(exactMarkup, "8-agent-name")).toContain(
      'data-field-mode="guided"'
    );
    expect(officialFieldTag(exactMarkup, "8-agent-name")).toContain(
      'data-guide-count="39"'
    );

    const overflow = structuredClone(normalFixture) as RenderEnvelope;
    overflow.taxpayer.name = "B".repeat(41);
    const overflowMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: overflow })
    );
    expect(officialFieldTag(overflowMarkup, "8-agent-name")).toContain(
      'data-field-mode="plain"'
    );
    expect(overflowMarkup).toContain(`aria-label="${"B".repeat(41)}"`);
    expect(overflowMarkup).toContain('data-adaptive-max-font-px="10"');
    expect(overflowMarkup).toContain('data-adaptive-min-font-px="6"');
    expect(overflowMarkup).toContain('data-adaptive-step-px="0.5"');
  });

  it("fails closed on missing fields, mutable fixed codes, or schedules", () => {
    const missing = structuredClone(normalFixture) as Record<string, any>;
    delete missing.fields.item_19_total_amount_of_remittance;
    expect(() => assertRenderEnvelope(missing)).toThrow(
      "item_19_total_amount_of_remittance"
    );

    const wrongAtc = structuredClone(normalFixture) as Record<string, any>;
    wrongAtc.fields.item_13_atc.value = "WMF99";
    expect(() => assertRenderEnvelope(wrongAtc)).toThrow("fixed Item 13 ATC WMF10");

    const schedule = structuredClone(normalFixture) as Record<string, any>;
    schedule.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => assertRenderEnvelope(schedule)).toThrow(
      "0619F has no repeatable renderer schedule"
    );
  });
});

function officialFieldTag(markup: string, field: string): string {
  const match = markup.match(
    new RegExp(`<[^>]+data-official-field="${field.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"[^>]*>`)
  );
  expect(match, `missing official field ${field}`).not.toBeNull();
  return match?.[0] ?? "";
}
