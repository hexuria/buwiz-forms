import { describe, expect, it } from "vitest";
import {
  formatAtcRate,
  requireOfficialCellCapacity,
  splitOfficialCombRows
} from "../src/forms/Form2551Q";

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
});
