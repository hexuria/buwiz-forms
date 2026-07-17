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
import allLinesFixture from "../../form-contracts/fixtures/1701q-all-lines.json";
import longFixture from "../../form-contracts/fixtures/1701q-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1701q-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1701q-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1701q-validation-edge.json";
import { FormDocument } from "../src/FormDocument";
import {
  OFFICIAL_1701Q_PDF417_PATHS,
  OFFICIAL_1701Q_PDF417_PAYLOADS
} from "../src/forms/official1701QAssets";

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
      expect(markup.match(/data-paper="folio"/g)).toHaveLength(2);
      expect(markup).toContain("PART V – COMPUTATION OF TAX DUE");
      expect(markup).toContain("Schedule IV - Penalties");
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

  it("keeps the official page-two wording and peso notation", () => {
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: structuredClone(normalFixture) as RenderEnvelope })
    );
    expect(markup).toContain("Schedule III - Tax Credits/Payments");
    expect(markup).toContain("Item 45 x Applicable Tax Rate based on Tax Table below");
    expect(markup).toContain("in the amount of P 250,000");
    expect(markup).toContain("P 2,410,000 + 35% of the excess over P 8,000,000");
    expect(markup).not.toContain("₱");
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

  it("preserves both exact official PDF417 matrices and live captions", () => {
    expect(pdf417ModuleDigest(OFFICIAL_1701Q_PDF417_PATHS[1])).toEqual({
      digest: "81ffb136537f07b8b526cb6f5968802f20734fd3026269c1a4a0945a653788df",
      blackModules: 480
    });
    expect(pdf417ModuleDigest(OFFICIAL_1701Q_PDF417_PATHS[2])).toEqual({
      digest: "9457e35a53d6a2a04442d9a0346a19b079ac11bda065e554e3f31d97f31d1120",
      blackModules: 476
    });

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(FormDocument, { envelope: fixture })
    );
    for (const page of [1, 2] as const) {
      expect(markup).toContain(
        `aria-label="${OFFICIAL_1701Q_PDF417_PAYLOADS[page]}"`
      );
      expect(markup).toContain(`data-barcode-page="${page}"`);
      expect(markup).toContain(
        `<small>${OFFICIAL_1701Q_PDF417_PAYLOADS[page]}</small>`
      );
    }
    expect(markup.match(/viewBox="0 0 120 7"/g)).toHaveLength(2);
    expect(markup.match(/shape-rendering="crispEdges"/g)).toHaveLength(2);
    expect(markup).not.toContain("1701q-barcode-page-1.png");
    expect(markup).not.toContain("1701q-barcode-page-2.png");
  });

  it("embeds the exact official object-derived grayscale seal", () => {
    const sealPath = path.resolve(HERE, "../src/forms/assets/1701q-seal.png");
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e"
    );
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
