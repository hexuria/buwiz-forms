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
import capacityFixture from "../../form-contracts/fixtures/1701-fixed-capacity.json";
import longFixture from "../../form-contracts/fixtures/1701-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1701-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1701-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1701-validation-edge.json";
import { Form1701 } from "../src/forms/Form1701";
import { OFFICIAL_1701_PDF417 } from "../src/forms/official1701Assets";

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

  it("preserves all four exact official PDF417 matrices and live captions", () => {
    const expected = [
      ["a70883c8ea82b51527ab13d9d7a84acea444a1b941107cf0be13d3132066a068", 483],
      ["fc575f786121c3fdfd0b3acc6e7b0757fcf0098bef23b3beac77a359826d3354", 474],
      ["28f993cc6172d3b5891ebd5caa3c0e68696698ecf194c27a10acb774d99a4f6e", 484],
      ["209e791883eab663776176777be715f5292b13b386a3a461c1f5bd829fbae1d6", 482]
    ] as const;
    for (const [index, artwork] of OFFICIAL_1701_PDF417.entries()) {
      expect(pdf417ModuleDigest(artwork.path)).toEqual({
        digest: expected[index][0],
        blackModules: expected[index][1]
      });
    }

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    for (const artwork of OFFICIAL_1701_PDF417) {
      expect(markup).toContain(`aria-label="${artwork.payload}"`);
      expect(markup).toContain(`data-barcode-page="${artwork.page}"`);
      expect(markup).toContain(`<small>${artwork.payload}</small>`);
    }
    expect(markup.match(/viewBox="0 0 120 7"/g)).toHaveLength(4);
    expect(markup.match(/shape-rendering="crispEdges"/g)).toHaveLength(4);
    expect(markup).not.toContain("1701-barcode-page-");
  });

  it("embeds the exact official object-derived grayscale seal", () => {
    const sealPath = path.resolve(HERE, "../src/forms/assets/1701-seal.png");
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "50d1fc573146e251138b78074b5790dd569f6dbde335feea908334adef4dd7b0"
    );
  });
});
