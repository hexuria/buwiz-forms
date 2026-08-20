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
import longFixture from "../../form-contracts/fixtures/0619e-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/0619e-minimum.json";
import normalFixture from "../../form-contracts/fixtures/0619e-normal.json";
import paymentFixture from "../../form-contracts/fixtures/0619e-payment.json";
import validationEdgeFixture from "../../form-contracts/fixtures/0619e-validation-edge.json";
import { FormDocument } from "../src/FormDocument";
import {
  OFFICIAL_0619E_PDF417_PAGE_ONE_PATH,
  OFFICIAL_0619E_PDF417_PAYLOAD
} from "../src/forms/official0619EAssets";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const sourceMetadata = JSON.parse(fs.readFileSync(path.resolve(
  HERE,
  "../references/0619e-2018-source.json"
), "utf8")) as {
  form: {
    official_source_sha256: string;
    page_count: number;
    page_height_pt: number;
    page_width_pt: number;
    reviewed_supporting_sources: Array<Record<string, unknown>>;
  };
};

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
  paymentFixture
] as const;

describe("0619E:2018 runtime render contract", () => {
  it("pins the exact official PDF and both reviewed editable-source variants", () => {
    expect(sourceMetadata.form).toMatchObject({
      official_source_sha256: "0418160d63d4e6f68c34f2bad553273a5d148c3686d8562d338d35fcdd0c5215",
      page_count: 1,
      page_height_pt: 792,
      page_width_pt: 612,
      reviewed_supporting_sources: [
        {
          field_count: 58,
          kind: "editable_xml",
          semantic_replay: "exact_after_typed_lexical_canonicalization",
          source_file: "external:00000000000000-0619E-042026.xml",
          source_sha256: "a6f21e372a1ce6d707ede13f2447290683ab302d859c3b684a06c55788cbfade"
        },
        {
          decrypted_extra_fields: ["frm0619E:txtAddress2"],
          field_count: 59,
          kind: "encrypted_editable_xml",
          semantic_differences: [
            "txtFinalFlag is 0 instead of 1 and is preserved without inferred lifecycle meaning",
            "frm0619E:txtLineBus uses one fewer URL-encoding layer but resolves to the same typed text"
          ],
          semantic_replay: "exact_after_decryption_and_typed_lexical_canonicalization",
          source_file: "external:00000000000000-0619E-042026#codeitlikemiley@gmail.com#.xml",
          source_sha256: "1c49950df1197906bb73ddbb5d0f5f5e1c3f488f376e05b6d53febc1b32016ab"
        }
      ]
    });
  });

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

  it("renders the Rust-owned printable TIN instead of repairing the raw TIN", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    fixture.taxpayer.tin = "99999999999999";
    fixture.fields.printable_tin_segment_1 = { type: "text", value: "123" };
    fixture.fields.printable_tin_segment_2 = { type: "text", value: "456" };
    fixture.fields.printable_tin_segment_3 = { type: "text", value: "789" };
    fixture.fields.printable_tin_branch = { type: "text", value: "00123" };

    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain('aria-label="123-456-789-00123"');
    expect(markup).not.toContain('aria-label="999-999-999-99999"');
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
    expect(payment.fields.payment_21_drawee_bank_or_agency).toEqual({
      type: "text",
      value: ""
    });

    const minimum = structuredClone(minimumFixture) as RenderEnvelope;
    expect(minimum.fields.payment_19_amount_present).toEqual({
      type: "boolean",
      value: false
    });
  });

  it("keeps reviewed plain fields plain and rejects data in the non-applicable Item 21 bank cell", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    const signatureFooter = markup.match(
      /<div class="signature-footer-0619e">([\s\S]*?)<\/div>/
    )?.[1];
    expect(signatureFooter).toBeDefined();
    expect(signatureFooter?.match(/data-overflow-mode="plain"/g)).toHaveLength(3);
    expect(signatureFooter).not.toContain("comb-value");
    expect(markup).toContain('class="rdo-value-0619e"');
    expect(markup).toContain('class="payment-nonapplicable-0619e"');
    expect(markup.match(/data-field-mode="plain"/g)).toHaveLength(2);
    expect(markup).toContain(
      'class="code-value-0619e" data-field-mode="plain">WME10</span>'
    );
    expect(markup).toContain(
      'class="code-value-0619e" data-field-mode="plain">WE</span>'
    );
    expect(markup).toContain(
      "<strong>Total Amount of Remittance</strong>"
    );
    expect(markup).toContain("<em>(Item 14 Less Item 15)</em>");
    expect(markup).toContain("<b>*NOTE:</b>");
    expect(markup).not.toContain(
      "<small>(if not filed with an Authorized Agent Bank)</small>"
    );

    const impossible = structuredClone(normalFixture) as RenderEnvelope;
    impossible.fields.payment_21_drawee_bank_or_agency = {
      type: "text",
      value: "BIR"
    };
    expect(() => renderToStaticMarkup(
      createElement(FormDocument, { envelope: impossible })
    )).toThrow(
      "0619E Item 21 Drawee Bank/Agency is non-applicable"
    );
  });

  it("preserves the exact official PDF417 matrix and live caption", () => {
    expect(pdf417ModuleDigest(OFFICIAL_0619E_PDF417_PAGE_ONE_PATH)).toEqual({
      digest: "0238537d56f19276b790a8395c429a9f7645ac570eecc097064051b456dc5dfa",
      blackModules: 480
    });

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain(`aria-label="${OFFICIAL_0619E_PDF417_PAYLOAD}"`);
    expect(markup).toContain('viewBox="0 0 120 7"');
    expect(markup).toContain('shape-rendering="crispEdges"');
    expect(markup).toContain(`<small>${OFFICIAL_0619E_PDF417_PAYLOAD}</small>`);
    expect(markup).not.toContain("0619e-barcode-page-1.png");
  });

  it("embeds the losslessly extracted official seal object", () => {
    const sealPath = path.resolve(
      HERE,
      "../src/forms/assets/0619e-seal.png"
    );
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "6cd637975f8088cc8b56aa67c26794db23331864a45c08517c66b70f53ff2610"
    );
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

    const malformedTin = structuredClone(normalFixture) as Record<string, any>;
    malformedTin.fields.printable_tin_branch.value = "123";
    expect(() => assertRenderEnvelope(malformedTin)).toThrow(
      "printable_tin_branch"
    );

    const malformedPaymentDate = structuredClone(normalFixture) as Record<string, any>;
    malformedPaymentDate.fields.payment_19_date.value = "05102026X";
    expect(() => assertRenderEnvelope(malformedPaymentDate)).toThrow(
      "payment_19_date"
    );
  });
});
