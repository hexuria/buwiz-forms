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
import capacityFixture from "../../form-contracts/fixtures/1601c-3-rows.json";
import longFixture from "../../form-contracts/fixtures/1601c-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1601c-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1601c-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1601c-validation-edge.json";
import { FormDocument } from "../src/FormDocument";
import {
  OFFICIAL_1601C_PDF417_PATHS,
  OFFICIAL_1601C_PDF417_PAYLOADS
} from "../src/forms/official1601CAssets";

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
  capacityFixture
] as const;

describe("1601C:2018 runtime render contract", () => {
  it("accepts and renders every Rust-generated fixture as exactly two folio pages", () => {
    for (const fixture of fixtures) {
      const value = structuredClone(fixture);
      expect(() => assertRenderEnvelope(value)).not.toThrow();
      const markup = renderToStaticMarkup(
        createElement(FormDocument, { envelope: value as RenderEnvelope })
      );
      expect(markup.match(/class="[^"]*form-page/g)).toHaveLength(2);
      expect(markup).toContain("Part IV - Schedule");
    }
  });

  it("prints Rust-owned schedule values and Item 26 without calculating in React", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    expect(markup).toContain('data-row-key="schedule-1-1"');
    expect(markup).toContain("PAY-1601C-001");
    expect(markup).toContain('data-item="26"');
    expect(fixture.fields.tax_26_adjustment).toEqual({ type: "decimal", value: 1100 });
  });

  it("prints the reviewed official wording and semantic page-two identity labels", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );

    expect(markup).toContain("From Part IV-Schedule 1, Item 4");
    expect(markup).not.toContain("From Part IV-Schedule I, Item 4");
    expect(markup).toContain("is true and correct, pursuant to the provisions");
    expect(markup.match(/identity-label-1601c/g)).toHaveLength(2);
    for (const marker of ["a.1", "a.2", "b.1", "b.2", "b.3", "b.4"]) {
      expect(markup).toContain(`>${marker}</b>`);
    }
  });

  it("uses the reviewed plain Item 5 field and measured overflow contract", () => {
    const normalMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: structuredClone(normalFixture) as RenderEnvelope })
    );
    expect(normalMarkup).toContain("atc-plain-1601c");
    expect(normalMarkup).toContain('aria-label="WW010"');

    const longMarkup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: structuredClone(longFixture) as RenderEnvelope })
    );
    expect(longMarkup).toContain('data-adaptive-min-font-px="8"');
    expect(longMarkup).toContain('data-adaptive-max-font-px="9.6"');
    expect(longMarkup).toContain('data-adaptive-step-px="0.5"');
    expect(longMarkup).not.toMatch(/font-size:\s*(?:3\.5|4|5|6|7\.2)pt/);
  });

  it("renders long profile and schedule values through reviewed plain boxes", () => {
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

  it("preserves both exact official PDF417 matrices and live captions", () => {
    expect(pdf417ModuleDigest(OFFICIAL_1601C_PDF417_PATHS[1])).toEqual({
      digest: "7e4a3607ef9e721686f43cef71aba5b7426e2727b830149e37866e1d35be9a45",
      blackModules: 474
    });
    expect(pdf417ModuleDigest(OFFICIAL_1601C_PDF417_PATHS[2])).toEqual({
      digest: "af50f07764d907447d25e8a18fde78e407679bfbffab5b32eb07fc82aff851c4",
      blackModules: 476
    });

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    for (const page of [1, 2] as const) {
      expect(markup).toContain(
        `aria-label="${OFFICIAL_1601C_PDF417_PAYLOADS[page]}"`
      );
      expect(markup).toContain(`data-barcode-page="${page}"`);
      expect(markup).toContain(
        `<small>${OFFICIAL_1601C_PDF417_PAYLOADS[page]}</small>`
      );
    }
    expect(markup.match(/viewBox="0 0 120 7"/g)).toHaveLength(2);
    expect(markup.match(/shape-rendering="crispEdges"/g)).toHaveLength(2);
    expect(markup).not.toContain("1601c-barcode-page-1.png");
    expect(markup).not.toContain("1601c-barcode-page-2.png");
  });

  it("embeds the exact official object-derived grayscale seal", () => {
    const sealPath = path.resolve(
      HERE,
      "../src/forms/assets/1601c-seal.png"
    );
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "de602852cef008b3182bb77b03c06d1ec3f0a6ea2484d3d25c0d161df56f270b"
    );
  });

  it("fails closed on missing legal fields or rows beyond verified capacity", () => {
    const missing = structuredClone(normalFixture) as Record<string, any>;
    delete missing.fields.tax_36_total_amount_payable;
    expect(() => assertRenderEnvelope(missing)).toThrow(
      "tax_36_total_amount_payable"
    );

    const overflow = structuredClone(capacityFixture) as Record<string, any>;
    overflow.schedules[0].rows.push(structuredClone(overflow.schedules[0].rows[0]));
    overflow.schedules[0].rows[3].key = "schedule-1-4";
    expect(() => assertRenderEnvelope(overflow)).toThrow(
      "verified Schedule I capacity is three rows"
    );

    const wrongAtc = structuredClone(normalFixture) as Record<string, any>;
    wrongAtc.fields.atc = { type: "text", value: "WC010" };
    expect(() => assertRenderEnvelope(wrongAtc)).toThrow(
      "fixed Item 5 ATC WW010"
    );
  });
});
