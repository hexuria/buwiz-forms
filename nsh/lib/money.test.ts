import { describe, expect, test } from "nsh/test";
import { formatMoney, wholePesosOf } from "./money";

describe("formatMoney", () => {
  test("groups thousands and shows two decimals", () => {
    expect(formatMoney("1234567.5")).toBe("1,234,567.50");
    expect(formatMoney("1234.56")).toBe("1,234.56");
    expect(formatMoney("999.99")).toBe("999.99");
  });
  test("a whole amount gets .00, and zero too", () => {
    expect(formatMoney("1234")).toBe("1,234.00");
    expect(formatMoney("0")).toBe("0.00");
  });
  test("a number is taken as its text", () => {
    expect(formatMoney(1234.5)).toBe("1,234.50");
  });
  test("commas, the peso sign and spaces typed in are read as nothing", () => {
    expect(formatMoney("1,234.50")).toBe("1,234.50");
    expect(formatMoney("₱1,234.50")).toBe("1,234.50");
    expect(formatMoney(" 1 234.5 ")).toBe("1,234.50");
  });
  test("a fraction shows two decimals; a bare point reads as a number", () => {
    expect(formatMoney("12.5")).toBe("12.50");
    expect(formatMoney("0.1")).toBe("0.10");
    expect(formatMoney(".5")).toBe("0.50");
    expect(formatMoney("5.")).toBe("5.00");
  });
  test("negative amounts keep their sign and their commas", () => {
    expect(formatMoney("-1234.5")).toBe("-1,234.50");
    expect(formatMoney("-1234567")).toBe("-1,234,567.00");
  });
  test("empty and unreadable text stays as it is", () => {
    expect(formatMoney("")).toBe("");
    expect(formatMoney("   ")).toBe("   ");
    expect(formatMoney("abc")).toBe("abc");
    expect(formatMoney("12.3.4")).toBe("12.3.4");
  });
  test("a whole-peso box shows no centavos", () => {
    expect(formatMoney("1234567", true)).toBe("1,234,567");
    expect(formatMoney("-1234", true)).toBe("-1,234");
    expect(formatMoney("1234.00", true)).toBe("1,234");
    expect(formatMoney("0", true)).toBe("0");
    expect(formatMoney("", true)).toBe("");
  });
  test("a fraction in a whole-peso box stays as typed, so its message can explain it", () => {
    expect(formatMoney("12.5", true)).toBe("12.5");
    expect(formatMoney("1,234.50", true)).toBe("1,234.50");
  });
  test("old behaviour kept: a tiny negative fraction shows a minus sign on zero", () => {
    expect(formatMoney("-0.004")).toBe("-0.00");
  });
  test("old behaviour kept: a word that reads as Infinity gets an undefined fraction", () => {
    expect(formatMoney("Infinity")).toBe("Infinity.undefined");
  });
  test("old behaviour kept: commas are dropped wherever they fall", () => {
    expect(formatMoney("1,2,3")).toBe("123.00");
  });
});

describe("wholePesosOf", () => {
  test("rounds half up to whole pesos and says whether centavos were dropped", () => {
    expect(wholePesosOf("1,234.50")).toEqual({ amount: 1235, rounded: true });
    expect(wholePesosOf("2.4")).toEqual({ amount: 2, rounded: true });
    expect(wholePesosOf("2.5")).toEqual({ amount: 3, rounded: true });
    expect(wholePesosOf("2")).toEqual({ amount: 2, rounded: false });
  });
  test("the peso sign, commas and spaces are read as nothing", () => {
    expect(wholePesosOf("₱5,000")).toEqual({ amount: 5000, rounded: false });
    expect(wholePesosOf(" 1 234 ")).toEqual({ amount: 1234, rounded: false });
  });
  test("a number is taken as its text", () => {
    expect(wholePesosOf(99.6)).toEqual({ amount: 100, rounded: true });
  });
  test("a blank, a zero, an amount that rounds to zero and unreadable text give null", () => {
    expect(wholePesosOf("")).toBeNull();
    expect(wholePesosOf("   ")).toBeNull();
    expect(wholePesosOf(null)).toBeNull();
    expect(wholePesosOf(undefined)).toBeNull();
    expect(wholePesosOf("0")).toBeNull();
    expect(wholePesosOf("0.4")).toBeNull();
    expect(wholePesosOf("abc")).toBeNull();
    expect(wholePesosOf("Infinity")).toBeNull();
  });
  test("a negative amount rounds with Math.round (half toward plus infinity)", () => {
    expect(wholePesosOf("-2.5")).toEqual({ amount: -2, rounded: true });
    expect(wholePesosOf("-1,234.6")).toEqual({ amount: -1235, rounded: true });
  });
});
