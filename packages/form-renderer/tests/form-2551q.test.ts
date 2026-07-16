import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  formatAtcRate,
  OFFICIAL_2551Q_COMB_CAPACITIES,
  OfficialPaymentCombValue,
  OfficialPaymentOtherDescriptionValue,
  requireOfficialCellCapacity,
  splitOfficialCombRows
} from "../src/forms/Form2551Q";
import {
  OFFICIAL_2551Q_PDF417_PAGE_ONE_PATH,
  OFFICIAL_2551Q_PDF417_PAGE_TWO_PATH
} from "../src/forms/official2551QAssets";

const HERE = path.dirname(fileURLToPath(import.meta.url));

function pdf417ModuleDigest(path: string): { digest: string; blackModules: number } {
  const modules = Array<boolean>(120 * 7).fill(false);
  const command = /M(\d+) (\d+)h(\d+)v1H(\d+)z/g;
  const consumed: string[] = [];
  let blackModules = 0;

  for (const match of path.matchAll(command)) {
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

  expect(consumed.join("")).toBe(path);
  const bits = modules.map((module) => module ? "1" : "0").join("");
  return {
    digest: createHash("sha256").update(bits).digest("hex"),
    blackModules
  };
}

describe("2551Q ATC rate display", () => {
  it("does not expose binary floating-point artifacts", () => {
    expect(formatAtcRate(0.07)).toBe("7%");
    expect(formatAtcRate(0.025)).toBe("2.5%");
  });
});

describe("2551Q official comb capacity", () => {
  it("wraps an address at a word boundary without dropping characters", () => {
    const address = "53 SANTOL EXTENSION, NEW CABALAN, OLONGAPO CITY";
    const [first, second] = splitOfficialCombRows(
      address,
      40,
      31,
      "taxpayer.registered_address"
    );

    expect(first).toBe("53 SANTOL EXTENSION, NEW CABALAN, ");
    expect(second).toBe("OLONGAPO CITY");
    expect(first + second).toBe(address);
  });

  it("fails clearly when a value exceeds the official combined capacity", () => {
    expect(() =>
      splitOfficialCombRows("A".repeat(72), 40, 31, "registered address")
    ).toThrow("allows 71");
  });

  it("hard-splits an in-capacity long token without dropping characters", () => {
    const value = "A".repeat(41);
    const [first, second] = splitOfficialCombRows(
      value,
      40,
      31,
      "registered address"
    );

    expect(first).toBe("A".repeat(40));
    expect(second).toBe("A");
    expect(first + second).toBe(value);
  });

  it("returns in-capacity values unchanged", () => {
    expect(requireOfficialCellCapacity("12345678900000", 14, "TIN")).toBe(
      "12345678900000"
    );
  });

  it("pins the exact page-two and Part III capacities from the official PDF", () => {
    expect(OFFICIAL_2551Q_COMB_CAPACITIES).toEqual({
      pageTwoTaxpayerName: 26,
      payment: {
        otherDescription: 7,
        draweeBankOrAgency: 5,
        number: 7,
        date: 8,
        amountInteger: 12,
        amountFraction: 2
      }
    });
  });

  it.each([
    ["item-25-drawee", "", 5],
    ["item-25-number", "ABC", 7],
    ["item-25-date", "07172026", 8],
    ["item-26-drawee", "AB CD", 5]
  ])("keeps every official guide for fitting %s values", (field, value, cells) => {
    const markup = renderToStaticMarkup(
      createElement(OfficialPaymentCombValue, { field, value, cells })
    );

    expect(markup).toContain(`data-payment-field="${field}"`);
    expect(markup).toContain(`data-cell-capacity="${cells}"`);
    expect(markup).toContain('class="comb-value"');
    expect(markup).not.toContain('data-overflow-mode="plain"');
    expect(markup.match(/<span/g)).toHaveLength(cells + 2);
  });

  it("keeps Item 28's leading inset separate from its seven writable cells", () => {
    const markup = renderToStaticMarkup(
      createElement(OfficialPaymentOtherDescriptionValue, { value: "OTHERS7" })
    );

    expect(markup).toContain('data-payment-field="item-28-description"');
    expect(markup).toContain('data-cell-capacity="7"');
    expect(markup).toContain('class="payment-other-description-leading"');
    expect(markup).toContain('class="comb-value"');
    expect(markup).not.toContain('data-overflow-mode="plain"');
    expect(markup.match(/<span/g)).toHaveLength(10);
  });

  it("uses one untruncated plain box when a payment value exceeds its field capacity", () => {
    const value = "AB CDE";
    const markup = renderToStaticMarkup(
      createElement(OfficialPaymentCombValue, {
        field: "item-25-drawee",
        value,
        cells: OFFICIAL_2551Q_COMB_CAPACITIES.payment.draweeBankOrAgency
      })
    );

    expect(markup).toContain('data-cell-capacity="5"');
    expect(markup).toContain('data-overflow-mode="plain"');
    expect(markup).toContain(`aria-label="${value}"`);
    expect(markup).not.toContain('class="comb-value"');
    expect(markup).toContain(`>${value}</span>`);
  });
});

describe("2551Q official PDF417 module geometry", () => {
  it("preserves every reviewed module from both official PDF XObjects", () => {
    expect(pdf417ModuleDigest(OFFICIAL_2551Q_PDF417_PAGE_ONE_PATH)).toEqual({
      digest: "0b22b8418dc0dadb2043dd6022cb337e6fd60193b50105a3e7c47de3e565b9ca",
      blackModules: 491
    });
    expect(pdf417ModuleDigest(OFFICIAL_2551Q_PDF417_PAGE_TWO_PATH)).toEqual({
      digest: "b9257f08de39c25c64cbb7b8cbef835c060a5e347866c67b4cee558669f3233f",
      blackModules: 484
    });
  });

  it("embeds the losslessly extracted native official seal", () => {
    const sealPath = path.resolve(HERE, "../src/forms/assets/2551q-seal.png");
    expect(createHash("sha256").update(fs.readFileSync(sealPath)).digest("hex")).toBe(
      "7db0df0c022263481d219eebc2077631866f626521b4fe93967910a6a9422a4f"
    );
  });
});
