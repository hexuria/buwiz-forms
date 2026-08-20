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
import capacityFixture from "../../form-contracts/fixtures/1702mx-fixed-capacity.json";
import longFixture from "../../form-contracts/fixtures/1702mx-long-values.json";
import minimumFixture from "../../form-contracts/fixtures/1702mx-minimum.json";
import normalFixture from "../../form-contracts/fixtures/1702mx-normal.json";
import validationEdgeFixture from "../../form-contracts/fixtures/1702mx-validation-edge.json";
import { Form1702MX } from "../src/forms/Form1702MX";
import {
  OFFICIAL_1702MX_PDF417_MATRICES,
  OFFICIAL_1702MX_PDF417_PATHS,
  OFFICIAL_1702MX_PDF417_PAYLOADS
} from "../src/forms/official1702MXAssets";

const HERE = path.dirname(fileURLToPath(import.meta.url));

function pdf417ModuleDigest(pathData: string): {
  digest: string;
  pathDigest: string;
  blackModules: number;
} {
  const modules = Array<boolean>(120 * 8).fill(false);
  const command = /M(\d+) (\d+)h(\d+)v1h-(\d+)z/g;
  const consumed: string[] = [];
  let blackModules = 0;

  for (const match of pathData.matchAll(command)) {
    consumed.push(match[0]);
    const x = Number(match[1]);
    const y = Number(match[2]);
    const width = Number(match[3]);
    expect(Number(match[4])).toBe(width);
    expect(y).toBeGreaterThanOrEqual(0);
    expect(y).toBeLessThan(8);
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
    pathDigest: createHash("sha256").update(pathData).digest("hex"),
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

describe("1702MX:2018C semantic HTML contract", () => {
  it("pins the four-page base return and separate two-page mandatory attachment source pack", () => {
    const source = JSON.parse(fs.readFileSync(
      path.resolve(HERE, "../references/1702mx-2018c-source.json"),
      "utf8"
    )) as {
      form: {
        official_source_sha256: string;
        page_count: number;
        page_height_pt: number;
        page_width_pt: number;
        reviewed_supporting_sources: Array<Record<string, unknown>>;
      };
    };

    expect(source.form).toMatchObject({
      official_source_sha256: "81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2",
      page_count: 4,
      page_height_pt: 936,
      page_width_pt: 612
    });
    expect(source.form.reviewed_supporting_sources).toEqual([
      expect.objectContaining({
        field_count: 210,
        kind: "editable_xml",
        semantic_replay: "exact",
        source_sha256: "ed96c5b56eecee68f1f73eef50dda00f69a42bd0dc5d0849e2cbe22c6b70b239"
      }),
      expect.objectContaining({
        kind: "encrypted_editable_xml",
        semantic_replay: "hash_lock_only",
        source_sha256: "ab4896a21603c7853985b6589a918c3d0189872b1817a4a55453ebea063a47b4",
        submission_contract_reviewed: false
      }),
      expect.objectContaining({
        kind: "mandatory_attachment_pdf",
        page_count: 2,
        page_height_pt: 936,
        page_width_pt: 612,
        rendering_supported: false,
        role: "separate_conditional_part_v_per_activity",
        source_sha256: "36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26",
        transport_supported: false
      })
    ]);
  });

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
    expect(markup).toContain("data-adaptive-fit-state=\"pending\"");
  });

  it("splits the official Item 8 capacity into three 38-cell guide rows", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    );
    expect(markup).toContain(
      "data-item-number=\"8\" data-field-mode=\"guided\" data-cell-capacity=\"114\" data-line-count=\"3\" data-line-cell-capacity=\"38,38,38\""
    );
    expect(markup).not.toContain("font-size:4pt");
  });

  it("keeps the reviewed page-one instructions and legal qualifiers verbatim", () => {
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: structuredClone(normalFixture) as RenderEnvelope })
    );
    for (const copy of [
      "Two copies MUST be filed with the BIR and one held by the taxpayer.",
      "[Section 34(L) NIRC, as amended]",
      "If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905",
      "(From Part IV Item 33D)",
      "49 Centavos or Less drop down; 50 or more round up",
      "pursuant to the provisions of the National Internal Revenue Code, as amended",
      "Receipt (RO’s Signature/Bank Teller’s Initial)"
    ]) {
      expect(markup).toContain(copy);
    }
  });

  it("preserves the official Schedule 1 field map without guides in merged unavailable cells", () => {
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: structuredClone(normalFixture) as RenderEnvelope })
    );

    expect(markup.match(/data-relief-row="[1-6]"/g)).toHaveLength(6);
    expect(markup.match(/relief-character-field-1702mx/g)).toHaveLength(9);
    expect(markup.match(/relief-date-field-1702mx/g)).toHaveLength(6);
    expect(markup.match(/data-guide-cells="10"/g)).toHaveLength(10);
    expect(markup.match(/data-guide-cells="9"/g)).toHaveLength(5);
    expect(markup.match(/data-unavailable-field="true"/g)).toHaveLength(2);
    expect(markup).toContain('data-rate-format="00.0%"');
    expect(markup).toContain('data-applicable-character-cells="3"');
    expect(markup).toContain('data-static-cells="2"');
    expect(markup).toContain('data-applicable-cells="8"');
  });

  it("preserves the official page 2 regime unavailable-cell matrix", () => {
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: structuredClone(normalFixture) as RenderEnvelope })
    );

    const unavailableCells = Array.from(markup.matchAll(
      /class="amount-cell-1702mx regime-unavailable-cell-1702mx" data-regime-column="([^"]+)"(?: data-unavailable-block-continues="true")? data-unavailable-cell="true"/g
    )).map((match) => match[1]);
    expect(unavailableCells).toEqual([
      "exempt",
      "exempt", "special",
      "total",
      "exempt",
      "exempt",
      "exempt", "special",
      "exempt",
      "exempt", "special",
      "exempt", "special"
    ]);
    expect(markup.match(/data-unavailable-cell="true"/g)).toHaveLength(13);
    expect(markup.match(/data-unavailable-block-continues="true"/g)).toHaveLength(3);
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

  it("preserves four exact official PDF417 matrices, half-module padding, and live captions", () => {
    const expected = {
      1: {
        digest: "58fe10021bc79bf3c031e1dcecb264a3d643f4fa8a5e54fff0e8279106f2fde2",
        pathDigest: "6fb048d1d2d6d2b307bc43721e734a89763ed0a9a63113409c952fba02843518",
        blackModules: 551
      },
      2: {
        digest: "38f0943e4710bb7c87585e94cbf82e6b4a338379b0d4f95edeb25128364dae25",
        pathDigest: "3b075e1128792795dbfeff0eb22e1046fa8e9fc019d823c1d652dea4a9ecf6f4",
        blackModules: 561
      },
      3: {
        digest: "447a3cd3ae20ed9fcf9527180fb539ef9fbcf5dcd2aeebd6aa272ccd7d4f7602",
        pathDigest: "a79be84a5d683b5dc4cdb59ed0b61a02eb45ca28380b1a455577443f0e5bb809",
        blackModules: 555
      },
      4: {
        digest: "4dce7fd23395f73f138612cade05ad4bf707ddd597293eedeac889cd3eda7a2d",
        pathDigest: "42ad85ee8e08ecf57ac20786c5f6d569bf51060c174e0e514c82c586dbdc3a5d",
        blackModules: 547
      }
    } as const;

    for (const page of [1, 2, 3, 4] as const) {
      expect(OFFICIAL_1702MX_PDF417_MATRICES[page]).toHaveLength(8);
      expect(OFFICIAL_1702MX_PDF417_MATRICES[page].every((row) => row.length === 120)).toBe(true);
      expect(pdf417ModuleDigest(OFFICIAL_1702MX_PDF417_PATHS[page])).toEqual(expected[page]);
    }

    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    );
    for (const page of [1, 2, 3, 4] as const) {
      expect(markup).toContain(`aria-label="${OFFICIAL_1702MX_PDF417_PAYLOADS[page]}"`);
      expect(markup).toContain(`data-barcode-page="${page}"`);
      expect(markup).toContain(`<small>${OFFICIAL_1702MX_PDF417_PAYLOADS[page]}</small>`);
    }
    expect(markup.match(/viewBox="0 0 120.5 8"/g)).toHaveLength(4);
    expect(markup.match(/shape-rendering="crispEdges"/g)).toHaveLength(4);
    expect(markup).not.toContain("1702mx-barcode-page-");
  });

  it("embeds the exact official object-derived native grayscale seal", () => {
    const sealPath = path.resolve(HERE, "../src/forms/assets/1702mx-seal.png");
    const digest = createHash("sha256")
      .update(fs.readFileSync(sealPath))
      .digest("hex");
    expect(digest).toBe(
      "92b7a3fd81ee9db5705482563925d79842d1e961a5a0a931fc6d838ec7a1402e"
    );
  });

  it("fails closed if an undeclared continuation schedule reaches the component", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    fixture.schedules.push({ id: "invented", columns: [], rows: [] });
    expect(() => renderToStaticMarkup(
      createElement(Form1702MX, { envelope: fixture })
    )).toThrow("uses fixed official schedule rows");
  });
});
