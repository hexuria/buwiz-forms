import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  AdaptiveCombValue,
  combCharacters,
  formatMoneyParts
} from "../src/components";

describe("official comb formatting", () => {
  it("right-aligns without truncating low-order characters", () => {
    expect(combCharacters("123", 5, "right")).toEqual([" ", " ", "1", "2", "3"]);
  });

  it("fails closed when a value exceeds the official cell capacity", () => {
    expect(() => combCharacters("123456", 5, "right")).toThrow(
      "requires 6 cells"
    );
  });

  it("uses a single plain text box when readable text exceeds the comb", () => {
    const value = "REGISTERED NAME LONGER THAN THE OFFICIAL CELLS";
    const markup = renderToStaticMarkup(
      createElement(AdaptiveCombValue, {
        value,
        cells: 20,
        fitToField: true,
        minFontSizePx: 8
      })
    );

    expect(markup).toContain('data-overflow-mode="plain"');
    expect(markup).toContain('data-adaptive-fit-state="pending"');
    expect(markup).toContain('data-adaptive-max-font-px="9.6"');
    expect(markup).toContain('data-adaptive-min-font-px="8"');
    expect(markup).toContain('data-adaptive-step-px="0.5"');
    expect(markup).toContain('style="font-size:9.6px"');
    expect(markup).toContain(`aria-label="${value}"`);
    expect(markup).not.toContain('class="comb-value"');
    expect(markup).not.toContain('font-size:4pt');
  });

  it("preserves right alignment after switching an overflowing comb to plain text", () => {
    const markup = renderToStaticMarkup(
      createElement(AdaptiveCombValue, {
        value: "123456789",
        cells: 5,
        align: "right",
        fitToField: true
      })
    );

    expect(markup).toContain("adaptive-align-right");
    expect(markup).toContain('data-overflow-mode="plain"');
  });

  it("rejects an invalid measured font range", () => {
    expect(() => renderToStaticMarkup(
      createElement(AdaptiveCombValue, {
        value: "REGISTERED NAME LONGER THAN FIVE CELLS",
        cells: 5,
        fitToField: true,
        maxFontSizePx: 8,
        minFontSizePx: 9
      })
    )).toThrow("positive ordered font range");
  });

  it("formats money without locale grouping characters", () => {
    expect(formatMoneyParts(1_234_567.89)).toEqual(["1234567", "89"]);
    expect(formatMoneyParts(-0)).toEqual(["0", "00"]);
  });

  it("rejects non-finite money values", () => {
    expect(() => formatMoneyParts(Number.NaN)).toThrow("finite");
  });
});
