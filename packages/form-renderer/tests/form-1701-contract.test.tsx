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
  it("pins the complete reviewed source pack and its mixed page geometries", () => {
    const source = JSON.parse(fs.readFileSync(
      path.resolve(HERE, "../references/1701-2018-source.json"),
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
      official_source_sha256: "19be91d78258eb7c255f2615610db2739f10c378f8ac97adc0887c1bf40d1b2e",
      page_count: 4,
      page_height_pt: 936,
      page_width_pt: 612
    });
    expect(source.form.reviewed_supporting_sources).toEqual([
      expect.objectContaining({
        field_count: 837,
        kind: "editable_xml",
        semantic_replay: "exact",
        source_sha256: "b168c7b3273d30a10f28f4653847519b876d5a88e77ed82911718a80f65c7827"
      }),
      expect.objectContaining({
        decrypted_extra_fields: ["frm1701:txtPg1I9Address2"],
        field_count: 838,
        kind: "encrypted_editable_xml",
        semantic_replay: "exact_after_decryption",
        source_sha256: "3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3"
      }),
      expect.objectContaining({
        kind: "attachment_pdf",
        page_count: 2,
        page_height_pt: 792,
        page_width_pt: 612,
        source_sha256: "e71799dc613c08d4c383fcd66bed83032b182ab43721c8665d7b608047766cad"
      }),
      expect.objectContaining({
        kind: "consolidated_pdf",
        page_count: 2,
        page_height_pt: 612,
        page_width_pt: 936,
        source_sha256: "eac0ce426cc57c473e24638accb14a978ddd54f8cf795cc4303f527088416871"
      })
    ]);
  });

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

  it("maps only Rust-owned calculations and keeps unentered cells as empty official guides", () => {
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
    expect(markup).toContain('data-field-key="schedule_4_1_taxpayer" data-field-mode="guided" data-cell-capacity="9"><span class="comb-value"');
    expect(markup).not.toContain("blank-amount-1701");
  });

  it("uses only source-visible plain and guided modes at the measured official capacities", () => {
    const fixture = structuredClone(normalFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));

    for (const fragment of [
      'data-field-mode="guided" data-cell-capacity="40"><span><b>8</b> Taxpayer’s Name',
      'class="labeled-comb-1701 address-1701" data-item-number="9" data-field-mode="guided" data-cell-capacity="71"',
      'data-field-mode="guided" data-cell-capacity="8"><span><b>10</b> Date of Birth',
      'data-field-mode="guided" data-cell-capacity="32"><span><b>11</b> Email Address',
      'data-field-key="payment_34_bank" data-field-mode="guided" data-cell-capacity="6"',
      'data-field-key="payment_34_number" data-field-mode="guided" data-cell-capacity="10"',
      'data-field-key="payment_34_date" data-field-mode="guided" data-cell-capacity="8"',
      'data-field-key="payment_34_amount" data-field-mode="guided" data-cell-capacity="9"',
      'data-field-key="payment_37_description" data-field-mode="guided" data-cell-capacity="7"',
      'data-field-key="schedule_5_taxpayer_1_description" data-field-mode="guided" data-cell-capacity="21"',
      'data-field-key="schedule_5_taxpayer_1_legal_basis" data-field-mode="guided" data-cell-capacity="9"',
      'data-field-key="schedule_6_taxpayer_1_amount" data-field-mode="plain"'
    ]) {
      expect(markup).toContain(fragment);
    }

    expect(markup).not.toContain('data-field-key="payment_36_bank"');
    expect(markup).toContain('class="row-description-1701" data-field-mode="plain"><span class="adaptive-plain-value');
    expect(markup).toContain('class="inline-description-1701" data-field-mode="plain"><span class="adaptive-plain-value');

    const css = fs.readFileSync(path.resolve(HERE, "../src/forms/Form1701.css"), "utf8");
    expect(css).not.toContain("repeating-linear-gradient");
    expect(css).not.toContain("blank-amount-1701");
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

  it("preserves the official Schedule 1 employer name and TIN partitions", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    const employerName = (fixture.fields.employer_1_name as { value: string }).value;
    expect(markup).toContain("a. Name of Employer");
    expect(markup.match(/class="employer-tin-label-1701"/g)).toHaveLength(2);
    expect(markup.match(/Employer’s TIN/g)).toHaveLength(2);
    expect(markup).toContain("DO NOT enter Centavos; 49 Centavos or Less drop down; 50 or more round up");
    expect(markup).toContain(`aria-label="${employerName}"`);
    expect(markup).toContain("To Part V Schedule 2 Item 4A and Part VII Item 5A");
    expect(markup).toContain("To Part V Schedule 2 Item 4B and Part VII Item 5B");
  });

  it("preserves the official Schedule 3.A row-group partitions", () => {
    const fixture = structuredClone(longFixture) as RenderEnvelope;
    const markup = renderToStaticMarkup(createElement(Form1701, { envelope: fixture }));
    const schedule = markup.match(/<section class="paired-section-1701 compact-table-1701 schedule-three-a-1701">[\s\S]*?<\/section>/)?.[0];
    expect(schedule).toBeDefined();
    expect(schedule).toContain("If graduated rates, fill in items 8 to 24; if 8% flat income tax rate, fill in items 25 to 30");
    expect(schedule).toContain("Less: Deductions Allowable under Existing Laws");
    expect(schedule).toContain("or-subtitle-1701\">OR");
    expect(schedule).toContain("Add: Other Non-Operating Income (specify below)");
    expect(schedule).toContain("A VALID NON-OPERATING INCOME DESCRIPTION LONGER THAN THE OFFICIAL COMB CAPACITY");
    expect(schedule).toContain("To Part VI Item 1");
    expect(schedule).not.toContain("paired-head-1701");
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
