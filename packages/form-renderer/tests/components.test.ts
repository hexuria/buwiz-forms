import { describe, expect, it } from "vitest";
import { combCharacters, formatMoneyParts } from "../src/components";

describe("official comb formatting", () => {
  it("right-aligns without truncating low-order characters", () => {
    expect(combCharacters("123", 5, "right")).toEqual([" ", " ", "1", "2", "3"]);
  });

  it("fails closed when a value exceeds the official cell capacity", () => {
    expect(() => combCharacters("123456", 5, "right")).toThrow(
      "requires 6 cells"
    );
  });

  it("formats money without locale grouping characters", () => {
    expect(formatMoneyParts(1_234_567.89)).toEqual(["1234567", "89"]);
    expect(formatMoneyParts(-0)).toEqual(["0", "00"]);
  });

  it("rejects non-finite money values", () => {
    expect(() => formatMoneyParts(Number.NaN)).toThrow("finite");
  });
});
