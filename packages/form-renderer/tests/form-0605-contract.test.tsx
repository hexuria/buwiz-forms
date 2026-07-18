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
import longFixture from "../../form-contracts/fixtures/0605-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/0605-minimum.json";
import normalFixture from "../../form-contracts/fixtures/0605-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/0605-validation-edge.json";
import variantFixture from "../../form-contracts/fixtures/0605-variant.json";
import fieldGuideInventory from "../references/0605-1999-field-guide-inventory.json";
import { FormDocument } from "../src/FormDocument";

const HERE = path.dirname(fileURLToPath(import.meta.url));

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
      expect(markup.match(/<img/g)).toHaveLength(1);
      expect(markup).not.toMatch(/data-(?:barcode-page|symbology)=/);
      expect(markup).not.toMatch(/class="[^"]*(?:barcode|pdf417|qr-code)/i);
      expect(markup).not.toMatch(/aria-label="[^"]*(?:PDF417|QR code)/i);
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

  it("embeds the exact native DeviceGray seal XObject", () => {
    const sealPath = path.resolve(HERE, "../src/forms/assets/0605-seal.png");
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "183a1300cfbfd19689c22b6c1f544e5de3d0c6d10098c916e185e9c951a9c94d"
    );

    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: normalFixture as RenderEnvelope })
    );
    expect(markup).toContain('data-official-pdf-object="13 0"');
    expect(markup).not.toContain("crop_box_px");
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

  it("encodes the pinned 1999 plain fields and exact short-guide counts", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: normalFixture as RenderEnvelope })
    );
    const inventory = new Map(
      fieldGuideInventory.fields.map((field) => [field.field, field])
    );

    for (const field of [
      "6-atc",
      "12-line-of-business",
      "13-taxpayer-name",
      "15-registered-address",
      "17-other-manner",
      "18-installment-count",
      "19-basic-tax",
      "20a-surcharge",
      "20b-interest",
      "20c-compromise",
      "20d-total-penalties",
      "21-total-payable",
      "22a-taxpayer-signature",
      "22a-title-position",
      "22b-head-of-office-signature",
      "23-amount",
      "24a-bank",
      "24b-number",
      "24d-amount",
      "25a-number",
      "25c-amount",
      "26a-bank",
      "26b-number",
      "26d-amount",
      "machine-validation"
    ]) {
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="plain"');
      expect(tag, field).toContain('data-guide-count="0"');
    }
    expect(inventory.get("18-installment-count")?.mode).toBe("plain_unframed");

    const installmentMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: variantFixture as RenderEnvelope })
    );
    expect(installmentMarkup).toContain(
      'class="check-box checked" aria-hidden="true">X</span><span>No. of Installment</span>'
    );
    expect(officialFieldTag(installmentMarkup, "18-installment-count")).toContain(
      'data-field-mode="plain"'
    );

    for (const [field, segments, guides] of [
      ["2-year-ended", "2-4", 4],
      ["4-due-date", "2-2-4", 5],
      ["5-sheets", "2", 1],
      ["7-return-period", "2-2-4", 5],
      ["8-tax-type", "2", 1],
      ["10-rdo", "3", 2],
      ["16-zip", "4", 3],
      ["24c-date", "2-2-4", 5],
      ["25b-date", "2-2-4", 5],
      ["26c-date", "2-2-4", 5]
    ] as const) {
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="guided"');
      expect(tag, field).toContain(`data-guide-segments="${segments}"`);
      expect(tag, field).toContain(`data-guide-count="${guides}"`);
      expect(inventory.get(field)?.short_tick_count, field).toBe(guides);
    }
  });

  it("uses measured 0.5px adaptive fitting for reviewed long plain fields", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: longFixture as RenderEnvelope })
    );

    for (const field of [
      "13-taxpayer-name",
      "15-registered-address",
      "22a-taxpayer-signature",
      "22a-title-position",
      "22b-head-of-office-signature",
      "machine-validation"
    ]) {
      const tag = officialFieldTag(markup, field);
      expect(tag, field).toContain('data-field-mode="plain"');
      expect(tag, field).toContain('data-guide-count="0"');
    }
    expect(markup).toContain('data-adaptive-max-font-px="10"');
    expect(markup).toContain('data-adaptive-min-font-px="6"');
    expect(markup).toContain('data-adaptive-step-px="0.5"');
    expect(markup).not.toMatch(/font-size:[^;]*calc\(/);
  });

  it("switches the whole TIN field to plain mode only when it exceeds the official 12 guides", () => {
    const fourteenDigitMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: normalFixture as RenderEnvelope })
    );
    expect(officialFieldTag(fourteenDigitMarkup, "9-tin")).toContain(
      'data-field-mode="plain"'
    );

    const legacyTin = structuredClone(normalFixture) as RenderEnvelope;
    legacyTin.taxpayer.tin = "123456789000";
    const legacyMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: legacyTin })
    );
    const tag = officialFieldTag(legacyMarkup, "9-tin");
    expect(tag).toContain('data-field-mode="guided"');
    expect(tag).toContain('data-guide-segments="3-3-3-3"');
    expect(tag).toContain('data-guide-count="8"');
  });

  it("switches only contract-valid guide overflow to one plain field", () => {
    const cases: Array<{
      field: string;
      exact: string | number;
      overflow: string | number;
      mutate: (fixture: Record<string, any>, value: string | number) => void;
    }> = [
      {
        field: "5-sheets",
        exact: 99,
        overflow: 100,
        mutate: (fixture, value) => { fixture.fields.number_of_sheets.value = value; }
      },
      {
        field: "8-tax-type",
        exact: "IT",
        overflow: "ITX",
        mutate: (fixture, value) => { fixture.fields.tax_type_code.value = value; }
      },
      {
        field: "9-tin",
        exact: "123456789000",
        overflow: "1234567890001",
        mutate: (fixture, value) => { fixture.taxpayer.tin = value; }
      },
      {
        field: "14-telephone",
        exact: "1234567",
        overflow: "12345678",
        mutate: (fixture, value) => { fixture.taxpayer.contact_number = value; }
      }
    ];

    for (const boundary of cases) {
      for (const [value, expectedMode] of [
        [boundary.exact, "guided"],
        [boundary.overflow, "plain"]
      ] as const) {
        const fixture = structuredClone(normalFixture) as Record<string, any>;
        boundary.mutate(fixture, value);
        expect(() => assertRenderEnvelope(fixture)).not.toThrow();
        const markup = renderToStaticMarkup(
          createElement(FormDocument, { envelope: fixture as RenderEnvelope })
        );
        const tag = officialFieldTag(markup, boundary.field);
        expect(tag, `${boundary.field} value=${String(value)}`).toContain(
          `data-field-mode="${expectedMode}"`
        );
        if (expectedMode === "plain") {
          expect(tag).toContain('data-guide-count="0"');
          expect(tag).not.toContain("data-guide-segments");
        }
      }
    }
  });

  it("fails closed when fixed identifiers or dates exceed their legal contract", () => {
    const invalidCases: Array<{
      path: string;
      mutate: (fixture: Record<string, any>) => void;
    }> = [
      {
        path: "envelope.fields.due_date.value",
        mutate: (fixture) => { fixture.fields.due_date.value = "01/02/20260"; }
      },
      {
        path: "envelope.fields.return_period.value",
        mutate: (fixture) => { fixture.fields.return_period.value = "01/02/20260"; }
      },
      {
        path: "envelope.fields.payment_24_date.value",
        mutate: (fixture) => { fixture.fields.payment_24_date.value = "01/02/20260"; }
      },
      {
        path: "envelope.taxpayer.rdo_code",
        mutate: (fixture) => { fixture.taxpayer.rdo_code = "0180"; }
      },
      {
        path: "envelope.taxpayer.zip_code",
        mutate: (fixture) => { fixture.taxpayer.zip_code = "22000"; }
      }
    ];

    for (const invalidCase of invalidCases) {
      const fixture = structuredClone(normalFixture) as Record<string, any>;
      invalidCase.mutate(fixture);
      expect(() => assertRenderEnvelope(fixture), invalidCase.path).toThrow(
        invalidCase.path
      );
    }
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

function officialFieldTag(markup: string, field: string): string {
  const match = markup.match(
    new RegExp(`<[^>]+data-official-field="${field.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"[^>]*>`)
  );
  expect(match, `missing official field ${field}`).not.toBeNull();
  return match?.[0] ?? "";
}
