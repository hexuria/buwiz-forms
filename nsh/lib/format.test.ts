import { describe, expect, test } from "nsh/test";
import { displayCode, formatTin, zipChoices } from "./format";

describe("formatTin", () => {
  test("a 9-digit root is grouped 3-3-3, leading zeros kept", () => {
    expect(formatTin("000123453")).toBe("000-123-453");
    expect(formatTin("009123456")).toBe("009-123-456");
  });
  test("a branch code follows the root after a dash", () => {
    expect(formatTin("000123453", "00000")).toBe("000-123-453-00000");
    expect(formatTin("009123456", "00001")).toBe("009-123-456-00001");
  });
  test("a missing branch leaves no trailing dash", () => {
    expect(formatTin("000123453", undefined)).toBe("000-123-453");
    expect(formatTin("000123453", null)).toBe("000-123-453");
    expect(formatTin("000123453", "")).toBe("000-123-453");
  });
  test("a root that is already formatted, or typed with spaces, reads the same", () => {
    expect(formatTin("000-123-453")).toBe("000-123-453");
    expect(formatTin("000-123-453", "00000")).toBe("000-123-453-00000");
    expect(formatTin(" 000 123 453 ")).toBe("000-123-453");
  });
  test("a short root stays as its digits, with no dashes", () => {
    expect(formatTin("1")).toBe("1");
    expect(formatTin("12345")).toBe("12345");
    expect(formatTin("12345678")).toBe("12345678");
    expect(formatTin("123-45")).toBe("12345");
  });
  test("a short root still takes its branch code", () => {
    expect(formatTin("12345", "00000")).toBe("12345-00000");
  });
  test("an empty or missing root is empty", () => {
    expect(formatTin("")).toBe("");
    expect(formatTin(null)).toBe("");
    expect(formatTin(undefined)).toBe("");
    expect(formatTin("abc")).toBe("");
  });
  test("a root of more than 9 digits shows the first nine; the branch is the caller's", () => {
    expect(formatTin("00012345300000")).toBe("000-123-453");
    expect(formatTin("00012345300000", "00000")).toBe("000-123-453-00000");
  });
  test("a number is read as its digits", () => {
    expect(formatTin(123456789)).toBe("123-456-789");
    expect(formatTin(123456789, "00000")).toBe("123-456-789-00000");
  });
  test("the demo taxpayer", () => {
    expect(formatTin("000123453", "00000")).toBe("000-123-453-00000");
  });
});

describe("displayCode", () => {
  test("a form code shows as it is", () => {
    expect(displayCode("2551Q")).toBe("2551Q");
    expect(displayCode("1601C")).toBe("1601C");
    expect(displayCode("1702-RT")).toBe("1702-RT");
  });
  test("no code is empty text", () => {
    expect(displayCode("")).toBe("");
    expect(displayCode(null)).toBe("");
    expect(displayCode(undefined)).toBe("");
  });
  test("a number is its text", () => {
    expect(displayCode(2307)).toBe("2307");
  });
});

describe("zipChoices", () => {
  test("places that share a code become one choice naming them all", () => {
    expect(zipChoices(["1100 - Central", "1100 - Piñahan", "1101 - Diliman", "1100 - Quezon City CPO"])).toEqual([
      { value: "1100", label: "1100 - Central, Piñahan, Quezon City CPO" },
      { value: "1101", label: "1101 - Diliman" },
    ]);
  });
  test("no two choices carry the same code", () => {
    const codes = zipChoices(["1226 - Bel-Air", "1100 - Central", "1226 - San Lorenzo", "1100 - Central"]).map((c) => c.value);
    expect(codes).toEqual(["1226", "1100"]);
  });
  test("a repeated place is named once, a bare code is its own label, an entry with no code is left out", () => {
    expect(zipChoices(["1100 - Central", "1100 - Central", "4000", " - Nowhere", ""])).toEqual([
      { value: "1100", label: "1100 - Central" },
      { value: "4000", label: "4000" },
    ]);
  });
});
