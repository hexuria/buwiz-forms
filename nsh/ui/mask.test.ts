// The masks' pure functions: TIN, money, percent, integers, the caret rule, dates.
import { expect, test } from "nsh/test";
import {
  applyMask, DATE_ERROR, digitsOnly, formatIso, formatLongDate, formatMDY, formatMoney, formatTin, formatTinDigits, intMask,
  MONEY_MASK, moneyMask, moneyPlain, parseTypedDate, PERCENT_MASK, reconcile, shownDate, TIN_MASK,
} from "./mask";

test("TIN: partial input groups as it goes, no trailing dash", () => {
  expect(formatTinDigits("")).toBe("");
  expect(formatTinDigits("0")).toBe("0");
  expect(formatTinDigits("000")).toBe("000");
  expect(formatTinDigits("0001")).toBe("000-1");
  expect(formatTinDigits("000123453")).toBe("000-123-453");
  expect(formatTinDigits("0001234530")).toBe("000-123-453-0");
  expect(formatTinDigits("00012345300000")).toBe("000-123-453-00000");
});

test("TIN: leading zeros stay, extra digits past 14 are cut", () => {
  expect(formatTinDigits("00000000000000")).toBe("000-000-000-00000");
  expect(formatTinDigits("0001234530000099")).toBe("000-123-453-00000");
  expect(applyMask("000", TIN_MASK)).toBe("000");
});

test("TIN: a paste with dashes or spaces keeps every digit", () => {
  expect(applyMask("000-123-453-00000", TIN_MASK)).toBe("000-123-453-00000");
  expect(applyMask(" 000 123 453 ", TIN_MASK)).toBe("000-123-453");
  expect(reconcile("", "000-123-453-00000", TIN_MASK)).toEqual({ text: "000-123-453-00000", caret: 17 });
  expect(digitsOnly("TIN: 000-123-453")).toBe("000123453");
});

test("TIN: typing at the end puts the caret after the inserted dash", () => {
  expect(reconcile("000", "0001", TIN_MASK)).toEqual({ text: "000-1", caret: 5 });
  expect(reconcile("000-12", "000-123", TIN_MASK)).toEqual({ text: "000-123", caret: 7 });
  expect(reconcile("000-123", "000-1234", TIN_MASK)).toEqual({ text: "000-123-4", caret: 9 });
});

test("TIN: typing and deleting in the middle keep the caret by the digit", () => {
  // "000-1|23" + "9" -> "000-192-3", caret after the 9.
  expect(reconcile("000-123", "000-1923", TIN_MASK)).toEqual({ text: "000-192-3", caret: 6 });
  // Backspace on "000-12|3": "000-13", caret after the 1.
  expect(reconcile("000-123", "000-13", TIN_MASK)).toEqual({ text: "000-13", caret: 5 });
  // Backspace on the dash "000-|123" deletes the 0 before it.
  expect(reconcile("000-123", "000123", TIN_MASK)).toEqual({ text: "001-23", caret: 2 });
  // A paste in the middle: "000-|123" + "456" -> digits flow on.
  expect(reconcile("000-123", "000-456123", TIN_MASK)).toEqual({ text: "000-456-123", caret: 7 });
});

test("money: thousands separators while typing, never a lost digit", () => {
  expect(applyMask("1250000", MONEY_MASK)).toBe("1,250,000");
  expect(applyMask("1250000.5", MONEY_MASK)).toBe("1,250,000.5");
  expect(applyMask("1,250,000.50", MONEY_MASK)).toBe("1,250,000.50");
  expect(applyMask("123456789012345", MONEY_MASK)).toBe("123,456,789,012,345");
  expect(applyMask("12.345", MONEY_MASK)).toBe("12.345");
  expect(applyMask("", MONEY_MASK)).toBe("");
});

test("money: one decimal point, letters dropped, leading zeros of the whole part dropped", () => {
  expect(applyMask("1.2.3", MONEY_MASK)).toBe("1.23");
  expect(applyMask("12a3", MONEY_MASK)).toBe("123");
  expect(applyMask("007", MONEY_MASK)).toBe("7");
  expect(applyMask("0", MONEY_MASK)).toBe("0");
  expect(applyMask("0.5", MONEY_MASK)).toBe("0.5");
  expect(applyMask("000.5", MONEY_MASK)).toBe("0.5");
  expect(applyMask(".5", MONEY_MASK)).toBe(".5");
  expect(applyMask("₱ 1,000", MONEY_MASK)).toBe("1,000");
});

test("money: a minus only where allowed, and only first", () => {
  expect(applyMask("-1500", MONEY_MASK)).toBe("1,500");
  const neg = moneyMask({ negative: true });
  expect(applyMask("-1500", neg)).toBe("-1,500");
  expect(applyMask("15-00", neg)).toBe("1,500");
  expect(applyMask("--15", neg)).toBe("-15");
});

test("money: the caret across inserted and removed separators", () => {
  expect(reconcile("123", "1234", MONEY_MASK)).toEqual({ text: "1,234", caret: 5 });
  expect(reconcile("1,234", "1,2345", MONEY_MASK)).toEqual({ text: "12,345", caret: 6 });
  // "1,2|34" + "9": "12,934", caret after the 9.
  expect(reconcile("1,234", "1,2934", MONEY_MASK)).toEqual({ text: "12,934", caret: 4 });
  // Backspace on "1,234|": the separator goes with the digit.
  expect(reconcile("1,234", "1,23", MONEY_MASK)).toEqual({ text: "123", caret: 3 });
  // Backspace on "1,|234" (the separator) deletes the 1.
  expect(reconcile("1,234", "1234", MONEY_MASK)).toEqual({ text: "234", caret: 0 });
  // A dropped leading zero: "0" then "05" reads "5", caret after it.
  expect(reconcile("0", "05", MONEY_MASK)).toEqual({ text: "5", caret: 1 });
  // A paste of a formatted amount into an empty box.
  expect(reconcile("", "1,250,000.00", MONEY_MASK)).toEqual({ text: "1,250,000.00", caret: 12 });
});

test("money: plain numbers out, the old formatMoney for showing", () => {
  expect(moneyPlain("1,250,000.50")).toBe("1250000.50");
  expect(moneyPlain("₱ -12")).toBe("-12");
  expect(moneyPlain("")).toBe("");
  expect(formatMoney("1250000")).toBe("1,250,000.00");
  expect(formatMoney("1234.5")).toBe("1,234.50");
  expect(formatMoney("-1234.5")).toBe("-1,234.50");
  expect(formatMoney("1,234")).toBe("1,234.00");
  expect(formatMoney("1234", true)).toBe("1,234");
  expect(formatMoney("1234.5", true)).toBe("1234.5");
  expect(formatMoney("")).toBe("");
  expect(formatMoney("abc")).toBe("abc");
  expect(formatMoney("0")).toBe("0.00");
});

test("percent and integers", () => {
  expect(applyMask("12.5", PERCENT_MASK)).toBe("12.5");
  expect(applyMask("1.2.5%", PERCENT_MASK)).toBe("1.25");
  expect(applyMask("0012", intMask())).toBe("0012");
  expect(applyMask("12345", intMask(4))).toBe("1234");
  expect(applyMask("1a2b", intMask())).toBe("12");
  expect(reconcile("12", "1x2", intMask())).toEqual({ text: "12", caret: 1 });
});

test("TIN from root and branch", () => {
  expect(formatTin("000123453", "00000")).toBe("000-123-453-00000");
  expect(formatTin("000123453")).toBe("000-123-453");
  expect(formatTin("1234")).toBe("1234");
});

test("dates: typed forms, real days only", () => {
  const dec1 = { y: 2020, m: 12, d: 1 };
  expect(parseTypedDate("12012020")).toEqual(dec1);
  expect(parseTypedDate("12/01/2020")).toEqual(dec1);
  expect(parseTypedDate("12-01-2020")).toEqual(dec1);
  expect(parseTypedDate("2020-12-01")).toEqual(dec1);
  expect(parseTypedDate("Dec 01 2020")).toEqual(dec1);
  expect(parseTypedDate("December 1, 2020")).toEqual(dec1);
  expect(parseTypedDate("02/30/2020")).toBeNull();
  expect(parseTypedDate("02/29/2020")).toEqual({ y: 2020, m: 2, d: 29 });
  expect(parseTypedDate("13/01/2020")).toBeNull();
  expect(parseTypedDate("")).toBeNull();
  expect(parseTypedDate("12/01")).toBeNull();
  expect(formatMDY(dec1)).toBe("12/01/2020");
  expect(formatLongDate(dec1)).toBe("Dec 1, 2020");
  expect(formatIso(dec1)).toBe("2020-12-01");
  expect(shownDate("12/01/2020")).toBe("Dec 1, 2020");
  expect(shownDate("12/0")).toBe("12/0");
  expect(DATE_ERROR).toContain("MM/DD/YYYY");
});
